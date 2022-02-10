use super::{any::AnyGPU, score::GPUScore};
use std::{cmp::Ordering, sync::Arc};
use vulkano::{device::physical::PhysicalDevice, instance::Instance};

#[derive(Debug, Clone, Eq)]
pub struct UnsuitableGPU {
    pub p_device: usize,
    pub instance: Arc<Instance>,
    pub score: GPUScore,
}

impl AnyGPU for UnsuitableGPU {
    fn score(&self) -> &'_ GPUScore {
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

impl Ord for UnsuitableGPU {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score().cmp(other.score())
    }
}

impl PartialOrd for UnsuitableGPU {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for UnsuitableGPU {
    fn eq(&self, other: &Self) -> bool {
        self.score() == other.score()
    }
}
