mod util;
mod experimental;

use std::collections::HashMap;
use util::per_window::PerWindow;

use crate::util::logging::{ConsoleLogger, Logged};
use crate::util::per_window::{WindowBuilder};
use crate::util::windows_ffi::WindowsFFI;
use ash::{ext, vk};
use ash::Instance;
use ash::{khr, Device, Entry};
use colored::Colorize;
use log::{debug, error, info, warn, LevelFilter};
use std::error::Error;
use std::ffi::{c_char, CStr};
use std::{env, mem, ptr};
use std::borrow::Cow;
use std::cell::{LazyCell, OnceCell};
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex, Once, OnceLock};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, Size};
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::WindowId;
use once_cell::unsync::Lazy;
use crate::util::helpers::{record_into_buffer, recreate_swapchain};
use crate::util::swapchain::PerSwapchain;

const APPLICATION_TITLE: &str = "EMBER";
const WINDOW_COUNT: usize = 3;
const MAX_FRAMES_IN_FLIGHT: u32 = 2;

const VALIDATION_LAYERS: [&CStr;1] = [
    //c"VK_LAYER_LUNARG_api_dump",
    //c"VK_LAYER_KHRONOS_synchronization2",
    c"VK_LAYER_KHRONOS_validation",
    //c"VK_LAYER_LUNARG_monitor",
    //c"VK_LAYER_KHRONOS_profiles",
    //c"VK_LAYER_KHRONOS_shader_object",
];
const REQUIRED_EXTENSIONS: [&CStr; 1] = [
    khr::surface::NAME,];
const OPTIONAL_EXTENSIONS: [&CStr; 2] = [
    ext::debug_utils::NAME,
    ext::debug_report::NAME,];
const REQUIRED_DEVICE_EXTENSIONS: [&CStr; 1] = [
    khr::swapchain::NAME,];
const OPTIONAL_DEVICE_EXTENSIONS: [&CStr; 1] = [
    ext::device_address_binding_report::NAME,];

static OPT_EXT_LOCK: Mutex<Vec<&CStr>> = Mutex::new(vec![]);

static LOGGER: ConsoleLogger = ConsoleLogger;

#[derive(Debug)]
struct SyncWrapper(RawDisplayHandle);
unsafe impl Sync for SyncWrapper {}

struct Antistatic<T>(T);
impl<T> Antistatic<T> {
    unsafe fn mutate(&self, val: T) {
        self.0 = val;
    }
}
impl<T> Deref for Antistatic<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &*self.0
    }
}

static DISPLAY_HANDLE: SyncWrapper = SyncWrapper(unsafe { mem::zeroed() });

static ENTRY: LazyLock<Entry> = LazyLock::new(Entry::linked);

static INSTANCE: OnceLock<Instance> = OnceLock::new();


static DEBUG_UTILS_CREATE_INFO: LazyLock<vk::DebugUtilsMessengerCreateInfoEXT> = LazyLock::new(||vk::DebugUtilsMessengerCreateInfoEXT {
    message_severity: { type Flags = vk::DebugUtilsMessageSeverityFlagsEXT;
        Flags::VERBOSE
    },
    message_type: { type Flags = vk::DebugUtilsMessageTypeFlagsEXT;
        Flags::GENERAL | Flags::DEVICE_ADDRESS_BINDING | Flags::PERFORMANCE | Flags::VALIDATION
    },
    pfn_user_callback: Some(util::logging::debug_callback),
    p_user_data: ptr::null_mut(),
    ..Default::default()});


