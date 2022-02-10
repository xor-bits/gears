use super::{any::AnyGPU, score::GPUScore};
use std::{cmp::Ordering, sync::Arc};
use vulkano::{device::physical::PhysicalDevice, instance::Instance};

#[derive(Debug, Clone, Eq)]
pub struct SuitableGPU {
    pub p_device: usize,
    pub instance: Arc<Instance>,
    pub score: GPUScore,
}

impl AnyGPU for SuitableGPU {
    fn score(&self) -> &'_ GPUScore {
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

impl Ord for SuitableGPU {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score().cmp(other.score())
    }
}

impl PartialOrd for SuitableGPU {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for SuitableGPU {
    fn eq(&self, other: &Self) -> bool {
        self.score() == other.score()
    }
}
