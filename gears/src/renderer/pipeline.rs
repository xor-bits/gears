/* pub mod compute;
pub mod factory;
pub mod graphics; */

use glam::Vec4;
use std::{borrow::Cow, ffi::CStr, fmt::Display, io::Cursor, sync::Arc};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    device::Device,
    pipeline::{
        vertex::{VertexDefinition, VertexInput},
        viewport::Viewport,
        GraphicsPipeline,
    },
    render_pass::Subpass,
};

// pipeline error

fn x(device: Arc<Device>) {
    mod vertex_shader {
        vulkano_shaders::shader! {
           ty: "vertex",
           path: "../examples/gear/res/default.vert.glsl"
        }
    }

    mod fragment_shader {
        vulkano_shaders::shader! {
            ty: "fragment",
            path: "../examples/gear/res/default.frag.glsl"
        }
    }
    use vulkano::descriptor_set::PersistentDescriptorSet;
    // use ;
    use vulkano::pipeline::layout::PipelineLayout;

    let vert_shader_module = vertex_shader::Shader::load(device.clone())
        .expect("failed to create vertex shader module!");
    let frag_shader_module = fragment_shader::Shader::load(device.clone())
        .expect("failed to create fragment shader module!");

    let buffer_a = CpuAccessibleBuffer::from_data(
        device,
        BufferUsage::uniform_buffer_transfer_destination(),
        false,
        (32, 22, 11),
    )
    .unwrap();

    let x = Arc::new(
        GraphicsPipeline::start()
            .vertex_input_single_buffer()
            .vertex_shader(vert_shader_module.main_entry_point(), ())
            .fragment_shader(frag_shader_module.main_entry_point(), ())
            .viewports_dynamic_scissors_irrelevant(1)
            .build(device.clone())
            .unwrap(),
    );

    let set0 = Arc::new(
        PersistentDescriptorSet::start(x.clone())
            .add_buffer(buffer_a)
            .unwrap()
            .build()
            .unwrap(),
    );
}

/* #[derive(Debug)]
pub enum PipelineError {
    BufferError(BufferError),
    LayoutMismatch(String),
    CompileError(String),
}

impl Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::BufferError(err) => write!(f, "BufferError: {:?}", err),
            PipelineError::LayoutMismatch(err) => write!(f, "LayoutMismatch: {}", err),
            PipelineError::CompileError(err) => write!(f, "CompileError: {}", err),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct RGBAOutput {
    _color: Vec4,
}

pub struct Yes;
pub struct No;

pub trait Input: PartialEq + Default {
    type Fields;
    const BINDING_DESCRIPTION: &'static [vk::VertexInputBindingDescription];
    const ATTRIBUTE_DESCRIPTION: &'static [vk::VertexInputAttributeDescription];
}

pub trait Output: PartialEq {
    type Fields;
}

pub trait Uniform: PartialEq + Default {
    type Fields;
    type HasFields;
}

impl Input for () {
    type Fields = ();
    const BINDING_DESCRIPTION: &'static [vk::VertexInputBindingDescription] = &[];
    const ATTRIBUTE_DESCRIPTION: &'static [vk::VertexInputAttributeDescription] = &[];
}

impl Output for () {
    type Fields = ();
}

impl Uniform for () {
    type Fields = ();
    type HasFields = No;
}

impl Output for RGBAOutput {
    type Fields = (Vec4,);
}

// shader builder

pub struct Module<'a, Uf> {
    pub spirv: Cow<'a, [u8]>,
    pub uniform: Option<(Uf, u32)>,
}

impl<'a, Uf> Module<'a, Uf> {
    pub const fn none() -> Self {
        Self {
            spirv: Cow::Borrowed(&[]),
            uniform: None,
        }
    }

    pub const fn new(spirv: Cow<'a, [u8]>) -> Self {
        Self {
            spirv,
            uniform: None,
        }
    }

    pub const fn with(spirv: Cow<'a, [u8]>, initial_uniform_data: Uf, binding: u32) -> Self {
        Self {
            spirv,
            uniform: Some((initial_uniform_data, binding)),
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
 */