fn create_instance() -> Result<Instance, Box<dyn Error>> {
    let extensions: Vec<*const c_char> = {
        let mut opt_ext_lock = OPT_EXT_LOCK.lock().unwrap();
        //(platform-dependent!) extension for surface creation.
        let mut prerequisite = match &DISPLAY_HANDLE.0 {
            RawDisplayHandle::Windows(_) => khr::win32_surface::NAME,
            RawDisplayHandle::Xlib(_) => khr::xlib_surface::NAME,
            RawDisplayHandle::Xcb(_) => khr::xcb_surface::NAME,
            RawDisplayHandle::Wayland(_) => khr::wayland_surface::NAME,
            tmp => { error!("Support for {} is unimplemented",format!("{:?}",tmp).bright_purple()); panic!() }};
        //when shadowing a variable, it's allowed to own references to the previous binding.
        let available: Vec<vk::ExtensionProperties> = unsafe { ENTRY.enumerate_instance_extension_properties(None).unwrap() };
        let available: Vec<&CStr> = available.iter().map(|ext|ext.extension_name_as_c_str().unwrap()).collect();
        //checking if extensions we want are available, then storing the raw pointers
        let mut extensions: Vec<*const c_char> = Vec::with_capacity(1+REQUIRED_EXTENSIONS.len()+OPTIONAL_EXTENSIONS.len());
        if available.contains(&prerequisite) { extensions.push(prerequisite.as_ptr()) }
        else { error!("Prerequisite extension {} unavailable!", format!("{:?}",prerequisite).bright_purple()); panic!() }
        for required in REQUIRED_EXTENSIONS { if available.contains(&required) { extensions.push(required.as_ptr()) }
        else { error!("Required extension {} unavailable!", format!("{:?}",required).bright_purple()); panic!() } }
        for optional in OPTIONAL_EXTENSIONS { if available.contains(&optional) { extensions.push(optional.as_ptr()) }
        else {
            opt_ext_lock.push(optional);
            error!("Optional extension {} unavailable; Corresponding features {}",format!("{:?}",optional).bright_purple(),"locked".red()) } }
        extensions
    };



    let layers = {
        //same idea as in the "extensions"-block.
        let available: Vec<vk::LayerProperties> = unsafe { ENTRY.enumerate_instance_layer_properties()? };
        let available: Vec<&CStr> = available.iter().map(|layer|layer.layer_name_as_c_str().unwrap()).collect();
        VALIDATION_LAYERS.iter().filter_map(|layer| {
            if available.contains(layer) { Some(layer.as_ptr()) }
            else { warn!("Validation Layer {} is unavailable",format!("{:?}",layer).bright_purple()); None }
        }).collect::<Vec<*const c_char>>()
    };
    let app_info = vk::ApplicationInfo {
        p_application_name: APPLICATION_TITLE.as_ptr().cast(),
        api_version: vk::make_api_version(0,1,0,0),
        ..Default::default()};
    let create_info = vk::InstanceCreateInfo {
        p_next: ptr::from_ref(&DEBUG_UTILS_CREATE_INFO).cast(),
        p_application_info: &app_info,
        pp_enabled_extension_names: extensions.as_ptr(),
        enabled_extension_count: extensions.len() as u32,
        pp_enabled_layer_names: layers.as_ptr(),
        enabled_layer_count: layers.len() as u32,
        ..Default::default()};

    Ok(unsafe { ENTRY.create_instance(&create_info, None)? })
}

