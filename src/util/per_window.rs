use core::ffi;
use std::error::Error;
use std::ptr;
use crate::util::windows_ffi::{WCAData, WCAttribute, WindowsFFI};
use ash::{vk, Device};
use ash::util::Align;
use log::{debug, error};
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::{Window, WindowAttributes, WindowId};
use crate::{ExtensionHolder, OSSurface, SCHolder, UniformBufferObject, INSTANCE, MAX_FRAMES_IN_FLIGHT};
use crate::util::helpers::{create_framebuffers, create_graphics_pipeline, create_render_pass, create_views, Vertex, VERTICES};
use crate::util::logging::Logged;
use crate::util::swapchain::PerSwapchain;


type HWND = isize;
pub struct PerWindow {
    pub window: Window,
    pub surface: vk::SurfaceKHR,
    pub swapchain: PerSwapchain,

    pub render_pass: vk::RenderPass,
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub command_buffers: Vec<vk::CommandBuffer>,
    pub vertex_buffer: vk::Buffer,
    pub vertex_buffer_mem: vk::DeviceMemory,
    pub push_constant_range: vk::PushConstantRange,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub ubufs: Vec<vk::Buffer>,
    pub ubufs_mem: Vec<vk::DeviceMemory>,
    pub ubufs_map: Vec<*mut ffi::c_void>,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptor_sets: Vec<vk::DescriptorSet>,

    pub id: i32,
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
    pub(crate) unsafe fn destroy(&self, device: &Device) {
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
    pub fn build(&self, event_loop: &'a ActiveEventLoop, screencast: Option<&SCHolder>) -> (WindowId, PerWindow) {
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


        let (swapchain,format,extent,sync) = unsafe {
            PerSwapchain::create_swapchain(
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
        let (pipeline,layout,push_constant_range,descriptor_set_layout,descriptor_pool) = unsafe { create_graphics_pipeline(self.device,extent,render_pass) };
        let framebuffers: Vec<vk::Framebuffer> = unsafe { create_framebuffers(self.device,&window,&views,render_pass) };



        let vertex_buffer_info = vk::BufferCreateInfo {
            size: (VERTICES.len() * size_of::<Vertex>()) as _,
            usage: { type Flags = vk::BufferUsageFlags;
                Flags::VERTEX_BUFFER },
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()};

        let vertex_buffer = unsafe { self.device.create_buffer(&vertex_buffer_info, None).unwrap() };

        let vb_mem_req = unsafe { self.device.get_buffer_memory_requirements(vertex_buffer) };

        let mem_properties = unsafe { INSTANCE.get_physical_device_memory_properties(self.physical_device) };

        let req_flags = { type Flags = vk::MemoryPropertyFlags;
            Flags::HOST_VISIBLE | Flags::HOST_COHERENT };
        let mem_idx = mem_properties.memory_types[..mem_properties.memory_type_count as _]
            .iter().enumerate().find(|(idx,mem_type)|{
            (1u32 << idx) & vb_mem_req.memory_type_bits != 0
            && mem_type.property_flags &  req_flags == req_flags
        }).map(|(idx,mem_type)| idx as _ ).expect("no matching mem type found");

        let vb_allocate_info = vk::MemoryAllocateInfo {
            allocation_size: vb_mem_req.size,
            memory_type_index: mem_idx,
            ..Default::default()};

        let vertex_buffer_mem = unsafe { self.device.allocate_memory( &vb_allocate_info, None).unwrap() };

        let vert_ptr = unsafe { self.device.map_memory(vertex_buffer_mem, 0, vb_mem_req.size, vk::MemoryMapFlags::empty()).unwrap() };
        let mut vert_align = unsafe { Align::new(
            vert_ptr,
            align_of::<Vertex>() as u64,
            vb_mem_req.size,
        ) };
        vert_align.copy_from_slice(&VERTICES);
        unsafe { self.device.unmap_memory(vertex_buffer_mem) };

        unsafe { self.device.bind_buffer_memory(vertex_buffer, vertex_buffer_mem, 0).unwrap() };

        let mut ubufs: Vec<vk::Buffer> = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);
        let mut ubufs_mem: Vec<vk::DeviceMemory> = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);
        let mut ubufs_map: Vec<*mut ffi::c_void> = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);


        let buf_info = vk::BufferCreateInfo {
            flags: vk::BufferCreateFlags::default(),
            size: size_of::<UniformBufferObject>() as u64,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()};
        let buf_mem_info = vk::MemoryAllocateInfo {
            allocation_size: buf_info.size,
            memory_type_index: 0, // todo! actually check memtype
            ..Default::default()};

        (0..MAX_FRAMES_IN_FLIGHT).for_each(|_|{
            let buf = unsafe { self.device.create_buffer(&buf_info, None).unwrap() };
            let mem = unsafe { self.device.allocate_memory(&buf_mem_info, None).unwrap() };
            unsafe { self.device.bind_buffer_memory(buf,mem,0).unwrap() };
            let map = unsafe { self.device.map_memory(mem, 0, buf_info.size, vk::MemoryMapFlags::default()).unwrap() };
            ubufs.push(buf);
            ubufs_mem.push(mem);
            ubufs_map.push(map);
        });


        let sets = vec![descriptor_set_layout,descriptor_set_layout];

        let descriptor_set_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool,
            descriptor_set_count: MAX_FRAMES_IN_FLIGHT,
            p_set_layouts: sets.as_ptr(),
            ..Default::default()};


        let descriptor_sets = unsafe { self.device.allocate_descriptor_sets(&descriptor_set_info).unwrap() };
        println!("desc set alloc done");

        (0..MAX_FRAMES_IN_FLIGHT).for_each(|idx|{
            let buf_info = vk::DescriptorBufferInfo {
                buffer: ubufs[idx as usize],
                offset: 0,
                range: size_of::<UniformBufferObject>() as u64,
            };

            let img_info = vk::DescriptorImageInfo {
                sampler: screencast.unwrap().sampler,
                image_view: screencast.unwrap().view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            };

            let descriptor_write_ubo = vk::WriteDescriptorSet {
                dst_set: descriptor_sets[idx as usize],
                dst_binding: 0,
                dst_array_element: 0,
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                p_image_info: ptr::null(),
                p_buffer_info: &buf_info,
                p_texel_buffer_view: ptr::null(),
                ..Default::default()};

            let descriptor_write_img = vk::WriteDescriptorSet {
                dst_set: descriptor_sets[idx as usize],
                dst_binding: 1,
                dst_array_element: 0,
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                p_image_info: &img_info,
                ..Default::default()};
            let descriptor_writes: Vec<vk::WriteDescriptorSet> = vec![descriptor_write_ubo,descriptor_write_img];
            unsafe { self.device.update_descriptor_sets(descriptor_writes.as_slice(), &[]) };
        });


        let cmd_alloc_info = vk::CommandBufferAllocateInfo {
            command_pool: self.command_pool,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: MAX_FRAMES_IN_FLIGHT,
            ..Default::default()};
        let command_buffers = unsafe { self.device.allocate_command_buffers(&cmd_alloc_info).unwrap() };


        (window.id(), PerWindow { window, surface,
            swapchain: PerSwapchain {
                handle: swapchain, format, extent, images, views, framebuffers, sync },
            render_pass,
            pipeline, layout,
            command_buffers,
            vertex_buffer,
            vertex_buffer_mem,
            push_constant_range,
            descriptor_set_layout,
            ubufs,
            ubufs_mem,
            ubufs_map,
            descriptor_pool,
            descriptor_sets,
            id: 0,
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

