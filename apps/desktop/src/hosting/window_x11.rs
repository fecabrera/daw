//! Separate X11/XWayland parent, serviced on GTK's main thread.
use std::{
    cell::Cell,
    ffi::{CString, c_char, c_int, c_long, c_ulong, c_void},
    ptr,
};
type Display = c_void;
#[repr(C)]
#[derive(Clone, Copy)]
struct Client {
    kind: c_int,
    serial: c_ulong,
    sent: c_int,
    display: *mut Display,
    window: c_ulong,
    message: c_ulong,
    format: c_int,
    data: [c_long; 5],
}
#[repr(C)]
union Event {
    kind: c_int,
    client: Client,
    configure: Configure,
    padding: [c_long; 24],
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Configure {
    kind: c_int,
    serial: c_ulong,
    sent: c_int,
    display: *mut Display,
    event: c_ulong,
    window: c_ulong,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border: c_int,
    above: c_ulong,
    overridden: c_int,
}
struct Api {
    _library: libloading::Library,
    close: unsafe extern "C" fn(*mut Display) -> c_int,
    destroy: unsafe extern "C" fn(*mut Display, c_ulong) -> c_int,
    resize: unsafe extern "C" fn(*mut Display, c_ulong, u32, u32) -> c_int,
    map: unsafe extern "C" fn(*mut Display, c_ulong) -> c_int,
    flush: unsafe extern "C" fn(*mut Display) -> c_int,
    pending: unsafe extern "C" fn(*mut Display) -> c_int,
    next: unsafe extern "C" fn(*mut Display, *mut Event) -> c_int,
}
pub struct Window {
    api: Api,
    display: *mut Display,
    handle: c_ulong,
    delete: c_ulong,
    visible: bool,
    size: Cell<(u32, u32)>,
    focused: Cell<bool>,
}
impl Window {
    pub fn new(title: &str) -> Result<Self, String> {
        unsafe {
            let library = libloading::Library::new("libX11.so.6").map_err(|e| e.to_string())?;
            macro_rules! function {
                ($name:literal,$ty:ty) => {
                    *library
                        .get::<$ty>(concat!($name, "\0").as_bytes())
                        .map_err(|e| e.to_string())?
                };
            }
            let open = function!(
                "XOpenDisplay",
                unsafe extern "C" fn(*const c_char) -> *mut Display
            );
            let root = function!(
                "XDefaultRootWindow",
                unsafe extern "C" fn(*mut Display) -> c_ulong
            );
            let create = function!(
                "XCreateSimpleWindow",
                unsafe extern "C" fn(
                    *mut Display,
                    c_ulong,
                    c_int,
                    c_int,
                    u32,
                    u32,
                    u32,
                    c_ulong,
                    c_ulong,
                ) -> c_ulong
            );
            let name = function!(
                "XStoreName",
                unsafe extern "C" fn(*mut Display, c_ulong, *const c_char) -> c_int
            );
            let atom = function!(
                "XInternAtom",
                unsafe extern "C" fn(*mut Display, *const c_char, c_int) -> c_ulong
            );
            let protocols = function!(
                "XSetWMProtocols",
                unsafe extern "C" fn(*mut Display, c_ulong, *mut c_ulong, c_int) -> c_int
            );
            let select = function!(
                "XSelectInput",
                unsafe extern "C" fn(*mut Display, c_ulong, c_long) -> c_int
            );
            let api = Api {
                close: function!("XCloseDisplay", unsafe extern "C" fn(*mut Display) -> c_int),
                destroy: function!(
                    "XDestroyWindow",
                    unsafe extern "C" fn(*mut Display, c_ulong) -> c_int
                ),
                resize: function!(
                    "XResizeWindow",
                    unsafe extern "C" fn(*mut Display, c_ulong, u32, u32) -> c_int
                ),
                map: function!(
                    "XMapRaised",
                    unsafe extern "C" fn(*mut Display, c_ulong) -> c_int
                ),
                flush: function!("XFlush", unsafe extern "C" fn(*mut Display) -> c_int),
                pending: function!("XPending", unsafe extern "C" fn(*mut Display) -> c_int),
                next: function!(
                    "XNextEvent",
                    unsafe extern "C" fn(*mut Display, *mut Event) -> c_int
                ),
                _library: library,
            };
            let display = open(ptr::null());
            if display.is_null() {
                return Err(
                    "Native plugin editors require an available X11 or XWayland display".into(),
                );
            }
            let handle = create(display, root(display), 0, 0, 640, 480, 0, 0, 0);
            if handle == 0 {
                (api.close)(display);
                return Err("Cannot create X11 plugin editor parent".into());
            }
            let title = CString::new(title.replace('\0', " ")).map_err(|e| e.to_string())?;
            name(display, handle, title.as_ptr());
            select(display, handle, (1 << 17) | (1 << 21));
            let mut delete = atom(display, c"WM_DELETE_WINDOW".as_ptr(), 0);
            protocols(display, handle, &mut delete, 1);
            Ok(Self {
                api,
                display,
                handle,
                delete,
                visible: true,
                size: Cell::new((640, 480)),
                focused: Cell::new(false),
            })
        }
    }
    pub fn parent(&self) -> daw_plugin_host::NativeParent {
        daw_plugin_host::NativeParent::X11(self.handle)
    }
    pub fn resize(&self, width: u32, height: u32) {
        self.size.set((width, height));
        unsafe {
            (self.api.resize)(self.display, self.handle, width, height);
            (self.api.flush)(self.display);
        }
    }
    pub fn size(&self) -> (u32, u32) {
        self.size.get()
    }
    pub fn scale(&self, fallback: f64) -> f64 {
        fallback
    }
    pub fn focused(&self) -> bool {
        self.focused.get()
    }
    pub fn resizable(&self, _allowed: bool) {}
    pub fn show(&self) {
        unsafe {
            (self.api.map)(self.display, self.handle);
            (self.api.flush)(self.display);
        }
    }
    pub fn visible(&mut self) -> bool {
        unsafe {
            for _ in 0..256 {
                if (self.api.pending)(self.display) == 0 {
                    break;
                }
                let mut event = Event { padding: [0; 24] };
                (self.api.next)(self.display, &mut event);
                if event.kind == 22 {
                    self.size.set((
                        event.configure.width.max(1) as u32,
                        event.configure.height.max(1) as u32,
                    ));
                }
                if event.kind == 9 {
                    self.focused.set(true);
                }
                if event.kind == 10 {
                    self.focused.set(false);
                }
                if event.kind == 17
                    || event.kind == 33 && event.client.data[0] as c_ulong == self.delete
                {
                    self.visible = false;
                }
            }
        }
        self.visible
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            (self.api.destroy)(self.display, self.handle);
            (self.api.close)(self.display);
        }
    }
}
