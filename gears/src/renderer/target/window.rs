use std::sync::Arc;

use vulkano::{
    format::Format,
    image::{ImageUsage, SwapchainImage},
    instance::Instance,
    swapchain::{
        acquire_next_image, Capabilities, ColorSpace, CompositeAlpha, PresentMode, Surface,
        SurfaceTransform, Swapchain, SwapchainAcquireFuture,
    },
    sync::SharingMode,
};
use vulkano_win::create_vk_surface;
use winit::window::Window;

use crate::{context::ContextError, renderer::device::Dev, SyncMode};

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
    pub surface: Arc<Surface<Arc<Window>>>,
}

impl WindowTargetBuilder {
    pub fn new(window: Arc<Window>, instance: Arc<Instance>) -> Result<Self, ContextError> {
        let size = window.inner_size();
        let surface = create_vk_surface(window, instance)
            .map_err(|err| ContextError::SurfaceCreationError(err))?;

        Ok(Self {
            extent: [size.width, size.height],
            surface,
        })
    }

    pub fn build(
        mut self,
        device: &Dev,
        sync: SyncMode,
    ) -> Result<(WindowTarget, Vec<Arc<SwapchainImage<Arc<Window>>>>), ContextError> {
        let info = self.swapchain_info(device, sync)?;

        let sharing = if device.queues.present == device.queues.graphics {
            SharingMode::Exclusive
        } else {
            SharingMode::from(&[&device.queues.present, &device.queues.graphics][..])
        };

        let (swapchain, images) = Swapchain::start(device.logical().clone(), self.surface.clone())
            .num_images(info.len)
            .format(info.format.0)
            .color_space(info.format.1)
            .dimensions(info.extent)
            .usage(ImageUsage::color_attachment())
            .sharing_mode(sharing)
            .transform(info.transform)
            .composite_alpha(info.composite_alpha)
            .present_mode(info.present)
            .clipped(true)
            .layers(1)
            .build()
            .map_err(|err| ContextError::SwapchainCreationError(err))?;

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
            format: self.pick_format(&caps)?,
            present: self.pick_present_mode(&caps, sync)?,

            len: self.swapchain_len(&caps),
            extent: self.swapchain_extent(&caps),
            transform: self.swapchain_transform(&caps),
            composite_alpha: self.swapchain_composite_alpha(&caps),
        })
    }

    fn capabilities(&self, device: &Dev) -> Result<Capabilities, ContextError> {
        self.surface
            .capabilities(device.physical())
            .map_err(|err| ContextError::CapabilitiesError(err))
    }

    fn pick_format(
        &self,
        surface_caps: &Capabilities,
    ) -> Result<(Format, ColorSpace), ContextError> {
        let format = surface_caps
            .supported_formats
            .iter()
            .find(|(format, color_space)| {
                format == &Format::R8G8B8A8Srgb && color_space == &ColorSpace::SrgbNonLinear
            })
            .unwrap_or(&surface_caps.supported_formats[0]);
        let format = format.clone();

        log::debug!(
            "Surface format chosen: {:?} from {:?}",
            format,
            surface_caps.supported_formats
        );

        Ok(format)
    }

    fn pick_present_mode(
        &self,
        surface_caps: &Capabilities,
        sync: SyncMode,
    ) -> Result<PresentMode, ContextError> {
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
            SyncMode::Immediate => {
                fallback(surface_caps.present_modes.immediate, PresentMode::Immediate)
            }
            SyncMode::Mailbox => fallback(surface_caps.present_modes.mailbox, PresentMode::Mailbox),
        };

        log::debug!("Surface present mode chosen: {:?}", mode,);

        Ok(mode)
    }

    fn swapchain_len(&self, surface_caps: &Capabilities) -> u32 {
        let preferred = surface_caps.min_image_count + 1;

        if let Some(max_image_count) = surface_caps.max_image_count {
            preferred.min(max_image_count)
        } else {
            preferred
        }
    }

    fn swapchain_extent(&mut self, surface_caps: &Capabilities) -> [u32; 2] {
        if let Some(extent) = surface_caps.current_extent {
            self.extent = extent;
        } else {
            for i in 0..=1 {
                self.extent[i] = self.extent[i]
                    .max(surface_caps.min_image_extent[i])
                    .min(surface_caps.max_image_extent[i]);
            }
        };

        self.extent.clone()
    }

    fn swapchain_transform(&self, surface_caps: &Capabilities) -> SurfaceTransform {
        if surface_caps.supported_transforms.identity {
            SurfaceTransform::Identity
        } else {
            surface_caps.current_transform
        }
    }

    fn swapchain_composite_alpha(&self, surface_caps: &Capabilities) -> CompositeAlpha {
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
    pub swapchain: Arc<Swapchain<Arc<Window>>>,
}

impl WindowTarget {
    pub fn acquire_image(&self) -> Option<(usize, SwapchainAcquireFuture<Arc<Window>>)> {
        match acquire_next_image(self.swapchain.clone(), None) {
            Ok((image_index, _, future)) => Some((image_index, future)),
            Err(_) => None,
        }
    }

    pub fn extent(&mut self, device: &Dev) -> Result<[u32; 2], ContextError> {
        let surface_caps = self.base.capabilities(device)?;
        Ok(self.base.swapchain_extent(&surface_caps))
    }

    pub fn recreate(&mut self) -> Result<Vec<Arc<SwapchainImage<Arc<Window>>>>, ContextError> {
        let (swapchain, images) = self
            .swapchain
            .recreate()
            .build()
            .map_err(|err| ContextError::SwapchainCreationError(err))?;

        self.swapchain = swapchain;
        Ok(images)
    }
}
