/// controls:
/// - W,A,S,D,Space,C to move around
/// - Mouse to look around
/// - R to regenerate voxels with new seed
/// - N to generate cube mesh
/// - M to generate marching cubes mesh
/// - Tab to toggle wireframe
use std::collections::HashMap;

use cgmath::{perspective, Deg, EuclideanSpace, InnerSpace, Matrix4, Point3, Vector2, Vector3};
use cubes::generate_cubes;
use gears::{
    input_state::InputState,
    renderer::{
        buffer::{IndexBuffer, VertexBuffer},
        pipeline::{Pipeline, PipelineBuilder},
        FrameInfo,
    },
    Application, Gears, GearsRenderer, VSync, B, UPS,
};
use instant::Instant;
use marching_cubes::generate_marching_cubes;
use simdnoise::NoiseBuilder;
use winit::{
    dpi::{LogicalPosition, PhysicalPosition},
    event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent},
    window::Window,
};

mod cubes;
mod marching_cubes;

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

enum Lighting {
    Top,
    Bottom,
    X,
    Z,
}

struct App {
    vb: VertexBuffer<shader::VertexData, B>,
    ib: IndexBuffer<B>,
    shaders: (Pipeline<B>, Pipeline<B>),

    input: InputState,

    look_dir: Vector2<f32>,
    position: Point3<f32>,
    velocity: Vector3<f32>,

    focused: bool,
    next_ignored_value: Option<PhysicalPosition<f64>>,
    next_ignored: bool,
    debug: bool,
    voxels: Vec<f32>,

    ups: UPS,
}

impl Lighting {
    fn to_exposure(&self) -> f32 {
        match self {
            Self::Top => 1.0,
            Self::Z => 0.75,
            Self::X => 0.5,
            Self::Bottom => 0.25,
        }
    }
}

fn generate_voxels(seed: i32) -> Vec<f32> {
    let voxels = NoiseBuilder::fbm_3d(WIDTH, HEIGHT, DEPTH)
        .with_freq(0.02)
        .with_octaves(4)
        .with_gain(0.8)
        .with_lacunarity(1.5)
        .with_seed(seed)
        .generate_scaled(0.0, 1.0);
    voxels
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            let x = i % WIDTH;
            let y = (i / WIDTH) % HEIGHT;
            let z = i / (WIDTH * HEIGHT);

            let fade_x = 1.0 - (2.0 / WIDTH as f32 * x as f32 - 1.0).powf(4.0);
            let fade_y = 1.0 - (2.0 / HEIGHT as f32 * y as f32 - 1.0).powf(4.0);
            let fade_z = 1.0 - (2.0 / DEPTH as f32 * z as f32 - 1.0).powf(4.0);

            let fade = fade_x * fade_y * fade_z;
            /* let fade = 1.0; */

            v * fade
        })
        .collect::<Vec<_>>()
}

fn point_to_index(x: usize, y: usize, z: usize) -> usize {
    x + y * WIDTH + z * WIDTH * HEIGHT
}

impl App {
    fn remesh(
        &mut self,
        renderer: &GearsRenderer<B>,
        vertices: Vec<shader::VertexData>,
        indices: Vec<u32>,
    ) {
        // TODO: impl VertexBuffer::resize
        let vb_resize = self.vb.len() < vertices.len();
        let ib_resize = self.ib.len() < indices.len();
        if vb_resize || ib_resize {
            renderer.wait();
        }
        if vb_resize {
            self.vb = VertexBuffer::new_with_data(renderer, &vertices[..]).unwrap();
        } else {
            self.vb.write(0, &vertices[..]).unwrap();
        }
        if ib_resize {
            self.ib = IndexBuffer::new_with_data(renderer, &indices[..]).unwrap();
        } else {
            self.ib.write(0, &indices[..]).unwrap();
        }
    }
}

impl Application for App {
    fn init(input: InputState, renderer: &mut GearsRenderer<B>) -> Self {
        let voxels = generate_voxels(0);
        let (vertices, indices) = generate_cubes(&voxels);

        let vb = VertexBuffer::new_with_data(renderer, &vertices[..]).unwrap();
        let ib = IndexBuffer::new_with_data(renderer, &indices[..]).unwrap();

        let fill_shader = PipelineBuilder::new(renderer)
            .with_ubo::<shader::UBO>()
            .with_graphics_modules(shader::VERT_SPIRV, shader::FRAG_SPIRV)
            .with_input::<shader::VertexData>()
            .build(false)
            .unwrap();
        let line_shader = PipelineBuilder::new(renderer)
            .with_ubo::<shader::UBO>()
            .with_graphics_modules(shader::VERT_SPIRV, debug_shader::FRAG_SPIRV)
            .with_geometry_module(shader::GEOM_SPIRV)
            .with_input::<shader::VertexData>()
            .build(false)
            .unwrap();

        Self {
            vb,
            ib,
            shaders: (fill_shader, line_shader),

            input,

            look_dir: Vector2::new(
                -std::f32::consts::FRAC_PI_4 * 3.0,
                -std::f32::consts::PI / 5.0,
            ),
            position: Point3::new(-26.0, -26.0, -26.0),
            velocity: Vector3::new(0.0, 0.0, 0.0),

            focused: false,
            next_ignored_value: None,
            next_ignored: false,
            debug: false,
            voxels,

            ups: UPS::new(UPDATES_PER_SECOND),
        }
    }

