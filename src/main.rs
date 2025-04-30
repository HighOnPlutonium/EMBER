mod util;

use std::any::type_name_of_val;
use std::collections::HashMap;
use util::per_window::PerWindow;

use crate::util::per_window::WindowBuilder;
use crate::util::windows_ffi::WindowsFFI;
use crate::util::logging::ConsoleLogger;
use ash::khr::swapchain;
use ash::vk;
use ash::Instance;
use ash::{khr, Device, Entry};
use std::error::Error;
use std::ffi::{c_char, CStr, CString};
use std::hash::Hash;
use std::ops::Deref;
use std::slice;
use colored::Colorize;
use log::{debug, error, info, warn, LevelFilter};

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::raw_window_handle::{DisplayHandle, HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

const APPLICATION_TITLE: &str = "EMBER";
const WINDOW_COUNT: usize = 2;

const VALIDATION_LAYERS: [&CStr;1] = [
    c"VK_LAYER_KHRONOS_validation", ];
const REQUIRED_EXTENSIONS: [&CStr; 1] = [
    khr::surface::NAME ];
const OPTIONAL_EXTENSIONS: [&CStr; 0] = [];

static LOGGER: ConsoleLogger = ConsoleLogger;
fn main() -> Result<(),Box<dyn Error>> {
    log::set_logger(&LOGGER)?;
    log::set_max_level(LevelFilter::Trace);

    let event_loop = EventLoop::new()?;
    let entry = Entry::linked();

    let instance = {
        let extensions: Vec<*const c_char> = {
            let prerequisite: &CStr = {
                #[cfg(target_os = "windows")]
                { khr::win32_surface::NAME }
                #[cfg(target_os = "linux")]
                match event_loop.display_handle()?.as_raw() {
                    RawDisplayHandle::Xlib(_) => khr::xlib_surface::NAME,
                    RawDisplayHandle::Xcb(_) => khr::xcb_surface::NAME,
                    RawDisplayHandle::Wayland(_) => khr::wayland_surface::NAME,
                    tmp => { error!("Support for {} is unimplemented",format!("{:?}",tmp).bright_purple()); panic!() } }
                #[cfg(target_os = "none")]
                { error!("WHAT DO YOU MEAN THERE'S {}","NO TARGET OS".bright_purple()); panic!() }
            };
            let available: Vec<vk::ExtensionProperties> = unsafe { entry.enumerate_instance_extension_properties(None).unwrap() };
            let available: Vec<&CStr> = available.iter().map(|ext|ext.extension_name_as_c_str().unwrap()).collect();

            let mut extensions: Vec<*const c_char> = Vec::with_capacity(1+REQUIRED_EXTENSIONS.len()+OPTIONAL_EXTENSIONS.len());
            if available.contains(&prerequisite) { extensions.push(prerequisite.as_ptr()) }
            else { error!("Prerequisite extension {} unavailable!", format!("{:?}",prerequisite).bright_purple()); panic!() }
            for required in REQUIRED_EXTENSIONS { if available.contains(&required) { extensions.push(required.as_ptr()) }
            else { error!("Required extension {} unavailable!", format!("{:?}",required).bright_purple()); panic!() } }
            for optional in OPTIONAL_EXTENSIONS { if available.contains(&optional) { extensions.push(optional.as_ptr()) }
            else { error!("Optional extension {} unavailable; Corresponding features {}",format!("{:?}",optional).bright_purple(),"locked".red()) } }
            extensions
        };
        let layers = {
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
            p_application_info: &app_info,
            pp_enabled_extension_names: extensions.as_ptr(),
            enabled_extension_count: extensions.len() as u32,
            pp_enabled_layer_names: layers.as_ptr(),
            enabled_layer_count: layers.len() as u32,
            ..Default::default()};

        unsafe { entry.create_instance(&create_info, None)? }
    };

    match event_loop.run_app(&mut App {
        entry, instance,
        per_window: HashMap::with_capacity(WINDOW_COUNT),
        windows_function_pointers: None, })
    {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e))
    }

}


struct App {
    entry: Entry,
    instance: Instance,

    per_window: HashMap<WindowId,PerWindow>,

    windows_function_pointers: Option<WindowsFFI>,
}



#[allow(unused)]
impl ApplicationHandler for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {

        let mut builder = WindowBuilder::new(event_loop, &self.entry, &self.instance);
        builder.attributes = builder.attributes
            .with_title(APPLICATION_TITLE)
            .with_active(true)
            .with_transparent(true);

    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        let per_window = self.per_window.get(&window_id);
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
                self.per_window.remove(&window_id);
                if self.per_window.len() == 0 { event_loop.exit() };
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
        info!("cleaning up...");
        unsafe {
            self.instance.destroy_instance(None);
        }
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        println!("MEMORY WARNING");
    }
}
