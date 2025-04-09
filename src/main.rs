mod util;

use std::collections::HashMap;
use util::per_window::PerWindow;

use crate::util::per_window::WindowBuilder;
use crate::util::windows_ffi::WindowsFFI;
use ash::khr::swapchain;
use ash::vk;
use ash::vk::{LayerProperties, SurfaceKHR};
use ash::Instance;
use ash::{khr, Device, Entry};
use std::error::Error;
use std::ffi::{c_char, CStr, CString};
use std::ops::Deref;
use log::{debug, warn};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowId};

const APPLICATION_TITLE: &str = "EMBER";
const WINDOW_COUNT: usize = 2;

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
}


impl App {
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
        let required_extensions = ash_window::enumerate_required_extensions(event_loop.display_handle()?.as_raw())?.to_vec();

        //let available_extensions = unsafe { entry.enumerate_instance_extension_properties(None).unwrap() };
        //dbg!(&available_extensions);
        //let required_extension_names = &required_extensions.iter().map(|&x|unsafe{CStr::from_bytes_until_nul(x.cast::<[u8;256]>().as_ref().unwrap().as_slice()).unwrap()}).collect::<Vec<&CStr>>();
        //dbg!(required_extension_names);


        let enabled_extensions = required_extensions;


        //list wanted layers
        let requested_layers = [
            c"LAYER_NAME",
        ];
        let available_layers = unsafe { entry
            .enumerate_instance_layer_properties()
            .unwrap().into_iter()
            .map(|layer|layer.layer_name_as_c_str().unwrap().to_owned())
            .collect::<Vec<CString>>() };

        let requested_layers = requested_layers.iter().filter_map(|&layer|{
            if let Some(layer) = available_layers.iter().find(|&available|available == &layer.to_owned()) {
                Some(layer.as_ptr())
            } else {
                warn!("VALIDATION LAYER {:?} NOT FOUND", layer);
                None
            }
        }).collect::<Vec<*const c_char>>();

        let create_info = vk::InstanceCreateInfo {
            p_application_info: &app_info,
            pp_enabled_extension_names: enabled_extensions.as_ptr(),
            enabled_extension_count: enabled_extensions.len() as _,
            pp_enabled_layer_names: requested_layers.as_ptr(),
            enabled_layer_count: requested_layers.len() as u32,
            ..Default::default()
        };
        //INSTANCE CREATION
        let instance = unsafe { entry.create_instance(&create_info, None)? };





        Ok(Self {entry, instance, per_window: HashMap::with_capacity(WINDOW_COUNT), windows_function_pointers: None})
    }
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

        (0..WINDOW_COUNT).for_each(|_|{ (|(x,y)|self.per_window.insert(x,y))(builder.build()); });
        unsafe {
            self.windows_function_pointers = Some(WindowsFFI::load_function_pointers());
            self.per_window.iter().enumerate()
                .for_each(|(idx,(_, &ref per_window))| {
                    per_window.toggle_blur(&self.windows_function_pointers.as_ref().unwrap());
                    per_window.window.set_title(format!("{} - #{}", per_window.window.title(), idx + 1).as_ref());
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
                //draw calls
                window.pre_present_notify();
                //swapchain submit

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