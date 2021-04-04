use gears_traits::{Vertex, UBO};
use parking_lot::Mutex;
use std::{
    any::{type_name, TypeId},
    collections::HashMap,
    io::Cursor,
    iter,
    mem::{self, ManuallyDrop},
    ptr,
    sync::Arc,
};

use gfx_hal::{
    adapter::MemoryType,
    buffer::SubRange,
    command::CommandBuffer,
    device::Device,
    pass::Subpass,
    pso::{
        AttributeDesc, BlendState, BufferDescriptorFormat, BufferDescriptorType, ColorBlendDesc,
        ColorMask, Comparison, DepthStencilDesc, DepthTest, Descriptor, DescriptorPool,
        DescriptorPoolCreateFlags, DescriptorRangeDesc, DescriptorSetLayoutBinding,
        DescriptorSetWrite, DescriptorType, EntryPoint, Face, FrontFace, GraphicsPipelineDesc,
        InputAssemblerDesc, PolygonMode, Primitive, PrimitiveAssemblerDesc, Rasterizer,
        ShaderStageFlags, Specialization, State, StencilTest, VertexBufferDesc,
    },
    Backend,
};

use crate::GearsRenderer;

use super::buffer::{BufferError, GenericUniformBuffer, UniformBuffer};

pub struct PipelineBuilder<'a, B: Backend> {
    device: Arc<B::Device>,
    render_pass: &'a B::RenderPass,
    available_memory_types: &'a Vec<MemoryType>,
    set_count: usize,

    ubos: HashMap<
        TypeId,
        (
            ShaderStageFlags,
            Result<Vec<Arc<Mutex<dyn GenericUniformBuffer<B> + Send>>>, BufferError>,
        ),
    >,
}

pub struct GraphicsPipelineBuilder<'a, B: Backend> {
    base: PipelineBuilder<'a, B>,

    vert_input_binding: Vec<VertexBufferDesc>,
    vert_input_attribute: Vec<AttributeDesc>,

    vert_spirv: &'a [u8],
    geom_spirv: Option<&'a [u8]>,
    frag_spirv: &'a [u8],
}

/* TODO: pub struct ComputePipelineBuilder<'a, B: Backend> {
    base: PipelineBuilder<'a, B>,

    comp_spirv: &'a [u8],
} */

pub struct Pipeline<B: Backend> {
    device: Arc<B::Device>,

    desc_pool: Option<B::DescriptorPool>,

    desc_set_layout: ManuallyDrop<B::DescriptorSetLayout>,
    desc_sets: Vec<(
        B::DescriptorSet,
        HashMap<TypeId, Arc<Mutex<dyn GenericUniformBuffer<B> + Send>>>,
    )>,

    pipeline_layout: ManuallyDrop<B::PipelineLayout>,
    pipeline: ManuallyDrop<B::GraphicsPipeline>,
}

impl<'a, B: Backend> PipelineBuilder<'a, B> {
    pub fn new(renderer: &'a GearsRenderer<B>) -> Self {
        Self {
            device: renderer.device.clone(),
            render_pass: &renderer.render_pass,
            available_memory_types: &renderer.memory_types,
            set_count: renderer.frames_in_flight,

            ubos: HashMap::new(),
        }
    }

    pub fn new_with_device(
        device: Arc<B::Device>,
        render_pass: &'a B::RenderPass,
        available_memory_types: &'a Vec<MemoryType>,
        set_count: usize,
    ) -> Self {
        Self {
            device,
            render_pass,
            available_memory_types,
            set_count,

            ubos: HashMap::new(),
        }
    }

    pub fn with_graphics_modules(
        self,
        vert_spirv: &'a [u8],
        frag_spirv: &'a [u8],
    ) -> GraphicsPipelineBuilder<'a, B> {
        GraphicsPipelineBuilder::<'a, B> {
            base: self,

            vert_input_binding: Vec::new(),
            vert_input_attribute: Vec::new(),

