use std::{io::Cursor, iter};

use cgmath::{Vector2, Vector3};
use gfx_hal::{
    device::Device,
    format::Format,
    pass::Subpass,
    pso::{
        AttributeDesc, BlendState, ColorBlendDesc, ColorMask, Element, EntryPoint,
        GraphicsPipelineDesc, InputAssemblerDesc, Primitive, PrimitiveAssemblerDesc, Rasterizer,
        Specialization, VertexBufferDesc, VertexInputRate,
    },
    Backend,
};

pub trait Vertex /* <const N: usize> */ {
    // const generics not yet stable
    fn binding_desc() -> Vec<VertexBufferDesc>;
    fn attribute_desc() -> Vec<AttributeDesc>;
}

pub struct VertexData {
    pub position: Vector2<f32>,
    pub color: Vector3<f32>,
}

impl Vertex for VertexData {
    fn binding_desc() -> Vec<VertexBufferDesc> {
        vec![VertexBufferDesc {
            binding: 0,
            rate: VertexInputRate::Vertex,
            stride: std::mem::size_of::<VertexData>() as u32,
        }]
    }

    fn attribute_desc() -> Vec<AttributeDesc> {
        vec![
            AttributeDesc {
                binding: 0,
                location: 0,
                element: Element {
                    format: Format::Rg32Sfloat,
                    offset: 0,
                },
            },
            AttributeDesc {
                binding: 0,
                location: 1,
                element: Element {
                    format: Format::Rgb32Sfloat,
                    offset: 4 * 2,
                },
            },
        ]
    }
}

pub fn create_pipeline<B: Backend, V: Vertex>(
    device: &B::Device,
    render_pass: &B::RenderPass,
) -> B::GraphicsPipeline {
    mod default_pipeline {
        gears_pipeline::pipeline! {
            vs: { path: "res/default.glsl" }
            fs: { path: "res/default.glsl" }
        }
    }
    let vert_module = {
        let spirv =
            gfx_auxil::read_spirv(Cursor::new(&default_pipeline::VERTEX_SPIRV[..])).unwrap();
        unsafe { device.create_shader_module(&spirv) }
            .expect("Could not create a vertex shader module")
    };
    let vert_entry = EntryPoint {
        entry: "main",
        module: &vert_module,
        specialization: Specialization::default(),
    };
    let frag_module = {
        let spirv =
            gfx_auxil::read_spirv(Cursor::new(&default_pipeline::FRAGMENT_SPIRV[..])).unwrap();
        unsafe { device.create_shader_module(&spirv) }
            .expect("Could not create a fragment shader module")
    };
    let frag_entry = EntryPoint {
        entry: "main",
        module: &frag_module,
        specialization: Specialization::default(),
    };

    /* let set_layout = unsafe {
        device.create_descriptor_set_layout(
            vec![DescriptorSetLayoutBinding {
                binding: 1,
                ty: DescriptorType::Buffer {
                    format: BufferDescriptorFormat::Structured {
                        dynamic_offset: false,
                    },
                    ty: BufferDescriptorType::Uniform,
                },
                count: 1,
                stage_flags: ShaderStageFlags::VERTEX,
                immutable_samplers: false,
            }]
            .into_iter(),
            iter::empty(),
        )
    }
    .expect("Could not create a descriptor set layout");

    let mut desc_pool = unsafe {
        device.create_descriptor_pool(
            1, // sets
            vec![DescriptorRangeDesc {
                ty: DescriptorType::Buffer {
                    format: BufferDescriptorFormat::Structured {
                        dynamic_offset: false,
                    },
                    ty: BufferDescriptorType::Uniform,
                },
                count: 1,
            }]
            .into_iter(),
            DescriptorPoolCreateFlags::empty(),
        )
    }
    .expect("Could not create a descriptor pool");
    let mut desc_set = unsafe { desc_pool.allocate_one(&set_layout) }.unwrap(); */

    let pipeline_layout = unsafe {
        device.create_pipeline_layout(
            iter::empty(), /* iter::once(&set_layout) */
            iter::empty(),
        )
    }
    .expect("Could not create a pipeline layout");

    let subpass = Subpass {
        index: 0,
        main_pass: render_pass,
    };

    let buffers = V::binding_desc();
    let attributes = V::attribute_desc();
    let mut pipeline_desc = GraphicsPipelineDesc::new(
        PrimitiveAssemblerDesc::Vertex {
            buffers: &buffers[..],       // &[vertex_buffers],
            attributes: &attributes[..], // &attributes,
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

    pipeline.expect("Could not create a graphics pipeline")
}
