use crate::{BufferError, GraphicsPipeline, Input, Module, Output, Renderer, Uniform};
use std::{borrow::Cow, marker::PhantomData};

// pipeline

pub struct Pipeline {}

impl Pipeline {
    pub const fn builder() -> PipelineBuilder {
        PipelineBuilder {}
    }
}

// pipeline builder

#[must_use]
pub struct PipelineBuilder {}

impl PipelineBuilder {
    fn graphics_builder<'a>(self) -> GPipelineBuilder<'a, (), (), (), (), (), false, false, false> {
        GPipelineBuilder::<'a, (), (), (), (), (), false, false, false>::new(self)
    }
}

// graphics pipeline builder

#[must_use]
pub struct GPipelineBuilder<
    'a,
    In,
    Out,
    UfVert,
    UfGeom,
    UfFrag,
    const VERT: bool,
    const GEOM: bool,
    const FRAG: bool,
> where
    In: Input,
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    vert: Module<'a, UfVert>,
    geom: Option<Module<'a, UfGeom>>,
    frag: Module<'a, UfFrag>,

    base: PipelineBuilder,

    _p0: PhantomData<In>,
    _p1: PhantomData<Out>,
}

impl<'a, In, Out, UfVert, UfGeom, UfFrag, const VERT: bool, const GEOM: bool, const FRAG: bool>
    GPipelineBuilder<'a, In, Out, UfVert, UfGeom, UfFrag, VERT, GEOM, FRAG>
where
    In: Input,
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    pub fn new(
        base: PipelineBuilder,
    ) -> GPipelineBuilder<'a, (), (), (), (), (), false, false, false> {
        GPipelineBuilder {
            vert: Module::none(),
            geom: None,
            frag: Module::none(),

            base,

            _p0: PhantomData {},
            _p1: PhantomData {},
        }
    }
}

impl<'a, In, Out, UfVert, UfGeom, UfFrag, const GEOM: bool>
    GPipelineBuilder<'a, In, Out, UfVert, UfGeom, UfFrag, true, GEOM, true>
where
    In: Input,
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    pub fn build(
        self,
        renderer: &Renderer,
    ) -> Result<GraphicsPipeline<In, Out, UfVert, UfGeom, UfFrag>, BufferError> {
        GraphicsPipeline::new(
            renderer.device.clone(),
            renderer.render_pass(),
            renderer.parallel_object_count(),
            self.vert,
            self.geom,
            self.frag,
            false,
        )
    }
}

// graphics pipeline io

impl<'a, Out, UfVert, UfGeom, UfFrag, const VERT: bool, const GEOM: bool, const FRAG: bool>
    GPipelineBuilder<'a, (), Out, UfVert, UfGeom, UfFrag, VERT, GEOM, FRAG>
where
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    pub fn input<In>(
        self,
    ) -> GPipelineBuilder<'a, In, Out, UfVert, UfGeom, UfFrag, VERT, GEOM, FRAG>
    where
        In: Input,
    {
        GPipelineBuilder {
            vert: self.vert,
            geom: self.geom,
            frag: self.frag,

            base: self.base,

            _p0: PhantomData {},
            _p1: self._p1,
        }
    }
}

impl<'a, In, UfVert, UfGeom, UfFrag, const VERT: bool, const GEOM: bool, const FRAG: bool>
    GPipelineBuilder<'a, In, (), UfVert, UfGeom, UfFrag, VERT, GEOM, FRAG>
where
    In: Input,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    pub fn output<Out>(
        self,
    ) -> GPipelineBuilder<'a, In, Out, UfVert, UfGeom, UfFrag, VERT, GEOM, FRAG>
    where
        Out: Output,
    {
        GPipelineBuilder {
            vert: self.vert,
            geom: self.geom,
            frag: self.frag,

            base: self.base,

            _p0: self._p0,
            _p1: PhantomData {},
        }
    }
}

// graphics pipeline vertex

