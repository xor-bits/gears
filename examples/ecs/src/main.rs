use gears::{
    glam::{Mat4, Vec2},
    Buffer, ElementState, EventLoopTarget, Frame, FrameLoop, FrameLoopTarget, FramePerfReport,
    ImmediateFrameInfo, IndexBuffer, IndirectBuffer, InputState, KeyboardInput, RenderRecordInfo,
    Renderer, RendererRecord, SparseBatchBuffer, SparseBatchElement, SyncMode, UpdateLoop,
    UpdateLoopTarget, UpdateRate, UpdateRecordInfo, VertexBuffer, VirtualKeyCode, WindowEvent,
};
use parking_lot::{Mutex, RwLock};
use specs::{
    prelude::ParallelIterator, Builder, Component, DispatcherBuilder, Join, ParJoin, Read,
    ReadStorage, System, VecStorage, World, WorldExt, Write, WriteStorage,
};
use std::{
    sync::{
        mpsc::{sync_channel, SyncSender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

mod shader {
    use gears::{
        glam::{Mat4, Vec2},
        module, pipeline, FormatOf, Input, RGBAOutput, Uniform,
    };

    #[derive(Input, PartialEq, Default)]
    #[repr(C)]
    pub struct VertexData {
        pub pos: Vec2,
    }

    #[derive(Uniform, PartialEq, Default)]
    #[repr(C)]
    pub struct UniformData {
        pub mvp: Mat4,
    }

    module! {
        kind = "vert",
        path = "examples/ecs/res/vert.glsl",
        name = "VERT"
    }

    module! {
        kind = "frag",
        path = "examples/ecs/res/frag.glsl",
        name = "FRAG"
    }

    pipeline! {
        "DefaultPipeline"
        VertexData -> RGBAOutput
        mod "VERT" as "vert" where { in UniformData as 0 }
        mod "FRAG" as "frag"
    }
}

const UPDATE_RATE: UpdateRate = UpdateRate::PerSecond(50);
const MAX_COUNT: usize = 500;
const VERTICES: usize = 4;
const INDICES: usize = 6;
const MAX_VBO_LEN: usize = VERTICES * MAX_COUNT;

gears::static_assertions::const_assert!(MAX_VBO_LEN < (std::u16::MAX as usize));

#[derive(Component)]
#[storage(VecStorage)]
struct QuadMesh(SparseBatchElement<VertexBuffer<shader::VertexData>, shader::VertexData, VERTICES>);

#[derive(Component)]
#[storage(VecStorage)]
struct Pos(Vec2);

#[derive(Component)]
#[storage(VecStorage)]
struct Vel(Vec2);

#[derive(Component)]
#[storage(VecStorage)]
struct Acc(Vec2);

struct Move;

impl<'a> System<'a> for Move {
    type SystemData = (
        ReadStorage<'a, Acc>,
        WriteStorage<'a, Vel>,
        WriteStorage<'a, Pos>,
    );

    fn run(&mut self, (acc_storage, mut vel_storage, mut pos_storage): Self::SystemData) {
        (&acc_storage, &mut vel_storage, &mut pos_storage)
            .par_join()
            .for_each(|(acc, vel, pos)| {
                pos.0 += vel.0 + 0.5 * acc.0;
                vel.0 += acc.0;
            });
    }
}

struct BoundingBox;

impl<'a> System<'a> for BoundingBox {
    type SystemData = (
        WriteStorage<'a, Pos>,
        WriteStorage<'a, Vel>,
        ReadStorage<'a, Acc>,
    );

    fn run(&mut self, (mut pos_storage, mut vel_storage, acc_storage): Self::SystemData) {
        (&mut pos_storage, &mut vel_storage, &acc_storage)
            .par_join()
            .for_each(|(pos, vel, acc)| {
                // i know, this is slightly over-engineered for an example
                let x = pos.0;
                let v = vel.0;
                let a = acc.0;
                let v0 = v - a;
                let x0 = x - v;
                let ground = 1.0;
                if x.y > ground {
                    // calculate the time point where this entity hit the ground
                    let time_point_of_hit_pm =
                        (2.0 * a.y * ground - 2.0 * a.y * x0.y + v0.y.powf(2.0)).sqrt();
                    let t = (-v0.y + time_point_of_hit_pm) / a.y;

                    if t.is_nan() {
                        pos.0.y = ground;
                        return;
                    }

                    // advance time till it hits the ground
                    let x = x0 + v0 * t + 0.5 * a * t.powf(2.0); // == (x.x, 0.9)
                    let v = v0 + a * t;

                    // reverse the velocity
                    let v = v * -1.0;

                    // advance time till where we started
                    let t = 1.0 - t;
                    let x = x + v * t + 0.5 * a * t.powf(2.0);
                    let v = v + a * t;

                    vel.0 = v;
                    pos.0 = x;
                }
            });
    }
}

struct VertexBatch(
    Option<
        Arc<
            RwLock<
                SparseBatchBuffer<VertexBuffer<shader::VertexData>, shader::VertexData, VERTICES>,
            >,
        >,
    >,
);

impl Default for VertexBatch {
    fn default() -> Self {
        Self { 0: None }
    }
}

#[derive(Debug, Default)]
struct DeltaTime(f32);

struct UpdateMesh;

impl<'a> System<'a> for UpdateMesh {
    type SystemData = (
        ReadStorage<'a, Pos>,
        ReadStorage<'a, Vel>,
        ReadStorage<'a, Acc>,
        ReadStorage<'a, QuadMesh>,
        Read<'a, DeltaTime>,
        Write<'a, VertexBatch>,
    );

    fn run(
        &mut self,
        (pos_storage, vel_storage, acc_storage, quad_storage, delta_time, vertex_batch): Self::SystemData,
    ) {
        let dt = delta_time.0;
        let mut mm = vertex_batch.0.as_ref().unwrap().read().mod_map();
        for (pos, vel, acc, quad) in
            (&pos_storage, &vel_storage, &acc_storage, &quad_storage).join()
        {
            // x = x0 + v0 * t + 1/2 * a * t^2
            let o = pos.0 + vel.0 * dt + 0.5 * acc.0 * dt.powf(2.0);
            quad.0.write(
                &mut mm,
                [
                    shader::VertexData {
                        pos: o + Vec2::new(-0.02, -0.02),
                    },
                    shader::VertexData {
                        pos: o + Vec2::new(0.02, -0.02),
                    },
                    shader::VertexData {
                        pos: o + Vec2::new(0.02, 0.02),
                    },
                    shader::VertexData {
                        pos: o + Vec2::new(-0.02, 0.02),
                    },
                ],
            );
        }
        vertex_batch.0.as_ref().unwrap().write().write(mm).unwrap();
    }
}

enum DispatcherWork {
    Update,
    Render,
    Confirm,
}

struct App {
    _frame: Frame,
    renderer: Renderer,
    input: RwLock<InputState>,

    shader: shader::DefaultPipeline,
    vertex_batch: Arc<
        RwLock<SparseBatchBuffer<VertexBuffer<shader::VertexData>, shader::VertexData, VERTICES>>,
    >,
    index_buffer: RwLock<IndexBuffer<u16>>,
    indirect_buffer: RwLock<IndirectBuffer>,

    dispatcher_tx: Mutex<SyncSender<DispatcherWork>>,

    /* render_dispatcher: Arc<Mutex<Dispatcher>>,
    update_dispatcher: Arc<Mutex<Dispatcher>>, */
    delta_time: RwLock<Instant>,
    world: Arc<RwLock<World>>,
}

impl App {
    fn init(_frame: Frame, renderer: Renderer) -> Arc<RwLock<Self>> {
        let input = InputState::new();
        let indices = (0..MAX_COUNT as u16)
            .map(|i| {
                [
                    i * 4 + 0,
                    i * 4 + 1,
                    i * 4 + 2,
                    i * 4 + 0,
                    i * 4 + 2,
                    i * 4 + 3,
                ]
            })
            .flatten()
            .collect::<Vec<u16>>();

        let shader = shader::DefaultPipeline::build(&renderer).unwrap();
        let vertex_batch = Arc::new(RwLock::new(SparseBatchBuffer::new(
            shader.create_vertex_buffer(MAX_VBO_LEN).unwrap(),
        )));
        let index_buffer = RwLock::new(shader.create_index_buffer_with(&indices).unwrap());
        let indirect_buffer = RwLock::new(shader.create_indirect_buffer().unwrap());

        let mut world = World::new();
        world.register::<QuadMesh>();
        world.register::<Pos>();
        world.register::<Vel>();
        world.register::<Acc>();
        world.insert(VertexBatch(Some(vertex_batch.clone())));
        world.insert(DeltaTime(0.0));
        let world = Arc::new(RwLock::new(world));

        let (dispatcher_tx, dispatcher_rx) = sync_channel(0);
        let dispatcher_tx = Mutex::new(dispatcher_tx);
        let dispatcher_world = world.clone();

        thread::spawn(move || {
            let mut render_dispatcher = DispatcherBuilder::new()
                .with(UpdateMesh, "mesh", &[])
                .build();
            let mut update_dispatcher = DispatcherBuilder::new()
                .with(Move, "move", &[])
                .with(BoundingBox, "bb", &["move"])
                .build();

            loop {
                match dispatcher_rx.recv() {
                    Ok(DispatcherWork::Update) => {
                        update_dispatcher.dispatch(&dispatcher_world.write())
                    }
                    Ok(DispatcherWork::Render) => {
                        render_dispatcher.dispatch(&dispatcher_world.write())
                    }
                    Ok(DispatcherWork::Confirm) => {}
                    Err(_) => break,
                }
            }
        });

        Arc::new(RwLock::new(Self {
            _frame,
            renderer,
            input,

            shader,
            vertex_batch,
            index_buffer,
            indirect_buffer,

            dispatcher_tx,

            delta_time: RwLock::new(Instant::now()),
            world,
        }))
    }
}

impl RendererRecord for App {
    fn immediate(&self, imfi: &ImmediateFrameInfo) {
        let mvp = Mat4::orthographic_rh(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0);
        let ubo = shader::UniformData { mvp };
        let dt = (Instant::now() - *self.delta_time.read()).as_secs_f32()
            / UPDATE_RATE.to_interval().as_secs_f32();
        self.shader.write_vertex_uniform(imfi, &ubo).unwrap();

        let world = self.world.write();
        world.write_resource::<DeltaTime>().0 = dt;
        drop(world);

        let dispatcher_tx = self.dispatcher_tx.lock();
        dispatcher_tx.send(DispatcherWork::Render).unwrap();
        dispatcher_tx.send(DispatcherWork::Confirm).unwrap();
    }

    unsafe fn update(&self, uri: &UpdateRecordInfo) -> bool {
        [
            self.shader.update(uri),
            self.vertex_batch.write().update(uri),
            self.index_buffer.write().update(uri),
            self.indirect_buffer.write().update(uri),
        ]
        .iter()
        .any(|b| *b)
    }

    unsafe fn record(&self, rri: &RenderRecordInfo) {
        self.shader
            .draw(rri)
            .vertex(self.vertex_batch.read().buffer())
            .index(&self.index_buffer.read())
            .indirect(&self.indirect_buffer.read())
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
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::A),
                        ..
                    },
                ..
            } => {}
            _ => return,
        }

        let count = self.vertex_batch.read().len();
        if count >= MAX_COUNT {
            return;
        }

        let vertices = self.vertex_batch.write().create_one();
        let indices = INDICES * (count + 1);
        self.indirect_buffer
            .write()
            .write(indices as u32, 0)
            .unwrap();

        let (x, y): (f32, f32) = rand::random();
        let (x, y) = (x * 2.0 - 1.0, y * 2.0 - 1.0);

        self.world
            .write()
            .create_entity()
            .with(QuadMesh { 0: vertices })
            .with(Acc {
                0: Vec2::new(0.0, 0.001),
            })
            .with(Vel { 0: Vec2::ZERO })
            .with(Pos { 0: Vec2::new(x, y) })
            .build();
    }
}

impl FrameLoopTarget for App {
    fn frame(&self) -> FramePerfReport {
        self.renderer.frame(self)
    }
}

impl UpdateLoopTarget for App {
    fn update(&self, _: &Duration) {
        // simulated freeze
        while self.input.read().key_held(VirtualKeyCode::Space) {
            thread::sleep(Duration::from_millis(500));
        }
        *self.delta_time.write() = Instant::now();

        let dispatcher_tx = self.dispatcher_tx.lock();
        dispatcher_tx.send(DispatcherWork::Update).unwrap();
    }
}

fn main() {
    env_logger::init();

    let (frame, event_loop) = Frame::new()
        .with_title("Simple Example")
        .with_size(600, 600)
        .build();

    let context = frame.default_context().unwrap();

    let app = App::init(
        frame,
        Renderer::new()
            .with_sync(SyncMode::Immediate)
            .build(context)
            .unwrap(),
    );

    let _ = UpdateLoop::new()
        .with_rate(UPDATE_RATE)
        .with_target(app.clone())
        .build()
        .run();

    let _ = FrameLoop::new()
        .with_event_loop(event_loop)
        .with_event_target(app.clone())
        .with_frame_target(app)
        .build()
        .run();
}
