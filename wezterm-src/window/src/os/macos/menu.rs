//! macOS NSMenu / NSMenuItem 封装.
//!
//! 历史: 早期实现用 cocoa 0.x crate + objc 0.2 的手工 retain/release
//! (StrongPtr) + ClassDecl 注册自定义 wrapper class, 在 macOS 26.4.1 (Tahoe)
//! 上跟 NSWindowsMenu 自动管理产生 race: AppKit 的 `_findMenuItemsForWindow`
//! 在窗口 becomeKeyWindow 时遍历 windows menu, 访问已被 cocoa 0.x StrongPtr
//! 释放的 NSMenuItem, 触发 PAC failure → SIGSEGV.
//!
//! 当前实现: NSMenu / NSMenuItem 仍走 objc 0.2 的 msg_send 调用 (API 完全不
//! 变, commands.rs 等调用方零修改), 但 retain/release 改用 objc2::rc::Retained
//! 自动管理 (drop 时按 ARC 规则 release). 自定义 wrapper class 的 dealloc /
//! isEqual 两个 extern "C" 函数包了 catch_unwind, 防止 panic 跨 FFI 触发
//! panic_cannot_unwind 杀进程 (跟 spawn.rs::trigger 同源问题).

use crate::macos::{nsstring, nsstring_to_str};
use crate::superclass;
pub use cocoa::appkit::NSEventModifierFlags;
use cocoa::appkit::{NSApp, NSApplication};
pub use cocoa::base::SEL;
use cocoa::base::{id, nil};
use cocoa::foundation::NSInteger;
use config::keyassignment::KeyAssignment;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, BOOL, NO, YES};
pub use objc::*;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use std::ffi::c_void;

/// 把 cocoa raw id 包成 Retained<AnyObject>:
/// - `consume_owned`: id 已经持有 +1 retain (alloc/init / copy / new pattern),
///   直接转交所有权给 Retained, drop 时 release.
/// - `retain_borrowed`: id 是 +0 (autoreleased), 需要 +1 防止 pool 回收.
unsafe fn consume_owned(ptr: id) -> Option<Retained<AnyObject>> {
    if ptr.is_null() {
        None
    } else {
        Retained::from_raw(ptr as *mut AnyObject)
    }
}

unsafe fn retain_borrowed(ptr: id) -> Option<Retained<AnyObject>> {
    if ptr.is_null() {
        None
    } else {
        Retained::retain(ptr as *mut AnyObject)
    }
}

pub struct Menu {
    menu: Retained<AnyObject>,
}

impl Menu {
    fn raw(&self) -> id {
        Retained::as_ptr(&self.menu) as id
    }

    pub fn new_with_title(title: &str) -> Self {
        unsafe {
            let alloc: id = msg_send![class!(NSMenu), alloc];
            let init: id = msg_send![alloc, initWithTitle:*nsstring(title)];
            let menu = consume_owned(init).expect("NSMenu init returned nil");
            Self { menu }
        }
    }

    /// 历史 API: 旧版返回 `*mut Object`, 这里维持 cocoa::base::id 类型;
    /// Retained 把所有权转出 (+1, 调用方负责释放).
    pub fn autorelease(self) -> *mut Object {
        let raw = Retained::into_raw(self.menu) as *mut Object;
        unsafe {
            let _: () = msg_send![raw, autorelease];
        }
        raw
    }

    pub fn item_at_index(&self, index: usize) -> Option<MenuItem> {
        unsafe {
            let item: id = msg_send![self.raw(), itemAtIndex: index as NSInteger];
            retain_borrowed(item).map(|item| MenuItem { item })
        }
    }

    pub fn assign_as_main_menu(&self) {
        unsafe {
            let ns_app = NSApp();
            let _: () = msg_send![ns_app, setMainMenu: self.raw()];
        }
    }

