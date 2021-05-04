use ash::{util::read_spv, version::DeviceV1_0, vk};
use gears_traits::{Vertex, UBO};
use parking_lot::Mutex;
use std::{
    any::{type_name, Any, TypeId},
    collections::HashMap,
    ffi::CStr,
    io::Cursor,
    sync::Arc,
};

use crate::{
    renderer::buffer::Buffer, renderer::ImmediateFrameInfo, renderer::RenderRecordInfo,
    renderer::Renderer, renderer::UpdateQuery, renderer::UpdateRecordInfo, ExpectLog,
};

use super::{
    buffer::{uniform::UniformBuffer, BufferError, WriteType},
    device::RenderDevice,
};

trait UniformBufferT {
    fn updates_t(&self, uq: &UpdateQuery) -> bool;
    unsafe fn update_t(&mut self, uri: &UpdateRecordInfo);
    fn as_any(&mut self) -> &mut dyn Any;
}

impl<U: 'static> UniformBufferT for UniformBuffer<U> {
    fn updates_t(&self, uq: &UpdateQuery) -> bool {
        self.updates(uq)
    }

    unsafe fn update_t(&mut self, uri: &UpdateRecordInfo) {
        self.update(uri)
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

type UBStorage = Arc<Mutex<dyn UniformBufferT + Send>>;

pub struct PipelineBuilder {
    device: Arc<RenderDevice>,
    render_pass: vk::RenderPass,
    set_count: usize,

    ubos: HashMap<
        TypeId,
        (
            vk::ShaderStageFlags,
            Result<Vec<(vk::Buffer, UBStorage)>, BufferError>,
        ),
    >,
}

pub struct GraphicsPipelineBuilder<'a> {
    base: PipelineBuilder,

    vert_input_binding: Vec<vk::VertexInputBindingDescription>,
    vert_input_attribute: Vec<vk::VertexInputAttributeDescription>,

    vert_spirv: &'a [u8],
    geom_spirv: Option<&'a [u8]>,
    frag_spirv: &'a [u8],
}

/* TODO: pub struct ComputePipelineBuilder<'a, B: Backend> {
    base: PipelineBuilder<'a, B>,

    comp_spirv: &'a [u8],
} */

pub struct Pipeline {
    device: Arc<RenderDevice>,

    desc_pool: Option<vk::DescriptorPool>,

    desc_set_layout: vk::DescriptorSetLayout,
    desc_sets: Vec<(vk::DescriptorSet, HashMap<TypeId, UBStorage>)>,

    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

impl PipelineBuilder {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            device: renderer.rdevice.clone(),
            render_pass: renderer.render_pass,
            set_count: renderer.target_images.len(),

            ubos: HashMap::new(),
        }
    }

    pub fn new_with_device(
        device: Arc<RenderDevice>,
        render_pass: vk::RenderPass,
        set_count: usize,
    ) -> Self {
        Self {
            device,
            render_pass,
            set_count,

            ubos: HashMap::new(),
        }
    }

    pub fn with_graphics_modules<'a>(
        self,
        vert_spirv: &'a [u8],
        frag_spirv: &'a [u8],
    ) -> GraphicsPipelineBuilder<'a> {
        GraphicsPipelineBuilder::<'a> {
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
            .map(|_| -> Result<(vk::Buffer, UBStorage), BufferError> {
                let mut ubo = UniformBuffer::<U>::new_with_device(self.device.clone())?;
                ubo.write(&U::default())?;
                Ok((ubo.get(), Arc::new(Mutex::new(ubo))))
            })
            .collect::<Result<Vec<_>, BufferError>>();

        self.ubos.insert(TypeId::of::<U>(), (U::STAGE, buffers));

        self
    }
}

impl<'a> GraphicsPipelineBuilder<'a> {
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

