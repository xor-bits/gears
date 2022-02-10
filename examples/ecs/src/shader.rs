use gears::{
    gears_pipeline::Input,
    glam::Mat4,
    renderer::simple_renderer::Renderer,
    vulkano::{
        buffer::CpuBufferPool,
        pipeline::{
            graphics::{
                input_assembly::InputAssemblyState, vertex_input::BuffersDefinition,
                viewport::ViewportState,
            },
            GraphicsPipeline,
        },
        render_pass::Subpass,
    },
};
use std::sync::Arc;
use vulkano::{descriptor_set::SingleLayoutDescSetPool, pipeline::Pipeline};

//

#[derive(Input, Debug, PartialEq, Copy, Clone, Default)]
#[repr(C)]
pub struct VertexData {
    pub pos: [f32; 2],
}

#[derive(Debug, PartialEq, Copy, Clone, Default)]
#[repr(C)]
pub struct UniformData {
    pub mvp: Mat4,
}

mod vert {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "ecs/res/vert.glsl"
    }
}

mod frag {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "ecs/res/frag.glsl"
    }
}

pub struct DefaultPipeline {
    pub pipeline: Arc<GraphicsPipeline>,
    pub desc_pool: SingleLayoutDescSetPool,
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
            //
            .render_pass(Subpass::from(renderer.render_pass(), 0).unwrap())
            //
            .build(renderer.device.logical().clone())
            .unwrap();

        let layout = pipeline.layout().descriptor_set_layouts()[0].clone();
        let desc_pool = SingleLayoutDescSetPool::new(layout);
        let buffer_pool =
            CpuBufferPool::<UniformData>::uniform_buffer(renderer.device.logical().clone());

        Self {
            pipeline,
            desc_pool,
            buffer_pool,
        }
    }
}

/* pipeline! {
    "DefaultPipeline"
    VertexData -> RGBAOutput
    mod "VERT" as "vert" where { in UniformData as 0 }
    mod "FRAG" as "frag"
} */
