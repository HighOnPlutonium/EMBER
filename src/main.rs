mod util;

use std::collections::HashMap;
use util::per_window::PerWindow;

use std::error::Error;
use std::ops::Deref;
use ash::{khr, Device, Entry};
use ash::Instance;
use ash::khr::swapchain;
use ash::vk;
use ash::vk::SurfaceKHR;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowId};
use crate::util::per_window::WindowBuilder;
use crate::util::windows_ffi::WindowsFFI;

const APPLICATION_TITLE: &str = "EMBER";
const WINDOW_COUNT: usize = 1;

fn main() {
    let event_loop = event_loop::EventLoop::new().unwrap();
    let mut app = App::new(&event_loop).unwrap();
    event_loop.run_app(&mut app).unwrap();
}

struct App {
    entry: Entry,
    instance: Instance,

    per_window: HashMap<WindowId,PerWindow>,

    windows_function_pointers: Option<WindowsFFI>,
    root: Option<WindowId>,
    new_window_id: Option<WindowId>,
}


impl App {
    /// Done here:
    /// - Entrypoint Loading
    /// - Instance Creation
    ///
    /// Maybe(?) done here:
    /// - Function Pointer Loading
    /// - Debug Callbacks
    fn new(event_loop: &EventLoop<()>) -> Result<Self,Box<dyn Error>> {

        //ENTRYPOINT LOADING
        let entry = Entry::linked();
        let app_info = vk::ApplicationInfo {
            p_application_name: APPLICATION_TITLE.as_ptr().cast(),
            //application_version: 0,
            //p_engine_name: (),
            //engine_version: 0,
            api_version: vk::make_api_version(0,1,0,0),
            ..Default::default()
        };
        let enabled_extensions = ash_window::enumerate_required_extensions(event_loop.display_handle()?.as_raw())?.to_vec();
        let create_info = vk::InstanceCreateInfo {
            p_application_info: &app_info,
            pp_enabled_extension_names: enabled_extensions.as_ptr(),
            enabled_extension_count: enabled_extensions.len() as _,
            ..Default::default()
        };
        //INSTANCE CREATION
        let instance = unsafe { entry.create_instance(&create_info, None)? };

        Ok(Self {entry, instance, per_window: HashMap::with_capacity(WINDOW_COUNT), windows_function_pointers: None, root: None, new_window_id: None})
    }
}


