use super::shader_module;
use crate::{
    renderer::device::Dev, Buffer, BufferError, ImmediateFrameInfo, Input, Module, PipelineBase,
    RenderRecordInfo, Uniform, UniformBuffer, UpdateRecordInfo, WriteType,
};
use ash::{version::DeviceV1_0, vk};
use log::debug;
use parking_lot::RwLock;
use std::marker::PhantomData;

pub struct GraphicsPipeline<I, UfVert, UfFrag>
where
    I: Input,
{
    base: PipelineBase,
    ubos: GraphicsPipelineUBOS<UfVert, UfFrag>,

    descriptor: Option<(vk::DescriptorPool, Vec<vk::DescriptorSet>)>,

    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    pipeline_descriptor_layout: vk::DescriptorSetLayout,

    _p0: PhantomData<I>,
    _p1: PhantomData<UfVert>,
    _p2: PhantomData<UfFrag>,
}

struct UBOModule<Uf>(Option<Vec<RwLock<UniformBuffer<Uf>>>>);

impl<Uf> UBOModule<Uf> {
    fn write(&self, imfi: &ImmediateFrameInfo, data: &Uf) -> Result<WriteType, BufferError> {
        let ubo = self
            .0
            .as_ref()
            .map(|v| v.get(imfi.image_index))
            .flatten()
            .unwrap();

        ubo.write().write(data)
    }

    unsafe fn update(&self, uri: &UpdateRecordInfo) -> bool {
        let ubo = self
            .0
            .as_ref()
            .map(|sets| sets.get(uri.image_index))
            .flatten();
        if let Some(ubo) = ubo {
            ubo.read().update(uri)
        } else {
            false
        }
    }
}

struct GraphicsPipelineUBOS<UfVert, UfFrag> {
    vert: UBOModule<UfVert>,
    frag: UBOModule<UfFrag>,
    count: usize,
}

