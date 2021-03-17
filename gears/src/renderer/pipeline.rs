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
        DescriptorSetLayoutBinding, DescriptorSetWrite, DescriptorType, EntryPoint, Face,
        FrontFace, GraphicsPipelineDesc, InputAssemblerDesc, PolygonMode, Primitive,
        PrimitiveAssemblerDesc, Rasterizer, ShaderStageFlags, Specialization, State,
        VertexBufferDesc,
    },
    Backend,
};

use super::buffer::{Buffer, UniformBuffer};

pub struct PipelineBuilder<'a, B: Backend> {
    device: &'a B::Device,
    render_pass: &'a B::RenderPass,
    available_memory_types: &'a Vec<MemoryType>,
    set_count: usize,

    vert_spirv: Option<&'a [u8]>,
    frag_spirv: Option<&'a [u8]>,

    vert_input_binding: Vec<VertexBufferDesc>,
    vert_input_attribute: Vec<AttributeDesc>,

    ubos: HashMap<TypeId, (ShaderStageFlags, Vec<UniformBuffer<B>>)>,
}

pub struct Pipeline<B: Backend> {
    desc_pool: Option<B::DescriptorPool>,

    desc_set_layout: B::DescriptorSetLayout,
    desc_sets: Vec<(B::DescriptorSet, HashMap<TypeId, UniformBuffer<B>>)>,

    pipeline_layout: B::PipelineLayout,
    pipeline: B::GraphicsPipeline,
}

impl<'a, B: Backend> PipelineBuilder<'a, B> {
    pub fn new(
        device: &'a B::Device,
        render_pass: &'a B::RenderPass,
        available_memory_types: &'a Vec<MemoryType>,
        set_count: usize,
    ) -> Self {
        Self {
            device,
            render_pass,
            available_memory_types,
            set_count,

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

    pub fn with_ubo<U: 'static + UBO + Default>(mut self) -> Self {
        let size = mem::size_of::<U>();

        let buffers = (0..self.set_count)
            .map(|_| {
                let mut ubo =
                    UniformBuffer::<B>::new(self.device, self.available_memory_types, size);
                ubo.write(self.device, 0, &[U::default()]);
                ubo
            })
            .collect();

        self.ubos.insert(TypeId::of::<U>(), (U::STAGE, buffers));

        self
    }

    pub fn build(mut self) -> Pipeline<B> {
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
            .map(|(_, (stage, _))| DescriptorSetLayoutBinding {
                binding: 0,
                ty: DescriptorType::Buffer {
                    format: BufferDescriptorFormat::Structured {
                        dynamic_offset: false,
                    },
                    ty: BufferDescriptorType::Uniform,
                },
                count: 1,
                stage_flags: stage.clone(),
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
                    self.set_count,
                    descriptor_ranges.into_iter(),
                    DescriptorPoolCreateFlags::empty(),
                )
            }
            .expect("Could not create a descriptor pool");

            let desc_sets = (0..self.set_count)
                .into_iter()
                .map(|_| {
                    let mut desc_set = unsafe { desc_pool.allocate_one(&desc_set_layout) }.unwrap();
                    let ubos = self
                        .ubos
                        .iter_mut()
                        .map(|(id, (_, ubos))| (id.clone(), ubos.remove(0)))
                        .collect::<HashMap<_, _>>();

                    unsafe {
                        self.device.write_descriptor_set(DescriptorSetWrite {
                            set: &mut desc_set,
                            descriptors: iter::once(Descriptor::Buffer(
                                ubos.iter().next().unwrap().1.get(),
                                SubRange::WHOLE,
                            )),
                            array_offset: 0,
                            binding: 0,
                        });
                    }

                    (desc_set, ubos)
                })
                .collect();

            (Some(desc_pool), desc_sets)
        } else {
            (None, Vec::new())
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
            /* Rasterizer {
                polygon_mode: PolygonMode::Fill,
                cull_face: Face::BACK,
                front_face: FrontFace::Clockwise,
                depth_clamping: false,
                depth_bias: None,
                conservative: false,
                line_width: State::Static(1.0),
            }, */
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
        }
    }
}

impl<B: Backend> Pipeline<B> {
    pub fn bind(&self, command_buffer: &mut B::CommandBuffer, set_index: usize) {
        unsafe {
            command_buffer.bind_graphics_pipeline(&self.pipeline);

            if let Some((desc_set, _)) = self.desc_sets.get(set_index) {
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
        if let Some((set, ubo_set)) = self.desc_sets.get_mut(set_index) {
            let ubo = ubo_set.get_mut(&TypeId::of::<U>()).unwrap();
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
            for (_, ubos) in self.desc_sets {
                for (_, ubo) in ubos {
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