impl<'a, In, Out, UfVert, UfGeom, UfFrag, const GEOM: bool, const FRAG: bool>
    GPipelineBuilder<'a, In, Out, UfVert, UfGeom, UfFrag, false, GEOM, FRAG>
where
    In: Input,
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    pub fn vertex(
        self,
        spirv: Cow<'a, [u8]>,
    ) -> GPipelineBuilder<'a, In, Out, UfVert, UfGeom, UfFrag, true, GEOM, FRAG> {
        GPipelineBuilder {
            vert: Module::new(spirv),
            geom: self.geom,
            frag: self.frag,

            base: self.base,

            _p0: self._p0,
            _p1: self._p1,
        }
    }

    pub fn vertex_uniform<NewUfVert>(
        self,
        spirv: Cow<'a, [u8]>,
        initial_uniform_data: NewUfVert,
        binding: u32,
    ) -> GPipelineBuilder<'a, In, Out, NewUfVert, UfGeom, UfFrag, true, GEOM, FRAG>
    where
        NewUfVert: Uniform,
    {
        GPipelineBuilder {
            vert: Module::with(spirv, initial_uniform_data, binding),
            geom: self.geom,
            frag: self.frag,

            base: self.base,

            _p0: self._p0,
            _p1: self._p1,
        }
    }
}

impl PipelineBuilder {
    pub fn vertex<'a>(
        self,
        spirv: Cow<'a, [u8]>,
    ) -> GPipelineBuilder<'a, (), (), (), (), (), true, false, false> {
        self.graphics_builder().vertex(spirv)
    }

    pub fn vertex_uniform<'a, UfVert>(
        self,
        spirv: Cow<'a, [u8]>,
        initial_uniform_data: UfVert,
        binding: u32,
    ) -> GPipelineBuilder<'a, (), (), UfVert, (), (), true, false, false>
    where
        UfVert: Uniform,
    {
        self.graphics_builder()
            .vertex_uniform(spirv, initial_uniform_data, binding)
    }
}

// graphics pipeline fragment

impl<'a, In, Out, UfVert, UfGeom, UfFrag, const VERT: bool, const GEOM: bool>
    GPipelineBuilder<'a, In, Out, UfVert, UfGeom, UfFrag, VERT, GEOM, false>
where
    In: Input,
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    pub fn fragment(
        self,
        spirv: Cow<'a, [u8]>,
    ) -> GPipelineBuilder<'a, In, Out, UfVert, UfGeom, UfFrag, VERT, GEOM, true> {
        GPipelineBuilder {
            vert: self.vert,
            geom: self.geom,
            frag: Module::new(spirv),

            base: self.base,

            _p0: self._p0,
            _p1: self._p1,
        }
    }

    pub fn fragment_uniform<NewUfFrag>(
        self,
        spirv: Cow<'a, [u8]>,
        initial_uniform_data: NewUfFrag,
        binding: u32,
    ) -> GPipelineBuilder<'a, In, Out, UfVert, UfGeom, NewUfFrag, VERT, GEOM, true>
    where
        NewUfFrag: Uniform,
    {
        GPipelineBuilder {
            vert: self.vert,
            geom: self.geom,
            frag: Module::with(spirv, initial_uniform_data, binding),

            base: self.base,

            _p0: self._p0,
            _p1: self._p1,
        }
    }
}

impl PipelineBuilder {
    pub fn fragment<'a>(
        self,
        spirv: Cow<'a, [u8]>,
    ) -> GPipelineBuilder<'a, (), (), (), (), (), false, false, true> {
        self.graphics_builder().fragment(spirv)
    }

    pub fn fragment_uniform<'a, UfFrag>(
        self,
        spirv: Cow<'a, [u8]>,
        initial_uniform_data: UfFrag,
        binding: u32,
    ) -> GPipelineBuilder<'a, (), (), (), (), UfFrag, false, false, true>
    where
        UfFrag: Uniform,
    {
        self.graphics_builder()
            .fragment_uniform(spirv, initial_uniform_data, binding)
    }
}

// graphics pipeline geometry