    pub fn get_main_menu() -> Option<Self> {
        unsafe {
            let ns_app = NSApp();
            let existing: id = msg_send![ns_app, mainMenu];
            retain_borrowed(existing).map(|menu| Self { menu })
        }
    }

    pub fn assign_as_help_menu(&self) {
        unsafe {
            let ns_app = NSApp();
            let _: () = msg_send![ns_app, setHelpMenu: self.raw()];
        }
    }

    pub fn assign_as_windows_menu(&self) {
        unsafe {
            let ns_app = NSApp();
            let _: () = msg_send![ns_app, setWindowsMenu: self.raw()];
        }
    }

    pub fn assign_as_services_menu(&self) {
        unsafe {
            let ns_app = NSApp();
            let _: () = msg_send![ns_app, setServicesMenu: self.raw()];
        }
    }

    pub fn assign_as_app_menu(&self) {
        unsafe {
            let ns_app = NSApp();
            let _: () = msg_send![ns_app, performSelector:sel!(setAppleMenu:) withObject:self.raw()];
        }
    }

    pub fn add_item(&self, item: &MenuItem) {
        unsafe {
            let _: () = msg_send![self.raw(), addItem: item.raw()];
        }
    }

    pub fn item_with_title(&self, title: &str) -> Option<MenuItem> {
        unsafe {
            let item: id = msg_send![self.raw(), itemWithTitle:*nsstring(title)];
            retain_borrowed(item).map(|item| MenuItem { item })
        }
    }

    pub fn get_or_create_sub_menu<F: FnOnce(&Menu)>(&self, title: &str, on_create: F) -> Menu {
        match self.item_with_title(title) {
            Some(m) => m.get_sub_menu().unwrap(),
            None => {
                let item = MenuItem::new_with(title, None, "");
                let menu = Menu::new_with_title(title);
                item.set_sub_menu(&menu);
                self.add_item(&item);
                on_create(&menu);
                menu
            }
        }
    }

    pub fn get_sub_menu(&self, title: &str) -> Menu {
        self.item_with_title(title).unwrap().get_sub_menu().unwrap()
    }

    pub fn remove_all_items(&self) {
        unsafe {
            let _: () = msg_send![self.raw(), removeAllItems];
        }
    }

    pub fn remove_item(&self, item: &MenuItem) {
        unsafe {
            let _: () = msg_send![self.raw(), removeItem: item.raw()];
        }
    }

    pub fn items(&self) -> Vec<MenuItem> {
        unsafe {
            let n: NSInteger = msg_send![self.raw(), numberOfItems];
            let mut items = Vec::with_capacity(n as usize);
            for i in 0..n {
                items.push(self.item_at_index(i as usize).expect("index to be valid"));
            }
            items
        }
    }

    pub fn index_of_item_with_represented_object(&self, object: id) -> Option<usize> {
        unsafe {
            let n: NSInteger =
                msg_send![self.raw(), indexOfItemWithRepresentedObject: object];
            if n == -1 {
                None
            } else {
                Some(n as usize)
            }
        }
    }

    pub fn index_of_item_with_represented_item(&self, item: &RepresentedItem) -> Option<usize> {
        let wrapped = item.clone().wrap();
        let raw = unsafe { Retained::as_ptr(&wrapped) as id };
        // wrapped 是临时查找 key, 函数 return 时自动 drop → release.
        self.index_of_item_with_represented_object(raw)
    }

    pub fn get_item_with_represented_item(&self, item: &RepresentedItem) -> Option<MenuItem> {
        let idx = self.index_of_item_with_represented_item(item)?;
        self.item_at_index(idx)
    }
}

