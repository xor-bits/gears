use crate::{
    pipeline::shader_module, renderer::device::Dev, Buffer, BufferError, ImmediateFrameInfo,
    IndexBuffer, IndirectBuffer, Input, Module, MultiWriteBuffer, Output, RenderPass,
    RenderRecordInfo, UInt, Uniform, UniformBuffer, UpdateRecordInfo, VertexBuffer, WriteBuffer,
    WriteType, Yes,
};
use ash::{version::DeviceV1_0, vk};
use log::debug;
use parking_lot::RwLock;
use std::marker::PhantomData;

pub mod draw;

pub struct GraphicsPipeline<In, Out, UfVert, UfGeom, UfFrag>
where
    In: Input,
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    device: Dev,
    ubos: GraphicsPipelineUBOS<UfVert, UfGeom, UfFrag>,

    descriptor: Option<(vk::DescriptorPool, Vec<vk::DescriptorSet>)>,

    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    pipeline_descriptor_layout: vk::DescriptorSetLayout,

    _p: PhantomData<(In, Out, UfVert, UfGeom, UfFrag)>,
}

struct UBOModule<Uf>(Option<(Vec<RwLock<UniformBuffer<Uf>>>, u32)>)
where
    Uf: Uniform;

impl<Uf> UBOModule<Uf>
where
    Uf: Uniform,
{
    fn write(&self, imfi: &ImmediateFrameInfo, data: &Uf) -> Result<WriteType, BufferError> {
        let ubo = self
            .0
            .as_ref()
            .map(|(v, _)| v.get(imfi.image_index))
            .flatten()
            .unwrap();

        ubo.write().write(data)
    }

    unsafe fn update(&self, uri: &UpdateRecordInfo) -> bool {
        let ubo = self
            .0
            .as_ref()
            .map(|(sets, _)| sets.get(uri.image_index))
            .flatten();
        if let Some(ubo) = ubo {
            ubo.write().update(uri)
        } else {
            false
        }
    }
}

struct GraphicsPipelineUBOS<UfVert, UfGeom, UfFrag>
where
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    vert: UBOModule<UfVert>,
    geom: UBOModule<UfGeom>,
    frag: UBOModule<UfFrag>,
    count: usize,
}

impl<In, Out, UfVert, UfGeom, UfFrag> GraphicsPipeline<In, Out, UfVert, UfGeom, UfFrag>
where
    In: Input,
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    pub fn new(
        device: Dev,
        render_pass: RenderPass,
        set_count: usize,
        vert: Module<UfVert>,
        geom: Option<Module<UfGeom>>,
        frag: Module<UfFrag>,
        debug: bool,
    ) -> Result<Self, BufferError> {
        // modules

        let (vert_module, vert_stage) =
            shader_module(&device, &vert.spirv, vk::ShaderStageFlags::VERTEX);
        let (frag_module, frag_stage) =
            shader_module(&device, &frag.spirv, vk::ShaderStageFlags::FRAGMENT);
        let mut shader_stages = vec![vert_stage, frag_stage];
        let mut shader_modules = vec![vert_module, frag_module];

        // optional modules

        if let Some(geom) = geom.as_ref() {
            let (geom_module, geom_stage) =
                shader_module(&device, &geom.spirv, vk::ShaderStageFlags::GEOMETRY);
            shader_stages.push(geom_stage);
            shader_modules.push(geom_module);
        }

        // uniform buffer objects

        let ubos = Self::get_ubos(&device, set_count, &vert, &geom, &frag)?;

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
            .vertex_binding_descriptions(In::BINDING_DESCRIPTION)
            .vertex_attribute_descriptions(In::ATTRIBUTE_DESCRIPTION);

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

        let tmp_viewport = [render_pass.viewport];
        let tmp_scissors = [render_pass.scissor];
        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&tmp_viewport)
            .scissors(&tmp_scissors);

        let viewport_dynamic_state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&viewport_dynamic_state);

        let pipeline_info = [vk::GraphicsPipelineCreateInfo::builder()
            .subpass(0)
            .render_pass(render_pass.render_pass)
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
            for shader_module in shader_modules {
                device.destroy_shader_module(shader_module, None);
            }
        }

        let pipeline = pipeline.expect("Graphics pipeline creation failed")[0];

        Ok(Self {
            device,
            ubos,

            descriptor,

            pipeline,
            pipeline_layout,
            pipeline_descriptor_layout: pipeline_descriptor_layouts[0],

            _p: PhantomData {},
        })
    }

    pub unsafe fn update(&self, uri: &UpdateRecordInfo) -> bool {
        [
            self.ubos.vert.update(uri),
            self.ubos.geom.update(uri),
            self.ubos.frag.update(uri),
        ]
        .iter()
        .any(|u| *u)
    }

    pub unsafe fn bind(&self, rri: &RenderRecordInfo) {
        if rri.debug_calls {
            debug!("cmd_bind_pipeline");
        }

        self.device.cmd_bind_pipeline(
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

            self.device.cmd_bind_descriptor_sets(
                rri.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &descriptor_sets,
                &offsets,
            );
        }
    }

    pub fn create_vertex_buffer(&self, size: usize) -> Result<VertexBuffer<In>, BufferError> {
        VertexBuffer::new(&self.device, size)
    }

    pub fn create_vbo_with(&self, data: &[In]) -> Result<VertexBuffer<In>, BufferError> {
        let mut vbo = self.create_vertex_buffer(data.len())?;
        vbo.write(0, data)?;
        Ok(vbo)
    }

    pub fn create_index_buffer<I: UInt>(&self, size: usize) -> Result<IndexBuffer<I>, BufferError> {
        IndexBuffer::new(&self.device, size)
    }

    pub fn create_index_buffer_with<I: UInt>(
        &self,
        data: &[I],
    ) -> Result<IndexBuffer<I>, BufferError> {
        let mut vbo = self.create_index_buffer(data.len())?;
        vbo.write(0, data)?;
        Ok(vbo)
    }

    pub fn create_indirect_buffer(&self) -> Result<IndirectBuffer, BufferError> {
        self.create_indirect_buffer_with(0, 0)
    }

    pub fn create_indirect_buffer_with(
        &self,
        count: u32,
        offset: u32,
    ) -> Result<IndirectBuffer, BufferError> {
        IndirectBuffer::new_with(&self.device, count, offset)
    }
}

