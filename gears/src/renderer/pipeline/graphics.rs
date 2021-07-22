use super::shader_module;
use crate::{renderer::device::RenderDevice, Input, Module, PipelineBase, RenderRecordInfo};
use ash::{version::DeviceV1_0, vk};
use log::debug;
use std::{marker::PhantomData, sync::Arc};

pub struct GraphicsPipeline<I: Input, UfVert, UfFrag> {
    base: PipelineBase,

    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,

    _p0: PhantomData<I>,
    _p1: PhantomData<UfVert>,
    _p2: PhantomData<UfFrag>,
}

impl<I: Input, UfVert, UfFrag> GraphicsPipeline<I, UfVert, UfFrag> {
    pub fn new(
        device: Arc<RenderDevice>,
        render_pass: vk::RenderPass,
        vert: Module<UfVert>,
        frag: Module<UfFrag>,
        debug: bool,
    ) -> Self {
        // modules
        let vert = shader_module(&device, vert.spirv, vk::ShaderStageFlags::VERTEX);
        let frag = shader_module(&device, frag.spirv, vk::ShaderStageFlags::FRAGMENT);

        let shader_stages = vec![vert.1, frag.1];

        // pipeline layout
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder();
        let pipeline_layout = unsafe { device.create_pipeline_layout(&pipeline_layout_info, None) }
            .expect("Pipeline layout creation failed");

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
            .vertex_input_state(&vertex_state)
            .input_assembly_state(&vertex_assembly_state)
            .rasterization_state(&rasterizer_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_stencil_state)
            .color_blend_state(&color_blend_state)
            .stages(&shader_stages[..])
            .viewport_state(&viewport_state)
            .dynamic_state(&dynamic_state)
            .build()];

        let pipeline = unsafe {
            device.create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
        };

        unsafe {
            device.destroy_shader_module(frag.0, None);
            device.destroy_shader_module(vert.0, None);
        }

        let pipeline = pipeline.expect("Graphics pipeline creation failed")[0];

        Self {
            base: PipelineBase { device },

            pipeline,
            pipeline_layout,

            _p0: PhantomData {},
            _p1: PhantomData {},
            _p2: PhantomData {},
        }
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

        /* if let Some((desc_set, _)) = self.desc_sets.get(rri.image_index) {
            if rri.debug_calls {
                debug!("cmd_bind_descriptor_sets");
            }

            let desc_set = [*desc_set];
            self.device.cmd_bind_descriptor_sets(
                rri.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &desc_set,
                &[],
            );
        } */
    }
}

impl<I: Input, UfVert, UfFrag> Drop for GraphicsPipeline<I, UfVert, UfFrag> {
    fn drop(&mut self) {
        // self.desc_sets.clear();

        unsafe {
            self.base
                .device
                .destroy_pipeline_layout(self.pipeline_layout, None);

            self.base.device.destroy_pipeline(self.pipeline, None);

            /* self.base
                .device
                .destroy_descriptor_set_layout(self.desc_set_layout, None);

            if let Some(desc_pool) = self.desc_pool.take() {
                self.base.device.destroy_descriptor_pool(desc_pool, None);
            } */
        }
    }
}
