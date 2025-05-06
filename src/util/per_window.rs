use std::error::Error;
use crate::util::windows_ffi::{WCAData, WCAttribute, WindowsFFI};
use ash::{khr, vk, Device, Entry, Instance};
use colored::Colorize;
use log::error;
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::platform::windows::HWND;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::{Window, WindowAttributes, WindowId};
use crate::{cleanup, ExtensionHolder, OSSurface};
use crate::util::logging::Logged;

pub struct PerWindow {
    pub window: Window,
    pub surface: vk::SurfaceKHR,
    pub swapchain: vk::SwapchainKHR,
    pub images: Vec<vk::Image>,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
}

pub struct WindowBuilder<'a> {
    entry: &'a Entry,
    instance: &'a Instance,
    ext: &'a ExtensionHolder,
    device: &'a Device,
    physical_device: &'a vk::PhysicalDevice,

    pub attributes: WindowAttributes,
}


impl<'a> WindowBuilder<'a> {
    pub fn new(entry: &'a Entry, instance: &'a Instance, ext: &'a ExtensionHolder, device: &'a Device, physical_device: &'a vk::PhysicalDevice) -> Self {
        WindowBuilder {
            entry,instance, ext, device, physical_device,
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

        //if we want to do anything fun we'll need a swapchain - and that's a per-surface thingy
        // todo! actually use all this information, and decide on proper swapchain settings based on them
        let capabilities = unsafe { self.ext.surface.get_physical_device_surface_capabilities(*self.physical_device, surface).unwrap() };
        let formats = unsafe { self.ext.surface.get_physical_device_surface_formats(*self.physical_device, surface).unwrap() };
        #[allow(unused)]
        let present_modes = unsafe { self.ext.surface.get_physical_device_surface_present_modes(*self.physical_device, surface).unwrap() };
        #[allow(unused)]
        let (formats,color_spaces) = formats.iter().map(|format|(format.format,format.color_space)).collect::<(Vec<vk::Format>,Vec<vk::ColorSpaceKHR>)>();

        //in case neither 32bit BGRA SRGB or 32bit RGBA SRGB are available, a default value.
        let mut format = *formats.first().unwrap();
        //ain't fuckin with any of the other color spaces, nor dealing with their availability for now. honestly go find a device other than a washing mashien or something that deosn't support SRGB color spaces
        let mut color_space = vk::ColorSpaceKHR::SRGB_NONLINEAR;
        //SRGB is common and good. B8G8R8 format is also shockingly common in displays?
        if formats.contains(&vk::Format::B8G8R8A8_SRGB) { format = vk::Format::B8G8R8A8_SRGB }
        else if formats.contains(&vk::Format::R8G8B8A8_SRGB) { format = vk::Format::R8G8B8A8_SRGB }

        let extent = {
            let PhysicalSize { width, height } = window.inner_size();
            vk::Extent2D::default().width(width).height(height) };

        let swapchain_create_info = vk::SwapchainCreateInfoKHR {
            flags: vk::SwapchainCreateFlagsKHR::default(),
            surface,
            min_image_count: capabilities.min_image_count,
            image_format: format,
            image_color_space: color_space,
            image_extent: extent,
            image_array_layers: 1,
            //for now, we'll only use the swapchain as a framebuffer color attachment
            image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            //exclusive sharing between queue families has the best performance, but forces you to deal with ownership in between families if you use multiple ones. we don't. we use one family without checking for presentation support. yay
            image_sharing_mode: vk::SharingMode::EXCLUSIVE,
            //          queue family infos are only needed if we're using CONCURRENT image sharing.
            //queue_family_index_count: ,
            //p_queue_family_indices: ,
            pre_transform: capabilities.current_transform,
            composite_alpha: vk::CompositeAlphaFlagsKHR::INHERIT,
            present_mode: vk::PresentModeKHR::IMMEDIATE,
            //we don't care about obscured pixels (for now)
            clipped: vk::TRUE,
            //really quite pleasant that the ash bindings implement Default for pretty much all those structs
            ..Default::default()};

        //error handling? who's that? whaddya the code started off with "good" error handling and went downhill?
        let swapchain = unsafe { self.ext.swapchain.create_swapchain(&swapchain_create_info, None).unwrap() };
        //oh and thanks to the ash devs, i don't need to query the amount of swapchain images to allocate space for before getting to fetch them.
        //LOOK AT ash::prelude::read_into_uninitialized_vector() - genuinely a pleasant solution to all this
        /// just click then press F4:
        /// [ash::prelude::read_into_uninitialized_vector]
        let images = unsafe { self.ext.swapchain.get_swapchain_images(swapchain).unwrap() };

        (window.id(), PerWindow { window, surface, swapchain, images, format, extent })
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