    pub fn build(self, debug: bool) -> Result<Pipeline, BufferError> {
        // modules
        let vert = shader_module(
            &self.base.device,
            self.vert_spirv,
            vk::ShaderStageFlags::VERTEX,
        );
        let frag = shader_module(
            &self.base.device,
            self.frag_spirv,
            vk::ShaderStageFlags::FRAGMENT,
        );
        // optional module(s)
        let geom = self.geom_spirv.map(|geom_spirv| {
            shader_module(
                &self.base.device,
                geom_spirv,
                vk::ShaderStageFlags::GEOMETRY,
            )
        });

        let mut stages = vec![vert.1, frag.1];
        geom.map(|geom| stages.push(geom.1));

        let bindings = self
            .base
            .ubos
            .iter()
            .map(|(_, (stage, _))| {
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(stage.clone())
                    .build()
            })
            .collect::<Vec<_>>();

        let desc_set_layout_info =
            vk::DescriptorSetLayoutCreateInfo::builder().bindings(&bindings[..]);

        let desc_set_layout = [unsafe {
            self.base
                .device
                .create_descriptor_set_layout(&desc_set_layout_info, None)
        }
        .expect("Descriptor set layout creation failed")];

        let descriptor_sizes: Vec<vk::DescriptorPoolSize> = self
            .base
            .ubos
            .iter()
            .map(|_| {
                vk::DescriptorPoolSize::builder()
                    .descriptor_count(1)
                    .ty(vk::DescriptorType::UNIFORM_BUFFER)
                    .build()
            })
            .collect();

        let (desc_pool, desc_sets) = if descriptor_sizes.len() > 0 {
            let desc_pool_info = vk::DescriptorPoolCreateInfo::builder()
                .max_sets(self.base.set_count as u32)
                .pool_sizes(&descriptor_sizes);

            let desc_pool = unsafe {
                self.base
                    .device
                    .create_descriptor_pool(&desc_pool_info, None)
            }
            .expect("Descriptor pool creation failed");

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
                    let desc_set = unsafe {
                        device.allocate_descriptor_sets(
                            &vk::DescriptorSetAllocateInfo::builder()
                                .descriptor_pool(desc_pool)
                                .set_layouts(&desc_set_layout),
                        )
                    }
                    .unwrap()[0];
                    let ubos = ubos
                        .iter_mut()
                        .map(|(id, (_, ubos))| (id.clone(), ubos.remove(0)))
                        .collect::<HashMap<TypeId, (vk::Buffer, UBStorage)>>();

                    let first_ubo = ubos.iter().next().unwrap().1 .0;

                    unsafe {
                        let buffer_info = [vk::DescriptorBufferInfo::builder()
                            .offset(0)
                            .range(vk::WHOLE_SIZE)
                            .buffer(first_ubo)
                            .build()];

                        let write_set = [vk::WriteDescriptorSet::builder()
                            .dst_array_element(0)
                            .dst_binding(0)
                            .dst_set(desc_set)
                            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                            .buffer_info(&buffer_info)
                            .build()];

                        device.update_descriptor_sets(&write_set, &[]);
                    }

                    let ubos = ubos
                        .into_iter()
                        .map(|(id, (_, ubos))| (id, ubos))
                        .collect::<HashMap<TypeId, UBStorage>>();

                    (desc_set, ubos)
                })
                .collect();

            (Some(desc_pool), desc_sets)
        } else {
            (None, Vec::new())
        };

        let pipeline_layout_info =
            vk::PipelineLayoutCreateInfo::builder().set_layouts(&desc_set_layout);

        let pipeline_layout = unsafe {
            self.base
                .device
                .create_pipeline_layout(&pipeline_layout_info, None)
        }
        .expect("Pipeline layout creation failed");

        let vertex_state = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&self.vert_input_binding[..])
            .vertex_attribute_descriptions(&self.vert_input_attribute[..]);

        let vertex_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let rasterizer_state = vk::PipelineRasterizationStateCreateInfo::builder()
            .polygon_mode(if debug {
                vk::PolygonMode::LINE
            } else {
                vk::PolygonMode::FILL
            })
            .cull_mode(if debug {
                vk::CullModeFlags::NONE
            } else {
                vk::CullModeFlags::BACK
            })
            .front_face(vk::FrontFace::CLOCKWISE)
            .depth_clamp_enable(false)
            .depth_bias_enable(false)
            .depth_bias_constant_factor(0.0)
            .depth_bias_clamp(0.0)
            .depth_bias_slope_factor(0.0)
            .rasterizer_discard_enable(false)
            .line_width(1.0);

        let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .min_sample_shading(1.0)
            .sample_mask(&[])
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false);

        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::builder()
            .stencil_test_enable(false)
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
            .depth_bounds_test_enable(false)
            .min_depth_bounds(0.0)
            .max_depth_bounds(1.0);

        let color_blend_attachment = [vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::all())
            .blend_enable(false)
            .build()];
        let color_blend_state =
            vk::PipelineColorBlendStateCreateInfo::builder().attachments(&color_blend_attachment);

        let tmp_viewport = [vk::Viewport::builder()
            .width(600.0)
            .height(600.0)
            .x(0.0)
            .y(0.0)
            .min_depth(0.0)
            .max_depth(1.0)
            .build()];
        let tmp_scissors = [vk::Rect2D::builder()
            .offset(vk::Offset2D { x: 0, y: 0 })
            .extent(vk::Extent2D {
                width: 600,
                height: 600,
            })
            .build()];
        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&tmp_viewport)
            .scissors(&tmp_scissors);

        let viewport_dynamic_state = [vk::DynamicState::VIEWPORT];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&viewport_dynamic_state);

        let pipeline_info = [vk::GraphicsPipelineCreateInfo::builder()
            .subpass(0)
            .render_pass(self.base.render_pass)
            .layout(pipeline_layout)
            .vertex_input_state(&vertex_state)
            .input_assembly_state(&vertex_assembly_state)
            .rasterization_state(&rasterizer_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_stencil_state)
            .color_blend_state(&color_blend_state)
            .stages(&stages[..])
            .viewport_state(&viewport_state)
            .dynamic_state(&dynamic_state)
            .build()];

        let pipeline = unsafe {
            self.base.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &pipeline_info,
                None,
            )
        };

        unsafe {
            let device = &self.base.device;
            device.destroy_shader_module(frag.0, None);
            geom.map(|geom| device.destroy_shader_module(geom.0, None));
            device.destroy_shader_module(vert.0, None);
        }

        let pipeline = pipeline.expect("Graphics pipeline creation failed")[0];

        Ok(Pipeline {
            device: self.base.device,
            desc_pool,
            desc_sets,
            desc_set_layout: desc_set_layout[0],
            pipeline_layout,
            pipeline,
        })
    }
}

