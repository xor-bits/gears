use super::{
    queue::{QueueFamilies, Queues},
    target::window::WindowTargetBuilder,
};
use crate::context::{
    gpu::{any::AnyGPU, suitable::SuitableGPU},
    Context, ContextError,
};
use std::sync::Arc;
use vulkano::{
    device::{
        physical::{MemoryType, PhysicalDevice},
        Device, DeviceExtensions, Features,
    },
    instance::{debug::DebugCallback, Instance},
    swapchain::Surface,
};
use winit::window::Window;

pub struct ReducedContext {
    pub debugger: Option<DebugCallback>,
    pub p_device: SuitableGPU,
    pub instance: Arc<Instance>,
    surface: Arc<Surface<Arc<Window>>>,
}

impl ReducedContext {
    pub fn new(context: Context) -> (ReducedContext, WindowTargetBuilder) {
        (
            ReducedContext {
                debugger: context.debugger,
                p_device: context.p_device,
                instance: context.instance,
                surface: context.target.surface.clone(),
            },
            context.target,
        )
    }
}

pub struct RenderDevice {
    _debugger: Option<DebugCallback>,

    device: Arc<Device>,
    p_device: usize,

    pub queues: Queues,
    pub instance: Arc<Instance>,
}

pub type Dev = Arc<RenderDevice>;

impl RenderDevice {
    pub fn logical(&self) -> &'_ Arc<Device> {
        &self.device
    }

    pub fn physical(&self) -> PhysicalDevice<'_> {
        PhysicalDevice::from_index(&self.instance, self.p_device).unwrap()
    }

    pub fn memory_types(&self) -> impl ExactSizeIterator<Item = MemoryType<'_>> {
        self.physical().memory_types()
    }

    fn device_extensions(p_device: PhysicalDevice) -> DeviceExtensions {
        DeviceExtensions {
            khr_swapchain: true,
            ..p_device.required_extensions().clone()
        }
    }

    pub fn from_context(context: ReducedContext) -> Result<Dev, ContextError> {
        let p_device = context.p_device.device();

        // device extensions

        let device_extensions = Self::device_extensions(p_device);

        // queue infos

        let queue_families = QueueFamilies::new(&context.surface, p_device)?
            .expect("Selected physical device was not suitable");
        let queue_create_infos = queue_families.get();

        // features

        let features = Features {
            geometry_shader: true,
            ..Default::default()
        };

        // device

        let (device, queues) =
            Device::new(p_device, &features, &device_extensions, queue_create_infos)
                .map_err(|err| ContextError::DeviceCreationError(err))?;

        // queues

        let queues = queue_families.get_queues(queues);

        Ok(Arc::new(Self {
            _debugger: context.debugger,

            device,
            p_device: p_device.index(),

            queues,
            instance: context.instance,
        }))
    }
}
