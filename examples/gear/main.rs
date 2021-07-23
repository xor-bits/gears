use std::{sync::Arc, time::Instant};

use gears::{
    load_obj, Buffer, ContextGPUPick, ContextValidation, EventLoopTarget, Frame, FrameLoop,
    FrameLoopTarget, FramePerfReport, ImmediateFrameInfo, InputState, KeyboardInput,
    RenderRecordInfo, Renderer, RendererRecord, SyncMode, UpdateRecordInfo, VertexBuffer,
    VirtualKeyCode, WindowEvent,
};
use glam::{Mat4, Vec3};
use parking_lot::{Mutex, RwLock};

mod shader {
    use gears::{FormatOf, GraphicsPipeline, Input, PipelineBuilderBase, Uniform};
    use gears_pipeline::*;
    use glam::{Mat4, Vec3};
    use static_assertions::assert_type_eq_all;

    #[derive(Input, Default)]
    #[repr(C)]
    pub struct VertexData {
        pub pos: Vec3,
        pub norm: Vec3,
    }

    #[derive(Uniform, Default)]
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

    /*
    pub const SHADER: ShaderData = pipeline! {
        in VertexData

        mod VERT_SPIRV {
            in UniformData
        }

        mod FRAG_SPIRV
    }; TODO: generates: bellow
    */

    pub type Pipeline = GraphicsPipeline<VertexData, UniformData, ()>;
    pub fn build(renderer: &gears::Renderer) -> Pipeline {
        assert_type_eq_all!(<VertexData as Input>::FIELDS, VERT::INPUT);
        assert_type_eq_all!(<UniformData as Uniform>::FIELDS, VERT::UNIFORM);

        PipelineBuilderBase::new(renderer)
            .vertex_uniform(VERT::SPIRV, UniformData::default())
            .fragment(FRAG::SPIRV)
            .build()
            .unwrap()
    }
}

const MAX_VBO_LEN: usize = 50_000;

struct App {
    frame: Frame,
    renderer: Renderer,
    input: Arc<RwLock<InputState>>,

    vb: VertexBuffer<shader::VertexData>,
    shader: shader::Pipeline,

    delta_time: Mutex<Instant>,
    distance: Mutex<f32>,
    position: Mutex<Vec3>,
}

impl App {
    fn init(frame: Frame, renderer: Renderer, input: Arc<RwLock<InputState>>) -> Arc<RwLock<Self>> {
        let vb = VertexBuffer::new(&renderer, MAX_VBO_LEN).unwrap();
        let shader = shader::build(&renderer);

        let mut app = Self {
            frame,
            renderer,
            input,

            vb,
            shader,

            delta_time: Mutex::new(Instant::now()),
            distance: Mutex::new(2.5),
            position: Mutex::new(Vec3::new(0.0, 0.0, 0.0)),
        };

        app.reload_mesh();

        Arc::new(RwLock::new(app))
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
    fn immediate(&self, imfi: &ImmediateFrameInfo) {
        let dt_s = {
            let mut delta_time = self.delta_time.lock();
            let dt_s = delta_time.elapsed().as_secs_f32();
            *delta_time = Instant::now();
            dt_s
        };
        let aspect = self.frame.aspect();

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
            let mut distance = self.distance.lock();
            *distance += distance_delta * 3.0 * dt_s;
            *distance
        };
        let position = {
            let mut position = self.position.lock();

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

    fn update(&self, uri: &UpdateRecordInfo) -> bool {
        unsafe { self.shader.update(uri) || self.vb.update(uri) }
    }

    fn record(&self, rri: &RenderRecordInfo) {
        unsafe {
            self.shader.bind(rri);
            self.vb.draw(rri);
        }
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

    let input = InputState::new();
    let app = App::init(frame, renderer, input.clone());

    FrameLoop::new()
        .with_event_loop(event_loop)
        .with_event_target(input)
        .with_event_target(app.clone())
        .with_frame_target(app)
        .build()
        .run();
}
