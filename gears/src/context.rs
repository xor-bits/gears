use crate::{
    debug,
    renderer::{queue::QueueFamilies, target::window::WindowTargetBuilder},
};
use bytesize::ByteSize;
use colored::Colorize;
use std::{cmp::Ordering, env, fmt::Write, sync::Arc};
use vulkano::{
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        DeviceCreationError,
    },
    instance::{
        debug::{DebugCallback, DebugCallbackCreationError},
        layers_list, ApplicationInfo, Instance, InstanceCreationError, InstanceExtensions,
        LayerProperties, LayersListError,
    },
    swapchain::{CapabilitiesError, Surface, SurfaceCreationError, SwapchainCreationError},
    Version,
};
use winit::window::Window;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ContextGPUPick {
    /// Automatically picks the GPU.
    Automatic,

    /// Pick the GPU with the commandline.
    Manual,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ContextValidation {
    /// No vulkan validation layers: greatly increased performance, but reduced debug output.
    /// Used when testing performance or when exporting release builds in production.
    NoValidation,

    /// Vulkan validation: Sacrafice the performance for vulkan API usage validity.
    /// Should be used almost always.
    WithValidation,
}

#[derive(Debug, Clone)]
pub enum ContextError {
    MissingInstanceExtensions,
    MissingDeviceExtensions,
    MissingSurfaceConfigs,

    WindowAlreadyTaken,

    InstanceCreationError(InstanceCreationError),
    LayersListError(LayersListError),
    DebugCallbackCreationError(DebugCallbackCreationError),
    SurfaceCreationError(SurfaceCreationError),
    CapabilitiesError(CapabilitiesError),
    DeviceCreationError(DeviceCreationError),
    SwapchainCreationError(SwapchainCreationError),

    OutOfMemory,
    UnsupportedPlatform,
    NoSuitableGPUs,

    DriverCrash,
    FrameLost,
    FrameInUse,

    InternalError,
}

pub struct Context {
    pub validation: ContextValidation,
    pub debugger: Option<DebugCallback>,
    pub p_device: SuitablePhysicalDevice,
    pub target: WindowTargetBuilder,
    pub instance: Arc<Instance>,
}

impl Default for ContextGPUPick {
    fn default() -> Self {
        ContextGPUPick::Automatic
    }
}

impl Default for ContextValidation {
    fn default() -> Self {
        ContextValidation::WithValidation
    }
}

#[derive(Debug, Clone, Copy, Eq)]
pub struct PhysicalDeviceScore {
    type_score: usize,
    memory: u64,
}

impl PhysicalDeviceScore {
    pub fn new(p_device: PhysicalDevice) -> Self {
        // based on the device type
        let type_score = match p_device.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 5,
            PhysicalDeviceType::IntegratedGpu => 4,
            PhysicalDeviceType::VirtualGpu => 3,
            PhysicalDeviceType::Cpu => 2,
            PhysicalDeviceType::Other => 1,
        };

        // based on the local device memory
        let memory = p_device
            .memory_heaps()
            .filter_map(|heap| heap.is_device_local().then(|| heap))
            .map(|heap| heap.size())
            .fold(0, |acc, memory| acc + memory);

        Self { type_score, memory }
    }

    pub fn score(&self) -> u128 {
        self.type_score as u128 * self.memory as u128
    }
}

impl Ord for PhysicalDeviceScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score().cmp(&other.score())
    }
}

impl PartialOrd for PhysicalDeviceScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PhysicalDeviceScore {
    fn eq(&self, other: &Self) -> bool {
        self.score() == other.score()
    }
}

#[derive(Debug, Clone)]
pub struct SuitablePhysicalDevice {
    pub p_device: usize,
    pub instance: Arc<Instance>,
    pub score: PhysicalDeviceScore,
}

#[derive(Debug, Clone)]
pub struct UnsuitablePhysicalDevice {
    pub p_device: usize,
    pub instance: Arc<Instance>,
    pub score: PhysicalDeviceScore,
}

#[derive(Debug, Clone)]
pub enum PhysicalDevicePicker {
    Suitable(SuitablePhysicalDevice),
    Unsuitable(UnsuitablePhysicalDevice),
}

