use super::{point_to_index, shader, DEPTH, HEIGHT, WIDTH};
use crate::shader::VertexData;
use gears::glam::Vec3;

//

enum Lighting {
    Top,
    Bottom,
    X,
    Z,
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

fn quad(
    x: u8,
    y: u8,
    z: u8,
    sx: u8,
    sy: u8,
    sz: u8,
    inv: bool,
    light: Lighting,
    vertices: &mut Vec<VertexData>,
) {
    let (al, bl, ar, br) = if inv { (0, 1, 1, 0) } else { (1, 0, 1, 0) };

    let varying_ax = if sx == 1 { 1 } else { 0 };
    let varying_bx = if sx == 2 { 1 } else { 0 };
    let varying_ay = if sy == 1 { 1 } else { 0 };
    let varying_by = if sy == 2 { 1 } else { 0 };
    let varying_az = if sz == 1 { 1 } else { 0 };
    let varying_bz = if sz == 2 { 1 } else { 0 };

    vertices.push(VertexData {
        vi_pos: Vec3::new(
            (x + al * varying_ax + ar * varying_bx) as f32,
            (y + al * varying_ay + ar * varying_by) as f32,
            (z + al * varying_az + ar * varying_bz) as f32,
        )
        .to_array(),
        vi_exp: light.to_exposure(),
    });
    vertices.push(VertexData {
        vi_pos: Vec3::new(
            (x + al * varying_ax + br * varying_bx) as f32,
            (y + al * varying_ay + br * varying_by) as f32,
            (z + al * varying_az + br * varying_bz) as f32,
        )
        .to_array(),
        vi_exp: light.to_exposure(),
    });
    vertices.push(VertexData {
        vi_pos: Vec3::new(
            (x + bl * varying_ax + br * varying_bx) as f32,
            (y + bl * varying_ay + br * varying_by) as f32,
            (z + bl * varying_az + br * varying_bz) as f32,
        )
        .to_array(),
        vi_exp: light.to_exposure(),
    });
    vertices.push(shader::VertexData {
        vi_pos: Vec3::new(
            (x + bl * varying_ax + ar * varying_bx) as f32,
            (y + bl * varying_ay + ar * varying_by) as f32,
            (z + bl * varying_az + ar * varying_bz) as f32,
        )
        .to_array(),
        vi_exp: light.to_exposure(),
    });
}

fn cube(
    x: u8,
    y: u8,
    z: u8,
    neg_x: bool,
    pos_x: bool,
    neg_y: bool,
    pos_y: bool,
    neg_z: bool,
    pos_z: bool,
    vertices: &mut Vec<shader::VertexData>,
) {
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
}

pub fn generate_cubes(voxels: &[f32]) -> (Vec<shader::VertexData>, Vec<u32>) {
    const VERT_PER_QUAD: usize = 4;
    const INDX_PER_QUAD: usize = 6;

    // generate cubes
    let mut vertices = Vec::new();
    for voxel_z in 0..DEPTH {
        for voxel_y in 0..HEIGHT {
            for voxel_x in 0..WIDTH {
                let exists = |v: f32| v > 0.5;
                if !exists(voxels[point_to_index(voxel_x, voxel_y, voxel_z)]) {
                    continue;
                }

                let neg_x =
                    voxel_x == 0 || !exists(voxels[point_to_index(voxel_x - 1, voxel_y, voxel_z)]);
                let pos_x = voxel_x == WIDTH - 1
                    || !exists(voxels[point_to_index(voxel_x + 1, voxel_y, voxel_z)]);
                let neg_y =
                    voxel_y == 0 || !exists(voxels[point_to_index(voxel_x, voxel_y - 1, voxel_z)]);
                let pos_y = voxel_y == HEIGHT - 1
                    || !exists(voxels[point_to_index(voxel_x, voxel_y + 1, voxel_z)]);
                let neg_z =
                    voxel_z == 0 || !exists(voxels[point_to_index(voxel_x, voxel_y, voxel_z - 1)]);
                let pos_z = voxel_z == DEPTH - 1
                    || !exists(voxels[point_to_index(voxel_x, voxel_y, voxel_z + 1)]);

                cube(
                    voxel_x as u8,
                    voxel_y as u8,
                    voxel_z as u8,
                    neg_x,
                    pos_x,
                    neg_y,
                    pos_y,
                    neg_z,
                    pos_z,
                    &mut vertices,
                );
            }
        }
    }

    // generate indices
    // 0 1 2   0 2 3
    // 4 5 6   4 6 7
    // ...
    let cube_indices = [0, 1, 2, 0, 2, 3];
    let indices: Vec<u32> = (0..vertices.len() / VERT_PER_QUAD * INDX_PER_QUAD)
        .map(|i| {
            let cube_index = (i / 6) as u32;
            cube_index * 4 + cube_indices[i % 6]
        })
        .collect();

    (vertices, indices)
}
