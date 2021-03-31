use cgmath::{
    perspective, Deg, EuclideanSpace, InnerSpace, Matrix4, Point3, Rad, Vector2, Vector3, Vector4,
};
use gears::{
    input_state::InputState,
    renderer::{
        buffer::VertexBuffer,
        pipeline::{Pipeline, PipelineBuilder},
        FrameInfo,
    },
    Application, Gears, GearsRenderer, VSync, B,
};
use noise::{Fbm, NoiseFn};
use winit::{
    dpi::{LogicalPosition, PhysicalPosition},
    event::{VirtualKeyCode, WindowEvent},
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

struct App {
    vb: VertexBuffer<B>,
    shader: Pipeline<B>,

    input: InputState,

    look_dir: Vector2<f32>,
    position: Point3<f32>,

    focused: bool,
}

impl Application for App {
    fn init(input: InputState, renderer: &mut GearsRenderer<B>) -> Self {
        const WIDTH: usize = 40;
        const HEIGHT: usize = 40;
        const DEPTH: usize = 40;
        const QUADS: usize = WIDTH * HEIGHT * DEPTH;
        const VERT_PER_QUAD: usize = 6;
        const QUAD_PER_CUBE: usize = 6;
        const VERT_PER_CUBE: usize = VERT_PER_QUAD * QUAD_PER_CUBE;
        const MAX_VERT: usize = VERT_PER_CUBE * QUADS;

        // output vertices
        let mut vertices = Vec::with_capacity(MAX_VERT);

        // generate quad vertices
        let mut quad = |mat: Matrix4<f32>, norm: Vector3<f32>| {
            vertices.push(shader::VertexData {
                pos: (mat * Vector4::new(-0.5, 0.5, 0.5, 1.0)).xyz(),
                norm,
            });
            vertices.push(shader::VertexData {
                pos: (mat * Vector4::new(-0.5, 0.5, -0.5, 1.0)).xyz(),
                norm,
            });
            vertices.push(shader::VertexData {
                pos: (mat * Vector4::new(-0.5, -0.5, -0.5, 1.0)).xyz(),
                norm,
            });
            vertices.push(shader::VertexData {
                pos: (mat * Vector4::new(-0.5, 0.5, 0.5, 1.0)).xyz(),
                norm,
            });
            vertices.push(shader::VertexData {
                pos: (mat * Vector4::new(-0.5, -0.5, -0.5, 1.0)).xyz(),
                norm,
            });
            vertices.push(shader::VertexData {
                pos: (mat * Vector4::new(-0.5, -0.5, 0.5, 1.0)).xyz(),
                norm,
            });
        };

        // generate cube vertices
        let mut cube = |mat: Matrix4<f32>,
                        neg_x: bool,
                        pos_x: bool,
                        neg_y: bool,
                        pos_y: bool,
                        neg_z: bool,
                        pos_z: bool| {
            if neg_x {
                quad(mat * Matrix4::from_scale(1.0), Vector3::new(-1.0, 0.0, 0.0));
            }
            if pos_x {
                quad(
                    mat * Matrix4::from_angle_y(Rad {
                        0: std::f32::consts::PI,
                    }),
                    Vector3::new(1.0, 0.0, 0.0),
                );
            }
            if neg_y {
                quad(
                    mat * Matrix4::from_angle_z(Rad {
                        0: std::f32::consts::FRAC_PI_2,
                    }),
                    Vector3::new(0.0, 1.0, 0.0),
                );
            }
            if pos_y {
                quad(
                    mat * Matrix4::from_angle_z(Rad {
                        0: -std::f32::consts::FRAC_PI_2,
                    }),
                    Vector3::new(0.0, -1.0, 0.0),
                );
            }
            if neg_z {
                quad(
                    mat * Matrix4::from_angle_y(Rad {
                        0: -std::f32::consts::FRAC_PI_2,
                    }),
                    Vector3::new(0.0, 0.0, 1.0),
                );
            }
            if pos_z {
                quad(
                    mat * Matrix4::from_angle_y(Rad {
                        0: std::f32::consts::FRAC_PI_2,
                    }),
                    Vector3::new(0.0, 0.0, -1.0),
                );
            }
        };
        // fill voxels randomly
        let mut fbm = Fbm::new();
        fbm.frequency = 0.03;
        let mut voxels = [[[false; WIDTH]; HEIGHT]; DEPTH];
        for (voxel_z, voxel_zi) in voxels.iter_mut().enumerate() {
            for (voxel_y, voxel_yi) in voxel_zi.iter_mut().enumerate() {
                for (voxel_x, voxel) in voxel_yi.iter_mut().enumerate() {
                    *voxel = fbm.get([voxel_x as f64, voxel_y as f64, voxel_z as f64]) > 0.0;
                }
            }
        }

        // generate cubes
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
                        Matrix4::from_translation(Vector3::new(
                            voxel_x as f32 - WIDTH as f32 / 2.0,
                            voxel_y as f32 - HEIGHT as f32 / 2.0,
                            voxel_z as f32 - DEPTH as f32 / 2.0,
                        )),
                        neg_x,
                        pos_x,
                        neg_y,
                        pos_y,
                        neg_z,
                        pos_z,
                    );
                }
            }
        }

        let mut vb = VertexBuffer::new::<shader::VertexData>(renderer, vertices.len());
        vb.write(0, &vertices[..]);

        println!("triangles: {}", vertices.len() / 3);

        Self {
            vb,
            shader: PipelineBuilder::new(renderer)
                .with_input::<shader::VertexData>()
                .with_module_vert(shader::VERT_SPIRV)
                .with_module_frag(shader::FRAG_SPIRV)
                .with_ubo::<shader::UBO>()
                .build(true),

            input,

            look_dir: Vector2::new(0.0, 0.0),
            position: Point3::new(0.0, 0.0, 0.0),

            focused: false,
        }
    }

    fn event(&mut self, event: &WindowEvent, window: &Window) {
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
            (WindowEvent::CursorMoved { position, .. }, true) => {
                let centered_position = PhysicalPosition::new(
                    (position.x as f32 - middle.x) * 0.002,
                    (position.y as f32 - middle.y) * 0.002,
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

    fn render(&mut self, frame: &mut FrameInfo<B>) {
        let dt_s = frame.delta_time.as_secs_f32();

        let dir = Vector3::new(
            self.look_dir.y.cos() * self.look_dir.x.sin(),
            self.look_dir.y.sin(),
            self.look_dir.y.cos() * self.look_dir.x.cos(),
        );
        let eye = self.position;
        let focus = (eye - dir).to_vec();
        let focus = Point3::from_vec(focus);
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

        let ubo = shader::UBO {
            model_matrix: Matrix4::from_scale(1.0),
            view_matrix: Matrix4::look_at_rh(eye, focus, up),
            projection_matrix: perspective(Deg { 0: 60.0 }, frame.aspect, 0.01, 100.0),
            light_dir: Vector3::new(0.2, 2.0, 0.1).normalize(),
        };

        self.shader.write_ubo(ubo, frame.frame_in_flight);
        self.shader.bind(frame.commands, frame.frame_in_flight);
        self.vb.draw(frame.commands);
    }

    fn update(&mut self, _: gears::UpsThread) {}
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