impl<'a, In, Out, UfVert, UfGeom, UfFrag, const VERT: bool, const FRAG: bool>
    GPipelineBuilder<'a, In, Out, UfVert, UfGeom, UfFrag, VERT, false, FRAG>
where
    In: Input,
    Out: Output,
    UfVert: Uniform,
    UfGeom: Uniform,
    UfFrag: Uniform,
{
    pub fn geometry(
        self,
        spirv: Cow<'a, [u8]>,
    ) -> GPipelineBuilder<'a, In, Out, UfVert, UfGeom, UfFrag, VERT, true, FRAG> {
        GPipelineBuilder {
            vert: self.vert,
            geom: Some(Module::new(spirv)),
            frag: self.frag,

            base: self.base,

            _p0: self._p0,
            _p1: self._p1,
        }
    }

    pub fn geometry_uniform<NewUfGeom>(
        self,
        spirv: Cow<'a, [u8]>,
        initial_uniform_data: NewUfGeom,
        binding: u32,
    ) -> GPipelineBuilder<'a, In, Out, UfVert, NewUfGeom, UfFrag, VERT, true, FRAG>
    where
        NewUfGeom: Uniform,
    {
        GPipelineBuilder {
            vert: self.vert,
            geom: Some(Module::with(spirv, initial_uniform_data, binding)),
            frag: self.frag,

            base: self.base,

            _p0: self._p0,
            _p1: self._p1,
        }
    }
}

impl PipelineBuilder {
    pub fn geometry<'a>(
        self,
        spirv: Cow<'a, [u8]>,
    ) -> GPipelineBuilder<'a, (), (), (), (), (), false, true, false> {
        self.graphics_builder().geometry(spirv)
    }

    pub fn geometry_uniform<'a, UfGeom>(
        self,
        spirv: Cow<'a, [u8]>,
        initial_uniform_data: UfGeom,
        binding: u32,
    ) -> GPipelineBuilder<'a, (), (), (), UfGeom, (), false, true, false>
    where
        UfGeom: Uniform,
    {
        self.graphics_builder()
            .geometry_uniform(spirv, initial_uniform_data, binding)
    }
}

