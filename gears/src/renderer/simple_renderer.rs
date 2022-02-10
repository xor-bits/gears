use super::{
    device::Dev,
    query::{PerfQuery, RecordPerf},
    target::window::{SwapchainImages, WindowTarget},
    BeginInfoRecorder, Recorder,
};
use crate::{
    context::ContextError,
    frame::Frame,
    game_loop::State,
    renderer::{device::RenderDevice, target::window::WindowTargetBuilder},
};
use parking_lot::{Mutex, MutexGuard};
use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, SubpassContents,
    },
    format::{ClearValue, Format},
    image::{view::ImageView, AttachmentImage, ImageAccess, SwapchainImage},
    pipeline::graphics::viewport::{Scissor, Viewport},
    render_pass::{Framebuffer, RenderPass},
    single_pass_renderpass,
    swapchain::SwapchainAcquireFuture,
    sync::{self, FenceSignalFuture, FlushError, GpuFuture, JoinFuture},
};
use winit::window::Window;

//

struct SwapchainObjects {
    render_pass: Arc<RenderPass>,
    window_target: WindowTarget,
}

#[allow(unused)]
struct RenderTarget {
    // the actual render target
    framebuffer: Arc<Framebuffer>,

    // performance debugging
    perf: Arc<PerfQuery>,
    triangles: usize,
}

//

impl RenderTarget {
    fn new(
        device: Dev,
        render_pass: Arc<RenderPass>,
        color_image: Arc<SwapchainImage<Window>>,
    ) -> Self {
        // images
        let color_image = color_image;
        let depth_image = AttachmentImage::new(
            device.logical().clone(),
            color_image.dimensions().width_height(),
            Format::D24_UNORM_S8_UINT,
        )
        .unwrap();

        // image views
        let color_image_view = ImageView::new(color_image).unwrap();
        let depth_image_view = ImageView::new(depth_image).unwrap();

        // framebuffer
        let framebuffer = Framebuffer::start(render_pass)
            .add(color_image_view)
            .unwrap()
            .add(depth_image_view)
            .unwrap()
            .build()
            .unwrap();

        Self {
            framebuffer,

            perf: Arc::new(PerfQuery::new_with_device(&device)),
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
    frame_fences: [Option<Arc<Future>>; Renderer::frame_count()],

    pub device: Dev,
}

type Future = FenceSignalFuture<Box<dyn GpuFuture>>;

pub struct RendererBuilder<'f> {
    frame: &'f Frame,
}

#[must_use]
pub struct FrameData {
    pub recorder: Recorder<false>,
    pub viewport: Viewport,
    pub scissor: Scissor,
    pub perf: Arc<PerfQuery>,

    pub image_index: usize,
    pub frame_in_flight: usize,
    pub future: JoinFuture<Box<dyn GpuFuture>, SwapchainAcquireFuture<Window>>,
}

impl FrameData {
    pub fn cleanup_finished(&mut self) {
        self.future.cleanup_finished()
    }

    pub fn viewport_and_scissor(&self) -> (Viewport, Scissor) {
        (self.viewport.clone(), self.scissor)
    }
}

impl Renderer {
    pub fn builder(frame: &Frame) -> RendererBuilder {
        RendererBuilder { frame }
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

    pub fn begin_frame(&mut self, state: &mut State) -> FrameData {
        loop {
            match self.try_begin_frame(state) {
                Some(frame_data) => break frame_data,
                None => continue,
            }
        }
    }

    pub fn try_begin_frame(&mut self, state: &mut State) -> Option<FrameData> {
        // frame in flight can be 0 or 1
        // xor:ing with 1 swaps it between these two
        //   xor 0,0 = 0
        //   xor 1,0 = 1
        //   xor 0,1 = 1
        //   xor 1,1 = 0
        // so with fetch_xor 1 we can cycle the frame in flight
        // and get the index for this frame
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
        let (recorder, perf, gpu_time) = Self::begin_record(
            &self.device,
            &mut target.lock(),
            image_index,
            /* frame_in_flight, */
        );
        if let Some(gpu_time) = gpu_time {
            state.gpu_frame_reporter.manual(gpu_time);
        }

        // setup default dynamic state
        let extent = self.swapchain_objects.window_target.base.extent;
        let viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [extent[0] as f32, extent[1] as f32],
            depth_range: 0.0..1.0,
        };
        let scissor = Scissor::irrelevant();

        // wait for the fence set up in the last same frame_in_flight
        // waiting is necessary to unlock any resources it uses
        if let Some(fence) = self.frame_fences[frame_in_flight].as_ref() {
            fence.wait(None).unwrap();
        }

        Some(FrameData {
            recorder,
            viewport,
            scissor,
            perf,

            image_index,
            frame_in_flight,
            future,
        })
    }

    pub fn end_frame(&mut self, frame_data: FrameData) {
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
    }

    fn begin_record(
        device: &Dev,
        render_target: &mut MutexGuard<RenderTarget>,
        image_index: usize,
        /* frame_in_flight: usize, */
    ) -> (Recorder<false>, Arc<PerfQuery>, Option<Duration>) {
        // allocate a new command buffer for render calls
        let mut render_cb = AutoCommandBufferBuilder::primary(
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

        let perf = render_target.perf.clone();
        let gpu_time = perf.get();
        render_cb.reset_perf(&perf);

        // let the user record whatever
        (
            Recorder::new(
                render_cb,
                begin_render_pass_lambda,
                image_index,
                /* frame_in_flight, */
            ),
            perf,
            gpu_time,
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

impl<'f> RendererBuilder<'f> {
    pub fn build(self) -> Result<Renderer, ContextError> {
        // device
        let device = RenderDevice::from_frame(self.frame)?;

        // swapchain + images
        let (target, color_images) =
            WindowTargetBuilder::new(self.frame.surface())?.build(&device, self.frame.sync())?;

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
                    format: Format::D24_UNORM_S8_UINT,
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
        .unwrap()
    }

    fn create_render_targets(
        color_images: SwapchainImages,
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
