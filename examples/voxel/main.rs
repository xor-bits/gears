use std::collections::HashMap;

use cgmath::{perspective, Deg, EuclideanSpace, InnerSpace, Matrix4, Point3, Vector2, Vector3};
use gears::{
    input_state::InputState,
    renderer::{
        buffer::VertexBuffer,
        pipeline::{Pipeline, PipelineBuilder},
        FrameInfo,
    },
    Application, Gears, GearsRenderer, VSync, B, UPS,
};
use instant::Instant;
use noise::{NoiseFn, Seedable};
use winit::{
    dpi::{LogicalPosition, PhysicalPosition},
    event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent},
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
        vs: { path: "voxel/res/default.vert.glsl" }
        fs: { path: "voxel/res/default.frag.glsl" }
    }
}

const UPDATES_PER_SECOND: u32 = 60;

enum Lighting {
    Top,
    Bottom,
    X,
    Z,
}

struct App {
    vb: VertexBuffer<B>,
    shaders: (Pipeline<B>, Pipeline<B>),

    input: InputState,

    look_dir: Vector2<f32>,
    position: Point3<f32>,
    velocity: Vector3<f32>,

    focused: bool,
    debug: bool,

    ups: UPS,
}

impl Lighting {
    fn to_raw(&self) -> u32 {
        match self {
            Self::Top => 3 << 21,
            Self::Z => 2 << 21,
            Self::X => 1 << 21,
            Self::Bottom => 0 << 21,
        }
    }
}

fn to_u7_vec(x: u8, y: u8, z: u8) -> u32 {
    (x as u32 & 0x7F) | ((y as u32 & 0x7F) << 7) | ((z as u32 & 0x7F) << 14)
}

fn generate() -> Vec<shader::VertexData> {
    const WIDTH: usize = 64;
    const HEIGHT: usize = 64;
    const DEPTH: usize = 64;
    const QUADS: usize = WIDTH * HEIGHT * DEPTH;
    const VERT_PER_QUAD: usize = 6;
    const QUAD_PER_CUBE: usize = 6;
    const VERT_PER_CUBE: usize = VERT_PER_QUAD * QUAD_PER_CUBE;
    const MAX_VERT: usize = VERT_PER_CUBE * QUADS;

    // generate quad vertices
    let quad = |x: u8,
                y: u8,
                z: u8,
                sx: u8,
                sy: u8,
                sz: u8,
                inv: bool,
                light: Lighting,
                vertices: &mut Vec<shader::VertexData>| {
        let light_raw = light.to_raw();
        let (al, bl, ar, br) = if inv { (0, 1, 1, 0) } else { (1, 0, 1, 0) };

        let varying_ax = if sx == 1 { 1 } else { 0 };
        let varying_bx = if sx == 2 { 1 } else { 0 };
        let varying_ay = if sy == 1 { 1 } else { 0 };
        let varying_by = if sy == 2 { 1 } else { 0 };
        let varying_az = if sz == 1 { 1 } else { 0 };
        let varying_bz = if sz == 2 { 1 } else { 0 };

        vertices.push(shader::VertexData {
            raw_data: to_u7_vec(
                x + al * varying_ax + ar * varying_bx,
                y + al * varying_ay + ar * varying_by,
                z + al * varying_az + ar * varying_bz,
            ) | light_raw,
        });
        vertices.push(shader::VertexData {
            raw_data: to_u7_vec(
                x + al * varying_ax + br * varying_bx,
                y + al * varying_ay + br * varying_by,
                z + al * varying_az + br * varying_bz,
            ) | light_raw,
        });
        vertices.push(shader::VertexData {
            raw_data: to_u7_vec(
                x + bl * varying_ax + br * varying_bx,
                y + bl * varying_ay + br * varying_by,
                z + bl * varying_az + br * varying_bz,
            ) | light_raw,
        });
        vertices.push(shader::VertexData {
            raw_data: to_u7_vec(
                x + al * varying_ax + ar * varying_bx,
                y + al * varying_ay + ar * varying_by,
                z + al * varying_az + ar * varying_bz,
            ) | light_raw,
        });
        vertices.push(shader::VertexData {
            raw_data: to_u7_vec(
                x + bl * varying_ax + br * varying_bx,
                y + bl * varying_ay + br * varying_by,
                z + bl * varying_az + br * varying_bz,
            ) | light_raw,
        });
        vertices.push(shader::VertexData {
            raw_data: to_u7_vec(
                x + bl * varying_ax + ar * varying_bx,
                y + bl * varying_ay + ar * varying_by,
                z + bl * varying_az + ar * varying_bz,
            ) | light_raw,
        });
    };

    // generate cube vertices
    let cube = |x: u8,
                y: u8,
                z: u8,
                neg_x: bool,
                pos_x: bool,
                neg_y: bool,
                pos_y: bool,
                neg_z: bool,
                pos_z: bool,
                vertices: &mut Vec<shader::VertexData>| {
        if neg_x {
            quad(x, y, z, 0, 1, 2, false, Lighting::X, vertices);
        }
        if pos_x {
            quad(x + 1, y, z, 0, 1, 2, true, Lighting::X, vertices);
        }
        if neg_y {
            quad(x, y, z, 1, 0, 2, true, Lighting::Top, vertices);
        }
        if pos_y {
            quad(x, y + 1, z, 1, 0, 2, false, Lighting::Bottom, vertices);
        }
        if neg_z {
            quad(x, y, z, 1, 2, 0, false, Lighting::Z, vertices);
        }
        if pos_z {
            quad(x, y, z + 1, 1, 2, 0, true, Lighting::Z, vertices);
        }
    };
    // fill voxels randomly
    let mut noise = noise::Fbm::new();
    // let mut noise = noise::SuperSimplex::new();
    // let mut noise = noise::Perlin::new();
    noise = noise.set_seed(rand::random());
    let mut voxels = [[[false; WIDTH]; HEIGHT]; DEPTH];
    for (voxel_z, voxel_zi) in voxels.iter_mut().enumerate() {
        for (voxel_y, voxel_yi) in voxel_zi.iter_mut().enumerate() {
            for (voxel_x, voxel) in voxel_yi.iter_mut().enumerate() {
                *voxel = noise.get([
                    voxel_x as f64 * 0.03,
                    voxel_y as f64 * 0.03,
                    voxel_z as f64 * 0.03,
                ]) > 0.0;
            }
        }
    }

    // generate cubes
    let mut vertices = Vec::with_capacity(MAX_VERT);
    for (voxel_z, voxel_zi) in voxels.iter().enumerate() {
        for (voxel_y, voxel_yi) in voxel_zi.iter().enumerate() {
            for (voxel_x, voxel) in voxel_yi.iter().enumerate() {
                if !voxel {
                    continue;
                }

                let neg_x = voxel_x == 0 || !voxels[voxel_z][voxel_y][voxel_x - 1];
                let pos_x = voxel_x == WIDTH - 1 || !voxels[voxel_z][voxel_y][voxel_x + 1];
                let neg_y = voxel_y == 0 || !voxels[voxel_z][voxel_y - 1][voxel_x];
                let pos_y = voxel_y == HEIGHT - 1 || !voxels[voxel_z][voxel_y + 1][voxel_x];
                let neg_z = voxel_z == 0 || !voxels[voxel_z - 1][voxel_y][voxel_x];
                let pos_z = voxel_z == DEPTH - 1 || !voxels[voxel_z + 1][voxel_y][voxel_x];

                cube(
                    voxel_x as u8,
                    voxel_y as u8,
                    voxel_z as u8,
                    /* true, */ neg_x,
                    /* true, */ pos_x,
                    /* true, */ neg_y,
                    /* true, */ pos_y,
                    /* true, */ neg_z,
                    /* true, */ pos_z,
                    &mut vertices,
                );
            }
        }
    }
    vertices
}

