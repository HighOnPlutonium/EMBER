#[cfg(windows)]
pub(crate) mod ffi;


#[cfg(not(windows))]
pub(crate) mod ffi {
    use crate::platform::{platform_mismatch, Platform};

    use libloading::Library;
    use std::os::raw::c_void;

    type HWND = isize;
    #[allow(unused)]
    pub struct WindowsFFI {
        pub set_window_composition_attribute: fn(HWND, *mut WCAData) -> bool,
        pub get_window_composition_attribute: fn(HWND, *mut WCAData) -> bool,
    }
    impl WindowsFFI {
        pub unsafe fn load_function_pointers() -> Self {
            crate::platform::platform_mismatch(crate::platform::Platform::WINDOWS)
        }
    }

    #[allow(unused)]
    #[allow(non_camel_case_types)]
    #[repr(u32)]
    pub enum WCAttribute {
        WCA_ACCENT_POLICY { state: u32, flags: u32, gradient: u32, animation: u32 } = 19
    }
    #[repr(C)]
    pub struct WCAData {
        attrib: u32,
        pv_data: *mut core::ffi::c_void,
        cb_data: usize
    }
    impl WCAData {
        pub unsafe fn new(_attribute: &mut WCAttribute) -> Self {
            crate::platform::platform_mismatch(crate::platform::Platform::WINDOWS)
        }
    }
}