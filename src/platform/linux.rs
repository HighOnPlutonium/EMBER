#[cfg(target_os = "linux")]
pub(crate) mod kwin;
#[cfg(target_os = "linux")]
pub(crate) mod vmem;
#[cfg(target_os = "linux")]
pub(crate) mod util;


#[cfg(not(target_os = "linux"))]
pub(crate) mod util {
    use crate::platform::{platform_mismatch, Platform};
    
    use std::error::Error;
    
    pub fn get_pid(_name: &str, _all_users: bool) -> Result<usize, Box<dyn Error>> {
        platform_mismatch(Platform::LINUX)
    }

    pub struct Root(i32);
    impl Root {
        pub fn new() -> Self {
            platform_mismatch(Platform::LINUX)
        }

        pub fn claim(&mut self) {
            platform_mismatch(Platform::LINUX)
        }

        pub fn release(&mut self) {
            platform_mismatch(Platform::LINUX)
        }
    }

    pub fn elf_offset(_path: &str, _symbol: &str) -> usize {
        platform_mismatch(Platform::LINUX)
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) mod kwin {
    #![warn(unsafe_op_in_unsafe_fn)]

    use std::ffi::c_void;
    use crate::platform::{platform_mismatch, Platform};

    pub unsafe fn libkwin_base_address(_pid: usize) -> *mut c_void {
        platform_mismatch(Platform::LINUX)
    }

    pub fn get_mouse_pos(_pid: usize, _base: *mut c_void, _offset: usize) -> [u8; 16] {
        platform_mismatch(Platform::LINUX)
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) mod vmem {
    
}