fn main() -> Result<(),Box<dyn Error>>
{
    ansi_term::enable_ansi_support().unwrap();
    unsafe { env::set_var("COLORTERM","truecolor"); }

    log::set_logger(&LOGGER)?;
    log::set_max_level(LevelFilter::Trace);

    let event_loop = EventLoop::new()?;


    static VAL: Antistatic<bool> = Antistatic(false);
    dbg!(&VAL);
    *VAL = true;
    dbg!(&VAL);

    //i'm sorry
    println!("before display handle set");
    unsafe { (ptr::from_ref(&DISPLAY_HANDLE) as *mut SyncWrapper).write(SyncWrapper(event_loop.display_handle()?.as_raw())) };

    println!("before instance set");
    _ = INSTANCE.set(create_instance()?);

    let mut opt_device_ext_lock: Vec<&CStr> = Vec::with_capacity(0);
    // todo!    MAKE THIS SECTION LESS FUCKING UGLY
    let rated_devices: Vec<(u32,vk::PhysicalDevice,vk::PhysicalDeviceProperties,vk::PhysicalDeviceFeatures,Vec<*const c_char>)> = unsafe {
        INSTANCE.get_or_init(||create_instance().unwrap()).enumerate_physical_devices()?
            .iter().filter_map(|device|{
            let mut rating = 0u32;

            let properties = INSTANCE.get_or_init(||create_instance().unwrap()).get_physical_device_properties(*device);
            let features   = INSTANCE.get_or_init(||create_instance().unwrap()).get_physical_device_features(*device);
            let available_extensions: Vec<vk::ExtensionProperties> = INSTANCE.get_or_init(||create_instance().unwrap()).enumerate_device_extension_properties(*device).unwrap();
            let available_extensions: Vec<&CStr> = available_extensions.iter().map(|properties|properties.extension_name_as_c_str().unwrap()).collect();
            let mut extensions: Vec<*const c_char> = Vec::with_capacity(
                REQUIRED_DEVICE_EXTENSIONS.len()+OPTIONAL_DEVICE_EXTENSIONS.len());
            for ext in REQUIRED_DEVICE_EXTENSIONS {
                if !available_extensions.contains(&ext) {
                    warn!("Device {} is unsuitable because extension {} is missing.",
                        format!("{:?}",properties.device_name_as_c_str().unwrap()).bright_purple(),
                        format!("{:?}",ext).bright_purple());
                    return None }
                extensions.push(ext.as_ptr())}
            for ext in OPTIONAL_DEVICE_EXTENSIONS {
                if !available_extensions.contains(&ext) {
                    warn!("Device {} doesn't support extension {}, rating adjusted accordingly.",
                        format!("{:?}",properties.device_name_as_c_str().unwrap()).bright_purple(),
                        format!("{:?}",ext).bright_purple());
                    opt_device_ext_lock.push(ext);
                    rating += 1 } else {
                    extensions.push(ext.as_ptr())}}

            Some((rating,*device,properties,features,extensions))}).collect()};
    let device_opt = {
        let mut best_rating = u32::MAX;
        rated_devices.iter().for_each(|(rating,..)|{ best_rating = best_rating.min(*rating) });
        let suitable_devices = rated_devices.iter().filter_map(|(rating,device,properties,features,extensions)|{
            if *rating > best_rating { return None }
            Some((device,properties,features,extensions)) });
        suitable_devices.min_by_key(|(_,properties,_,_)|{
            type Type = vk::PhysicalDeviceType;
            match properties.device_type {
                Type::DISCRETE_GPU => { 1 }
                Type::INTEGRATED_GPU => { 2 }
                Type::VIRTUAL_GPU => { 3 }
                Type::CPU => { 4 }
                _ => { 5 }
            }})};

    #[allow(unused)]
    let Some((&phys_device,&phys_device_properties,&phys_device_features,phys_device_extensions)) = device_opt
        else { error!("No suitable device found!"); panic!()  };

    // todo!("check for valid queue families during physical device selection")
    // todo!("deal with presentation support and possible dedicated queues per task")
    let queue_family_index = unsafe {
        let queue_families = INSTANCE.get_or_init(||create_instance().unwrap()).get_physical_device_queue_family_properties(phys_device);
        queue_families.iter().enumerate().filter_map(|(usize,&properties)| {
            properties.queue_flags.contains(vk::QueueFlags::GRAPHICS).then_some(usize)
        }).next().unwrap()} as u32;
    let device_queue_create_info = vk::DeviceQueueCreateInfo {
        queue_family_index,
        queue_count: 1,
        p_queue_priorities: &1f32,
        ..Default::default()};
    let mut address_debug_info: Option<vk::PhysicalDeviceAddressBindingReportFeaturesEXT> = None;
    if !opt_device_ext_lock.contains(&ext::device_address_binding_report::NAME) {
        address_debug_info = Some(
            vk::PhysicalDeviceAddressBindingReportFeaturesEXT {
                report_address_binding: vk::TRUE,
                ..Default::default()})};

    let device_create_info = vk::DeviceCreateInfo {
        p_next: if let Some(address_debug_info) = address_debug_info {
            ptr::from_ref(&address_debug_info).cast() } else { ptr::null() },
        queue_create_info_count: 1,
        p_queue_create_infos: &device_queue_create_info,
        enabled_extension_count: phys_device_extensions.len() as u32,
        pp_enabled_extension_names: phys_device_extensions.as_ptr(),
        p_enabled_features: &phys_device_features,
        ..Default::default()};

    let device = unsafe { INSTANCE.get_or_init(||create_instance().unwrap()).create_device(phys_device, &device_create_info, None).logged("Logical device creation failed") };
    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };






    let extension_holder = {
        let opt_ext_lock = OPT_EXT_LOCK.lock().unwrap();
        ExtensionHolder {
            surface: khr::surface::Instance::new(&ENTRY,&INSTANCE.get_or_init(||create_instance().unwrap())),
            os_surface: match event_loop.display_handle()?.as_raw() {
                RawDisplayHandle::Windows(_) => OSSurface::WINDOWS(khr::win32_surface::Instance::new(&ENTRY,&INSTANCE.get_or_init(||create_instance().unwrap()))),
                RawDisplayHandle::Wayland(_) => OSSurface::WAYLAND(khr::wayland_surface::Instance::new(&ENTRY,&INSTANCE.get_or_init(||create_instance().unwrap()))),
                RawDisplayHandle::Xcb(_)     => OSSurface::XCB(khr::xcb_surface::Instance::new(&ENTRY,&INSTANCE.get_or_init(||create_instance().unwrap()))),
                RawDisplayHandle::Xlib(_)    => OSSurface::XLIB(khr::xlib_surface::Instance::new(&ENTRY,&INSTANCE.get_or_init(||create_instance().unwrap()))),
                _ => { unreachable!() }},
            debug_utils: (!opt_ext_lock.contains(&ext::debug_utils::NAME)).then(||
                ext::debug_utils::Instance::new(&ENTRY,&INSTANCE.get_or_init(||create_instance().unwrap()))),
            debug_report: (!opt_ext_lock.contains(&ext::debug_report::NAME)).then(||
                ext::debug_report::Instance::new(&ENTRY,&INSTANCE.get_or_init(||create_instance().unwrap()))),
            swapchain: khr::swapchain::Device::new(&INSTANCE.get_or_init(||create_instance().unwrap()),&device),
        }};

    let command_pool_info = vk::CommandPoolCreateInfo {
        //declare that we want to reset singular/specific command buffers in the pool, instead of everything at once
        flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
        queue_family_index,
        ..Default::default()};
    let command_pool = unsafe { device.create_command_pool(&command_pool_info,None).unwrap() };

    //in case we either don't want a debug messenger or the related extension isn't available.
    //otherwise we put the messenger this Option<> here
    let mut debug_messenger: Option<vk::DebugUtilsMessengerEXT> = None;
    if let Some(debug_utils) = extension_holder.debug_utils.as_ref() {
        debug_messenger = match unsafe { debug_utils.create_debug_utils_messenger(&DEBUG_UTILS_CREATE_INFO, None) } {
            Ok(debug_messenger) => Some(debug_messenger),
            Err(e) => { error!("Debug Messenger creation failed: {:?}; Execution will continue without it.",e); None }
        }}


    info!("Using Device {}",format!("{:?}",phys_device_properties.device_name_as_c_str().unwrap()).bright_purple());


    //THIS IS THE LAST THING THAT ENDS UP RUNNING IN HERE - AFTER THIS, IT'S OFF TO THE WINDOW EVENT LOOP
    //and once the event loop exits we also exit the actual application
    match event_loop.run_app(&mut App {
        //instance,
        device,
        physical_device: phys_device,
        queue,
        command_pool,

        windows: HashMap::with_capacity(WINDOW_COUNT),

        ext: extension_holder,
        win32_fp: None,

        debug_messenger,
        current_frame: 0,
    })
    {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e))
    }
}