/* pub struct GraphicsPipelineOptionals<'a, UfGeom>
where
    UfGeom: Default,
{
    geometry: Option<Module<'a, UfGeom>>,
}

pub struct VertexPipelineBuilder<'a, I, UfVert, UfGeom>
where
    I: Input,
    UfVert: Default,
    UfGeom: Default,
{
    base: PipelineBuilderBase,
    vertex: Module<'a, UfVert>,

    optionals: GraphicsPipelineOptionals<'a, UfGeom>,

    _p: PhantomData<I>,
}
pub struct FragmentPipelineBuilder<'a, UfGeom, UfFrag>
where
    UfGeom: Default,
    UfFrag: Default,
{
    base: PipelineBuilderBase,
    fragment: Module<'a, UfFrag>,

    optionals: GraphicsPipelineOptionals<'a, UfGeom>,
}
pub struct GraphicsPipelineBuilder<'a, I, UfVert, UfGeom, UfFrag>
where
    I: Input,
    UfVert: Default,
    UfGeom: Default,
    UfFrag: Default,
{
    base: PipelineBuilderBase,
    vertex: Module<'a, UfVert>,
    fragment: Module<'a, UfFrag>,

    optionals: GraphicsPipelineOptionals<'a, UfGeom>,

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

    pub fn vertex<'a, I: Input, UfGeom>(
        self,
        spirv: &[u8],
    ) -> VertexPipelineBuilder<I, (), UfGeom> {
        VertexPipelineBuilder {
            base: self,
            vertex: Module {
                spirv,
                initial_uniform_data: (),
                has_uniform: false,
            },

            optionals: GraphicsPipelineOptionals { geometry: None },

            _p: PhantomData {},
        }
    }

    pub fn vertex_uniform<'a, I: Input, UfVert: Uniform, UfGeom>(
        self,
        spirv: &'a [u8],
        initial_uniform_data: UfVert,
    ) -> VertexPipelineBuilder<'a, I, UfVert, UfGeom> {
        VertexPipelineBuilder {
            base: self,
            vertex: Module {
                spirv,
                initial_uniform_data,
                has_uniform: true,
            },

            optionals: GraphicsPipelineOptionals { geometry: None },

            _p: PhantomData {},
        }
    }

    pub fn fragment<UfGeom>(self, spirv: &[u8]) -> FragmentPipelineBuilder<UfGeom, ()> {
        FragmentPipelineBuilder {
            base: self,
            fragment: Module {
                spirv,
                initial_uniform_data: (),
                has_uniform: false,
            },

            optionals: GraphicsPipelineOptionals { geometry: None },
        }
    }

    pub fn fragment_uniform<'a, UfGeom, UfFrag: Uniform>(
        self,
        spirv: &'a [u8],
        initial_uniform_data: UfFrag,
    ) -> FragmentPipelineBuilder<'a, UfGeom, UfFrag> {
        FragmentPipelineBuilder {
            base: self,
            fragment: Module {
                spirv,
                initial_uniform_data,
                has_uniform: true,
            },

            optionals: GraphicsPipelineOptionals { geometry: None },
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

impl<'a, I: Input, UfVert, UfGeom> VertexPipelineBuilder<'a, I, UfVert, UfGeom> {
    pub fn fragment(self, spirv: &'a [u8]) -> GraphicsPipelineBuilder<I, UfVert, UfGeom, ()> {
        GraphicsPipelineBuilder {
            base: self.base,
            vertex: self.vertex,
            fragment: Module {
                spirv,
                initial_uniform_data: (),
                has_uniform: false,
            },

            optionals: self.optionals,

            _p: PhantomData {},
        }
    }

    pub fn fragment_uniform<UfFrag: Uniform>(
        self,
        spirv: &'a [u8],
        initial_uniform_data: UfFrag,
    ) -> GraphicsPipelineBuilder<'a, I, UfVert, UfGeom, UfFrag> {
        GraphicsPipelineBuilder {
            base: self.base,
            vertex: self.vertex,
            fragment: Module {
                spirv,
                initial_uniform_data,
                has_uniform: true,
            },

            optionals: self.optionals,

            _p: PhantomData {},
        }
    }

    pub fn geometry(self, spirv: &'a [u8]) -> Self {
        self.optionals.geometry = Some(Module {
            spirv,
            initial_uniform_data: (),
            has_uniform: false,
        });
        self
    }
}

impl<'a, UfGeom, UfFrag> FragmentPipelineBuilder<'a, UfGeom, UfFrag> {
    pub fn vertex<I: Input>(
        self,
        spirv: &'a [u8],
    ) -> GraphicsPipelineBuilder<I, (), UfGeom, UfFrag> {
        GraphicsPipelineBuilder {
            base: self.base,
            fragment: self.fragment,
            vertex: Module {
                spirv,
                initial_uniform_data: (),
                has_uniform: false,
            },

            optionals: self.optionals,

            _p: PhantomData {},
        }
    }

    pub fn vertex_uniform<I: Input, UfVert: Uniform>(
        self,
        spirv: &'a [u8],
        initial_uniform_data: UfVert,
    ) -> GraphicsPipelineBuilder<'a, I, UfVert, UfGeom, UfFrag> {
        GraphicsPipelineBuilder {
            base: self.base,
            fragment: self.fragment,
            vertex: Module {
                spirv,
                initial_uniform_data,
                has_uniform: true,
            },

            optionals: self.optionals,

            _p: PhantomData {},
        }
    }
}

impl<'a, I: Input, UfVert, UfGeom, UfFrag> GraphicsPipelineBuilder<'a, I, UfVert, UfGeom, UfFrag> {
    pub fn build(self) -> Result<GraphicsPipeline<I, UfVert, UfGeom, UfFrag>, BufferError> {
        GraphicsPipeline::new(
            self.base.device,
            self.base.render_pass,
            self.base.set_count,
            self.vertex,
            self.optionals.geometry,
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
 */
