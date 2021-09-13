use std::{env, sync::Arc};

use winit::{
    dpi::LogicalSize,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use crate::{
    context::{Context, ContextError, ContextGPUPick, ContextValidation},
    ExpectLog,
};

pub struct Frame {
    window: Arc<Window>,
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

    pub fn scale(&self) -> f64 {
        self.window.scale_factor()
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
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
        let window = Arc::new(
            window_builder
                .with_title(self.title)
                .build(&event_loop)
                .expect_log("Window creation failed"),
        );

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
