extern crate core;

mod util;
mod experimental;

use std::collections::HashMap;
use util::per_window::PerWindow;

use crate::experimental::Antistatic;
use crate::util::helpers::{record_into_buffer, recreate_swapchain};
use crate::util::logging::{ConsoleLogger, Logged};
use crate::util::per_window::WindowBuilder;
use crate::util::swapchain::PerSwapchain;
use crate::util::windows_ffi::WindowsFFI;
use ash::vk::{Handle, PFN_vkAllocateMemory};
use ash::Instance;
use ash::{ext, vk};
use ash::{khr, Device, Entry};
use colored::Colorize;
use log::{debug, error, info, trace, warn, LevelFilter};
use once_cell::unsync::Lazy;
use std::borrow::Cow;
use std::cell::{LazyCell, OnceCell, UnsafeCell};
use std::error::Error;
use std::ffi::{c_char, c_void, CStr};
use std::io::Read;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{ControlFlow, Deref};
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex, Once, OnceLock};
use std::time::{Duration, Instant, SystemTime};
use std::{env, fmt, fs, mem, ptr, slice, thread};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, Size};
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::{WindowId, WindowLevel};

use std::os::fd::{AsFd, AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
use drm_fourcc::{DrmFormat, DrmFourcc, DrmModifier};
use pipewire::properties::properties;

const APPLICATION_TITLE: &str = "EMBER";
const WINDOW_COUNT: usize = 1;
const MAX_FRAMES_IN_FLIGHT: u32 = 1;

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
const OPTIONAL_EXTENSIONS: [&CStr; 6] = [
    ext::debug_utils::NAME,
    ext::debug_report::NAME,

    ext::direct_mode_display::NAME,
    ext::acquire_drm_display::NAME,

    khr::external_memory_capabilities::NAME,
    khr::get_physical_device_properties2::NAME,

];
const REQUIRED_DEVICE_EXTENSIONS: [&CStr; 1] = [
    khr::swapchain::NAME,];
const OPTIONAL_DEVICE_EXTENSIONS: [&CStr; 8] = [
    ext::device_address_binding_report::NAME,
    khr::external_memory_fd::NAME,
    khr::external_memory::NAME,
    ext::external_memory_dma_buf::NAME,

    ext::image_drm_format_modifier::NAME,

    khr::bind_memory2::NAME,
    khr::sampler_ycbcr_conversion::NAME,
    khr::image_format_list::NAME,
];

static OPT_EXT_LOCK: Mutex<Vec<&CStr>> = Mutex::new(vec![]);




static KHR_SURFACE: LazyLock<khr::surface::Instance> = LazyLock::new(||khr::surface::Instance::new(&*ENTRY,&*INSTANCE));

static T_ZERO: LazyLock<Instant> = LazyLock::new(Instant::now);

static          ENTRY:   LazyLock<Entry>            =   LazyLock::new(Entry::linked);
static DISPLAY_HANDLE: Antistatic<RawDisplayHandle> = Antistatic::new();
static       INSTANCE: Antistatic<Instance>         = Antistatic::new();



static LOGGER: ConsoleLogger = ConsoleLogger;
fn main() -> Result<(),Box<dyn Error>>
{
    #[cfg(windows)]
    ansi_term::enable_ansi_support().unwrap();
    unsafe { env::set_var("COLORTERM","truecolor"); }

    log::set_logger(&LOGGER)?;
    log::set_max_level(LevelFilter::Trace);

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(event_loop::ControlFlow::Poll);
    DISPLAY_HANDLE.set(event_loop.display_handle()?.as_raw());

    let debug_utils_create_info = vk::DebugUtilsMessengerCreateInfoEXT {
        message_severity: {
            type Flags = vk::DebugUtilsMessageSeverityFlagsEXT;
            Flags::VERBOSE
        },
        message_type: {
            type Flags = vk::DebugUtilsMessageTypeFlagsEXT;
            Flags::GENERAL | Flags::DEVICE_ADDRESS_BINDING | Flags::PERFORMANCE | Flags::VALIDATION
        },
        pfn_user_callback: Some(util::logging::debug_callback),
        p_user_data: ptr::null_mut(),
        ..Default::default()};
    let debug_reporter_create_info = vk::DebugReportCallbackCreateInfoEXT {
        flags: {
            type Flags = vk::DebugReportFlagsEXT;
            Flags::INFORMATION | Flags::ERROR  | Flags::DEBUG | Flags::WARNING | Flags::PERFORMANCE_WARNING
        },
        pfn_callback: Some(util::logging::debug_reporter),
        p_user_data: ptr::null_mut(),
        ..Default::default() };

    let instance = {
        let extensions: Vec<*const c_char> = {
            let mut opt_ext_lock = OPT_EXT_LOCK.lock().unwrap();
            //(platform-dependent!) extension for surface creation.
            let mut prerequisite = match *DISPLAY_HANDLE {
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
            api_version: vk::make_api_version(0,1,4,0),
            ..Default::default()};
        let create_info = vk::InstanceCreateInfo {
            p_next: ptr::from_ref(&debug_utils_create_info).cast(),
            p_application_info: &app_info,
            pp_enabled_extension_names: extensions.as_ptr(),
            enabled_extension_count: extensions.len() as u32,
            pp_enabled_layer_names: layers.as_ptr(),
            enabled_layer_count: layers.len() as u32,
            ..Default::default()};

        unsafe { ENTRY.create_instance(&create_info, None)? }
    };
    INSTANCE.set(instance);

    let mut opt_device_ext_lock: Vec<&CStr> = Vec::with_capacity(0);
    // todo!    MAKE THIS SECTION LESS FUCKING UGLY
    let rated_devices: Vec<(u32,vk::PhysicalDevice,vk::PhysicalDeviceProperties,vk::PhysicalDeviceFeatures,Vec<*const c_char>)> = unsafe {
        INSTANCE.enumerate_physical_devices()?
            .iter().filter_map(|device|{
            let mut rating = 0u32;

            let properties = INSTANCE.get_physical_device_properties(*device);
            let features   = INSTANCE.get_physical_device_features(*device);
            let available_extensions: Vec<vk::ExtensionProperties> = INSTANCE.enumerate_device_extension_properties(*device).unwrap();
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
    let Some((&phys_device,&phys_device_properties,phys_device_features,phys_device_extensions)) = device_opt
        else { error!("No suitable device found!"); panic!()  };

    // todo!("check for valid queue families during physical device selection")
    // todo!("deal with presentation support and possible dedicated queues per task")
    let queue_family_index = unsafe {
        let queue_families = INSTANCE.get_physical_device_queue_family_properties(phys_device);
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

    let device_features = phys_device_features.clone().sampler_anisotropy(true);

    let device_create_info = vk::DeviceCreateInfo {
        p_next: if let Some(mut address_debug_info) = address_debug_info {
            ptr::from_ref(&mut address_debug_info).cast() } else { ptr::null() },
        queue_create_info_count: 1,
        p_queue_create_infos: &device_queue_create_info,
        enabled_extension_count: phys_device_extensions.len() as u32,
        pp_enabled_extension_names: phys_device_extensions.as_ptr(),
        p_enabled_features: &device_features,
        ..Default::default()};
    info!("Creating logical device over physical device {}",phys_device_properties.device_name_as_c_str()?.to_str()?.bright_purple());
    let device = unsafe { INSTANCE.create_device(phys_device, &device_create_info, None).logged("Logical device creation failed") };
    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };



    let extension_holder = {
        let opt_ext_lock = OPT_EXT_LOCK.lock().unwrap();
        ExtensionHolder {
            surface: khr::surface::Instance::new(&ENTRY,&INSTANCE),
            os_surface: match event_loop.display_handle()?.as_raw() {
                RawDisplayHandle::Windows(_) => OSSurface::WINDOWS(khr::win32_surface::Instance::new(&ENTRY,&INSTANCE)),
                RawDisplayHandle::Wayland(_) => OSSurface::WAYLAND(khr::wayland_surface::Instance::new(&ENTRY,&INSTANCE)),
                RawDisplayHandle::Xcb(_)     => OSSurface::XCB(khr::xcb_surface::Instance::new(&ENTRY,&INSTANCE)),
                RawDisplayHandle::Xlib(_)    => OSSurface::XLIB(khr::xlib_surface::Instance::new(&ENTRY,&INSTANCE)),
                _ => { unreachable!() }},
            debug_utils: (!opt_ext_lock.contains(&ext::debug_utils::NAME)).then(||
                ext::debug_utils::Instance::new(&ENTRY,&INSTANCE)),
            debug_report: (!opt_ext_lock.contains(&ext::debug_report::NAME)).then(||
                ext::debug_report::Instance::new(&ENTRY,&INSTANCE)),
            swapchain: khr::swapchain::Device::new(&INSTANCE,&device),
            direct_mode: (!opt_ext_lock.contains(&ext::direct_mode_display::NAME)).then(||
                ext::direct_mode_display::Instance::new(&ENTRY,&INSTANCE)),
            linux_drm: (!opt_ext_lock.contains(&ext::acquire_drm_display::NAME)).then(||
                ext::acquire_drm_display::Instance::new(&ENTRY,&INSTANCE)),
            extmem_fd: (!opt_ext_lock.contains(&khr::external_memory_fd::NAME)).then(||
                khr::external_memory_fd::Device::new(&INSTANCE,&device)),
            extmem_caps: (!opt_ext_lock.contains(&khr::external_memory_capabilities::NAME)).then(||
                khr::external_memory_capabilities::Instance::new(&ENTRY,&INSTANCE)),
            image_drm_format_modifier: (!opt_ext_lock.contains(&ext::image_drm_format_modifier::NAME)).then(||
                ext::image_drm_format_modifier::Device::new(&INSTANCE,&device)),

            bind_memory2: (!opt_ext_lock.contains(&khr::bind_memory2::NAME)).then(||
                khr::bind_memory2::Device::new(&INSTANCE,&device)),
            get_physical_device_properties2: (!opt_ext_lock.contains(&khr::get_physical_device_properties2::NAME)).then(||
                khr::get_physical_device_properties2::Instance::new(&ENTRY, &INSTANCE)),
            sampler_ycbcr_conversion: (!opt_ext_lock.contains(&khr::sampler_ycbcr_conversion::NAME)).then(||
                khr::sampler_ycbcr_conversion::Device::new(&INSTANCE,&device)),
        }};

    let command_pool_info = vk::CommandPoolCreateInfo {
        //declare that we want to reset singular/specific command buffers in the pool, instead of everything at once
        flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
        queue_family_index,
        ..Default::default()};
    let command_pool = unsafe { device.create_command_pool(&command_pool_info,None).unwrap() };


    let mut debug_messenger: Option<vk::DebugUtilsMessengerEXT> = None;
    if let Some(debug_utils) = extension_holder.debug_utils.as_ref() {
        debug_messenger = match unsafe { debug_utils.create_debug_utils_messenger(&debug_utils_create_info, None) } {
            Ok(debug_messenger) => Some(debug_messenger),
            Err(e) => { error!("Debug Messenger creation failed: {:?}; Execution will continue without it.",e); None }
        }}
    let mut debug_reporter: Option<vk::DebugReportCallbackEXT> = None;
    if let Some(debug_report) = extension_holder.debug_report.as_ref() {
        debug_reporter = match unsafe { debug_report.create_debug_report_callback(&debug_reporter_create_info, None) } {
            Ok(debug_reporter) => Some(debug_reporter),
            Err(e) => { error!("Debug Reporter creation failed: {:?}; Execution will continue without it.",e); None }
        }}



    let mut holder = SCHolder::default();
    unsafe {
        let mut fd = Arc::new(OnceLock::<i32>::new());
        let mut fd_clone = fd.clone();
        let fn_pw = || {
                use pipewire;
                use portal_screencast_waycap;
                use pipewire::spa;

                let (pw_sender, pw_recv) = pipewire::channel::channel::<()>();

                let screencast = portal_screencast_waycap::ScreenCast::new()?
                    .start(None)?;

                let pipewire_fd = screencast.pipewire_fd() as i32;
                let screencast_stream = screencast.streams().next().unwrap();
                let stream_node = screencast_stream.pipewire_node();

                let pw_loop = Arc::new(pipewire::main_loop::MainLoopBox::new(None)?);
                let pw_loop_clone = pw_loop.clone();
                let pw_context = pipewire::context::ContextBox::new(pw_loop.loop_(), None)?;
                let pw_core = pw_context.connect_fd(OwnedFd::from_raw_fd(pipewire_fd), None)?;
                let core_listener = pw_core
                    .add_listener_local()
                    .info(|i| log::debug!("VIDEO CORE:\n{i:#?}"))
                    .error(|e, f, g, h| log::error!("{e},{f},{g},{h}"))
                    .done(|d, _| log::debug!("DONE: {d}"))
                    .register();
                let mut stream = pipewire::stream::StreamBox::new(
                    &*pw_core,
                    "EMBER_CAPTURE",
                    properties! {
                    *pipewire::keys::MEDIA_TYPE => "Video",
                    *pipewire::keys::MEDIA_CATEGORY => "Capture",
                    *pipewire::keys::MEDIA_ROLE => "Screen",
                })?;
                let stream_listener = stream
                    .add_local_listener::<()>()
                    .state_changed(move |_, _, old, new| {
                        info!("Video Stream State Changed: {old:?} -> {new:?}");
                    })
                    .param_changed(move |_, _, id, param| {
                        let Some(param) = param else { return; };

                        if id != spa::param::ParamType::Format.as_raw() { return; }

                        let (media_type, media_subtype) =
                            match spa::param::format_utils::parse_format(param) {
                                Ok(v) => v,
                                Err(_) => return,
                            };

                        if media_type != spa::param::format::MediaType::Video
                            || media_subtype != spa::param::format::MediaSubtype::Raw
                        { return; }
                    })
                    .process(move |stream, _| {
                        match stream.dequeue_buffer() {
                            None => debug!("out of buffers"),
                            Some(mut buffer) => {
                                let datas = buffer.datas_mut();
                                if datas.is_empty() { return; }
                                let data = &mut datas[0];
                                let raw_data = data.as_raw();

                                if data.type_() == spa::buffer::DataType::DmaBuf {
                                    let _fd = raw_data.fd;
                                    if _fd > 0 { fd_clone.set(_fd as i32).unwrap_or_default() }
                                    //pw_sender.clone().send(()).unwrap();
                                }
                            }
                        }
                    })
                    .register()?;


                let pw_obj = spa::pod::object!(
                spa::utils::SpaTypes::ObjectParamFormat,
                spa::param::ParamType::EnumFormat,
                spa::pod::property!(
                    spa::param::format::FormatProperties::MediaType,
                    Id,
                    spa::param::format::MediaType::Video
                ),
                spa::pod::property!(
                    spa::param::format::FormatProperties::MediaSubtype,
                    Id,
                    spa::param::format::MediaSubtype::Raw
                ),
                spa::pod::property!(
                    spa::param::format::FormatProperties::VideoModifier,
                    Long,
                    0
                ),
                spa::pod::property!(
                    spa::param::format::FormatProperties::VideoFormat,
                    Choice,
                    Enum,
                    Id,
                    spa::param::video::VideoFormat::NV12,
                    spa::param::video::VideoFormat::I420,
                    spa::param::video::VideoFormat::BGRA,
                ),
                spa::pod::property!(
                    spa::param::format::FormatProperties::VideoSize,
                    Choice,
                    Range,
                    Rectangle,
                    spa::utils::Rectangle {
                        width: 2560,
                        height: 1440
                    }, // Default
                    spa::utils::Rectangle {
                        width: 1,
                        height: 1
                    }, // Min
                    spa::utils::Rectangle {
                        width: 4096,
                        height: 4096
                    } // Max
                ),
                spa::pod::property!(
                    spa::param::format::FormatProperties::VideoFramerate,
                    Choice,
                    Range,
                    Fraction,
                    spa::utils::Fraction { num: 240, denom: 1 }, // Default
                    spa::utils::Fraction { num: 0, denom: 1 },   // Min
                    spa::utils::Fraction { num: 244, denom: 1 }  // Max
                ),
            );

                let video_spa_values: Vec<u8> = spa::pod::serialize::PodSerializer::serialize(
                    std::io::Cursor::new(Vec::new()),
                    &spa::pod::Value::Object(pw_obj),
                )?.0.into_inner();

                let mut video_params = [spa::pod::Pod::from_bytes(&video_spa_values).unwrap()];
                stream.connect(
                    spa::utils::Direction::Input,
                    Some(stream_node),
                    pipewire::stream::StreamFlags::AUTOCONNECT
                        | pipewire::stream::StreamFlags::RT_PROCESS,
                    &mut video_params)?;

                let _recv = pw_recv.attach(pw_loop.loop_(), move |_| {
                    debug!("Terminating video capture loop");
                    pw_loop_clone.quit();
                });

                pw_loop.run();
            Ok(())
        };
        let handle: thread::JoinHandle<Result<(),Box<dyn Error + Send + Sync>>> = thread::Builder::new()
            .name("pipewire".to_owned())
            .spawn(fn_pw).unwrap();
        while fd.get().is_none() {
            thread::sleep(Duration::from_millis(100));
        }

        let mem_import_info = vk::ImportMemoryFdInfoKHR {
            handle_type: vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
            fd: *fd.get().unwrap(),
            ..Default::default()};
        let mem_alloc_info = vk::MemoryAllocateInfo {
            p_next: ptr::from_ref(&mem_import_info).cast(),
            allocation_size: 1920*1200*4,   // todo! hardcoded shit
            memory_type_index: 0,
            ..Default::default()};
        let mem = device.allocate_memory(&mem_alloc_info, None)?;

        let format_modifiers = vec![DrmFourcc::Abgr8888 as u64, DrmModifier::Linear.into()];
        let drm_format_modifier_list = vk::ImageDrmFormatModifierListCreateInfoEXT {
            drm_format_modifier_count: format_modifiers.len() as u32,
            p_drm_format_modifiers: format_modifiers.as_ptr(),
            ..Default::default() };
        let ext_img_info = vk::ExternalMemoryImageCreateInfo {
            p_next: ptr::from_ref(&drm_format_modifier_list).cast(),
            handle_types: vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
            ..Default::default()};
        let img_info = vk::ImageCreateInfo {
            p_next: ptr::from_ref(&ext_img_info).cast(),
            flags: {
                type Flags = vk::ImageCreateFlags;
                Flags::default()
            },
            image_type: vk::ImageType::TYPE_2D,
            format: vk::Format::B8G8R8A8_SRGB,
            extent: vk::Extent3D { width: 1920, height: 1200, depth: 1 },
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::TYPE_1,
            tiling: vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT,
            usage: {
                type Flags = vk::ImageUsageFlags;
                Flags::SAMPLED
            },
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            ..Default::default()};
        let img = device.create_image(&img_info, None)?;

        device.bind_image_memory(img, mem, 0)?;

        let view_info = vk::ImageViewCreateInfo {
            flags: vk::ImageViewCreateFlags::default(),
            image: img,
            view_type: vk::ImageViewType::TYPE_2D,
            format: img_info.format,
            components: vk::ComponentMapping::default(),
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
            ..Default::default()};
        let view = device.create_image_view(&view_info, None)?;

        let sampler_info = vk::SamplerCreateInfo {
            flags: vk::SamplerCreateFlags::default(),
            mag_filter: vk::Filter::LINEAR,
            min_filter: vk::Filter::LINEAR,
            mipmap_mode: vk::SamplerMipmapMode::NEAREST,
            address_mode_u: vk::SamplerAddressMode::CLAMP_TO_BORDER,
            address_mode_v: vk::SamplerAddressMode::CLAMP_TO_BORDER,
            address_mode_w: vk::SamplerAddressMode::CLAMP_TO_BORDER,
            mip_lod_bias: 0.0,
            anisotropy_enable: vk::TRUE,
            max_anisotropy: 4f32.min(INSTANCE.get_physical_device_properties(phys_device).limits.max_sampler_anisotropy),
            compare_enable: vk::FALSE,
            compare_op: vk::CompareOp::ALWAYS,
            min_lod: 0.0,
            max_lod: 0.0,
            border_color: vk::BorderColor::FLOAT_OPAQUE_BLACK,
            unnormalized_coordinates: vk::FALSE,
            ..Default::default()};
        let sampler = device.create_sampler(&sampler_info, None)?;

        holder.mem = mem;
        holder.img = img;
        holder.view = view;
        holder.sampler = sampler;
    };

    info!("Using Device {}",format!("{:?}",phys_device_properties.device_name_as_c_str().unwrap()).bright_purple());
    match event_loop.run_app(&mut App {
        device,
        physical_device: phys_device,
        queue,
        command_pool,

        windows: HashMap::with_capacity(WINDOW_COUNT),

        ext: extension_holder,
        win32_fp: None,

        debug_messenger,
        debug_reporter,

        resized: false,
        current_frame: 0,

        screencast: Some(holder),
        ctrl_vals: [[0.0,0.0,2.0],[0.0,0.0,0.0,],[0.0,0.0,0.0],[0.0,0.0,0.0]],
        mode: 0,
    })
    {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e))
    }
}
#[derive(Default)]
struct SCHolder {
    mem: vk::DeviceMemory,
    img: vk::Image,
    view: vk::ImageView,
    sampler: vk::Sampler,
}
pub(crate) struct App {
    #[allow(unused)]
    device: Device,
    //i *think* there's no way to retrieve the physical device handle from a logical device
    physical_device: vk::PhysicalDevice,
    queue: vk::Queue,
    command_pool: vk::CommandPool,

    windows: HashMap<WindowId,PerWindow>,

    ext: ExtensionHolder,
    win32_fp: Option<WindowsFFI>,

    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    debug_reporter: Option<vk::DebugReportCallbackEXT>,

    resized: bool,
    current_frame: usize,

    screencast: Option<SCHolder>,
    ctrl_vals: [[f32;3];4],
    mode: usize,
}

struct ExtensionHolder {
    surface: khr::surface::Instance,
    os_surface: OSSurface,
    debug_utils: Option<ext::debug_utils::Instance>,
    debug_report: Option<ext::debug_report::Instance>,
    // i guess i'll put device level functions into the same struct as instance level functions?
    swapchain: khr::swapchain::Device,

    direct_mode: Option<ext::direct_mode_display::Instance>,
    linux_drm: Option<ext::acquire_drm_display::Instance>,

    extmem_fd: Option<khr::external_memory_fd::Device>,
    extmem_caps: Option<khr::external_memory_capabilities::Instance>,
    image_drm_format_modifier: Option<ext::image_drm_format_modifier::Device>,
    bind_memory2: Option<khr::bind_memory2::Device>,
    get_physical_device_properties2: Option<khr::get_physical_device_properties2::Instance>,
    sampler_ycbcr_conversion: Option<khr::sampler_ycbcr_conversion::Device>,
}

enum OSSurface {
    WINDOWS(khr::win32_surface::Instance),
    WAYLAND(khr::wayland_surface::Instance),
    XCB(khr::xcb_surface::Instance),
    XLIB(khr::xlib_surface::Instance),
}


unsafe fn cleanup(
    ext: &ExtensionHolder,
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    debug_reporter: Option<vk::DebugReportCallbackEXT>,
    device: &Device
) {
    if let (Some(debug_utils),Some(debug_messenger)) = (ext.debug_utils.as_ref(),debug_messenger) {
        debug_utils.destroy_debug_utils_messenger(debug_messenger,None);
    }
    if let (Some(debug_report),Some(debug_reporter)) = (ext.debug_report.as_ref(),debug_reporter) {
        debug_report.destroy_debug_report_callback(debug_reporter,None);
    }
    device.destroy_device(None);
    INSTANCE.destroy_instance(None);
}

const FOV: f32 = 75.0;
const NEAR: f32 = 0.01;
const FAR: f32 = 20.0;

#[repr(C)]
struct UniformBufferObject {
    model: glm::Mat4,
    view: glm::Mat4,
    proj: glm::Mat4,
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
            .with_inner_size(Size::Logical(LogicalSize::new(400f64,400f64)))
            .with_decorations(true);

        let mut window_count = WINDOW_COUNT;
        if WINDOW_COUNT > 5 {
            window_count -= 1;
        }

        (0..window_count).for_each(|idx| {
            builder.attributes.title = format!("{}  #{}",APPLICATION_TITLE,idx+1);
            let (window_id, mut per_window) = builder.build(event_loop, self.screencast.as_ref());
            per_window.id = idx as i32;
            /*
            let fp = unsafe { WindowsFFI::load_function_pointers() };
            per_window.toggle_blur(&fp);
            */
            _ = self.windows.insert(window_id,per_window);
        });

        // IF we create more than 5 windows. just for funsies + so that whoever's trying this out knows why there's so many windows being created
        if WINDOW_COUNT != window_count {
            debug!("THE LARGE AMOUNT OF WINDOWS IS INTENTIONAL.");
            info!("by the way, that above was on \"{}\" due to the color being highly visible, not because of it being debugging-related.","DEBUG".bright_cyan());
            builder.attributes.title = "yes, this is intentional".to_owned();
            let (window_id, mut per_window) = builder.build(event_loop, self.screencast.as_ref());
            per_window.id = window_count as i32;
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
            command_buffers,
            ..
        } = per_window;

        match event {
            WindowEvent::KeyboardInput { event, .. } =>
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    match keycode {
                        KeyCode::Escape => { self.window_event(event_loop, window_id, WindowEvent::CloseRequested) }
                        KeyCode::Enter => {
                            if (!event.state.is_pressed() || event.repeat) { return; }
                            window.set_decorations(!window.is_decorated()) }


                        KeyCode::AltLeft => {
                            if !event.state.is_pressed() { return; }
                            self.mode = (self.mode + 1) % 4;
                            debug!("mode: {}",self.mode);
                        }
                        KeyCode::ArrowLeft => {
                            if !event.state.is_pressed() { return; }
                            let [x,y,z] = self.ctrl_vals[self.mode];
                            self.ctrl_vals[self.mode] = [x-0.1,y,z];
                            debug!("{} -> {:?}",self.mode,self.ctrl_vals[self.mode]);
                        }
                        KeyCode::ArrowRight => {
                            if !event.state.is_pressed() { return; }
                            let [x,y,z] = self.ctrl_vals[self.mode];
                            self.ctrl_vals[self.mode] = [x+0.1,y,z];
                            debug!("{} -> {:?}",self.mode,self.ctrl_vals[self.mode]);
                        }
                        KeyCode::ArrowUp => {
                            if !event.state.is_pressed() { return; }
                            let [x,y,z] = self.ctrl_vals[self.mode];
                            self.ctrl_vals[self.mode] = [x,y-0.1,z];
                            debug!("{} -> {:?}",self.mode,self.ctrl_vals[self.mode]);
                        }
                        KeyCode::ArrowDown => {
                            if !event.state.is_pressed() { return; }
                            let [x,y,z] = self.ctrl_vals[self.mode];
                            self.ctrl_vals[self.mode] = [x,y+0.1,z];
                            debug!("{} -> {:?}",self.mode,self.ctrl_vals[self.mode]);
                        }
                        KeyCode::Space => {
                            if !event.state.is_pressed() { return; }
                            let [x,y,z] = self.ctrl_vals[self.mode];
                            self.ctrl_vals[self.mode] = [x,y,z-0.1];
                            debug!("{} -> {:?}",self.mode,self.ctrl_vals[self.mode]);
                        }
                        KeyCode::ShiftLeft => {
                            if !event.state.is_pressed() { return; }
                            let [x,y,z] = self.ctrl_vals[self.mode];
                            self.ctrl_vals[self.mode] = [x,y,z+0.1];
                            debug!("{} -> {:?}",self.mode,self.ctrl_vals[self.mode]);
                        }
                        _ => {}
                    }}

            WindowEvent::CloseRequested => {
                //fetching the number inside WindowId structs using unsafe code. Why? makes the console output look better.
                //this should also cause UB if you use any system with 32bit window handles/IDs.
                //  issue is, if other systems get implemented that don't use a 64bit value as their window handle,
                //  this would guarantee UB whenever a window gets closed
                warn!(
                    "Closing Window with {}",
                    format!("ID {}",unsafe {mem::transmute_copy::<_,isize>(&window_id) }).bright_purple());

                let PerWindow {surface,layout,pipeline,render_pass,swapchain,vertex_buffer,vertex_buffer_mem,
                    ubufs,ubufs_mem,descriptor_set_layout,descriptor_pool, .. }
                    = self.windows.remove(&window_id).unwrap();
                unsafe {
                    //VERY IMPORTANT! otherwise, we'd try cleaning up semaphores n stuff while they're still in use
                    self.device.device_wait_idle().unwrap();

                    self.device.destroy_pipeline(pipeline,None);
                    self.device.destroy_pipeline_layout(layout,None);

                    swapchain.cleanup(&self.device, &self.ext.swapchain);

                    ubufs_mem.iter().for_each(|mem|{
                        self.device.unmap_memory(*mem);
                    });
                    ubufs.iter().for_each(|buf|{
                        self.device.destroy_buffer(*buf, None);
                    });

                    self.device.destroy_descriptor_pool(descriptor_pool, None);
                    self.device.destroy_descriptor_set_layout(descriptor_set_layout,None);

                    self.device.destroy_buffer(vertex_buffer, None);
                    self.device.free_memory(vertex_buffer_mem, None);

                    self.device.destroy_render_pass(render_pass,None);
                    self.ext.surface.destroy_surface(surface,None);
                }
                if self.windows.len() == 0 { event_loop.exit() };
            }
            WindowEvent::Resized(size) => {
                //should probably do some of the swapchain recreation here
                //although that's left for a later date: todo!
                self.resized = true;
                per_window.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let device = &self.device;
                let ext = &self.ext;

                unsafe { device.wait_for_fences(&[swapchain.sync[self.current_frame].in_flight],true,u64::MAX).unwrap() };

                let next = unsafe {
                    let mut result = ext.swapchain.acquire_next_image(swapchain.handle, u64::MAX, swapchain.sync[self.current_frame].swapchain.clone(), vk::Fence::null());
                    if self.resized { result = Err(vk::Result::ERROR_OUT_OF_DATE_KHR); }
                    match result {
                        Ok((next,false)) => {
                            next }
                        Ok(_) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                            recreate_swapchain(
                                &self.device,
                                self.physical_device,
                                per_window,
                                &self.ext.surface,
                                &self.ext.swapchain
                            );
                            let (next,is_suboptimal) = ext.swapchain.acquire_next_image(per_window.swapchain.handle, u64::MAX, per_window.swapchain.sync[self.current_frame].swapchain, vk::Fence::null()).unwrap();
                            self.resized = is_suboptimal;
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
                    command_buffers,
                    vertex_buffer,
                    push_constant_range,
                    ubufs,
                    ubufs_map,
                    descriptor_sets,
                    id,
                    ..
                } = per_window;

                unsafe { device.reset_fences(&[swapchain.sync[self.current_frame].in_flight]).unwrap() };

                let map = unsafe { ubufs_map[self.current_frame].cast::<UniformBufferObject>().as_mut().unwrap() };


                let [[cx,cy,cz],[cu,cv,cw],[ox,oy,oz],[ou,ov,ow]] = self.ctrl_vals;
                let aspect = (swapchain.extent.width as f32) / (swapchain.extent.height as f32);

                map.model = glm::mat4(
                    1.0,    0.0,    0.0,    0.0,
                    0.0,    1.0,    0.0,    0.0,
                    0.0,    0.0,    1.0,    0.0,
                    ox,    oy,    oz,    1.0);

                map.model = glm::ext::rotate(&map.model, ou, glm::vec3(1.0,0.0,0.0));
                map.model = glm::ext::rotate(&map.model, ov, glm::vec3(0.0,1.0,0.0));
                map.model = glm::ext::rotate(&map.model, ow, glm::vec3(0.0,0.0,1.0));


                map.view = glm::mat4(
                    1.0,    0.0,    0.0,    0.0,
                    0.0,    1.0,    0.0,    0.0,
                    0.0,    0.0,    1.0,    0.0,
                    -cx,    -cy,    -cz,    1.0);

                map.view = glm::ext::rotate(&map.view, cu, glm::vec3(1.0,0.0,0.0));
                map.view = glm::ext::rotate(&map.view, cv, glm::vec3(0.0,1.0,0.0));
                map.view = glm::ext::rotate(&map.view, cw, glm::vec3(0.0,0.0,1.0));


                map.proj = glm::ext::perspective(FOV.to_radians(), aspect, NEAR, FAR);


                unsafe { device.reset_command_buffer(command_buffers[self.current_frame],Default::default()).unwrap() };
                unsafe { record_into_buffer(device, window, *pipeline, *render_pass, swapchain.framebuffers[next as usize],
                                            swapchain.extent, command_buffers[self.current_frame], self.current_frame, *vertex_buffer, *layout, *push_constant_range,
                                            self.screencast.as_ref().unwrap().img, descriptor_sets.clone(), *id) };

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
                        println!("recreated swapchain");
                        per_window.window.request_redraw();
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
        thread::sleep(Duration::from_millis(33));
        self.windows.iter().for_each(|(window_id,per_window)|per_window.window.request_redraw());
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        info!("Cleaning up...");
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device.destroy_command_pool(self.command_pool,None);
            if let Some(holder) = self.screencast.as_mut() {
                self.device.destroy_sampler(holder.sampler, None);
                self.device.destroy_image_view(holder.view, None);
                self.device.destroy_image(holder.img, None);
                self.device.free_memory(holder.mem, None);
            }
            cleanup(&self.ext,self.debug_messenger,self.debug_reporter,&self.device);
        }

    }
}
