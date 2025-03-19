use std::error::Error;
use ash::Entry;
use ash::Instance;
use ash::vk;
use ash::vk::InstanceCreateFlags;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::window::{WindowAttributes, WindowId};

const APPLICATION_TITLE: &str = "EMBER";
fn main() {
    let event_loop = event_loop::EventLoop::new().unwrap();
    let mut app = App::new(&event_loop).unwrap();
    event_loop.run_app(&mut app).unwrap();
}

struct App {
    entry: Entry,
    instance: Instance,

    per_window: Option<PerWindow>
}
struct PerWindow {
    window: winit::window::Window,
    surface: vk::SurfaceKHR,
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
        let enabled_extensions = ash_window::enumerate_required_extensions(event_loop.raw_display_handle()?)?.to_vec();
        let create_info = vk::InstanceCreateInfo {
            p_application_info: &app_info,
            pp_enabled_extension_names: enabled_extensions.as_ptr(),
            enabled_extension_count: enabled_extensions.len() as _,
            ..Default::default()
        };
        //INSTANCE CREATION
        let instance = unsafe { entry.create_instance(&create_info, None)? };



        Ok(Self {entry, instance, per_window: None})
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

        let window = event_loop.create_window(
            WindowAttributes::default()
                .with_title(APPLICATION_TITLE)
                .with_active(true)
        ).unwrap();

        let surface = unsafe {
            ash_window::create_surface(
            &self.entry, &self.instance,
            window.raw_display_handle().unwrap(),
            window.raw_window_handle().unwrap(),
            None).unwrap()
        };

        self.per_window = Some(PerWindow { window, surface });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
    }

    fn device_event(&mut self, event_loop: &ActiveEventLoop, device_id: DeviceId, event: DeviceEvent) {
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
    }
}