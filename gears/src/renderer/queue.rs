use std::sync::Arc;

use ash::{
    extensions::khr::Surface,
    version::{DeviceV1_0, InstanceV1_0},
    vk,
};

use crate::{ContextError, MapErrorLog};

const PRIORITY: [f32; 1] = [1.0];
pub struct QueueFamilies {
    pub present: Option<usize>,
    pub graphics: Option<usize>,
}

pub struct Queues {
    pub present: vk::Queue,
    pub present_family: usize,
    pub graphics: vk::Queue,
    pub graphics_family: usize,
}

impl QueueFamilies {
    pub fn new(
        instance: &ash::Instance,
        surface_loader: &Surface,
        surface: vk::SurfaceKHR,
        pdevice: vk::PhysicalDevice,
    ) -> Result<Self, ContextError> {
        let mut queue_families = Self {
            present: None,
            graphics: None,
        };

        let queue_family_properties =
            unsafe { instance.get_physical_device_queue_family_properties(pdevice) };

        for (index, queue_family_property) in queue_family_properties.into_iter().enumerate() {
            let present_support = unsafe {
                surface_loader.get_physical_device_surface_support(pdevice, index as u32, surface)
            }
            .map_err_log(
                "Physical device surface support query failed",
                ContextError::OutOfMemory,
            )?;

            let graphics_support = queue_family_property
                .queue_flags
                .contains(vk::QueueFlags::GRAPHICS);

            if present_support {
                queue_families.present = Some(index);
            }
            if graphics_support {
                queue_families.graphics = Some(index);
            }

            if queue_families.finished() {
                break;
            }
        }

        Ok(queue_families)
    }

    pub fn finished(&self) -> bool {
        self.present.is_some() && self.graphics.is_some()
    }

    pub fn same(&self) -> Option<bool> {
        Some(self.present? == self.graphics?)
    }

    pub fn get_vec(&self) -> Option<Vec<vk::DeviceQueueCreateInfo>> {
        if self.same()? {
            Some(vec![vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(self.present.unwrap() as u32)
                .queue_priorities(&PRIORITY)
                .build()])
        } else {
            Some(vec![
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(self.present.unwrap() as u32)
                    .queue_priorities(&PRIORITY)
                    .build(),
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(self.graphics.unwrap() as u32)
                    .queue_priorities(&PRIORITY)
                    .build(),
            ])
        }
    }

    pub fn get_queues(&self, device: Arc<ash::Device>) -> Option<Queues> {
        if self.same()? {
            let queue = unsafe { device.get_device_queue(self.present.unwrap() as u32, 0) };

            Some(Queues {
                present: queue,
                present_family: self.present.unwrap(),
                graphics: queue,
                graphics_family: self.present.unwrap(),
            })
        } else {
            let present = unsafe { device.get_device_queue(self.present.unwrap() as u32, 0) };
            let graphics = unsafe { device.get_device_queue(self.graphics.unwrap() as u32, 0) };

            Some(Queues {
                present,
                present_family: self.present.unwrap(),
                graphics,
                graphics_family: self.graphics.unwrap(),
            })
        }
    }
}
