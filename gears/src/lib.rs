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
use winit::event_loop::EventLoop;

pub struct Gears {
    event_loop: EventLoop<()>,
    _window: winit::window::Window,
    renderer: GearsRenderer<gfx_back::Backend>,
}

fn adapter_to_string<B: gfx_hal::Backend>(adapter: &Adapter<B>) -> String {
    format!(
        "{} (type:{})",
        adapter.info.name.cyan(),
        format!("{:?}", adapter.info.device_type).green(),
    )
}

impl Gears {
    pub fn new(width: u32, height: u32) -> Self {
        #[cfg(not(any(
            feature = "vulkan",
            feature = "dx11",
            feature = "dx12",
            feature = "metal",
            feature = "gl",
        )))]
        warn!("Empty backend will have no graphical output");

        let title = "Gears";
        let name = "gears";

        let event_loop = winit::event_loop::EventLoop::new();
        let _window = winit::window::WindowBuilder::new()
            .with_min_inner_size(winit::dpi::Size::Logical(winit::dpi::LogicalSize::new(
                64.0, 64.0,
            )))
            .with_inner_size(winit::dpi::Size::Physical(winit::dpi::PhysicalSize::new(
                width, height,
            )))
            .with_title(title)
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

        let instance = gfx_back::Instance::create(name, 1).expect("Failed to create an instance");

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
            Extent2D { width, height },
        );

        Self {
            event_loop,
            _window,
            renderer,
        }
    }

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
