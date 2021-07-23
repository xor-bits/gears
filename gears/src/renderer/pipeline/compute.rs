use std::marker::PhantomData;

use ash::vk;

use crate::{renderer::device::Dev, ImmediateFrameInfo, Module, PipelineBase, Uniform};

pub struct ComputePipeline<Uf> {
    base: PipelineBase,
    _p: PhantomData<Uf>,
}

impl<Uf> ComputePipeline<Uf> {
    pub fn new(device: Dev, render_pass: vk::RenderPass, comp: Module<Uf>, debug: bool) -> Self {
        todo!()
    }
}

impl<Uf> ComputePipeline<Uf>
where
    Uf: Uniform,
{
    pub fn write_uniform(&self, imfi: &ImmediateFrameInfo, data: &Uf) {}
}

impl<Uf> Drop for ComputePipeline<Uf> {
    fn drop(&mut self) {}
}