pub trait AnyPhysicalDevice {
    fn score(&self) -> &'_ PhysicalDeviceScore;
    fn device(&self) -> PhysicalDevice<'_>;
    fn suitable(&self) -> bool;
    fn name(&self) -> &'_ String;
}

impl AnyPhysicalDevice for SuitablePhysicalDevice {
    fn score(&self) -> &'_ PhysicalDeviceScore {
        &self.score
    }

    fn device(&self) -> PhysicalDevice<'_> {
        PhysicalDevice::from_index(&self.instance, self.p_device).unwrap()
    }

    fn suitable(&self) -> bool {
        true
    }

    fn name(&self) -> &'_ String {
        &self.device().properties().device_name
    }
}

impl AnyPhysicalDevice for UnsuitablePhysicalDevice {
    fn score(&self) -> &'_ PhysicalDeviceScore {
        &self.score
    }

    fn device(&self) -> PhysicalDevice<'_> {
        PhysicalDevice::from_index(&self.instance, self.p_device).unwrap()
    }

    fn suitable(&self) -> bool {
        false
    }

    fn name(&self) -> &'_ String {
        &self.device().properties().device_name
    }
}

impl PhysicalDevicePicker {
    fn get_internal(&self) -> &dyn AnyPhysicalDevice {
        match self {
            PhysicalDevicePicker::Suitable(d) => d,
            PhysicalDevicePicker::Unsuitable(d) => d,
        }
    }
}

impl AnyPhysicalDevice for PhysicalDevicePicker {
    fn score(&self) -> &'_ PhysicalDeviceScore {
        self.get_internal().score()
    }

    fn device(&self) -> PhysicalDevice<'_> {
        self.get_internal().device()
    }

    fn suitable(&self) -> bool {
        self.get_internal().suitable()
    }

    fn name(&self) -> &'_ String {
        self.get_internal().name()
    }
}

impl Context {
    fn get_layers(valid: ContextValidation) -> Vec<&'static str> {
        /* // query available layers from instance
        let available_layers = layers_list()
            .map_err(|err| ContextError::LayersListError(err))?
            .map(|l| l.name().to_string())
            .collect::<Vec<_>>();

        // requested layers

        const khronos_validation_layer: &str = "VK_LAYER_KHRONOS_validation";
        const lunarg_monitor_layer: &str = "VK_LAYER_LUNARG_monitor";
        let mut requested_layers: Vec<&str> = if valid == ContextValidation::WithValidation {
            vec![khronos_validation_layer, lunarg_monitor_layer]
        } else {
            vec![]
        };

        // check for missing layers
        let missing_layers = requested_layers
            .iter()
            .cloned()
            .filter_map(|layer| {
                if available_layers.contains(layer) {
                    Some(layer)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        log::debug!(
            "Requested layers: {:?}\nAvailable layers: {:?}\nMissing layers: {:?}",
            requested_layers,
            available_layers,
            missing_layers
        );
        if missing_layers.len() > 0 {
            log::warn!(
                "Missing layers: {:?}, continuing without these validation layers",
                missing_layers
            );

            requested_layers = requested_layers
                .iter()
                .cloned()
                .filter_map(|layer| {
                    if missing_layers.contains(layer) {
                        None
                    } else {
                        Some(layer)
                    }
                })
                .collect();
        } else {
            log::debug!("No missing layers");
        }

        Ok(requested_layers) */

        // query available layers from instance

        let available_layers = layers_list().unwrap().collect::<Vec<LayerProperties>>();

        // requested layers

        const VALIDATE: [&str; 2] = ["VK_LAYER_KHRONOS_validation", "VK_LAYER_LUNARG_monitor"];
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

    fn pick_gpu(
        instance: &Arc<Instance>,
        surface: &Arc<Surface<Arc<Window>>>,
        pick: ContextGPUPick,
    ) -> Result<SuitablePhysicalDevice, ContextError> {
        let p_devices = PhysicalDevice::enumerate(instance)
            .map(|p_device| {
                let queue_families = QueueFamilies::new(surface, p_device)?;
                let score = PhysicalDeviceScore::new(p_device);

                if queue_families.is_some() {
                    Ok(PhysicalDevicePicker::Suitable(SuitablePhysicalDevice {
                        instance: instance.clone(),
                        p_device: p_device.index(),
                        score,
                    }))
                } else {
                    Ok(PhysicalDevicePicker::Unsuitable(UnsuitablePhysicalDevice {
                        instance: instance.clone(),
                        p_device: p_device.index(),
                        score,
                    }))
                }
            })
            .collect::<Result<Vec<PhysicalDevicePicker>, ContextError>>()?;

        Self::gpu_picker(&p_devices, pick)
    }

