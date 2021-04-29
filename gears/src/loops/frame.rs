use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use log::debug;
use parking_lot::Mutex;
pub use winit::event::*;
use winit::event_loop::EventLoop;

const PERF_LOG_INTERVAL: usize = 5;

pub trait FrameLoopTarget {
    fn frame(&mut self) -> Option<Duration>;
}

pub trait EventLoopTarget {
    #[allow(unused_variables)]
    fn event(&mut self, event: &WindowEvent);
}

pub struct FrameLoop {
    base: FrameLoopBuilder,
}

pub struct FrameLoopBuilder {
    event_loop: EventLoop<()>,
    frame_targets: Vec<Arc<Mutex<dyn FrameLoopTarget + Send>>>,
    event_targets: Vec<Arc<Mutex<dyn EventLoopTarget + Send>>>,
}

impl FrameLoop {
    pub fn new() -> FrameLoopBuilder {
        FrameLoopBuilder {
            event_loop: EventLoop::new(),
            frame_targets: Vec::new(),
            event_targets: Vec::new(),
        }
    }

    pub fn run(self) -> ! {
        let event_loop = self.base.event_loop;
        let frame_targets = self.base.frame_targets;
        let event_targets = self.base.event_targets;

        let mut frames = 0usize;
        let mut frame_count_check_tp = Instant::now();
        let mut avg_cpu_frametime = Duration::from_secs(0);
        let mut avg_gpu_frametime = Duration::from_secs(0);

        event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Poll;
            // debug!("event: {:?}", event);

            match event {
                Event::WindowEvent { event, .. } => {
                    for target in event_targets.iter() {
                        target.lock().event(&event);
                    }

                    match event {
                        WindowEvent::CloseRequested => {
                            *control_flow = winit::event_loop::ControlFlow::Exit;
                        }
                        _ => (),
                    }
                }
                Event::RedrawEventsCleared => {
                    let cpu_frametime = Instant::now();
                    for target in frame_targets.iter() {
                        if let Some(ft) = target.lock().frame() {
                            avg_gpu_frametime += ft;
                        }
                    }
                    avg_cpu_frametime += cpu_frametime.elapsed();
                    frames += 1;

                    if frame_count_check_tp.elapsed() > Duration::from_secs(PERF_LOG_INTERVAL as u64) {
                        frame_count_check_tp = Instant::now();

                        const MICRO_EXP: f32 = 1.0e-6_f32;
                        let cpu_ms = (avg_cpu_frametime.as_nanos() as f32) / frames as f32 * MICRO_EXP;
                        let gpu_ms = (avg_gpu_frametime.as_nanos() as f32) / frames as f32 * MICRO_EXP;
						let fps = 1000.0 / cpu_ms;

                        debug!("FPS: {} (real: {})\n - average CPU frametime: {} ms\n - average GPU frametime: {} ms", fps, frames / PERF_LOG_INTERVAL, cpu_ms, gpu_ms);

                        avg_cpu_frametime = Duration::from_secs(0);
                        avg_gpu_frametime = Duration::from_secs(0);
                        frames = 0;
                    }
                }
                _ => (),
            }
        })
    }
}

impl FrameLoopBuilder {
    pub fn with_frame_target(mut self, target: Arc<Mutex<dyn FrameLoopTarget + Send>>) -> Self {
        self.frame_targets.push(target);
        self
    }

    pub fn with_event_target(mut self, target: Arc<Mutex<dyn EventLoopTarget + Send>>) -> Self {
        self.event_targets.push(target);
        self
    }

    pub fn with_event_loop(mut self, event_loop: EventLoop<()>) -> Self {
        self.event_loop = event_loop;
        self
    }

    pub fn build(self) -> FrameLoop {
        FrameLoop { base: self }
    }
}
