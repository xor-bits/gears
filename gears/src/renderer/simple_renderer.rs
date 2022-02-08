use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};

use crate::{
    buffer::Image,
    device::{Dev, ReducedContext, RenderDevice},
    query::PerfQuery,
    Context, ContextError, DerefDev, FramePerfReport, ImageBuilder, ImageFormat, ImageUsage,
    ImmediateFrameInfo, MapErrorLog, RenderPass, RenderRecordInfo, RendererRecord, Surface,
    Swapchain, SyncMode, UpdateRecordInfo,
};
use ash::{version::DeviceV1_0, vk};
use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};
use winit::window::Window;

struct SwapchainObjects {
    render_pass: RenderPass,
    swapchain: Option<Swapchain>,
    surface: Surface,
}

struct RenderTarget {
    // fence for waiting until the gpu is done with this frame
    frame_done_fence: vk::Fence,

    // the actual render target
    _color_image: Image,
    _depth_image: Image,
    framebuffer: vk::Framebuffer,

    // gpu commands
    render_cb: vk::CommandBuffer,
    update_cb: vk::CommandBuffer,
    update_cb_recording: bool,
    update_cb_pending: bool,
    command_pool: vk::CommandPool,

    // performance debugging
    perf: PerfQuery,
    triangles: usize,

    // device handle for uninitialization
    device: Dev,
}

impl RenderTarget {
    fn new(
        device: Dev,
        render_pass: &RenderPass,
        color_image: vk::Image,
        color_format: vk::Format,
        extent: vk::Extent2D,
    ) -> Result<Self, ContextError> {
        let _color_image = ImageBuilder::new(&device)
            .build_with_image(color_image, ImageUsage::WRITE, color_format)
            .map_err_log("Color image creation failed", ContextError::OutOfMemory)?;

        let _depth_image = ImageBuilder::new(&device)
            .with_width(extent.width)
            .with_height(extent.height)
            .build(ImageUsage::WRITE, ImageFormat::<f32>::D)
            .map_err_log("Depth image creation failed", ContextError::OutOfMemory)?;

        let attachments = [_color_image.view(), _depth_image.view()];

        let framebuffer_info = vk::FramebufferCreateInfo::builder()
            .attachments(&attachments)
            .render_pass(render_pass.render_pass)
            .width(extent.width)
            .height(extent.height)
            .layers(1);

        let framebuffer = unsafe { device.create_framebuffer(&framebuffer_info, None) }
            .map_err_log("Framebuffer creation failed", ContextError::OutOfMemory)?;

        let command_pool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(device.queues.graphics_family as u32)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        let command_pool = unsafe { device.create_command_pool(&command_pool_info, None) }
            .map_err_log("Command pool creation failed", ContextError::OutOfMemory)?;

        let command_buffer_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(2)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_pool(command_pool);

        let mut command_buffers = unsafe { device.allocate_command_buffers(&command_buffer_info) }
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
            frame_done_fence: vk::Fence::null(),

            _color_image,
            _depth_image,
            framebuffer,

            render_cb,
            update_cb,
            update_cb_recording: false,
            update_cb_pending: false,
            command_pool,

            perf: PerfQuery::new_with_device(device.clone()),
            triangles: 0,

            device,
        })
    }
}

struct FrameSync {
    // fence for waiting until the gpu is done with this image
    frame_done_fence: vk::Fence,

    // for gpu to wait for image to be ready
    image_semaphore: vk::Semaphore,

    // for gpu to wait for update to complete
    update_semaphore: vk::Semaphore,

    // for gpu to wait for render to complete
    render_semaphore: vk::Semaphore,

    // device handle for uninitialization
    device: Dev,
}

impl FrameSync {
    fn new(device: Dev) -> Result<Self, ContextError> {
        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);

