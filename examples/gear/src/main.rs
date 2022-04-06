use gears::{
    context::Context,
    frame::Frame,
    game_loop::{Event, Runnable, State},
    glam::{Mat4, Vec3},
    io::input_state::{Input, InputAxis, InputState, Triggered},
    renderer::{
        buffer::StagedBuffer,
        object::load_obj,
        query::RecordPerf,
        simple_renderer::{FrameData, Renderer},
    },
    vulkano::buffer::{BufferUsage, TypedBufferAccess},
    SyncMode,
};
use shader::UniformData;
use std::{sync::Arc, time::Instant};
use vulkano::{
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    pipeline::{Pipeline, PipelineBindPoint},
};

//

mod shader;

//

struct App {
    frame: Frame,
    renderer: Renderer,
    input: InputState,

    shader: shader::DefaultPipeline,
    vb: StagedBuffer<[shader::VertexData]>,

    distance: f32,
    position: Vec3,
    dt: Instant,
}

impl App {
    fn init(frame: Frame, renderer: Renderer) -> Self {
        let input = InputState::new();
        let shader = shader::DefaultPipeline::build(&renderer);

        let vertices = Self::vertex_data();
        let vb = StagedBuffer::from_iter(
            &renderer.device,
            BufferUsage::vertex_buffer(),
            vertices.into_iter(),
        )
        .unwrap();

        Self {
            frame,
            renderer,
            input,

            shader,
            vb,

            distance: 2.5,
            position: Vec3::new(0.0, 0.0, 0.0),
            dt: Instant::now(),
        }
    }

    fn vertex_data() -> Vec<shader::VertexData> {
        // TODO: make a macro for loading objects at compile time
        load_obj(include_str!("../res/gear.obj"), None, |pos, norm| {
            shader::VertexData {
                vi_pos: pos.to_array(),
                vi_norm: norm.to_array(),
            }
        })
    }

    fn update_uniform_buffer(&mut self) -> Arc<PersistentDescriptorSet> {
        let aspect = self.frame.aspect();

        let delta = self.dt.elapsed().as_secs_f32();
        self.dt = Instant::now();

        let distance_delta = self.input.get_axis(InputAxis::Roll, 0).x;
        let velocity = self.input.get_axis(InputAxis::Move, 0);
        let roll = self.input.get_axis(InputAxis::Trigger, 0).x;
        let velocity = Vec3::new(-velocity.x, velocity.y, roll);

        self.distance += distance_delta * 3.0 * delta;
        self.position += velocity * 3.0 * delta;
        self.position.y = self
            .position
            .y
            .min(std::f32::consts::PI / 2.0 - 0.0001)
            .max(-std::f32::consts::PI / 2.0 + 0.0001);

        let eye = Vec3::new(
            self.position.x.sin() * self.position.y.cos(),
            self.position.y.sin(),
            self.position.x.cos() * self.position.y.cos(),
        ) * self.distance;
        let focus = Vec3::new(0.0, 0.0, 0.0);
        let up = Vec3::new(0.0, -1.0, 0.0);

        let ubo = UniformData {
            model_matrix: Mat4::from_rotation_x(self.position.z).to_cols_array_2d(),
            view_matrix: Mat4::look_at_rh(eye, focus, up).to_cols_array_2d(),
            projection_matrix: Mat4::perspective_rh(1.0, aspect, 0.01, 100.0).to_cols_array_2d(),
            light_dir: Vec3::new(0.2, 2.0, 0.5).normalize().to_array(),
        };

        let ubo = self.shader.buffer_pool.next(ubo).unwrap();
        let layout = self.shader.pipeline.layout().set_layouts()[0].clone();
        PersistentDescriptorSet::new_with_pool(
            layout,
            0,
            &mut self.shader.desc_pool,
            [WriteDescriptorSet::buffer(0, ubo)],
        )
        .unwrap()
    }
}

impl Runnable for App {
    fn draw(&mut self, state: &mut State, _: f32) {
        let FrameData {
            mut recorder,
            viewport,
            scissor,
            perf,

            image_index,
            frame_in_flight,
            future,
        } = self.renderer.begin_frame(state);

        // outside of render pass
        self.vb.update(&mut recorder).unwrap();
        let set = self.update_uniform_buffer();

        // inside of render pass
        let mut recorder = recorder.begin_render_pass();
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
            .bind_vertex_buffers(0, self.vb.local.clone())
            .set_viewport(0, [viewport.clone()])
            .draw(self.vb.local.len() as u32, 1, 0, 0)
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

    fn event(&mut self, state: &mut State, event: &Event) {
        self.frame.event(event);
        self.input.event(event);

        if self.input.should_close()
            || self.input.get_input(Input::Pause, 0).triggered()
            || self.input.get_input(Input::RollLeft, 0).triggered()
        {
            state.stop = true
        }
    }
}

fn main() {
    env_logger::init();

    let context = Context::env().unwrap();

    let mut frame = Frame::builder(context)
        .with_title("Simple Example")
        .with_size(600, 600)
        .with_sync(SyncMode::Immediate)
        .build()
        .unwrap();

    let game_loop = frame.game_loop().unwrap();

    let renderer = Renderer::builder(&frame).build().unwrap();

    let app = App::init(frame, renderer);

    game_loop.run(None, app);
}
