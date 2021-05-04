/// controls:
/// - W,A,S,D,Space,C to move around
/// - Mouse to look around
/// - R to regenerate voxels with new seed
/// - B to generate cube mesh
/// - N to generate marching cubes mesh
/// - M to generate smoothed marching cubes mesh
/// - Tab to toggle wireframe
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use cgmath::{perspective, Deg, EuclideanSpace, InnerSpace, Matrix4, Point3, Vector2, Vector3};
use cubes::generate_cubes;
use gears::{
    input_state::InputState,
    renderer::{
        buffer::{IndexBuffer, VertexBuffer},
        pipeline::{Pipeline, PipelineBuilder},
    },
    Buffer, CursorController, ElementState, EventLoopTarget, Frame, FrameLoop, FrameLoopTarget,
    HideMode, ImmediateFrameInfo, RenderRecordInfo, Renderer, RendererRecord, UpdateLoop,
    UpdateLoopTarget, UpdateQuery, UpdateRate, UpdateRecordInfo, VSync, VirtualKeyCode,
    WindowEvent,
};
use marching_cubes::generate_marching_cubes;
use parking_lot::Mutex;
use simdnoise::NoiseBuilder;

mod cubes;
mod marching_cubes;

mod shader {
    gears_pipeline::pipeline! {
        vs: { path: "voxel/res/default.vert.glsl" }
        ge: { path: "voxel/res/default.geom.glsl" }
        fs: { path: "voxel/res/default.frag.glsl" }
    }
}

mod debug_shader {
    gears_pipeline::pipeline! {
        fs: { path: "voxel/res/default.frag.glsl" define: ["DEBUGGING"] }
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
    renderer: Box<Option<Renderer>>,

    vb: VertexBuffer<shader::VertexData>,
    ib: IndexBuffer<u32>,
    shaders: (Pipeline, Pipeline),

    cursor_controller: CursorController,
    input: Arc<Mutex<InputState>>,

    look_dir: Vector2<f32>,
    position: Point3<f32>,
    velocity: Vector3<f32>,

    debug: bool,
    voxels: Vec<f32>,
    mesh: MeshMode,

    delta_time: Instant,
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
    fn init(frame: Frame, renderer: Renderer, input: Arc<Mutex<InputState>>) -> Self {
        let voxels = generate_voxels(0);
        let (vertices, indices) = generate_cubes(&voxels);

        let vb = VertexBuffer::new_with_data(&renderer, &vertices[..]).unwrap();
        let ib = IndexBuffer::new_with_data(&renderer, &indices[..]).unwrap();

        let fill_shader = PipelineBuilder::new(&renderer)
            .with_ubo::<shader::UBO>()
            .with_graphics_modules(shader::VERT_SPIRV, shader::FRAG_SPIRV)
            .with_input::<shader::VertexData>()
            .build(false)
            .unwrap();
        let line_shader = PipelineBuilder::new(&renderer)
            .with_ubo::<shader::UBO>()
            .with_graphics_modules(shader::VERT_SPIRV, debug_shader::FRAG_SPIRV)
            .with_geometry_module(shader::GEOM_SPIRV)
            .with_input::<shader::VertexData>()
            .build(false)
            .unwrap();

        let cursor_controller = CursorController::new().with_hide_mode(HideMode::GrabCursor);

        Self {
            frame,
            renderer: Box::new(Some(renderer)),

            vb,
            ib,
            shaders: (fill_shader, line_shader),

            cursor_controller,
            input,

            look_dir: Vector2::new(
                -std::f32::consts::FRAC_PI_4 * 3.0,
                -std::f32::consts::PI / 5.0,
            ),
            position: Point3::new(-26.0, -26.0, -26.0),
            velocity: Vector3::new(0.0, 0.0, 0.0),

            debug: false,
            voxels,
            mesh: MeshMode::MarchingCubes,

            delta_time: Instant::now(),
        }
    }

    fn remesh(&mut self) {
        let (vertices, indices) = self.mesh.gen_mesh(&self.voxels);

        // TODO: impl VertexBuffer::resize
        let vb_resize = self.vb.len() < vertices.len();
        let ib_resize = self.ib.len() < indices.len();
        if vb_resize || ib_resize {
            let mut renderer = self.renderer.take().unwrap();

            renderer.wait();
            if vb_resize {
                self.vb = VertexBuffer::new_with_data(&renderer, &vertices[..]).unwrap();
            }
            if ib_resize {
                self.ib = IndexBuffer::new_with_data(&renderer, &indices[..]).unwrap();
            }
            renderer.request_rerecord();

            *self.renderer.as_mut() = Some(renderer);
        }

        if !vb_resize {
            self.vb.write(0, &vertices[..]).unwrap();
        }
        if !ib_resize {
            self.ib.write(0, &indices[..]).unwrap();
        }
    }
}

impl RendererRecord for App {
    fn immediate(&mut self, imfi: &ImmediateFrameInfo) {
        let dt_s = self.delta_time.elapsed().as_secs_f32();
        let aspect = self.frame.aspect();

        let dir = Vector3::new(
            self.look_dir.y.cos() * self.look_dir.x.sin(),
            self.look_dir.y.sin(),
            self.look_dir.y.cos() * self.look_dir.x.cos(),
        );
        let eye = self.position + self.velocity * dt_s;
        let focus = (eye - dir).to_vec();
        let focus = Point3::from_vec(focus);
        let up = Vector3::new(0.0, 1.0, 0.0);

        let ubo = shader::UBO {
            mvp: perspective(Deg { 0: 60.0 }, aspect, 0.01, 500.0)
                * Matrix4::look_at_rh(eye, focus, up)
                * Matrix4::from_scale(1.0),
        };

        self.shaders.0.write_ubo(imfi, &ubo).unwrap();
        self.shaders.1.write_ubo(imfi, &ubo).unwrap();
    }

