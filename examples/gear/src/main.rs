use gears::{
    frame::Frame,
    glam::{Mat4, Vec3},
    io::input_state::InputState,
    loops::frame::{FrameLoop, FrameLoopTarget},
    renderer::{
        buffer::StagedBuffer, object::load_obj, simple_renderer::Renderer, FramePerfReport,
    },
    vulkano::{buffer::BufferUsage, sync::GpuFuture},
    SyncMode,
};
use parking_lot::RwLock;
use std::time::Instant;
use winit::event::{KeyboardInput, VirtualKeyCode, WindowEvent};

mod shader {
    use gears::{
        glam::{Mat4, Vec3},
        renderer::simple_renderer::Renderer,
        vulkano::{
            buffer::{BufferUsage, CpuAccessibleBuffer},
            descriptor_set::{
                fixed_size_pool::FixedSizeDescriptorSet, persistent::PersistentDescriptorSetBuf,
                FixedSizeDescriptorSetsPool,
            },
            pipeline::{vertex::BuffersDefinition, GraphicsPipeline, GraphicsPipelineAbstract},
            render_pass::Subpass,
        },
        Input,
    };
    use std::sync::Arc;

    #[derive(Input, Debug, PartialEq, Copy, Clone, Default)]
    #[repr(C)]
    pub struct VertexData {
        pub pos: [f32; 3],
        pub norm: [f32; 3],
    }

    #[derive(Debug, PartialEq, Copy, Clone, Default)]
    #[repr(C)]
    pub struct UniformData {
        pub model_matrix: Mat4,
        pub view_matrix: Mat4,
        pub projection_matrix: Mat4,
        pub light_dir: Vec3,
    }

    gears::modules! {
        vert: {
            ty: "vertex",
            path: "gear/res/default.vert.glsl"
        }
        frag: {
            ty: "fragment",
            path: "gear/res/default.frag.glsl"
        }
    }

    pub struct DefaultPipeline {
        pub pipeline: Arc<GraphicsPipeline<BuffersDefinition>>,
        pub sets: Box<
            [(
                Arc<
                    FixedSizeDescriptorSet<(
                        (),
                        PersistentDescriptorSetBuf<Arc<CpuAccessibleBuffer<UniformData>>>,
                    )>,
                >,
                Arc<CpuAccessibleBuffer<UniformData>>,
            )],
        >,
    }

    impl DefaultPipeline {
        pub fn build(renderer: &Renderer) -> Self {
            let vert = vert::Shader::load(renderer.device.logical().clone()).unwrap();

            let frag = frag::Shader::load(renderer.device.logical().clone()).unwrap();

            let pipeline = Arc::new(
                GraphicsPipeline::start()
                    .vertex_input_single_buffer::<VertexData>()
                    .vertex_shader(vert.main_entry_point(), ())
                    .fragment_shader(frag.main_entry_point(), ())
                    .depth_stencil_simple_depth()
                    .render_pass(Subpass::from(renderer.render_pass().clone(), 0).unwrap())
                    .viewports_dynamic_scissors_irrelevant(1)
                    .build(renderer.device.logical().clone())
                    .unwrap(),
            );

            let layout = pipeline.layout().descriptor_set_layouts()[0].clone();
            let mut desc_set_pool = FixedSizeDescriptorSetsPool::new(layout);

            let sets = (0..renderer.image_count())
                .map(|_| {
                    let uniform_buffer = CpuAccessibleBuffer::from_data(
                        renderer.device.logical().clone(),
                        BufferUsage::uniform_buffer(),
                        false,
                        UniformData::default(),
                    )
                    .unwrap();
                    let set = Arc::new(
                        desc_set_pool
                            .next()
                            .add_buffer(uniform_buffer.clone())
                            .unwrap()
                            .build()
                            .unwrap(),
                    );

                    (set, uniform_buffer)
                })
                .collect::<Box<_>>();

            Self { pipeline, sets }
        }
    }

    /* pipeline! {
        "DefaultPipeline"
        VertexData -> RGBAOutput
        mod "VERT" as "vert" where { in UniformData as 0 }
        mod "FRAG" as "frag"
    } */
}

struct App {
    frame: Frame,
    renderer: Renderer,
    input: RwLock<InputState>,

    shader: shader::DefaultPipeline,
    vb: StagedBuffer<[shader::VertexData]>,

    delta_time: RwLock<Instant>,
    distance: RwLock<f32>,
    position: RwLock<Vec3>,
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

            delta_time: RwLock::new(Instant::now()),
            distance: RwLock::new(2.5),
            position: RwLock::new(Vec3::new(0.0, 0.0, 0.0)),
        }
    }

    fn vertex_data() -> Vec<shader::VertexData> {
        load_obj(include_str!("../res/gear.obj"), None, |position, normal| {
            shader::VertexData {
                pos: position.to_array(),
                norm: normal.to_array(),
            }
        })
    }

    fn reload_mesh(&self) {
        /* let vertices = Self::vertex_data();
        if vertices.len() as u64 > self.vb.len() {
            self.vb = StagedBuffer::from_iter(
                &self.renderer.device,
                BufferUsage::vertex_buffer(),
                vertices.into_iter(),
            )
            .unwrap();
        } else {
            self.vb.write(recorder)
        }

        self.vb
            .self
            .vb
            .write()
            .unwrap()
            .copy_from_slice(&vertices[..vertices.len().min(MAX_VBO_LEN)])
            .unwrap(); */
    }

    fn update_uniform_buffer(&self, image_index: usize, future: &mut dyn GpuFuture) {
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

        // non spin lock
        // let mut lock = self.shader.sets[image_index].1.write().unwrap();

        // spin lock
        // spinlocking seems to be faster than waiting for future
        let mut lock = loop {
            // hopefully just a temporary™ spinlock
            match self.shader.sets[image_index].1.write() {
                Ok(lock) => break lock,
                Err(_) => {}
            }

            future.cleanup_finished();
        };

        *lock = ubo;
    }
}

impl FrameLoopTarget for App {
    fn frame(&mut self) -> Option<FramePerfReport> {
        let mut frame = self.renderer.begin_frame()?;

        // outside of render pass
        let mut recorder = frame.recorder;
        self.vb.update(&mut recorder).unwrap();
        self.update_uniform_buffer(frame.image_index, &mut frame.future);

        // inside of render pass
        let mut recorder = recorder.begin_render_pass();
        recorder
            .record()
            .draw(
                self.shader.pipeline.clone(),
                &frame.dynamic,
                self.vb.local.clone(),
                self.shader.sets[frame.image_index].0.clone(),
                (),
            )
            .unwrap();

        // outside of render pass again
        let recorder = recorder.end_render_pass();
        frame.recorder = recorder;

        self.renderer.end_frame(frame)
    }

    fn event(&mut self, event: &WindowEvent) {
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

fn main() {
    env_logger::init();

    let (frame, event_loop) = Frame::new()
        .with_title("Simple Example")
        .with_size(600, 600)
        .build();

    let context = frame.default_context().unwrap();

    let renderer = Renderer::new()
        .with_sync(SyncMode::Immediate)
        .build(context)
        .unwrap();

    let app = App::init(frame, renderer);

    FrameLoop::new(event_loop, Box::new(app)).run();
}
