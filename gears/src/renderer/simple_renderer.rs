use super::{device::Dev, query::PerfQuery, target::window::WindowTarget};
use crate::{
    context::{Context, ContextError, ContextValidation},
    cstr,
    renderer::device::{ReducedContext, RenderDevice},
    SyncMode,
};
use parking_lot::Mutex;
use std::sync::{atomic::AtomicBool, Arc};
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, SubpassContents,
    },
    format::ClearValue,
    image::{view::ImageView, SwapchainImage},
    render_pass::{Framebuffer, FramebufferAbstract, RenderPass},
    single_pass_renderpass,
    sync::{self, GpuFuture},
};
use winit::window::Window;

struct SwapchainObjects {
    render_pass: Arc<RenderPass>,
    window_target: WindowTarget,
}

struct RenderTarget {
    // the actual render target
    framebuffer: Arc<dyn FramebufferAbstract>,

    // gpu commands
    render_cb: Arc<PrimaryAutoCommandBuffer>,
    update_cb: Arc<PrimaryAutoCommandBuffer>,
    update_cb_recording: bool,
    update_cb_pending: bool,

    // performance debugging
    perf: PerfQuery,
    triangles: usize,
}

impl RenderTarget {
    fn new(
        device: Dev,
        render_pass: Arc<RenderPass>,
        color_image: Arc<SwapchainImage<Arc<Window>>>,
        validation: ContextValidation,
    ) -> Self {
        // images
        let color_image = color_image;
        /* let depth_image = AttachmentImage::new(
            device.logical().clone(),
            color_image.dimensions(),
            Format::D24Unorm_S8Uint,
        )
        .unwrap(); */

        // image views
        let color_image = ImageView::new(color_image).unwrap();
        /* let depth_image = ImageView::new(depth_image).unwrap(); */

        // framebuffer
        let framebuffer = Arc::new(
            Framebuffer::start(render_pass)
                .add(color_image)
                .unwrap()
                /* .add(depth_image)
                .unwrap() */
                .build()
                .unwrap(),
        );

        // command buffer for copying, etc.
        let update_cb = Arc::new(
            AutoCommandBufferBuilder::primary(
                device.logical().clone(),
                device.queues.graphics.family(),
                CommandBufferUsage::SimultaneousUse, /* MultipleSubmit */
            )
            .unwrap()
            .build()
            .unwrap(),
        );

        // command buffer for drawing
        let mut render_cb = AutoCommandBufferBuilder::primary(
            device.logical().clone(),
            device.queues.graphics.family(),
            CommandBufferUsage::SimultaneousUse, /* MultipleSubmit */
        )
        .unwrap();

        // record some initial commands
        render_cb
            .begin_render_pass(
                framebuffer.clone(),
                SubpassContents::Inline,
                [
                    ClearValue::Float([0.0, 0.0, 0.0, 0.0]),
                    /* ClearValue::DepthStencil((1.0, 0)), */
                ]
                .iter()
                .cloned(),
            )
            .unwrap();
        if validation == ContextValidation::WithValidation {
            render_cb
                .debug_marker_insert(
                    cstr!("Empty render command buffer wasn't meant to be used"),
                    [1.0, 0.7, 0.0, 1.0],
                )
                .unwrap();
        }
        render_cb.end_render_pass().unwrap();

        let render_cb = Arc::new(render_cb.build().unwrap());

        Self {
            framebuffer,

            render_cb,
            update_cb,
            update_cb_recording: false,
            update_cb_pending: false,

            perf: PerfQuery::new_with_device(&device),
            triangles: 0,
        }
    }
}

pub struct Renderer {
    swapchain_objects: Mutex<SwapchainObjects>,

    // one render target per swapchain image
    // and a bool to tell to rerecord the render command buffers
    render_targets: Box<[(Mutex<RenderTarget>, AtomicBool)]>,

    // future for the previous frame
    previous_frame: Option<Box<dyn GpuFuture>>,

    validation: ContextValidation,

    pub device: Dev,
}

pub struct RendererBuilder {
    sync: SyncMode,
}

impl Renderer {
    pub fn new() -> RendererBuilder {
        RendererBuilder {
            sync: SyncMode::Mailbox,
        }
    }

