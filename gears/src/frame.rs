use crate::{
    context::{gpu::suitable::SuitableGPU, Context, ContextError, ContextGPUPick},
    game_loop::{Event, Loop},
    ExpectLog, SyncMode,
};
use std::{sync::Arc, time::Instant};
use vulkano::swapchain::Surface;
use vulkano_win::VkSurfaceBuild;
use winit::{
    dpi::LogicalSize,
    event::{Event as WinitEvent, WindowEvent},
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

pub struct Frame {
    context: Context,
    window: Arc<Surface<Window>>,
    p_device: Arc<SuitableGPU>,
    sync: SyncMode,

    size: (u32, u32),
    aspect: f32,

    event_loop: Option<EventLoop<()>>,
    init_timer: Instant,
}

pub struct FrameBuilder<'a> {
    context: Context,
    title: &'a str,
    size: (u32, u32),
    min_size: (u32, u32),
    max_size: Option<(u32, u32)>,
    sync: SyncMode,
}

impl Frame {
    pub fn builder<'a>(context: Context) -> FrameBuilder<'a> {
        FrameBuilder {
            context,
            title: "Gears",
            size: (600, 600),
            min_size: (32, 32),
            max_size: None,
            sync: SyncMode::Mailbox,
        }
    }

    pub fn game_loop(&mut self) -> Option<Loop> {
        Some(Loop::new(
            self.window.clone(),
            self.event_loop.take()?,
            self.init_timer,
        ))
    }

    /// Won't update unless events are sent to the surface as well
    pub const fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Won't update unless events are sent to the surface as well
    pub const fn aspect(&self) -> f32 {
        self.aspect
    }

    pub const fn sync(&self) -> SyncMode {
        self.sync
    }

    pub fn scale(&self) -> f64 {
        self.window.window().scale_factor()
    }

    pub fn window(&self) -> &Window {
        self.window.window()
    }

    pub fn surface(&self) -> Arc<Surface<Window>> {
        self.window.clone()
    }

    pub fn gpu(&self) -> Arc<SuitableGPU> {
        self.p_device.clone()
    }

    pub fn context(&self) -> Context {
        self.context.clone()
    }

    pub fn event(&mut self, event: &Event) {
        if let Event::WinitEvent(WinitEvent::WindowEvent {
            event: WindowEvent::Resized(_),
            ..
        }) = event
        {
            let (size, aspect) = Self::calc_size_and_aspect(self.window());

            self.size = size;
            self.aspect = aspect;
        }
    }

    fn calc_size_and_aspect(window: &Window) -> ((u32, u32), f32) {
        let size = window.inner_size();
        let scale = window.scale_factor();

        let size = lsize_to_tuple(size.to_logical(scale));
        let mut aspect = (size.0 as f32) / (size.1 as f32);
        aspect = if !aspect.is_finite() { 1.0 } else { aspect };

        (size, aspect)
    }
}

impl<'a> FrameBuilder<'a> {
    pub fn with_title<S: Into<&'a str>>(mut self, title: S) -> Self {
        self.title = title.into();
        self
    }

    pub const fn with_size(mut self, width: u32, height: u32) -> Self {
        self.size = (width, height);
        self
    }

    pub const fn with_min_size(mut self, width: u32, height: u32) -> Self {
        self.min_size = (width, height);
        self
    }

    pub const fn with_max_size(mut self, width: u32, height: u32) -> Self {
        self.max_size = Some((width, height));
        self
    }

    /// No sync, Fifo or Mailbox
    pub const fn with_sync(mut self, sync: SyncMode) -> Self {
        self.sync = sync;
        self
    }

    pub fn build(self) -> Result<Frame, ContextError> {
        let FrameBuilder {
            context,
            title,
            size,
            min_size,
            max_size,
            sync,
        } = self;

        // events loop
        let event_loop = EventLoop::new();

        // window info
        let mut window_builder = WindowBuilder::new()
            .with_min_inner_size(tuple_to_lsize(min_size))
            .with_inner_size(tuple_to_lsize(size))
            .with_title(title)
            .with_visible(false);
        if let Some(max_size) = max_size {
            window_builder = window_builder.with_max_inner_size(tuple_to_lsize(max_size));
        }

        // window itself
        let window = window_builder
            .build_vk_surface(&event_loop, context.instance.clone())
            .expect_log("Window creation failed");

        let (size, aspect) = Frame::calc_size_and_aspect(window.window());

        // physical device

        let p_device = Arc::new(SuitableGPU::pick(
            context.instance.clone(),
            &window,
            ContextGPUPick::default(),
        )?);

        Ok(Frame {
            context,
            window,
            p_device,
            sync,

            size,
            aspect,

            event_loop: Some(event_loop),
            init_timer: Instant::now(),
        })
    }
}

const fn tuple_to_lsize(size: (u32, u32)) -> LogicalSize<u32> {
    LogicalSize {
        width: size.0,
        height: size.1,
    }
}

const fn lsize_to_tuple(size: LogicalSize<u32>) -> (u32, u32) {
    (size.width, size.height)
}
