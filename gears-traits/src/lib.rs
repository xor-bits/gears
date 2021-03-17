pub use cgmath::{Matrix2, Matrix3, Matrix4, Vector2, Vector3, Vector4};
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
