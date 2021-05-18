use proc_macro::TokenStream;

use pipeline::{Pipeline, PipelineInput};
use quote::ToTokens;
use syn::parse_macro_input;

mod compiler;
mod module;
mod pipeline;
mod ubo;

/// # gears-pipeline main macro
///
/// ## usage
/// ### modules
/// First token is the module id.
/// It defines the shader module type.
/// - ```vertex: { /* module options */ }``` (with aliases ```vs``` and ```v```)
/// - ```fragment: { /* module options */ }``` (with aliases ```fs``` and ```f```)
/// ### module options
/// #### ```source: "..."```
/// Has aliases: ```src``` and ```s```
/// Raw text form GLSL source to be compiled.
/// Only one ```source``` or ```path``` can be given.
/// #### ```path: "..."```
/// Has alias: ```p```
/// Path to GLSL source to be compiled.
/// Fills ```include``` if not already given.
/// Only one ```source``` or ```path``` can be given.
/// #### ```include: "..."```
/// Has aliases: ```inc``` and ```i```
/// Path to be used with #include.
/// Overwrites ```include``` if already given.
/// #### ```define: ["NAME1" = "VALUE", "NAME2"]```
/// Has aliases: ```def``` and ```d```
/// Adds a list of macros.
/// #### ```no-autodefine```
/// Has aliases: ```na``` and ```n```
/// Disables gears-pipeline defines.
/// #### ```entry: "..."```
/// Has aliases: ```ep``` and ```e```
/// Specifies the entry point name.
/// #### ```debug```
/// Dumps glsl as a compile error
///
/// ## gears-pipeline defines
///
/// ### for vertex shaders:
///  - ```#define GEARS_VERTEX```
///  - ```#define GEARS_IN(_location, _data) layout(location = _location) in _data;```
///  - ```#define GEARS_INOUT(_location, _data) layout(location = _location) out _data;```
///  - ```#define GEARS_OUT(_location, _data) _data;```
///
/// ### for vertex shaders:
///  - ```#define GEARS_FRAGMENT```
///  - ```#define GEARS_IN(_location, _data) _data;```
///  - ```#define GEARS_INOUT(_location, _data) layout(location = _location) in _data;```
///  - ```#define GEARS_OUT(_location, _data) layout(location = _location) out _data;```
///
/// ## gears-pipeline default entry points
/// - vertex shader: ```vert```
/// - fragment shader: ```frag```
///
/// ### rust like attribute macros:
/// ```#[gears_bindgen]```
/// This expands a struct or uniform in the glsl source and generates rust bindings for it.
/// Arguments for it can be given after 'gears_bindgen' in parentheses.
/// Possible arguments:
///  - shader input: ```in```
///  - shader output: ```out```
///  - uniforms: ```unifom(binding = 0)``` (the binding can be any integer)
///
/// ```#[gears_gen]```
/// This is the same as ```#[gears_bindgen]``` but will not generate the rust bindings.
///
/// ### example
/// ```
/// mod pl {
///     gears_pipeline::pipeline! {
///         vs: {
///             path: "tests/test.glsl"
///             def: [ "FRAGMENT", "VALUE" = "2" ]
///         }
///         fs: {
///             source: "#version 440\n#include \"include.glsl\""
///             include: "tests/"
///         }
///     }
/// }
///
/// // check SPIRV generation
/// assert_eq!(1248, pl::VERT_SPIRV.len(), "Vert spirv not what expected");
/// assert_eq!(252, pl::FRAG_SPIRV.len(), "Frag spirv not what expected");
///
/// // check UBO struct generation
/// pl::UBO { time: 0f32 };
/// ```
#[proc_macro]
pub fn pipeline(input: TokenStream) -> TokenStream {
    match Pipeline::new(parse_macro_input!(input as PipelineInput)) {
        Err(err) => err.to_compile_error().into(),
        Ok(pipeline) => pipeline.to_token_stream().into(),
    }
}