pub struct MenuItem {
    item: Retained<AnyObject>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RepresentedItem {
    KeyAssignment(KeyAssignment),
}

impl RepresentedItem {
    /// 用自定义 NSObject wrapper 包一层, 返回 +1 retain 的 Retained;
    /// drop 时 wrapper 的 dealloc 会释放内部 Box<RepresentedItem>.
    fn wrap(self) -> Retained<AnyObject> {
        unsafe {
            let alloc: id = msg_send![get_wrapper_class(), alloc];
            let init: id = msg_send![alloc, init];
            let item = Box::new(self);
            let item_ptr = Box::into_raw(item) as *const c_void;
            (*init).set_ivar(WRAPPER_FIELD_NAME, item_ptr);
            consume_owned(init).expect("wrapper alloc/init returned nil")
        }
    }

    unsafe fn ref_item(wrapper: id) -> Option<RepresentedItem> {
        let item = (*wrapper).get_ivar::<*const c_void>(WRAPPER_FIELD_NAME);
        let item = (*item) as *const RepresentedItem;
        if item.is_null() {
            None
        } else {
            Some((*item).clone())
        }
    }
}

impl MenuItem {
    fn raw(&self) -> id {
        Retained::as_ptr(&self.item) as id
    }

    /// 接收外部 cocoa raw id (+0 autoreleased), retain 一次让 MenuItem 持有.
    pub fn with_menu_item(item: id) -> Self {
        let item = unsafe { retain_borrowed(item) }.expect("menu item is nil");
        Self { item }
    }

    pub fn new_separator() -> Self {
        unsafe {
            // separatorItem 返回 +0 (autoreleased), 需要 retain.
            let item: id = msg_send![class!(NSMenuItem), separatorItem];
            let item = retain_borrowed(item).expect("separatorItem returned nil");
            Self { item }
        }
    }

    pub fn new_with(title: &str, action: Option<SEL>, key: &str) -> Self {
        unsafe {
            let alloc: id = msg_send![class!(NSMenuItem), alloc];
            let action_sel: SEL = action.unwrap_or_else(|| SEL::from_ptr(std::ptr::null()));
            let init: id = msg_send![
                alloc,
                initWithTitle: *nsstring(title)
                action: action_sel
                keyEquivalent: *nsstring(key)
            ];
            let item = consume_owned(init).expect("NSMenuItem init returned nil");
            Self { item }
        }
    }

    pub fn get_action(&self) -> Option<SEL> {
        unsafe {
            let s: SEL = msg_send![self.raw(), action];
            if s.as_ptr().is_null() {
                None
            } else {
                Some(s)
            }
        }
    }

    pub fn set_tool_tip(&self, tip: &str) {
        unsafe {
            let _: () = msg_send![self.raw(), setToolTip:*nsstring(tip)];
        }
    }

    pub fn set_target(&self, target: id) {
        unsafe {
            let _: () = msg_send![self.raw(), setTarget: target];
        }
    }

    pub fn set_sub_menu(&self, menu: &Menu) {
        unsafe {
            let _: () = msg_send![self.raw(), setSubmenu: menu.raw()];
        }
    }

    pub fn get_sub_menu(&self) -> Option<Menu> {
        unsafe {
            let menu: id = msg_send![self.raw(), submenu];
            retain_borrowed(menu).map(|menu| Menu { menu })
        }
    }

    pub fn get_parent_item(&self) -> Option<Self> {
        unsafe {
            let item: id = msg_send![self.raw(), parentItem];
            retain_borrowed(item).map(|item| Self { item })
        }
    }

    pub fn get_menu(&self) -> Option<Menu> {
        unsafe {
            let menu: id = msg_send![self.raw(), menu];
            retain_borrowed(menu).map(|menu| Menu { menu })
        }
    }

    /// Set an integer tag to identify this item
    pub fn set_tag(&self, tag: NSInteger) {
        unsafe {
            let _: () = msg_send![self.raw(), setTag: tag];
        }
    }

    pub fn get_title(&self) -> String {
        unsafe {
            let title: id = msg_send![self.raw(), title];
            nsstring_to_str(title).to_string()
        }
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            let _: () = msg_send![self.raw(), setTitle:*nsstring(title)];
        }
    }

