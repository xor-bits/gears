//! controls:
//! - W,A,S,D,Space,C to move around
//! - Mouse to look around
//! - R to regenerate voxels with new seed
//! - B to generate cube mesh
//! - N to generate marching cubes mesh
//! - M to generate smoothed marching cubes mesh
//! - Tab to toggle wireframe

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use cubes::generate_cubes;
use gears::{
    glam::{Mat4, Vec2, Vec3, Vec4},
    input_state::InputState,
    renderer::buffer::{IndexBuffer, VertexBuffer},
    Buffer, ContextGPUPick, ContextValidation, CursorController, ElementState, EventLoopTarget,
    Frame, FrameLoop, FrameLoopTarget, FramePerfReport, HideMode, ImmediateFrameInfo,
    MultiWriteBuffer, RenderRecordBeginInfo, RenderRecordInfo, Renderer, RendererRecord, SyncMode,
    UpdateLoop, UpdateLoopTarget, UpdateRate, UpdateRecordInfo, VirtualKeyCode, WindowEvent,
};
use marching_cubes::generate_marching_cubes;
use parking_lot::RwLock;
use simdnoise::NoiseBuilder;

mod cubes;
mod marching_cubes;

mod shader {
    use gears::{
        glam::{Mat4, Vec3},
        module, pipeline, FormatOf, Input, RGBAOutput, Uniform,
    };

    #[derive(Input, PartialEq, Default)]
    #[repr(C)]
    pub struct VertexData {
        pub position: Vec3,
        pub exposure: f32,
    }

    #[derive(Uniform, PartialEq, Default)]
    #[repr(C)]
    pub struct UniformData {
        pub mvp: Mat4,
    }

    module! {
        kind = "vert",
        path = "examples/voxel/res/default.vert.glsl",
        name = "VERT"
    }

    module! {
        kind = "geom",
        path = "examples/voxel/res/default.geom.glsl",
        name = "GEOM"
    }

    module! {
        kind = "frag",
        path = "examples/voxel/res/default.frag.glsl",
        name = "FRAG"
    }

    module! {
        kind = "frag",
        path = "examples/voxel/res/default.frag.glsl",
        name = "DEBUG_FRAG",
        define = "DEBUGGING"
    }

    pipeline! {
        "DefaultPipeline"
        VertexData -> RGBAOutput

        mod "VERT" as "vert" where { in UniformData as 0 }
        mod "FRAG" as "frag"
    }

    pipeline! {
        "DebugPipeline"
        VertexData -> RGBAOutput

        mod "VERT" as "vert" where { in UniformData as 0 }
        mod "GEOM" as "geom"
        mod "DEBUG_FRAG" as "frag"
    }
}

const UPDATES_PER_SECOND: u32 = 60;

const WIDTH: usize = 64;
const HEIGHT: usize = 64;
const DEPTH: usize = 64;

const ISLAND: bool = true;

enum MeshMode {
    Cubes,
    MarchingCubes,
    SMarchingCubes,
}

impl MeshMode {
    fn gen_mesh(&self, voxels: &Vec<f32>) -> (Vec<shader::VertexData>, Vec<u32>) {
        match &self {
            MeshMode::Cubes => generate_cubes(voxels),
            MeshMode::MarchingCubes => generate_marching_cubes(voxels, false),
            MeshMode::SMarchingCubes => generate_marching_cubes(voxels, true),
        }
    }
}

struct App {
    frame: Frame,
    renderer: Renderer,

    shaders: (shader::DefaultPipeline, shader::DebugPipeline),
    vb: RwLock<VertexBuffer<shader::VertexData>>,
    ib: RwLock<IndexBuffer<u32>>,

    cursor_controller: RwLock<CursorController>,
    input: RwLock<InputState>,

    look_dir: RwLock<Vec2>,
    position: RwLock<Vec3>,
    velocity: RwLock<Vec3>,

    debug: AtomicBool,
    voxels: RwLock<Vec<f32>>,
    mesh: RwLock<MeshMode>,

    update_rate: Duration,
    delta_time: RwLock<Instant>,
}

fn generate_voxels(seed: i32) -> Vec<f32> {
    let voxels = NoiseBuilder::fbm_3d(WIDTH, HEIGHT, DEPTH)
        .with_freq(0.02)
        .with_octaves(4)
        .with_gain(0.95)
        .with_lacunarity(1.7)
        .with_seed(seed)
        .generate_scaled(0.0, 1.0);
    voxels
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            let x = i % WIDTH;
            let y = (i / WIDTH) % HEIGHT;
            let z = i / (WIDTH * HEIGHT);

            if ISLAND {
                let fade_x = 1.0 - (2.0 / WIDTH as f32 * x as f32 - 1.0).powf(4.0);
                let fade_y = 1.0 - (2.0 / HEIGHT as f32 * y as f32 - 1.0).powf(4.0);
                let fade_z = 1.0 - (2.0 / DEPTH as f32 * z as f32 - 1.0).powf(4.0);

                v * fade_x * fade_y * fade_z
            } else {
                v
            }
        })
        .collect::<Vec<_>>()
}