impl<I, UfVert, UfFrag> GraphicsPipeline<I, UfVert, UfFrag>
where
    I: Input,
{
    pub fn new(
        device: Dev,
        render_pass: vk::RenderPass,
        set_count: usize,
        vert: Module<UfVert>,
        frag: Module<UfFrag>,
        debug: bool,
    ) -> Result<Self, BufferError> {
        // modules

        let vert_stage = shader_module(&device, vert.spirv, vk::ShaderStageFlags::VERTEX);
        let frag_stage = shader_module(&device, frag.spirv, vk::ShaderStageFlags::FRAGMENT);
        let shader_stages = vec![vert_stage.1, frag_stage.1];

        // uniform buffer objects

        let ubos = Self::get_ubos(&device, set_count, &vert, &frag)?;

        // pipeline layout

        let pipeline_descriptor_layouts = Self::descriptor_layout(&device, &ubos);
        let pipeline_layout = Self::pipeline_layout(&device, &pipeline_descriptor_layouts);

        // uniforms descriptors

        let descriptor = if ubos.count != 0 {
            // descriptor pool
            let desc_pool = Self::descriptor_pool(&device, set_count, &ubos);

            // all descriptor sets
            let desc_sets =
                Self::descriptor_sets(&device, set_count, desc_pool, &pipeline_descriptor_layouts);

            // write descriptor sets
            Self::write_descriptor_sets(&device, &desc_sets, &ubos);

            Some((desc_pool, desc_sets))
        } else {
            None
        };

        // fixed states

        let vertex_state = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(I::BINDING_DESCRIPTION)
            .vertex_attribute_descriptions(I::ATTRIBUTE_DESCRIPTION);

        let vertex_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let rasterizer_state = vk::PipelineRasterizationStateCreateInfo::builder()
            .polygon_mode(
                vk::PolygonMode::FILL, /* if debug {
                                           vk::PolygonMode::LINE
                                       } else {
                                           vk::PolygonMode::FILL
                                       } */
            )
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
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ZERO)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .build()];

        let color_blend_state =
            vk::PipelineColorBlendStateCreateInfo::builder().attachments(&color_blend_attachment);

        let tmp_viewport = [vk::Viewport::builder()
            .width(32.0)
            .height(32.0)
            .x(0.0)
            .y(0.0)
            .min_depth(0.0)
            .max_depth(1.0)
            .build()];
        let tmp_scissors = [vk::Rect2D::builder()
            .offset(vk::Offset2D { x: 0, y: 0 })
            .extent(vk::Extent2D {
                width: 32,
                height: 32,
            })
            .build()];
        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&tmp_viewport)
            .scissors(&tmp_scissors);

        let viewport_dynamic_state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&viewport_dynamic_state);

        let pipeline_info = [vk::GraphicsPipelineCreateInfo::builder()
            .subpass(0)
            .render_pass(render_pass)
            .layout(pipeline_layout)
            .stages(&shader_stages[..])
            .vertex_input_state(&vertex_state)
            .input_assembly_state(&vertex_assembly_state)
            .rasterization_state(&rasterizer_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_stencil_state)
            .color_blend_state(&color_blend_state)
            .viewport_state(&viewport_state)
            .dynamic_state(&dynamic_state)
            .build()];

        let pipeline = unsafe {
            device.create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
        };

        unsafe {
            device.destroy_shader_module(frag_stage.0, None);
            device.destroy_shader_module(vert_stage.0, None);
        }

        let pipeline = pipeline.expect("Graphics pipeline creation failed")[0];

        Ok(Self {
            base: PipelineBase { device },
            ubos,

            descriptor,

            pipeline,
            pipeline_layout,
            pipeline_descriptor_layout: pipeline_descriptor_layouts[0],

            _p0: PhantomData {},
            _p1: PhantomData {},
            _p2: PhantomData {},
        })
    }

    pub unsafe fn update(&self, uri: &UpdateRecordInfo) -> bool {
        let mut updates = false;

        updates = updates || self.ubos.vert.update(uri);
        updates = updates || self.ubos.frag.update(uri);

        updates
    }

    pub unsafe fn bind(&self, rri: &RenderRecordInfo) {
        if rri.debug_calls {
            debug!("cmd_bind_pipeline");
        }

        self.base.device.cmd_bind_pipeline(
            rri.command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline,
        );

        let descriptor_set = self
            .descriptor
            .as_ref()
            .map(|(_, sets)| sets.get(rri.image_index))
            .flatten();

        if let Some(&descriptor_set) = descriptor_set {
            if rri.debug_calls {
                debug!("cmd_bind_descriptor_sets");
            }

            let descriptor_sets = [descriptor_set];
            let offsets = [];

            self.base.device.cmd_bind_descriptor_sets(
                rri.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &descriptor_sets,
                &offsets,
            );
        }
    }

    fn get_ubos(
        device: &Dev,
        set_count: usize,
        vert: &Module<UfVert>,
        frag: &Module<UfFrag>,
    ) -> Result<GraphicsPipelineUBOS<UfVert, UfFrag>, BufferError> {
        let mut target = GraphicsPipelineUBOS {
            vert: UBOModule(None),
            frag: UBOModule(None),
            count: 0,
        };

        if vert.has_uniform {
            target.vert.0 = Some(
                (0..set_count)
                    .map(|_| Ok(RwLock::new(UniformBuffer::new_with_device(device.clone())?)))
                    .collect::<Result<_, _>>()?,
            );
            target.count += 1;
        }
        if frag.has_uniform {
            target.frag.0 = Some(
                (0..set_count)
                    .map(|_| Ok(RwLock::new(UniformBuffer::new_with_device(device.clone())?)))
                    .collect::<Result<_, _>>()?,
            );
            target.count += 1;
        }

        Ok(target)
    }

    fn get_bindings(
        ubos: &GraphicsPipelineUBOS<UfVert, UfFrag>,
    ) -> Vec<vk::DescriptorSetLayoutBinding> {
        /* TODO: let CLONE: vk::DescriptorSetLayoutBinding = vk::DescriptorSetLayoutBinding {
            binding: 0,
            ..Default::default()
        }; */

        let mut vec = Vec::new();
        if ubos.vert.0.is_some() {
            vec.push(
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::VERTEX)
                    .build(),
            )
        }
        if ubos.frag.0.is_some() {
            vec.push(
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                    .build(),
            )
        }

        vec
    }

    fn get_sizes(ubos: &GraphicsPipelineUBOS<UfVert, UfFrag>) -> Vec<vk::DescriptorPoolSize> {
        const CLONE: vk::DescriptorPoolSize = vk::DescriptorPoolSize {
            descriptor_count: 1,
            ty: vk::DescriptorType::UNIFORM_BUFFER,
        };

        vec![CLONE; ubos.count]
    }

    fn pipeline_layout(
        device: &Dev,
        desc_set_layouts: &[vk::DescriptorSetLayout],
    ) -> vk::PipelineLayout {
        let pipeline_layout_info =
            vk::PipelineLayoutCreateInfo::builder().set_layouts(desc_set_layouts);
        unsafe { device.create_pipeline_layout(&pipeline_layout_info, None) }
            .expect("Pipeline layout creation failed")
    }

    fn descriptor_layout(
        device: &Dev,
        ubos: &GraphicsPipelineUBOS<UfVert, UfFrag>,
    ) -> [vk::DescriptorSetLayout; 1] {
        let bindings = Self::get_bindings(&ubos);

        let desc_set_layout_info =
            vk::DescriptorSetLayoutCreateInfo::builder().bindings(&bindings[..]);
        let desc_set_layout =
            unsafe { device.create_descriptor_set_layout(&desc_set_layout_info, None) }
                .expect("Descriptor set layout creation failed");

        [desc_set_layout]
    }

    fn descriptor_pool(
        device: &Dev,
        set_count: usize,
        ubos: &GraphicsPipelineUBOS<UfVert, UfFrag>,
    ) -> vk::DescriptorPool {
        let sizes = Self::get_sizes(ubos);

        let desc_pool_info = vk::DescriptorPoolCreateInfo::builder()
            .max_sets(set_count as u32)
            .pool_sizes(&sizes[..]);

        unsafe { device.create_descriptor_pool(&desc_pool_info, None) }
            .expect("Descriptor pool creation failed")
    }

    fn descriptor_sets(
        device: &Dev,
        set_count: usize,
        desc_pool: vk::DescriptorPool,
        desc_set_layouts: &[vk::DescriptorSetLayout],
    ) -> Vec<vk::DescriptorSet> {
        let desc_set_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(desc_pool)
            .set_layouts(desc_set_layouts);

        (0..set_count)
            .into_iter()
            .map(|_| unsafe { device.allocate_descriptor_sets(&desc_set_info) }.unwrap()[0])
            .collect()
    }

    fn write_descriptor_sets(
        device: &Dev,
        desc_sets: &[vk::DescriptorSet],
        ubos: &GraphicsPipelineUBOS<UfVert, UfFrag>,
    ) {
        let buf = ubos
            .vert
            .0
            .as_ref()
            .map(|v| v.first().unwrap().read().get())
            .unwrap_or_else(|| {
                ubos.frag
                    .0
                    .as_ref()
                    .map(|v| v.first().unwrap().read().get())
                    .unwrap()
            });

        for &desc_set in desc_sets {
            let buffer_info = [vk::DescriptorBufferInfo::builder()
                .offset(0)
                .range(vk::WHOLE_SIZE)
                .buffer(buf)
                .build()];

            let write_set = [vk::WriteDescriptorSet::builder()
                .dst_array_element(0)
                .dst_binding(0)
                .dst_set(desc_set)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&buffer_info)
                .build()];

            let copy_set = [];

            unsafe { device.update_descriptor_sets(&write_set, &copy_set) };
        }
    }
}