    fn updates(&mut self, uq: &UpdateQuery) -> bool {
        self.shaders.0.updates(uq)
            || self.shaders.1.updates(uq)
            || self.ib.updates(uq)
            || self.vb.updates(uq)
    }

    fn update(&mut self, uri: &UpdateRecordInfo) {
        unsafe {
            self.shaders.0.update(uri);
            self.shaders.1.update(uri);
            self.ib.update(uri);
            self.vb.update(uri);
        }
    }

    fn record(&mut self, rri: &RenderRecordInfo) {
        unsafe {
            if self.debug {
                self.shaders.0.bind(rri);
            } else {
                self.shaders.1.bind(rri);
            }
            self.ib.draw(rri, &self.vb);
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

impl EventLoopTarget for App {
    fn event(&mut self, event: &WindowEvent) {
        if let WindowEvent::KeyboardInput { input, .. } = event {
            match (input.virtual_keycode, input.state) {
                (Some(VirtualKeyCode::Tab), ElementState::Pressed) => {
                    let mut renderer = self.renderer.take().unwrap();
                    renderer.request_rerecord();
                    *self.renderer.as_mut() = Some(renderer);
                    self.debug = !self.debug;
                }
                (Some(VirtualKeyCode::R), ElementState::Pressed) => {
                    let tp = Instant::now();
                    self.voxels = generate_voxels(rand::random());
                    self.remesh();
                    println!("Regen and remesh took: {}ms", tp.elapsed().as_millis());
                }
                (Some(VirtualKeyCode::B), ElementState::Pressed) => {
                    let tp = Instant::now();
                    self.mesh = MeshMode::Cubes;
                    self.remesh();
                    println!("Remesh took: {}ms", tp.elapsed().as_millis());
                }
                (Some(VirtualKeyCode::N), ElementState::Pressed) => {
                    let tp = Instant::now();
                    self.mesh = MeshMode::MarchingCubes;
                    self.remesh();
                    println!("Remesh took: {}ms", tp.elapsed().as_millis());
                }
                (Some(VirtualKeyCode::M), ElementState::Pressed) => {
                    let tp = Instant::now();
                    self.mesh = MeshMode::SMarchingCubes;
                    self.remesh();
                    println!("Remesh took: {}ms", tp.elapsed().as_millis());
                }
                _ => {}
            }
        }

        if let Some((delta_x, delta_y)) = self.cursor_controller.event(event, &self.frame) {
            self.look_dir -= Vector2::new(delta_x as f32, delta_y as f32);

            self.look_dir.y = self.look_dir.y.clamp(
                -std::f32::consts::PI / 2.0 + 0.0001,
                std::f32::consts::PI / 2.0 - 0.0001,
            );
        }
    }
}

impl UpdateLoopTarget for App {
    fn update(&mut self, delta_time: &Duration) {
        let dt_s = delta_time.as_secs_f32();
        self.delta_time = Instant::now();

        let dir = Vector3::new(
            self.look_dir.y.cos() * self.look_dir.x.sin(),
            self.look_dir.y.sin(),
            self.look_dir.y.cos() * self.look_dir.x.cos(),
        );
        let up = Vector3::new(0.0, 1.0, 0.0);

        {
            let input = self.input.lock();
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
                let mut dir = dir;
                dir.y = 0.0;
                dir.normalize() * speed
            };

            self.velocity = self.position.to_vec();
            if input.key_held(VirtualKeyCode::W) {
                self.position -= dir;
            }
            if input.key_held(VirtualKeyCode::S) {
                self.position += dir;
            }
            if input.key_held(VirtualKeyCode::A) {
                self.position += dir.cross(up);
            }
            if input.key_held(VirtualKeyCode::D) {
                self.position -= dir.cross(up);
            }
            if input.key_held(VirtualKeyCode::Space) {
                self.position.y -= speed;
            }
            if input.key_held(VirtualKeyCode::C) {
                self.position.y += speed;
            }
        }

        self.velocity = (self.position.to_vec() - self.velocity) / dt_s;
    }
}

fn main() {
    env_logger::init();

    let (frame, event_loop) = Frame::new()
        .with_title("Simple Example")
        .with_size(600, 600)
        // TODO: .with_multisamples(4)
        .build();

    let context = frame.context_auto_pick().unwrap();

    let renderer = Renderer::new()
        .with_vsync(VSync::Off)
        .build(context)
        .unwrap();

    let input = Arc::new(Mutex::new(InputState::new()));
    let app = Arc::new(Mutex::new(App::init(frame, renderer, input.clone())));

    let frame_loop = FrameLoop::new()
        .with_event_loop(event_loop)
        .with_event_target(input)
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
