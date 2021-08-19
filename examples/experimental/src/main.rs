use std::time::{Duration, Instant};

use gears::{
    context::{ContextGPUPick, ContextValidation},
    frame::Frame,
    renderer::simple_renderer::Renderer,
    SyncMode,
};
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
};

fn main() {
    env_logger::init();

    let (frame, events) = Frame::new().build();
    let context = frame
        .context(ContextGPUPick::Automatic, ContextValidation::NoValidation)
        .unwrap();

    let mut frame = 0;
    let mut renderer = Renderer::new()
        .with_sync(SyncMode::Immediate)
        .build(context)
        .unwrap();
    let mut last_frame_check = Instant::now();

    events.run(move |event, _, control| {
        *control = ControlFlow::Poll;

        if last_frame_check.elapsed() > Duration::from_secs(1) {
            log::debug!("FPS = {}", frame);
            last_frame_check = Instant::now();
            frame = 0;
        }

        match event {
            Event::RedrawRequested(_) | Event::RedrawEventsCleared => {
                frame += 1;
                renderer.frame();
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control = ControlFlow::Exit;
            }
            _ => {}
        }
    })
}
