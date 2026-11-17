//! Bounded controller/processor messaging. Queue ownership and final message
//! releases stay off audio; no controller callback runs on the audio thread.
use super::*;
const CAPACITY: usize = 256;
struct Link {
    shared: *const Shared,
    processor: ComPtr<IConnectionPoint>,
    controller: ComPtr<IConnectionPoint>,
    to_processor: RefCell<rtrb::Producer<ComPtr<IMessage>>>,
    processor_in: UnsafeCell<rtrb::Consumer<ComPtr<IMessage>>>,
    to_controller: UnsafeCell<rtrb::Producer<ComPtr<IMessage>>>,
    controller_in: RefCell<rtrb::Consumer<ComPtr<IMessage>>>,
    retire: UnsafeCell<rtrb::Producer<ComPtr<IMessage>>>,
    retired: RefCell<rtrb::Consumer<ComPtr<IMessage>>>,
}
impl Link {
    fn shared(&self) -> &Shared {
        unsafe { &*self.shared }
    }
    unsafe fn borrow_message(message: *mut IMessage) -> Option<ComPtr<IMessage>> {
        if message.is_null() {
            return None;
        }
        let unknown = message.cast::<FUnknown>();
        unsafe {
            ((*(*unknown).vtbl).addRef)(unknown);
            ComPtr::from_raw(message)
        }
    }
    fn quiescent(&self) -> bool {
        let s = self.shared();
        s.returned.load(Ordering::Acquire)
            || s.suspend.load(Ordering::Acquire) && s.suspended.load(Ordering::Acquire)
    }
    pub fn main(&self) {
        while let Ok(message) = self.controller_in.borrow_mut().pop() {
            if unsafe { self.controller.notify(message.as_ptr()) } != kResultOk {
                self.shared().fault.store(true, Ordering::Release);
            }
        }
        while let Ok(message) = self.retired.borrow_mut().pop() {
            drop(message);
        }
    }
    pub fn audio(&self) {
        // Reserve a retirement slot before consuming a command. The controller
        // notification queue has a separate producer and cannot take this slot.
        let retire = unsafe { &mut *self.retire.get() };
        let input = unsafe { &mut *self.processor_in.get() };
        for _ in 0..CAPACITY {
            if retire.slots() == 0 {
                break;
            }
            let Ok(message) = input.pop() else {
                break;
            };
            if unsafe { self.processor.notify(message.as_ptr()) } != kResultOk {
                self.shared().fault.store(true, Ordering::Release);
            }
            if let Err(error) = retire.push(message) {
                // Only this audio thread produces retirement entries. The
                // reserved capacity cannot shrink between slots() and push().
                std::mem::forget(error);
                self.shared().fault.store(true, Ordering::Release);
            }
        }
    }
}
struct Proxy {
    link: Rc<Link>,
    controller_bound: bool,
}
impl Class for Proxy {
    type Interfaces = (IConnectionPoint,);
}
impl IConnectionPointTrait for Proxy {
    unsafe fn connect(&self, _: *mut IConnectionPoint) -> tresult {
        kResultOk
    }
    unsafe fn disconnect(&self, _: *mut IConnectionPoint) -> tresult {
        kResultOk
    }
    unsafe fn notify(&self, message: *mut IMessage) -> tresult {
        if message.is_null() {
            return kInvalidArgument;
        }
        let shared = self.link.shared();
        let main = thread_key() == shared.main_thread;
        if self.controller_bound {
            if main {
                return unsafe { self.link.controller.notify(message) };
            }
            if thread_key() != shared.audio_thread.load(Ordering::Acquire) {
                return kResultFalse;
            }
            let queue = unsafe { &mut *self.link.to_controller.get() };
            if queue.slots() == 0 {
                return kResultFalse;
            }
            let Some(message) = (unsafe { Link::borrow_message(message) }) else {
                return kInvalidArgument;
            };
            if queue.push(message).is_ok() {
                kResultOk
            } else {
                kResultFalse
            }
        } else {
            if !main {
                return kResultFalse;
            }
            if self.link.quiescent() {
                return unsafe { self.link.processor.notify(message) };
            }
            let mut queue = self.link.to_processor.borrow_mut();
            if queue.slots() == 0 {
                return kResultFalse;
            }
            let Some(message) = (unsafe { Link::borrow_message(message) }) else {
                return kInvalidArgument;
            };
            if queue.push(message).is_ok() {
                shared.flush.store(true, Ordering::Release);
                kResultOk
            } else {
                kResultFalse
            }
        }
    }
}
pub(super) struct Connections {
    link: Rc<Link>,
    component_proxy: ComPtr<IConnectionPoint>,
    controller_proxy: ComPtr<IConnectionPoint>,
}
impl Connections {
    pub fn connect(
        processor: ComPtr<IConnectionPoint>,
        controller: ComPtr<IConnectionPoint>,
        shared: &Shared,
    ) -> Result<Self> {
        let (tp, pi) = rtrb::RingBuffer::new(CAPACITY);
        let (tc, ci) = rtrb::RingBuffer::new(CAPACITY);
        let (retire, retired) = rtrb::RingBuffer::new(CAPACITY);
        let link = Rc::new(Link {
            shared,
            processor,
            controller,
            to_processor: RefCell::new(tp),
            processor_in: UnsafeCell::new(pi),
            to_controller: UnsafeCell::new(tc),
            controller_in: RefCell::new(ci),
            retire: UnsafeCell::new(retire),
            retired: RefCell::new(retired),
        });
        let component_proxy = ComWrapper::new(Proxy {
            link: link.clone(),
            controller_bound: true,
        })
        .to_com_ptr::<IConnectionPoint>()
        .expect("declared connection proxy");
        let controller_proxy = ComWrapper::new(Proxy {
            link: link.clone(),
            controller_bound: false,
        })
        .to_com_ptr::<IConnectionPoint>()
        .expect("declared connection proxy");
        check(
            unsafe { link.processor.connect(component_proxy.as_ptr()) },
            "component connect",
        )?;
        if let Err(error) = check(
            unsafe { link.controller.connect(controller_proxy.as_ptr()) },
            "controller connect",
        ) {
            unsafe {
                link.processor.disconnect(component_proxy.as_ptr());
            }
            return Err(error);
        }
        Ok(Self {
            link,
            component_proxy,
            controller_proxy,
        })
    }
    pub fn main(&self) {
        self.link.main();
    }
    pub fn audio_pointer(&self) -> *const c_void {
        Rc::as_ptr(&self.link).cast()
    }
    /// Only the unique processor lease calls this; the main broker retains Link.
    pub unsafe fn audio(pointer: *const c_void) {
        if !pointer.is_null() {
            unsafe { &*pointer.cast::<Link>() }.audio();
        }
    }
    pub fn disconnect(&self) {
        unsafe {
            self.link
                .processor
                .disconnect(self.component_proxy.as_ptr());
            self.link
                .controller
                .disconnect(self.controller_proxy.as_ptr());
        }
        self.main();
    }
}
