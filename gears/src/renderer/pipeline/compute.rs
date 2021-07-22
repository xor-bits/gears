use std::{marker::PhantomData, sync::Arc};

use ash::vk;

use crate::{renderer::device::RenderDevice, Module, PipelineBase};

pub struct ComputePipeline<Uf> {
    base: PipelineBase,
    _p: PhantomData<Uf>,
}

impl<Uf> ComputePipeline<Uf> {
    pub fn new(
        device: Arc<RenderDevice>,
        render_pass: vk::RenderPass,
        comp: Module<Uf>,
        debug: bool,
    ) -> Self {
        Self {
            base: PipelineBase { device },
            _p: PhantomData {},
        }
    }
}

impl<Uf> Drop for ComputePipeline<Uf> {
    fn drop(&mut self) {}
}
