use std::collections::HashMap;

use cgmath::{perspective, Deg, InnerSpace, Matrix4, Point3, Rad, Vector2, Vector3};
use gears::{
    renderer::{
        buffer::VertexBuffer,
        object::load_obj,
        pipeline::{Pipeline, PipelineBuilder},
        FrameInfo,
    },
    Application, ElementState, Gears, GearsRenderer, KeyboardInput, VSync, VirtualKeyCode,
    WindowEvent, B,
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
        vs: { path: "main/res/default.vert.glsl" }
        fs: { path: "main/res/default.frag.glsl" }
    }
}

const MAX_VBO_LEN: usize = 50_000;

struct App {
    vb: VertexBuffer<B>,
    shader: Pipeline<B>,

    keymap: HashMap<VirtualKeyCode, bool>,

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

    fn get_key(&self, key: VirtualKeyCode) -> bool {
        if let Some(value) = self.keymap.get(&key) {
            *value
        } else {
            false
        }
    }
}

impl Application for App {
    fn init(renderer: &mut GearsRenderer<B>) -> Self {
        let mut app = Self {
            vb: VertexBuffer::new::<shader::VertexData>(renderer, MAX_VBO_LEN),
            shader: PipelineBuilder::new(renderer)
                .with_input::<shader::VertexData>()
                .with_module_vert(shader::VERT_SPIRV)
                .with_module_frag(shader::FRAG_SPIRV)
                .with_ubo::<shader::UBO>()
                .build(),
            keymap: HashMap::new(),
            position: Vector3::new(0.0, 0.0, 0.0),
            velocity: Vector3::new(0.0, 0.0, 0.0),
        };

        app.reload_mesh();

        app
    }

    fn event(&mut self, event: WindowEvent) {
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

        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(k),
                        state,
                        ..
                    },
                ..
            } => {
                self.keymap.insert(
                    k,
                    match state {
                        ElementState::Pressed => true,
                        _ => false,
                    },
                );
            }
            _ => {}
        }
    }

    fn render(&mut self, frame_info: FrameInfo, frame: &mut gears::FrameCommands<B>, fifi: usize) {
        let dt_s = frame_info.delta_time.as_secs_f32();
        self.velocity = Vector3::new(0.0, 0.0, 0.0);
        if self.get_key(VirtualKeyCode::A) {
            self.velocity.x += 1.0;
        }
        if self.get_key(VirtualKeyCode::D) {
            self.velocity.x -= 1.0;
        }
        if self.get_key(VirtualKeyCode::W) {
            self.velocity.y += 1.0;
        }
        if self.get_key(VirtualKeyCode::S) {
            self.velocity.y -= 1.0;
        }
        if self.get_key(VirtualKeyCode::Space) {
            self.velocity.z += 2.0;
        }
        self.position += self.velocity * 3.0 * dt_s;
        self.position.y = self
            .position
            .y
            .min(std::f32::consts::PI / 2.0 - f32::EPSILON)
            .max(-std::f32::consts::PI / 2.0 + f32::EPSILON);

        let ubo = shader::UBO {
            model_matrix: Matrix4::from_angle_x(Rad { 0: self.position.z }),
            view_matrix: Matrix4::look_at_rh(
                Point3::new(
                    self.position.x.sin() * self.position.y.cos(),
                    self.position.y.sin(),
                    self.position.x.cos() * self.position.y.cos(),
                ) * 2.5,
                Point3::new(0.0, 0.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
            ),
            projection_matrix: perspective(Deg { 0: 60.0 }, frame_info.aspect, 0.01, 100.0),
            light_dir: Vector3::new(0.2, 2.0, 0.5).normalize(),
        };
        self.shader.write_ubo(ubo, fifi);

        self.shader.bind(frame, fifi);
        self.vb.draw(frame);
    }
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
