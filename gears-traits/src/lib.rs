pub use ash::vk;
pub use cgmath::{Matrix2, Matrix3, Matrix4, Vector2, Vector3, Vector4};

pub trait UBO {
    const STAGE: vk::ShaderStageFlags;
}

pub trait Vertex /* <const N: usize> */ {
    // const generics not yet stable
    fn binding_desc() -> Vec<vk::VertexInputBindingDescription>;
    fn attribute_desc() -> Vec<vk::VertexInputAttributeDescription>;
}