pub(crate) struct App {
    #[allow(unused)]
    //instance: Instance,
    device: Device,
    //i *think* there's no way to retrieve the physical device handle from a logical device
    physical_device: vk::PhysicalDevice,
    queue: vk::Queue,
    command_pool: vk::CommandPool,

    windows: HashMap<WindowId,PerWindow>,

    ext: ExtensionHolder,
    #[allow(unused)]
    win32_fp: Option<WindowsFFI>,

    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,

    current_frame: usize,
}

struct ExtensionHolder {
    surface: khr::surface::Instance,
    os_surface: OSSurface,
    debug_utils: Option<ext::debug_utils::Instance>,
    debug_report: Option<ext::debug_report::Instance>,
    // i guess i'll put device level functions into the same struct as instance level functions?
    swapchain: khr::swapchain::Device,
}
enum OSSurface {
    WINDOWS(khr::win32_surface::Instance),
    WAYLAND(khr::wayland_surface::Instance),
    XCB(khr::xcb_surface::Instance),
    XLIB(khr::xlib_surface::Instance),
}


unsafe fn cleanup(
    instance: &Instance, ext: &ExtensionHolder,
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    device: &Device
) {
    if let (Some(debug_utils),Some(debug_messenger)) = (ext.debug_utils.as_ref(),debug_messenger) {
        debug_utils.destroy_debug_utils_messenger(debug_messenger,None);
    }
    device.destroy_device(None);
    instance.destroy_instance(None);
}