    fn gpu_list<'a>(
        p_devices: impl Iterator<Item = &'a dyn AnyPhysicalDevice>,
        ignore_invalid: bool,
    ) -> String {
        let mut buf = String::new();
        for (i, p_device) in p_devices
            .filter(|p_device| {
                if !ignore_invalid {
                    true
                } else {
                    p_device.suitable()
                }
            })
            .enumerate()
        {
            let suitable = if p_device.suitable() {
                "[\u{221a}]".green()
            } else {
                "[X]".red()
            };
            let device = p_device.device().properties();
            let name = device.device_name.blue();
            let ty = format!("{:?}", device.device_type).blue();
            let mem = ByteSize::b(p_device.score().memory).to_string().blue();
            let score = p_device.score().score().to_string().yellow();

            writeln!(buf, "- GPU index: {}", i).unwrap();
            writeln!(buf, "  - suitable: {}", suitable).unwrap();
            writeln!(buf, "  - name: {}", name).unwrap();
            writeln!(buf, "  - type: {}", ty).unwrap();
            writeln!(buf, "  - memory: {}", mem).unwrap();
            writeln!(buf, "  - automatic score: {}", score).unwrap();
        }
        buf
    }

    fn gpu_picker(
        p_devices: &Vec<PhysicalDevicePicker>,
        pick: ContextGPUPick,
    ) -> Result<SuitablePhysicalDevice, ContextError> {
        let mut suitable = p_devices
            .iter()
            .filter_map(|p_device| match p_device {
                PhysicalDevicePicker::Suitable(d) => Some(d.clone()),
                PhysicalDevicePicker::Unsuitable(_) => None,
            })
            .collect::<Vec<SuitablePhysicalDevice>>();
        let all_iter = p_devices.iter().map(|d| d as &dyn AnyPhysicalDevice);
        let suitable_iter = suitable.iter().map(|d| d as &dyn AnyPhysicalDevice);

        let p_device = if suitable.len() == 0 {
            None
        } else if suitable.len() == 1 {
            if pick == ContextGPUPick::Manual {
                log::warn!(
                    "ContextGPUPick was set to Manual but only one suitable GPU was available"
                )
            }

            Some(suitable.remove(0))
        } else if pick == ContextGPUPick::Manual {
            println!("Pick a GPU:\n{}", Self::gpu_list(suitable_iter, true,));

            let stdin = std::io::stdin();
            let mut stdout = std::io::stdout();
            let i = loop {
                print!("Number: ");
                std::io::Write::flush(&mut stdout).unwrap();
                let mut buf = String::new();
                stdin.read_line(&mut buf).unwrap();

                match buf.trim_end().parse::<usize>() {
                    Ok(i) => {
                        if i >= suitable.len() {
                            println!(
                                "{} is not a valid GPU index between 0 and {}",
                                i,
                                suitable.len() + 1
                            );
                        } else {
                            break i;
                        }
                    }
                    Err(_) => {
                        println!("'{}' is not a valid GPU index", buf);
                    }
                }
            };

            // optionally save the manual pick
            // p_device.properties().device_uuid;

            Some(suitable.remove(i))
        } else {
            suitable.sort_by_key(|d| d.score().clone());
            Some(suitable.remove(0))
        };

        match p_device {
            Some(p_device) => {
                log::info!(
                    "Picked: GPU index: {} ({}) from:\n{}",
                    p_device.device().index(),
                    p_device.device().properties().device_name.blue(),
                    Self::gpu_list(all_iter, false)
                );
                Ok(p_device)
            }
            None => {
                log::error!(
                    "None of the GPUs (bellow) are suitable:\n{}",
                    Self::gpu_list(all_iter, false)
                );
                Err(ContextError::NoSuitableGPUs)
            }
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

        let p_device = Self::pick_gpu(&instance, &target.surface, pick)?;

        Ok(Self {
            validation: valid,
            instance,
            target,
            p_device,
            debugger,
        })
    }
}
