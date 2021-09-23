use super::{
    device::Dev, query::PerfQuery, target::window::WindowTarget, BeginInfoRecorder,
    FramePerfReport, Recorder,
};
use crate::{
    context::{Context, ContextError},
    renderer::{
        device::{ReducedContext, RenderDevice},
        query::PerfQueryResult,
    },
    SyncMode,
};
use parking_lot::{Mutex, MutexGuard};
use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::Instant,
};
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, DynamicState, PrimaryAutoCommandBuffer,
        SubpassContents,
    },
    format::{ClearValue, Format},
    image::{view::ImageView, AttachmentImage, SwapchainImage},
    pipeline::viewport::Viewport,
    render_pass::{Framebuffer, FramebufferAbstract, RenderPass},
    single_pass_renderpass,
    swapchain::SwapchainAcquireFuture,
    sync::{self, FenceSignalFuture, FlushError, GpuFuture, JoinFuture},
};
use winit::window::Window;

struct SwapchainObjects {
    render_pass: Arc<RenderPass>,
    window_target: WindowTarget,
}

#[allow(unused)]
struct RenderTarget {
    // the actual render target
    framebuffer: Arc<dyn FramebufferAbstract + Send + Sync>,

    // performance debugging
    perf: PerfQuery,
    triangles: usize,
}

impl RenderTarget {
    fn new(
        device: Dev,
        render_pass: Arc<RenderPass>,
        color_image: Arc<SwapchainImage<Arc<Window>>>,
    ) -> Self {
        // images
        let color_image = color_image;
        let depth_image = AttachmentImage::new(
            device.logical().clone(),
            color_image.dimensions(),
            Format::D24Unorm_S8Uint,
        )
        .unwrap();

        // image views
        let color_image_view = ImageView::new(color_image.clone()).unwrap();
        let depth_image_view = ImageView::new(depth_image).unwrap();

        // framebuffer
        let framebuffer = Arc::new(
            Framebuffer::start(render_pass)
                .add(color_image_view)
                .unwrap()
                .add(depth_image_view)
                .unwrap()
                .build()
                .unwrap(),
        );

        Self {
            framebuffer,

            perf: PerfQuery::new_with_device(&device),
            triangles: 0,
        }
    }
}

pub struct Renderer {
    swapchain_objects: SwapchainObjects,

    // one render target per swapchain image
    render_targets: Box<[Arc<Mutex<RenderTarget>>]>,

    // future for the previous frame
    previous_frame: Option<Box<dyn GpuFuture>>,

    frame_in_flight: AtomicU8,
    frame_fences: [Option<Arc<FenceSignalFuture<Box<dyn GpuFuture>>>>; Renderer::frame_count()],

    pub device: Dev,
}

pub struct RendererBuilder {
    sync: SyncMode,
}

#[must_use]
pub struct FrameData {
    pub recorder: Recorder<false>,
    pub dynamic: DynamicState,

    pub image_index: usize,
    pub frame_in_flight: usize,
    pub future: JoinFuture<Box<dyn GpuFuture>, SwapchainAcquireFuture<Arc<Window>>>,

    beginning: Instant,
}

impl FrameData {
    pub fn cleanup_finished(&mut self) {
        self.future.cleanup_finished()
    }
}

impl Renderer {
    pub fn new() -> RendererBuilder {
        RendererBuilder {
            sync: SyncMode::Mailbox,
        }
    }

    pub fn render_pass(&self) -> Arc<RenderPass> {
        self.swapchain_objects.render_pass.clone()
    }

    /// Swapchain images.
    pub fn image_count(&self) -> usize {
        self.render_targets.len()
    }

    /// Frames in flight.
    /// Any changing buffers should have this many duplicates.
    /// This count is always two.
    pub const fn frame_count() -> usize {
        2
    }

    pub fn begin_frame(&mut self) -> Option<FrameData> {
        let beginning = Instant::now();

        // frame in flight can be 0 or 1
        // xor:ing with 1 swaps it between these two
        //   xor 0,0 = 0
        //   xor 1,0 = 1
        //   xor 0,1 = 1
        //   xor 1,1 = 0
        // so with fetch_xor 1 we can cycle the frame in flight
        // and get the index for this frame
        // *my longest and the most in detail comment so far*
        let frame_in_flight = self.frame_in_flight.fetch_xor(1, Ordering::SeqCst) as usize;
        assert!((0..=1).contains(&frame_in_flight));

        self.previous_frame.as_mut().unwrap().cleanup_finished();

        // acquire the target image (future) and its index
        let (image_index, acquire_future) =
            match self.swapchain_objects.window_target.acquire_image() {
                Some(v) => v,
                None => {
                    self.recreate_swapchain().unwrap();
                    return None;
                }
            };

        // join the last frame and this frame
        let future = self.previous_frame.take().unwrap().join(acquire_future);

        // objects to render to
        let target = &self.render_targets[image_index];

        // begin recording a render command buffer
        let recorder = Self::begin_record(
            &self.device,
            &mut target.lock(),
            image_index,
            /* frame_in_flight, */
        );

        // setup default dynamic state
        let extent = self.swapchain_objects.window_target.base.extent;
        let dynamic = DynamicState {
            viewports: Some(vec![Viewport {
                origin: [0.0, 0.0],
                dimensions: [extent[0] as f32, extent[1] as f32],
                depth_range: 0.0..1.0,
            }]),
            ..DynamicState::none()
        };

        // wait for the fence set up in the last same frame_in_flight
        // waiting is necessary to unlock any resources it uses
        if let Some(fence) = self.frame_fences[frame_in_flight].as_ref() {
            fence.wait(None).unwrap();
        }

        Some(FrameData {
            recorder,
            dynamic,

            image_index,
            frame_in_flight,
            future,

            beginning,
        })
    }

