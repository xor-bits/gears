use super::{shader, DEPTH, HEIGHT, WIDTH};
use crate::point_to_index;
use gears::glam::Vec3;

fn triangle(
    p_a: Vec3,
    p_b: Vec3,
    p_c: Vec3,
    vertices: &mut Vec<shader::VertexData>,
    indices: &mut Vec<u32>,
) {
    let ab = p_b - p_a;
    let ac = p_c - p_a;
    let normal = ac.cross(ab).normalize();

    let vi_exp = Vec3::new(0.241_402_27, 0.965_609_1, 0.096_560_91).dot(normal) * 0.375 + 0.625;

    let i = vertices.len();
    vertices.push(shader::VertexData {
        vi_pos: p_a.to_array(),
        vi_exp,
    });
    vertices.push(shader::VertexData {
        vi_pos: p_b.to_array(),
        vi_exp,
    });
    vertices.push(shader::VertexData {
        vi_pos: p_c.to_array(),
        vi_exp,
    });
    indices.push((i) as u32);
    indices.push((i + 1) as u32);
    indices.push((i + 2) as u32);
}

fn quad(
    p_a: Vec3,
    p_b: Vec3,
    p_c: Vec3,
    p_d: Vec3,
    vertices: &mut Vec<shader::VertexData>,
    indices: &mut Vec<u32>,
) {
    triangle(p_a, p_b, p_c, vertices, indices);
    triangle(p_a, p_c, p_d, vertices, indices);
}

fn pentagon(
    p_a: Vec3,
    p_b: Vec3,
    p_c: Vec3,
    p_d: Vec3,
    p_e: Vec3,
    vertices: &mut Vec<shader::VertexData>,
    indices: &mut Vec<u32>,
) {
    quad(p_a, p_b, p_c, p_d, vertices, indices);
    triangle(p_a, p_d, p_e, vertices, indices);
}

fn hexagon(
    p_a: Vec3,
    p_b: Vec3,
    p_c: Vec3,
    p_d: Vec3,
    p_e: Vec3,
    p_f: Vec3,
    vertices: &mut Vec<shader::VertexData>,
    indices: &mut Vec<u32>,
) {
    triangle(p_a, p_b, p_c, vertices, indices);
    triangle(p_a, p_c, p_d, vertices, indices);
    triangle(p_a, p_d, p_e, vertices, indices);
    triangle(p_a, p_e, p_f, vertices, indices);
}

macro_rules! tri {
    ($a:ident $b:ident $c:ident, $vertices:ident, $indices:ident) => {
        triangle($a, $b, $c, &mut $vertices, &mut $indices)
    };
}

macro_rules! qua {
    ($a:ident $b:ident $c:ident $d:ident, $vertices:ident, $indices:ident) => {
        quad($a, $b, $c, $d, &mut $vertices, &mut $indices)
    };
}

macro_rules! pen {
    ($a:ident $b:ident $c:ident $d:ident $e:ident, $vertices:ident, $indices:ident) => {
        pentagon($a, $b, $c, $d, $e, &mut $vertices, &mut $indices)
    };
}

macro_rules! hex {
    ($a:ident $b:ident $c:ident $d:ident $e:ident $f:ident, $vertices:ident, $indices:ident) => {
        hexagon($a, $b, $c, $d, $e, $f, &mut $vertices, &mut $indices)
    };
}

