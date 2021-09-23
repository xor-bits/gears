use gears::{
    frame::Frame,
    glam::{Mat4, Vec3},
    io::input_state::InputState,
    loops::frame::{FrameLoop, FrameLoopTarget},
    renderer::{
        buffer::StagedBuffer, object::load_obj, simple_renderer::Renderer, FramePerfReport,
    },
    vulkano::{buffer::BufferUsage, descriptor_set::DescriptorSetsCollection},
    SyncMode,
};
use shader::UniformData;
use std::time::Instant;
use winit::event::{VirtualKeyCode, WindowEvent};

mod shader {
    use gears::{
        glam::{Mat4, Vec3},
        renderer::simple_renderer::Renderer,
        vulkano::{
            buffer::CpuBufferPool,
            descriptor_set::FixedSizeDescriptorSetsPool,
            pipeline::{vertex::BuffersDefinition, GraphicsPipeline, GraphicsPipelineAbstract},
            render_pass::Subpass,
        },
        Input,
    };
    use std::sync::Arc;

    #[derive(Input, Debug, PartialEq, Copy, Clone, Default)]
    #[repr(C)]
    pub struct VertexData {
        pub pos: Vec3,
        pub norm: Vec3,
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
        pub buffer_pool: CpuBufferPool<UniformData>,
        pub desc_pool: FixedSizeDescriptorSetsPool,
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
            let desc_pool = FixedSizeDescriptorSetsPool::new(layout);
            let buffer_pool =
                CpuBufferPool::<UniformData>::uniform_buffer(renderer.device.logical().clone());

            Self {
                pipeline,
                buffer_pool,
                desc_pool,
            }
        }
    }

    /* TODO: pipeline! {
        "DefaultPipeline"
        VertexData -> RGBAOutput
        mod "VERT" as "vert" where { in UniformData as 0 }
        mod "FRAG" as "frag"
    } */
}

struct App {
    frame: Frame,
    renderer: Renderer,
    input: InputState,

    shader: shader::DefaultPipeline,
    vb: StagedBuffer<[shader::VertexData]>,

    delta_time: Instant,
    distance: f32,
    position: Vec3,
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

            delta_time: Instant::now(),
            distance: 2.5,
            position: Vec3::new(0.0, 0.0, 0.0),
        }
    }

    fn vertex_data() -> Vec<shader::VertexData> {
        // TODO: make a macro for loading objects at compile time
        load_obj(include_str!("../res/gear.obj"), None, |pos, norm| {
            shader::VertexData { pos, norm }
        })
    }

    fn update_uniform_buffer(&mut self) -> impl DescriptorSetsCollection {
        let aspect = self.frame.aspect();
        let dt_s = self.delta_time.elapsed().as_secs_f32();
        self.delta_time = Instant::now();

        let mut distance_delta = 0.0;
        let mut velocity = Vec3::new(0.0, 0.0, 0.0);
        {
            if self.input.key_held(VirtualKeyCode::E) {
                distance_delta += 1.0;
            }
            if self.input.key_held(VirtualKeyCode::Q) {
                distance_delta -= 1.0;
            }
            if self.input.key_held(VirtualKeyCode::A) {
                velocity.x += 1.0;
            }
            if self.input.key_held(VirtualKeyCode::D) {
                velocity.x -= 1.0;
            }
            if self.input.key_held(VirtualKeyCode::W) {
                velocity.y += 1.0;
            }
            if self.input.key_held(VirtualKeyCode::S) {
                velocity.y -= 1.0;
            }
            if self.input.key_held(VirtualKeyCode::Space) {
                velocity.z += 2.0;
            }
        }
        self.distance += distance_delta * 3.0 * dt_s;
        self.position += velocity * 3.0 * dt_s;
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
            model_matrix: Mat4::from_rotation_x(self.position.z),
            view_matrix: Mat4::look_at_rh(eye, focus, up),
            projection_matrix: Mat4::perspective_rh(1.0, aspect, 0.01, 100.0),
            light_dir: Vec3::new(0.2, 2.0, 0.5).normalize(),
        };

        let buffer = self.shader.buffer_pool.next(ubo).unwrap();

        self.shader
            .desc_pool
            .next()
            .add_buffer(buffer)
            .unwrap()
            .build()
            .unwrap()
    }
}

impl FrameLoopTarget for App {
    fn frame(&mut self) -> Option<FramePerfReport> {
        let mut frame = self.renderer.begin_frame()?;

        // outside of render pass
        let mut recorder = frame.recorder;
        self.vb.update(&mut recorder).unwrap();
        let set = self.update_uniform_buffer();

        // inside of render pass
        let mut recorder = recorder.begin_render_pass();
        recorder
            .record()
            .draw(
                self.shader.pipeline.clone(),
                &frame.dynamic,
                self.vb.local.clone(),
                set,
                (),
            )
            .unwrap();

        // outside of render pass again
        let recorder = recorder.end_render_pass();
        frame.recorder = recorder;

        self.renderer.end_frame(frame)
    }

    fn event(&mut self, event: &WindowEvent) {
        self.input.event(event);
        self.frame.event(event);
    }
}

fn main() {
    env_logger::init();

    let (frame, event_loop) = Frame::new()
        .with_title("Simple Example")
        .with_size(600, 600)
        .build();

    let context = frame.default_context();

    let renderer = Renderer::new()
        .with_sync(SyncMode::Immediate)
        .build(context.unwrap())
        .unwrap();

    let app = App::init(frame, renderer);

    FrameLoop::new(event_loop, Box::new(app)).run()
}
