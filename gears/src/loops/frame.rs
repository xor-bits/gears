use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use log::debug;
use parking_lot::RwLock;
pub use winit::event::*;
use winit::event_loop::EventLoop;

use crate::renderer::FramePerfReport;

const PERF_LOG_INTERVAL: usize = 5;

pub trait FrameLoopTarget {
    fn frame(&self) -> FramePerfReport;
}

pub trait EventLoopTarget {
    #[allow(unused_variables)]
    fn event(&self, event: &WindowEvent);
}

pub struct FrameLoop {
    base: FrameLoopBuilder,
}

pub struct FrameLoopBuilder {
    event_loop: EventLoop<()>,
    frame_targets: Vec<Arc<RwLock<dyn FrameLoopTarget + Send + Sync>>>,
    event_targets: Vec<Arc<RwLock<dyn EventLoopTarget + Send + Sync>>>,
}

impl FrameLoop {
    pub fn new() -> FrameLoopBuilder {
        FrameLoopBuilder {
            event_loop: EventLoop::new(),
            frame_targets: Vec::new(),
            event_targets: Vec::new(),
        }
    }

    pub fn run<F: FnOnce() + 'static>(self, on_stop: F) -> ! {
        let event_loop = self.base.event_loop;
        let mut frame_targets = self.base.frame_targets;
        let mut event_targets = self.base.event_targets;

        let mut frame_count_check_tp = Instant::now();
        let mut frames: usize = 0;
        let mut avg_perf = FramePerfReport::default();

        let mut on_stop = Some(on_stop);

        event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Poll;
            // debug!("event: {:?}", event);

            match event {
                Event::WindowEvent { event, .. } => {
                    for target in event_targets.iter() {
                        target.read().event(&event);
                    }

                    if event == WindowEvent::CloseRequested {
                        *control_flow = winit::event_loop::ControlFlow::Exit;
                        log::debug!("Stopping");
                        frame_targets.clear();
                        event_targets.clear();
                        on_stop.take().unwrap()();
                    }
                }
                Event::RedrawEventsCleared => {
                    for target in frame_targets.iter() {
                        let ft = target.read().frame();
                        avg_perf.cpu_frametime += ft.cpu_frametime;
                        avg_perf.gpu_frametime += ft.gpu_frametime;
                        avg_perf.rerecord = avg_perf.rerecord || ft.rerecord;
                        avg_perf.updates = avg_perf.updates || ft.updates;
                        avg_perf.triangles = ft.triangles;
                    }
                    frames += 1;

                    if frame_count_check_tp.elapsed()
                        > Duration::from_secs(PERF_LOG_INTERVAL as u64)
                    {
                        frame_count_check_tp = Instant::now();

                        let cpu_ms =
                            print_nanos(avg_perf.cpu_frametime.as_nanos() / frames as u128);
                        let gpu_whole_ms = print_nanos(
                            avg_perf.gpu_frametime.whole_pipeline.as_nanos() / frames as u128,
                        );
                        let gpu_vert_ms =
                            print_nanos(avg_perf.gpu_frametime.vertex.as_nanos() / frames as u128);
                        let gpu_frag_ms = print_nanos(
                            avg_perf.gpu_frametime.fragment.as_nanos() / frames as u128,
                        );

                        debug!("Performance report (last {} seconds):", PERF_LOG_INTERVAL);
                        debug!(" - real FPS: {}", frames / PERF_LOG_INTERVAL);
                        debug!(" - latest triangles: {}", avg_perf.triangles);
                        debug!(" - any updates: {}", avg_perf.updates);
                        debug!(" - any rerecords: {}", avg_perf.rerecord);
                        debug!(" - average CPU frametime: {}", cpu_ms);
                        debug!(" - average GPU frametime: {}", gpu_whole_ms);
                        debug!("   - vertex: {}", gpu_vert_ms);
                        debug!("   - fragment: {}", gpu_frag_ms);

                        frames = 0;
                        avg_perf = FramePerfReport::default();
                    }
                }
                _ => (),
            }
        })
    }
}

impl FrameLoopBuilder {
    pub fn with_frame_target(
        mut self,
        target: Arc<RwLock<dyn FrameLoopTarget + Send + Sync>>,
    ) -> Self {
        self.frame_targets.push(target);
        self
    }

    pub fn with_event_target(
        mut self,
        target: Arc<RwLock<dyn EventLoopTarget + Send + Sync>>,
    ) -> Self {
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

fn print_nanos(nanos: u128) -> String {
    if nanos > 1_000_000_000 {
        format!("{} seconds", nanos / 1_000_000_000)
    } else if nanos > 1_000_000 {
        format!("{} milliseconds", nanos / 1_000_000)
    } else if nanos > 1_000 {
        format!("{} microseconds", nanos / 1_000)
    } else {
        format!("{} nanoseconds", nanos)
    }
}
