use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod derive;
mod module;
mod pipeline;

/// ## `shader!` macro
/// WIP easier to use AIO shader macro
#[proc_macro]
pub fn shader(_input: TokenStream) -> TokenStream {
    todo!("simple macro that combines pipeline! and module!")
}

/// ## `pipeline!` macro
/// ### Example output
/// ```no_run
/// pub struct Pipeline(pub gears::renderer::pipeline::GraphicsPipeline<(), (), i32, (), ()>);
/// impl Pipeline {
/// 	pub fn build(renderer: &gears::renderer::Renderer) -> Result<Self, gears::renderer::pipeline::PipelineError> {
///			// validations filled in
///			// like: gears::static_assertions::assert_type_eq_all!()
///			Ok(Self {
///				0: gears::renderer::pipeline::factory::Pipeline::builder()
///					.vertex_uniform(VERT::load_spirv().map_err(|err| gears::renderer::pipeline::PipelineError::CompileError(err))?, 32)
///					.fragment(FRAG::load_spirv().map_err(|err| gears::renderer::pipeline::PipelineError::CompileError(err))?)
///					.input::<()>()
///					.output::<()>()
///					.build(renderer).map_err(|err| gears::renderer::pipeline::PipelineError::BufferError(err))?
///			})
/// 	}
/// }
/// // other impls like Deref
/// ```
/// ### Example input
/// ```
/// # use gears::{glam::Vec3, Input, Uniform, pipeline, module, RGBAOutput};
/// # #[derive(Default, Input)]
/// # pub struct VertexData {}
/// # #[derive(Default, Uniform)]
/// # pub struct UniformData { col: Vec3 }
/// # module! { kind = "vert", path = "../gears-pipeline/res/_test.glsl", name = "VERT", define = "ENABLE" }
/// # module! { kind = "frag", path = "../gears-pipeline/res/_include_test.glsl", name = "FRAG" }
/// pipeline! {
///		// Pipeline name:
///     "Pipeline"
///		// Pipeline input and output:
///     VertexData -> RGBAOutput
///
///		// Pipeline modules:
///     mod "VERT" as "vert" where { in UniformData }
///     mod "FRAG" as "frag"
/// }
/// ```
/// ## Usage
/// This macro takes in first a literal string as the name for this pipeline.
/// Example: `"Pipeline"`
///
/// Then (order doesn't matter, except for the name: it has to be the first)
/// it takes input for the vertex shader followed by a right arrow operator (->)
/// and after that the output of the fragment shader.
/// Example: `VertexData -> RGBAOutput`
///
/// Pipeline stages or modules are set with `mod "NAME" as "kind"` and
/// uniforms for it are set with `where { in UniformData }`.
/// "NAME" is the name of the module generated by `module!` macro and "kind" is the
/// shader module kind. The `module!` macro doc has all possible values listed.
#[proc_macro]
pub fn pipeline(input: TokenStream) -> TokenStream {
    pipeline::pipeline(input)
}

/// ## `module!` macro
/// ### Example output
/// ```no_run
/// // module name based on the macro input
/// pub mod MODULE {
///     // these values are just examples
///
///     // const fn spirv loader for compile time shaders
///     // and non-const fn spirv loader for runtime shaders
///     pub const fn load_spirv() -> Result<std::borrow::Cow<'static, [u8]>, String> { ... }
///
///     // include_str!() macro to trick cargo to recompile
///     // this source file if the shader source file got
///     // modified since the last build
///     // for runtime modules, this is the source for the
///     // runtime validator
///     pub const SOURCE: &'static str = "the source code";
///
///     // SPIRV byte-code for Vulkan (or OpenGL or whatever)
///     pub const SPIRV: &'static [u8] = &[0, 0, 0];
///
///     // type alias as tuple to tell what this shader module
///     // takes in
///     pub type INPUT = (f32,);
///
///     // same as INPUT but for OUTPUT
///     pub type OUTPUT = (f32,);
///
///     // similar to INPUT and OUTPUT but tells the uniform
///     pub type UNIFORM = ();
/// }
/// ```
/// ### Example input
/// ```
/// # use gears::{module};
/// module! {
///		// Module kind:
/// 	kind = "vert",
///		// Module source path:
///		path = "../gears-pipeline/res/_test.glsl",
///		// Module (pub mod) name:
///		name = "VERTEX_MODULE",
///		// Module compile definitions:
///		define = "ENABLE"
/// }
/// ```
/// ## Usage
/// module! macro interprets the input as attributes
///
/// possible keys:
/// ### `kind`
/// tells the shader module kind
///
/// possible values:
///  - `vert` or `vertex` for vertex shader modules
///  - `frag` or `fragment` for fragment shader modules
///  - `geom` or `geometry` for geometry shader modules
///
/// ### `path`
///	tells the file path containing the source code and
/// where to include other files from
///
/// ### `name`
/// tells the shader module name
///
/// ### `define`
/// tells the shader compiler definitions
///
/// can be used multiple times
///
/// example: `define = "CONSTANT=3.0"` or `define = "ENABLE"`
///
/// ### `runtime`
/// tells the shader module to use `path` for validation
/// and to compile spirv at runtime from a file provided
/// by the function passed to `runtime`
///
/// the function signature is as follows:
/// fn reader(name: &'static str) -> (String, Option<PathBuf>);
///
/// example: `runtime = "fn_to_read_glsl"`
#[proc_macro]
pub fn module(input: TokenStream) -> TokenStream {
    module::module(input)
}

