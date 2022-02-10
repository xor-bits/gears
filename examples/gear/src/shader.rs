use gears::{
    gears_pipeline::Input,
    glam::{Mat4, Vec3},
    renderer::simple_renderer::Renderer,
    vulkano::{buffer::CpuBufferPool, pipeline::GraphicsPipeline, render_pass::Subpass},
};
use std::sync::Arc;
use vulkano::{
    descriptor_set::pool::StdDescriptorPool,
    pipeline::graphics::{
        depth_stencil::DepthStencilState, input_assembly::InputAssemblyState,
        vertex_input::BuffersDefinition, viewport::ViewportState,
    },
};

#[derive(Input, Debug, PartialEq, Copy, Clone, Default)]
#[repr(C)]
pub struct VertexData {
    pub vi_pos: [f32; 3],
    pub vi_norm: [f32; 3],
}

#[derive(Debug, PartialEq, Copy, Clone, Default)]
#[repr(C)]
pub struct UniformData {
    pub model_matrix: Mat4,
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
    pub light_dir: Vec3,
}

mod vert {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "gear/res/default.vert.glsl"
    }
}

mod frag {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "gear/res/default.frag.glsl"
    }
}

pub struct DefaultPipeline {
    pub pipeline: Arc<GraphicsPipeline>,
    pub desc_pool: Arc<StdDescriptorPool>,
    pub buffer_pool: CpuBufferPool<UniformData>,
}

impl DefaultPipeline {
    pub fn build(renderer: &Renderer) -> Self {
        let vert = vert::load(renderer.device.logical().clone()).unwrap();
        let frag = frag::load(renderer.device.logical().clone()).unwrap();

        let pipeline = GraphicsPipeline::start()
            //
            .input_assembly_state(InputAssemblyState::new())
            //
            .vertex_input_state(BuffersDefinition::new().vertex::<VertexData>())
            .vertex_shader(vert.entry_point("main").unwrap(), ())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            //
            .fragment_shader(frag.entry_point("main").unwrap(), ())
            .depth_stencil_state(DepthStencilState::simple_depth_test())
            //
            .render_pass(Subpass::from(renderer.render_pass(), 0).unwrap())
            //
            .build(renderer.device.logical().clone())
            .unwrap();

        let desc_pool = Arc::new(StdDescriptorPool::new(renderer.device.logical().clone()));
        let buffer_pool =
            CpuBufferPool::<UniformData>::uniform_buffer(renderer.device.logical().clone());

        Self {
            pipeline,
            buffer_pool,
            desc_pool,
        }
    }
}

/* TODO: pipeline! {
    "DefaultPipeline"
    VertexData -> RGBAOutput
    mod "VERT" as "vert" where { in UniformData as 0 }
    mod "FRAG" as "frag"
} */
