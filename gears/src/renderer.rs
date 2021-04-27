pub mod buffer;
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
    MapErrorElseLogResult, MapErrorLog, VSync,
};

use ash::{
    extensions::khr,
    version::{DeviceV1_0, InstanceV1_0},
    vk,
};
use buffer::{Image, ImageBuilder, ImageFormat, ImageUsage};
use log::{debug, error};
use queue::Queues;
use std::{ffi::CStr, sync::Arc, time::Duration};

use self::query::PerfQuery;

struct TargetImage {
    rerecord_requested: bool,

    image_in_use_fence: vk::Fence,

    _color_image: Image,
    _depth_image: Image,
    framebuffer: vk::Framebuffer,

    command_buffer: vk::CommandBuffer,
    command_pool: vk::CommandPool,
    perf: PerfQuery,
}

struct FrameObject {
    frame_fence: vk::Fence,
    image_semaphore: vk::Semaphore,
    submit_semaphore: vk::Semaphore,
}

pub struct ImmediateFrameInfo {
    pub image_index: usize,
}

pub struct RenderRecordInfo {
    pub command_buffer: vk::CommandBuffer,
    pub image_index: usize,
}

pub trait RendererRecord {
    #[allow(unused_variables)]
    fn immediate(&mut self, imfi: &ImmediateFrameInfo) {}

    #[allow(unused_variables)]
    fn record(&mut self, rri: &RenderRecordInfo) {}
}

pub struct Renderer {
    queues: Queues,
    device: Arc<ash::Device>,
    context: Context,
    extent: vk::Extent2D,
    memory_properties: vk::PhysicalDeviceMemoryProperties,

    format: vk::SurfaceFormatKHR,
    present: vk::PresentModeKHR,
    swapchain_loader: khr::Swapchain,
    swapchain: vk::SwapchainKHR,
    viewport: vk::Viewport,
    scissor: vk::Rect2D,

    render_pass: vk::RenderPass,
    target_images: Vec<TargetImage>,

    frame: usize,
    frames_in_flight: usize,
    frame_objects: Vec<FrameObject>,
}

pub struct RendererBuilder {
    vsync: VSync,
}

impl TargetImage {
    fn new(
        device: Arc<ash::Device>,
        render_pass: vk::RenderPass,
        queues: &Queues,
        color_image: vk::Image,
        color_format: vk::Format,
        extent: vk::Extent2D,
        memory_properties: &vk::PhysicalDeviceMemoryProperties,
    ) -> Result<Self, ContextError> {
        let color_image =
            ImageBuilder::new_with_device(device.clone(), &memory_properties.memory_types)
                .build_with_image(color_image, ImageUsage::WRITE, color_format)
                .map_err_log("Color image creation failed", ContextError::OutOfMemory)?;

        let depth_image =
            ImageBuilder::new_with_device(device.clone(), &memory_properties.memory_types)
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

        let framebuffer = unsafe { device.create_framebuffer(&framebuffer_info, None) }
            .map_err_log("Framebuffer creation failed", ContextError::OutOfMemory)?;

        let command_pool_info =
            vk::CommandPoolCreateInfo::builder().queue_family_index(queues.graphics_family as u32);

        let command_pool = unsafe { device.create_command_pool(&command_pool_info, None) }
            .map_err_log("Command pool creation failed", ContextError::OutOfMemory)?;

        let command_buffer_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(1)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_pool(command_pool);

        let mut command_buffers = unsafe { device.allocate_command_buffers(&command_buffer_info) }
            .map_err_log(
                "Command buffer allocation failed",
                ContextError::OutOfMemory,
            )?;

        if command_buffers.len() != 1 {
            unreachable!("Allocated command buffer count not 1");
        }

        let command_buffer = command_buffers.remove(0);

        Ok(Self {
            rerecord_requested: true,

            image_in_use_fence: vk::Fence::null(),

            _color_image: color_image,
            _depth_image: depth_image,
            framebuffer,

            command_pool,
            command_buffer,
            perf: PerfQuery::new_with_device(device.clone()),
        })
    }
}

impl FrameObject {
    fn new(device: &Arc<ash::Device>) -> Self {
        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);

        Self {
            frame_fence: unsafe { device.create_fence(&fence_info, None) }
                .expect("Fence creation failed"),
            image_semaphore: unsafe { device.create_semaphore(&semaphore_info, None) }
                .expect("Semaphore creation failed"),
            submit_semaphore: unsafe { device.create_semaphore(&semaphore_info, None) }
                .expect("Semaphore creation failed"),
        }
    }
}

impl RendererRecord for () {
    fn record(&mut self, _: &RenderRecordInfo) {}
}

impl Renderer {
    pub fn new() -> RendererBuilder {
        RendererBuilder { vsync: VSync::On }
    }

