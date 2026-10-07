use daw_ui::{DawUi, FileAction};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString, ns_string};
use std::sync::mpsc;

struct MenuTargetIvars {
    sender: mpsc::Sender<FileAction>,
    context: egui::Context,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements. This target stays on the
    // AppKit main thread and does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MenuTargetIvars]
    struct DawFileMenuTarget;

    // SAFETY: NSObjectProtocol has no additional requirements.
    unsafe impl NSObjectProtocol for DawFileMenuTarget {}

    impl DawFileMenuTarget {
        // SAFETY: The selector takes an NSMenuItem sender and returns void.
        #[unsafe(method(performFileAction:))]
        fn perform_file_action(&self, item: &NSMenuItem) {
            if let Ok(index) = usize::try_from(item.tag())
                && let Some(action) = FileAction::ALL.get(index)
            {
                let _ = self.ivars().sender.send(*action);
                self.ivars().context.request_repaint();
            }
        }
    }
);

pub struct NativeMenu {
    // NSMenuItem does not retain its target. Keep it alive until targets are detached.
    _target: Retained<DawFileMenuTarget>,
    items: Vec<(FileAction, Retained<NSMenuItem>)>,
    receiver: mpsc::Receiver<FileAction>,
}

impl NativeMenu {
    pub fn new(context: egui::Context) -> Self {
        let mtm = MainThreadMarker::new().expect("AppKit menus require the main thread");
        let (sender, receiver) = mpsc::channel();
        let allocated =
            DawFileMenuTarget::alloc(mtm).set_ivars(MenuTargetIvars { sender, context });
        // SAFETY: NSObject's init has this signature and initializes the allocated target.
        let target: Retained<DawFileMenuTarget> = unsafe { msg_send![super(allocated), init] };

        let app = NSApplication::sharedApplication(mtm);
        // Preserve winit's application menu, including Services and Hide commands.
        let menubar = app
            .mainMenu()
            .expect("winit must initialize the application menu");
        let file_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("File"));
        file_menu.setAutoenablesItems(false);
        let mut items = Vec::new();
        for (index, action) in FileAction::ALL.into_iter().enumerate() {
            if matches!(
                action,
                FileAction::Import
                    | FileAction::Save
                    | FileAction::Export
                    | FileAction::CloseProject
            ) {
                file_menu.addItem(&NSMenuItem::separatorItem(mtm));
            }
            let shortcut = action.shortcut();
            let key = shortcut.logical_key.name().to_lowercase();
            // SAFETY: The selector is defined above, with an NSMenuItem argument.
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(mtm),
                    &NSString::from_str(action.label()),
                    Some(sel!(performFileAction:)),
                    &NSString::from_str(&key),
                )
            };
            let mut modifiers = NSEventModifierFlags::Command;
            if shortcut.modifiers.shift {
                modifiers |= NSEventModifierFlags::Shift;
            }
            item.setKeyEquivalentModifierMask(modifiers);
            item.setTag(index as isize);
            // SAFETY: NativeMenu retains the target, and detaches it before dropping.
            unsafe { item.setTarget(Some(&target)) };
            file_menu.addItem(&item);
            items.push((action, item));
        }
        let file_item = NSMenuItem::new(mtm);
        file_item.setTitle(ns_string!("File"));
        file_item.setSubmenu(Some(&file_menu));
        menubar.addItem(&file_item);
        app.setMainMenu(Some(&menubar));

        Self {
            _target: target,
            items,
            receiver,
        }
    }

    pub fn dispatch(&self, ui: &mut DawUi) {
        for action in self.receiver.try_iter() {
            ui.perform_file_action(action);
        }
    }

    pub fn update_enabled(&self, ui: &DawUi) {
        for (action, item) in &self.items {
            item.setEnabled(ui.file_action_enabled(*action));
        }
    }
}

impl Drop for NativeMenu {
    fn drop(&mut self) {
        for (_, item) in &self.items {
            // SAFETY: Removing the target prevents callbacks after it is released.
            unsafe { item.setTarget(None) };
        }
    }
}
