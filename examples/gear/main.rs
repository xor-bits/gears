use std::{collections::HashMap, time::Instant};

use cgmath::{perspective, Deg, InnerSpace, Matrix4, Point3, Rad, Vector3};
use gears::{
    input_state::InputState,
    renderer::{
        buffer::VertexBuffer,
        object::load_obj,
        pipeline::{Pipeline, PipelineBuilder},
        FrameInfo,
    },
    Application, Gears, GearsRenderer, VSync, B, UPS,
};
use winit::{
    event::{KeyboardInput, VirtualKeyCode, WindowEvent},
    window::Window,
};

#[cfg(target_arch = "wasm32")]
use log::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    main();
}

mod shader {
    gears_pipeline::pipeline! {
        vs: { path: "gear/res/default.vert.glsl" }
        fs: { path: "gear/res/default.frag.glsl" }
    }
}

const MAX_VBO_LEN: usize = 50_000;

struct App {
    vb: VertexBuffer<B>,
    shader: Pipeline<B>,

    input: InputState,

    position: Vector3<f32>,
    velocity: Vector3<f32>,
}

impl App {
    fn reload_mesh(&mut self) {
        let vertices = load_obj(
            include_str!("res/gears_smooth.obj"),
            None,
            |position, normal| shader::VertexData {
                pos: position,
                norm: normal,
            },
        );

        self.vb
            .write(0, &vertices[..vertices.len().min(MAX_VBO_LEN)]);
    }
}

impl Application for App {
    fn init(input: InputState, renderer: &mut GearsRenderer<B>) -> Self {
        let mut app = Self {
            vb: VertexBuffer::new::<shader::VertexData>(renderer, MAX_VBO_LEN),
            shader: PipelineBuilder::new(renderer)
                .with_input::<shader::VertexData>()
                .with_module_vert(shader::VERT_SPIRV)
                .with_module_frag(shader::FRAG_SPIRV)
                .with_ubo::<shader::UBO>()
                .build(false),
            input,
            position: Vector3::new(0.0, 0.0, 0.0),
            velocity: Vector3::new(0.0, 0.0, 0.0),
        };

        app.reload_mesh();

        app
    }

    fn event(&mut self, event: &WindowEvent, window: &Window, _: &mut GearsRenderer<B>) {
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

        self.input.update(event, window);
    }

    fn render(&mut self, frame: &mut FrameInfo<B>, _: &HashMap<UPS, Instant>) {
        let dt_s = frame.delta_time.as_secs_f32();
        self.velocity = Vector3::new(0.0, 0.0, 0.0);
        if self.input.key_held(VirtualKeyCode::A) {
            self.velocity.x += 1.0;
        }
        if self.input.key_held(VirtualKeyCode::D) {
            self.velocity.x -= 1.0;
        }
        if self.input.key_held(VirtualKeyCode::W) {
            self.velocity.y += 1.0;
        }
        if self.input.key_held(VirtualKeyCode::S) {
            self.velocity.y -= 1.0;
        }
        if self.input.key_held(VirtualKeyCode::Space) {
            self.velocity.z += 2.0;
        }
        self.position += self.velocity * 3.0 * dt_s;
        self.position.y = self
            .position
            .y
            .min(std::f32::consts::PI / 2.0 - 0.0001)
            .max(-std::f32::consts::PI / 2.0 + 0.0001);

        let eye = Point3::new(
            self.position.x.sin() * self.position.y.cos(),
            self.position.y.sin(),
            self.position.x.cos() * self.position.y.cos(),
        ) * 2.5;
        let focus = Point3::new(0.0, 0.0, 0.0);

        let ubo = shader::UBO {
            model_matrix: Matrix4::from_angle_x(Rad { 0: self.position.z }),
            view_matrix: Matrix4::look_at_rh(eye, focus, Vector3::new(0.0, -1.0, 0.0)),
            projection_matrix: perspective(Deg { 0: 60.0 }, frame.aspect, 0.01, 100.0),
            light_dir: Vector3::new(0.2, 2.0, 0.5).normalize(),
        };

        self.shader.write_ubo(ubo, frame.frame_in_flight);
        self.shader.bind(frame.commands, frame.frame_in_flight);
        self.vb.draw(frame.commands);
    }

    fn update(&mut self, _: &UPS) {}
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    #[cfg(target_arch = "wasm32")]
    wasm_logger::init(
        wasm_logger::Config::new(Level::Debug), /* .module_prefix("main")
                                                .module_prefix("gears::renderer") */
    );
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    Gears::new().with_vsync(VSync::Off).run_with::<App>();
}