/// ## Input derive macro
/// ## Tests
/// ### Multiple fields
/// ```
/// # use gears::{glam::Vec2, Input, FormatOf, static_assertions::assert_type_eq_all};
/// #[derive(Input)]
/// pub struct VertexData {
///     light: f32,
///     dir: Vec2,
///     active: i32,
/// }
///
/// assert_type_eq_all!(<VertexData as Input>::Fields, (f32, Vec2, i32));
/// ```
/// ### Single field
/// ```
/// # use gears::{glam::Vec3, Input, FormatOf, static_assertions::assert_type_eq_all};
/// #[derive(Input)]
/// pub struct VertexData {
///		value: Vec3,
/// }
///
/// assert_type_eq_all!(<VertexData as Input>::Fields, (Vec3,));
/// ```
/// ### No fields
/// ```
/// # use gears::{Input, FormatOf, static_assertions::assert_type_eq_all};
/// #[derive(Input)]
/// pub struct VertexData {}
///
/// assert_type_eq_all!(<VertexData as Input>::Fields, ());
/// ```
/// ### Invalid fields
/// ```compile_fail
/// # use gears::{Input, FormatOf, static_assertions::assert_type_eq_all};
/// #[derive(Input)]
/// pub struct VertexData {
///     invalid: bool,
/// }
///
/// assert_type_eq_all!(<VertexData as Input>::Fields, (bool,));
/// ```
#[proc_macro_derive(Input)]
pub fn derive_input(input: TokenStream) -> TokenStream {
    derive::impl_trait_input(parse_macro_input!(input as DeriveInput)).into()
}

/// ## Output derive macro
/// WIP
#[proc_macro_derive(Output)]
pub fn derive_output(_input: TokenStream) -> TokenStream {
    todo!()
}

/// ## Uniform derive macro
/// ```
/// # use gears::{glam::Vec2, Uniform, FormatOf, static_assertions::assert_type_eq_all};
/// #[derive(Uniform)]
/// pub struct UniformData {
///     light: f32,
///     dir: Vec2,
///     active: i32,
/// }
///
/// assert_type_eq_all!(<UniformData as Uniform>::Fields, (f32, Vec2, i32));
/// ```
/// ```
/// # use gears::{glam::Vec3, Uniform, FormatOf, static_assertions::assert_type_eq_all};
/// #[derive(Uniform)]
/// pub struct UniformData {
///		value: Vec3,
/// }
///
/// assert_type_eq_all!(<UniformData as Uniform>::Fields, (Vec3,));
/// ```
/// ```
/// # use gears::{Uniform, FormatOf, static_assertions::assert_type_eq_all};
/// #[derive(Uniform)]
/// pub struct UniformData {}
///
/// assert_type_eq_all!(<UniformData as Uniform>::Fields, ());
/// ```
#[proc_macro_derive(Uniform)]
pub fn derive_uniform(input: TokenStream) -> TokenStream {
    derive::impl_trait_uniform(parse_macro_input!(input as DeriveInput)).into()
}