        Ok(Self {
            frame_done_fence: unsafe { device.create_fence(&fence_info, None) }
                .map_err_log("Fence creation failed", ContextError::OutOfMemory)?,
            image_semaphore: unsafe { device.create_semaphore(&semaphore_info, None) }
                .map_err_log("Semaphore creation failed", ContextError::OutOfMemory)?,
            update_semaphore: unsafe { device.create_semaphore(&semaphore_info, None) }
                .map_err_log("Semaphore creation failed", ContextError::OutOfMemory)?,
            render_semaphore: unsafe { device.create_semaphore(&semaphore_info, None) }
                .map_err_log("Semaphore creation failed", ContextError::OutOfMemory)?,

            device,
        })
    }
}

pub struct Renderer {
    swapchain_objects: Mutex<SwapchainObjects>,

    // one render target per swapchain image
    render_targets: Box<[Mutex<RenderTarget>]>,
    // rerecord the render command buffers
    rerecord_render_targets: Box<[AtomicBool]>,

    // one set of sync objects for each frame in flight
    frame_syncs: Box<[Mutex<FrameSync>]>,

    // current frame in flight
    frame: AtomicUsize,

    /* // next free sync object
    frame: AtomicUsize, */
    pub device: Dev,
}

pub struct RendererBuilder {
    sync: SyncMode,
    frames_in_flight: usize,
}

impl DerefDev for Renderer {
    fn deref_dev(&self) -> &Dev {
        &self.device
    }
}

impl Renderer {
    pub fn new() -> RendererBuilder {
        RendererBuilder {
            sync: SyncMode::Mailbox,
            frames_in_flight: 3,
        }
    }

