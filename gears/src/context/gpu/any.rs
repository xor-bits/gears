use vulkano::device::physical::PhysicalDevice;

use super::{
    score::GPUScore, suitable::SuitableGPU,
    unsuitable::UnsuitableGPU,
};

pub trait AnyGPU {
    fn score(&self) -> &'_ GPUScore;
    fn device(&self) -> PhysicalDevice<'_>;
    fn suitable(&self) -> bool;
    fn name(&self) -> &'_ String;
}

#[derive(Debug, Clone)]
pub enum GPUPicker {
    Suitable(SuitableGPU),
    Unsuitable(UnsuitableGPU),
}

impl GPUPicker {
    fn get_internal(&self) -> &dyn AnyGPU {
        match self {
            GPUPicker::Suitable(d) => d,
            GPUPicker::Unsuitable(d) => d,
        }
    }
}

impl AnyGPU for GPUPicker {
    fn score(&self) -> &'_ GPUScore {
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