            vert_spirv,
            geom_spirv: None,
            frag_spirv,
        }
    }

    /* TODO: pub fn with_compute_module(self, comp_spirv: &'a [u8]) -> ComputePipelineBuilder<'a, B> {
        ComputePipelineBuilder::<'a, B> {
            base: self,
            comp_spirv,
        }
    } */

    pub fn with_ubo<U: 'static + UBO + Default + Send>(mut self) -> Self {
        let buffers = (0..self.set_count)
            .map(
                |_| -> Result<Arc<Mutex<dyn GenericUniformBuffer<B> + Send>>, BufferError> {
                    let mut ubo = UniformBuffer::<U, B>::new_with_device(
                        self.device.clone(),
                        self.available_memory_types,
                    )?;
                    ubo.write(&U::default());
                    Ok(Arc::new(Mutex::new(ubo)))
                },
            )
            .collect::<Result<Vec<_>, BufferError>>();

        self.ubos.insert(TypeId::of::<U>(), (U::STAGE, buffers));

        self
    }
}

impl<'a, B: Backend> GraphicsPipelineBuilder<'a, B> {
    pub fn with_input<V: Vertex>(mut self) -> Self {
        self.vert_input_binding = V::binding_desc();
        self.vert_input_attribute = V::attribute_desc();
        self
    }

    pub fn with_geometry_module(mut self, geom_spirv: &'a [u8]) -> Self {
        self.geom_spirv = Some(geom_spirv);
        self
    }

