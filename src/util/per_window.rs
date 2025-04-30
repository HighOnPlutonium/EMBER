use crate::util::windows_ffi::{WCAData, WCAttribute, WindowsFFI};
use ash::{vk, Entry, Instance};
use winit::event_loop::ActiveEventLoop;
use winit::platform::windows::HWND;
use winit::raw_window_handle::{HasDisplayHandle, HasRawDisplayHandle, HasRawWindowHandle, HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowAttributes, WindowId};

pub struct PerWindow {
    pub window: Window,
    pub surface: vk::SurfaceKHR,
}

pub struct WindowBuilder<'a> {
    event_loop: &'a ActiveEventLoop,
    entry: &'a Entry,
    instance: &'a Instance,
    pub attributes: WindowAttributes,
}

impl<'a> WindowBuilder<'a> {
    pub fn new(event_loop: &'a ActiveEventLoop, entry: &'a Entry, instance: &'a Instance) -> Self {
        WindowBuilder {
            event_loop,
            entry,
            instance,
            attributes: WindowAttributes::default()}
    }
    pub fn build(&self) -> (WindowId, PerWindow) {

        let window = self.event_loop.create_window(
            self.attributes.clone()
        ).unwrap();

        let surface = unsafe {
            ash_window::create_surface(self.entry,self.instance,self.event_loop.raw_display_handle().unwrap(),window.raw_window_handle().unwrap(), None).unwrap()
        };

        ( window.id(), PerWindow { window, surface } )
    }
}

impl PerWindow {

    pub fn toggle_blur(&self, function_pointers: &WindowsFFI) {
        if let RawWindowHandle::Win32(handle) = self.window.window_handle().unwrap().as_raw() {
            let mut attribute = WCAttribute::WCA_ACCENT_POLICY { state: 3, flags: 480, gradient: 0, animation: 0 };
            unsafe {
                (function_pointers.set_window_composition_attribute)(
                    HWND::from(handle.hwnd),
                    std::ptr::from_mut(&mut WCAData::new(&mut attribute))
                ); //.then_some(()).expect("failure from function pointer call")
            }
            return

        };
        panic!("Severe lack of Win32 window handles");
    }

}