    pub fn render_pass(&self) -> Arc<RenderPass> {
        self.swapchain_objects.lock().render_pass.clone()
    }

    /// swapchain images
    pub fn image_count(&self) -> usize {
        self.render_targets.len()
    }

    pub fn frame(&mut self) {
        self.previous_frame.as_mut().unwrap().cleanup_finished();

        let swapchain = self.swapchain_objects.lock();
        let (image_index, acquire_future) = match swapchain.window_target.acquire_image() {
            Some(v) => v,
            None => return,
        };

        let target = self.render_targets[image_index].0.lock();

        let future = self
            .previous_frame
            .take()
            .unwrap()
            .join(acquire_future)
            .then_execute(
                self.device.queues.graphics.clone(),
                target.render_cb.clone(),
            )
            .unwrap()
            .then_swapchain_present(
                self.device.queues.present.clone(),
                swapchain.window_target.swapchain.clone(),
                image_index,
            )
            .then_signal_fence_and_flush();

        self.previous_frame = match future {
            Ok(future) => Some(future.boxed()),
            Err(e) => {
                log::error!("Frame error: {}", e);
                Some(sync::now(self.device.logical().clone()).boxed())
            }
        }
    }

    /* pub fn frame<T>(&self, recorder: &T) -> FramePerfReport
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
        let gpu_frametime = render_target
            .perf
            .get()
            .unwrap_or(PerfQueryResult::default());

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
            gpu_frametime: gpu_frametime,

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
        if render_target.update_cb_recording == false {
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
                .command_buffers(&update_cb)
                .wait_semaphores(&image_wait)
                .signal_semaphores(&update_wait)
                .wait_dst_stage_mask(&update_stage)
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
            .command_buffers(&render_cb)
            .wait_semaphores(&image_wait)
            .signal_semaphores(&render_wait)
            .wait_dst_stage_mask(&render_stage)
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
    } */
}

impl RendererBuilder {
    /// No sync, Fifo or Mailbox
    pub fn with_sync(mut self, sync: SyncMode) -> Self {
        self.sync = sync;
        self
    }

    pub fn build(self, context: Context) -> Result<Renderer, ContextError> {
        log::debug!("Renderer created");

        let validation = context.validation;

        // device
        let (r_context, target_builder) = ReducedContext::new(context);
        let device = RenderDevice::from_context(r_context)?;

        // surface + swapchain + images
        let (target, color_images) = target_builder.build(&device, self.sync)?;

        // swapchain image count
        let image_count = color_images.len();
        // swapchain image count - 1
        // let frame_count = 1.max(image_count - 1);

        assert!(image_count != 0);

        // AttachmentDesc

        // main render pass
        let render_pass = Arc::new(
            single_pass_renderpass!(device.logical().clone(),
                attachments: {
                    c: {
                        load: Clear,
                        store: Store,
                        format: target.format.0,
                        samples: 1,
                        initial_layout: ImageLayout::Undefined,
                        final_layout: ImageLayout::PresentSrc,
                    }/* ,
                    d: {
                        load: Clear,
                        store: DontCare,
                        format: Format::D24Unorm_S8Uint,
                        samples: 1,
                        initial_layout: ImageLayout::Undefined,
                        final_layout: ImageLayout::DepthStencilAttachmentOptimal,
                    } */
                },
                pass: {
                    color: [ c ],
                    depth_stencil: { /* d */ }
                }
            )
            .unwrap(),
        );

        let render_targets = color_images
            .iter()
            .map(|image| {
                (
                    Mutex::new(RenderTarget::new(
                        device.clone(),
                        render_pass.clone(),
                        image.clone(),
                        validation,
                    )),
                    AtomicBool::new(true),
                )
            })
            .collect::<Box<[(Mutex<RenderTarget>, AtomicBool)]>>();

        let swapchain_objects = Mutex::new(SwapchainObjects {
            render_pass,
            window_target: target,
        });

        let previous_frame = Some(sync::now(device.logical().clone()).boxed());

        Ok(Renderer {
            swapchain_objects,

            render_targets,

            previous_frame,

            validation,

            device,
        })
    }
}
