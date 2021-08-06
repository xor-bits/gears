use gears::{
    glam::{Mat4, Vec3},
    load_obj, Buffer, ContextGPUPick, ContextValidation, EventLoopTarget, Frame, FrameLoop,
    FrameLoopTarget, FramePerfReport, ImmediateFrameInfo, InputState, KeyboardInput,
    MultiWriteBuffer, RenderRecordInfo, Renderer, RendererRecord, SyncMode, UpdateRecordInfo,
    VertexBuffer, VirtualKeyCode, WindowEvent,
};
use parking_lot::RwLock;
use std::{sync::Arc, time::Instant};

mod shader {
    use gears::{
        glam::{Mat4, Vec3},
        module, pipeline, FormatOf, Input, RGBAOutput, Uniform,
    };

    #[derive(Input, PartialEq, Default)]
    #[repr(C)]
    pub struct VertexData {
        pub pos: Vec3,
        pub norm: Vec3,
    }

    #[derive(Uniform, PartialEq, Default)]
    #[repr(C)]
    pub struct UniformData {
        pub model_matrix: Mat4,
        pub view_matrix: Mat4,
        pub projection_matrix: Mat4,
        pub light_dir: Vec3,
    }

    module! {
        kind = "vert",
        path = "examples/gear/res/default.vert.glsl",
        name = "VERT"
    }

    module! {
        kind = "frag",
        path = "examples/gear/res/default.frag.glsl",
        name = "FRAG"
    }

    pipeline! {
        "DefaultPipeline"
        VertexData -> RGBAOutput
        mod "VERT" as "vert" where { in UniformData as 0 }
        mod "FRAG" as "frag"
    }
}

const MAX_VBO_LEN: usize = 50_000;

struct App {
    frame: Frame,
    renderer: Renderer,
    input: RwLock<InputState>,

    shader: shader::DefaultPipeline,
    vb: RwLock<VertexBuffer<shader::VertexData>>,

    delta_time: RwLock<Instant>,
    distance: RwLock<f32>,
    position: RwLock<Vec3>,
}

impl App {
    fn init(frame: Frame, renderer: Renderer) -> Arc<RwLock<Self>> {
        let input = InputState::new();
        let shader = shader::DefaultPipeline::build(&renderer).unwrap();
        let vb = RwLock::new(
            shader
                .create_vbo_with(Self::vertex_data().as_slice())
                .unwrap(),
        );

        Arc::new(RwLock::new(Self {
            frame,
            renderer,
            input,

            shader,
            vb,

            delta_time: RwLock::new(Instant::now()),
            distance: RwLock::new(2.5),
            position: RwLock::new(Vec3::new(0.0, 0.0, 0.0)),
        }))
    }

    fn vertex_data() -> Vec<shader::VertexData> {
        load_obj(include_str!("../res/gear.obj"), None, |position, normal| {
            shader::VertexData {
                pos: position,
                norm: normal,
            }
        })
    }

    fn reload_mesh(&self) {
        let vertices = Self::vertex_data();
        self.vb
            .write()
            .write(0, &vertices[..vertices.len().min(MAX_VBO_LEN)])
            .unwrap();
    }
}

impl RendererRecord for App {
    fn immediate(&self, imfi: &ImmediateFrameInfo) {
        let aspect = self.frame.aspect();
        let dt_s = {
            let mut delta_time = self.delta_time.write();
            let dt_s = delta_time.elapsed().as_secs_f32();
            *delta_time = Instant::now();
            dt_s
        };

        let mut distance_delta = 0.0;
        let mut velocity = Vec3::new(0.0, 0.0, 0.0);
        {
            let input = self.input.read();
            if input.key_held(VirtualKeyCode::E) {
                distance_delta += 1.0;
            }
            if input.key_held(VirtualKeyCode::Q) {
                distance_delta -= 1.0;
            }
            if input.key_held(VirtualKeyCode::A) {
                velocity.x += 1.0;
            }
            if input.key_held(VirtualKeyCode::D) {
                velocity.x -= 1.0;
            }
            if input.key_held(VirtualKeyCode::W) {
                velocity.y += 1.0;
            }
            if input.key_held(VirtualKeyCode::S) {
                velocity.y -= 1.0;
            }
            if input.key_held(VirtualKeyCode::Space) {
                velocity.z += 2.0;
            }
        }
        let distance = {
            let mut distance = self.distance.write();
            *distance += distance_delta * 3.0 * dt_s;
            *distance
        };
        let position = {
            let mut position = self.position.write();

            *position += velocity * 3.0 * dt_s;
            position.y = position
                .y
                .min(std::f32::consts::PI / 2.0 - 0.0001)
                .max(-std::f32::consts::PI / 2.0 + 0.0001);

            *position
        };

        let eye = Vec3::new(
            position.x.sin() * position.y.cos(),
            position.y.sin(),
            position.x.cos() * position.y.cos(),
        ) * distance;
        let focus = Vec3::new(0.0, 0.0, 0.0);
        let up = Vec3::new(0.0, -1.0, 0.0);

        let ubo = shader::UniformData {
            model_matrix: Mat4::from_rotation_x(position.z),
            view_matrix: Mat4::look_at_rh(eye, focus, up),
            projection_matrix: Mat4::perspective_rh(1.0, aspect, 0.01, 100.0),
            light_dir: Vec3::new(0.2, 2.0, 0.5).normalize(),
        };

        self.shader.write_vertex_uniform(imfi, &ubo).unwrap();
    }

    unsafe fn update(&self, uri: &UpdateRecordInfo) -> bool {
        [self.shader.update(uri), self.vb.write().update(uri)]
            .iter()
            .any(|b| *b)
    }

    unsafe fn record(&self, rri: &RenderRecordInfo) {
        self.shader
            .draw(rri)
            .vertex(&self.vb.read())
            .direct(self.vb.read().elem_capacity() as u32, 0)
            .execute();
    }
}

impl EventLoopTarget for App {
    fn event(&self, event: &WindowEvent) {
        self.input.write().update(event);
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
    fn frame(&self) -> FramePerfReport {
        self.renderer.frame(self)
    }
}

fn main() {
    env_logger::init();

    let (frame, event_loop) = Frame::new()
        .with_title("Simple Example")
        .with_size(600, 600)
        .build();

    let context = frame
        .context(ContextGPUPick::Automatic, ContextValidation::WithValidation)
        .unwrap();

    let renderer = Renderer::new()
        .with_sync(SyncMode::Immediate)
        .build(context)
        .unwrap();

    let app = App::init(frame, renderer);

    FrameLoop::new()
        .with_event_loop(event_loop)
        .with_event_target(app.clone())
        .with_frame_target(app)
        .build()
        .run();
}