impl Pipeline {
    pub fn updates(&self, uq: &UpdateQuery) -> bool {
        let (_, ubos) = &self.desc_sets[uq.image_index];

        ubos.iter().any(|(_, ubo)| {
            let ubo_lock = ubo.lock();
            ubo_lock.updates_t(uq)
        })
    }

    pub unsafe fn update(&self, uri: &UpdateRecordInfo) {
        let (_, ubos) = &self.desc_sets[uri.image_index];

        for (_, ubo) in ubos {
            let mut ubo_lock = ubo.lock();
            ubo_lock.update_t(uri);
        }
    }

    pub unsafe fn bind(&self, rri: &RenderRecordInfo) {
        let (desc_set, _) = &self.desc_sets[rri.image_index];

        self.device.cmd_bind_pipeline(
            rri.command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline,
        );

        let desc_set = [*desc_set];
        self.device.cmd_bind_descriptor_sets(
            rri.command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline_layout,
            0,
            &desc_set,
            &[],
        );
    }

    pub fn write_ubo<'a, U: 'static + UBO>(
        &mut self,
        imfi: &ImmediateFrameInfo,
        new_data: &U,
    ) -> Result<WriteType, BufferError> {
        let (_, ubos) = &self.desc_sets[imfi.image_index];

        let mut ubo_lock = ubos
            .get(&TypeId::of::<U>())
            .expect_log(&*format!(
                "Type {:?} is not an UBO for this pipeline",
                type_name::<U>()
            ))
            .lock();
        let ubo = ubo_lock
            .as_any()
            .downcast_mut::<UniformBuffer<U>>()
            .unwrap();

        ubo.write(new_data)
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.desc_sets.clear();

        unsafe {
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);

            self.device.destroy_pipeline(self.pipeline, None);

            self.device
                .destroy_descriptor_set_layout(self.desc_set_layout, None);

            if let Some(desc_pool) = self.desc_pool.take() {
                self.device.destroy_descriptor_pool(desc_pool, None);
            }
        }
    }
}

fn shader_module(
    device: &Arc<RenderDevice>,
    spirv: &[u8],
    stage: vk::ShaderStageFlags,
) -> (vk::ShaderModule, vk::PipelineShaderStageCreateInfo) {
    let spirv = read_spv(&mut Cursor::new(&spirv[..])).expect("SPIR-V read failed");

    let module_info = vk::ShaderModuleCreateInfo::builder().code(&spirv[..]);

    let module = unsafe { device.create_shader_module(&module_info, None) }
        .expect("Vertex shader module creation failed");

    let stage = vk::PipelineShaderStageCreateInfo::builder()
        .module(module)
        .stage(stage)
        .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
        .build();

    (module, stage)
}