fn point_to_index(x: usize, y: usize, z: usize) -> usize {
    x + y * WIDTH + z * WIDTH * HEIGHT
}

impl App {
    fn init(frame: Frame, renderer: Renderer) -> Arc<RwLock<Self>> {
        let voxels = generate_voxels(0);
        let (vertices, indices) = generate_cubes(&voxels);
        let voxels = RwLock::new(voxels);

        let vb = RwLock::new(VertexBuffer::new_with_data(&renderer, &vertices[..]).unwrap());
        let ib = RwLock::new(IndexBuffer::new_with_data(&renderer, &indices[..]).unwrap());

        let fill_shader = shader::DefaultPipeline::build(&renderer).unwrap();
        let line_shader = shader::DebugPipeline::build(&renderer).unwrap();

        let input = InputState::new();
        let cursor_controller =
            RwLock::new(CursorController::new().with_hide_mode(HideMode::GrabCursor));

        Arc::new(RwLock::new(Self {
            frame,
            renderer,

            vb,
            ib,
            shaders: (fill_shader, line_shader),

            cursor_controller,
            input,

            look_dir: RwLock::new(Vec2::new(
                -std::f32::consts::FRAC_PI_4 * 3.0,
                -std::f32::consts::PI / 5.0,
            )),
            position: RwLock::new(Vec3::new(-26.0, -26.0, -26.0)),
            velocity: RwLock::new(Vec3::new(0.0, 0.0, 0.0)),

            debug: AtomicBool::new(false),
            voxels,
            mesh: RwLock::new(MeshMode::MarchingCubes),

            update_rate: UpdateRate::PerSecond(UPDATES_PER_SECOND).to_interval(),
            delta_time: RwLock::new(Instant::now()),
        }))
    }

    fn re_mesh(&self) {
        let (vertices, indices) = self.mesh.read().gen_mesh(&self.voxels.read());

        // TODO: impl VertexBuffer::resize
        let vb_resize = self.vb.read().len() < vertices.len();
        let ib_resize = self.ib.read().len() < indices.len();
        if vb_resize || ib_resize {
            self.renderer.wait();
            if vb_resize {
                *self.vb.write() =
                    VertexBuffer::new_with_data(&self.renderer, &vertices[..]).unwrap();
            }
            if ib_resize {
                *self.ib.write() =
                    IndexBuffer::new_with_data(&self.renderer, &indices[..]).unwrap();
            }
            self.renderer.request_rerecord();
        }

        if !vb_resize {
            self.vb.write().write(0, &vertices[..]).unwrap();
        }
        if !ib_resize {
            self.ib.write().write(0, &indices[..]).unwrap();
        }
    }
}

impl RendererRecord for App {
    fn immediate(&self, imfi: &ImmediateFrameInfo) {
        let dt_s = self.delta_time.read().elapsed().as_secs_f32() / self.update_rate.as_secs_f32();
        let aspect = self.frame.aspect();

        let look_dir = *self.look_dir.read();
        let position = *self.position.read();
        let velocity = *self.velocity.read();

        let dir = Vec3::new(
            look_dir.y.cos() * look_dir.x.sin(),
            look_dir.y.sin(),
            look_dir.y.cos() * look_dir.x.cos(),
        );
        let eye = position + velocity * dt_s;
        let focus = eye - dir;
        let up = Vec3::new(0.0, 1.0, 0.0);

        let ubo = shader::UniformData {
            mvp: Mat4::perspective_rh(1.0, aspect, 0.01, 500.0)
                * Mat4::look_at_rh(eye, focus, up)
                * Mat4::from_scale(Vec3::new(1.0, 1.0, 1.0)),
        };

        self.shaders.0.write_vertex_uniform(imfi, &ubo).unwrap();
        self.shaders.1.write_vertex_uniform(imfi, &ubo).unwrap();
    }

    unsafe fn update(&self, uri: &UpdateRecordInfo) -> bool {
        [
            self.shaders.0.update(uri),
            self.shaders.1.update(uri),
            self.ib.write().update(uri),
            self.vb.write().update(uri),
        ]
        .iter()
        .any(|b| *b)
    }

    fn begin_info(&self) -> RenderRecordBeginInfo {
        RenderRecordBeginInfo {
            clear_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            debug_calls: true,
        }
    }

    unsafe fn record(&self, rri: &RenderRecordInfo) {
        if self.debug.load(Ordering::SeqCst) {
            self.shaders.0.draw(rri)
        } else {
            self.shaders.1.draw(rri)
        }
        .vertex(&self.vb.read())
        .index(&self.ib.read())
        .direct(self.ib.read().len() as u32, 0)
        .execute();
    }
}

