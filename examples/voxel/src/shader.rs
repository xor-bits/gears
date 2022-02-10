use std::sync::Arc;

use gears::{gears_pipeline::Input, glam::Mat4, renderer::simple_renderer::Renderer};
use vulkano::{
    buffer::CpuBufferPool,
    descriptor_set::pool::StdDescriptorPool,
    pipeline::{
        graphics::{
            depth_stencil::DepthStencilState, input_assembly::InputAssemblyState,
            vertex_input::BuffersDefinition, viewport::ViewportState,
        },
        GraphicsPipeline,
    },
    render_pass::Subpass,
};

//

#[derive(Debug, Input, PartialEq, Default)]
#[repr(C)]
pub struct VertexData {
    pub vi_pos: [f32; 3],
    pub vi_exp: f32,
}

#[derive(Debug, PartialEq, Default)]
#[repr(C)]
pub struct UniformData {
    pub mvp: Mat4,
}

//

mod vert {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "voxel/res/default.vert.glsl"
    }
}

/* mod geom {
    vulkano_shaders::shader! {
        ty: "geometry",
        path: "voxel/res/default.geom.glsl"
    }
} */

mod frag {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "voxel/res/default.frag.glsl"
    }
}

mod debug_frag {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "voxel/res/default.frag.glsl",
        define: [("DEBUGGING", "")]
    }
}

//

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

//

pub struct DebugPipeline {
    pub pipeline: Arc<GraphicsPipeline>,
    pub desc_pool: Arc<StdDescriptorPool>,
    pub buffer_pool: CpuBufferPool<UniformData>,
}

impl DebugPipeline {
    pub fn build(renderer: &Renderer) -> Self {
        let vert = vert::load(renderer.device.logical().clone()).unwrap();
        // let geom = geom::
        let frag = debug_frag::load(renderer.device.logical().clone()).unwrap();

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
