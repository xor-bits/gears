use std::{io::Cursor, iter};

use gfx_hal::{
    device::Device,
    pass::Subpass,
    pso::{
        BlendState, ColorBlendDesc, ColorMask, EntryPoint, GraphicsPipelineDesc,
        InputAssemblerDesc, Primitive, PrimitiveAssemblerDesc, Rasterizer, Specialization,
    },
    Backend,
};

use crate::log::LogWrap;

pub fn create_pipeline<B: Backend>(
    device: &B::Device,
    render_pass: &B::RenderPass,
) -> B::GraphicsPipeline {
    let vert_module = {
        let spirv =
            gfx_auxil::read_spirv(Cursor::new(include_bytes!("shader/vert.glsl.spv"))).unwrap_log();
        unsafe { device.create_shader_module(&spirv) }
            .expect_log("Could not create a vertex shader module")
    };
    let vert_entry = EntryPoint {
        entry: "main",
        module: &vert_module,
        specialization: Specialization::default(),
    };
    let frag_module = {
        let spirv =
            gfx_auxil::read_spirv(Cursor::new(include_bytes!("shader/frag.glsl.spv"))).unwrap_log();
        unsafe { device.create_shader_module(&spirv) }
            .expect_log("Could not create a fragment shader module")
    };
    let frag_entry = EntryPoint {
        entry: "main",
        module: &frag_module,
        specialization: Specialization::default(),
    };

    let pipeline_layout = unsafe { device.create_pipeline_layout(iter::empty(), iter::empty()) }
        .expect_log("Could not create a pipeline layout");

    let subpass = Subpass {
        index: 0,
        main_pass: render_pass,
    };

    let mut pipeline_desc = GraphicsPipelineDesc::new(
        PrimitiveAssemblerDesc::Vertex {
            buffers: &[],    // &[vertex_buffers],
            attributes: &[], // &attributes,
            input_assembler: InputAssemblerDesc {
                primitive: Primitive::TriangleList,
                with_adjacency: false,
                restart_index: None,
            },
            vertex: vert_entry,
            geometry: None,
            tessellation: None,
        },
        Rasterizer::FILL,
        Some(frag_entry),
        &pipeline_layout,
        subpass,
    );

    pipeline_desc.blender.targets.push(ColorBlendDesc {
        mask: ColorMask::ALL,
        blend: Some(BlendState::ALPHA),
    });

    let pipeline = unsafe { device.create_graphics_pipeline(&pipeline_desc, None) };

    unsafe {
        device.destroy_pipeline_layout(pipeline_layout);
        device.destroy_shader_module(frag_module);
        device.destroy_shader_module(vert_module);
    }

    pipeline.expect_log("Could not create a graphics pipeline")
}