    fn event(&mut self, event: &WindowEvent, window: &Window, renderer: &mut GearsRenderer<B>) {
        self.input.update(event, window);

        let middle = LogicalPosition {
            x: self.input.window_size().width / 2,
            y: self.input.window_size().height / 2,
        }
        .to_physical::<f64>(window.scale_factor());

        let focused = self.input.window_focused();
        if self.focused != focused {
            self.focused = focused;
            window.set_cursor_visible(!self.focused);

            // just focused
            if self.focused {
                window.set_cursor_position(middle).unwrap();
                self.next_ignored = true;
            }
        }

        match (event, self.focused) {
            (
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Tab),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                },
                _,
            ) => {
                self.debug = !self.debug;
            }
            (
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::R),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                },
                _,
            ) => {
                let tp = Instant::now();
                self.voxels = generate_voxels(rand::random());
                let (vertices, indices) = generate_cubes(&self.voxels);
                self.remesh(&renderer, vertices, indices);
                println!("Regen and remesh took: {}ms", tp.elapsed().as_millis());
            }
            (
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::N),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                },
                _,
            ) => {
                let tp = Instant::now();
                let (vertices, indices) = generate_cubes(&self.voxels);
                self.remesh(&renderer, vertices, indices);
                println!("Remesh took: {}ms", tp.elapsed().as_millis());
            }
            (
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::M),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                },
                _,
            ) => {
                let tp = Instant::now();
                let (vertices, indices) = generate_marching_cubes(&self.voxels);
                self.remesh(&renderer, vertices, indices);
                println!("Remesh took: {}ms", tp.elapsed().as_millis());
            }
            (WindowEvent::CursorMoved { position, .. }, true) => loop {
                let centered_position = PhysicalPosition::new(
                    (position.x - middle.x) * 0.001,
                    (position.y - middle.y) * 0.001,
                );

                if self.next_ignored {
                    self.next_ignored_value = Some(centered_position);
                    self.next_ignored = false;
                    break;
                }
                if let Some(next_ignored_value) = self.next_ignored_value {
                    if next_ignored_value == centered_position {
                        break;
                    } else {
                        self.next_ignored_value = None;
                    }
                }

                if !(centered_position.x == 0.0 && centered_position.y == 0.0) && self.focused {
                    self.look_dir -=
                        Vector2::new(centered_position.x as f32, centered_position.y as f32);

                    self.look_dir.y = self.look_dir.y.clamp(
                        -std::f32::consts::PI / 2.0 + 0.0001,
                        std::f32::consts::PI / 2.0 - 0.0001,
                    );

                    window.set_cursor_position(middle).unwrap();
                }
                break;
            },
            _ => (),
        }
    }

    fn render(&mut self, frame: &mut FrameInfo<B>, update_tps: &HashMap<UPS, Instant>) {
        let dt_s = update_tps[&self.ups].elapsed().as_secs_f32();

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
            mvp: perspective(Deg { 0: 60.0 }, frame.aspect, 0.01, 500.0)
                * Matrix4::look_at_rh(eye, focus, up)
                * Matrix4::from_scale(1.0),
        };

        self.shaders.0.write_ubo(&ubo, frame.frame_in_flight);
        self.shaders.0.bind(frame.commands, frame.frame_in_flight);
        self.ib.draw(&self.vb, frame.commands);

        if self.debug {
            self.shaders.1.write_ubo(&ubo, frame.frame_in_flight);
            self.shaders.1.bind(frame.commands, frame.frame_in_flight);
            self.ib.draw(&self.vb, frame.commands);
        }
    }

    fn update(&mut self, ups: &UPS) {
        let dt_s = ups.update_time.as_secs_f32();

        let dir = Vector3::new(
            self.look_dir.y.cos() * self.look_dir.x.sin(),
            self.look_dir.y.sin(),
            self.look_dir.y.cos() * self.look_dir.x.cos(),
        );
        let up = Vector3::new(0.0, 1.0, 0.0);

        let speed = {
            let mut speed = 10.0 * dt_s;
            if self.input.key_held(VirtualKeyCode::LShift) {
                speed *= 10.0;
            }
            if self.input.key_held(VirtualKeyCode::LAlt) {
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
        if self.input.key_held(VirtualKeyCode::W) {
            self.position -= dir;
        }
        if self.input.key_held(VirtualKeyCode::S) {
            self.position += dir;
        }
        if self.input.key_held(VirtualKeyCode::A) {
            self.position += dir.cross(up);
        }
        if self.input.key_held(VirtualKeyCode::D) {
            self.position -= dir.cross(up);
        }
        if self.input.key_held(VirtualKeyCode::Space) {
            self.position.y -= speed;
        }
        if self.input.key_held(VirtualKeyCode::C) {
            self.position.y += speed;
        }
        self.velocity = (self.position.to_vec() - self.velocity) / dt_s;
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

    /* let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        generate();
    } */

    Gears::new()
        .with_vsync(VSync::Off)
        .with_ups(UPS::new(UPDATES_PER_SECOND))
        .run_with::<App>();
}
