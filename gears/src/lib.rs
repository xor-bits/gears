#[cfg(feature = "dx11")]
use gfx_backend_dx11 as gfx_back;
#[cfg(feature = "dx12")]
use gfx_backend_dx12 as gfx_back;
#[cfg(not(any(
    feature = "vulkan",
    feature = "dx11",
    feature = "dx12",
    feature = "metal",
    feature = "gl",
)))]
use gfx_backend_empty as gfx_back;
#[cfg(feature = "gl")]
use gfx_backend_gl as gfx_back;
#[cfg(feature = "metal")]
use gfx_backend_metal as gfx_back;
#[cfg(feature = "vulkan")]
use gfx_backend_vulkan as gfx_back;

//
mod renderer;

use log::*;
use renderer::{queue::QueueFamilies, GearsRenderer};

use colored::Colorize;
use gfx_hal::{adapter::Adapter, window::Extent2D, Instance};
use winit::{dpi::LogicalSize, event_loop::EventLoop};

#[derive(Debug, PartialEq, Eq)]
pub enum VSync {
    Off,
    On,
}

pub trait Application {}

pub struct GearsBuilder {
    title: String,

    min_size: LogicalSize<u32>,
    initial_size: LogicalSize<u32>,
    max_size: Option<LogicalSize<u32>>,

    vsync: VSync,
}

pub struct Gears {
    event_loop: EventLoop<()>,
    _window: winit::window::Window,
    renderer: GearsRenderer<gfx_back::Backend>,
}

impl Default for GearsBuilder {
    fn default() -> Self {
        Self {
            title: "Gears".into(),
            min_size: LogicalSize::new(64, 64),
            initial_size: LogicalSize::new(600, 600),
            max_size: None,

            vsync: VSync::On,
        }
    }
}

impl GearsBuilder {
    pub fn new() -> Self {
        GearsBuilder::default()
    }

    pub fn with_title<S: Into<String>>(mut self, title: S) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_min_size(mut self, width: u32, height: u32) -> Self {
        self.min_size = LogicalSize::new(width, height);
        self
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.initial_size = LogicalSize::new(width, height);
        self
    }

    pub fn with_max_size(mut self, width: u32, height: u32) -> Self {
        self.max_size = Some(LogicalSize::new(width, height));
        self
    }

    pub fn with_vsync(mut self, vsync: VSync) -> Self {
        self.vsync = vsync;
        self
    }

    pub fn build(self) -> Gears {
        #[cfg(not(any(
            feature = "vulkan",
            feature = "dx11",
            feature = "dx12",
            feature = "metal",
            feature = "gl",
        )))]
        warn!("Empty backend will have no graphical output");

        let event_loop = winit::event_loop::EventLoop::new();
        let mut window_builder = winit::window::WindowBuilder::new()
            .with_min_inner_size(self.min_size)
            .with_inner_size(self.initial_size);
        if let Some(max_size) = self.max_size {
            window_builder = window_builder.with_max_inner_size(max_size);
        }
        let _window = window_builder
            .with_title(self.title)
            .build(&event_loop)
            .unwrap();

        #[cfg(target_arch = "wasm32")]
        web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .body()
            .unwrap()
            .append_child(&winit::platform::web::WindowExtWebSys::canvas(&_window))
            .unwrap();

        let instance =
            gfx_back::Instance::create("gears", 1).expect("Failed to create an instance");

        let surface = unsafe {
            instance
                .create_surface(&_window)
                .expect("Failed to create a surface")
        };

        let mut adapter_names = Vec::new();
        let adapter = {
            let mut suitable_adapters = instance
                .enumerate_adapters()
                .into_iter()
                .filter_map(|adapter| {
                    let queue_families = QueueFamilies::new(&surface, &adapter);
                    adapter_names.push(adapter.info.name.clone());
                    if !queue_families.finished() {
                        None
                    } else {
                        let device_type = adapter.info.device_type.clone();
                        Some((
                            adapter,
                            queue_families,
                            match device_type {
                                gfx_hal::adapter::DeviceType::DiscreteGpu => 4,
                                gfx_hal::adapter::DeviceType::IntegratedGpu => 3,
                                gfx_hal::adapter::DeviceType::VirtualGpu => 2,
                                gfx_hal::adapter::DeviceType::Cpu => 1,
                                gfx_hal::adapter::DeviceType::Other => 0,
                            },
                        ))
                    }
                })
                .collect::<Vec<_>>();

            suitable_adapters.sort_by(|lhs, rhs| rhs.2.cmp(&lhs.2));
            if suitable_adapters.len() == 0 {
                None
            } else {
                Some(suitable_adapters.remove(0))
            }
        };
        let (adapter, queue_families, _) = adapter.expect("No suitable GPUs");
        info!("Selected GPU: {}", adapter_to_string(&adapter));

        let renderer = GearsRenderer::new(
            instance,
            surface,
            adapter,
            queue_families,
            Extent2D {
                width: self.initial_size.width,
                height: self.initial_size.height,
            },
            self.vsync == VSync::On,
        );

        Gears {
            event_loop,
            _window,
            renderer,
        }
    }
}

fn adapter_to_string<B: gfx_hal::Backend>(adapter: &Adapter<B>) -> String {
    format!(
        "{} (type:{})",
        adapter.info.name.cyan(),
        format!("{:?}", adapter.info.device_type).green(),
    )
}

impl Gears {
    pub fn run(self) {
        let mut renderer = self.renderer;
        renderer.render();

        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Poll;

            match event {
                winit::event::Event::WindowEvent { event, .. } => match event {
                    winit::event::WindowEvent::CloseRequested => {
                        *control_flow = winit::event_loop::ControlFlow::Exit
                    }
                    winit::event::WindowEvent::KeyboardInput {
                        input:
                            winit::event::KeyboardInput {
                                virtual_keycode: Some(winit::event::VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = winit::event_loop::ControlFlow::Exit,
                    winit::event::WindowEvent::Resized(dims) => {
                        debug!("resized to {:?}", dims);
                        renderer.dimensions = Extent2D {
                            width: dims.width,
                            height: dims.height,
                        };
                        renderer.recreate_swapchain();
                    }
                    _ => {}
                },
                winit::event::Event::RedrawEventsCleared => {
                    renderer.render();
                }
                _ => {}
            }
        });
    }
}
