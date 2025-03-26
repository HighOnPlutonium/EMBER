use std::error::Error;
use ash::Entry;
use ash::Instance;
use ash::vk;
use windows::Win32::Graphics::Dwm::DWM_BLURBEHIND;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawWindowHandle};
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
        let enabled_extensions = ash_window::enumerate_required_extensions(event_loop.display_handle()?.as_raw())?.to_vec();
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
            window.display_handle()
                .unwrap()
                .as_raw(),
            window.window_handle()
                .unwrap()
                .as_raw(),
            None).unwrap()
        };

        if let RawWindowHandle::Win32(mut handle) = window.window_handle().unwrap().as_raw() {
            unsafe {
                use windows::Win32::{Foundation::HWND};
                let handle = std::mem::transmute::<_,HWND>(handle.hwnd);
                #[repr(C)]
                struct WindowCompositionAttribData {
                    attrib: u32,
                    pv_data: *mut core::ffi::c_void,
                    cb_data: usize}
                type SetWindowsCompositionAttribute = extern "system" fn(HWND, *mut WindowCompositionAttribData) -> windows::core::BOOL;
                let lib = libloading::Library::new("C:/Windows/System32/user32.dll").unwrap();
                let func: libloading::Symbol<SetWindowsCompositionAttribute> = lib.get(b"SetWindowCompositionAttribute").unwrap();
                #[repr(C)]
                struct Data {
                    state: u32,
                    flags: u32,
                    gradient: u32,
                    animation: u32}
                let mut attribute = WindowCompositionAttribData {
                    attrib: 19,
                    pv_data: &Data { state: 3, flags: 480, gradient: 0, animation: 0 } as *const _ as _,
                    cb_data: 16};
                let success = func(handle, std::ptr::from_mut(&mut attribute));
                println!("{}", success.as_bool());
            }
        }


        self.per_window = Some(PerWindow { window, surface });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } =>
                if let PhysicalKey::Code(keycode) = event.physical_key { match keycode {
                    KeyCode::Escape => { self.window_event(event_loop, window_id, WindowEvent::CloseRequested) }
                    _ => {}
                }}
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                self.per_window.as_ref().expect("window missing").window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Some(PerWindow {window, surface}) = self.per_window.as_ref() {
                    //draw calls
                    window.pre_present_notify();
                    //swapchain submit
                }
            }

            _ => {}
        }
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