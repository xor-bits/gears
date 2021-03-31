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
pub mod input_state;
pub mod renderer;

use input_state::InputState;
use instant::{Duration, Instant};
use log::*;
use parking_lot::Mutex;
use renderer::{queue::QueueFamilies, FrameInfo};

use colored::Colorize;
use gfx_hal::{adapter::Adapter, window::Extent2D, Instance};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    window::Window,
};

pub use renderer::{FrameCommands, GearsRenderer};

pub type B = gfx_back::Backend;

#[derive(Debug, PartialEq, Eq)]
pub enum VSync {
    Off,
    On,
}

#[derive(Debug, Clone, Copy)]
pub struct UPS {
    pub updates_per_second: u32,
    pub update_time: Duration,

    _p: (),
}

impl PartialEq for UPS {
    fn eq(&self, other: &Self) -> bool {
        self.updates_per_second == other.updates_per_second
    }
}

impl Eq for UPS {}

impl Hash for UPS {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.updates_per_second)
    }
}

impl UPS {
    pub fn new(updates_per_second: u32) -> Self {
        if updates_per_second == 0 {
            error!("updates_per_second cannot be 0");
            panic!();
        }

        Self {
            updates_per_second,
            update_time: Duration::from_secs_f64(1.0).div_f64(updates_per_second as f64),

            _p: (),
        }
    }
}

pub trait Application {
    fn init(input_state: InputState, gears: &mut GearsRenderer<B>) -> Self;

    fn event(&mut self, event: &WindowEvent, window: &Window, gears: &mut GearsRenderer<B>);

    fn render(&mut self, frame_info: &mut FrameInfo<B>, update_tps: &HashMap<UPS, Instant>);

    fn update(&mut self, ups_thread_id: &UPS);
}

pub struct NoApplication {}

pub struct Gears {
    title: String,

    min_size: LogicalSize<u32>,
    initial_size: LogicalSize<u32>,
    max_size: Option<LogicalSize<u32>>,

    vsync: VSync,

    ups: HashSet<UPS>,
}

impl Application for NoApplication {
    fn init(_: InputState, _: &mut GearsRenderer<B>) -> Self {
        Self {}
    }

    fn event(&mut self, _: &WindowEvent, _: &Window, _: &mut GearsRenderer<B>) {}

    fn render(&mut self, _: &mut FrameInfo<B>, _: &HashMap<UPS, Instant>) {}

    fn update(&mut self, _: &UPS) {}
}

impl Default for Gears {
    fn default() -> Self {
        Self {
            title: "Gears".into(),
            min_size: LogicalSize::new(64, 64),
            initial_size: LogicalSize::new(600, 600),
            max_size: None,

            vsync: VSync::On,

            ups: HashSet::new(),
        }
    }
}

impl<'a> Gears {
    pub fn new() -> Self {
        Gears::default()
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

    pub fn with_ups(mut self, ups: UPS) -> Self {
        self.ups.insert(ups);
        self
    }

    pub fn run_with<A: Application + 'static + Send>(self) {
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
        let window = window_builder
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
            .append_child(&winit::platform::web::WindowExtWebSys::canvas(&window))
            .unwrap();

        let instance =
            gfx_back::Instance::create("gears", 1).expect("Failed to create an instance");

        let surface = unsafe {
            instance
                .create_surface(&window)
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

        let mut renderer = GearsRenderer::new(
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
        let input = InputState::new(
            false,
            self.initial_size,
            self.initial_size.to_physical(window.scale_factor()),
        );
        let mut application = Some(Arc::new(Mutex::new(A::init(input, &mut renderer))));
        let running = Arc::new(AtomicBool::new(true));

        let thread_tps = Arc::new(Mutex::new(HashMap::new()));
        let mut threads = self
            .ups
            .into_iter()
            .map(|u| {
                let app = application.as_ref().unwrap().clone();
                let running = running.clone();
                thread_tps.lock().insert(u, Instant::now());
                let thread_tps = thread_tps.clone();
                (
                    u,
                    thread::spawn(move || {
                        let time_zero: Duration = Duration::from_secs_f64(0.0);
                        let mut lag = time_zero;

                        while running.load(Ordering::Relaxed) {
                            let begin = Instant::now();
                            thread_tps.lock().insert(u, begin);

                            app.lock().update(&u);

                            let update_time = begin.elapsed();
                            if update_time <= u.update_time {
                                let time_left = u.update_time - update_time;
                                // there is leftover time
                                if lag <= time_zero {
                                    // no lag to reduce
                                    thread::sleep(time_left);
                                } else {
                                    // lag to be reduced
                                    if time_left >= lag {
                                        // can be fixed in a single update
                                        thread::sleep(time_left - lag);
                                        lag = time_zero;
                                    } else {
                                        // cannot --
                                        lag -= time_left;
                                    }
                                }
                            } else {
                                // falling behind
                                lag += update_time - u.update_time;
                                thread::sleep(u.update_time);
                                warn!("{:?} is behind: {} seconds", u, lag.as_secs());
                            }
                        }
                    }),
                )
            })
            .collect::<HashMap<_, _>>();

        event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Poll;

            match event {
                Event::WindowEvent { event, .. } => {
                    application
                        .as_ref()
                        .unwrap()
                        .lock()
                        .event(&event, &window, &mut renderer);

                    match event {
                        WindowEvent::CloseRequested => {
                            *control_flow = winit::event_loop::ControlFlow::Exit
                        }
                        WindowEvent::Resized(dims) => {
                            let dims = dims.to_logical(window.scale_factor());

                            renderer.dimensions = Extent2D {
                                width: dims.width,
                                height: dims.height,
                            };
                            renderer.recreate_swapchain();
                        }
                        _ => (),
                    }
                }
                Event::RedrawEventsCleared => {
                    if let Some((
                        mut frame_info,
                        swapchain_image,
                        frame,
                        frametime_id,
                        frametime_tp,
                    )) = renderer.begin_render()
                    {
                        application
                            .as_ref()
                            .unwrap()
                            .lock()
                            .render(&mut frame_info, &*thread_tps.lock());
                        renderer.end_render(swapchain_image, frame, frametime_id, frametime_tp);
                    }
                }
                Event::LoopDestroyed => {
                    running.store(false, Ordering::Relaxed);
                    for thread in threads.drain() {
                        thread.1.join().unwrap();
                    }

                    renderer.wait();
                    application = None;
                }
                _ => (),
            }
        });
    }

    pub fn run(self) {
        self.run_with::<NoApplication>()
    }
}

fn adapter_to_string(adapter: &Adapter<B>) -> String {
    format!(
        "{} (type:{})",
        adapter.info.name.cyan(),
        format!("{:?}", adapter.info.device_type).green(),
    )
}
