use ash::{khr, vk, Device};
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
    pub(crate) unsafe fn cleanup(
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
}