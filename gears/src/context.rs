use super::{debug::Debugger, renderer::queue::QueueFamilies, MapErrorLog};

use ash::{
    extensions::{ext, khr},
    prelude::VkResult,
    version::{EntryV1_0, InstanceV1_0},
    vk, Entry,
};
use colored::Colorize;
use log::{debug, error, info, warn};
use std::{
    collections::HashSet,
    env,
    ffi::CStr,
    io::{self, Write},
    os::raw::c_char,
};
use winit::window::Window;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ContextGPUPick {
    /// Automatically picks the GPU, or asks if environment value 'GEARS_GPU' is set to 'pick'
    Automatic,

    /// Pick the GPU with the commandline
    Manual,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ContextValidation {
    /// No vulkan validation layers: greatly increased performance, but reduced debug output
    /// Used when testing performance or when exporting release builds in production
    NoValidation,

    /// Vulkan validation: Sacrafice the performance for vulkan API usage validity.
    /// Should be used almost always.
    WithValidation,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
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

impl Context {
    pub fn new(
        window: &Window,
        size: (u32, u32),
        pick: ContextGPUPick,
        valid: ContextValidation,
    ) -> Result<Self, ContextError> {
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

        debug!(
            "Vulkan API version requested: {}.{}.{}",
            vk::version_major(application_info.api_version),
            vk::version_minor(application_info.api_version),
            vk::version_patch(application_info.api_version)
        );

        // layers

        // query available layers from instance
        let available_layers = entry
            .enumerate_instance_layer_properties()
            .map_err_log("Could not query instance layers", ContextError::OutOfMemory)?;

        // requested layers
        let khronos_validation_layer =
            CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap();
        let mut requested_layers: Vec<&CStr> = if valid == ContextValidation::WithValidation {
            vec![khronos_validation_layer]
        } else {
            vec![]
        };
        let mut requested_layers_raw: Vec<*const c_char> = requested_layers
            .iter()
            .map(|layer| layer.as_ptr())
            .collect();

        // check for missing layers
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
        debug!("No missing layers");

        // extensions

        // query available extensions from instance and layers
        const INSTANCE_QUERY_ERROR_MSG: &str = "Could not query instance extensions";
        const INSTANCE_QUERY_ERROR: ContextError = ContextError::OutOfMemory;
        let mut available_extensions_unique = HashSet::new();
        let available_extensions = requested_layers
            .iter()
            .map(|layer| {
                enumerate_instance_extension_properties_with_layer(&entry, layer)
                    .map_err_log(INSTANCE_QUERY_ERROR_MSG, INSTANCE_QUERY_ERROR)
            })
            .collect::<Result<Vec<_>, ContextError>>()?
            .into_iter()
            .flatten()
            .chain(
                entry
                    .enumerate_instance_extension_properties()
                    .map_err_log(INSTANCE_QUERY_ERROR_MSG, INSTANCE_QUERY_ERROR)?,
            )
            .filter_map(|properties| {
                if available_extensions_unique.contains(&properties.extension_name) {
                    None
                } else {
                    available_extensions_unique.insert(properties.extension_name);
                    Some(properties)
                }
            })
            .collect::<Vec<_>>();

        // requested extensions
        let mut requested_extensions = if valid == ContextValidation::WithValidation {
            vec![ext::DebugUtils::name()]
        } else {
            vec![]
        };
        requested_extensions.append(
            &mut ash_window::enumerate_required_extensions(window).map_err_log(
                "Could not query window extensions",
                ContextError::UnsupportedPlatform,
            )?,
        );
        let requested_extensions_raw: Vec<*const c_char> = requested_extensions
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();

        // check for missing extensions
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
            "Requested extensions: {:?}\nAvailable extensions: {:?}",
            requested_extensions, available_extensions
        );
        if missing_extensions.len() > 0 {
            error!("Missing extensions: {:?}", missing_extensions);
            return Err(ContextError::MissingInstanceExtensions);
        }
        debug!("No missing extensions");

        // instance

        let instance_info = vk::InstanceCreateInfo::builder()
            .application_info(&application_info)
            .enabled_layer_names(&requested_layers_raw[..])
            .enabled_extension_names(&requested_extensions_raw[..]);

        let instance = unsafe { entry.create_instance(&instance_info, None) }
            .map_err_log("Instance creation failed", ContextError::OutOfMemory)?;
        debug!("Vulkan instance created");

        // surface

        let surface = unsafe { ash_window::create_surface(&entry, &instance, window, None) }
            .map_err_log(
                "Surface creation failed",
                ContextError::MissingInstanceExtensions,
            )?;
        debug!("Surface created");

        // debugger

        let debugger = Debugger::new(&entry, &instance)
            .map_err_log("Debugger creation failed", ContextError::OutOfMemory)?;
        debug!("Debugger created");

        // physical device

        let surface_loader = khr::Surface::new(&entry, &instance);
        let mut pdevice_names = Vec::new();
        let pdevice = {
            let mut suitable_pdevices = unsafe { instance.enumerate_physical_devices() }
                .map_err_log("Physical device query failed", ContextError::OutOfMemory)?
                .into_iter()
                .filter_map(|pdevice| {
                    let queue_families = unsafe {
                        QueueFamilies::new(&instance, &surface_loader, surface, pdevice).ok()?
                    };

                    let (pdevice_name, pdevice_type) = pdevice_name_and_type(&instance, pdevice);
                    let finished = queue_families.finished();

                    pdevice_names.push((pdevice_name, pdevice_type, finished));

                    if !finished {
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

            if suitable_pdevices.len() == 0 {
                None
            } else if pick == ContextGPUPick::Manual
                || env::var("GEARS_GPU").map_or(false, |value| value.to_lowercase() == "pick")
            {
                println!(
                    "Pick a GPU: {}",
                    all_pdevices_to_string(&pdevice_names, true)
                );

                let stdin = io::stdin();
                let mut stdout = io::stdout();
                let i = loop {
                    print!("Number: ");
                    stdout.flush().unwrap();
                    let mut buf = String::new();
                    stdin.read_line(&mut buf).unwrap();

                    match buf.trim_end().parse::<usize>() {
                        Ok(i) => {
                            if i >= suitable_pdevices.len() {
                                println!(
                                    "{} is not a valid GPU index between 0 and {}",
                                    i,
                                    suitable_pdevices.len() + 1
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

                Some(suitable_pdevices.remove(i))
            } else {
                suitable_pdevices.sort_by(|lhs, rhs| rhs.2.cmp(&lhs.2));
                Some(suitable_pdevices.remove(0))
            }
        };

        let (pdevice, queue_families, _) = pdevice.ok_or_else(|| {
            error!(
                "None of the GPUs (bellow) are suitable: {}",
                all_pdevices_to_string(&pdevice_names, false)
            );
            ContextError::NoSuitableGPUs
        })?;
        info!(
            "GPU chosen: {} from: {}",
            pdevice_to_string(&instance, pdevice),
            all_pdevices_to_string(&pdevice_names, false)
        );

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

fn enumerate_instance_extension_properties_with_layer(
    entry: &Entry,
    p_layer_name: &CStr,
) -> VkResult<Vec<vk::ExtensionProperties>> {
    unsafe {
        let mut num = 0;
        entry
            .fp_v1_0()
            .enumerate_instance_extension_properties(
                p_layer_name.as_ptr(),
                &mut num,
                std::ptr::null_mut(),
            )
            .result()?;
        let mut data = Vec::with_capacity(num as usize);
        let err_code = entry.fp_v1_0().enumerate_instance_extension_properties(
            p_layer_name.as_ptr(),
            &mut num,
            data.as_mut_ptr(),
        );
        data.set_len(num as usize);
        err_code.result_with_success(data)
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

fn all_pdevices_to_string(
    pdevice_names: &Vec<(String, vk::PhysicalDeviceType, bool)>,
    ignore_invalid: bool,
) -> String {
    let mut len = 0;
    for (name, _, _) in pdevice_names.iter() {
        len += name.len();
    }

    let mut buf = String::with_capacity(len);
    for (i, (name, pdevice_type, suitable)) in pdevice_names
        .iter()
        .filter(|(_, _, s)| if ignore_invalid { *s } else { true })
        .enumerate()
    {
        buf.push_str(
            format!(
                "\n - {}: [{}] {} (type:{})",
                i,
                if *suitable {
                    "\u{221a}".green()
                } else {
                    " ".white()
                },
                name.cyan(),
                format!("{:?}", pdevice_type).green()
            )
            .as_str(),
        );
    }
    buf
}
