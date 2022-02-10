use crate::{report::Reporter, UpdateRate, io::input_state::InputState};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use gilrs::{Event as GilrsEvent, GilrsBuilder};
use vulkano::swapchain::Surface;
use winit::{
    dpi::PhysicalPosition,
    event::{ WindowEvent, Event as WinitEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

//

pub trait Runnable {
    #[allow(unused_variables)]
    fn update(&mut self, state: &mut State, delta: f32) {}

    #[allow(unused_variables)]
    fn event(&mut self, state: &mut State, event: &Event) {}

    #[allow(unused_variables)]
    fn draw(&mut self, state: &mut State, delta: f32) {}
}

//


#[derive(Debug, PartialEq)]
pub enum Event<'e> {
    /// Controller/gamepad/joystick related events
    GilrsEvent(GilrsEvent),

    /// Window/Keyboard/Cursor/Device events
    WinitEvent(WinitEvent<'e, ()>)
}

//

pub struct Loop {
    window: Arc<Surface<Window>>,
    event_loop: Option<EventLoop<()>>,
    init_timer: Instant,
}

pub struct State {
    //
    pub cpu_frame_reporter: Reporter,
    pub gpu_frame_reporter: Reporter,
    pub update_reporter: Reporter,

    // window size
    pub size: (f32, f32),

    // window aspect ratio
    pub aspect: f32,

    // is cursor inside the window?
    pub cursor_in: bool,

    // cursor position
    pub cursor_pos: PhysicalPosition<f64>,

    // window scaling factor
    pub scale_factor: f64,

    // update interval
    pub interval: Option<Duration>,

    // the loop should stop
    pub stop: bool,
}

//

impl Loop {
    pub fn new(
        window: Arc<Surface<Window>>,
        event_loop: EventLoop<()>,
        init_timer: Instant,
    ) -> Self {
        Self {
            window,
            event_loop: Some(event_loop),
            init_timer,
        }
    }

    pub fn run(mut self, update_rate: Option<UpdateRate>, app: impl Runnable + 'static) -> ! {
        log::debug!("Initialization took: {:?}", self.init_timer.elapsed());

        let window = self.window.window();
        let size = window.inner_size().into();
        let scale_factor = window.scale_factor();
        let interval = update_rate.map(|rate| rate.to_interval());
        window.set_visible(true);

        let mut previous = Instant::now();
        let mut lag = Duration::from_secs_f64(0.0);
        let mut state = State {
            cpu_frame_reporter: Reporter::new(),
            gpu_frame_reporter: Reporter::new(),
            update_reporter: Reporter::new(),
            size,
            aspect: size.0 / size.1,
            cursor_in: false,
            cursor_pos: Default::default(),
            scale_factor,
            interval,
            stop: false,
        };
        let mut opt_app = Some(app);

        let mut gilrs = match GilrsBuilder::new()/* .with_default_filters(false) */.build() {
            Ok(gilrs) => Some(gilrs),
            Err(err) => {
                log::error!("Failed to init Gilrs, gamepad/joystick input disabled: {err}");
                None
            }
        };

        self.event_loop
            .take()
            .unwrap()
            .run(move |event, _, control| {
                let app = if let Some(app) = opt_app.as_mut() {
                    app
                }  else {
                    return;
                };

                *control = ControlFlow::Poll;
                if state.stop {
                    *control = ControlFlow::Exit;
                    log::debug!("Dropping app");
                    {
                        opt_app.take().unwrap();
                    }
                    log::debug!("App dropped");
                    return;
                }

                if let Some(gilrs) = gilrs.as_mut() {
                    let event = gilrs.next_event();
                    let event = InputState::deadzone(event, gilrs);
                    if let Some(event) = event {
                        app.event(&mut state, &Event::GilrsEvent(event));
                    };
                }

                match &event {
                    WinitEvent::WindowEvent {
                        event: WindowEvent::CursorEntered { .. },
                        ..
                    } => state.cursor_in = true,
                    WinitEvent::WindowEvent {
                        event: WindowEvent::CursorLeft { .. },
                        ..
                    } => state.cursor_in = false,
                    WinitEvent::WindowEvent {
                        event: WindowEvent::CursorMoved { position, .. },
                        ..
                    } => {
                        state.cursor_pos = *position;
                    }
                    WinitEvent::WindowEvent {
                        event: WindowEvent::Resized(s),
                        ..
                    } => {
                        state.size = (s.width as f32, s.height as f32);
                        let s = s.to_logical::<f32>(state.scale_factor);
                        state.aspect = s.width / s.height;
                    }
                    WinitEvent::RedrawRequested(_) => {
                        // main game loop source:
                        //  - https://gameprogrammingpatterns.com/game-loop.html
                        if let Some(interval) = state.interval {
                            let elapsed = previous.elapsed();
                            previous = Instant::now();
                            lag += elapsed;
    
                            // updates
                            // stop after 20 to avoid freezing completely caused by the input 
                            // if those updates take longer than they should
                            let mut i = 0;
                            while lag >= interval && i <= 20 {
                                i += 1;
                                let timer = state.update_reporter.begin();
                                app.update(&mut state, interval.as_secs_f32());
                                state.update_reporter.end(timer);
                                lag -= interval;
                            }
                        }

                        // frames
                        let timer = state.cpu_frame_reporter.begin();
                        {
							let dt = if let Some(interval) = state.interval {lag.as_secs_f32() / interval.as_secs_f32() } else {self.init_timer.elapsed().as_secs_f32()};
                            app.draw(
                                &mut state,
                                dt,
                            );
                        }
                        let should_report = state.cpu_frame_reporter.end(timer);

                        // reports
                        if should_report {
                            let int = state.cpu_frame_reporter.report_interval();
                            let (u_int, u_per_sec) = state.update_reporter.last_string();
                            let (cf_int, cf_per_sec) = state.cpu_frame_reporter.last_string();
                            let (gf_int, gf_per_sec) = state.gpu_frame_reporter.last_string();

                            #[cfg(debug_assertions)]
                            const DEBUG: &str = "debug build";
                            #[cfg(not(debug_assertions))]
                            const DEBUG: &str = "release build";

                            log::debug!(
                                "Report ({:?})({})\n              per second @ time per\nUPDATES:{:>16} @ {}\nCPU FRAMES:{:>13} @ {}\nGPU FRAMES:{:>13} @ {}",
                                int,
                                DEBUG,
                                u_per_sec,
                                u_int,
                                cf_per_sec,
                                cf_int,
								gf_per_sec,
								gf_int
                            );
                        }

                        return;
                    }
                    WinitEvent::MainEventsCleared => {
                        self.window.window().request_redraw();
                    }
                    _ => {}
                }

                app.event(&mut state, &Event::WinitEvent(event));
            })
    }
}
