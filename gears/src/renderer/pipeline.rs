pub mod compute;
pub mod factory;
pub mod graphics;

use std::{ffi::CStr, io::Cursor};

#[cfg(feature = "short_namespaces")]
pub use compute::*;
#[cfg(feature = "short_namespaces")]
pub use factory::*;
use glam::Vec4;
#[cfg(feature = "short_namespaces")]
pub use graphics::*;

use super::device::Dev;
use ash::{util::read_spv, version::DeviceV1_0, vk};

// todo: runtime shader

/* #[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleInput {
    Float,
    Vec2,
    Vec3,
    Vec4,

    Mat2,
    Mat3,
    Mat4,

    Int,
    UInt,
}

pub enum ModuleOutput {
    Float,
    Vec2,
    Vec3,
    Vec4,
}

#[derive(Debug, Clone, Copy)]
pub struct ModuleData {
    pub source: &'static str,
    pub spirv: &'static [u8],
    pub inputs: &'static [ModuleInput],
    pub uniforms: &'static [ModuleInput],
}

#[derive(Debug, Clone, Copy)]
pub struct ShaderData {} */

// compile time shaders

pub struct RGBAOutput {
    _color: Vec4,
}

pub trait Input {
    type FIELDS;
    const BINDING_DESCRIPTION: &'static [vk::VertexInputBindingDescription];
    const ATTRIBUTE_DESCRIPTION: &'static [vk::VertexInputAttributeDescription];
}

pub trait Output {
    type FIELDS;
}

pub trait Uniform {
    type FIELDS;
    const IS_EMPTY: bool = false;
}

impl Input for () {
    type FIELDS = ();
    const BINDING_DESCRIPTION: &'static [vk::VertexInputBindingDescription] = &[];
    const ATTRIBUTE_DESCRIPTION: &'static [vk::VertexInputAttributeDescription] = &[];
}

impl Output for () {
    type FIELDS = ();
}

impl Uniform for () {
    type FIELDS = ();
    const IS_EMPTY: bool = true;
}

impl Output for RGBAOutput {
    type FIELDS = (Vec4,);
}

// shader builder

pub struct Module<'a, Uf> {
    pub spirv: &'a [u8],
    pub uniform: Option<Uf>,
}

impl<'a, Uf> Module<'a, Uf> {
    pub const fn none() -> Self {
        Self {
            spirv: &[],
            uniform: None,
        }
    }

    pub const fn new(spirv: &'a [u8]) -> Self {
        Self {
            spirv,
            uniform: None,
        }
    }

    pub const fn with(spirv: &'a [u8], initial_uniform_data: Uf) -> Self {
        Self {
            spirv,
            uniform: Some(initial_uniform_data),
        }
    }
}

fn shader_module(
    device: &Dev,
    spirv: &[u8],
    stage: vk::ShaderStageFlags,
) -> (vk::ShaderModule, vk::PipelineShaderStageCreateInfo) {
    let spirv = read_spv(&mut Cursor::new(&spirv[..])).expect("SPIR-V read failed");

    let module_info = vk::ShaderModuleCreateInfo::builder().code(&spirv[..]);

    let module = unsafe { device.create_shader_module(&module_info, None) }
        .expect("Vertex shader module creation failed");

    let stage = vk::PipelineShaderStageCreateInfo::builder()
        .module(module)
        .stage(stage)
        .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
        .build();

    (module, stage)
}
