use std::error::Error;
use crate::util::windows_ffi::{WCAData, WCAttribute, WindowsFFI};
use ash::{khr, vk, Entry, Instance};
use colored::Colorize;
use log::error;
use winit::event_loop::ActiveEventLoop;
use winit::platform::windows::HWND;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::{Window, WindowAttributes, WindowId};
use crate::{cleanup, ExtensionHolder, OSSurface};
use crate::util::logging::Logged;

pub struct PerWindow {
    pub window: Window,
    pub surface: vk::SurfaceKHR,
}

pub struct WindowBuilder<'a> {
    entry: &'a Entry,
    instance: &'a Instance,
    ext: &'a ExtensionHolder,

    pub attributes: WindowAttributes,
}


impl<'a> WindowBuilder<'a> {
    pub fn new(entry: &'a Entry, instance: &'a Instance, ext: &'a ExtensionHolder) -> Self {
        WindowBuilder {
            entry,instance, ext,
            attributes: WindowAttributes::default()}
    }
    pub fn build(&self, event_loop: &'a ActiveEventLoop) -> (WindowId, PerWindow) {
        let window = event_loop.create_window(self.attributes.clone()).unwrap();

        let surface = unsafe {
            let window_handle = window.window_handle().unwrap().as_raw();
            let display_handle = event_loop.display_handle().unwrap().as_raw();

            match &self.ext.os_surface {
                OSSurface::WINDOWS(instance) => {
                    let RawWindowHandle::Win32(hwnd) = window_handle else { error!("Window/Display Handles don't match. Just no."); panic!() };
                    let create_info = vk::Win32SurfaceCreateInfoKHR {
                        hwnd: hwnd.hwnd.into(),
                        hinstance: hwnd.hinstance.unwrap().into(),
                        ..Default::default()};
                    instance.create_win32_surface(&create_info, None).logged("Surface creation failure")
                }
                OSSurface::WAYLAND(instance) => {
                    let RawWindowHandle::Wayland(mut hwnd) = window_handle else { error!("Window/Display Handles don't match. Just no."); panic!() };
                    let RawDisplayHandle::Wayland(mut hdsp) = display_handle else { unreachable!() };
                    let create_info = vk::WaylandSurfaceCreateInfoKHR {
                        display: hdsp.display.as_mut(),
                        surface: hwnd.surface.as_mut(),
                        ..Default::default()};
                    instance.create_wayland_surface(&create_info,None).logged("Surface creation failure")
                }
                OSSurface::XCB(instance) => {
                    let RawWindowHandle::Xcb(hwnd) = window_handle else { error!("Window/Display Handles don't match. Just no."); panic!() };
                    let RawDisplayHandle::Xcb(hdsp) = display_handle else { unreachable!() };
                    let create_info = vk::XcbSurfaceCreateInfoKHR {
                        connection: hdsp.connection.unwrap().as_mut(),
                        window: hwnd.window.into(),
                        ..Default::default()};
                    instance.create_xcb_surface(&create_info,None).logged("Surface creation failure")
                }
                OSSurface::XLIB(instance) => {
                    let RawWindowHandle::Xlib(hwnd) = window_handle else { error!("Window/Display Handles don't match. Just no."); panic!() };
                    let RawDisplayHandle::Xlib(hdsp) = display_handle else { unreachable!() };
                    let create_info = vk::XlibSurfaceCreateInfoKHR {
                        dpy: hdsp.display.unwrap().as_mut(),
                        window: hwnd.window,
                        ..Default::default()};
                    instance.create_xlib_surface(&create_info,None).logged("Surface creation failure")
                }
                //you'd have to actively try to get this error and even then, I doubt it's possible without manually fiddling with the applications' memory.
                _ => { error!("Window Handle doesn't match display handles tolerated by this application");
                    //unsafe { cleanup() };  todo!
                    panic!() }
            }
        };

        (window.id(), PerWindow { window, surface })
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

