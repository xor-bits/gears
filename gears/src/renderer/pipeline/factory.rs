use crate::{
    BufferError, ComputePipeline, GraphicsPipeline, Module, PipelineBuilderBase, Renderer,
};
use ash::vk;
use std::marker::PhantomData;

// TODO: remove
pub trait UBOo {
    const STAGE: vk::ShaderStageFlags;
}

// TODO: remove
pub trait Vertexo /* <const N: usize> */ {
    // const generics not yet stable
    fn binding_desc() -> Vec<vk::VertexInputBindingDescription>;
    fn attribute_desc() -> Vec<vk::VertexInputAttributeDescription>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/* pub enum ModuleOutput {
    Float,
    Vec2,
    Vec3,
    Vec4,
} */

#[derive(Debug, Clone, Copy)]
pub struct ModuleData {
    pub source: &'static str,
    pub spirv: &'static [u8],
    pub inputs: &'static [ModuleInput],
    pub uniforms: &'static [ModuleInput],
}

#[derive(Debug, Clone, Copy)]
pub struct ShaderData {}

pub trait Input {
    type FIELDS;
    const BINDING_DESCRIPTION: &'static [vk::VertexInputBindingDescription];
    const ATTRIBUTE_DESCRIPTION: &'static [vk::VertexInputAttributeDescription];
}

/* pub trait Output {
    fn output_info() -> &'static [ModuleInput];
} */

pub trait Uniform {
    type FIELDS;
    const IS_EMPTY: bool = false;
}

impl Uniform for () {
    type FIELDS = ();
    const IS_EMPTY: bool = true;
}

// pipeline builder

pub struct VertexPipelineBuilder<'a, I: Input, Uf> {
    base: PipelineBuilderBase,
    vertex: Module<'a, Uf>,

    _p: PhantomData<I>,
}
pub struct FragmentPipelineBuilder<'a, Uf> {
    base: PipelineBuilderBase,
    fragment: Module<'a, Uf>,
}
pub struct GraphicsPipelineBuilder<'a, I: Input, UfVert, UfFrag> {
    base: PipelineBuilderBase,
    vertex: Module<'a, UfVert>,
    fragment: Module<'a, UfFrag>,

    _p: PhantomData<I>,
}
pub struct ComputePipelineBuilder<'a, Uf> {
    base: PipelineBuilderBase,
    compute: Module<'a, Uf>,
}

impl PipelineBuilderBase {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            device: renderer.rdevice.clone(),
            render_pass: renderer.data.read().swapchain_objects.read().render_pass,
            set_count: renderer.data.read().render_objects.len(),
            debug: false,
        }
    }

    pub fn vertex<'a, I: Input>(self, spirv: &[u8]) -> VertexPipelineBuilder<I, ()> {
        VertexPipelineBuilder {
            base: self,
            vertex: Module {
                spirv,
                initial_uniform_data: (),
                has_uniform: false,
            },

            _p: PhantomData {},
        }
    }

    pub fn vertex_uniform<'a, I: Input, U: Uniform>(
        self,
        spirv: &'a [u8],
        initial_uniform_data: U,
    ) -> VertexPipelineBuilder<'a, I, U> {
        VertexPipelineBuilder {
            base: self,
            vertex: Module {
                spirv,
                initial_uniform_data,
                has_uniform: true,
            },

            _p: PhantomData {},
        }
    }

    pub fn fragment(self, spirv: &[u8]) -> FragmentPipelineBuilder<()> {
        FragmentPipelineBuilder {
            base: self,
            fragment: Module {
                spirv,
                initial_uniform_data: (),
                has_uniform: false,
            },
        }
    }

    pub fn fragment_uniform<'a, U: Uniform>(
        self,
        spirv: &'a [u8],
        initial_uniform_data: U,
    ) -> FragmentPipelineBuilder<'a, U> {
        FragmentPipelineBuilder {
            base: self,
            fragment: Module {
                spirv,
                initial_uniform_data,
                has_uniform: true,
            },
        }
    }

    pub fn compute(self, spirv: &[u8]) -> ComputePipelineBuilder<()> {
        ComputePipelineBuilder {
            base: self,
            compute: Module {
                spirv,
                initial_uniform_data: (),
                has_uniform: false,
            },
        }
    }

    pub fn compute_uniform<'a, U: Uniform>(
        self,
        spirv: &'a [u8],
        initial_uniform_data: U,
    ) -> ComputePipelineBuilder<'a, U> {
        ComputePipelineBuilder {
            base: self,
            compute: Module {
                spirv,
                initial_uniform_data,
                has_uniform: true,
            },
        }
    }
}

impl<'a, I: Input, Uf> VertexPipelineBuilder<'a, I, Uf> {
    pub fn fragment(self, spirv: &'a [u8]) -> GraphicsPipelineBuilder<I, Uf, ()> {
        GraphicsPipelineBuilder {
            base: self.base,
            vertex: self.vertex,
            fragment: Module {
                spirv,
                initial_uniform_data: (),
                has_uniform: false,
            },

            _p: PhantomData {},
        }
    }

    pub fn fragment_uniform<U: Uniform>(
        self,
        spirv: &'a [u8],
        initial_uniform_data: U,
    ) -> GraphicsPipelineBuilder<'a, I, Uf, U> {
        GraphicsPipelineBuilder {
            base: self.base,
            vertex: self.vertex,
            fragment: Module {
                spirv,
                initial_uniform_data,
                has_uniform: true,
            },

            _p: PhantomData {},
        }
    }
}

impl<'a, Uf> FragmentPipelineBuilder<'a, Uf> {
    pub fn vertex<I: Input>(self, spirv: &'a [u8]) -> GraphicsPipelineBuilder<I, (), Uf> {
        GraphicsPipelineBuilder {
            base: self.base,
            fragment: self.fragment,
            vertex: Module {
                spirv,
                initial_uniform_data: (),
                has_uniform: false,
            },

            _p: PhantomData {},
        }
    }

    pub fn vertex_uniform<I: Input, U: Uniform>(
        self,
        spirv: &'a [u8],
        initial_uniform_data: U,
    ) -> GraphicsPipelineBuilder<'a, I, U, Uf> {
        GraphicsPipelineBuilder {
            base: self.base,
            fragment: self.fragment,
            vertex: Module {
                spirv,
                initial_uniform_data,
                has_uniform: true,
            },

            _p: PhantomData {},
        }
    }
}

impl<'a, I: Input, UfVert: Uniform, UfFrag: Uniform>
    GraphicsPipelineBuilder<'a, I, UfVert, UfFrag>
{
    pub fn build(self) -> Result<GraphicsPipeline<I, UfVert, UfFrag>, BufferError> {
        GraphicsPipeline::new(
            self.base.device,
            self.base.render_pass,
            self.base.set_count,
            self.vertex,
            self.fragment,
            self.base.debug,
        )
    }
}

impl<'a, Uf> ComputePipelineBuilder<'a, Uf> {
    pub fn build(self) -> ComputePipeline<Uf> {
        ComputePipeline::new(
            self.base.device,
            self.base.render_pass,
            self.compute,
            self.base.debug,
        )
    }
}