impl ApplicationHandler for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
    }

    /// Done here:
    /// - Window Creation
    ///
    /// Maybe(?) done here:
    /// -
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {

        let mut builder = WindowBuilder::new(event_loop, &self.entry, &self.instance);
        builder.attributes = builder.attributes
            .with_title(APPLICATION_TITLE)
            .with_active(true)
            .with_transparent(true)
            .with_decorations(false)
            .with_undecorated_shadow(true);

        (0..WINDOW_COUNT).for_each(|_|{ (|(x,y)|self.per_window.insert(x,y))(builder.build()); });
        unsafe {
            self.windows_function_pointers = Some(WindowsFFI::load_function_pointers());
            self.per_window.iter().enumerate()
                .for_each(|(idx,(_, &ref per_window))| {
                    if idx == 0 {
                        self.root = Some(per_window.window.id())
                    }
                    per_window.toggle_blur(&self.windows_function_pointers.as_ref().unwrap());
                    per_window.window.set_title(format!("{} - #{}", per_window.window.title(), idx + 1).as_ref());
                });
        }
        {
            self.per_window.iter()
                .for_each(|(_, PerWindow { window, surface })| {
                    self.we_dont_talk_about_this(window,surface);
                });
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        let per_window = self.per_window.get(&window_id);
        //early return, in case none of our windows match the window id of the current window event
        if per_window.is_none() { return };
        //we can safely pattern-match the unwrapped struct, because we already tested whether it has a value.
        let PerWindow {window, surface} = per_window.unwrap();
        match event {
            WindowEvent::Focused(true) => { self.root = Some(window_id); }
            WindowEvent::KeyboardInput { event, .. } =>
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    match keycode {
                        KeyCode::NumpadAdd => {
                            if (self.root.unwrap() != window_id)
                            | !event.state.is_pressed()
                            |   event.repeat { return }
                            let mut builder = WindowBuilder::new(event_loop, &self.entry, &self.instance);
                            builder.attributes = builder.attributes
                                .with_title::<&str>(format!("{} - #{}", APPLICATION_TITLE, self.per_window.len() + 1).as_ref())
                                .with_active(true)
                                .with_transparent(true)
                                .with_decorations(false)
                                .with_undecorated_shadow(true);
                            let (new_window_id,new_window) = builder.build();
                            new_window.toggle_blur(&self.windows_function_pointers.as_ref().unwrap());
                            self.per_window.insert(new_window_id.clone(),new_window);
                            self.new_window_id = Some(new_window_id.clone());
                            self.window_event(event_loop, new_window_id, WindowEvent::RedrawRequested)
                        }
                        KeyCode::ShiftLeft => { if !event.repeat { self.per_window.iter().for_each(|per_window|per_window.1.window.set_decorations(event.state.is_pressed())) } }
                        KeyCode::Escape => { self.window_event(event_loop, window_id, WindowEvent::CloseRequested) }
                        _ => {}
                    }
            }
            WindowEvent::CloseRequested => {
                self.per_window.remove(&window_id);
                if self.per_window.len() == 0 { event_loop.exit() };
            }
            WindowEvent::Resized(_) => {
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                //draw calls
                window.pre_present_notify();
                //swapchain submit
                if self.new_window_id.is_some() { if window_id == self.new_window_id.unwrap() { self.new_window_id = None; self.we_dont_talk_about_this(window,surface); } }

            }

            _ => {}
        }
    }

    fn device_event(&mut self, event_loop: &ActiveEventLoop, device_id: DeviceId, event: DeviceEvent) {
        //dbg!(&event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        unsafe { self.instance.destroy_instance(None); }
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        println!("MEMORY WARNING");
    }
}

































































































































































































































































































































































































































































































impl App {
    fn we_dont_talk_about_this(&self, window: &Window, surface: &SurfaceKHR) {
        unsafe {
            let pdevices = self.instance
                .enumerate_physical_devices()
                .expect("Physical device error");
            let surface_loader = khr::surface::Instance::new(&self.entry, &self.instance);
            let (pdevice, queue_family_index) = pdevices
                .iter()
                .find_map(|pdevice| {
                    self.instance
                        .get_physical_device_queue_family_properties(*pdevice)
                        .iter()
                        .enumerate()
                        .find_map(|(index, info)| {
                            let supports_graphic_and_surface =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                    && surface_loader
                                    .get_physical_device_surface_support(
                                        *pdevice,
                                        index as u32,
                                        *surface,
                                    )
                                    .unwrap();
                            if supports_graphic_and_surface {
                                Some((*pdevice, index))
                            } else {
                                None
                            }
                        })
                })
                .expect("Couldn't find suitable device.");
            let queue_family_index = queue_family_index as u32;
            let device_extension_names_raw = [
                swapchain::NAME.as_ptr(),
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                ash::khr::portability_subset::NAME.as_ptr(),
            ];
            let features = vk::PhysicalDeviceFeatures {
                shader_clip_distance: 1,
                ..Default::default()
            };
            let priorities = [1.0];

            let queue_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family_index)
                .queue_priorities(&priorities);

            let device_create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(std::slice::from_ref(&queue_info))
                .enabled_extension_names(&device_extension_names_raw)
                .enabled_features(&features);

            let device: Device = self.instance
                .create_device(pdevice, &device_create_info, None)
                .unwrap();

            let present_queue = device.get_device_queue(queue_family_index, 0);

            let surface_format = surface_loader
                .get_physical_device_surface_formats(pdevice, *surface)
                .unwrap()[0];

            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities(pdevice, *surface)
                .unwrap();
            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0
                && desired_image_count > surface_capabilities.max_image_count
            {
                desired_image_count = surface_capabilities.max_image_count;
            }
            let surface_resolution = match surface_capabilities.current_extent.width {
                u32::MAX => vk::Extent2D {
                    width: window.inner_size().width,
                    height: window.inner_size().height,
                },
                _ => surface_capabilities.current_extent,
            };
            let pre_transform = if surface_capabilities
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capabilities.current_transform
            };
            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(pdevice, *surface)
                .unwrap();
            let present_mode = present_modes
                .iter()
                .cloned()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(vk::PresentModeKHR::FIFO);
            let swapchain_loader = swapchain::Device::new(&self.instance, &device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
                .surface(*surface)
                .min_image_count(desired_image_count)
                .image_color_space(surface_format.color_space)
                .image_format(surface_format.format)
                .image_extent(surface_resolution)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(pre_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .image_array_layers(1);

            let swapchain = swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap();

            let pool_create_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_family_index);

            let pool = device.create_command_pool(&pool_create_info, None).unwrap();

            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_buffer_count(1)
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY);

