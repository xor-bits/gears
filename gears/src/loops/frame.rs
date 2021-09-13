use crate::renderer::FramePerfReport;
use std::time::{Duration, Instant};
use winit::event_loop::EventLoop;

pub use winit::event::*;

const PERF_LOG_INTERVAL: usize = 5;

pub trait FrameLoopTarget {
    fn frame(&mut self) -> Option<FramePerfReport>;

    #[allow(unused_variables)]
    fn event(&mut self, event: &WindowEvent) {}
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
        let mut target = self.target;

        let mut frame_count_check_tp = Instant::now();
        let mut frames: usize = 0;
        let mut avg_perf = FramePerfReport::default();

        event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Poll;
            // debug!("event: {:?}", event);

            match event {
                Event::WindowEvent { event, .. } => {
                    target.event(&event);

                    match event {
                        WindowEvent::CloseRequested => {
                            *control_flow = winit::event_loop::ControlFlow::Exit;
                        }
                        _ => (),
                    }
                }
                Event::RedrawEventsCleared => {
                    let ft = match target.frame() {
                        Some(ft) => ft,
                        None => return,
                    };

                    avg_perf.cpu_frame_time += ft.cpu_frame_time;
                    avg_perf.gpu_frame_time += ft.gpu_frame_time;
                    frames += 1;

                    if frame_count_check_tp.elapsed()
                        > Duration::from_secs(PERF_LOG_INTERVAL as u64)
                    {
                        frame_count_check_tp = Instant::now();

                        let cpu_ms =
                            print_nanos(avg_perf.cpu_frame_time.as_nanos() / frames as u128);
                        let gpu_whole_ms = print_nanos(
                            avg_perf.gpu_frame_time.whole_pipeline.as_nanos() / frames as u128,
                        );
                        let gpu_vert_ms =
                            print_nanos(avg_perf.gpu_frame_time.vertex.as_nanos() / frames as u128);
                        let gpu_frag_ms = print_nanos(
                            avg_perf.gpu_frame_time.fragment.as_nanos() / frames as u128,
                        );

                        log::debug!("Performance report (last {} seconds):", PERF_LOG_INTERVAL);
                        log::debug!(" - real FPS: {}", frames / PERF_LOG_INTERVAL);
                        log::debug!(" - average CPU frametime: {}", cpu_ms);
                        log::debug!(" - average GPU frametime: {}", gpu_whole_ms);
                        log::debug!("   - vertex: {}", gpu_vert_ms);
                        log::debug!("   - fragment: {}", gpu_frag_ms);

                        frames = 0;
                        avg_perf = FramePerfReport::default();
                    }
                }
                _ => (),
            }
        })
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
