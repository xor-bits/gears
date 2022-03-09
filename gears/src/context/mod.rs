use crate::debug;
use std::{env, sync::Arc};
use vulkano::{
    device::{physical::SurfacePropertiesError, DeviceCreationError},
    instance::{
        debug::{DebugCallback, DebugCallbackCreationError},
        layers_list, Instance, InstanceCreateInfo, InstanceCreationError, InstanceExtensions,
        LayerProperties, LayersListError,
    },
    swapchain::{SurfaceCreationError, SwapchainCreationError},
    Version,
};

pub mod gpu;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ContextGPUPick {
    /// Automatically picks the GPU.
    Automatic,

    /// Pick the GPU with the commandline.
    Manual,
}

impl Default for ContextGPUPick {
    fn default() -> Self {
        env::var("GEARS_GPU_PICK")
            .map_err(|_| ())
            .and_then(|value| {
                let valid = match value.to_lowercase().as_str() {
                    "auto" => ContextGPUPick::Automatic,
                    "pick" => ContextGPUPick::Manual,
                    other => {
                        log::warn!("Ignored invalid value: {}", other);
                        return Err(());
                    }
                };

                log::info!("Using override ContextGPUPick: {:?}", valid);
                Ok(valid)
            })
            .unwrap_or(ContextGPUPick::Automatic)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ContextValidation {
    /// No Vulkan validation layers: greatly increased performance, but reduced debug output.
    /// Used when testing performance or when exporting release builds in production.
    NoValidation,

    /// Vulkan validation: Sacrifice the performance for Vulkan API usage validity.
    /// Should be used almost always.
    WithValidation,
}

impl Default for ContextValidation {
    fn default() -> Self {
        env::var("GEARS_VALIDATION")
            .map_err(|_| ())
            .and_then(|value| {
                let valid = match value.to_lowercase().as_str() {
                    "full" => ContextValidation::WithValidation,
                    "none" => ContextValidation::NoValidation,
                    other => {
                        log::warn!("Ignored invalid value: {}", other);
                        return Err(());
                    }
                };

                log::info!("Using override ContextValidation: {:?}", valid);
                Ok(valid)
            })
            .unwrap_or(ContextValidation::WithValidation)
    }
}

#[derive(Debug, Clone)]
pub enum ContextError {
    InstanceCreationError(InstanceCreationError),
    LayersListError(LayersListError),
    DebugCallbackCreationError(DebugCallbackCreationError),
    SurfacePropertiesError(SurfacePropertiesError),
    SurfaceCreationError(SurfaceCreationError),
    DeviceCreationError(DeviceCreationError),
    SwapchainCreationError(SwapchainCreationError),
    NoSuitableGPUs,
}

#[derive(Clone)]
pub struct Context {
    pub pick: ContextGPUPick,
    pub validation: ContextValidation,
    pub debugger: Arc<Option<DebugCallback>>,
    pub instance: Arc<Instance>,
}

impl Context {
    fn get_layers(validation: ContextValidation) -> Vec<String> {
        // query available layers from instance

        let available_layers = layers_list().unwrap().collect::<Vec<LayerProperties>>();

        // requested layers

        const VALIDATE: &[&str] = &[
            "VK_LAYER_KHRONOS_validation", /* , "VK_LAYER_LUNARG_monitor" */
        ];
        const NO_VALIDATE: [&str; 0] = [];

        let requested_layers = if validation == ContextValidation::WithValidation {
            VALIDATE
        } else {
            &NO_VALIDATE[..]
        };

        // remove missing layers

        requested_layers
            .iter()
            .cloned()
            .filter(|requested| {
                let found = available_layers
                    .iter()
                    .any(|available| available.name() == *requested);

                if !found {
                    log::warn!("Missing layer: {:?}, continuing without it", requested);
                }

                found
            })
            .map(str::to_string)
            .collect()
    }

    fn get_extensions(validation: ContextValidation) -> InstanceExtensions {
        InstanceExtensions {
            ext_debug_utils: validation == ContextValidation::WithValidation,
            ..vulkano_win::required_extensions()
        }
    }

    /// ### ContextGPUPick
    ///
    /// Environment value `GEARS_GPU_PICK` overrides the `ContextGPUPick` if present.
    ///
    /// Possible values: `auto`, `pick`.
    ///
    /// Defaults to `auto`.
    ///
    /// ### ContextValidation
    ///
    /// Environment value `GEARS_VALIDATION` overrides the `ContextValidation` if present.
    ///
    /// Possible values: `none`, `full`.
    ///
    /// Defaults to `full`.
    pub fn env() -> Result<Self, ContextError> {
        Self::new(Default::default(), Default::default())
    }

    pub fn new(pick: ContextGPUPick, validation: ContextValidation) -> Result<Self, ContextError> {
        // versions

        let engine_version = (
            env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
            env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
        );

        // layers

        let enabled_layers = Self::get_layers(validation);

        // extensions

        let enabled_extensions = Self::get_extensions(validation);

        // instance

        let instance_info = InstanceCreateInfo {
            application_name: None,
            application_version: Version::V1_0,
            engine_name: Some("gears".into()),
            engine_version: Version::major_minor(engine_version.0, engine_version.1),

            enabled_layers,
            enabled_extensions,

            ..Default::default()
        };
        let instance = Instance::new(instance_info).map_err(ContextError::InstanceCreationError)?;

        // debugger

        let debugger = Arc::new(if validation == ContextValidation::WithValidation {
            let debugger =
                DebugCallback::new(&instance, debug::SEVERITY, debug::TY, debug::callback)
                    .map_err(ContextError::DebugCallbackCreationError)?;

            log::warn!("Debugger enabled");
            Some(debugger)
        } else {
            None
        });

        Ok(Self {
            pick,
            validation,
            instance,
            debugger,
        })
    }
}