    pub fn render_pass(&self) -> MappedMutexGuard<'_, RenderPass> {
        MutexGuard::map(self.swapchain_objects.lock(), |swapchain_objects| {
            &mut swapchain_objects.render_pass
        })
    }

    pub fn parallel_object_count(&self) -> usize {
        self.device.set_count
        /* self.render_targets.len() */
    }

    pub fn frame<T>(&self, recorder: &T) -> FramePerfReport
    where
        T: RendererRecord,
    {
        let cpu_frametime = Instant::now();

        // acquire one free frame sync object
        let frame = self.acquire_frame_sync();

        // wait for gpu to be done with it
        self.wait_for_fence(frame.frame_done_fence);

        // acquire the next image
        let target = self.acquire_frame_image(&frame);

        // wait for the gpu to be done with this target image
        let mut render_target = self.render_targets[target].lock();
        self.wait_for_fence(render_target.frame_done_fence);

        // and set the new render target fence to be the one that this frame sync object is controlling
        render_target.frame_done_fence = frame.frame_done_fence;
        self.reset_fence(render_target.frame_done_fence);

        // update buffers
        self.update(recorder, &mut render_target, target);
        self.immediate(recorder, target);
        let rerecord = self.maybe_record(recorder, &mut render_target, target);

        // fetch the last frame gpu time(s)
        let gpu_frametime = render_target.perf.get().unwrap_or_default();

        // submit

        let render_cb = [render_target.render_cb];
        let update_cb = [render_target.update_cb];
        let image_wait = [frame.image_semaphore];
        let update_wait = [frame.update_semaphore];
        let render_wait = [frame.render_semaphore];
        let update_stage = [vk::PipelineStageFlags::ALL_COMMANDS];
        let render_stage = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let frame_fence = frame.frame_done_fence;

        if render_target.update_cb_pending {
            self.submit_update(
                &render_cb,
                &update_cb,
                &image_wait,
                &render_wait,
                &update_wait,
                &update_stage,
                &render_stage,
                frame_fence,
            )
        } else {
            self.submit_render(
                &render_cb,
                &update_cb,
                &image_wait,
                &render_wait,
                &update_wait,
                &update_stage,
                &render_stage,
                frame_fence,
            )
        };

        let updates = render_target.update_cb_pending;
        let triangles = render_target.triangles;

        // submit present

        if self
            .swapchain_objects
            .lock()
            .swapchain
            .as_ref()
            .unwrap()
            .present(self.device.queues.present, &render_wait, target)
        {
            self.wait_for_fence(frame.frame_done_fence);
            self.wait_for_fence(render_target.frame_done_fence);
            drop(render_target);
            self.re_create_swapchain().unwrap();
        }

        FramePerfReport {
            cpu_frametime: cpu_frametime.elapsed(),
            gpu_frametime,

            rerecord,
            updates,
            triangles,
        }
    }

    pub fn request_rerecord(&self) {
        for target in self.rerecord_render_targets.iter() {
            target.store(true, Ordering::SeqCst);
        }
    }

    fn update<T>(&self, recorder: &T, render_target: &mut MutexGuard<RenderTarget>, target: usize)
    where
        T: RendererRecord,
    {
        if !render_target.update_cb_recording {
            unsafe {
                self.device.reset_command_buffer(
                    render_target.update_cb,
                    vk::CommandBufferResetFlags::empty(),
                )
            }
            .expect("Command buffer reset failed");
            unsafe {
                self.device.begin_command_buffer(
                    render_target.update_cb,
                    &vk::CommandBufferBeginInfo::builder(),
                )
            }
            .expect("Command buffer begin failed");

            render_target.update_cb_recording = true;
        }

        render_target.update_cb_pending = unsafe {
            recorder.update(&UpdateRecordInfo {
                command_buffer: render_target.update_cb,
                image_index: target,
            })
        };

        if render_target.update_cb_pending {
            render_target.update_cb_recording = false;
            unsafe { self.device.end_command_buffer(render_target.update_cb) }
                .expect("Command buffer end failed");
        }
    }

    fn immediate<T>(&self, recorder: &T, target: usize)
    where
        T: RendererRecord,
    {
        recorder.immediate(&ImmediateFrameInfo {
            image_index: target,
        })
    }

    fn maybe_record<T>(
        &self,
        recorder: &T,
        render_target: &mut MutexGuard<RenderTarget>,
        target: usize,
    ) -> bool
    where
        T: RendererRecord,
    {
        let rerecord = self.rerecord_render_targets[target].swap(false, Ordering::SeqCst);
        if rerecord {
            self.record(recorder, render_target, target)
        }
        rerecord
    }

    fn record<T>(&self, recorder: &T, render_target: &mut MutexGuard<RenderTarget>, target: usize)
    where
        T: RendererRecord,
    {
        let begin_info = recorder.begin_info();

        let swapchain_objects = self.swapchain_objects.lock();
        let rri = RenderRecordInfo {
            command_buffer: render_target.render_cb,
            image_index: target,
            triangles: AtomicUsize::new(0),
            debug_calls: begin_info.debug_calls,
        };

        let viewport = [swapchain_objects.render_pass.viewport];
        let scissor = [swapchain_objects.render_pass.scissor];
        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [
                        begin_info.clear_color.x,
                        begin_info.clear_color.y,
                        begin_info.clear_color.z,
                        begin_info.clear_color.w,
                    ],
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
            .framebuffer(render_target.framebuffer)
            .render_pass(swapchain_objects.render_pass.render_pass)
            .render_area(swapchain_objects.render_pass.scissor);

        if begin_info.debug_calls {
            log::debug!("begin_command_buffer with: {:?}", begin_info);
        }

        unsafe {
            self.device.reset_command_buffer(
                render_target.render_cb,
                vk::CommandBufferResetFlags::empty(),
            )
        }
        .expect("Command buffer reset failed");

        unsafe {
            self.device.begin_command_buffer(
                render_target.render_cb,
                &vk::CommandBufferBeginInfo::builder(),
            )
        }
        .expect("Command buffer begin failed");

        unsafe {
            self.device
                .cmd_set_viewport(render_target.render_cb, 0, &viewport);
        }

        unsafe {
            self.device
                .cmd_set_scissor(render_target.render_cb, 0, &scissor);
        }

        drop(swapchain_objects);

        unsafe {
            render_target.perf.reset(&rri);
        }

        unsafe {
            self.device.cmd_begin_render_pass(
                render_target.render_cb,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );
        }

        unsafe {
            recorder.record(&rri);
            render_target.perf.bind(&rri);
        }
        render_target.triangles = rri.triangles.load(Ordering::SeqCst);

        unsafe {
            self.device.cmd_end_render_pass(render_target.render_cb);
        }

        unsafe { self.device.end_command_buffer(render_target.render_cb) }
            .expect("Command buffer end failed");
    }

    fn submit_update<'a>(
        &self,
        render_cb: &'a [vk::CommandBuffer],
        update_cb: &'a [vk::CommandBuffer],
        image_wait: &'a [vk::Semaphore],
        render_wait: &'a [vk::Semaphore],
        update_wait: &'a [vk::Semaphore],
        update_stage: &'a [vk::PipelineStageFlags],
        render_stage: &'a [vk::PipelineStageFlags],
        frame_fence: vk::Fence,
    ) {
        let submits = [
            vk::SubmitInfo::builder()
                .command_buffers(update_cb)
                .wait_semaphores(image_wait)
                .signal_semaphores(update_wait)
                .wait_dst_stage_mask(update_stage)
                .build(),
            vk::SubmitInfo::builder()
                .command_buffers(render_cb)
                .wait_semaphores(update_wait)
                .signal_semaphores(render_wait)
                .wait_dst_stage_mask(render_stage)
                .build(),
        ];

        unsafe {
            self.device
                .queue_submit(self.device.queues.graphics, &submits, frame_fence)
        }
        .expect("Graphics queue submit failed");
    }

    fn submit_render<'a>(
        &self,
        render_cb: &'a [vk::CommandBuffer],
        _update_cb: &'a [vk::CommandBuffer],
        image_wait: &'a [vk::Semaphore],
        render_wait: &'a [vk::Semaphore],
        _update_wait: &'a [vk::Semaphore],
        _update_stage: &'a [vk::PipelineStageFlags],
        render_stage: &'a [vk::PipelineStageFlags],
        frame_fence: vk::Fence,
    ) {
        let submits = [vk::SubmitInfo::builder()
            .command_buffers(render_cb)
            .wait_semaphores(image_wait)
            .signal_semaphores(render_wait)
            .wait_dst_stage_mask(render_stage)
            .build()];

        unsafe {
            self.device
                .queue_submit(self.device.queues.graphics, &submits, frame_fence)
        }
        .expect("Graphics queue submit failed");
    }

    fn acquire_frame_sync(&self) -> MutexGuard<FrameSync> {
        let frame = self.frame.fetch_add(1, Ordering::SeqCst) % self.frame_syncs.len();

        /* for i in 0..FRAMES_IN_FLIGHT {
            if let Some(lock) = self.frame_syncs[(frame + i) % FRAMES_IN_FLIGHT].try_lock() {
                return lock;
            }
        } */

        self.frame_syncs[frame].lock()
    }

    fn acquire_frame_image(&self, frame_sync: &MutexGuard<FrameSync>) -> usize {
        self.swapchain_objects
            .lock()
            .swapchain
            .as_ref()
            .unwrap()
            .acquire_image(frame_sync.image_semaphore, vk::Fence::null())
    }

    fn wait_for_fence(&self, fence: vk::Fence) {
        if fence == vk::Fence::null() {
            return;
        }

        let fence = [fence];
        unsafe { self.device.wait_for_fences(&fence, true, !0) }.expect("Failed to wait for fence");
    }

    fn reset_fence(&self, fence: vk::Fence) {
        assert!(fence != vk::Fence::null(), "Cannot reset a null fence");

        let fence = [fence];
        unsafe { self.device.reset_fences(&fence) }.expect("Failed to reset fence");
    }

    fn re_create_swapchain_silent(&self) -> Result<(), ContextError> {
        // lock render targets
        let render_targets = self
            .render_targets
            .iter()
            .map(|render_object| render_object.lock())
            .collect::<Vec<_>>();

        // lok swapchain
        let mut swapchain_objects = self.swapchain_objects.lock();

        let sync = swapchain_objects.swapchain.take().unwrap().sync; // take sync and drop swapchain
        let (swapchain, format, extent) = swapchain_objects.surface.build_swapchain(sync)?;

        swapchain_objects.render_pass.reset_area(extent);

        for (image, mut render_target) in swapchain.images()?.into_iter().zip(render_targets) {
            *render_target = RenderTarget::new(
                self.device.clone(),
                &swapchain_objects.render_pass,
                image,
                format,
                extent,
            )?;
        }

        swapchain_objects.swapchain = Some(swapchain);

        Ok(())
    }

    pub fn re_create_swapchain(&self) -> Result<(), ContextError> {
        self.re_create_swapchain_silent()?;
        self.request_rerecord();
        Ok(())
    }

    pub fn recreate_surface(&self, window: &Window) -> Result<(), ContextError> {
        let surface = unsafe {
            ash_window::create_surface(&self.device.entry, &self.device.instance, window, None)
        }
        .expect("Surface creation failed");

        self.swapchain_objects.lock().surface.re_create(surface);
        self.re_create_swapchain()
    }
}

