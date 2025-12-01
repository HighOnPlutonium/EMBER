use ash::{ext, khr};

struct ExtensionHolder {
    surface: khr::surface::Instance,
    os_surface: OSSurface,
    debug_utils: Option<ext::debug_utils::Instance>,
    debug_report: Option<ext::debug_report::Instance>,

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

impl ExtensionHolder {
    
}