#[allow(unused)]
impl ApplicationHandler for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {

        let mut builder = WindowBuilder::new(&self.ext,&self.device,self.physical_device,self.command_pool);
        builder.attributes = builder.attributes
            .with_title(APPLICATION_TITLE)
            .with_active(true)
            .with_transparent(true)
            .with_inner_size(Size::Logical(LogicalSize::new(400f64,400f64)));

        let mut window_count = WINDOW_COUNT;
        if WINDOW_COUNT > 5 {
            window_count -= 1;
        }

        (0..window_count).for_each(|idx| {
            builder.attributes.title = format!("{}  #{}",APPLICATION_TITLE,idx+1);
            let (window_id,per_window) = builder.build(event_loop);
            let fp = unsafe { WindowsFFI::load_function_pointers() };
            per_window.toggle_blur(&fp);
            _ = self.windows.insert(window_id,per_window);
        });

        // IF we create more than 5 windows. just for funsies + so that whoever's trying this out knows why there's so many windows being created
        if WINDOW_COUNT != window_count {
            debug!("THE LARGE AMOUNT OF WINDOWS IS INTENTIONAL.");
            info!("by the way, that above was on \"{}\" due to the color being highly visible, not because of it being debugging-related.","DEBUG".bright_cyan());
            builder.attributes.title = "yes, this is intentional".to_owned();
            let (window_id,per_window) = builder.build(event_loop);
            _ = self.windows.insert(window_id,per_window)
        }


    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        let per_window = self.windows.get_mut(&window_id);
        //early return, in case none of our windows match the window id of the current window event
        let Some(per_window) = per_window else { return };
        //we can safely pattern-match the unwrapped struct, because we already tested whether it has a value.
        let PerWindow {
            window,
            surface,
            ref swapchain,
            render_pass,
            pipeline,
            layout,
            command_buffers
        } = per_window;