    pub fn set_key_equivalent(&self, equiv: &str) {
        unsafe {
            let _: () = msg_send![self.raw(), setKeyEquivalent:*nsstring(equiv)];
        }
    }

    pub fn get_tag(&self) -> NSInteger {
        unsafe { msg_send![self.raw(), tag] }
    }

    /// Associate the item to an object
    fn set_represented_object(&self, object: id) {
        unsafe {
            let _: () = msg_send![self.raw(), setRepresentedObject: object];
        }
    }

    fn get_represented_object(&self) -> Option<Retained<AnyObject>> {
        unsafe {
            let object: id = msg_send![self.raw(), representedObject];
            retain_borrowed(object)
        }
    }

    pub fn set_represented_item(&self, item: RepresentedItem) {
        let wrapper = item.wrap();
        let raw = Retained::as_ptr(&wrapper) as id;
        // setRepresentedObject 内部 retain 一次 (objc 标准); 我们的 wrapper +1
        // 在函数 return 时 drop release, NSMenuItem 还持有它.
        self.set_represented_object(raw);
    }

    pub fn get_represented_item(&self) -> Option<RepresentedItem> {
        let wrapper = self.get_represented_object()?;
        unsafe { RepresentedItem::ref_item(Retained::as_ptr(&wrapper) as id) }
    }

    pub fn set_key_equiv_modifier_mask(&self, mods: NSEventModifierFlags) {
        unsafe {
            let _: () = msg_send![self.raw(), setKeyEquivalentModifierMask: mods];
        }
    }
}

const WRAPPER_CLS_NAME: &str = "WezTermNSMenuRepresentedItem";
const WRAPPER_FIELD_NAME: &str = "item";

/// 自定义 NSObject 子类: 在 ivar 里存一个 `Box<RepresentedItem>` 的 raw 指针,
/// 用作 NSMenuItem.representedObject 的 wrapper.
///
/// dealloc / is_equal 都包了 catch_unwind, 防止 panic 跨 FFI 边界触发
/// panic_cannot_unwind 杀进程 (跟 spawn.rs::trigger 同源问题).
fn get_wrapper_class() -> &'static Class {
    Class::get(WRAPPER_CLS_NAME).unwrap_or_else(|| {
        let mut cls =
            ClassDecl::new(WRAPPER_CLS_NAME, class!(NSObject)).expect("Unable to register class");

        extern "C" fn dealloc(this: &mut Object, _sel: Sel) {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                let item = this.get_ivar::<*mut c_void>(WRAPPER_FIELD_NAME);
                let item = (*item) as *mut RepresentedItem;
                if !item.is_null() {
                    let item = Box::from_raw(item);
                    drop(item);
                }
                let superclass = superclass(this);
                let _: () = msg_send![super(this, superclass), dealloc];
            }));
        }

        extern "C" fn is_equal(this: &mut Object, _sel: Sel, that: *mut Object) -> BOOL {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                let this_item = RepresentedItem::ref_item(this);
                let that_item = RepresentedItem::ref_item(that);
                this_item == that_item
            }));
            match result {
                Ok(true) => YES,
                Ok(false) | Err(_) => NO,
            }
        }

        cls.add_ivar::<*mut c_void>(WRAPPER_FIELD_NAME);
        unsafe {
            cls.add_method(sel!(dealloc), dealloc as extern "C" fn(&mut Object, Sel));
            cls.add_method(
                sel!(isEqual:),
                is_equal as extern "C" fn(&mut Object, Sel, *mut Object) -> BOOL,
            );
        }
        cls.register()
    })
}

// 向后兼容: 保留 `nil` 在 module scope (历史 cocoa::base::nil 在原版被
// `pub use objc::*` 间接暴露给 commands.rs?  实际未在 commands.rs 出现,
// 但保险起见保留 import 路径).
#[allow(dead_code)]
const _NIL_KEEP_ALIVE: id = nil;
