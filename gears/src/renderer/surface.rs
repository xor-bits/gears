use ash::{extensions::khr, vk};

use crate::{device::Dev, ContextError, MapErrorElseLogResult, MapErrorLog, Swapchain, SyncMode};

#[must_use]
pub struct SurfaceBuilder {
    pub extent: vk::Extent2D,
    pub loader: khr::Surface,
    pub surface: vk::SurfaceKHR,
}

pub struct Surface {
    extent: vk::Extent2D,

    device: Dev,
    swapchain_loader: khr::Swapchain,
    surface_loader: khr::Surface,
    surface: vk::SurfaceKHR,
}

pub struct SwapchainInfo {
    format: vk::SurfaceFormatKHR,
    present: vk::PresentModeKHR,
    len: u32,
    extent: vk::Extent2D,
    transform: vk::SurfaceTransformFlagsKHR,
    composite_alpha: vk::CompositeAlphaFlagsKHR,
}

impl SurfaceBuilder {
    pub fn build(self, device: Dev) -> Surface {
        let swapchain_loader = khr::Swapchain::new(&device.instance, &**device);
        let surface_loader = self.loader;
        let surface = self.surface;
        let extent = self.extent;

        Surface {
            extent,

            device,
            swapchain_loader,
            surface_loader,
            surface,
        }
    }
}

impl Surface {
    pub fn re_create(&mut self, surface: vk::SurfaceKHR) {
        self.surface = surface;
    }

    pub fn build_swapchain(
        &mut self,
        sync: SyncMode,
    ) -> Result<(Swapchain, vk::Format, vk::Extent2D), ContextError> {
        let info = self.swapchain_info(sync)?;

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(self.surface)
            .min_image_count(info.len)
            .image_color_space(info.format.color_space)
            .image_format(info.format.format)
            .image_extent(info.extent)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(info.transform)
            .composite_alpha(info.composite_alpha)
            .present_mode(info.present)
            .clipped(true)
            .image_array_layers(1);

        let swapchain = unsafe {
            self.swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
        }
        .map_err_else_log("Swapchain creation failed", |err| match err {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                ContextError::OutOfMemory
            }
            vk::Result::ERROR_DEVICE_LOST => ContextError::DriverCrash,
            _ => ContextError::FrameInUse,
        })?;

        log::debug!(
            "Swapchain images: {} - Swapchain format: {:?}",
            info.len,
            info.format
        );

        let format = info.format.format;
        let extent = info.extent;

        Ok((
            Swapchain {
                sync,
                loader: self.swapchain_loader.clone(),
                swapchain,
            },
            format,
            extent,
        ))
    }

    fn swapchain_info(&mut self, sync: SyncMode) -> Result<SwapchainInfo, ContextError> {
        let caps = self.capabilities()?;
        Ok(SwapchainInfo {
            format: self.pick_format()?,
            present: self.pick_present_mode(sync)?,

            len: self.swapchain_len(&caps),
            extent: self.swapchain_extent(&caps),
            transform: self.swapchain_transform(&caps),
            composite_alpha: self.swapchain_composite_alpha(&caps),
        })
    }

    fn capabilities(&self) -> Result<vk::SurfaceCapabilitiesKHR, ContextError> {
        unsafe {
            self.surface_loader
                .get_physical_device_surface_capabilities(self.device.pdevice, self.surface)
        }
        .map_err_else_log("Surface capability query failed", |err| match err {
            vk::Result::ERROR_SURFACE_LOST_KHR => ContextError::FrameLost,
            _ => ContextError::OutOfMemory,
        })
    }

    fn pick_format(&self) -> Result<vk::SurfaceFormatKHR, ContextError> {
        let available = unsafe {
            self.surface_loader
                .get_physical_device_surface_formats(self.device.pdevice, self.surface)
        }
        .map_err_log("Surface format query failed", ContextError::OutOfMemory)?;

        if available.len() == 0 {
            log::error!("No surface formats available");
            return Err(ContextError::MissingSurfaceConfigs);
        }

        let format = available
            .iter()
            .find(|format| {
                format.format == vk::Format::R8G8B8A8_SRGB
                    && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .unwrap_or(&available[0]);
        let format = format.clone();

        log::debug!("Surface format chosen: {:?} from {:?}", format, available);

        Ok(format)
    }

    fn pick_present_mode(&self, sync: SyncMode) -> Result<vk::PresentModeKHR, ContextError> {
        let available = unsafe {
            self.surface_loader
                .get_physical_device_surface_present_modes(self.device.pdevice, self.surface)
        }
        .map_err_log(
            "Surface present mode query failed",
            ContextError::OutOfMemory,
        )?;

        if available.len() == 0 {
            log::error!("No surface present modes available");
            return Err(ContextError::MissingSurfaceConfigs);
        }

        let mode = match sync {
            SyncMode::Fifo => vk::PresentModeKHR::FIFO,
            SyncMode::Immediate => available
                .iter()
                .find(|&&present| present == vk::PresentModeKHR::IMMEDIATE)
                .unwrap_or(&vk::PresentModeKHR::FIFO)
                .clone(),
            SyncMode::Mailbox => available
                .iter()
                .find(|&&present| present == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(&vk::PresentModeKHR::FIFO)
                .clone(),
        };

        log::debug!(
            "Surface present mode chosen: {:?} from {:?}",
            mode,
            available
        );

        Ok(mode)
    }

    fn swapchain_len(&self, surface_caps: &vk::SurfaceCapabilitiesKHR) -> u32 {
        let preferred = surface_caps.min_image_count + 1;

        if surface_caps.max_image_count != 0 {
            preferred.min(surface_caps.max_image_count)
        } else {
            preferred
        }
    }

    fn swapchain_extent(&mut self, surface_caps: &vk::SurfaceCapabilitiesKHR) -> vk::Extent2D {
        self.extent = if surface_caps.current_extent.width != u32::MAX {
            surface_caps.current_extent
        } else {
            vk::Extent2D {
                width: self
                    .extent
                    .width
                    .max(surface_caps.min_image_extent.width)
                    .min(surface_caps.max_image_extent.width),
                height: self
                    .extent
                    .height
                    .max(surface_caps.min_image_extent.height)
                    .min(surface_caps.max_image_extent.height),
            }
        };

        self.extent.clone()
    }

    fn swapchain_transform(
        &self,
        surface_caps: &vk::SurfaceCapabilitiesKHR,
    ) -> vk::SurfaceTransformFlagsKHR {
        const PREFERRED: vk::SurfaceTransformFlagsKHR = vk::SurfaceTransformFlagsKHR::IDENTITY;

        if surface_caps.supported_transforms.contains(PREFERRED) {
            PREFERRED
        } else {
            surface_caps.current_transform
        }
    }

    fn swapchain_composite_alpha(
        &self,
        surface_caps: &vk::SurfaceCapabilitiesKHR,
    ) -> vk::CompositeAlphaFlagsKHR {
        const PREFERRED: vk::CompositeAlphaFlagsKHR = vk::CompositeAlphaFlagsKHR::OPAQUE;

        if surface_caps.supported_composite_alpha.contains(PREFERRED) {
            PREFERRED
        } else {
            vk::CompositeAlphaFlagsKHR::INHERIT
        }
    }
}
