use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use cgmath::{perspective, Deg, InnerSpace, Matrix4, Point3, Rad, Vector3};
use gears::{
    load_obj, EventLoopTarget, Frame, FrameLoopTarget, ImmediateFrameInfo, InputState,
    KeyboardInput, Pipeline, PipelineBuilder, RenderRecordInfo, Renderer, RendererRecord,
    VertexBuffer, VirtualKeyCode, WindowEvent,
};
use parking_lot::Mutex;

mod shader {
    gears_pipeline::pipeline! {
        vs: { path: "gear/res/default.vert.glsl" }
        fs: { path: "gear/res/default.frag.glsl" }
    }
}

const MAX_VBO_LEN: usize = 50_000;

struct App {
    frame: Frame,
    renderer: Box<Option<Renderer>>,

    vb: VertexBuffer<shader::VertexData>,
    shader: Pipeline,

    input: Arc<Mutex<InputState>>,
    delta_time: Instant,

    position: Vector3<f32>,
    velocity: Vector3<f32>,
}

impl App {
    fn init(frame: gears::Frame, context: gears::Context, input: Arc<Mutex<InputState>>) -> Self {
        let renderer = gears::Renderer::new()
            .with_vsync(gears::VSync::Off)
            .build(context)
            .unwrap();

        let vb = VertexBuffer::new(&renderer, MAX_VBO_LEN).unwrap();
        let shader = PipelineBuilder::new(&renderer)
            .with_graphics_modules(shader::VERT_SPIRV, shader::FRAG_SPIRV)
            .with_input::<shader::VertexData>()
            .with_ubo::<shader::UBO>()
            .build(false)
            .unwrap();

        let mut app = Self {
            frame,
            renderer: Box::new(Some(renderer)),

            vb,
            shader,

            input,
            delta_time: Instant::now(),

            position: Vector3::new(0.0, 0.0, 0.0),
            velocity: Vector3::new(0.0, 0.0, 0.0),
        };

        app.reload_mesh();

        app
    }

    fn reload_mesh(&mut self) {
        let vertices = load_obj(include_str!("res/gear.obj"), None, |position, normal| {
            shader::VertexData {
                pos: position,
                norm: normal,
            }
        });

        self.vb
            .write(0, &vertices[..vertices.len().min(MAX_VBO_LEN)])
            .unwrap();
    }
}

impl RendererRecord for App {
    fn immediate(&mut self, imfi: &ImmediateFrameInfo) {
        let dt_s = self.delta_time.elapsed().as_secs_f32();
        self.delta_time = Instant::now();
        let aspect = self.frame.aspect();

        self.velocity = Vector3::new(0.0, 0.0, 0.0);
        {
            let input = self.input.lock();
            if input.key_held(VirtualKeyCode::A) {
                self.velocity.x += 1.0;
            }
            if input.key_held(VirtualKeyCode::D) {
                self.velocity.x -= 1.0;
            }
            if input.key_held(VirtualKeyCode::W) {
                self.velocity.y += 1.0;
            }
            if input.key_held(VirtualKeyCode::S) {
                self.velocity.y -= 1.0;
            }
            if input.key_held(VirtualKeyCode::Space) {
                self.velocity.z += 2.0;
            }
        }
        self.position += self.velocity * 3.0 * dt_s;
        self.position.y = self
            .position
            .y
            .min(std::f32::consts::PI / 2.0 - 0.0001)
            .max(-std::f32::consts::PI / 2.0 + 0.0001);

        let eye = Point3::new(
            self.position.x.sin() * self.position.y.cos(),
            self.position.y.sin(),
            self.position.x.cos() * self.position.y.cos(),
        ) * 2.5;
        let focus = Point3::new(0.0, 0.0, 0.0);

        let ubo = shader::UBO {
            model_matrix: Matrix4::from_angle_x(Rad { 0: self.position.z }),
            view_matrix: Matrix4::look_at_rh(eye, focus, Vector3::new(0.0, -1.0, 0.0)),
            projection_matrix: perspective(Deg { 0: 60.0 }, aspect, 0.01, 5.0),
            light_dir: Vector3::new(0.2, 2.0, 0.5).normalize(),
        };

        self.shader.write_ubo(imfi, &ubo);
    }

    fn record(&mut self, rri: &RenderRecordInfo) {
        self.shader.bind(rri);
        self.vb.draw(rri);
    }
}

impl EventLoopTarget for App {
    fn event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(VirtualKeyCode::R),
                        ..
                    },
                ..
            } => {
                self.reload_mesh();
            }
            _ => {}
        }
    }
}

impl FrameLoopTarget for App {
    fn frame(&mut self) -> Option<Duration> {
        let mut renderer = self.renderer.take().unwrap();
        let result = renderer.frame(self);
        *self.renderer.as_mut() = Some(renderer);

        result
    }
}

fn main() {
    env_logger::init();

    let (frame, event_loop) = gears::Frame::new()
        .with_title("Simple Example")
        .with_size(600, 600)
        .build();

    let context = frame.context().unwrap();

    let input = Arc::new(Mutex::new(InputState::new()));
    let app = Arc::new(Mutex::new(App::init(frame, context, input.clone())));

    gears::FrameLoop::new()
        .with_event_loop(event_loop)
        .with_event_target(input)
        .with_event_target(app.clone())
        .with_frame_target(app)
        .build()
        .run();
}
