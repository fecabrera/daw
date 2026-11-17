//! Linux editor timer and descriptor services, on the native main thread.
use super::*;
use daw_plugin_vst3::sys::Steinberg::Linux::*;
use std::time::{Duration, Instant};
struct Timer {
    handler: ComPtr<ITimerHandler>,
    period: Duration,
    next: Instant,
}
pub(super) struct RunLoop {
    events: RefCell<Vec<(ComPtr<IEventHandler>, i32)>>,
    timers: RefCell<Vec<Timer>>,
}
impl RunLoop {
    pub fn new() -> Self {
        Self {
            events: RefCell::new(Vec::with_capacity(128)),
            timers: RefCell::new(Vec::with_capacity(128)),
        }
    }
    pub fn pump(&self) {
        let mut ready = [super::super::linux::PollFd {
            fd: -1,
            events: 1,
            revents: 0,
        }; 128];
        let mut handlers: [Option<ComPtr<IEventHandler>>; 128] = std::array::from_fn(|_| None);
        let count;
        {
            let events = self.events.borrow();
            count = events.len();
            for ((out, handler), (event, fd)) in
                ready.iter_mut().zip(&mut handlers).zip(events.iter())
            {
                out.fd = *fd;
                *handler = Some(event.clone());
            }
        }
        if super::super::linux::ready(&mut ready[..count]) {
            for (fd, handler) in ready[..count].iter().zip(handlers.iter()) {
                if fd.revents != 0
                    && let Some(handler) = handler
                    && self.events.borrow().iter().any(|(current, number)| {
                        current.as_ptr() == handler.as_ptr() && *number == fd.fd
                    })
                {
                    unsafe {
                        handler.onFDIsSet(fd.fd);
                    }
                }
            }
        }
        let now = Instant::now();
        let mut due: [Option<ComPtr<ITimerHandler>>; 128] = std::array::from_fn(|_| None);
        {
            let mut timers = self.timers.borrow_mut();
            for (out, timer) in due.iter_mut().zip(timers.iter_mut()) {
                if now >= timer.next {
                    *out = Some(timer.handler.clone());
                    timer.next = now + timer.period;
                }
            }
        }
        for handler in due.iter().flatten() {
            if self
                .timers
                .borrow()
                .iter()
                .any(|t| t.handler.as_ptr() == handler.as_ptr())
            {
                unsafe {
                    handler.onTimer();
                }
            }
        }
    }
}
unsafe fn retain_unknown(pointer: *mut FUnknown) {
    unsafe {
        ((*(*pointer).vtbl).addRef)(pointer);
    }
}
impl IRunLoopTrait for Bridge {
    unsafe fn registerEventHandler(
        &self,
        handler: *mut IEventHandler,
        fd: FileDescriptor,
    ) -> tresult {
        if handler.is_null() || fd < 0 || thread_key() != self.shared().main_thread {
            return kInvalidArgument;
        }
        let mut events = self.run_loop.events.borrow_mut();
        if events.len() == 128
            || events
                .iter()
                .any(|(h, n)| h.as_ptr() == handler && *n == fd)
        {
            return kResultFalse;
        }
        unsafe {
            retain_unknown(handler.cast());
        }
        events.push((
            unsafe { ComPtr::from_raw(handler) }.expect("checked handler"),
            fd,
        ));
        kResultOk
    }
    unsafe fn unregisterEventHandler(&self, handler: *mut IEventHandler) -> tresult {
        if thread_key() != self.shared().main_thread {
            return kInvalidArgument;
        }
        let mut events = self.run_loop.events.borrow_mut();
        let count = events.len();
        events.retain(|(h, _)| h.as_ptr() != handler);
        if events.len() < count {
            kResultOk
        } else {
            kResultFalse
        }
    }
    unsafe fn registerTimer(
        &self,
        handler: *mut ITimerHandler,
        milliseconds: TimerInterval,
    ) -> tresult {
        if handler.is_null()
            || milliseconds == 0
            || milliseconds > u64::from(u32::MAX)
            || thread_key() != self.shared().main_thread
        {
            return kInvalidArgument;
        }
        let mut timers = self.run_loop.timers.borrow_mut();
        if timers.len() == 128 || timers.iter().any(|t| t.handler.as_ptr() == handler) {
            return kResultFalse;
        }
        let period = Duration::from_millis(milliseconds);
        unsafe {
            retain_unknown(handler.cast());
        }
        timers.push(Timer {
            handler: unsafe { ComPtr::from_raw(handler) }.expect("checked handler"),
            period,
            next: Instant::now() + period,
        });
        kResultOk
    }
    unsafe fn unregisterTimer(&self, handler: *mut ITimerHandler) -> tresult {
        if thread_key() != self.shared().main_thread {
            return kInvalidArgument;
        }
        let mut timers = self.run_loop.timers.borrow_mut();
        let count = timers.len();
        timers.retain(|t| t.handler.as_ptr() != handler);
        if timers.len() < count {
            kResultOk
        } else {
            kResultFalse
        }
    }
}
