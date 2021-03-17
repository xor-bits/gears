use gears_traits::{Vertex, UBO};
use std::{
    any::{type_name, TypeId},
    collections::HashMap,
    io::Cursor,
    iter, mem,
};

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

use super::buffer::{Buffer, UniformBuffer};

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

    desc_set_layout: B::DescriptorSetLayout,
    desc_sets: Vec<B::DescriptorSet>,

    pipeline_layout: B::PipelineLayout,
    pipeline: B::GraphicsPipeline,

    ubos: HashMap<TypeId, Vec<UniformBuffer<B>>>,
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

    pub fn build(self, set_count: usize) -> Pipeline<B> {
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

        let descriptor_ranges = self.ubos.iter().map(|_| DescriptorRangeDesc {
            ty: DescriptorType::Buffer {
                format: BufferDescriptorFormat::Structured {
                    dynamic_offset: false,
                },
                ty: BufferDescriptorType::Uniform,
            },
            count: 1,
        });

        let desc_set_layout = unsafe {
            self.device
                .create_descriptor_set_layout(bindings.into_iter(), iter::empty())
        }
        .expect("Could not create a descriptor set layout");

        let (desc_pool, desc_sets) = if descriptor_ranges.len() > 0 {
            let mut desc_pool = unsafe {
                self.device.create_descriptor_pool(
                    set_count,
                    descriptor_ranges.into_iter(),
                    DescriptorPoolCreateFlags::empty(),
                )
            }
            .expect("Could not create a descriptor pool");

            let desc_sets = (0..set_count)
                .into_iter()
                .map(|_| unsafe { desc_pool.allocate_one(&desc_set_layout) }.unwrap())
                .collect();

            (Some(desc_pool), desc_sets)
        } else {
            (None, Vec::new())
        };

        let ubos = {
            let device = self.device;
            let available_memory_types = self.available_memory_types;
            self.ubos
                .into_iter()
                .map(|ubo| {
                    (
                        ubo.0.clone(),
                        (0..set_count)
                            .into_iter()
                            .map(|_| {
                                UniformBuffer::<B>::new(device, available_memory_types, ubo.1 .1)
                            })
                            .collect(),
                    )
                })
                .collect::<HashMap<_, _>>()
        };

        let pipeline_layout = unsafe {
            self.device
                .create_pipeline_layout(iter::once(&desc_set_layout), iter::empty())
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
            self.device.destroy_shader_module(frag_module);
            self.device.destroy_shader_module(vert_module);
        }

        let pipeline = pipeline.expect("Could not create a graphics pipeline");

        Pipeline::<B> {
            desc_pool,

            desc_sets,
            desc_set_layout,

            pipeline_layout,
            pipeline,

            ubos,
        }
    }
}

impl<B: Backend> Pipeline<B> {
    pub fn bind(&self, command_buffer: &mut B::CommandBuffer, set_index: usize) {
        unsafe {
            command_buffer.bind_graphics_pipeline(&self.pipeline);

            if let Some(desc_set) = self.desc_sets.get(set_index) {
                command_buffer.bind_graphics_descriptor_sets(
                    &self.pipeline_layout,
                    0,
                    iter::once(desc_set),
                    iter::empty(),
                );
            }
        }
    }

    pub fn write_ubo<U: 'static + UBO>(
        &mut self,
        device: &B::Device,
        new_data: U,
        set_index: usize,
    ) {
        if let Some(ubo_set) = self.ubos.get_mut(&TypeId::of::<U>()) {
            let ubo = &mut ubo_set[set_index];
            let set = &mut self.desc_sets[set_index];
            ubo.write(device, 0, &[new_data]);

            unsafe {
                device.write_descriptor_set(DescriptorSetWrite {
                    set,
                    descriptors: iter::once(Descriptor::Buffer(ubo.get(), SubRange::WHOLE)),
                    array_offset: 0,
                    binding: 0,
                });
            }
        } else {
            panic!(
                "Type {:?} is not an UBO for tihs pipeline",
                type_name::<U>()
            );
        }
    }

    pub fn destroy(self, device: &B::Device) {
        unsafe {
            for (_, ubo_set) in self.ubos {
                for ubo in ubo_set {
                    ubo.destroy(device);
                }
            }

            device.destroy_pipeline_layout(self.pipeline_layout);
            device.destroy_graphics_pipeline(self.pipeline);
            device.destroy_descriptor_set_layout(self.desc_set_layout);
            if let Some(desc_pool) = self.desc_pool {
                device.destroy_descriptor_pool(desc_pool);
            }
        }
    }
}
