use std::error::Error;
use ash::{khr, vk, Device};
use winit::dpi::PhysicalSize;
use winit::window::Window;
use crate::MAX_FRAMES_IN_FLIGHT;
use crate::util::per_window::SYN;


pub struct PerSwapchain {
    pub handle: vk::SwapchainKHR,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub images: Vec<vk::Image>,
    pub views: Vec<vk::ImageView>,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub sync: Vec<SYN>,
}
impl PerSwapchain {
    pub unsafe fn cleanup(
        &self,
        device: &Device,
        ext_swapchain: &khr::swapchain::Device,
    ) {
        for sync in &self.sync {
            sync.destroy(device);
        }
        for framebuffer in &self.framebuffers {
            device.destroy_framebuffer(*framebuffer, None);
        }
        for view in &self.views {
            device.destroy_image_view(*view, None);
        }
        ext_swapchain.destroy_swapchain(self.handle,None);
    }

    pub unsafe fn create_swapchain(
        window: &Window,
        surface: vk::SurfaceKHR,
        device: &Device,
        physical_device: vk::PhysicalDevice,
        ext_surface: &khr::surface::Instance,
        ext_swapchain: &khr::swapchain::Device
    ) -> Result<(vk::SwapchainKHR,vk::Format,vk::Extent2D,Vec<SYN>),Box<dyn Error>> {
        //currently we just propagate possible issues to the caller. which kinda sucks.
        //if we want to do anything fun we'll need a swapchain - and that's a per-surface thingy
        // todo! actually use all this information, and decide on proper swapchain settings based on them
        let capabilities = ext_surface.get_physical_device_surface_capabilities(physical_device, surface)?;
        let formats = ext_surface.get_physical_device_surface_formats(physical_device, surface)?;
        let present_modes = ext_surface.get_physical_device_surface_present_modes(physical_device, surface)?;
        let (formats, color_spaces) = formats.iter().map(|format| (format.format, format.color_space)).collect::<(Vec<vk::Format>, Vec<vk::ColorSpaceKHR>)>();

        //in case neither 32bit BGRA SRGB or 32bit RGBA SRGB are available, a default value.
        let mut format = *formats.first().unwrap();
        //ain't fuckin with any of the other color spaces, nor dealing with their availability for now. honestly go find a device other than a washing mashien or something that deosn't support SRGB color spaces
        let color_space = vk::ColorSpaceKHR::SRGB_NONLINEAR;
        //SRGB is common and good. B8G8R8 format is also shockingly common in displays?
        if formats.contains(&vk::Format::B8G8R8A8_SRGB) { format = vk::Format::B8G8R8A8_SRGB } else if formats.contains(&vk::Format::R8G8B8A8_SRGB) { format = vk::Format::R8G8B8A8_SRGB }

        let extent = {
            let PhysicalSize { width, height } = window.inner_size();
            vk::Extent2D::default().width(width).height(height)
        };

        let swapchain_create_info = vk::SwapchainCreateInfoKHR {
            flags: vk::SwapchainCreateFlagsKHR::default(),
            surface,
            min_image_count: capabilities.min_image_count,
            image_format: format,
            image_color_space: color_space,
            image_extent: extent,
            image_array_layers: 1,
            //for now, we'll only use the swapchain as a framebuffer color attachment
            image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            //exclusive sharing between queue families has the best performance, but forces you to deal with ownership in between families if you use multiple ones.
            // we don't. we use one family without checking for presentation support. yay
            image_sharing_mode: vk::SharingMode::EXCLUSIVE,
            //          queue family infos are only needed if we're using CONCURRENT image sharing.
            //queue_family_index_count: ,
            //p_queue_family_indices: ,
            pre_transform: capabilities.current_transform,
            composite_alpha: vk::CompositeAlphaFlagsKHR::INHERIT, // todo!   INHERIT would be better but for some reason it's causing trouble on my PC (but not my laptop)
            present_mode: vk::PresentModeKHR::FIFO,
            //we don't care about obscured pixels (for now)
            clipped: vk::TRUE,
            //really quite pleasant that the ash bindings implement Default for pretty much all those structs
            ..Default::default()};

        let mut syn: Vec<SYN> = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);
        //missing synchronization object creation error handling...
        (0..MAX_FRAMES_IN_FLIGHT).for_each(|_|unsafe { syn.push(SYN::new(device).unwrap()) });

        Ok((ext_swapchain.create_swapchain(&swapchain_create_info, None)?, format, extent, syn))
    }
}