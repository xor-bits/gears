use std::cmp::Ordering;

use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};

#[derive(Debug, Clone, Copy, Eq)]
pub struct GPUScore {
    type_score: usize,
    pub memory: u64,
}

impl GPUScore {
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
            .sum();

        Self { type_score, memory }
    }

    pub fn score(&self) -> u128 {
        self.type_score as u128 * self.memory as u128
    }
}

impl Ord for GPUScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score().cmp(&other.score())
    }
}

impl PartialOrd for GPUScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for GPUScore {
    fn eq(&self, other: &Self) -> bool {
        self.score() == other.score()
    }
}
