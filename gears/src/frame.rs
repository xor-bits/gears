use std::{env, sync::Arc};

use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::WindowEvent,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use crate::{
    context::{Context, ContextError, ContextGPUPick, ContextValidation},
    ExpectLog,
};

pub struct Frame {
    window: Arc<Window>,
    size: (u32, u32),
    aspect: f32,
}

pub struct FrameBuilder<'a> {
    title: &'a str,
    size: (u32, u32),
    min_size: (u32, u32),
    max_size: Option<(u32, u32)>,
}

impl Frame {
    pub const fn new() -> FrameBuilder<'static> {
        FrameBuilder {
            title: "Gears",
            size: (600, 600),
            min_size: (32, 32),
            max_size: None,
        }
    }

    ///
    pub fn default_context(&self) -> Result<Context, ContextError> {
        Context::new(
            self.window.clone(),
            ContextGPUPick::default(),
            ContextValidation::default(),
        )
    }

    /// Environment value `GEARS_GPU_PICK` overrides the `pick` argument if present.
    /// Possible values: `auto`, `pick`
    ///
    /// Environment value `GEARS_VALIDATION` overrides the `valid` argument if present.
    /// Possible values: `none`, `full`
    pub fn context(
        &self,
        pick: ContextGPUPick,
        valid: ContextValidation,
    ) -> Result<Context, ContextError> {
        let pick = env::var("GEARS_GPU_PICK").map_or(pick, |value| {
            let valid = match value.to_lowercase().as_str() {
                "auto" => ContextGPUPick::Automatic,
                "pick" => ContextGPUPick::Manual,
                other => {
                    log::warn!("Ignored invalid value: {}", other);
                    pick
                }
            };

            log::info!("Using override ContextGPUPick: {:?}", valid);
            valid
        });

        let valid = env::var("GEARS_VALIDATION").map_or(valid, |value| {
            let valid = match value.to_lowercase().as_str() {
                "full" => ContextValidation::WithValidation,
                "none" => ContextValidation::NoValidation,
                other => {
                    log::warn!("Ignored invalid value: {}", other);
                    return valid;
                }
            };

            log::info!("Using override ContextValidation: {:?}", valid);
            valid
        });

        Context::new(self.window.clone(), pick, valid)
    }

    pub const fn size(&self) -> (u32, u32) {
        self.size
    }

    pub const fn aspect(&self) -> f32 {
        self.aspect
    }

    pub fn scale(&self) -> f64 {
        self.window.scale_factor()
    }

    pub const fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                let (size, aspect) =
                    Self::calc_size_and_aspect(size.clone(), self.window.scale_factor());

                self.size = size;
                self.aspect = aspect;
            }
            _ => {}
        }
    }

    fn calc_size_and_aspect(size: PhysicalSize<u32>, scale: f64) -> ((u32, u32), f32) {
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

    pub fn build(self) -> (Frame, EventLoop<()>) {
        // events loop
        let event_loop = EventLoop::new();

        // window info
        let mut window_builder = WindowBuilder::new()
            .with_min_inner_size(tuple_to_lsize(self.min_size))
            .with_inner_size(tuple_to_lsize(self.size));
        if let Some(max_size) = self.max_size {
            window_builder = window_builder.with_max_inner_size(tuple_to_lsize(max_size));
        }

        // window itself
        let window = Arc::new(
            window_builder
                .with_title(self.title)
                .build(&event_loop)
                .expect_log("Window creation failed"),
        );

        let (size, aspect) =
            Frame::calc_size_and_aspect(window.inner_size(), window.scale_factor());

        (
            Frame {
                window,
                size,
                aspect,
            },
            event_loop,
        )
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