impl FrameLoopTarget for App {
    fn frame(&self) -> FramePerfReport {
        self.renderer.frame(self)
    }
}

impl EventLoopTarget for App {
    fn event(&self, event: &WindowEvent) {
        self.input.write().update(event);
        if let WindowEvent::KeyboardInput { input, .. } = event {
            match (input.virtual_keycode, input.state) {
                (Some(VirtualKeyCode::Tab), ElementState::Pressed) => {
                    self.renderer.request_rerecord();
                    self.debug.fetch_xor(true, Ordering::SeqCst); // xor a, 1 == !a
                }
                (Some(VirtualKeyCode::R), ElementState::Pressed) => {
                    let tp = Instant::now();
                    *self.voxels.write() = generate_voxels(rand::random());
                    self.re_mesh();
                    println!("Re-gen and re-mesh took: {}ms", tp.elapsed().as_millis());
                }
                (Some(VirtualKeyCode::B), ElementState::Pressed) => {
                    let tp = Instant::now();
                    *self.mesh.write() = MeshMode::Cubes;
                    self.re_mesh();
                    println!("Re-mesh took: {}ms", tp.elapsed().as_millis());
                }
                (Some(VirtualKeyCode::N), ElementState::Pressed) => {
                    let tp = Instant::now();
                    *self.mesh.write() = MeshMode::MarchingCubes;
                    self.re_mesh();
                    println!("Re-mesh took: {}ms", tp.elapsed().as_millis());
                }
                (Some(VirtualKeyCode::M), ElementState::Pressed) => {
                    let tp = Instant::now();
                    *self.mesh.write() = MeshMode::SMarchingCubes;
                    self.re_mesh();
                    println!("Re-mesh took: {}ms", tp.elapsed().as_millis());
                }
                _ => {}
            }
        }

        if let Some((delta_x, delta_y)) = self.cursor_controller.write().event(event, &self.frame) {
            let mut look_dir = self.look_dir.write();
            *look_dir -= Vec2::new(delta_x as f32, delta_y as f32);

            look_dir.y = look_dir.y.clamp(
                -std::f32::consts::PI / 2.0 + 0.0001,
                std::f32::consts::PI / 2.0 - 0.0001,
            );
        }
    }
}

impl UpdateLoopTarget for App {
    fn update(&self, delta_time: &Duration) {
        let dt_s = delta_time.as_secs_f32();
        *self.delta_time.write() = Instant::now();

        let look_dir = *self.look_dir.read();
        let look_dir = Vec3::new(
            look_dir.y.cos() * look_dir.x.sin(),
            look_dir.y.sin(),
            look_dir.y.cos() * look_dir.x.cos(),
        );
        let up = Vec3::new(0.0, 1.0, 0.0);

        {
            let input = self.input.read();
            let speed = {
                let mut speed = 10.0 * dt_s;
                if input.key_held(VirtualKeyCode::LShift) {
                    speed *= 10.0;
                }
                if input.key_held(VirtualKeyCode::LAlt) {
                    speed *= 0.1;
                }
                speed
            };
            let dir = {
                let mut dir = look_dir;
                dir.y = 0.0;
                dir.normalize() * speed
            };

            let mut velocity = Vec3::new(0.0, 0.0, 0.0);
            if input.key_held(VirtualKeyCode::W) {
                velocity -= dir;
            }
            if input.key_held(VirtualKeyCode::S) {
                velocity += dir;
            }
            if input.key_held(VirtualKeyCode::A) {
                velocity += dir.cross(up);
            }
            if input.key_held(VirtualKeyCode::D) {
                velocity -= dir.cross(up);
            }
            if input.key_held(VirtualKeyCode::Space) {
                velocity.y -= speed;
            }
            if input.key_held(VirtualKeyCode::C) {
                velocity.y += speed;
            }
            *self.velocity.write() = velocity;
            *self.position.write() += velocity;
        }
    }
}

fn main() {
    env_logger::init();

    let (frame, event_loop) = Frame::new()
        .with_title("Simple Example")
        .with_size(600, 600)
        // TODO: .with_multisamples(4)
        .build();

    let context = frame
        .context(ContextGPUPick::Automatic, ContextValidation::WithValidation)
        .unwrap();

    let renderer = Renderer::new()
        .with_sync(SyncMode::Immediate)
        .build(context)
        .unwrap();

    let app = App::init(frame, renderer);

    let frame_loop = FrameLoop::new()
        .with_event_loop(event_loop)
        .with_event_target(app.clone())
        .with_frame_target(app.clone())
        .build();

    let update_loop = UpdateLoop::new()
        .with_rate(UpdateRate::PerSecond(UPDATES_PER_SECOND))
        .with_target(app)
        .build();

    update_loop.run();
    frame_loop.run();
}