// draw

impl<In, Out, UfVert, UfGeom, UfFrag> GraphicsPipeline<In, Out, UfVert, UfGeom, UfFrag>
where
    In: Input,
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    pub unsafe fn draw<'a>(&'a self, rri: &'a RenderRecordInfo) -> DGDrawCommand<'a, In> {
        self.bind(rri);
        DrawCommand::new(&self.device, rri)
    }
}

// uniforms

impl<In, Out, UfVert, UfGeom, UfFrag> GraphicsPipeline<In, Out, UfVert, UfGeom, UfFrag>
where
    In: Input,
    Out: Output,
    UfVert: Uniform<HasFields = Yes>,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    pub fn write_vertex_uniform(
        &self,
        imfi: &ImmediateFrameInfo,
        data: &UfVert,
    ) -> Result<WriteType, BufferError> {
        self.ubos.vert.write(imfi, data)
    }
}

impl<In, Out, UfVert, UfGeom, UfFrag> GraphicsPipeline<In, Out, UfVert, UfGeom, UfFrag>
where
    In: Input,
    Out: Output,
    UfGeom: Uniform<HasFields = Yes>,
    UfVert: Uniform,
    UfFrag: Uniform,
{
    pub fn write_geometry_uniform(
        &self,
        imfi: &ImmediateFrameInfo,
        data: &UfGeom,
    ) -> Result<WriteType, BufferError> {
        self.ubos.geom.write(imfi, data)
    }
}

impl<In, Out, UfVert, UfGeom, UfFrag> GraphicsPipeline<In, Out, UfVert, UfGeom, UfFrag>
where
    In: Input,
    Out: Output,
    UfFrag: Uniform<HasFields = Yes>,
    UfVert: Uniform,
    UfGeom: Uniform,
{
    pub fn write_fragment_uniform(
        &self,
        imfi: &ImmediateFrameInfo,
        data: &UfFrag,
    ) -> Result<WriteType, BufferError> {
        self.ubos.frag.write(imfi, data)
    }
}

// privates

