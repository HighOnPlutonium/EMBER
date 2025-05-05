mod util;

use std::any::type_name_of_val;
use std::collections::HashMap;
use util::per_window::PerWindow;

use crate::util::logging::{ConsoleLogger, Logged};
use crate::util::per_window::WindowBuilder;
use crate::util::windows_ffi::WindowsFFI;
use ash::{ext, vk};
use ash::Instance;
use ash::{khr, Device, Entry};
use colored::Colorize;
use log::{debug, error, info, warn, LevelFilter};
use std::error::Error;
use std::ffi::{c_char, CStr, CString};
use std::hash::Hash;
use std::ops::Deref;
use std::process::exit;
use std::{env, mem, ptr, slice};
use ash::prelude::VkResult;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::raw_window_handle::{DisplayHandle, HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

const APPLICATION_TITLE: &str = "EMBER";
const WINDOW_COUNT: usize = 1;

const VALIDATION_LAYERS: [&CStr;1] = [
    c"VK_LAYER_KHRONOS_validation",
];
const REQUIRED_EXTENSIONS: [&CStr; 1] = [
    khr::surface::NAME ];
const OPTIONAL_EXTENSIONS: [&CStr; 1] = [
    ext::debug_utils::NAME ];
const REQUIRED_DEVICE_EXTENSIONS: [&CStr; 1] = [
    khr::swapchain::NAME ];

static LOGGER: ConsoleLogger = ConsoleLogger;
fn main() -> Result<(),Box<dyn Error>> {
    ansi_term::enable_ansi_support().unwrap();
    unsafe { env::set_var("COLORTERM","truecolor"); }

    log::set_logger(&LOGGER)?;
    log::set_max_level(LevelFilter::Trace);


    let event_loop = EventLoop::new()?;
    let entry = Entry::linked();



    let debug_utils_create_info = vk::DebugUtilsMessengerCreateInfoEXT {
        message_severity: { type Flags = vk::DebugUtilsMessageSeverityFlagsEXT;
            Flags::VERBOSE
        },
        message_type: { type Flags = vk::DebugUtilsMessageTypeFlagsEXT;
            Flags::GENERAL | Flags::DEVICE_ADDRESS_BINDING | Flags::PERFORMANCE | Flags::VALIDATION
        },
        pfn_user_callback: Some(util::logging::debug_callback),
        p_user_data: ptr::null_mut(),
        ..Default::default()};



    let mut opt_ext_lock: Vec<&CStr> = Vec::with_capacity(0);
    let instance = {

        let extensions: Vec<*const c_char> = {
            //(platform-dependent!) extension for surface creation.
            //originally, only the linux specific things used pattern matching, and the decision between linux and windows was made using #[cfg(target_os = )]
            let prerequisite: &CStr = match event_loop.display_handle()?.as_raw() {
                    RawDisplayHandle::Windows(_) => khr::win32_surface::NAME,
                    RawDisplayHandle::Xlib(_) => khr::xlib_surface::NAME,
                    RawDisplayHandle::Xcb(_) => khr::xcb_surface::NAME,
                    RawDisplayHandle::Wayland(_) => khr::wayland_surface::NAME,
                    tmp => { error!("Support for {} is unimplemented",format!("{:?}",tmp).bright_purple()); panic!() }};
            //when shadowing a variable, it's allowed to own references to the previous binding.
            let available: Vec<vk::ExtensionProperties> = unsafe { entry.enumerate_instance_extension_properties(None).unwrap() };
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
            let available: Vec<vk::LayerProperties> = unsafe { entry.enumerate_instance_layer_properties()? };
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
            p_next: ptr::from_ref(&debug_utils_create_info).cast(),
            p_application_info: &app_info,
            pp_enabled_extension_names: extensions.as_ptr(),
            enabled_extension_count: extensions.len() as u32,
            pp_enabled_layer_names: layers.as_ptr(),
            enabled_layer_count: layers.len() as u32,
            ..Default::default()};

        unsafe { entry.create_instance(&create_info, None)? }
    };

    let extension_holder = ExtensionHolder {
        surface: khr::surface::Instance::new(&entry,&instance),
        os_surface: match event_loop.display_handle()?.as_raw() {
            RawDisplayHandle::Windows(_) => OSSurface::WINDOWS(khr::win32_surface::Instance::new(&entry,&instance)),
            RawDisplayHandle::Wayland(_) => OSSurface::WAYLAND(khr::wayland_surface::Instance::new(&entry,&instance)),
            RawDisplayHandle::Xcb(_) => OSSurface::XCB(khr::xcb_surface::Instance::new(&entry,&instance)),
            RawDisplayHandle::Xlib(_) => OSSurface::XLIB(khr::xlib_surface::Instance::new(&entry,&instance)),
            //unreachable because we already pattern-match the same arms in the "extensions"-block.
            _ => { unreachable!() }},
        debug_utils: (!opt_ext_lock.contains(&ext::debug_utils::NAME)).then(||
            ext::debug_utils::Instance::new(&entry,&instance)),
    };


    let mut debug_messenger: Option<vk::DebugUtilsMessengerEXT> = None;
    if let Some(debug_utils) = extension_holder.debug_utils.as_ref() {
        debug_messenger = match unsafe { debug_utils.create_debug_utils_messenger(&debug_utils_create_info, None) } {
            Ok(debug_messenger) => Some(debug_messenger),
            Err(e) => { error!("Debug Messenger creation failed: {:?}; Execution will continue without it.",e); None }
        }}


    // todo!("check for presentation support")
    // todo!("DEVICE EXTENSION CHECK")
    let (phys_device,phys_device_properties,phys_device_features) = unsafe { instance
        .enumerate_physical_devices()?
        .iter().filter_map(|device|{
            let properties = unsafe { instance.get_physical_device_properties(*device) };
            let features   = unsafe { instance.get_physical_device_features(*device) };
            Some((*device,properties,features))})
        .min_by_key(|(_,properties,_)| { properties.device_type.as_raw() })
        .unwrap_or_else(||
            { error!("No suitable device found. Cannot continue without one.");
            //unsafe { cleanup(&instance,&extension_holder,debug_messenger,) }; todo!("deal with emergency cleanup")
            panic!() })};

    // todo!("check for valid queue families during physical device selection")
    // todo!("deal with presentation support and possible dedicated queues per task")
    let queue_family_index =  unsafe {
        let queue_families = instance.get_physical_device_queue_family_properties(phys_device);
        queue_families.iter().enumerate().filter_map(|(usize,&properties)| {
            properties.queue_flags.contains(vk::QueueFlags::GRAPHICS).then_some(usize)
        }).next().unwrap()};

    let device_queue_create_info = vk::DeviceQueueCreateInfo {
        queue_family_index: queue_family_index as u32,
        queue_count: 1,
        p_queue_priorities: &1f32,
        ..Default::default()};
    let device_create_info = vk::DeviceCreateInfo {
        queue_create_info_count: 1,
        p_queue_create_infos: &device_queue_create_info,
        //enabled_extension_count: 0,
        //pp_enabled_extension_names: (),
        p_enabled_features: &phys_device_features,
        ..Default::default()};
    let device = unsafe { instance.create_device(phys_device, &device_create_info, None).logged("Logical device creation failed") };
    let queue = unsafe { device.get_device_queue(queue_family_index as u32, 0) };


    match event_loop.run_app(&mut App {
        entry, instance,debug_messenger,device,queue,
        windows: HashMap::with_capacity(WINDOW_COUNT),
        win32_function_pointers: None,
        ext: extension_holder,
    })
    {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e))
    }

}



