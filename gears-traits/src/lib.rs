pub use cgmath::{Vector2, Vector3};
pub use gfx_hal::{
    format::Format,
    pso::Element,
    pso::{AttributeDesc, ShaderStageFlags, VertexBufferDesc, VertexInputRate},
};

pub trait UBO {
    const STAGE: ShaderStageFlags;
}

pub trait Vertex /* <const N: usize> */ {
    // const generics not yet stable
    fn binding_desc() -> Vec<VertexBufferDesc>;
    fn attribute_desc() -> Vec<AttributeDesc>;
}

pub struct VertexData {
    pub position: Vector2<f32>,
    pub color: Vector3<f32>,
}

impl Vertex for VertexData {
    fn binding_desc() -> Vec<VertexBufferDesc> {
        vec![VertexBufferDesc {
            binding: 0,
            rate: VertexInputRate::Vertex,
            stride: std::mem::size_of::<VertexData>() as u32,
        }]
    }

    fn attribute_desc() -> Vec<AttributeDesc> {
        vec![
            AttributeDesc {
                binding: 0,
                location: 0,
                element: Element {
                    format: Format::Rg32Sfloat,
                    offset: 0,
                },
            },
            AttributeDesc {
                binding: 0,
                location: 1,
                element: Element {
                    format: Format::Rgb32Sfloat,
                    offset: 4 * 2,
                },
            },
        ]
    }
}