impl<In, Out, UfVert, UfGeom, UfFrag> GraphicsPipeline<In, Out, UfVert, UfGeom, UfFrag>
where
    In: Input,
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    fn get_ubos(
        device: &Dev,
        set_count: usize,
        vert: &Module<UfVert>,
        geom: &Option<Module<UfGeom>>,
        frag: &Module<UfFrag>,
    ) -> Result<GraphicsPipelineUBOS<UfVert, UfGeom, UfFrag>, BufferError> {
        let mut target = GraphicsPipelineUBOS {
            vert: UBOModule(None),
            geom: UBOModule(None),
            frag: UBOModule(None),
            count: 0,
        };

        if let Some((_, binding)) = vert.uniform {
            target.vert.0 = Some((
                (0..set_count)
                    .map(|_| Ok(RwLock::new(UniformBuffer::new_single(device)?)))
                    .collect::<Result<_, _>>()?,
                binding,
            ));
            target.count += 1;
        }
        match geom {
            Some(Module {
                uniform: Some((_, binding)),
                ..
            }) => {
                target.geom.0 = Some((
                    (0..set_count)
                        .map(|_| Ok(RwLock::new(UniformBuffer::new_single(device)?)))
                        .collect::<Result<_, _>>()?,
                    *binding,
                ));
                target.count += 1;
            }
            _ => {}
        }
        if let Some((_, binding)) = frag.uniform {
            target.frag.0 = Some((
                (0..set_count)
                    .map(|_| Ok(RwLock::new(UniformBuffer::new_single(device)?)))
                    .collect::<Result<_, _>>()?,
                binding,
            ));
            target.count += 1;
        }

        Ok(target)
    }

    fn get_bindings(
        ubos: &GraphicsPipelineUBOS<UfVert, UfGeom, UfFrag>,
    ) -> Vec<vk::DescriptorSetLayoutBinding> {
        let mut vec = Vec::new();
        if let Some((_, binding)) = ubos.vert.0 {
            vec.push(
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(binding)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::VERTEX)
                    .build(),
            );
        }
        if let Some((_, binding)) = ubos.frag.0 {
            vec.push(
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(binding)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                    .build(),
            );
        }

        vec
    }

    fn get_sizes(
        set_count: usize,
        ubos: &GraphicsPipelineUBOS<UfVert, UfGeom, UfFrag>,
    ) -> Vec<vk::DescriptorPoolSize> {
        let clone: vk::DescriptorPoolSize = vk::DescriptorPoolSize {
            descriptor_count: set_count as u32,
            ty: vk::DescriptorType::UNIFORM_BUFFER,
        };

        vec![clone; ubos.count]
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
        ubos: &GraphicsPipelineUBOS<UfVert, UfGeom, UfFrag>,
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
        ubos: &GraphicsPipelineUBOS<UfVert, UfGeom, UfFrag>,
    ) -> vk::DescriptorPool {
        let sizes = Self::get_sizes(set_count, ubos);

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

    fn write_descriptor_sets_for_ubo<Uf>(
        device: &Dev,
        desc_sets: &[vk::DescriptorSet],
        ubo: &Vec<RwLock<UniformBuffer<Uf>>>,
        binding: u32,
    ) where
        Uf: PartialEq,
    {
        for (&desc_set, ubo) in desc_sets.iter().zip(ubo.iter()) {
            let buffer = ubo.read().buffer();

            let buffer_info = [vk::DescriptorBufferInfo::builder()
                .offset(0)
                .range(vk::WHOLE_SIZE)
                .buffer(buffer)
                .build()];

            let write_set = [vk::WriteDescriptorSet::builder()
                .dst_array_element(0)
                .dst_binding(binding)
                .dst_set(desc_set)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&buffer_info)
                .build()];

            let copy_set = [];

            unsafe { device.update_descriptor_sets(&write_set, &copy_set) };
        }
    }

    fn write_descriptor_sets(
        device: &Dev,
        desc_sets: &[vk::DescriptorSet],
        ubos: &GraphicsPipelineUBOS<UfVert, UfGeom, UfFrag>,
    ) {
        if let Some((buf, binding)) = ubos.vert.0.as_ref() {
            Self::write_descriptor_sets_for_ubo(device, desc_sets, buf, *binding);
        }
        if let Some((buf, binding)) = ubos.geom.0.as_ref() {
            Self::write_descriptor_sets_for_ubo(device, desc_sets, buf, *binding);
        }
        if let Some((buf, binding)) = ubos.frag.0.as_ref() {
            Self::write_descriptor_sets_for_ubo(device, desc_sets, buf, *binding);
        }
    }
}

impl<In, Out, UfVert, UfGeom, UfFrag> Drop for GraphicsPipeline<In, Out, UfVert, UfGeom, UfFrag>
where
    In: Input,
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    fn drop(&mut self) {
        unsafe {
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);

            self.device.destroy_pipeline(self.pipeline, None);

            self.device
                .destroy_descriptor_set_layout(self.pipeline_descriptor_layout, None);

            if let Some((descriptor_pool, _)) = self.descriptor.take() {
                self.device.destroy_descriptor_pool(descriptor_pool, None);
            }
        }
    }
}