    pub fn end_frame(&mut self, frame_data: FrameData) -> Option<FramePerfReport> {
        // end recording
        let cb = Self::end_record(frame_data.recorder);

        // rendering
        // signal fence to wait for unlocking resources
        // wrap to Arc so that it can be cloned
        let future = Arc::new(
            frame_data
                .future
                .then_execute(self.device.queues.graphics.clone(), cb)
                .unwrap()
                .boxed()
                .then_signal_fence(),
        );
        // store the fence and wait for it the next time this same frame_in_flight is used
        self.frame_fences[frame_data.frame_in_flight] = Some(future.clone());

        // presenting
        let future = future
            .then_swapchain_present(
                self.device.queues.present.clone(),
                self.swapchain_objects.window_target.swapchain.clone(),
                frame_data.image_index,
            )
            .then_signal_fence_and_flush();

        // handle window resize and print any other error
        match future {
            Ok(future) => self.previous_frame = Some(future.boxed()),
            Err(FlushError::OutOfDate) => (),
            Err(err) => log::error!("Frame error: {}", err),
        }

        if self.previous_frame.is_none() {
            self.previous_frame = Some(sync::now(self.device.logical().clone()).boxed())
        }

        Some(FramePerfReport {
            cpu_frame_time: frame_data.beginning.elapsed(),
            gpu_frame_time: /* TODO */ PerfQueryResult::default(),
        })
    }

    fn begin_record(
        device: &Dev,
        render_target: &mut MutexGuard<RenderTarget>,
        image_index: usize,
        /* frame_in_flight: usize, */
    ) -> Recorder<false> {
        // allocate a new command buffer for render calls
        let render_cb = AutoCommandBufferBuilder::primary(
            device.logical().clone(),
            device.queues.graphics.family(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let fb = render_target.framebuffer.clone();
        let begin_render_pass_lambda = move |(cb, cc): BeginInfoRecorder| {
            cb.begin_render_pass(
                fb.clone(),
                SubpassContents::Inline,
                [
                    ClearValue::Float(cc.c()), // cc.c is `clear color get color`, clearly
                    ClearValue::DepthStencil((1.0, 0)),
                ]
                .iter()
                .cloned(),
            )
            .unwrap();
        };

        // let the user record whatever
        Recorder::new(
            render_cb,
            begin_render_pass_lambda,
            image_index,
            /* frame_in_flight, */
        )
    }

    fn end_record(recorder: Recorder<false>) -> PrimaryAutoCommandBuffer {
        // end, build and return the command buffer
        recorder.inner.command_buffer.build().unwrap()
    }

    fn recreate_swapchain(&mut self) -> Result<(), ContextError> {
        let color_images = self.swapchain_objects.window_target.recreate()?;

        self.render_targets = RendererBuilder::create_render_targets(
            color_images,
            &self.device,
            &self.swapchain_objects.render_pass,
        );

        Ok(())
    }
}

impl RendererBuilder {
    /// No sync, Fifo or Mailbox
    pub fn with_sync(mut self, sync: SyncMode) -> Self {
        self.sync = sync;
        self
    }

    pub fn build(self, context: Context) -> Result<Renderer, ContextError> {
        // device
        let (r_context, target_builder) = ReducedContext::new(context);
        let device = RenderDevice::from_context(r_context)?;

        // surface + swapchain + images
        let (target, color_images) = target_builder.build(&device, self.sync)?;

        // main render pass
        let render_pass = Self::create_render_pass(&device, &target);

        // render targets (framebuffers, command buffers, ...)
        let render_targets = Self::create_render_targets(color_images, &device, &render_pass);

        // swapchain + renderpass
        let swapchain_objects = SwapchainObjects {
            render_pass,
            window_target: target,
        };

        let previous_frame = Some(sync::now(device.logical().clone()).boxed());
        let frame_in_flight = AtomicU8::new(0);
        let frame_fences = [None, None];

        log::debug!("Renderer created");

        Ok(Renderer {
            swapchain_objects,

            render_targets,

            previous_frame,

            frame_in_flight,
            frame_fences,

            device,
        })
    }

    fn create_render_pass(device: &Dev, target: &WindowTarget) -> Arc<RenderPass> {
        // AttachmentDesc

        Arc::new(
            single_pass_renderpass!(device.logical().clone(),
                attachments: {
                    c: {
                        load: Clear,
                        store: Store,
                        format: target.format.0,
                        samples: 1,
                        initial_layout: ImageLayout::Undefined,
                        final_layout: ImageLayout::PresentSrc,
                    },
                    d: {
                        load: Clear,
                        store: DontCare,
                        format: Format::D24Unorm_S8Uint,
                        samples: 1,
                        initial_layout: ImageLayout::Undefined,
                        final_layout: ImageLayout::DepthStencilAttachmentOptimal,
                    }
                },
                pass: {
                    color: [ c ],
                    depth_stencil: { d }
                }
            )
            .unwrap(),
        )
    }

    fn create_render_targets(
        color_images: Vec<Arc<SwapchainImage<Arc<Window>>>>,
        device: &Dev,
        render_pass: &Arc<RenderPass>,
    ) -> Box<[Arc<Mutex<RenderTarget>>]> {
        color_images
            .iter()
            .map(|image| {
                Arc::new(Mutex::new(RenderTarget::new(
                    device.clone(),
                    render_pass.clone(),
                    image.clone(),
                )))
            })
            .collect()
    }
}