impl<I, UfVert, UfFrag> GraphicsPipeline<I, UfVert, UfFrag>
where
    I: Input,
    UfVert: Uniform,
{
    pub fn write_vertex_uniform(
        &self,
        imfi: &ImmediateFrameInfo,
        data: &UfVert,
    ) -> Result<WriteType, BufferError> {
        self.ubos.vert.write(imfi, data)
    }
}

impl<I, UfVert, UfFrag> GraphicsPipeline<I, UfVert, UfFrag>
where
    I: Input,
    UfFrag: Uniform,
{
    pub fn write_fragment_uniform(
        &self,
        imfi: &ImmediateFrameInfo,
        data: &UfFrag,
    ) -> Result<WriteType, BufferError> {
        self.ubos.frag.write(imfi, data)
    }
}

impl<I, UfVert, UfFrag> Drop for GraphicsPipeline<I, UfVert, UfFrag>
where
    I: Input,
{
    fn drop(&mut self) {
        unsafe {
            self.base
                .device
                .destroy_pipeline_layout(self.pipeline_layout, None);

            self.base.device.destroy_pipeline(self.pipeline, None);

            self.base
                .device
                .destroy_descriptor_set_layout(self.pipeline_descriptor_layout, None);

            if let Some((descriptor_pool, _)) = self.descriptor.take() {
                self.base
                    .device
                    .destroy_descriptor_pool(descriptor_pool, None);
            }
        }
    }
}