pub fn generate_marching_cubes(
    voxels: &[f32],
    smooth: bool,
) -> (Vec<shader::VertexData>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut missing = 0u64;

    let c_a = Vec3::new(0.0, 0.0, 0.0);
    let c_b = Vec3::new(1.0, 0.0, 0.0);
    let c_c = Vec3::new(0.0, 1.0, 0.0);
    let c_d = Vec3::new(1.0, 1.0, 0.0);

    let c_e = Vec3::new(0.0, 0.0, 1.0);
    let c_f = Vec3::new(1.0, 0.0, 1.0);
    let c_g = Vec3::new(0.0, 1.0, 1.0);
    let c_h = Vec3::new(1.0, 1.0, 1.0);

    for z in 0..DEPTH - 1 {
        for y in 0..HEIGHT - 1 {
            for x in 0..WIDTH - 1 {
                let pv_a = voxels[point_to_index(x, y, z)];
                let pv_b = voxels[point_to_index(x + 1, y, z)];
                let pv_c = voxels[point_to_index(x, y + 1, z)];
                let pv_d = voxels[point_to_index(x + 1, y + 1, z)];
                let pv_e = voxels[point_to_index(x, y, z + 1)];
                let pv_f = voxels[point_to_index(x + 1, y, z + 1)];
                let pv_g = voxels[point_to_index(x, y + 1, z + 1)];
                let pv_h = voxels[point_to_index(x + 1, y + 1, z + 1)];

                let exists = |v: f32| v > 0.5;
                let p_a = exists(pv_a);
                let p_b = exists(pv_b);
                let p_c = exists(pv_c);
                let p_d = exists(pv_d);
                let p_e = exists(pv_e);
                let p_f = exists(pv_f);
                let p_g = exists(pv_g);
                let p_h = exists(pv_h);

                let origin = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);

                let lerp = |a: &Vec3, b: &Vec3, av: f32, bv: f32| {
                    let t = if smooth { (0.5 - bv) / (av - bv) } else { 0.5 };
                    origin + (*a) * t + (*b) * (1.0 - t)
                };

                let a = lerp(&c_a, &c_c, pv_a, pv_c);
                let b = lerp(&c_b, &c_d, pv_b, pv_d);
                let c = lerp(&c_a, &c_b, pv_a, pv_b);
                let d = lerp(&c_c, &c_d, pv_c, pv_d);
                let e = lerp(&c_e, &c_g, pv_e, pv_g);
                let f = lerp(&c_f, &c_h, pv_f, pv_h);
                let g = lerp(&c_e, &c_f, pv_e, pv_f);
                let h = lerp(&c_g, &c_h, pv_g, pv_h);
                let i = lerp(&c_a, &c_e, pv_a, pv_e);
                let j = lerp(&c_b, &c_f, pv_b, pv_f);
                let k = lerp(&c_c, &c_g, pv_c, pv_g);
                let l = lerp(&c_d, &c_h, pv_d, pv_h);

                match (p_a, p_b, p_c, p_d, p_e, p_f, p_g, p_h) {
                    // no faces
                    (false, false, false, false, false, false, false, false)
                    | (true, true, true, true, true, true, true, true) => {}

                    // single corners
                    // a    b      c      d      e      f      g      h
                    (true, false, false, false, false, false, false, false) => {
                        tri!(a i c, vertices, indices);
                    }
                    (false, true, false, false, false, false, false, false) => {
                        tri!(b c j, vertices, indices);
                    }
                    (false, false, true, false, false, false, false, false) => {
                        tri!(a d k, vertices, indices);
                    }
                    (false, false, false, true, false, false, false, false) => {
                        tri!(b l d, vertices, indices);
                    }
                    (false, false, false, false, true, false, false, false) => {
                        tri!(e g i, vertices, indices);
                    }
                    (false, false, false, false, false, true, false, false) => {
                        tri!(f j g, vertices, indices);
                    }
                    (false, false, false, false, false, false, true, false) => {
                        tri!(e k h, vertices, indices);
                    }
                    (false, false, false, false, false, false, false, true) => {
                        tri!(f h l, vertices, indices);
                    }

                    // inverted single corners
                    // a    b      c      d      e      f      g      h
                    (false, true, true, true, true, true, true, true) => {
                        tri!(a c i, vertices, indices);
                    }
                    (true, false, true, true, true, true, true, true) => {
                        tri!(b j c, vertices, indices);
                    }
                    (true, true, false, true, true, true, true, true) => {
                        tri!(a k d, vertices, indices);
                    }
                    (true, true, true, false, true, true, true, true) => {
                        tri!(b d l, vertices, indices);
                    }
                    (true, true, true, true, false, true, true, true) => {
                        tri!(e i g, vertices, indices);
                    }
                    (true, true, true, true, true, false, true, true) => {
                        tri!(f g j, vertices, indices);
                    }
                    (true, true, true, true, true, true, false, true) => {
                        tri!(e h k, vertices, indices);
                    }
                    (true, true, true, true, true, true, true, false) => {
                        tri!(f l h, vertices, indices);
                    }
                    // same face corners
                    // a    b      c      d      e      f      g      h
                    (false, true, true, false, true, true, true, true) => {
                        tri!(a c i, vertices, indices);
                        tri!(b d l, vertices, indices);
                    }
                    (false, true, true, true, true, false, true, true) => {
                        tri!(a c i, vertices, indices);
                        tri!(f g j, vertices, indices);
                    }
                    (false, true, true, true, true, true, false, true) => {
                        tri!(a c i, vertices, indices);
                        tri!(e h k, vertices, indices);
                    }
                    (true, false, true, true, false, true, true, true) => {
                        tri!(b j c, vertices, indices);
                        tri!(e i g, vertices, indices);
                    }
                    (true, false, false, true, true, true, true, true) => {
                        tri!(b j c, vertices, indices);
                        tri!(a k d, vertices, indices);
                    }
                    (true, false, true, true, true, true, true, false) => {
                        tri!(b j c, vertices, indices);
                        tri!(f l h, vertices, indices);
                    }
                    (true, true, false, true, true, true, true, false) => {
                        tri!(a k d, vertices, indices);
                        tri!(f l h, vertices, indices);
                    }
                    (true, true, true, true, false, true, true, false) => {
                        tri!(e i g, vertices, indices);
                        tri!(f l h, vertices, indices);
                    }
                    (true, true, true, false, true, true, false, true) => {
                        tri!(b d l, vertices, indices);
                        tri!(e h k, vertices, indices);
                    }
                    (true, true, true, true, true, false, false, true) => {
                        tri!(f g j, vertices, indices);
                        tri!(e h k, vertices, indices);
                    }
                    (true, true, true, false, true, false, true, true) => {
                        tri!(b d l, vertices, indices);
                        tri!(f g j, vertices, indices);
                    }
                    (true, true, false, true, false, true, true, true) => {
                        tri!(a k d, vertices, indices);
                        tri!(e i g, vertices, indices);
                    }

                    // inverted face corners
                    // a    b      c      d      e      f      g      h
                    (true, false, false, true, false, false, false, false) => {
                        qua!(a i l d, vertices, indices);
                        qua!(i c b l, vertices, indices);
                    }
                    (true, false, false, false, false, true, false, false) => {
                        qua!(a f j c, vertices, indices);
                        qua!(f a i g, vertices, indices);
                    }
                    (true, false, false, false, false, false, true, false) => {
                        qua!(a k h c, vertices, indices);
                        qua!(c h e i, vertices, indices);
                    }
                    (false, true, false, false, true, false, false, false) => {
                        qua!(e b c i, vertices, indices);
                        qua!(b e g j, vertices, indices);
                    }
                    (false, true, true, false, false, false, false, false) => {
                        qua!(d k j b, vertices, indices);
                        qua!(k a c j, vertices, indices);
                    }
                    (false, true, false, false, false, false, false, true) => {
                        qua!(c h l b, vertices, indices);
                        qua!(c j f h, vertices, indices);
                    }
                    (false, false, true, false, false, false, false, true) => {
                        qua!(k a f h, vertices, indices);
                        qua!(d l a f, vertices, indices);
                    }
                    (false, false, false, false, true, false, false, true) => {
                        qua!(h l i e, vertices, indices);
                        qua!(l f g i, vertices, indices);
                    }
                    (false, false, false, true, false, false, true, false) => {
                        qua!(h e b l, vertices, indices);
                        qua!(k d b e, vertices, indices);
                    }
                    (false, false, false, false, false, true, true, false) => {
                        qua!(k j g e, vertices, indices);
                        qua!(k h f j, vertices, indices);
                    }
                    (false, false, false, true, false, true, false, false) => {
                        qua!(f l d g, vertices, indices);
                        qua!(g d b j, vertices, indices);
                    }
                    (false, false, true, false, true, false, false, false) => {
                        qua!(d g i a, vertices, indices);
                        qua!(k e g d, vertices, indices);
                    }

                    // single edges
                    // a    b      c      d      e      f      g      h
                    (true, false, true, false, false, false, false, false) => {
                        qua!(d k i c, vertices, indices);
                    }
                    (false, true, false, true, false, false, false, false) => {
                        qua!(l d c j, vertices, indices);
                    }
                    (true, true, false, false, false, false, false, false) => {
                        qua!(b a i j, vertices, indices);
                    }
                    (false, false, true, true, false, false, false, false) => {
                        qua!(l k a b, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (false, false, false, false, true, false, true, false) => {
                        qua!(k h g i, vertices, indices);
                    }
                    (false, false, false, false, false, true, false, true) => {
                        qua!(h l j g, vertices, indices);
                    }
                    (false, false, false, false, true, true, false, false) => {
                        qua!(e f j i, vertices, indices);
                    }
                    (false, false, false, false, false, false, true, true) => {
                        qua!(k l f e, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (true, false, false, false, true, false, false, false) => {
                        qua!(a e g c, vertices, indices);
                    }
                    (false, true, false, false, false, true, false, false) => {
                        qua!(f b c g, vertices, indices);
                    }
                    (false, false, true, false, false, false, true, false) => {
                        qua!(d h e a, vertices, indices);
                    }
                    (false, false, false, true, false, false, false, true) => {
                        qua!(h d b f, vertices, indices);
                    }

                    // inverted single edges
                    // a    b      c      d      e      f      g      h
                    (false, true, false, true, true, true, true, true) => {
                        qua!(k d c i, vertices, indices);
                    }
                    (true, false, true, false, true, true, true, true) => {
                        qua!(d l j c, vertices, indices);
                    }
                    (false, false, true, true, true, true, true, true) => {
                        qua!(a b j i, vertices, indices);
                    }
                    (true, true, false, false, true, true, true, true) => {
                        qua!(k l b a, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (true, true, true, true, false, true, false, true) => {
                        qua!(h k i g, vertices, indices);
                    }
                    (true, true, true, true, true, false, true, false) => {
                        qua!(l h g j, vertices, indices);
                    }
                    (true, true, true, true, false, false, true, true) => {
                        qua!(f e i j, vertices, indices);
                    }
                    (true, true, true, true, true, true, false, false) => {
                        qua!(l k e f, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (false, true, true, true, false, true, true, true) => {
                        qua!(e a c g, vertices, indices);
                    }
                    (true, false, true, true, true, false, true, true) => {
                        qua!(b f g c, vertices, indices);
                    }
                    (true, true, false, true, true, true, false, true) => {
                        qua!(h d a e, vertices, indices);
                    }
                    (true, true, true, false, true, true, true, false) => {
                        qua!(d h f b, vertices, indices);
                    }

                    // faces
                    // a    b      c      d      e      f      g      h
                    (true, true, true, true, false, false, false, false) => {
                        qua!(l k i j, vertices, indices);
                    }
                    (false, false, false, false, true, true, true, true) => {
                        qua!(k l j i, vertices, indices);
                    }
                    (true, false, true, false, true, false, true, false) => {
                        qua!(d h g c, vertices, indices);
                    }
                    (false, true, false, true, false, true, false, true) => {
                        qua!(h d c g, vertices, indices);
                    }
                    (true, true, false, false, true, true, false, false) => {
                        qua!(e f b a, vertices, indices);
                    }
                    (false, false, true, true, false, false, true, true) => {
                        qua!(f e a b, vertices, indices);
                    }

                    // double edges
                    // a    b      c      d      e      f      g      h
                    (true, false, true, false, false, true, false, true) => {
                        qua!(d k i c, vertices, indices);
                        qua!(h l j g, vertices, indices);
                    }
                    (false, true, false, true, true, false, true, false) => {
                        qua!(k h g i, vertices, indices);
                        qua!(l d c j, vertices, indices);
                    }
                    (false, true, true, false, false, true, true, false) => {
                        qua!(d h e a, vertices, indices);
                        qua!(f b c g, vertices, indices);
                    }
                    (true, false, false, true, true, false, false, true) => {
                        qua!(a e g c, vertices, indices);
                        qua!(h d b f, vertices, indices);
                    }
                    (true, true, false, false, false, false, true, true) => {
                        qua!(b a i j, vertices, indices);
                        qua!(k l f e, vertices, indices);
                    }
                    (false, false, true, true, true, true, false, false) => {
                        qua!(l k a b, vertices, indices);
                        qua!(e f j i, vertices, indices);
                    }

                    // pentagons
                    // a    b      c      d      e      f      g      h
                    (true, true, false, false, true, false, false, false) => {
                        pen!(e g j b a, vertices, indices);
                    }
                    (true, true, false, false, false, true, false, false) => {
                        pen!(a i g f b, vertices, indices);
                    }
                    (false, true, false, false, true, true, false, false) => {
                        pen!(b c i e f, vertices, indices);
                    }
                    (true, false, false, false, true, true, false, false) => {
                        pen!(f j c a e, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (false, false, true, true, false, false, true, false) => {
                        pen!(b l h e a, vertices, indices);
                    }
                    (false, false, true, true, false, false, false, true) => {
                        pen!(f h k a b, vertices, indices);
                    }
                    (false, false, false, true, false, false, true, true) => {
                        pen!(e k d b f, vertices, indices);
                    }
                    (false, false, true, false, false, false, true, true) => {
                        pen!(a d l f e, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (true, true, true, false, false, false, false, false) => {
                        pen!(j b d k i, vertices, indices);
                    }
                    (true, true, false, true, false, false, false, false) => {
                        pen!(l d a i j, vertices, indices);
                    }
                    (true, false, true, true, false, false, false, false) => {
                        pen!(i c b l k, vertices, indices);
                    }
                    (false, true, true, true, false, false, false, false) => {
                        pen!(k a c j l, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (false, false, false, false, true, true, true, false) => {
                        pen!(k h f j i, vertices, indices);
                    }
                    (false, false, false, false, true, true, false, true) => {
                        pen!(i e h l j, vertices, indices);
                    }
                    (false, false, false, false, true, false, true, true) => {
                        pen!(l f g i k, vertices, indices);
                    }
                    (false, false, false, false, false, true, true, true) => {
                        pen!(j g e k l, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (false, false, true, false, true, false, true, false) => {
                        pen!(g i a d h, vertices, indices);
                    }
                    (true, false, false, false, true, false, true, false) => {
                        pen!(c a k h g, vertices, indices);
                    }
                    (true, false, true, false, false, false, true, false) => {
                        pen!(h e i c d, vertices, indices);
                    }
                    (true, false, true, false, true, false, false, false) => {
                        pen!(d k e g c, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (false, false, false, true, false, true, false, true) => {
                        pen!(d b j g h, vertices, indices);
                    }
                    (false, true, false, false, false, true, false, true) => {
                        pen!(h l b c g, vertices, indices);
                    }
                    (false, true, false, true, false, false, false, true) => {
                        pen!(c j f h d, vertices, indices);
                    }
                    (false, true, false, true, false, true, false, false) => {
                        pen!(g f l d c, vertices, indices);
                    }

                    // inverted pentagons
                    // a    b      c      d      e      f      g      h
                    (false, false, true, true, false, true, true, true) => {
                        pen!(b j g e a, vertices, indices);
                    }
                    (false, false, true, true, true, false, true, true) => {
                        pen!(f g i a b, vertices, indices);
                    }
                    (true, false, true, true, false, false, true, true) => {
                        pen!(e i c b f, vertices, indices);
                    }
                    (false, true, true, true, false, false, true, true) => {
                        pen!(a c j f e, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (true, true, false, false, true, true, false, true) => {
                        pen!(e h l b a, vertices, indices);
                    }
                    (true, true, false, false, true, true, true, false) => {
                        pen!(a k h f b, vertices, indices);
                    }
                    (true, true, true, false, true, true, false, false) => {
                        pen!(b d k e f, vertices, indices);
                    }
                    (true, true, false, true, true, true, false, false) => {
                        pen!(f l d a e, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (false, false, false, true, true, true, true, true) => {
                        pen!(k d b j i, vertices, indices);
                    }
                    (false, false, true, false, true, true, true, true) => {
                        pen!(i a d l j, vertices, indices);
                    }
                    (false, true, false, false, true, true, true, true) => {
                        pen!(l b c i k, vertices, indices);
                    }
                    (true, false, false, false, true, true, true, true) => {
                        pen!(j c a k l, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (true, true, true, true, false, false, false, true) => {
                        pen!(j f h k i, vertices, indices);
                    }
                    (true, true, true, true, false, false, true, false) => {
                        pen!(l h e i j, vertices, indices);
                    }
                    (true, true, true, true, false, true, false, false) => {
                        pen!(i g f l k, vertices, indices);
                    }
                    (true, true, true, true, true, false, false, false) => {
                        pen!(k e g j l, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (true, true, false, true, false, true, false, true) => {
                        pen!(d a i g h, vertices, indices);
                    }
                    (false, true, true, true, false, true, false, true) => {
                        pen!(h k a c g, vertices, indices);
                    }
                    (false, true, false, true, true, true, false, true) => {
                        pen!(c i e h d, vertices, indices);
                    }
                    (false, true, false, true, false, true, true, true) => {
                        pen!(g e k d c, vertices, indices);
                    }
                    // a    b      c      d      e      f      g      h
                    (true, true, true, false, true, false, true, false) => {
                        pen!(g j b d h, vertices, indices);
                    }
                    (true, false, true, true, true, false, true, false) => {
                        pen!(c b l h g, vertices, indices);
                    }
                    (true, false, true, false, true, true, true, false) => {
                        pen!(h f j c d, vertices, indices);
                    }
                    (true, false, true, false, true, false, true, true) => {
                        pen!(d l f g c, vertices, indices);
                    }

                    // hexagons
                    // a    b      c      d      e      f      g      h
                    (false, true, true, true, false, false, false, true) => {
                        hex!(f h k a c j, vertices, indices);
                    }
                    (true, false, true, true, false, false, true, false) => {
                        hex!(b l h e i c, vertices, indices);
                    }
                    (true, true, false, true, false, true, false, false) => {
                        hex!(f l d a i g, vertices, indices);
                    }
                    (true, true, true, false, true, false, false, false) => {
                        hex!(b d k e g j, vertices, indices);
                    }
                    (false, false, false, true, false, true, true, true) => {
                        hex!(d b j g e k, vertices, indices);
                    }
                    (false, false, true, false, true, false, true, true) => {
                        hex!(a d l f g i, vertices, indices);
                    }
                    (false, true, false, false, true, true, false, true) => {
                        hex!(e h l b c i, vertices, indices);
                    }
                    (true, false, false, false, true, true, true, false) => {
                        hex!(a k h f j c, vertices, indices);
                    }

                    // awkward hexagons
                    // a    b      c      d      e      f      g      h
                    (true, true, false, true, true, false, false, false) => {
                        hex!(j l d a e g, vertices, indices);
                    }
                    (true, true, false, false, false, true, false, true) => {
                        hex!(h l b a i g, vertices, indices);
                    }
                    (true, false, true, false, true, true, false, false) => {
                        hex!(c d k e f j, vertices, indices);
                    }
                    (true, false, true, true, false, false, false, true) => {
                        hex!(b f h k i c, vertices, indices);
                    }
                    (false, true, false, true, false, false, true, true) => {
                        hex!(e k d c j f, vertices, indices);
                    }
                    (false, true, false, false, true, true, true, false) => {
                        hex!(i k h f b c, vertices, indices);
                    }
                    (false, false, true, false, false, true, true, true) => {
                        hex!(a d l j g e, vertices, indices);
                    }
                    (false, false, true, true, true, false, true, false) => {
                        hex!(a b l h g i, vertices, indices);
                    }

                    // TODO: Add opposite corners (case 4)

                    // new
                    /* (false, false, false, false, false, false, false, false) => {
                        tri!(0 0 0, vertices, indices);
                        qua!(0 0 0 0, vertices, indices);
                        hex!(0 0 0 0 0 0, vertices, indices);
                    } */
                    // debug
                    _ => {
                        println!(
                            "Missing config: {} {} {} {} {} {} {} {}",
                            if p_a { 'A' } else { ' ' },
                            if p_b { 'B' } else { ' ' },
                            if p_c { 'C' } else { ' ' },
                            if p_d { 'D' } else { ' ' },
                            if p_e { 'E' } else { ' ' },
                            if p_f { 'F' } else { ' ' },
                            if p_g { 'G' } else { ' ' },
                            if p_h { 'H' } else { ' ' },
                        );
                        missing += 1;
                    }
                }
            }
        }
    }

    println!("Missing: {}", missing);

    (vertices, indices)
}
