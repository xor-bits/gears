use ash::{
    extensions::khr::Surface,
    version::{DeviceV1_0, InstanceV1_0},
    vk,
};

use crate::{context::ContextError, MapErrorLog};

const PRIORITY: [f32; 1] = [1.0];
pub struct QueueFamilies {
    pub present: Option<usize>,
    pub graphics: Option<usize>,
    pub transfer: Option<usize>,
}

pub struct Queues {
    pub present: vk::Queue,
    pub present_family: usize,
    pub graphics: vk::Queue,
    pub graphics_family: usize,
    pub transfer: vk::Queue,
    pub transfer_family: usize,
}

impl QueueFamilies {
    pub unsafe fn new(
        instance: &ash::Instance,
        surface_loader: &Surface,
        surface: vk::SurfaceKHR,
        pdevice: vk::PhysicalDevice,
    ) -> Result<Self, ContextError> {
        let mut queue_families = Self {
            present: None,
            graphics: None,
            transfer: None,
        };

        let queue_family_properties = instance.get_physical_device_queue_family_properties(pdevice);

        for (index, queue_family_property) in queue_family_properties.into_iter().enumerate() {
            let present_support = surface_loader
                .get_physical_device_surface_support(pdevice, index as u32, surface)
                .map_err_log(
                    "Physical device surface support query failed",
                    ContextError::OutOfMemory,
                )?;

            let graphics_support = queue_family_property
                .queue_flags
                .contains(vk::QueueFlags::GRAPHICS);

            /* let transfer_support = queue_family_property
            .queue_flags
            .contains(vk::QueueFlags::TRANSFER); */

            if present_support && queue_families.present.is_none() {
                queue_families.present = Some(index);
            }
            if graphics_support && queue_families.graphics.is_none() {
                queue_families.graphics = Some(index);
            }
            /* if transfer_support && queue_families.transfer.is_none() {
                queue_families.transfer = Some(index);
            } */

            if queue_families.finished() {
                break;
            }
        }

        Ok(queue_families)
    }

    pub fn finished(&self) -> bool {
        self.present.is_some() && self.graphics.is_some() /* && self.transfer.is_some() */
    }

    pub fn same(&self) -> Option<bool> {
        Some(
            self.present? == self.graphics?, /* && self.graphics? == self.transfer? */
        )
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

    pub unsafe fn get_queues(&self, device: &ash::Device) -> Option<Queues> {
        if !self.finished() {
            None
        } else {
            let present_family = self.present.unwrap();
            let present = device.get_device_queue(present_family as u32, 0);

            let graphics_family = self.graphics.unwrap();
            let graphics = device.get_device_queue(graphics_family as u32, 0);

            let transfer_family = graphics_family; //
            let transfer = device.get_device_queue(transfer_family as u32, 0);

            Some(Queues {
                present_family,
                present,
                graphics_family,
                graphics,
                transfer_family,
                transfer,
            })
        }
    }
}