impl RendererBuilder {
    /// No sync, Fifo or Mailbox
    pub fn with_sync(mut self, sync: SyncMode) -> Self {
        self.sync = sync;
        self
    }

    /// Increasing frames in flight **MIGHT** decrease the cpu frametime if the scene is simple
    ///
    /// Slightly increases input delay
    ///
    /// Recommended values: 2 or 3
    /// Default: 3
    ///
    /// 1 Never recommended and going above rarely improves anything
    pub fn with_frames_in_flight(mut self, frames_in_flight: usize) -> Self {
        self.frames_in_flight = frames_in_flight;
        self
    }

    pub fn build(self, context: Context) -> Result<Renderer, ContextError> {
        log::debug!("Renderer created");

        // device
        let (r_context, surface_builder) = ReducedContext::new(context);
        let device = RenderDevice::from_context(r_context, self.frames_in_flight)?;

        // surface
        let mut surface = surface_builder.build(device.clone());

        // swapchain
        let (swapchain, format, extent) = surface.build_swapchain(self.sync)?;
        let color_images = swapchain.images()?;

        // main render pass
        let render_pass = RenderPass::new(device.clone(), format, extent)?;

        let render_targets = color_images
            .iter()
            .map(|image| {
                Ok(Mutex::new(RenderTarget::new(
                    device.clone(),
                    &render_pass,
                    *image,
                    format,
                    extent,
                )?))
            })
            .collect::<Result<_, _>>()?;
        let rerecord_render_targets = color_images
            .into_iter()
            .map(|_| AtomicBool::new(true))
            .collect();

        let frame_syncs = (0..device.set_count)
            .map(|_| Ok(Mutex::new(FrameSync::new(device.clone())?)))
            .collect::<Result<_, _>>()?;

        let swapchain_objects = Mutex::new(SwapchainObjects {
            render_pass,
            swapchain: Some(swapchain),
            surface,
        });

        let frame = AtomicUsize::new(0);

        Ok(Renderer {
            swapchain_objects,
            render_targets,
            rerecord_render_targets,
            frame_syncs,
            frame,
            device,
        })
    }
}

impl Drop for FrameSync {
    fn drop(&mut self) {
        log::debug!("Dropping FrameSync");
        unsafe {
            self.device.destroy_fence(self.frame_done_fence, None);
            self.device.destroy_semaphore(self.image_semaphore, None);
            self.device.destroy_semaphore(self.update_semaphore, None);
            self.device.destroy_semaphore(self.render_semaphore, None);
        }
    }
}

impl Drop for RenderTarget {
    fn drop(&mut self) {
        log::debug!("Dropping RenderTarget");
        unsafe {
            let cbs = [self.render_cb, self.update_cb];
            self.device.destroy_framebuffer(self.framebuffer, None);
            self.device.free_command_buffers(self.command_pool, &cbs);
            self.device.destroy_command_pool(self.command_pool, None);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        log::debug!("Renderer dropped");
        unsafe {
            self.device.device_wait_idle().unwrap();
        }
    }
}
