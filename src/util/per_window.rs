use std::error::Error;
use crate::util::windows_ffi::{WCAData, WCAttribute, WindowsFFI};
use ash::{vk, Device};
use log::error;
use winit::event_loop::ActiveEventLoop;
use winit::platform::windows::HWND;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::{Window, WindowAttributes, WindowId};
use crate::{ExtensionHolder, OSSurface, MAX_FRAMES_IN_FLIGHT};
use crate::util::helpers::{create_framebuffers, create_graphics_pipeline, create_render_pass, create_swapchain, create_views};
use crate::util::logging::Logged;

pub struct PerWindow {
    pub window: Window,
    pub surface: vk::SurfaceKHR,
    pub swapchain: vk::SwapchainKHR,
    pub images: Vec<vk::Image>,
    pub views: Vec<vk::ImageView>,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub render_pass: vk::RenderPass,
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub command_buffers: Vec<vk::CommandBuffer>,
    pub synchronization: Vec<SYN>,
}

impl PerWindow {
    pub(crate) fn recreate(
        &mut self,
        swapchain: vk::SwapchainKHR,
        extent: vk::Extent2D,
        images: Vec<vk::Image>,
        views: Vec<vk::ImageView>,
        framebuffers: Vec<vk::Framebuffer>,
        syn: Vec<SYN>
    ) {
        self.swapchain = swapchain;
        self.extent = extent;
        self.images = images;
        self.views = views;
        self.framebuffers = framebuffers;
        self.synchronization = syn;
    }
}

#[derive(Copy,Clone)]
pub(crate) struct SYN {
    pub(crate) swapchain: vk::Semaphore,
    pub(crate) presentation: vk::Semaphore,
    pub(crate) in_flight: vk::Fence,
}
impl SYN {
    pub(crate) unsafe fn new(device: &Device) -> Result<SYN, Box<dyn Error>> {
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo {
            flags: vk::FenceCreateFlags::SIGNALED,
            ..Default::default()};
        let swapchain    = device.create_semaphore(&semaphore_info,None)?;
        let presentation = device.create_semaphore(&semaphore_info,None)?;
        let in_flight    = device.create_fence(&fence_info,None)?;
        Ok(Self {swapchain,presentation,in_flight})
    }
    pub(crate) unsafe fn destroy(self, device: &Device) {
        device.destroy_semaphore(self.swapchain,None);
        device.destroy_semaphore(self.presentation,None);
        device.destroy_fence(self.in_flight,None);
    }
}

pub struct WindowBuilder<'a> {
    ext: &'a ExtensionHolder,
    device: &'a Device,
    physical_device: vk::PhysicalDevice,
    command_pool: vk::CommandPool,

    pub attributes: WindowAttributes,
}


impl<'a> WindowBuilder<'a> {
    pub fn new(
        ext: &'a ExtensionHolder,
        device: &'a Device,
        physical_device: vk::PhysicalDevice,
        command_pool: vk::CommandPool
    ) -> Self {
        WindowBuilder {
            ext, device, physical_device, command_pool,
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
                        hwnd: hwnd.hwnd.get(),
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
            }
        };


        let (swapchain,format,extent,syn) = unsafe {
            create_swapchain(
                &window,
                surface,
                self.device,
                self.physical_device,
                &self.ext.surface,
                &self.ext.swapchain
            ).unwrap() };    // todo!    ERROR HANDLING
        let images = unsafe { self.ext.swapchain.get_swapchain_images(swapchain).unwrap() };
        let views = unsafe { create_views(self.device,&images,format) };

        let render_pass = unsafe { create_render_pass(self.device,format) };
        let (pipeline,layout) = unsafe { create_graphics_pipeline(self.device,extent,render_pass) };
        let framebuffers: Vec<vk::Framebuffer> = unsafe { create_framebuffers(self.device,&window,&views,render_pass) };

        let cmd_alloc_info = vk::CommandBufferAllocateInfo {
            command_pool: self.command_pool,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: MAX_FRAMES_IN_FLIGHT,
            ..Default::default()};
        let command_buffers = unsafe { self.device.allocate_command_buffers(&cmd_alloc_info).unwrap() };


        (window.id(), PerWindow { window, surface,
            swapchain, images, views,
            framebuffers,
            format, extent,
            render_pass,
            pipeline, layout,
            command_buffers,
            synchronization: syn,
        })
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

