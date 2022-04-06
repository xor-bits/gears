use ecs::{Acc, BoundingBox, Move, Pos, QuadMesh, UpdateMesh, Vel};
use gears::{
    context::Context,
    frame::Frame,
    game_loop::{Event, Runnable, State},
    glam::{Mat4, Vec2},
    io::input_state::{Input, InputState, Triggered},
    renderer::{buffer::StagedBuffer, query::RecordPerf, simple_renderer::Renderer},
    SyncMode, UpdateRate,
};
use shader::{UniformData, VertexData};
use specs::{Builder, DispatcherBuilder, World, WorldExt};
use std::{thread, time::Duration};
use vulkano::{
    buffer::{BufferUsage, CpuBufferPool},
    descriptor_set::WriteDescriptorSet,
    pipeline::{Pipeline, PipelineBindPoint},
};

//

mod ecs;
mod shader;

//

const UPDATE_RATE: UpdateRate = UpdateRate::PerSecond(50);
const MAX_COUNT: usize = 500;
const VERTICES: usize = 4;
const INDICES: usize = 6;
const MAX_IBO_LEN: usize = VERTICES * MAX_COUNT;
const MAX_VBO_LEN: usize = INDICES * MAX_COUNT;

gears::static_assertions::const_assert!(MAX_VBO_LEN < (std::u16::MAX as usize));

//

struct App {
    renderer: Renderer,
    input: InputState,

    shader: shader::DefaultPipeline,
    spot: usize,
    vertex_buffer: CpuBufferPool<[VertexData; MAX_VBO_LEN]>,
    index_buffer: StagedBuffer<[u16]>,

    // dispatcher: DispatcherWork,
    world: World,
}

impl App {
    fn init(renderer: Renderer) -> Self {
        let input = InputState::new();
        let indices = (0..MAX_COUNT as u16)
            .map(|i| [i * 4, i * 4 + 1, i * 4 + 2, i * 4, i * 4 + 2, i * 4 + 3])
            .flatten()
            .collect::<Vec<u16>>();
        // clippy warning fix
        indices.len();

        let shader = shader::DefaultPipeline::build(&renderer);
        let vertex_buffer = CpuBufferPool::vertex_buffer(renderer.device.logical().clone());
        let index_buffer = StagedBuffer::from_iter(
            &renderer.device,
            BufferUsage::index_buffer(),
            indices.into_iter(),
        )
        .unwrap();

        let mut world = World::new();
        world.register::<QuadMesh>();
        world.register::<Pos>();
        world.register::<Vel>();
        world.register::<Acc>();

        Self {
            renderer,
            input,

            shader,
            spot: 0,
            vertex_buffer,
            index_buffer,

            world,
        }
    }
}

impl Runnable for App {
    fn update(&mut self, state: &mut State, _: f32) {
        if self.input.get_input(Input::Pause, 0).triggered() {
            state.stop = true;
        }
        if self.input.get_input(Input::Jump, 0).triggered() {
            // simulated freeze
            thread::sleep(Duration::from_millis(500));
        }
        if self.spot < MAX_COUNT * 4 && self.input.get_input(Input::Stats, 0).triggered() {
            let spot = self.spot;
            self.spot += 4;

            let (x, y): (f32, f32) = rand::random();
            let (x, y) = (x * 2.0 - 1.0, y * 2.0 - 1.0);

            self.world
                .create_entity()
                .with(QuadMesh { 0: spot })
                .with(Acc {
                    0: Vec2::new(0.0, 0.001),
                })
                .with(Vel { 0: Vec2::ZERO })
                .with(Pos { 0: Vec2::new(x, y) })
                .build();
        }

        DispatcherBuilder::new()
            .with(Move, "move", &[])
            .with(BoundingBox, "bb", &["move"])
            .build()
            .dispatch(&self.world);
    }

    fn event(&mut self, state: &mut State, event: &Event) {
        self.input.event(event);
        if self.input.should_close() {
            state.stop = true;
        }
    }

    fn draw(&mut self, state: &mut State, delta: f32) {
        let mut fd = self.renderer.begin_frame(state);
        let viewport = fd.viewport_and_scissor().0;

        let mut recorder = fd.recorder;
        let perf = fd.perf;

        let mut data = [VertexData::default(); MAX_VBO_LEN];
        DispatcherBuilder::new()
            .with(UpdateMesh(delta, &mut data), "mesh", &[])
            .build()
            .dispatch(&self.world);
        let vbo = self.vertex_buffer.next(data).unwrap();
        self.index_buffer.update(&mut recorder).unwrap();

        let mut recorder = recorder.begin_render_pass();

        let mvp = Mat4::orthographic_rh(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0).to_cols_array_2d();
        let ubo = UniformData { mvp };
        let ubo = self.shader.buffer_pool.next(ubo).unwrap();
        let set = self
            .shader
            .desc_pool
            .next([WriteDescriptorSet::buffer(0, ubo)])
            .unwrap();
        recorder
            .record()
            .begin_perf(&perf)
            .bind_pipeline_graphics(self.shader.pipeline.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.shader.pipeline.layout().clone(),
                0,
                vec![set],
            )
            .bind_vertex_buffers(0, vbo)
            .bind_index_buffer(self.index_buffer.local.clone())
            .set_viewport(0, [viewport])
            .draw_indexed(MAX_IBO_LEN as u32, 1, 0, 0, 0)
            .unwrap()
            .end_perf(&perf);

        let recorder = recorder.end_render_pass();
        fd.recorder = recorder;
        fd.perf = perf;
        self.renderer.end_frame(fd);
    }
}

fn main() {
    env_logger::init();

    let mut frame = Frame::builder(Context::env().unwrap())
        .with_title("Simple Example")
        .with_size(600, 600)
        .with_sync(SyncMode::Mailbox)
        .build()
        .unwrap();

    frame.game_loop().unwrap().run(
        Some(UPDATE_RATE),
        App::init(Renderer::builder(&frame).build().unwrap()),
    );
}
