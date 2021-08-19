use std::fmt::Debug;

use vulkano::{
    format::Format, pipeline::viewport::Viewport, render_pass::RenderPassDesc,
    single_pass_renderpass,
};

use super::device::Dev;

#[derive(Clone)]
pub struct RenderPass {
    pub viewport: Viewport,
    pub scissor: Rect,
    pub render_pass: vulkano::render_pass::RenderPass,

    device: Dev,
}

impl Debug for RenderPass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderPass")
            .field("viewport", &self.viewport)
            .field("scissor", &self.scissor)
            .field("render_pass", &self.render_pass)
            .finish()
    }
}

impl RenderPass {
    pub fn reset_area(&mut self, extent: vk::Extent2D) {
        let (viewport, scissor) = Self::viewport_and_scissor(extent);
        self.viewport = viewport;
        self.scissor = scissor;
    }

    pub fn viewport_and_scissor(extent: vk::Extent2D) -> (vk::Viewport, vk::Rect2D) {
        (
            vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: extent.width as f32,
                height: extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            },
            vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: extent,
            },
        )
    }

    pub fn new(
        device: Dev,
        format: vk::Format,
        extent: vk::Extent2D,
    ) -> Result<Self, ContextError> {
        let color_attachment = vk::AttachmentDescription::builder()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .build();

        let color_attachment_ref = [vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build()];

        let depth_attachment = vk::AttachmentDescription::builder()
            .format(ImageFormat::<f32>::D.format())
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build();

        let depth_attachment_ref = vk::AttachmentReference::builder()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build();

        let dependencies = [vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            )
            .build()];

        let attachments = [color_attachment, depth_attachment];

        let subpasses = [vk::SubpassDescription::builder()
            .color_attachments(&color_attachment_ref)
            .depth_stencil_attachment(&depth_attachment_ref)
            .build()];

        let render_pass_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);

        single_pass_renderpass!(device.clone(),
        attachments: {
            c: {
                load: Clear,
                store: Store,
                format: Format::R8G8B8A8Unorm,
                samples: 1,
            },
            d: {
                load: Clear,
                store: DontCare,
                format: ImageFormat::<f32>::D.format(),
                samples: 1,
            }
        },
        pass: {
            color: [c],
            depth_stencil: d
        });

        vulkano::render_pass::RenderPass::new(device, description);

        let render_pass = unsafe { device.create_render_pass(&render_pass_info, None) }
            .map_err_log("Render pass creation failed", ContextError::OutOfMemory)?;

        let (viewport, scissor) = Self::viewport_and_scissor(extent);

        Ok(Self {
            viewport,
            scissor,
            render_pass,

            device,
        })
    }
}