pub(crate) struct App {
    entry: Entry,
    instance: Instance,
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    device: Device,
    queue: vk::Queue,

    windows: HashMap<WindowId,PerWindow>,

    win32_function_pointers: Option<WindowsFFI>,

    ext: ExtensionHolder,
}

struct ExtensionHolder {
    surface: khr::surface::Instance,
    os_surface: OSSurface,
    debug_utils: Option<ext::debug_utils::Instance>,
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

        let mut builder = WindowBuilder::new(&self.entry,&self.instance,&self.ext);
        builder.attributes = builder.attributes
            .with_title(APPLICATION_TITLE)
            .with_active(true)
            .with_transparent(true);


        (0..WINDOW_COUNT).for_each(|_| {
            let (window_id,per_window) = builder.build(event_loop);
            _ = self.windows.insert(window_id,per_window) });


    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        let per_window = self.windows.get(&window_id);
        //early return, in case none of our windows match the window id of the current window event
        if per_window.is_none() { return };
        //we can safely pattern-match the unwrapped struct, because we already tested whether it has a value.
        let PerWindow {window, surface} = per_window.unwrap();
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
                info!(
                    "Closing Window with {} and destroying its {}",
                    format!("ID {}",unsafe {mem::transmute_copy::<_,isize>(&window_id) }).bright_purple(),
                    "vk::SurfaceKHR".bright_purple());

                let PerWindow { window: _, surface} = self.windows.remove(&window_id).unwrap();
                unsafe { self.ext.surface.destroy_surface(surface,None); }
                if self.windows.len() == 0 { event_loop.exit() };
            }
            WindowEvent::Resized(_) => {
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                window.pre_present_notify();

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
            cleanup(&self.instance,&self.ext,self.debug_messenger,&self.device);
        }

    }
}
