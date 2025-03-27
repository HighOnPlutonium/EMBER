//! **WCA** - **W**indow**C**omposition**A**ttribute
//! # Windows API doc
//! [SetWindowCompositionAttribute function](https://learn.microsoft.com/en-us/windows/win32/dwm/setwindowcompositionattribute)
//!
//! [GetWindowCompositionAttribute function](https://learn.microsoft.com/en-us/windows/win32/dwm/getwindowcompositionattribute)
//!
//! [WINDOWCOMPOSITIONATTRIBDATA structure](https://learn.microsoft.com/en-us/windows/win32/dwm/windowcompositionattribdata)
//!
//! [WINDOWCOMPOSITIONATTRIB enumeration](https://learn.microsoft.com/en-us/windows/win32/dwm/windowcompositionattrib)

use std::os::raw::c_void;
use libloading::Library;
use winit::platform::windows::HWND;


pub struct WindowsFFI {
    pub set_window_composition_attribute: fn(HWND, *mut WCAData) -> bool,
    pub get_window_composition_attribute: fn(HWND, *mut WCAData) -> bool,
}
impl WindowsFFI {
    pub unsafe fn load_function_pointers() -> Self {
        let lib = Library::new("C:/Windows/System32/user32.dll").unwrap();
        Self {
            set_window_composition_attribute: *lib.get::<_>(b"SetWindowCompositionAttribute").unwrap(),
            get_window_composition_attribute: *lib.get::<_>(b"GetWindowCompositionAttribute").unwrap(),
        }}
}
#[repr(C)]
pub struct WCAData {
    attrib: u32,
    pv_data: *mut core::ffi::c_void,
    cb_data: usize
}
impl WCAData {
    pub unsafe fn new(attribute: &mut WCAttribute) -> Self {
        //the first field of WCAttribute enum variants is the discriminant, which is an u32 due to #[repr(u32)]
        //as such we can retrieve the discriminant by casting a *const u32 from the base of the enum variants raw pointer
        //and offsetting by one u32 lets us "safely" retrieve a *mut c_void to the WCAttributes value
        let discriminant = <*const _>::from(attribute).cast::<u32>();
        let data = discriminant.offset(1) as *mut c_void;
        Self {
            attrib: *discriminant,
            //*mut c_void pointing at a value. type is dependent on the chosen attribute
            pv_data: data,
            //the size of the value pointed at by pv_data - effectively the size of the WCAttribute enum variant minus the discriminants size
            cb_data: size_of_val(attribute) - 4
        }
    }
}
#[allow(non_camel_case_types)]
#[repr(u32)]
pub enum WCAttribute {
    WCA_ACCENT_POLICY { state: u32, flags: u32, gradient: u32, animation: u32 } = 19
}