    pub fn frame<T: RendererRecord>(&mut self, recorder: &mut T) -> Option<Duration> {
        let frame = self.frame;
        self.frame = (self.frame + 1) % self.frames_in_flight;
        let frame_objects = &self.frame_objects[frame];

        unsafe {
            self.device
                .wait_for_fences(&[frame_objects.frame_fence], true, !0)
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
                self.device
                    .wait_for_fences(&[target_image.image_in_use_fence], true, !0)
                    .expect("Failed to wait for fence");
            }
        }
        target_image.image_in_use_fence = frame_objects.frame_fence;
        unsafe {
            self.device
                .reset_fences(&[frame_objects.frame_fence])
                .expect("Failed to reset fence");
        }

        // update buffer
        let rerecord = target_image.rerecord_requested;
        self.immediate(recorder, image_index);
        if rerecord {
            self.record(recorder, image_index);
        }
        let frame_objects = &self.frame_objects[frame];
        let target_image = &mut self.target_images[image_index];
        let gpu_frametime = target_image.perf.get();

        // submit
        unsafe {
            self.device
                .queue_submit(
                    self.queues.graphics,
                    &[vk::SubmitInfo::builder()
                        .command_buffers(&[target_image.command_buffer])
                        .wait_semaphores(&[frame_objects.image_semaphore])
                        .signal_semaphores(&[frame_objects.submit_semaphore])
                        .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                        .build()],
                    frame_objects.frame_fence,
                )
                .expect("Graphics queue submit failed");
        }

        // present
        let suboptimal = unsafe {
            // present
            match self.swapchain_loader.queue_present(
                self.queues.present,
                &vk::PresentInfoKHR::builder()
                    .wait_semaphores(&[frame_objects.submit_semaphore])
                    .swapchains(&[self.swapchain])
                    .image_indices(&[image_index as u32]),
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

    fn immediate<T: RendererRecord>(&mut self, recorder: &mut T, image_index: usize) {
        let imfi = ImmediateFrameInfo { image_index };
        recorder.immediate(&imfi)
    }

    fn record<T: RendererRecord>(&mut self, recorder: &mut T, image_index: usize) {
        let target_image = &mut self.target_images[image_index];
        target_image.rerecord_requested = false;

        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder();
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

        unsafe {
            self.device
                .reset_command_pool(
                    target_image.command_pool,
                    vk::CommandPoolResetFlags::empty(),
                )
                .expect("Command pool reset failed");

            self.device
                .begin_command_buffer(target_image.command_buffer, &command_buffer_begin_info)
                .expect("Command buffer begin failed");

            let rri = RenderRecordInfo {
                command_buffer: target_image.command_buffer,
                image_index,
            };
            target_image.perf.reset(&rri);

            self.device
                .cmd_set_viewport(target_image.command_buffer, 0, &[self.viewport]);

            self.device
                .cmd_set_scissor(target_image.command_buffer, 0, &[self.scissor]);

            let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                .clear_values(&clear_values)
                .framebuffer(target_image.framebuffer)
                .render_pass(self.render_pass)
                .render_area(self.scissor);

            self.device.cmd_begin_render_pass(
                target_image.command_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );

            target_image.perf.begin(&rri);
            recorder.record(&rri);
            target_image.perf.end(&rri);

            self.device.cmd_end_render_pass(target_image.command_buffer);

            self.device
                .end_command_buffer(target_image.command_buffer)
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
            &self.context,
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
                self.device
                    .destroy_framebuffer(swapchain_objects.framebuffer, None);
            }

            let color_image = ImageBuilder::new_with_device(
                self.device.clone(),
                &self.memory_properties.memory_types,
            )
            .build_with_image(color_images[i], ImageUsage::WRITE, self.format.format)
            .expect("Color image creation failed");

            let depth_image = ImageBuilder::new_with_device(
                self.device.clone(),
                &self.memory_properties.memory_types,
            )
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
                unsafe { self.device.create_framebuffer(&framebuffer_info, None) }
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
            queue_wait_result(self.device.queue_wait_idle(self.queues.graphics));
            queue_wait_result(self.device.queue_wait_idle(self.queues.present));
        }
    }
}

impl RendererBuilder {
    pub fn with_vsync(mut self, vsync: VSync) -> Self {
        self.vsync = vsync;
        self
    }

    // ptrs are invalid as soon as context is
    fn device_layers(context: &Context) -> Vec<*const i8> {
        context
            .instance_layers
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect()
    }

    // ptrs are invalid as soon as context is
    fn device_extensions(context: &Context) -> Result<Vec<*const i8>, ContextError> {
        let available = unsafe {
            context
                .instance
                .enumerate_device_extension_properties(context.pdevice)
        }
        .map_err_log(
            "Could not query instance extensions",
            ContextError::OutOfMemory,
        )?;

        let requested = vec![khr::Swapchain::name()];
        let requested_raw: Vec<*const i8> =
            requested.iter().map(|raw_name| raw_name.as_ptr()).collect();

        let missing: Vec<_> = requested
            .iter()
            .filter_map(|ext| {
                if available
                    .iter()
                    .find(|aext| &unsafe { CStr::from_ptr(aext.extension_name.as_ptr()) } == ext)
                    .is_none()
                {
                    Some(ext)
                } else {
                    None
                }
            })
            .collect();

        debug!(
            "Requested device extensions: {:?}\nAvailable device extensions: {:?}",
            requested, available
        );
        if missing.len() > 0 {
            error!("Missing device extensions: {:?}", missing);
            return Err(ContextError::MissingDeviceExtensions);
        }

        Ok(requested_raw)
    }

