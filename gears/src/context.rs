use self::gpu::suitable::SuitableGPU;
use crate::{debug, renderer::target::window::WindowTargetBuilder};
use std::{env, sync::Arc};
use vulkano::{
    device::DeviceCreationError,
    instance::{
        debug::{DebugCallback, DebugCallbackCreationError},
        layers_list, ApplicationInfo, Instance, InstanceCreationError, InstanceExtensions,
        LayerProperties, LayersListError,
    },
    swapchain::{CapabilitiesError, SurfaceCreationError, SwapchainCreationError},
    Version,
};
use winit::window::Window;

pub mod gpu;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ContextGPUPick {
    /// Automatically picks the GPU.
    ///
    /// This is the default value.
    Automatic,

    /// Pick the GPU with the commandline.
    Manual,
}

impl Default for ContextGPUPick {
    fn default() -> Self {
        ContextGPUPick::Automatic
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ContextValidation {
    /// No vulkan validation layers: greatly increased performance, but reduced debug output.
    /// Used when testing performance or when exporting release builds in production.
    NoValidation,

    /// Vulkan validation: Sacrafice the performance for vulkan API usage validity.
    /// Should be used almost always.
    ///
    /// This is the default value.
    WithValidation,
}

impl Default for ContextValidation {
    fn default() -> Self {
        ContextValidation::WithValidation
    }
}

#[derive(Debug, Clone)]
pub enum ContextError {
    InstanceCreationError(InstanceCreationError),
    LayersListError(LayersListError),
    DebugCallbackCreationError(DebugCallbackCreationError),
    SurfaceCreationError(SurfaceCreationError),
    CapabilitiesError(CapabilitiesError),
    DeviceCreationError(DeviceCreationError),
    SwapchainCreationError(SwapchainCreationError),
    NoSuitableGPUs,
}

pub struct Context {
    pub validation: ContextValidation,
    pub debugger: Option<DebugCallback>,
    pub p_device: SuitableGPU,
    pub target: WindowTargetBuilder,
    pub instance: Arc<Instance>,
}

impl Context {
    fn get_layers(valid: ContextValidation) -> Vec<&'static str> {
        // query available layers from instance

        let available_layers = layers_list().unwrap().collect::<Vec<LayerProperties>>();

        // requested layers

        const VALIDATE: &[&str] = &[
            "VK_LAYER_KHRONOS_validation", /* , "VK_LAYER_LUNARG_monitor" */
        ];
        const NO_VALIDATE: [&str; 0] = [];

        let requested_layers = if valid == ContextValidation::WithValidation {
            &VALIDATE[..]
        } else {
            &NO_VALIDATE[..]
        };

        // remove missing layers

        requested_layers
            .into_iter()
            .cloned()
            .filter(|requested| {
                let found = available_layers
                    .iter()
                    .find(|available| available.name() == *requested)
                    .is_some();

                if !found {
                    log::warn!("Missing layer: {:?}, continuing without it", requested);
                }

                found
            })
            .collect()
    }

    fn get_extensions(valid: ContextValidation) -> InstanceExtensions {
        InstanceExtensions {
            ext_debug_utils: valid == ContextValidation::WithValidation,
            ..vulkano_win::required_extensions()
        }
    }

    pub fn new(
        window: Arc<Window>,
        pick: ContextGPUPick,
        valid: ContextValidation,
    ) -> Result<Self, ContextError> {
        // versions

        let api_version = Version::V1_2;

        let engine_version = (
            env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
            env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
        );

        let application_info = ApplicationInfo {
            application_name: None,
            application_version: None,
            engine_name: Some("gears".into()),
            engine_version: Some(Version::major_minor(engine_version.0, engine_version.1)),
        };

        // layers

        let layers = Self::get_layers(valid);

        // extensions

        let extensions = Self::get_extensions(valid);

        // instance

        let instance = Instance::new(
            Some(&application_info),
            api_version,
            &extensions,
            layers.iter().cloned(),
        )
        .map_err(|err| ContextError::InstanceCreationError(err))?;

        // debugger

        let debugger = if valid == ContextValidation::WithValidation {
            let debugger =
                DebugCallback::new(&instance, debug::SEVERITY, debug::TY, debug::callback)
                    .map_err(|err| ContextError::DebugCallbackCreationError(err))?;

            log::warn!("Debugger enabled");
            Some(debugger)
        } else {
            None
        };

        // target

        let target = WindowTargetBuilder::new(window, instance.clone())?;

        // physical device

        let p_device = SuitableGPU::pick(&instance, &target.surface, pick)?;

        Ok(Self {
            validation: valid,
            instance,
            target,
            p_device,
            debugger,
        })
    }
}
