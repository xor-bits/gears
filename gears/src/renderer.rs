pub mod buffer;
mod device;
pub mod object;
pub mod pipeline;
mod query;
pub mod queue;

#[cfg(feature = "short_namespaces")]
pub use buffer::*;
#[cfg(feature = "short_namespaces")]
pub use object::*;
#[cfg(feature = "short_namespaces")]
pub use pipeline::*;
#[cfg(feature = "short_namespaces")]
pub use queue::*;

use crate::{
    context::{Context, ContextError},
    renderer::device::ReducedContext,
    MapErrorElseLogResult, MapErrorLog, VSync,
};

use ash::{extensions::khr, version::DeviceV1_0, vk};
use buffer::{Image, ImageBuilder, ImageFormat, ImageUsage};
use log::{debug, error};
use std::{sync::Arc, time::Duration};

use self::{device::RenderDevice, query::PerfQuery};

struct TargetImage {
    rerecord_requested: bool,

    image_in_use_fence: vk::Fence,

    _color_image: Image,
    _depth_image: Image,
    framebuffer: vk::Framebuffer,

    render_cb: vk::CommandBuffer,
    update_cb: vk::CommandBuffer,
    update_cb_pending: bool,
    command_pool: vk::CommandPool,
    perf: PerfQuery,
}

struct FrameObject {
    frame_fence: vk::Fence,
    image_semaphore: vk::Semaphore,
    update_semaphore: vk::Semaphore,
    render_semaphore: vk::Semaphore,
}

pub struct ImmediateFrameInfo {
    pub image_index: usize,
}

pub struct RenderRecordInfo {
    command_buffer: vk::CommandBuffer,
    image_index: usize,
}

pub struct UpdateRecordInfo {
    command_buffer: vk::CommandBuffer,
    image_index: usize,
}

pub struct UpdateQuery {
    image_index: usize,
}

pub trait RendererRecord {
    #[allow(unused_variables)]
    fn immediate(&mut self, imfi: &ImmediateFrameInfo) {}

    #[allow(unused_variables)]
    fn updates(&mut self, uq: &UpdateQuery) -> bool {
        false
    }

    #[allow(unused_variables)]
    fn update(&mut self, uri: &UpdateRecordInfo) {}

    #[allow(unused_variables)]
    fn record(&mut self, rri: &RenderRecordInfo) {}
}

pub struct Renderer {
    extent: vk::Extent2D,
    format: vk::SurfaceFormatKHR,
    present: vk::PresentModeKHR,
    viewport: vk::Viewport,
    scissor: vk::Rect2D,

    render_pass: vk::RenderPass,
    target_images: Vec<TargetImage>,

    frame: usize,
    frames_in_flight: usize,
    frame_objects: Vec<FrameObject>,

    // all following need to be destroyed manually, in order and the last
    swapchain_loader: khr::Swapchain,
    swapchain: vk::SwapchainKHR,

    surface_loader: khr::Surface,
    surface: vk::SurfaceKHR,

    rdevice: Arc<RenderDevice>,
}

pub struct RendererBuilder {
    vsync: VSync,
    frames_in_flight: usize,
}