        match event {
            WindowEvent::KeyboardInput { event, .. } =>
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    match keycode {
                        KeyCode::Escape => { self.window_event(event_loop, window_id, WindowEvent::CloseRequested) }
                        _ => {}
                    }
            }
            WindowEvent::CloseRequested => {
                //fetching the number inside WindowId structs using unsafe code. Why? makes the console output look better.
                //this should also cause UB if you use any system with 32bit window handles/IDs.
                //  issue is, if other systems get implemented that don't use a 64bit value as their window handle,
                //  this would guarantee UB whenever a window gets closed
                warn!(
                    "Closing Window with {}",
                    format!("ID {}",unsafe {mem::transmute_copy::<_,isize>(&window_id) }).bright_purple());

                let PerWindow {surface,layout,pipeline,render_pass,swapchain, .. } = self.windows.remove(&window_id).unwrap();
                unsafe {
                    //VERY IMPORTANT! otherwise, we'd try cleaning up semaphores n stuff while they're still in use
                    self.device.device_wait_idle().unwrap();

                    self.device.destroy_pipeline(pipeline,None);
                    self.device.destroy_pipeline_layout(layout,None);

                    swapchain.cleanup(&self.device, &self.ext.swapchain);

                    self.device.destroy_render_pass(render_pass,None);
                    self.ext.surface.destroy_surface(surface,None);
                }
                if self.windows.len() == 0 { event_loop.exit() };
            }
            WindowEvent::Resized(_) => {
                //should probably do some of the swapchain recreation here
                //although that's left for a later date: todo!
                per_window.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let device = &self.device;
                let ext = &self.ext;

                unsafe { device.wait_for_fences(&[swapchain.sync[self.current_frame].in_flight],true,u64::MAX).unwrap() };

                let next = unsafe {
                    match ext.swapchain.acquire_next_image(swapchain.handle, u64::MAX, swapchain.sync[self.current_frame].swapchain.clone(), vk::Fence::null()) {
                        Ok((next,false)) => { next }
                        Ok(_) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                            recreate_swapchain(
                                &self.device,
                                self.physical_device,
                                per_window,
                                &self.ext.surface,
                                &self.ext.swapchain
                            );
                            let (next,_) = ext.swapchain.acquire_next_image(per_window.swapchain.handle, u64::MAX, per_window.swapchain.sync[self.current_frame].swapchain, vk::Fence::null()).unwrap();
                            next
                        }
                        Err(e) => { error!("Swapchain recreation failed fatally: {:?}",e); panic!() }   // todo!    EMERGENCY CLEANUP
                    }
                };

                //reborrow contents of per_window to allow swapchain recreation to actually work without fucking up the borrow checker
                let PerWindow {
                    window,
                    surface,
                    swapchain,
                    render_pass,
                    pipeline,
                    layout,
                    command_buffers
                } = per_window;

                unsafe { device.reset_fences(&[swapchain.sync[self.current_frame].in_flight]).unwrap() };



                unsafe { device.reset_command_buffer(command_buffers[self.current_frame],Default::default()).unwrap() };
                unsafe { record_into_buffer(device,window,*pipeline,*render_pass,swapchain.framebuffers[next as usize],swapchain.extent,command_buffers[self.current_frame],next) };

                window.pre_present_notify();

                let submit_info = vk::SubmitInfo {
                    wait_semaphore_count: 1,
                    p_wait_semaphores: ptr::from_ref(&swapchain.sync[self.current_frame].swapchain),
                    p_wait_dst_stage_mask: ptr::from_ref(&vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT),
                    command_buffer_count: 1,
                    p_command_buffers: ptr::from_ref(&command_buffers[self.current_frame]),
                    signal_semaphore_count: 1,
                    p_signal_semaphores: ptr::from_ref(&swapchain.sync[self.current_frame].presentation),
                    ..Default::default()};
                unsafe { device.queue_submit(self.queue,&[submit_info], swapchain.sync[self.current_frame].in_flight).unwrap() };

                let present_info = vk::PresentInfoKHR {
                    wait_semaphore_count: 1,
                    p_wait_semaphores: ptr::from_ref(&swapchain.sync[self.current_frame].presentation),
                    swapchain_count: 1,
                    p_swapchains: ptr::from_ref(&swapchain.handle),
                    p_image_indices: ptr::from_ref(&next),
                    p_results: ptr::null_mut(),
                    ..Default::default()};

                unsafe { match ext.swapchain.queue_present(self.queue,&present_info) {
                    Ok(_) => {}
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        recreate_swapchain(
                            &self.device,
                            self.physical_device,
                            per_window,
                            &self.ext.surface,
                            &self.ext.swapchain
                        );
                    }
                    Err(e) => { error!("Swapchain recreation failed fatally: {:?}",e); panic!() }   // todo!    EMERGENCY CLEANUP
                }};
                self.current_frame += 1;
                self.current_frame %= MAX_FRAMES_IN_FLIGHT as usize;
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
        info!("Cleaning up...");
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device.destroy_command_pool(self.command_pool,None);
            cleanup(INSTANCE.get().unwrap_unchecked(),&self.ext,self.debug_messenger,&self.device);
        }

    }
}
