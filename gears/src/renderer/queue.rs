use crate::context::ContextError;
use colored::Colorize;
use std::sync::Arc;
use vulkano::{
    device::{
        physical::{PhysicalDevice, QueueFamily},
        Queue, QueuesIter,
    },
    swapchain::Surface,
};
use winit::window::Window;

pub struct QueueFamilies<'a> {
    pub present: QueueFamily<'a>,
    pub graphics: QueueFamily<'a>,
    /* pub transfer: QueueFamily<'a>, */
}

pub struct Queues {
    pub present: Arc<Queue>,
    pub graphics: Arc<Queue>,
    /* pub transfer: Arc<Queue>, */
}

impl<'a> QueueFamilies<'a> {
    pub fn new(
        surface: &Arc<Surface<Window>>,
        p_device: PhysicalDevice<'a>,
    ) -> Result<Option<Self>, ContextError> {
        let mut present = None;
        let mut graphics = None;
        /* let mut transfer = None; */

        let queue_family_properties = p_device.queue_families();

        for queue_family_property in queue_family_properties {
            let present_support = surface
                .is_supported(queue_family_property)
                .map_err(ContextError::CapabilitiesError)?;

            let graphics_support = queue_family_property.supports_graphics();
            /* let transfer_support = queue_family_property.explicitly_supports_transfers(); */

            if present_support && present.is_none() {
                present = Some(queue_family_property);
            }
            if graphics_support && graphics.is_none() {
                graphics = Some(queue_family_property);
            }
            /* if transfer_support && transfer.is_none() {
                transfer = Some(queue_family_property);
            } */

            if let (Some(present), Some(graphics) /* , Some(transfer) */) =
                (present, graphics /* , transfer */)
            {
                return Ok(Some(Self {
                    present,
                    graphics,
                    /* transfer, */
                }));
            }
        }

        log::debug!(
            "{} is not suitable: (present, graphics) = {:?}",
            p_device.properties().device_name.blue(),
            (present.map(|v| v.id()), graphics.map(|v| v.id()))
        );

        Ok(None)
    }

    pub fn get(&self) -> Vec<(QueueFamily<'_>, f32)> {
        if self.present == self.graphics {
            vec![(self.present, 1.0)]
        } else {
            vec![(self.present, 1.0), (self.graphics, 1.0)]
        }
    }

    pub fn get_queues(&self, mut queue_iter: QueuesIter) -> Queues {
        let present = queue_iter.next().expect("Missing queue");
        if self.present == self.graphics {
            Queues {
                present: present.clone(),
                graphics: present,
            }
        } else {
            let graphics = queue_iter.next().expect("Missing queue");
            Queues { present, graphics }
        }
    }
}
