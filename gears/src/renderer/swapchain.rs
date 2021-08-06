use crate::{ContextError, MapErrorLog, SyncMode};
use ash::{extensions::khr, vk};

pub struct Swapchain {
    pub sync: SyncMode,
    pub loader: khr::Swapchain,
    pub swapchain: vk::SwapchainKHR,
}

impl Swapchain {
    pub fn acquire_image(&self, semaphore: vk::Semaphore, fence: vk::Fence) -> usize {
        match unsafe {
            self.loader
                .acquire_next_image(self.swapchain, !0, semaphore, fence)
        } {
            Ok((image_index, _)) => image_index as usize,
            Err(err) => panic!("Failed to acquire image from swapchain: {:?}", err),
        }
    }

    pub fn images(&self) -> Result<Vec<vk::Image>, ContextError> {
        unsafe { self.loader.get_swapchain_images(self.swapchain) }
            .map_err_log("Swapchain image query failed", ContextError::OutOfMemory)
    }

    pub fn present(&self, queue: vk::Queue, wait: &[vk::Semaphore], index: usize) -> bool {
        let swapchains = [self.swapchain];
        let index = index as u32;
        let indices = [index];
        let submit = vk::PresentInfoKHR::builder()
            .wait_semaphores(wait)
            .swapchains(&swapchains)
            .image_indices(&indices);

        match unsafe {
            // present
            self.loader.queue_present(queue, &submit)
        } {
            Ok(o) => o,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true,
            Err(e) => panic!("Present queue submit failed: {:?}", e),
        }
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_swapchain(self.swapchain, None) }
    }
}
