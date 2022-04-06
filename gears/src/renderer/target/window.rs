use crate::{context::ContextError, renderer::device::Dev, SyncMode};
use smallvec::SmallVec;
use std::{iter::FromIterator, sync::Arc};
use vulkano::{
    format::Format,
    image::{ImageUsage, SwapchainImage},
    swapchain::{
        acquire_next_image, ColorSpace, CompositeAlpha, PresentMode, Surface, SurfaceCapabilities,
        SurfaceInfo, SurfaceTransform, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo,
    },
    sync::Sharing,
};
use winit::window::Window;

//

pub struct SwapchainInfo {
    format: (Format, ColorSpace),
    present: PresentMode,
    len: u32,
    extent: [u32; 2],
    transform: SurfaceTransform,
    composite_alpha: CompositeAlpha,
}

pub struct WindowTargetBuilder {
    pub extent: [u32; 2],
    pub surface: Arc<Surface<Window>>,
}

//

pub type SwapchainImages = Vec<Arc<SwapchainImage<Window>>>;

//

impl WindowTargetBuilder {
    pub fn new(surface: Arc<Surface<Window>>) -> Result<Self, ContextError> {
        let size = surface.window().inner_size();
        Ok(Self {
            extent: [size.width, size.height],
            surface,
        })
    }

    pub fn build(
        mut self,
        device: &Dev,
        sync: SyncMode,
    ) -> Result<(WindowTarget, SwapchainImages), ContextError> {
        let info = self.swapchain_info(device, sync)?;

        let sharing = if device.queues.present == device.queues.graphics {
            Sharing::Exclusive
        } else {
            Sharing::Concurrent(SmallVec::from_iter([
                device.queues.present.family().id(),
                device.queues.graphics.family().id(),
            ]))
        };

        let create_info = SwapchainCreateInfo {
            min_image_count: info.len,
            image_format: Some(info.format.0),
            image_color_space: info.format.1,
            image_extent: info.extent,
            image_array_layers: 1,
            image_usage: ImageUsage::color_attachment(),
            image_sharing: sharing,
            pre_transform: info.transform,
            composite_alpha: info.composite_alpha,
            present_mode: info.present,
            clipped: true,
            ..Default::default()
        };

        let (swapchain, images) =
            Swapchain::new(device.logical().clone(), self.surface.clone(), create_info)
                .map_err(ContextError::SwapchainCreationError)?;

        Ok((
            WindowTarget {
                base: self,
                format: info.format,
                swapchain,
            },
            images,
        ))
    }

    fn swapchain_info(
        &mut self,
        device: &Dev,
        sync: SyncMode,
    ) -> Result<SwapchainInfo, ContextError> {
        let caps = self.capabilities(device)?;
        Ok(SwapchainInfo {
            format: self.pick_format(device)?,
            present: self.pick_present_mode(device, sync)?,

            len: self.swapchain_len(&caps),
            extent: self.swapchain_extent(&caps),
            transform: self.swapchain_transform(&caps),
            composite_alpha: self.swapchain_composite_alpha(&caps),
        })
    }

    fn capabilities(&self, device: &Dev) -> Result<SurfaceCapabilities, ContextError> {
        device
            .physical()
            .surface_capabilities(&self.surface, Default::default())
            .map_err(ContextError::SurfacePropertiesError)
    }

    fn pick_format(&self, device: &Dev) -> Result<(Format, ColorSpace), ContextError> {
        let supported_formats = device
            .physical()
            .surface_formats(&self.surface, SurfaceInfo::default() /* ? */)
            .map_err(ContextError::SurfacePropertiesError)?;
        let format = supported_formats
            .iter()
            .find(|(format, color_space)| {
                format == &Format::R8G8B8A8_SRGB && color_space == &ColorSpace::SrgbNonLinear
            })
            .unwrap_or(&supported_formats[0]);
        let format = *format;

        log::debug!(
            "Surface format chosen: {:?} from {:?}",
            format,
            supported_formats
        );

        Ok(format)
    }

    fn pick_present_mode(&self, device: &Dev, sync: SyncMode) -> Result<PresentMode, ContextError> {
        let mut immediate_supported = false;
        let mut mailbox_supported = false;
        device
            .physical()
            .surface_present_modes(&self.surface)
            .map_err(ContextError::SurfacePropertiesError)?
            .for_each(|mode| match mode {
                PresentMode::Immediate => immediate_supported = true,
                PresentMode::Mailbox => mailbox_supported = true,
                _ => {}
            });

        let fallback = |a: bool, b: PresentMode| -> PresentMode {
            if a {
                b
            } else {
                log::warn!("Requested present mode: '{:?}' not supported", b);
                PresentMode::Fifo
            }
        };

        let mode = match sync {
            SyncMode::Fifo => PresentMode::Fifo,
            SyncMode::Immediate => fallback(immediate_supported, PresentMode::Immediate),
            SyncMode::Mailbox => fallback(mailbox_supported, PresentMode::Mailbox),
        };

        log::debug!("Surface present mode chosen: {:?}", mode,);

        Ok(mode)
    }

    fn swapchain_len(&self, surface_caps: &SurfaceCapabilities) -> u32 {
        let preferred = surface_caps.min_image_count + 1;

        if let Some(max_image_count) = surface_caps.max_image_count {
            preferred.min(max_image_count)
        } else {
            preferred
        }
    }

    fn swapchain_extent(&mut self, surface_caps: &SurfaceCapabilities) -> [u32; 2] {
        if let Some(extent) = surface_caps.current_extent {
            self.extent = extent;
        } else {
            for i in 0..=1 {
                self.extent[i] = self.extent[i]
                    .max(surface_caps.min_image_extent[i])
                    .min(surface_caps.max_image_extent[i]);
            }
        };

        self.extent
    }

    fn swapchain_transform(&self, surface_caps: &SurfaceCapabilities) -> SurfaceTransform {
        if surface_caps.supported_transforms.identity {
            SurfaceTransform::Identity
        } else {
            surface_caps.current_transform
        }
    }

    fn swapchain_composite_alpha(&self, surface_caps: &SurfaceCapabilities) -> CompositeAlpha {
        if surface_caps.supported_composite_alpha.opaque {
            CompositeAlpha::Opaque
        } else {
            CompositeAlpha::Inherit
        }
    }
}

pub struct WindowTarget {
    pub base: WindowTargetBuilder,
    pub format: (Format, ColorSpace),
    pub swapchain: Arc<Swapchain<Window>>,
}

impl WindowTarget {
    pub fn acquire_image(&self) -> Option<(usize, SwapchainAcquireFuture<Window>)> {
        match acquire_next_image(self.swapchain.clone(), None) {
            Ok((image_index, false, future)) => Some((image_index, future)),
            Ok((_, true, _)) => None,
            Err(_) => None,
        }
    }

    pub fn extent(&mut self, device: &Dev) -> Result<[u32; 2], ContextError> {
        let surface_caps = self.base.capabilities(device)?;
        Ok(self.base.swapchain_extent(&surface_caps))
    }

    pub fn recreate(&mut self) -> Result<SwapchainImages, ContextError> {
        let create_info = SwapchainCreateInfo {
            image_extent: [0, 0],
            ..self.swapchain.create_info()
        };

        let (swapchain, images) = self
            .swapchain
            .recreate(create_info)
            .map_err(ContextError::SwapchainCreationError)?;

        self.base.extent = swapchain.image_extent();
        self.swapchain = swapchain;
        Ok(images)
    }
}