impl TargetImage {
    fn new(
        rdevice: Arc<RenderDevice>,
        render_pass: vk::RenderPass,
        color_image: vk::Image,
        color_format: vk::Format,
        extent: vk::Extent2D,
    ) -> Result<Self, ContextError> {
        let color_image = ImageBuilder::new_with_device(rdevice.clone())
            .build_with_image(color_image, ImageUsage::WRITE, color_format)
            .map_err_log("Color image creation failed", ContextError::OutOfMemory)?;

        let depth_image = ImageBuilder::new_with_device(rdevice.clone())
            .with_width(extent.width)
            .with_height(extent.height)
            .build(ImageUsage::WRITE, ImageFormat::<f32>::D)
            .map_err_log("Depth image creation failed", ContextError::OutOfMemory)?;

        let attachments = [color_image.view(), depth_image.view()];

        let framebuffer_info = vk::FramebufferCreateInfo::builder()
            .attachments(&attachments)
            .render_pass(render_pass)
            .width(extent.width)
            .height(extent.height)
            .layers(1);

        let framebuffer = unsafe { rdevice.create_framebuffer(&framebuffer_info, None) }
            .map_err_log("Framebuffer creation failed", ContextError::OutOfMemory)?;

        let command_pool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(rdevice.queues.graphics_family as u32)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        let command_pool = unsafe { rdevice.create_command_pool(&command_pool_info, None) }
            .map_err_log("Command pool creation failed", ContextError::OutOfMemory)?;

        let command_buffer_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(2)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_pool(command_pool);

        let mut command_buffers = unsafe { rdevice.allocate_command_buffers(&command_buffer_info) }
            .map_err_log(
                "Command buffer allocation failed",
                ContextError::OutOfMemory,
            )?;

        if command_buffers.len() != 2 {
            unreachable!("Allocated command buffer count not 1");
        }

        let update_cb = command_buffers.remove(0);
        let render_cb = command_buffers.remove(0);

        Ok(Self {
            rerecord_requested: true,

            image_in_use_fence: vk::Fence::null(),

            _color_image: color_image,
            _depth_image: depth_image,
            framebuffer,

            render_cb,
            update_cb,
            update_cb_pending: false,
            command_pool,
            perf: PerfQuery::new_with_device(rdevice),
        })
    }
}

impl FrameObject {
    fn new(device: &Arc<RenderDevice>) -> Self {
        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);

        Self {
            frame_fence: unsafe { device.create_fence(&fence_info, None) }
                .expect("Fence creation failed"),
            image_semaphore: unsafe { device.create_semaphore(&semaphore_info, None) }
                .expect("Semaphore creation failed"),
            update_semaphore: unsafe { device.create_semaphore(&semaphore_info, None) }
                .expect("Semaphore creation failed"),
            render_semaphore: unsafe { device.create_semaphore(&semaphore_info, None) }
                .expect("Semaphore creation failed"),
        }
    }
}

impl RendererRecord for () {
    fn record(&mut self, _: &RenderRecordInfo) {}
}

impl Renderer {
    pub fn new() -> RendererBuilder {
        RendererBuilder {
            vsync: VSync::On,
            frames_in_flight: 2,
        }
    }

