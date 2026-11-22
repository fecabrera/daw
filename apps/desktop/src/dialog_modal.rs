//! Native modal ownership for eframe's secondary dialog viewports.
pub(super) use platform::Modal;

#[cfg(target_os = "macos")]
mod platform {
    use block2::RcBlock;
    use eframe::native_viewport::Window;
    use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
    use objc2_app_kit::{
        NSEvent, NSEventMask, NSEventType, NSView, NSWindow, NSWindowOrderingMode,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{cell::RefCell, ptr::NonNull, rc::Rc};

    #[derive(Default)]
    pub(crate) struct Modal(Option<Owner>);

    struct Owner {
        root: Retained<NSWindow>,
        dialog: Rc<RefCell<Option<Retained<NSWindow>>>>,
        monitor: Retained<AnyObject>,
    }

    impl Modal {
        pub(crate) fn update(&mut self, root: Option<&Window>, dialog: Option<&Window>) {
            let Some(root) = root.and_then(native_window) else {
                self.0 = None;
                return;
            };
            if self
                .0
                .as_ref()
                .is_none_or(|owner| !same(&owner.root, &root))
            {
                self.0 = Owner::new(root);
            }
            if let Some(owner) = &mut self.0 {
                owner.set_dialog(dialog.and_then(native_window));
                if owner.root.isKeyWindow() {
                    bring_forward(owner.dialog.borrow().as_deref());
                }
            }
        }
    }

    impl Owner {
        fn new(root: Retained<NSWindow>) -> Option<Self> {
            let dialog = Rc::new(RefCell::new(None::<Retained<NSWindow>>));
            let owner_window = root.clone();
            let active = dialog.clone();
            let handler = RcBlock::new(move |event: NonNull<NSEvent>| {
                // SAFETY: AppKit passes a live event for this invocation.
                let event_ref = unsafe { event.as_ref() };
                let targets_owner = MainThreadMarker::new()
                    .and_then(|main| event_ref.window(main))
                    .is_some_and(|window| same(&window, &owner_window));
                if !targets_owner {
                    return event.as_ptr();
                }
                if owner_window.isKeyWindow()
                    || matches!(
                        event_ref.r#type(),
                        NSEventType::LeftMouseDown
                            | NSEventType::RightMouseDown
                            | NSEventType::OtherMouseDown
                    )
                {
                    bring_forward(active.borrow().as_deref());
                }
                std::ptr::null_mut()
            });
            // SAFETY: The block returns the supplied live event or null. AppKit copies
            // the block. The monitor is removed before its retained windows are released.
            let monitor = unsafe {
                NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::Any, &handler)
            }?;
            Some(Self {
                root,
                dialog,
                monitor,
            })
        }

        fn set_dialog(&self, next: Option<Retained<NSWindow>>) {
            let old = {
                let mut current = self.dialog.borrow_mut();
                if match (current.as_deref(), next.as_deref()) {
                    (None, None) => true,
                    (Some(a), Some(b)) => same(a, b),
                    _ => false,
                } {
                    return;
                }
                std::mem::replace(&mut *current, next.clone())
            };
            if let Some(old) = old {
                self.root.removeChildWindow(&old);
            }
            if let Some(next) = next {
                // SAFETY: Winit supplies two distinct, live top-level windows. The host
                // owns the link until the dialog changes or is dismissed.
                unsafe {
                    self.root
                        .addChildWindow_ordered(&next, NSWindowOrderingMode::Above)
                };
            }
        }
    }

    impl Drop for Owner {
        fn drop(&mut self) {
            // SAFETY: This is the token returned by AppKit and it is removed only once.
            unsafe { NSEvent::removeMonitor(&self.monitor) };
            self.set_dialog(None);
        }
    }

    fn bring_forward(dialog: Option<&NSWindow>) {
        if let Some(dialog) = dialog {
            let sheet = dialog.attachedSheet();
            let target = sheet.as_deref().unwrap_or(dialog);
            if target.isVisible() && !target.isKeyWindow() {
                target.makeKeyAndOrderFront(None);
            }
        }
    }

    fn same(left: &NSWindow, right: &NSWindow) -> bool {
        std::ptr::eq(left, right)
    }

    fn native_window(window: &Window) -> Option<Retained<NSWindow>> {
        let RawWindowHandle::AppKit(handle) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: Winit owns a live AppKit view. The settings host runs on the main thread.
        unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }?.window()
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use eframe::native_viewport::Window;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::{
        Foundation::HWND,
        UI::{
            Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
            WindowsAndMessaging::{GWLP_HWNDPARENT, SetWindowLongPtrW},
        },
    };

    #[derive(Default)]
    pub(crate) struct Modal {
        owner: Option<Owner>,
    }

    struct Owner {
        hwnd: HWND,
        was_enabled: bool,
        dialog: Option<(HWND, isize)>,
    }

    impl Modal {
        pub(crate) fn update(&mut self, root: Option<&Window>, dialog: Option<&Window>) {
            let Some(hwnd) = root.and_then(native_window) else {
                self.owner = None;
                return;
            };
            if self.owner.as_ref().is_none_or(|owner| owner.hwnd != hwnd) {
                self.owner = Some(Owner::new(hwnd));
            }
            if let Some(owner) = &mut self.owner {
                owner.set_dialog(dialog.and_then(native_window));
            }
        }
    }

    impl Owner {
        fn new(hwnd: HWND) -> Self {
            // SAFETY: Winit supplies a live top-level HWND on this UI thread.
            let was_enabled = unsafe { IsWindowEnabled(hwnd) != 0 };
            unsafe { EnableWindow(hwnd, 0) };
            Self {
                hwnd,
                was_enabled,
                dialog: None,
            }
        }

        fn set_dialog(&mut self, dialog: Option<HWND>) {
            if self.dialog.map(|(hwnd, _)| hwnd) == dialog {
                return;
            }
            // SAFETY: Windows from the host live on this thread. IsWindow guards
            // retirement, since Winit can destroy a viewport between host updates.
            unsafe {
                if let Some((old, previous)) = self.dialog.take()
                    && windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(old) != 0
                {
                    SetWindowLongPtrW(old, GWLP_HWNDPARENT, previous);
                }
                self.dialog = dialog.map(|hwnd| {
                    let previous = SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, self.hwnd);
                    (hwnd, previous)
                });
            }
        }
    }

    impl Drop for Owner {
        fn drop(&mut self) {
            self.set_dialog(None);
            // SAFETY: Restore the main window only if it still exists.
            unsafe {
                if windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(self.hwnd) != 0 {
                    EnableWindow(self.hwnd, i32::from(self.was_enabled));
                }
            }
        }
    }

    fn native_window(window: &Window) -> Option<HWND> {
        let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        Some(handle.hwnd.get())
    }
}
