use crate::renderer::FramePerfReport;
use std::time::{Duration, Instant};
use winit::event_loop::{ControlFlow, EventLoop};

pub use winit::event::*;

const PERF_LOG_INTERVAL: usize = 5;

pub trait FrameLoopTarget {
    fn frame(&mut self) -> Option<FramePerfReport>;

    #[allow(unused_variables)]
    fn event(&mut self, event: &WindowEvent);
}

pub struct FrameLoop {
    event_loop: EventLoop<()>,
    target: Box<dyn FrameLoopTarget>,
}

impl FrameLoop {
    pub fn new(event_loop: EventLoop<()>, target: Box<dyn FrameLoopTarget>) -> FrameLoop {
        FrameLoop { event_loop, target }
    }

    pub fn run(self) -> ! {
        let event_loop = self.event_loop;
        let mut handler = EventHandler {
            target: self.target,
            frame_count_check_tp: Instant::now(),
            frames: 0,
            avg_perf: FramePerfReport::default(),
        };
        /* let runtime = tokio::runtime::Builder::new_multi_thread().build().unwrap(); */

        event_loop.run(move |event, _, control_flow| {
            /* runtime.block_on(/*  */) */
            handler.event_handler(event, control_flow)
        })
    }
}

struct EventHandler {
    target: Box<dyn FrameLoopTarget>,
    frame_count_check_tp: Instant,
    frames: usize,
    avg_perf: FramePerfReport,
}

impl EventHandler {
    fn event_handler(&mut self, event: Event<'_, ()>, control_flow: &mut ControlFlow) {
        *control_flow = winit::event_loop::ControlFlow::Poll;
        // debug!("event: {:?}", event);

        match event {
            Event::WindowEvent { event, .. } => {
                self.target.event(&event);

                match event {
                    WindowEvent::CloseRequested => {
                        *control_flow = winit::event_loop::ControlFlow::Exit;
                    }
                    _ => (),
                }
            }
            Event::RedrawEventsCleared => {
                let ft = match self.target.frame() {
                    Some(ft) => ft,
                    None => return,
                };

                self.avg_perf.cpu_frame_time += ft.cpu_frame_time;
                self.avg_perf.gpu_frame_time += ft.gpu_frame_time;
                self.frames += 1;

                if self.frame_count_check_tp.elapsed()
                    > Duration::from_secs(PERF_LOG_INTERVAL as u64)
                {
                    self.frame_count_check_tp = Instant::now();

                    let cpu_ms =
                        print_nanos(self.avg_perf.cpu_frame_time.as_nanos() / self.frames as u128);
                    let gpu_whole_ms = print_nanos(
                        self.avg_perf.gpu_frame_time.whole_pipeline.as_nanos()
                            / self.frames as u128,
                    );
                    let gpu_vert_ms = print_nanos(
                        self.avg_perf.gpu_frame_time.vertex.as_nanos() / self.frames as u128,
                    );
                    let gpu_frag_ms = print_nanos(
                        self.avg_perf.gpu_frame_time.fragment.as_nanos() / self.frames as u128,
                    );

                    log::debug!("Performance report (last {} seconds):", PERF_LOG_INTERVAL);
                    log::debug!(" - real FPS: {}", self.frames / PERF_LOG_INTERVAL);
                    log::debug!(" - average CPU frametime: {}", cpu_ms);
                    log::debug!(" - average GPU frametime: {}", gpu_whole_ms);
                    log::debug!("   - vertex: {}", gpu_vert_ms);
                    log::debug!("   - fragment: {}", gpu_frag_ms);

                    self.frames = 0;
                    self.avg_perf = FramePerfReport::default();
                }
            }
            _ => (),
        }
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