impl Application for App {
    fn init(input: InputState, renderer: &mut GearsRenderer<B>) -> Self {
        let vertices = generate();

        let mut vb = VertexBuffer::new::<shader::VertexData>(renderer, vertices.len().max(1));
        vb.write(0, &vertices[..]);

        Self {
            vb,
            shaders: (
                PipelineBuilder::new(renderer)
                    .with_input::<shader::VertexData>()
                    .with_module_vert(shader::VERT_SPIRV)
                    .with_module_frag(shader::FRAG_SPIRV)
                    .with_ubo::<shader::UBO>()
                    .build(false),
                PipelineBuilder::new(renderer)
                    .with_input::<shader::VertexData>()
                    .with_module_vert(shader::VERT_SPIRV)
                    .with_module_frag(shader::FRAG_SPIRV)
                    .with_ubo::<shader::UBO>()
                    .build(true),
            ),

            input,

            look_dir: Vector2::new(0.0, 0.0),
            position: Point3::new(0.0, 0.0, 0.0),
            velocity: Vector3::new(0.0, 0.0, 0.0),

            focused: false,
            debug: false,

            ups: UPS::new(UPDATES_PER_SECOND),
        }
    }

    fn event(&mut self, event: &WindowEvent, window: &Window, renderer: &mut GearsRenderer<B>) {
        self.input.update(event, window);

        let focused = self.input.window_focused();
        if self.focused != focused {
            self.focused = focused;
            window.set_cursor_visible(!self.focused);
        }

        let middle = LogicalPosition {
            x: self.input.window_size().width / 2,
            y: self.input.window_size().height / 2,
        }
        .to_physical::<f32>(window.scale_factor());

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
                let vertices = generate();

                if self.vb.size::<shader::VertexData>() < vertices.len() {
                    renderer.wait();
                    self.vb =
                        VertexBuffer::new::<shader::VertexData>(renderer, vertices.len().max(1));
                }
                self.vb.write(0, &vertices[..]);
            }
            (WindowEvent::CursorMoved { position, .. }, true) => {
                let centered_position = PhysicalPosition::new(
                    (position.x as f32 - middle.x) * 0.001,
                    (position.y as f32 - middle.y) * 0.001,
                );

                if !(centered_position.x == 0.0 && centered_position.y == 0.0) && self.focused {
                    self.look_dir -= Vector2::new(centered_position.x, centered_position.y);

                    self.look_dir.y = self.look_dir.y.clamp(
                        -std::f32::consts::PI / 2.0 + 0.0001,
                        std::f32::consts::PI / 2.0 - 0.0001,
                    );

                    window.set_cursor_position(middle).unwrap();
                }
            }
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

        let shader = if self.debug {
            &mut self.shaders.1
        } else {
            &mut self.shaders.0
        };

        shader.write_ubo(ubo, frame.frame_in_flight);
        shader.bind(frame.commands, frame.frame_in_flight);
        self.vb.draw(frame.commands);
    }

    fn update(&mut self, ups: &UPS) {
        let dt_s = ups.update_time.as_secs_f32();

        let dir = Vector3::new(
            self.look_dir.y.cos() * self.look_dir.x.sin(),
            self.look_dir.y.sin(),
            self.look_dir.y.cos() * self.look_dir.x.cos(),
        );
        let up = Vector3::new(0.0, 1.0, 0.0);

        let speed = if self.input.key_held(VirtualKeyCode::LShift) {
            100.0
        } else {
            10.0
        } * dt_s;
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