    pub fn with_ubo<U: 'static + UBO + Default + Send>(mut self) -> Self {
        self.base = self.base.with_ubo::<U>();
        self
    }

    pub fn build(self, debug: bool) -> Result<Pipeline<B>, BufferError> {
        // vertex
        let vert_module = {
            let spirv = gfx_auxil::read_spirv(Cursor::new(&self.vert_spirv[..])).unwrap();
            unsafe { self.base.device.create_shader_module(&spirv) }
                .expect("Could not create a fragment shader module")
        };
        let vert_entry = EntryPoint::<B> {
            entry: "main",
            module: &vert_module,
            specialization: Specialization::default(),
        };

        // geometry
        let geom_module = self.geom_spirv.map(|geom_spirv| {
            let spirv = gfx_auxil::read_spirv(Cursor::new(&geom_spirv[..])).unwrap();
            unsafe { self.base.device.create_shader_module(&spirv) }
                .expect("Could not create a fragment shader module")
        });
        let geom_entry = geom_module.as_ref().map(|module| EntryPoint::<B> {
            entry: "main",
            module,
            specialization: Specialization::default(),
        });

        // fragment
        let frag_module = {
            let spirv = gfx_auxil::read_spirv(Cursor::new(&self.frag_spirv[..])).unwrap();
            unsafe { self.base.device.create_shader_module(&spirv) }
                .expect("Could not create a fragment shader module")
        };
        let frag_entry = EntryPoint::<B> {
            entry: "main",
            module: &frag_module,
            specialization: Specialization::default(),
        };

        let bindings = self
            .base
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

        let descriptor_ranges = self.base.ubos.iter().map(|_| DescriptorRangeDesc {
            ty: DescriptorType::Buffer {
                format: BufferDescriptorFormat::Structured {
                    dynamic_offset: false,
                },
                ty: BufferDescriptorType::Uniform,
            },
            count: 1,
        });

        let desc_set_layout = ManuallyDrop::new(
            unsafe {
                self.base
                    .device
                    .create_descriptor_set_layout(bindings.into_iter(), iter::empty())
            }
            .expect("Could not create a descriptor set layout"),
        );

        let (desc_pool, desc_sets) = if descriptor_ranges.len() > 0 {
            let mut desc_pool = unsafe {
                self.base.device.create_descriptor_pool(
                    self.base.set_count,
                    descriptor_ranges.into_iter(),
                    DescriptorPoolCreateFlags::empty(),
                )
            }
            .expect("Could not create a descriptor pool");

            let mut ubos = self
                .base
                .ubos
                .into_iter()
                .map(|(key, (stage, ubos))| match ubos {
                    Ok(ubos) => Ok((key, (stage, ubos))),
                    Err(e) => Err(e),
                })
                .collect::<Result<HashMap<_, _>, BufferError>>()?;
            let device = &self.base.device;
            let desc_sets = (0..self.base.set_count)
                .into_iter()
                .map(|_| {
                    let mut desc_set = unsafe { desc_pool.allocate_one(&desc_set_layout) }.unwrap();
                    let ubos = ubos
                        .iter_mut()
                        .map(|(id, (_, ubos))| (id.clone(), ubos.remove(0)))
                        .collect::<HashMap<TypeId, Arc<Mutex<dyn GenericUniformBuffer<B> + Send>>>>(
                        );

                    let first_ubo = ubos.iter().next().unwrap().1;

                    unsafe {
                        device.write_descriptor_set(DescriptorSetWrite {
                            set: &mut desc_set,
                            descriptors: iter::once(Descriptor::Buffer(
                                first_ubo.lock().get(),
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

        let pipeline_layout = ManuallyDrop::new(
            unsafe {
                self.base
                    .device
                    .create_pipeline_layout(iter::once(&*desc_set_layout), iter::empty())
            }
            .expect("Could not create a pipeline layout"),
        );

        let subpass = Subpass {
            index: 0,
            main_pass: self.base.render_pass,
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
                geometry: geom_entry,
                tessellation: None,
            },
            Rasterizer {
                polygon_mode: if debug {
                    PolygonMode::Line
                } else {
                    PolygonMode::Fill
                },
                cull_face: if debug { Face::NONE } else { Face::BACK },
                front_face: FrontFace::Clockwise,
                depth_clamping: false,
                depth_bias: None,
                conservative: false,
                line_width: State::Static(1.0),
            },
            Some(frag_entry),
            &*pipeline_layout,
            subpass,
        );

        pipeline_desc.depth_stencil = DepthStencilDesc {
            depth: Some(DepthTest {
                fun: if debug {
                    Comparison::Always
                } else {
                    Comparison::LessEqual
                },
                write: true,
            }),
            depth_bounds: false,
            stencil: Some(StencilTest::default()),
        };

        pipeline_desc.blender.targets.push(ColorBlendDesc {
            mask: ColorMask::ALL,
            blend: Some(BlendState::ALPHA),
        });

        let pipeline = unsafe {
            self.base
                .device
                .create_graphics_pipeline(&pipeline_desc, None)
        };

        unsafe {
            let device = &self.base.device;
            device.destroy_shader_module(frag_module);
            geom_module.map(|geom_module| device.destroy_shader_module(geom_module));
            device.destroy_shader_module(vert_module);
        }

        let pipeline = ManuallyDrop::new(pipeline.expect("Could not create a graphics pipeline"));

        Ok(Pipeline::<B> {
            device: self.base.device,
            desc_pool,
            desc_sets,
            desc_set_layout,
            pipeline_layout,
            pipeline,
        })
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

    pub fn write_ubo<U: 'static + UBO>(&mut self, new_data: &U, set_index: usize) {
        if let Some((set, ubo_set)) = self.desc_sets.get_mut(set_index) {
            let mut ubo = ubo_set.get_mut(&TypeId::of::<U>()).unwrap().lock();
            ubo.write_bytes(new_data as *const U as *const u8, mem::size_of::<U>());

            unsafe {
                self.device.write_descriptor_set(DescriptorSetWrite {
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
}

impl<B: Backend> Drop for Pipeline<B> {
    fn drop(&mut self) {
        self.desc_sets.clear();

        unsafe {
            let pipeline_layout = ManuallyDrop::into_inner(ptr::read(&self.pipeline_layout));
            self.device.destroy_pipeline_layout(pipeline_layout);

            let pipeline = ManuallyDrop::into_inner(ptr::read(&self.pipeline));
            self.device.destroy_graphics_pipeline(pipeline);

            let desc_set_layout = ManuallyDrop::into_inner(ptr::read(&self.desc_set_layout));
            self.device.destroy_descriptor_set_layout(desc_set_layout);

            if let Some(desc_pool) = self.desc_pool.take() {
                self.device.destroy_descriptor_pool(desc_pool);
            }
        }
    }
}