            let command_buffers = device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap();
            let setup_command_buffer = command_buffers[0];

            let present_images = swapchain_loader.get_swapchain_images(swapchain).unwrap();

            let fence_create_info =
                vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

            let setup_commands_reuse_fence = device
                .create_fence(&fence_create_info, None)
                .expect("Create fence failed.");

            pub fn record_submit_commandbuffer<F: FnOnce(&Device, vk::CommandBuffer)>(
                device: &Device,
                command_buffer: vk::CommandBuffer,
                command_buffer_reuse_fence: vk::Fence,
                submit_queue: vk::Queue,
                wait_mask: &[vk::PipelineStageFlags],
                wait_semaphores: &[vk::Semaphore],
                signal_semaphores: &[vk::Semaphore],
                f: F,
            ) {
                unsafe {
                    device
                        .wait_for_fences(&[command_buffer_reuse_fence], true, u64::MAX)
                        .expect("Wait for fence failed.");

                    device
                        .reset_fences(&[command_buffer_reuse_fence])
                        .expect("Reset fences failed.");

                    device
                        .reset_command_buffer(
                            command_buffer,
                            vk::CommandBufferResetFlags::RELEASE_RESOURCES,
                        )
                        .expect("Reset command buffer failed.");

                    let command_buffer_begin_info = vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

                    device
                        .begin_command_buffer(command_buffer, &command_buffer_begin_info)
                        .expect("Begin commandbuffer");
                    f(device, command_buffer);
                    device
                        .end_command_buffer(command_buffer)
                        .expect("End commandbuffer");

                    let command_buffers = vec![command_buffer];

                    let submit_info = vk::SubmitInfo::default()
                        .wait_semaphores(wait_semaphores)
                        .wait_dst_stage_mask(wait_mask)
                        .command_buffers(&command_buffers)
                        .signal_semaphores(signal_semaphores);

                    device
                        .queue_submit(submit_queue, &[submit_info], command_buffer_reuse_fence)
                        .expect("queue submit failed.");
                }
            }
            record_submit_commandbuffer(
                &device,
                setup_command_buffer,
                setup_commands_reuse_fence,
                present_queue,
                &[],
                &[],
                &[],
                |device, setup_command_buffer| {
                    let clearcolorvalue = vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 0.0] };
                    let range = vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    };
                    device.cmd_clear_color_image(setup_command_buffer, *present_images.get(0).unwrap(), vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, &clearcolorvalue, &[range]);

                    device.cmd_pipeline_barrier(
                        setup_command_buffer,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[],
                    );

                },
            );
            let swapchains = &[swapchain];
            {
                let present_info = vk::PresentInfoKHR::default()
                    .swapchains(swapchains)
                    .image_indices(&[0]);

                swapchain_loader
                    .queue_present(present_queue, &present_info)
                    .unwrap();
            }
        };
    }
}