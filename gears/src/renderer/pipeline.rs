pub mod vertex;

pub use vertex::{Vertex, VertexData};

use super::buffer::UniformBuffer;

use std::{io::Cursor, iter};

use gfx_hal::{
    adapter::MemoryType,
    buffer::SubRange,
    command::CommandBuffer,
    device::Device,
    pass::Subpass,
    pso::{
        BlendState, BufferDescriptorFormat, BufferDescriptorType, ColorBlendDesc, ColorMask,
        Descriptor, DescriptorPool, DescriptorPoolCreateFlags, DescriptorRangeDesc,
        DescriptorSetLayoutBinding, DescriptorSetWrite, DescriptorType, EntryPoint,
        GraphicsPipelineDesc, InputAssemblerDesc, Primitive, PrimitiveAssemblerDesc, Rasterizer,
        ShaderStageFlags, Specialization,
    },
    Backend,
};

pub struct Pipeline<B: Backend> {
    desc_set: B::DescriptorSet,
    desc_pool: B::DescriptorPool,
    pipeline: B::GraphicsPipeline,
}

impl<B: Backend> Pipeline<B> {
    pub fn new<V: Vertex>(
        device: &B::Device,
        render_pass: &B::RenderPass,
        available_memory_types: &Vec<MemoryType>,
    ) -> Self {
        mod default_pipeline {
            gears_pipeline::pipeline! {
                vs: { path: "res/default.glsl" }
                fs: { path: "res/default.glsl" }
            }
        }
        let vert_module = {
            let spirv =
                gfx_auxil::read_spirv(Cursor::new(&default_pipeline::VERT_SPIRV[..])).unwrap();
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
                gfx_auxil::read_spirv(Cursor::new(&default_pipeline::FRAG_SPIRV[..])).unwrap();
            unsafe { device.create_shader_module(&spirv) }
                .expect("Could not create a fragment shader module")
        };
        let frag_entry = EntryPoint {
            entry: "main",
            module: &frag_module,
            specialization: Specialization::default(),
        };

        let set_layout = unsafe {
            device.create_descriptor_set_layout(
                vec![DescriptorSetLayoutBinding {
                    binding: 0,
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
                1,
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
        let mut desc_set = unsafe { desc_pool.allocate_one(&set_layout) }.unwrap();

        let uniform_buffer =
            UniformBuffer::<B>::new::<default_pipeline::UBO>(device, available_memory_types, 1);

        unsafe {
            device.write_descriptor_set(DescriptorSetWrite {
                set: &mut desc_set,
                descriptors: iter::once(Descriptor::Buffer(uniform_buffer.get(), SubRange::WHOLE)),
                array_offset: 0,
                binding: 0,
            });
        }

        let pipeline_layout =
            unsafe { device.create_pipeline_layout(iter::once(&set_layout), iter::empty()) }
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
            device.destroy_descriptor_set_layout(set_layout);
            device.destroy_shader_module(frag_module);
            device.destroy_shader_module(vert_module);
        }

        let pipeline = pipeline.expect("Could not create a graphics pipeline");

        Self {
            desc_set,
            desc_pool,
            pipeline,
        }
    }

    pub fn bind(&self, command_buffer: &mut B::CommandBuffer) {
        unsafe { command_buffer.bind_graphics_pipeline(&self.pipeline) };
    }

    pub fn destroy(self, device: &B::Device) {
        unsafe {
            device.destroy_descriptor_pool(self.desc_pool);
            device.destroy_graphics_pipeline(self.pipeline);
        }
    }
}
