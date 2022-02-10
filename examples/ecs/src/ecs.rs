use crate::shader::VertexData;
use gears::glam::Vec2;
use specs::{
    prelude::ParallelIterator, Component, Join, ParJoin, ReadStorage, System, VecStorage,
    WriteStorage,
};

//

#[derive(Component)]
#[storage(VecStorage)]
pub struct QuadMesh(pub usize);

#[derive(Component)]
#[storage(VecStorage)]
pub struct Pos(pub Vec2);

#[derive(Component)]
#[storage(VecStorage)]
pub struct Vel(pub Vec2);

#[derive(Component)]
#[storage(VecStorage)]
pub struct Acc(pub Vec2);

pub struct Move;
pub struct BoundingBox;
pub struct UpdateMesh<'r>(pub f32, pub &'r mut [VertexData]);

impl<'a> System<'a> for Move {
    type SystemData = (
        ReadStorage<'a, Acc>,
        WriteStorage<'a, Vel>,
        WriteStorage<'a, Pos>,
    );

    fn run(&mut self, (acc_storage, mut vel_storage, mut pos_storage): Self::SystemData) {
        (&acc_storage, &mut vel_storage, &mut pos_storage)
            .par_join()
            .for_each(|(acc, vel, pos)| {
                pos.0 += vel.0 + 0.5 * acc.0;
                vel.0 += acc.0;
            });
    }
}

impl<'a> System<'a> for BoundingBox {
    type SystemData = (
        WriteStorage<'a, Pos>,
        WriteStorage<'a, Vel>,
        ReadStorage<'a, Acc>,
    );

    fn run(&mut self, (mut pos_storage, mut vel_storage, acc_storage): Self::SystemData) {
        (&mut pos_storage, &mut vel_storage, &acc_storage)
            .par_join()
            .for_each(|(pos, vel, acc)| {
                // i know, this is slightly over-engineered for an example
                let x = pos.0;
                let v = vel.0;
                let a = acc.0;
                let v0 = v - a;
                let x0 = x - v;
                let ground = 1.0;
                if x.y > ground {
                    // calculate the time point where this entity hit the ground
                    let time_point_of_hit_pm =
                        (2.0 * a.y * ground - 2.0 * a.y * x0.y + v0.y.powf(2.0)).sqrt();
                    let t = (-v0.y + time_point_of_hit_pm) / a.y;

                    if t.is_nan() {
                        pos.0.y = ground;
                        return;
                    }

                    // advance time till it hits the ground
                    let x = x0 + v0 * t + 0.5 * a * t.powf(2.0); // == (x.x, 0.9)
                    let v = v0 + a * t;

                    // reverse the velocity
                    let v = v * -1.0;

                    // advance time till where we started
                    let t = 1.0 - t;
                    let x = x + v * t + 0.5 * a * t.powf(2.0);
                    let v = v + a * t;

                    vel.0 = v;
                    pos.0 = x;
                }
            });
    }
}

impl<'a, 'r> System<'a> for UpdateMesh<'r> {
    type SystemData = (
        ReadStorage<'a, Pos>,
        ReadStorage<'a, Vel>,
        ReadStorage<'a, Acc>,
        ReadStorage<'a, QuadMesh>,
    );

    fn run(&mut self, (pos_storage, vel_storage, acc_storage, quad_storage): Self::SystemData) {
        let dt = self.0;
        for (pos, vel, acc, quad) in
            (&pos_storage, &vel_storage, &acc_storage, &quad_storage).join()
        {
            // x = x0 + v0 * t + 1/2 * a * t^2
            let o = pos.0 + vel.0 * dt + 0.5 * acc.0 * dt.powf(2.0);

            let new = [
                VertexData {
                    pos: (o + Vec2::new(-0.02, -0.02)).to_array(),
                },
                VertexData {
                    pos: (o + Vec2::new(0.02, -0.02)).to_array(),
                },
                VertexData {
                    pos: (o + Vec2::new(0.02, 0.02)).to_array(),
                },
                VertexData {
                    pos: (o + Vec2::new(-0.02, 0.02)).to_array(),
                },
            ];

            self.1[quad.0..quad.0 + 4].copy_from_slice(&new);
        }
    }
}