    fn create_device(context: &Context) -> Result<(Arc<ash::Device>, Queues), ContextError> {
        // legacy device layers
        let instance_layers = Self::device_layers(&context);

        // device extensions
        let device_extensions = Self::device_extensions(&context)?;

        // queues
        let queue_create_infos = context.queue_families.get_vec().unwrap();

        // features
        let features = vk::PhysicalDeviceFeatures {
            geometry_shader: vk::TRUE,
            ..Default::default()
        };

        // device
        let device_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_infos)
            .enabled_layer_names(&instance_layers[..])
            .enabled_extension_names(&device_extensions[..])
            .enabled_features(&features);

        let device = Arc::new(
            unsafe {
                context
                    .instance
                    .create_device(context.pdevice, &device_info, None)
            }
            .map_err_log("Logical device creation failed", ContextError::OutOfMemory)?,
        );

        let queues = context.queue_families.get_queues(device.clone()).unwrap();

        Ok((device, queues))
    }

    fn pick_surface_format(context: &Context) -> Result<vk::SurfaceFormatKHR, ContextError> {
        let available = unsafe {
            context
                .surface_loader
                .get_physical_device_surface_formats(context.pdevice, context.surface)
        }
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
        context: &Context,
        vsync: VSync,
    ) -> Result<vk::PresentModeKHR, ContextError> {
        let available = unsafe {
            context
                .surface_loader
                .get_physical_device_surface_present_modes(context.pdevice, context.surface)
        }
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
        context: &Context,
        swapchain_loader: &khr::Swapchain,
        format: vk::SurfaceFormatKHR,
        present: vk::PresentModeKHR,
        extent: &mut vk::Extent2D,
    ) -> Result<(vk::SwapchainKHR, vk::Viewport, vk::Rect2D), ContextError> {
        let surface_caps = unsafe {
            context
                .surface_loader
                .get_physical_device_surface_capabilities(context.pdevice, context.surface)
        }
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
            .surface(context.surface)
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

    fn memory_properties(context: &Context) -> vk::PhysicalDeviceMemoryProperties {
        let memory_properties = unsafe {
            context
                .instance
                .get_physical_device_memory_properties(context.pdevice)
        };
        debug!("Memory properties: {:?}", memory_properties);
        memory_properties
    }

    fn render_pass(
        device: &Arc<ash::Device>,
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

        let color_attachment_ref = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build();

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
            .color_attachments(&[color_attachment_ref])
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

        let mut extent = context.extent;

        // device
        let (device, queues) = Self::create_device(&context)?;

        // swapchain
        let format = Self::pick_surface_format(&context)?;
        let present = Self::pick_surface_present_mode(&context, self.vsync)?;

        let swapchain_loader = khr::Swapchain::new(&context.instance, device.as_ref());

        let (swapchain, viewport, scissor) =
            Self::swapchain(&context, &swapchain_loader, format, present, &mut extent)?;

        let color_images = Self::swapchain_images(&swapchain_loader, swapchain)?;

        // memory
        let memory_properties = Self::memory_properties(&context);

        // main render pass
        let render_pass = Self::render_pass(&device, format.format)?;

        let target_images: Vec<TargetImage> = color_images
            .into_iter()
            .map(|image| {
                TargetImage::new(
                    device.clone(),
                    render_pass,
                    &queues,
                    image,
                    format.format,
                    extent,
                    &memory_properties,
                )
            })
            .collect::<Result<_, _>>()?;

        let frames_in_flight = 2;
        let frame_objects = (0..frames_in_flight)
            .map(|_| FrameObject::new(&device))
            .collect();

        Ok(Renderer {
            queues,
            device,
            context,
            extent,
            memory_properties,

            format,
            present,
            swapchain_loader,
            swapchain,
            viewport,
            scissor,

            render_pass,
            target_images,

            frame: 0,
            frames_in_flight,
            frame_objects,
        })
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.wait();

        unsafe {
            for frame_objects in self.frame_objects.drain(..) {
                self.device.destroy_fence(frame_objects.frame_fence, None);
                self.device
                    .destroy_semaphore(frame_objects.image_semaphore, None);
                self.device
                    .destroy_semaphore(frame_objects.submit_semaphore, None);
            }

            for target_image in self.target_images.drain(..) {
                self.device
                    .destroy_framebuffer(target_image.framebuffer, None);
                self.device.free_command_buffers(
                    target_image.command_pool,
                    &[target_image.command_buffer],
                );
                self.device
                    .destroy_command_pool(target_image.command_pool, None);
            }

            self.device.destroy_render_pass(self.render_pass, None);
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);

            self.context
                .surface_loader
                .destroy_surface(self.context.surface, None);
        }
        debug!("Renderer dropped");
    }
}
