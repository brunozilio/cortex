//! AppKit: a "Settings…" item in the app menu that winit builds, placed
//! after "About" as macOS apps do, with a target object that calls back.

#![allow(unsafe_code)]

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSMenuItem};
use objc2_foundation::NSString;

use crate::AppMenu;

const SETTINGS_INDEX: isize = 2;

pub struct TargetIvars {
    on_settings: Box<dyn Fn()>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the target
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SuperfikMenuTarget"]
    #[ivars = TargetIvars]
    struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            (self.ivars().on_settings)();
        }
    }
);

impl MenuTarget {
    fn new(mtm: MainThreadMarker, on_settings: Box<dyn Fn()>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TargetIvars { on_settings });
        // SAFETY: NSObject's designated initialiser.
        unsafe { msg_send![super(this), init] }
    }
}

pub struct MacMenu {
    item: Retained<NSMenuItem>,
    _target: Retained<MenuTarget>,
}

impl std::fmt::Debug for MacMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MacMenu").finish_non_exhaustive()
    }
}

impl MacMenu {
    pub fn install(title: &str, on_settings: Box<dyn Fn()>) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let app = NSApplication::sharedApplication(mtm);
        let app_menu = app.mainMenu()?.itemAtIndex(0)?.submenu()?;
        let target = MenuTarget::new(mtm, on_settings);
        // SAFETY: the selector is implemented by `MenuTarget`, which the
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                Some(sel!(openSettings:)),
                &NSString::from_str(","),
            )
        };
        // SAFETY: `target` responds to the item's action.
        unsafe { item.setTarget(Some(&target)) };
        let index = SETTINGS_INDEX.min(app_menu.numberOfItems());
        app_menu.insertItem_atIndex(&item, index);
        app_menu.insertItem_atIndex(&NSMenuItem::separatorItem(mtm), index + 1);
        tracing::debug!("settings item added to the app menu");
        Some(Self { item, _target: target })
    }
}

impl AppMenu for MacMenu {
    fn set_settings_title(&mut self, title: &str) {
        self.item.setTitle(&NSString::from_str(title));
    }
}
