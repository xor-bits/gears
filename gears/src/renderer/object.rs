use cgmath::{InnerSpace, Vector3};
use wavefront_obj::obj::Primitive;

pub fn load_obj<V>(
    obj_data: &str,
    _: Option<&str>,
    construct_vertex: fn(position: Vector3<f32>, normal: Vector3<f32>) -> V,
) -> Vec<V> {
    let objset = wavefront_obj::obj::parse(obj_data).unwrap();
    // TODO: let mtlset = wavefront_obj::mtl::parse(mtl_data).unwrap();
    let obj = &objset.objects[0];
    let i_count = obj
        .geometry
        .iter()
        .map(|g| {
            g.shapes
                .iter()
                .map(|s| match &s.primitive {
                    Primitive::Triangle(_, _, _) => 3,
                    _ => panic!("Only triangles"),
                })
                .sum::<usize>()
        })
        .sum::<usize>();

    // fill vertex&index buffer
    let mut vertices = Vec::<V>::with_capacity(i_count);
    for g in obj.geometry.iter() {
        for s in g.shapes.iter() {
            match s.primitive {
                Primitive::Triangle(
                    (a_vert_id, _, a_norm_id),
                    (b_vert_id, _, b_norm_id),
                    (c_vert_id, _, c_norm_id),
                ) => {
                    let id_to_vertex = |vert: usize, norm: Option<usize>| -> V {
                        let vert = obj.vertices[vert];

                        let norm = if let Some(norm_id) = norm {
                            Vector3::new(
                                obj.normals[norm_id].x,
                                obj.normals[norm_id].y,
                                obj.normals[norm_id].z,
                            )
                            .cast::<f32>()
                            .unwrap()
                        } else {
                            let ab = Vector3::new(
                                obj.vertices[b_vert_id].x - obj.vertices[a_vert_id].x,
                                obj.vertices[b_vert_id].y - obj.vertices[a_vert_id].y,
                                obj.vertices[b_vert_id].z - obj.vertices[a_vert_id].z,
                            );

                            let ac = Vector3::new(
                                obj.vertices[c_vert_id].x - obj.vertices[a_vert_id].x,
                                obj.vertices[c_vert_id].y - obj.vertices[a_vert_id].y,
                                obj.vertices[c_vert_id].z - obj.vertices[a_vert_id].z,
                            );

                            ab.normalize().cross(ac.normalize()).cast::<f32>().unwrap()
                        };

                        construct_vertex(
                            Vector3::new(vert.x as f32, vert.y as f32, vert.z as f32),
                            Vector3::new(norm.x as f32, norm.y as f32, norm.z as f32),
                        )
                    };

                    vertices.push(id_to_vertex(a_vert_id, a_norm_id));
                    vertices.push(id_to_vertex(b_vert_id, b_norm_id));
                    vertices.push(id_to_vertex(c_vert_id, c_norm_id));
                }
                _ => panic!("Only triangles"),
            }
        }
    }

    vertices
}