    pub fn frame<T: RendererRecord>(&mut self, recorder: &mut T) -> Option<Duration> {
        let frame = self.frame;
        self.frame = (self.frame + 1) % self.frames_in_flight;
        let frame_objects = &self.frame_objects[frame];

        unsafe {
            let fence = [frame_objects.frame_fence];
            self.rdevice
                .wait_for_fences(&fence, true, !0)
                .expect("Failed to wait for fence");
        }

        // aquire image
        let image_index = match unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                !0,
                frame_objects.image_semaphore,
                vk::Fence::null(),
            )
        } {
            Ok((image_index, false)) => image_index as usize,
            Ok((_, true)) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain();
                return None;
            }
            Err(err) => panic!("Failed to aquire image from swapchain: {:?}", err),
        };
        let target_image = &mut self.target_images[image_index];

        if target_image.image_in_use_fence != vk::Fence::null() {
            unsafe {
                let fence = [target_image.image_in_use_fence];
                self.rdevice
                    .wait_for_fences(&fence, true, !0)
                    .expect("Failed to wait for fence");
            }
        }
        target_image.image_in_use_fence = frame_objects.frame_fence;
        unsafe {
            let fence = [frame_objects.frame_fence];
            self.rdevice
                .reset_fences(&fence)
                .expect("Failed to reset fence");
        }

        // update buffers
        let rerecord = target_image.rerecord_requested;
        self.update(recorder, image_index);
        self.immediate(recorder, image_index);
        if rerecord {
            self.record(recorder, image_index);
        }
        let frame_objects = &self.frame_objects[frame];
        let target_image = &mut self.target_images[image_index];
        let gpu_frametime = target_image.perf.get();

        // submit
        unsafe {
            let update_cb = [target_image.update_cb];
            let update_wait = [frame_objects.image_semaphore];
            let update_signal = [frame_objects.update_semaphore];
            let update_stage = [vk::PipelineStageFlags::ALL_COMMANDS];

            let render_cb = [target_image.render_cb];
            let render_wait = [frame_objects.update_semaphore];
            let render_signal = [frame_objects.render_semaphore];
            let render_stage = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

            let submit_both = [
                vk::SubmitInfo::builder()
                    .command_buffers(&update_cb)
                    .wait_semaphores(&update_wait)
                    .signal_semaphores(&update_signal)
                    .wait_dst_stage_mask(&update_stage)
                    .build(),
                vk::SubmitInfo::builder()
                    .command_buffers(&render_cb)
                    .wait_semaphores(&render_wait)
                    .signal_semaphores(&render_signal)
                    .wait_dst_stage_mask(&render_stage)
                    .build(),
            ];

            let submit_render = [vk::SubmitInfo::builder()
                .command_buffers(&render_cb)
                .wait_semaphores(&update_wait)
                .signal_semaphores(&render_signal)
                .wait_dst_stage_mask(&render_stage)
                .build()];

            let submits = if target_image.update_cb_pending {
                &submit_both[..]
            } else {
                &submit_render[..]
            };

            self.rdevice
                .queue_submit(
                    self.rdevice.queues.graphics,
                    &submits,
                    frame_objects.frame_fence,
                )
                .expect("Graphics queue submit failed");
        }

        // present
        let suboptimal = unsafe {
            let wait = [frame_objects.render_semaphore];
            let swapchain = [self.swapchain];
            let image_index = [image_index as u32];

            // present
            match self.swapchain_loader.queue_present(
                self.rdevice.queues.present,
                &vk::PresentInfoKHR::builder()
                    .wait_semaphores(&wait)
                    .swapchains(&swapchain)
                    .image_indices(&image_index),
            ) {
                Ok(o) => o,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true,
                Err(e) => panic!("Present queue submit failed: {:?}", e),
            }
        };

        // recreate swapchain if needed
        if suboptimal {
            self.recreate_swapchain();
        }

        gpu_frametime.ok()
    }

    fn update<T: RendererRecord>(&mut self, recorder: &mut T, image_index: usize) {
        let target_image = &mut self.target_images[image_index];

        unsafe {
            self.rdevice
                .reset_command_buffer(target_image.update_cb, vk::CommandBufferResetFlags::empty())
                .expect("Command buffer reset failed");
        }

        let uq = UpdateQuery { image_index };
        target_image.update_cb_pending = recorder.updates(&uq);
        if target_image.update_cb_pending {
            unsafe {
                self.rdevice
                    .begin_command_buffer(
                        target_image.update_cb,
                        &vk::CommandBufferBeginInfo::builder(),
                    )
                    .expect("Command buffer begin failed");

                let uri = UpdateRecordInfo {
                    command_buffer: target_image.update_cb,
                    image_index,
                };
                recorder.update(&uri);

                self.rdevice
                    .end_command_buffer(target_image.update_cb)
                    .expect("Command buffer end failed");
            }
        }
    }

    fn immediate<T: RendererRecord>(&mut self, recorder: &mut T, image_index: usize) {
        let imfi = ImmediateFrameInfo { image_index };
        recorder.immediate(&imfi)
    }

    fn record<T: RendererRecord>(&mut self, recorder: &mut T, image_index: usize) {
        let target_image = &mut self.target_images[image_index];

        unsafe {
            self.rdevice
                .reset_command_buffer(target_image.render_cb, vk::CommandBufferResetFlags::empty())
                .expect("Command buffer reset failed");

            self.rdevice
                .begin_command_buffer(
                    target_image.render_cb,
                    &vk::CommandBufferBeginInfo::builder(),
                )
                .expect("Command buffer begin failed");

            let rri = RenderRecordInfo {
                command_buffer: target_image.render_cb,
                image_index,
            };
            target_image.perf.reset(&rri);

            let viewport = [self.viewport];
            self.rdevice
                .cmd_set_viewport(target_image.render_cb, 0, &viewport);

            let scissor = [self.scissor];
            self.rdevice
                .cmd_set_scissor(target_image.render_cb, 0, &scissor);

            let clear_values = [
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.18, 0.18, 0.2, 1.0],
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ];
            let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                .clear_values(&clear_values)
                .framebuffer(target_image.framebuffer)
                .render_pass(self.render_pass)
                .render_area(self.scissor);

            self.rdevice.cmd_begin_render_pass(
                target_image.render_cb,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );

            target_image.perf.begin(&rri);
            recorder.record(&rri);
            target_image.perf.end(&rri);

            self.rdevice.cmd_end_render_pass(target_image.render_cb);

            self.rdevice
                .end_command_buffer(target_image.render_cb)
                .expect("Command buffer end failed");
        }
    }

    pub fn request_rerecord(&mut self) {
        for target in self.target_images.iter_mut() {
            target.rerecord_requested = true;
        }
    }

    pub fn recreate_swapchain(&mut self) {
        self.wait();

        self.frame = 0;

        unsafe {
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None)
        };
        let (swapchain, viewport, scissor) = RendererBuilder::swapchain(
            self.rdevice.pdevice,
            self.surface,
            &self.surface_loader,
            &self.swapchain_loader,
            self.format,
            self.present,
            &mut self.extent,
        )
        .unwrap();

        self.swapchain = swapchain;
        self.viewport = viewport;
        self.scissor = scissor;

        let color_images =
            RendererBuilder::swapchain_images(&self.swapchain_loader, swapchain).unwrap();

        for (i, swapchain_objects) in self.target_images.iter_mut().enumerate() {
            unsafe {
                self.rdevice
                    .destroy_framebuffer(swapchain_objects.framebuffer, None);
            }

            let color_image = ImageBuilder::new_with_device(self.rdevice.clone())
                .build_with_image(color_images[i], ImageUsage::WRITE, self.format.format)
                .expect("Color image creation failed");

            let depth_image = ImageBuilder::new_with_device(self.rdevice.clone())
                .with_width(self.extent.width)
                .with_height(self.extent.height)
                .build(ImageUsage::WRITE, ImageFormat::<f32>::D)
                .expect("Depth image creation failed");

            let attachments = [color_image.view(), depth_image.view()];

            let framebuffer_info = vk::FramebufferCreateInfo::builder()
                .attachments(&attachments)
                .render_pass(self.render_pass)
                .width(self.extent.width)
                .height(self.extent.height)
                .layers(1);

            swapchain_objects.framebuffer =
                unsafe { self.rdevice.create_framebuffer(&framebuffer_info, None) }
                    .expect("Framebuffer creation failed");

            swapchain_objects._color_image = color_image;
            swapchain_objects._depth_image = depth_image;
        }

        self.request_rerecord();
    }

    pub fn wait(&self) {
        let queue_wait_result = |res: Result<(), vk::Result>| {
            res.map_err_else_log("Could not wait for queue to become idle", |err| match err {
                vk::Result::ERROR_DEVICE_LOST => ContextError::DriverCrash,
                _ => ContextError::OutOfMemory,
            })
            .unwrap()
        };

        unsafe {
            queue_wait_result(self.rdevice.queue_wait_idle(self.rdevice.queues.graphics));
            queue_wait_result(self.rdevice.queue_wait_idle(self.rdevice.queues.present));
        }
    }
}

