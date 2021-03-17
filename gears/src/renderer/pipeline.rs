pub mod vertex;

pub use vertex::*;

use gears_traits::Vertex;
use std::{any::TypeId, collections::HashMap, io::Cursor, iter, mem};

use gfx_hal::{
    adapter::MemoryType,
    buffer::SubRange,
    command::CommandBuffer,
    device::Device,
    pass::Subpass,
    pso::{
        AttributeDesc, BlendState, BufferDescriptorFormat, BufferDescriptorType, ColorBlendDesc,
        ColorMask, Descriptor, DescriptorPool, DescriptorPoolCreateFlags, DescriptorRangeDesc,
        DescriptorSetLayoutBinding, DescriptorSetWrite, DescriptorType, EntryPoint,
        GraphicsPipelineDesc, InputAssemblerDesc, Primitive, PrimitiveAssemblerDesc, Rasterizer,
        ShaderStageFlags, Specialization, VertexBufferDesc,
    },
    Backend,
};

use super::buffer::UniformBuffer;

pub struct PipelineBuilder<'a, B: Backend> {
    device: &'a B::Device,
    render_pass: &'a B::RenderPass,
    available_memory_types: &'a Vec<MemoryType>,

    vert_spirv: Option<&'a [u8]>,
    frag_spirv: Option<&'a [u8]>,

    vert_input_binding: Vec<VertexBufferDesc>,
    vert_input_attribute: Vec<AttributeDesc>,

    ubos: HashMap<TypeId, (ShaderStageFlags, usize)>,
}

pub struct Pipeline<B: Backend> {
    desc_pool: Option<B::DescriptorPool>,
    pipeline: B::GraphicsPipeline,
}

impl<'a, B: Backend> PipelineBuilder<'a, B> {
    pub fn new(
        device: &'a B::Device,
        render_pass: &'a B::RenderPass,
        available_memory_types: &'a Vec<MemoryType>,
    ) -> Self {
        Self {
            device,
            render_pass,
            available_memory_types,

            vert_spirv: None,
            frag_spirv: None,

            vert_input_binding: Vec::new(),
            vert_input_attribute: Vec::new(),

            ubos: HashMap::new(),
        }
    }

    pub fn with_module_vert(mut self, vert_spirv: &'a [u8]) -> Self {
        self.vert_spirv = Some(vert_spirv);
        self
    }

    pub fn with_module_frag(mut self, frag_spirv: &'a [u8]) -> Self {
        self.frag_spirv = Some(frag_spirv);
        self
    }

    pub fn with_input<V: Vertex>(mut self) -> Self {
        self.vert_input_binding = V::binding_desc();
        self.vert_input_attribute = V::attribute_desc();
        self
    }

    pub fn with_ubo<U: 'static + UBO>(mut self) -> Self {
        self.ubos
            .insert(TypeId::of::<U>(), (U::STAGE, mem::size_of::<U>()));
        self
    }

    pub fn build(self) -> Pipeline<B> {
        let vert_module = {
            let spirv = gfx_auxil::read_spirv(Cursor::new(&self.vert_spirv.unwrap()[..])).unwrap();
            unsafe { self.device.create_shader_module(&spirv) }
                .expect("Could not create a vertex shader module")
        };
        let vert_entry = EntryPoint {
            entry: "main",
            module: &vert_module,
            specialization: Specialization::default(),
        };
        let frag_module = {
            let spirv = gfx_auxil::read_spirv(Cursor::new(&self.frag_spirv.unwrap()[..])).unwrap();
            unsafe { self.device.create_shader_module(&spirv) }
                .expect("Could not create a fragment shader module")
        };
        let frag_entry = EntryPoint {
            entry: "main",
            module: &frag_module,
            specialization: Specialization::default(),
        };

        let bindings = self
            .ubos
            .iter()
            .map(|ubo| DescriptorSetLayoutBinding {
                binding: 0,
                ty: DescriptorType::Buffer {
                    format: BufferDescriptorFormat::Structured {
                        dynamic_offset: false,
                    },
                    ty: BufferDescriptorType::Uniform,
                },
                count: 1,
                stage_flags: ubo.1 .0,
                immutable_samplers: false,
            })
            .collect::<Vec<_>>();

        let descriptor_ranges = self.ubos.iter().map(|ubo| DescriptorRangeDesc {
            ty: DescriptorType::Buffer {
                format: BufferDescriptorFormat::Structured {
                    dynamic_offset: false,
                },
                ty: BufferDescriptorType::Uniform,
            },
            count: 1,
        });

        let set_layout = unsafe {
            self.device
                .create_descriptor_set_layout(bindings.into_iter(), iter::empty())
        }
        .expect("Could not create a descriptor set layout");

        let desc_pool = if descriptor_ranges.len() > 0 {
            let mut desc_pool = unsafe {
                self.device.create_descriptor_pool(
                    1,
                    descriptor_ranges.into_iter(),
                    DescriptorPoolCreateFlags::empty(),
                )
            }
            .expect("Could not create a descriptor pool");
            let mut desc_set = unsafe { desc_pool.allocate_one(&set_layout) }.unwrap();

            Some(desc_pool)
        } else {
            None
        };

        /* let uniform_buffer = UniformBuffer::<B>::new(self.device, self.available_memory_types, 1);

        unsafe {
            self.device.write_descriptor_set(DescriptorSetWrite {
                set: &mut desc_set,
                descriptors: iter::once(Descriptor::Buffer(uniform_buffer.get(), SubRange::WHOLE)),
                array_offset: 0,
                binding: 0,
            });
        } */

        /* gears::renderer::pipeline::UBO; */

        let pipeline_layout = unsafe {
            self.device
                .create_pipeline_layout(iter::once(&set_layout), iter::empty())
        }
        .expect("Could not create a pipeline layout");

        let subpass = Subpass {
            index: 0,
            main_pass: self.render_pass,
        };

        let mut pipeline_desc = GraphicsPipelineDesc::new(
            PrimitiveAssemblerDesc::Vertex {
                buffers: &self.vert_input_binding[..],
                attributes: &self.vert_input_attribute[..],
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

        let pipeline = unsafe { self.device.create_graphics_pipeline(&pipeline_desc, None) };

        unsafe {
            self.device.destroy_pipeline_layout(pipeline_layout);
            self.device.destroy_descriptor_set_layout(set_layout);
            self.device.destroy_shader_module(frag_module);
            self.device.destroy_shader_module(vert_module);
        }

        let pipeline = pipeline.expect("Could not create a graphics pipeline");

        Pipeline::<B> {
            desc_pool,
            pipeline,
        }
    }
}

impl<B: Backend> Pipeline<B> {
    pub fn bind(&self, command_buffer: &mut B::CommandBuffer) {
        unsafe { command_buffer.bind_graphics_pipeline(&self.pipeline) };
    }

    pub fn destroy(self, device: &B::Device) {
        unsafe {
            if let Some(desc_pool) = self.desc_pool {
                device.destroy_descriptor_pool(desc_pool);
            }
            device.destroy_graphics_pipeline(self.pipeline);
        }
    }
}
