use super::{debug::Debugger, queue::QueueFamilies, MapErrorLog};

use ash::{
    extensions::{ext, khr},
    version::{EntryV1_0, InstanceV1_0},
    vk, Entry,
};
use colored::Colorize;
use log::{debug, error, info, warn};
use std::ffi::CStr;
use winit::window::Window;

#[derive(Debug)]
pub enum ContextError {
    MissingVulkan,
    MissingInstanceExtensions,
    MissingDeviceExtensions,
    MissingSurfaceConfigs,
    OutOfMemory,
    UnsupportedPlatform,
    NoSuitableGPUs,

    DriverCrash,
    FrameLost,
    FrameInUse,

    InternalError,
}

pub struct Context {
    pub debugger: Debugger,

    pub pdevice: vk::PhysicalDevice,
    pub queue_families: QueueFamilies,

    pub surface: vk::SurfaceKHR,
    pub surface_loader: khr::Surface,
    pub extent: vk::Extent2D,

    pub instance: ash::Instance,
    pub instance_layers: Vec<&'static CStr>,

    pub entry: Entry,
}

impl Context {
    pub fn new(window: &Window, size: (u32, u32)) -> Result<Self, ContextError> {
        let entry = unsafe { ash::Entry::new() }
            .map_err_log("Ash entry creation failed", ContextError::MissingVulkan)?;

        let api_version = match entry
            .try_enumerate_instance_version()
            .map_err_log("Instance version query failed", ContextError::OutOfMemory)?
        {
            Some(version) => version,
            None => vk::make_version(1, 0, 0),
        };

        let engine_version = (
            env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
            env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
            env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
        );

        let application_info = vk::ApplicationInfo::builder()
            .api_version(api_version.min(vk::HEADER_VERSION_COMPLETE))
            .engine_name(CStr::from_bytes_with_nul(b"gears\0").unwrap())
            .engine_version(vk::make_version(
                engine_version.0,
                engine_version.1,
                engine_version.2,
            ));

        // layers

        debug!(
            "Vulkan API version requested: {}.{}.{}",
            vk::version_major(application_info.api_version),
            vk::version_minor(application_info.api_version),
            vk::version_patch(application_info.api_version)
        );

        let available_layers = entry
            .enumerate_instance_layer_properties()
            .map_err_log("Could not query instance layers", ContextError::OutOfMemory)?;

        let mut requested_layers =
            vec![CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap()];
        let mut requested_layers_raw: Vec<*const i8> = requested_layers
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();

        // extensions

        let available_extensions = entry
            .enumerate_instance_extension_properties()
            .map_err_log(
                "Could not query instance extensions",
                ContextError::OutOfMemory,
            )?;

        let mut requested_extensions = vec![ext::DebugUtils::name()];
        requested_extensions.append(
            &mut ash_window::enumerate_required_extensions(window).map_err_log(
                "Could not query window extensions",
                ContextError::UnsupportedPlatform,
            )?,
        );
        let requested_extensions_raw: Vec<*const i8> = requested_extensions
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();

        let missing_layers: Vec<_> = requested_layers
            .iter()
            .filter_map(|layer| {
                if available_layers
                    .iter()
                    .find(|alayer| unsafe { CStr::from_ptr(alayer.layer_name.as_ptr()) } == *layer)
                    .is_none()
                {
                    Some(layer)
                } else {
                    None
                }
            })
            .collect();

        let missing_extensions: Vec<_> = requested_extensions
            .iter()
            .filter_map(|ext| {
                if available_extensions
                    .iter()
                    .find(|aext| unsafe { CStr::from_ptr(aext.extension_name.as_ptr()) } == *ext)
                    .is_none()
                {
                    Some(ext)
                } else {
                    None
                }
            })
            .collect();

        debug!(
            "Requested layers: {:?}\nAvailable layers: {:?}",
            requested_layers, available_layers
        );
        if missing_layers.len() > 0 {
            warn!(
                "Missing layers: {:?}, continuing without validation layers",
                missing_layers
            );
            requested_layers.clear();
            requested_layers_raw.clear();
        }

        debug!(
            "Requested extensions: {:?}\nAvailable extensions: {:?}",
            requested_extensions, available_extensions
        );
        if missing_extensions.len() > 0 {
            error!("Missing extensions: {:?}", missing_extensions);
            return Err(ContextError::MissingInstanceExtensions);
        }

        let instance_info = vk::InstanceCreateInfo::builder()
            .application_info(&application_info)
            .enabled_layer_names(&requested_layers_raw[..])
            .enabled_extension_names(&requested_extensions_raw[..]);

        let instance = unsafe { entry.create_instance(&instance_info, None) }
            .map_err_log("Instance creation failed", ContextError::OutOfMemory)?;

        let surface = unsafe { ash_window::create_surface(&entry, &instance, window, None) }
            .map_err_log(
                "Surface creation failed",
                ContextError::MissingInstanceExtensions,
            )?;

        let debugger = Debugger::new(&entry, &instance);

        let surface_loader = khr::Surface::new(&entry, &instance);
        let mut pdevice_names = Vec::new();
        let pdevice = {
            let mut suitable_pdevices = unsafe { instance.enumerate_physical_devices() }
                .map_err_log("Physical device query failed", ContextError::OutOfMemory)?
                .into_iter()
                .filter_map(|pdevice| {
                    let queue_families =
                        QueueFamilies::new(&instance, &surface_loader, surface, pdevice).ok()?;

                    let (pdevice_name, pdevice_type) = pdevice_name_and_type(&instance, pdevice);

                    pdevice_names.push(pdevice_name);

                    if !queue_families.finished() {
                        None
                    } else {
                        Some((
                            pdevice,
                            queue_families,
                            match pdevice_type {
                                vk::PhysicalDeviceType::DISCRETE_GPU => 4,
                                vk::PhysicalDeviceType::INTEGRATED_GPU => 3,
                                vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
                                vk::PhysicalDeviceType::CPU => 1,
                                _ /* vk::PhysicalDeviceType::OTHER */ => 0,
                            },
                        ))
                    }
                })
                .collect::<Vec<_>>();

            suitable_pdevices.sort_by(|lhs, rhs| rhs.2.cmp(&lhs.2));
            if suitable_pdevices.len() == 0 {
                None
            } else {
                Some(suitable_pdevices.remove(0))
            }
        };

        let (pdevice, queue_families, _) = pdevice.map_err_log(
            &*format!("None of the GPUs ({:?}) are suitable", pdevice_names),
            ContextError::NoSuitableGPUs,
        )?;
        info!("Selected GPU: {}", pdevice_to_string(&instance, pdevice));

        Ok(Self {
            entry,

            instance,
            instance_layers: requested_layers,

            surface,
            surface_loader,
            extent: vk::Extent2D {
                width: size.0,
                height: size.1,
            },

            pdevice,
            queue_families,

            debugger,
        })
    }
}

fn pdevice_name_and_type(
    instance: &ash::Instance,
    pdevice: vk::PhysicalDevice,
) -> (String, vk::PhysicalDeviceType) {
    let pdevice_properties = unsafe { instance.get_physical_device_properties(pdevice) };
    let pdevice_name = unsafe { CStr::from_ptr(pdevice_properties.device_name.as_ptr()) };
    let pdevice_type = pdevice_properties.device_type;

    (pdevice_name.to_str().unwrap().into(), pdevice_type)
}

fn pdevice_to_string(instance: &ash::Instance, pdevice: vk::PhysicalDevice) -> String {
    let (pdevice_name, pdevice_type) = pdevice_name_and_type(instance, pdevice);

    format!(
        "{} (type:{})",
        pdevice_name.cyan(),
        format!("{:?}", pdevice_type).green(),
    )
}