impl RendererBuilder {
    /// Limits the framerate to usually 60 depending on the display settings.
    /// Eliminates screen tearing.
    pub fn with_vsync(mut self, vsync: VSync) -> Self {
        self.vsync = vsync;
        self
    }

    /// Increasing frames in flight <u>MIGHT</u> decrease the cpu frametime if the scene is simple.
    ///
    /// Slightly increases input delay.
    ///
    /// Recommended values: 2 or 3.
    ///
    /// 1 Never recommended and going above rarely improves anything.
    pub fn with_frames_in_flight(mut self, frames_in_flight: usize) -> Self {
        self.frames_in_flight = frames_in_flight;
        self
    }

    fn pick_surface_format(
        pdevice: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &khr::Surface,
    ) -> Result<vk::SurfaceFormatKHR, ContextError> {
        let available =
            unsafe { surface_loader.get_physical_device_surface_formats(pdevice, surface) }
                .map_err_log("Surface format query failed", ContextError::OutOfMemory)?;

        if available.len() == 0 {
            error!("No surface formats available");
            return Err(ContextError::MissingSurfaceConfigs);
        }

        let format = available
            .iter()
            .find(|format| {
                format.format == vk::Format::R8G8B8A8_SRGB
                    && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .unwrap_or(&available[0]);
        let format = format.clone();

        debug!("Surface format chosen: {:?} from {:?}", format, available);

        Ok(format)
    }

    fn pick_surface_present_mode(
        pdevice: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &khr::Surface,
        vsync: VSync,
    ) -> Result<vk::PresentModeKHR, ContextError> {
        let available =
            unsafe { surface_loader.get_physical_device_surface_present_modes(pdevice, surface) }
                .map_err_log(
                "Surface present mode query failed",
                ContextError::OutOfMemory,
            )?;

        if available.len() == 0 {
            error!("No surface present modes available");
            return Err(ContextError::MissingSurfaceConfigs);
        }

        let mode = match vsync {
            VSync::Off => vk::PresentModeKHR::IMMEDIATE,
            VSync::On => available
                .iter()
                .find(|&&present| present == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(&vk::PresentModeKHR::FIFO)
                .clone(),
        };

        debug!(
            "Surface present mode chosen: {:?} from {:?}",
            mode, available
        );

        Ok(mode)
    }

    fn swapchain(
        pdevice: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        surface_loader: &khr::Surface,
        swapchain_loader: &khr::Swapchain,
        format: vk::SurfaceFormatKHR,
        present: vk::PresentModeKHR,
        extent: &mut vk::Extent2D,
    ) -> Result<(vk::SwapchainKHR, vk::Viewport, vk::Rect2D), ContextError> {
        let surface_caps =
            unsafe { surface_loader.get_physical_device_surface_capabilities(pdevice, surface) }
                .map_err_else_log("Surface capability query failed", |err| match err {
                    vk::Result::ERROR_SURFACE_LOST_KHR => ContextError::FrameLost,
                    _ => ContextError::OutOfMemory,
                })?;

        let mut min_swapchain_len = surface_caps.min_image_count + 1;
        if surface_caps.max_image_count > 0 && min_swapchain_len > surface_caps.max_image_count {
            min_swapchain_len = surface_caps.max_image_count;
        }

        if surface_caps.current_extent.width != u32::MAX {
            *extent = surface_caps.current_extent
        } else {
            (*extent).width = (*extent)
                .width
                .max(surface_caps.min_image_extent.width)
                .min(surface_caps.max_image_extent.width);
            (*extent).height = (*extent)
                .height
                .max(surface_caps.min_image_extent.height)
                .min(surface_caps.max_image_extent.height);
        };

        let transform = if surface_caps
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            surface_caps.current_transform
        };

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface)
            .min_image_count(min_swapchain_len)
            .image_color_space(format.color_space)
            .image_format(format.format)
            .image_extent(*extent)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present)
            .clipped(true)
            .image_array_layers(1);

        let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None) }
            .map_err_else_log("Swapchain creation failed", |err| match err {
                vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                    ContextError::OutOfMemory
                }
                vk::Result::ERROR_DEVICE_LOST => ContextError::DriverCrash,
                _ => ContextError::FrameInUse,
            })?;

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: extent.width as f32,
            height: extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D {
                width: extent.width,
                height: extent.height,
            },
        };

        Ok((swapchain, viewport, scissor))
    }

    fn swapchain_images(
        swapchain_loader: &khr::Swapchain,
        swapchain: vk::SwapchainKHR,
    ) -> Result<Vec<vk::Image>, ContextError> {
        unsafe { swapchain_loader.get_swapchain_images(swapchain) }
            .map_err_log("Swapchain image query failed", ContextError::OutOfMemory)
    }

    fn render_pass(
        device: Arc<RenderDevice>,
        format: vk::Format,
    ) -> Result<vk::RenderPass, ContextError> {
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

        unsafe { device.create_render_pass(&render_pass_info, None) }
            .map_err_log("Render pass creation failed", ContextError::OutOfMemory)
    }

    pub fn build(self, context: Context) -> Result<Renderer, ContextError> {
        debug!("Renderer created");

        // rdevice
        let (r_context, surface, surface_loader, mut extent) = ReducedContext::new(context);
        let rdevice = RenderDevice::from_context(r_context)?;

        // swapchain
        let format = Self::pick_surface_format(rdevice.pdevice, surface, &surface_loader)?;
        let present =
            Self::pick_surface_present_mode(rdevice.pdevice, surface, &surface_loader, self.vsync)?;

        let swapchain_loader = khr::Swapchain::new(&rdevice.instance, &**rdevice);

        let (swapchain, viewport, scissor) = Self::swapchain(
            rdevice.pdevice,
            surface,
            &surface_loader,
            &swapchain_loader,
            format,
            present,
            &mut extent,
        )?;

        let color_images = Self::swapchain_images(&swapchain_loader, swapchain)?;

        // main render pass
        let render_pass = Self::render_pass(rdevice.clone(), format.format)?;

        let target_images: Vec<TargetImage> = color_images
            .into_iter()
            .map(|image| {
                TargetImage::new(rdevice.clone(), render_pass, image, format.format, extent)
            })
            .collect::<Result<_, _>>()?;

        let frames_in_flight = self.frames_in_flight;
        let frame_objects = (0..frames_in_flight)
            .map(|_| FrameObject::new(&rdevice))
            .collect();

        Ok(Renderer {
            extent,
            format,
            present,
            viewport,
            scissor,

            render_pass,
            target_images,

            frame: 0,
            frames_in_flight,
            frame_objects,

            swapchain_loader,
            swapchain,

            surface_loader,
            surface,

            rdevice,
        })
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.wait();

        unsafe {
            for frame_objects in self.frame_objects.drain(..) {
                self.rdevice.destroy_fence(frame_objects.frame_fence, None);
                self.rdevice
                    .destroy_semaphore(frame_objects.image_semaphore, None);
                self.rdevice
                    .destroy_semaphore(frame_objects.update_semaphore, None);
                self.rdevice
                    .destroy_semaphore(frame_objects.render_semaphore, None);
            }

            for target_image in self.target_images.drain(..) {
                self.rdevice
                    .destroy_framebuffer(target_image.framebuffer, None);
                let cbs = [target_image.render_cb, target_image.update_cb];
                self.rdevice
                    .free_command_buffers(target_image.command_pool, &cbs);
                self.rdevice
                    .destroy_command_pool(target_image.command_pool, None);
            }

            self.rdevice.destroy_render_pass(self.render_pass, None);
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);

            self.surface_loader.destroy_surface(self.surface, None);
        }
        debug!("Renderer dropped");
    }
}
