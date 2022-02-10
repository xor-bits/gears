//! ### m&kb controls:
//! - W,A,S,D,Space,LShift to move around
//! - Mouse to look around
//! - R to regenerate voxels with new seed
//! - Q to generate cube mesh
//! - E to generate marching cubes mesh
//! - F to generate smoothed marching cubes mesh
//! - Tab to toggle wireframe
//!
//! ### gamepad controls:
//! - Left stick,X/A,O/B to move around
//! - Right stick to look around
//! - ☐/X to regenerate voxels with new seed
//! - DPadLeft to generate cube mesh
//! - DPadRight to generate marching cubes mesh
//! - DPadDown to generate smoothed marching cubes mesh
//! - Select to toggle wireframe

use cubes::generate_cubes;
use gears::{
    context::Context,
    frame::Frame,
    game_loop::{Event, Runnable, State},
    glam::{Mat4, Vec2, Vec3},
    io::{
        fpcam::FPCam,
        input_state::{Input, InputAxis, InputState, Triggered},
    },
    renderer::{
        buffer::StagedBuffer,
        query::RecordPerf,
        simple_renderer::{FrameData, Renderer},
    },
    winit::event::ElementState,
    SyncMode, UpdateRate,
};
use marching_cubes::generate_marching_cubes;
use shader::{DebugPipeline, DefaultPipeline, UniformData, VertexData};
use simdnoise::NoiseBuilder;
use std::time::Instant;
use vulkano::{
    buffer::{BufferUsage, TypedBufferAccess},
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    pipeline::{Pipeline, PipelineBindPoint},
};

//

mod cubes;
mod marching_cubes;
mod shader;

//

const UPDATE_RATE: UpdateRate = UpdateRate::PerSecond(60);

const WIDTH: usize = 64;
const HEIGHT: usize = 64;
const DEPTH: usize = 64;

const ISLAND: bool = true;

//

enum MeshMode {
    Cubes,
    Marching,
    SMarching,
}

impl MeshMode {
    fn gen_mesh(&self, voxels: &[f32]) -> (Vec<shader::VertexData>, Vec<u32>) {
        match &self {
            MeshMode::Cubes => generate_cubes(voxels),
            MeshMode::Marching => generate_marching_cubes(voxels, false),
            MeshMode::SMarching => generate_marching_cubes(voxels, true),
        }
    }
}

//

struct App {
    frame: Frame,
    renderer: Renderer,

    shaders: (DefaultPipeline, DebugPipeline),
    vb: StagedBuffer<[VertexData]>,
    ib: StagedBuffer<[u32]>,

    input: InputState,
    fpcam: FPCam,

    position: Vec3,
    velocity: Vec3,

    debug: bool,
    voxels: Vec<f32>,
    mesh: MeshMode,
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
    fn init(frame: Frame, renderer: Renderer) -> Self {
        let voxels = generate_voxels(0);
        let (vertices, indices) = generate_cubes(&voxels);

        let vb = StagedBuffer::from_iter(
            &renderer.device,
            BufferUsage::vertex_buffer(),
            vertices.into_iter(),
        )
        .unwrap();

        let ib = StagedBuffer::from_iter(
            &renderer.device,
            BufferUsage::index_buffer(),
            indices.into_iter(),
        )
        .unwrap();

        let fill_shader = shader::DefaultPipeline::build(&renderer);
        let line_shader = shader::DebugPipeline::build(&renderer);

        let input = InputState::new();
        let fpcam = FPCam::with_dir(Vec2::new(
            -std::f32::consts::FRAC_PI_4 * 3.0,
            -std::f32::consts::PI / 5.0,
        ));

        Self {
            frame,
            renderer,

            vb,
            ib,
            shaders: (fill_shader, line_shader),

            input,
            fpcam,

            position: Vec3::new(-26.0, -26.0, -26.0),
            velocity: Vec3::new(0.0, 0.0, 0.0),

            debug: false,
            voxels,
            mesh: MeshMode::Marching,
        }
    }

    fn re_mesh(&mut self) {
        let (vertices, indices) = self.mesh.gen_mesh(&self.voxels);

        self.vb = StagedBuffer::from_iter(
            &self.renderer.device,
            BufferUsage::vertex_buffer(),
            vertices.into_iter(),
        )
        .unwrap();

        self.ib = StagedBuffer::from_iter(
            &self.renderer.device,
            BufferUsage::index_buffer(),
            indices.into_iter(),
        )
        .unwrap();
    }

    fn ubo(&self, delta: f32) -> UniformData {
        let aspect = self.frame.aspect();

        let dir = Vec3::new(
            self.fpcam.dir(delta).y.cos() * self.fpcam.dir(delta).x.sin(),
            self.fpcam.dir(delta).y.sin(),
            self.fpcam.dir(delta).y.cos() * self.fpcam.dir(delta).x.cos(),
        );
        let eye = self.position + self.velocity * delta;
        let focus = eye - dir;
        let up = Vec3::new(0.0, 1.0, 0.0);

        UniformData {
            mvp: Mat4::perspective_rh(1.0, aspect, 0.01, 500.0)
                * Mat4::look_at_rh(eye, focus, up)
                * Mat4::from_scale(Vec3::new(1.0, 1.0, 1.0)),
        }
    }
}

