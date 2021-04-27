use winit::{
    dpi::LogicalSize,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use crate::{context::Context, ContextError, ExpectLog};

pub struct Frame {
    window: Window,
}

pub struct FrameBuilder<'a> {
    title: &'a str,
    size: (u32, u32),
    min_size: (u32, u32),
    max_size: Option<(u32, u32)>,
}

impl Frame {
    pub fn new<'a>() -> FrameBuilder<'a> {
        FrameBuilder::<'a> {
            title: "Gears",
            size: (600, 600),
            min_size: (64, 16),
            max_size: None,
        }
    }

    pub fn context(&self) -> Result<Context, ContextError> {
        Context::new(&self.window, self.size())
    }

    pub fn size(&self) -> (u32, u32) {
        lsize_to_tuple(
            self.window
                .inner_size()
                .to_logical(self.window.scale_factor()),
        )
    }

    pub fn aspect(&self) -> f32 {
        let size = self.window.inner_size();

        if size.height == 0 {
            1.0
        } else {
            (size.width as f32) / (size.height as f32)
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn window_mut(&mut self) -> &mut Window {
        &mut self.window
    }
}

impl<'a> FrameBuilder<'a> {
    pub fn with_title<S: Into<&'a str>>(mut self, title: S) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.size = (width, height);
        self
    }

    pub fn with_min_size(mut self, width: u32, height: u32) -> Self {
        self.min_size = (width, height);
        self
    }

    pub fn with_max_size(mut self, width: u32, height: u32) -> Self {
        self.max_size = Some((width, height));
        self
    }

    pub fn build(self) -> (Frame, EventLoop<()>) {
        let event_loop = EventLoop::new();
        let mut window_builder = WindowBuilder::new()
            .with_min_inner_size(tuple_to_lsize(self.min_size))
            .with_inner_size(tuple_to_lsize(self.size));
        if let Some(max_size) = self.max_size {
            window_builder = window_builder.with_max_inner_size(tuple_to_lsize(max_size));
        }
        let window = window_builder
            .with_title(self.title)
            .build(&event_loop)
            .expect_log("Window creation failed");

        (Frame { window }, event_loop)
    }
}

fn tuple_to_lsize(size: (u32, u32)) -> LogicalSize<u32> {
    LogicalSize {
        width: size.0,
        height: size.1,
    }
}

fn lsize_to_tuple(size: LogicalSize<u32>) -> (u32, u32) {
    (size.width, size.height)
}
