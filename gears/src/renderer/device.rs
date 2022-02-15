use super::queue::{QueueFamilies, Queues};
use crate::{
    context::{gpu::any::AnyGPU, Context, ContextError},
    frame::Frame,
};
use std::sync::Arc;
use vulkano::device::{
    physical::{MemoryType, PhysicalDevice},
    Device, DeviceCreateInfo, DeviceExtensions, Features,
};

//

pub struct RenderDevice {
    context: Context,

    device: Arc<Device>,
    p_device: usize,

    pub queues: Queues,
}

//

pub type Dev = Arc<RenderDevice>;

//

impl RenderDevice {
    pub fn logical(&self) -> &'_ Arc<Device> {
        &self.device
    }

    pub fn physical(&self) -> PhysicalDevice<'_> {
        PhysicalDevice::from_index(&self.context.instance, self.p_device).unwrap()
    }

    pub fn memory_types(&self) -> impl ExactSizeIterator<Item = MemoryType<'_>> {
        self.physical().memory_types()
    }

    fn device_extensions(p_device: PhysicalDevice) -> DeviceExtensions {
        DeviceExtensions {
            khr_swapchain: true,
            ..*p_device.required_extensions()
        }
    }

    pub fn from_frame(frame: &Frame) -> Result<Dev, ContextError> {
        let context = frame.context();
        let gpu = frame.gpu();
        let p_device = gpu.device();
        let surface = frame.surface();

        // device extensions

        let enabled_extensions = Self::device_extensions(p_device);

        // queue infos

        let queue_families = QueueFamilies::new(&surface, p_device)?
            .expect("Selected physical device was not suitable");
        let queue_create_infos = queue_families.get();

        // features

        let enabled_features = Features {
            geometry_shader: true,
            ..Default::default()
        };

        // device

        let device_info = DeviceCreateInfo {
            enabled_extensions,
            enabled_features,
            queue_create_infos,
            ..Default::default()
        };
        let (device, queues) =
            Device::new(p_device, device_info).map_err(ContextError::DeviceCreationError)?;

        // queues

        let queues = queue_families.get_queues(queues);

        Ok(Arc::new(Self {
            context,

            device,
            p_device: p_device.index(),

            queues,
        }))
    }
}

impl Drop for RenderDevice {
    fn drop(&mut self) {
        log::debug!("Dropping RenderDevice");
    }
}
