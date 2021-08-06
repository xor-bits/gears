use ash::{extensions::khr, version::InstanceV1_0, vk};
use log::{debug, error};
use std::{ffi::CStr, ops, os::raw::c_char, sync::Arc};

use crate::{
    context::{Context, ContextError},
    debug::Debugger,
    MapErrorLog, SurfaceBuilder,
};

use super::queue::{QueueFamilies, Queues};

pub struct ReducedContext {
    pub debugger: Debugger,

    pub pdevice: vk::PhysicalDevice,
    pub queue_families: QueueFamilies,

    pub instance: ash::Instance,
    pub instance_layers: Vec<&'static CStr>,

    pub entry: ash::Entry,
}

impl ReducedContext {
    pub fn new(context: Context) -> (ReducedContext, SurfaceBuilder) {
        (
            ReducedContext {
                debugger: context.debugger,

                pdevice: context.pdevice,
                queue_families: context.queue_families,

                instance: context.instance,
                instance_layers: context.instance_layers,

                entry: context.entry,
            },
            SurfaceBuilder {
                surface: context.surface,
                loader: context.surface_loader,
                extent: context.extent,
            },
        )
    }
}

pub struct RenderDevice {
    _debugger: Debugger,
    pub queues: Queues,

    pub memory_types: Vec<vk::MemoryType>,
    pub pdevice: vk::PhysicalDevice,

    device: ash::Device,
    pub instance: ash::Instance,
    pub entry: ash::Entry,

    pub set_count: usize,
}

pub type Dev = Arc<RenderDevice>;

pub trait DerefDev {
    fn deref_dev(&self) -> &Dev;
}

impl DerefDev for Dev {
    fn deref_dev(&self) -> &Dev {
        self
    }
}

impl RenderDevice {
    // safe if ptrs are not used after instance_layers is modified or dropped
    unsafe fn device_layers(instance_layers: &Vec<&CStr>) -> Vec<*const c_char> {
        instance_layers
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect()
    }

    // safe if instance and pdevice are valid
    unsafe fn device_extensions(
        instance: &ash::Instance,
        pdevice: vk::PhysicalDevice,
    ) -> Result<Vec<*const c_char>, ContextError> {
        let available = instance
            .enumerate_device_extension_properties(pdevice)
            .map_err_log(
                "Could not query instance extensions",
                ContextError::OutOfMemory,
            )?;

        let requested = vec![khr::Swapchain::name()];
        let requested_raw: Vec<*const c_char> =
            requested.iter().map(|raw_name| raw_name.as_ptr()).collect();

        let missing: Vec<_> = requested
            .iter()
            .filter_map(|ext| {
                if available
                    .iter()
                    .find(|aext| &CStr::from_ptr(aext.extension_name.as_ptr()) == ext)
                    .is_none()
                {
                    Some(ext)
                } else {
                    None
                }
            })
            .collect();

        debug!(
            "Requested device extensions: {:?}\nAvailable device extensions: {:?}",
            requested, available
        );
        if missing.len() > 0 {
            error!("Missing device extensions: {:?}", missing);
            return Err(ContextError::MissingDeviceExtensions);
        }

        Ok(requested_raw)
    }

    fn memory_properties(
        instance: &ash::Instance,
        pdevice: vk::PhysicalDevice,
    ) -> vk::PhysicalDeviceMemoryProperties {
        let memory_properties = unsafe { instance.get_physical_device_memory_properties(pdevice) };
        debug!("Memory properties: {:?}", memory_properties);
        memory_properties
    }

    pub fn from_context(
        context: ReducedContext,
        frames_in_flight: usize,
    ) -> Result<Dev, ContextError> {
        // legacy device layers
        // unsafe: instance_layers is dropped in this function
        let instance_layers = unsafe { Self::device_layers(&context.instance_layers) };

        // device extensions
        // unsafe: instance and pdevice are owned by this function
        let device_extensions =
            unsafe { Self::device_extensions(&context.instance, context.pdevice)? };

        // memory
        let memory_types = Self::memory_properties(&context.instance, context.pdevice)
            .memory_types
            .iter()
            .cloned()
            .collect();

        // queues
        let queue_create_infos = context.queue_families.get_vec().unwrap();

        // features
        let features = vk::PhysicalDeviceFeatures {
            geometry_shader: vk::TRUE,
            ..Default::default()
        };

        // device
        let device_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_infos)
            .enabled_layer_names(&instance_layers[..])
            .enabled_extension_names(&device_extensions[..])
            .enabled_features(&features);

        // unsafe: instance is again owned by this function and moving instance or entry will not invalidate device
        let device = unsafe {
            context
                .instance
                .create_device(context.pdevice, &device_info, None)
        }
        .map_err_log("Logical device creation failed", ContextError::OutOfMemory)?;

        // unsafe: queues does not live beyond device, instance or entry. Moving is allowed but destruction is not.
        let queues = unsafe { context.queue_families.get_queues(&device).unwrap() };

        let rdevice = Arc::new(Self {
            _debugger: context.debugger,
            queues,

            memory_types,
            pdevice: context.pdevice,

            device,
            instance: context.instance,
            entry: context.entry,

            set_count: frames_in_flight,
        });

        Ok(rdevice)
    }
}

impl ops::Deref for RenderDevice {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl ops::DerefMut for RenderDevice {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.device
    }
}