impl Runnable for App {
    fn update(&mut self, _: &mut State, delta: f32) {
        self.fpcam.update(&self.input, delta);
        let speed = delta
            * if self.input.get_input(Input::Decelerate, 0).triggered() {
                2.0
            } else {
                20.0
            };

        self.velocity = Vec3::new(0.0, 0.0, 0.0);
        let local_dir = self.input.get_axis(InputAxis::Move, 0);
        let local_dir = Vec3::new(
            local_dir.x,
            local_dir.y,
            self.input.get_axis(InputAxis::ZMove, 0).x,
        );
        self.velocity.x = local_dir.x * self.fpcam.dir(delta).x.cos()
            - local_dir.y * self.fpcam.dir(delta).x.sin();
        self.velocity.z = -local_dir.x * self.fpcam.dir(delta).x.sin()
            - local_dir.y * self.fpcam.dir(delta).x.cos();
        self.velocity.y = -local_dir.z;
        self.velocity *= speed;
        self.position += self.velocity;
    }

    fn event(&mut self, state: &mut State, event: &Event) {
        self.frame.event(event);
        self.input.event(event);
        self.fpcam.event(event, &self.frame);

        if self.input.should_close() {
            state.stop = true;
        }

        if let Some((_, _, ElementState::Pressed)) = self.input.to_input(event, Input::Stats) {
            self.debug = !self.debug;
        }
        if let Some((_, _, ElementState::Pressed)) = self.input.to_input(event, Input::Reload) {
            let tp = Instant::now();
            self.voxels = generate_voxels(rand::random());
            self.re_mesh();
            println!("Re-gen and re-mesh took: {}ms", tp.elapsed().as_millis());
        }
        if let Some((_, _, ElementState::Pressed)) = self.input.to_input(event, Input::RollLeft) {
            let tp = Instant::now();
            self.mesh = MeshMode::Cubes;
            self.re_mesh();
            println!("Re-mesh took: {}ms", tp.elapsed().as_millis());
        }
        if let Some((_, _, ElementState::Pressed)) = self.input.to_input(event, Input::RollRight) {
            let tp = Instant::now();
            self.mesh = MeshMode::Marching;
            self.re_mesh();
            println!("Re-mesh took: {}ms", tp.elapsed().as_millis());
        }
        if let Some((_, _, ElementState::Pressed)) = self.input.to_input(event, Input::RollDown) {
            let tp = Instant::now();
            self.mesh = MeshMode::SMarching;
            self.re_mesh();
            println!("Re-mesh took: {}ms", tp.elapsed().as_millis());
        }
    }

    fn draw(&mut self, state: &mut State, delta: f32) {
        let FrameData {
            mut recorder,
            viewport,
            scissor,
            perf,

            image_index,
            frame_in_flight,
            future,
        } = self.renderer.begin_frame(state);

        self.vb.update(&mut recorder).unwrap();
        self.ib.update(&mut recorder).unwrap();

        let ubo = self.ubo(delta);
        let (layout, set, pipeline) = if self.debug {
            let ubo = self.shaders.0.buffer_pool.next(ubo).unwrap();
            let layout = self.shaders.1.pipeline.layout().descriptor_set_layouts()[0].clone();
            (
                self.shaders.1.pipeline.layout().clone(),
                PersistentDescriptorSet::new_with_pool(
                    layout,
                    0,
                    &mut self.shaders.1.desc_pool,
                    [WriteDescriptorSet::buffer(0, ubo)],
                )
                .unwrap(),
                self.shaders.1.pipeline.clone(),
            )
        } else {
            let ubo = self.shaders.0.buffer_pool.next(ubo).unwrap();
            let layout = self.shaders.0.pipeline.layout().descriptor_set_layouts()[0].clone();
            (
                self.shaders.0.pipeline.layout().clone(),
                PersistentDescriptorSet::new_with_pool(
                    layout,
                    0,
                    &mut self.shaders.0.desc_pool,
                    [WriteDescriptorSet::buffer(0, ubo)],
                )
                .unwrap(),
                self.shaders.0.pipeline.clone(),
            )
        };

        // outside of render pass
        self.vb.update(&mut recorder).unwrap();

        // inside of render pass
        let mut recorder = recorder.begin_render_pass();
        recorder
            .record()
            .begin_perf(&perf)
            .bind_pipeline_graphics(pipeline)
            .bind_descriptor_sets(PipelineBindPoint::Graphics, layout, 0, vec![set])
            .bind_vertex_buffers(0, self.vb.local.clone())
            .bind_index_buffer(self.ib.local.clone())
            .set_viewport(0, [viewport.clone()])
            .draw_indexed(self.ib.len() as _, 1, 0, 0, 0)
            .unwrap()
            .end_perf(&perf);

        // outside of render pass again
        let recorder = recorder.end_render_pass();

        self.renderer.end_frame(FrameData {
            recorder,
            viewport,
            scissor,
            perf,

            image_index,
            frame_in_flight,
            future,
        });
    }
}

fn main() {
    env_logger::init();

    let context = Context::env().unwrap();

    let mut frame = Frame::builder(context)
        .with_title("Simple Example")
        .with_size(600, 600)
        .with_sync(SyncMode::Immediate)
        // TODO: .with_multisamples(4)
        .build()
        .unwrap();

    let game_loop = frame.game_loop().unwrap();

    let renderer = Renderer::builder(&frame).build().unwrap();

    let app = App::init(frame, renderer);

    game_loop.run(Some(UPDATE_RATE), app);
}
