use proc_macro::TokenStream;
use proc_macro2::{Group, Punct, Span};
use quote::quote;
use shaderc::CompilationArtifact;
use std::{env, fs::File, io::Read, path::Path};
use syn::{parse::ParseStream, parse_macro_input, Error, Ident, LitStr, Token};

// input

struct DefinesInput {
    defines: Vec<(String, Option<String>)>,
}

struct ModuleInput {
    source: String,
    include_path: Option<String>,
    defines: DefinesInput,
    default_defines: bool,
    entry: Option<String>,
    debug: bool,
    span: Span,
}

struct GraphicsPipelineInput {
    // name: String,
    vert_source: ModuleInput,
    frag_source: ModuleInput,
    // geom_source: Option<ModuleInput>,
    /* ... */
}

struct ComputePipelineInput {
    // name: String,
// comp_source: ModuleInput,
}

enum PipelineInput {
    Graphics(GraphicsPipelineInput),
    Compute(ComputePipelineInput),
}

// processed

struct CompiledModule {
    spirv: CompilationArtifact,
}

struct GraphicsPipeline {
    // name: String,
    vert: CompiledModule,
    frag: CompiledModule,
    // geom: Option<CompiledModule>,
    /* ... */
}

struct ComputePipeline {
    // name: String,
// comp: CompiledModule,
}

enum Pipeline {
    Graphics(GraphicsPipeline),
    Compute(ComputePipeline),
}

// imp input

impl std::ops::AddAssign for DefinesInput {
    fn add_assign(&mut self, rhs: Self) {
        let mut defines = rhs.defines;
        self.defines.append(&mut defines);
    }
}

impl syn::parse::Parse for DefinesInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut defines = Vec::new();

        while !input.is_empty() {
            let name: LitStr = input.parse()?;

            if input.is_empty() {
                defines.push((name.value(), None));
                break;
            }

            let punct: Punct = input.parse()?;
            match punct.as_char() {
                '=' => {
                    let value: LitStr = input.parse()?;
                    defines.push((name.value(), Some(value.value())));

                    if input.is_empty() {
                        break;
                    }

                    input.parse::<Token![,]>()?;
                }
                ',' => {
                    continue;
                }
                _ => {
                    return Err(Error::new(
                        punct.span(),
                        "Invalid punctuation, only '=' and ',' are valid",
                    ))
                }
            }
        }

        Ok(Self { defines })
    }
}

impl parse_macro_input::ParseMacroInput for PipelineInput
// impl syn::parse::Parse for PipelineInput
{
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut vert_source = None;
        let mut frag_source = None;

        while !input.is_empty() {
            let shader: Ident = input.parse()?;
            let shader_type_string = shader.to_string();

            input.parse::<Token![:]>()?;

            let module = match shader_type_string.as_str() {
                "vs" | "vertex" | "vert" => &mut vert_source,
                "fs" | "fragment" | "frag" => &mut frag_source,
                _ => {
                    return Err(Error::new(
                        shader.span(),
                        format!("Unknown shader type: {}", shader_type_string),
                    ));
                }
            };

            let group: Group = input.parse()?;
            let group_tokens: TokenStream = group.stream().into();
            module.replace(syn::parse::<ModuleInput>(group_tokens)?);
        }

        if vert_source.is_some() && frag_source.is_none() {
            panic!("GraphicsPipeline missing a fragment shader");
        } else if vert_source.is_none() && frag_source.is_some() {
            panic!("GraphicsPipeline missing a vertex shader");
        } else if let (Some(vert_source), Some(frag_source)) = (vert_source, frag_source) {
            Ok(PipelineInput::Graphics(GraphicsPipelineInput {
                // name: pipeline_name.to_string(),
                vert_source,
                frag_source,
            }))
        } else {
            Ok(PipelineInput::Compute(ComputePipelineInput {
                // name: pipeline_name.to_string(),
                // comp_source: ModuleInput::new(),
            }))
        }
    }
}

impl syn::parse::Parse for ModuleInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut end_span = input.span();
        let mut source = None;
        let mut include_path = None;
        let mut defines = DefinesInput {
            defines: Vec::new(),
        };
        let mut default_defines = true;
        let mut entry = None;
        let mut debug = false;

        while !input.is_empty() {
            let field_type: Ident = input.parse()?;
            let field_type_string = field_type.to_string();

            match field_type_string.as_str() {
                "p" | "path" => {
                    input.parse::<Token![:]>()?;

                    if source.is_some() {
                        return Err(Error::new(
                            field_type.span(),
                            "'source' or 'path' field already specified",
                        ));
                    }

                    let path: LitStr = input.parse()?;
                    end_span = path.span();
                    source = Some(read_shader_source(path.value(), path.span())?);

                    if include_path.is_none() {
                        let source_path_string = path.value();
                        let source_path = Path::new(source_path_string.as_str());
                        include_path = Some(
                            source_path
                                .parent()
                                .ok_or(Error::new(path.span(), "File does not have a directory"))?
                                .to_str()
                                .unwrap_or_else(|| panic!("Path unwrap failed"))
                                .into(),
                        );
                    }
                }
                "s" | "src" | "source" => {
                    input.parse::<Token![:]>()?;

                    if source.is_some() {
                        return Err(Error::new(
                            field_type.span(),
                            "'source' or 'path' field already specified",
                        ));
                    }

                    let source_lit: LitStr = input.parse()?;
                    end_span = source_lit.span();
                    source = Some(source_lit.value());
                }
                "i" | "inc" | "include" => {
                    input.parse::<Token![:]>()?;

                    let path: LitStr = input.parse()?;
                    end_span = path.span();
                    include_path = Some(path.value());
                }
                "d" | "def" | "define" => {
                    input.parse::<Token![:]>()?;

                    let group: Group = input.parse()?;
                    end_span = group.span();

                    let group_tokens: TokenStream = group.stream().into();
                    defines += syn::parse::<DefinesInput>(group_tokens)?;
                }
                "n" | "na" | "no-autodefine" => {
                    default_defines = false;
                }
                "e" | "ep" | "entry" => {
                    input.parse::<Token![:]>()?;

                    let ep: LitStr = input.parse()?;
                    end_span = ep.span();
                    entry = Some(ep.value());
                }
                "debug" => {
                    debug = true;
                }
                _ => {
                    return Err(Error::new(
                        field_type.span(),
                        format!("Invalid field '{}'", field_type_string),
                    ));
                }
            }
        }

        let source = source.ok_or(Error::new(
            end_span,
            "Missing shader source add either 'source' or 'path' field",
        ))?;

        Ok(Self {
            source,
            include_path,
            defines,
            default_defines,
            entry,
            debug,
            span: end_span,
        })
    }
}

fn read_shader_source(path: String, span: Span) -> syn::Result<String> {
    let root = // Span::call_site().source_file();
						env::var("CARGO_MANIFEST_DIR").unwrap_or(".".into());
    let root_path = Path::new(root.as_str());

    let full_path = root_path.join(path);

    if !full_path.is_file() {
        Err(Error::new(span, "File not found"))
    } else {
        let mut file = File::open(full_path).or(Err(Error::new(span, "Could not open file")))?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .or(Err(Error::new(span, "Could not read from file")))?;

        Ok(buf)
    }
}

// impl processed

impl Pipeline {
    fn new(input: PipelineInput) -> syn::Result<Self> {
        Ok(match input {
            PipelineInput::Graphics(p) => {
                let vertex_artifact = compile_shader_module(
                    shaderc::ShaderKind::Vertex,
                    p.vert_source.source.as_str(),
                    "vert",
                    p.vert_source.entry.as_ref().map_or("main", |e| e.as_str()),
                    p.vert_source
                        .include_path
                        .as_ref()
                        .map_or(None, |s| Some(Path::new(s))),
                    &p.vert_source.defines,
                    p.vert_source.default_defines,
                    p.vert_source.debug,
                )
                .or_else(|err| {
                    Err(Error::new(
                        p.vert_source.span,
                        format!("Module source compilation failed: {}", err),
                    ))
                })?;
                let fragment_artifact = compile_shader_module(
                    shaderc::ShaderKind::Fragment,
                    p.frag_source.source.as_str(),
                    "frag",
                    p.frag_source.entry.as_ref().map_or("main", |e| e.as_str()),
                    p.frag_source
                        .include_path
                        .as_ref()
                        .map_or(None, |s| Some(Path::new(s))),
                    &p.frag_source.defines,
                    p.frag_source.default_defines,
                    p.frag_source.debug,
                )
                .or_else(|err| {
                    Err(Error::new(
                        p.frag_source.span,
                        format!("Module source compilation failed: {}", err),
                    ))
                })?;

                Pipeline::Graphics(GraphicsPipeline {
                    // name: p.name.clone(),
                    vert: CompiledModule {
                        spirv: vertex_artifact,
                    },
                    frag: CompiledModule {
                        spirv: fragment_artifact,
                    },
                })
            }
            PipelineInput::Compute(_) => Pipeline::Compute(ComputePipeline { /* name: p.name */ }),
        })
    }
}

fn compile_shader_module(
    kind: shaderc::ShaderKind,
    source: &str,
    name: &str,
    entry: &str,
    include_path: Option<&Path>,
    defines: &DefinesInput,
    default_defines: bool,
    debug: bool,
) -> shaderc::Result<shaderc::CompilationArtifact> {
    let mut compiler =
        shaderc::Compiler::new().unwrap_or_else(|| panic!("Could not create a shaderc Compiler"));
    let mut options = shaderc::CompileOptions::new()
        .unwrap_or_else(|| panic!("Could not create a shaderc CompileOptions"));
    options.set_optimization_level(shaderc::OptimizationLevel::Performance);
    options.set_include_callback(
        |name: &str, _include_type: shaderc::IncludeType, _source: &str, _depth: usize| {
            let full_path = include_path.ok_or("No include path")?.join(name);
            let mut file = File::open(&full_path).or(Err(format!(
                "Could not open file '{}'",
                full_path.to_str().ok_or("Path unwrap failed")?
            )))?;

            let mut content = String::new();
            file.read_to_string(&mut content).or(Err(format!(
                "Could not read from file '{}'",
                full_path.to_str().ok_or("Path unwrap failed")?
            )))?;

            Ok(shaderc::ResolvedInclude {
                content,
                resolved_name: String::from(
                    full_path
                        .to_str()
                        .unwrap_or_else(|| panic!("Path unwrap failed")),
                ),
            })
        },
    );

    match (kind, default_defines) {
        (shaderc::ShaderKind::Vertex, true) => {
            options.add_macro_definition("GEARS_VERTEX", None);
            options.add_macro_definition("GEARS_FRAG(_code)", None);
            options.add_macro_definition(
                "GEARS_IN(_location, _data)",
                Some("layout(location = _location) in _data;"),
            );
            options.add_macro_definition("GEARS_OUT(_location, _data)", Some("_data;"));
            options.add_macro_definition(
                "GEARS_INOUT(_location, _data)",
                Some("layout(location = _location) out _data;"),
            );
        }
        (shaderc::ShaderKind::Fragment, true) => {
            options.add_macro_definition("GEARS_FRAGMENT", None);
            options.add_macro_definition("GEARS_IN(_location, _data)", Some("_data;"));
            options.add_macro_definition(
                "GEARS_OUT(_location, _data)",
                Some("layout(location = _location) out _data;"),
            );
            options.add_macro_definition(
                "GEARS_INOUT(_location, _data)",
                Some("layout(location = _location) in _data;"),
            );
        }
        _ => (),
    };

    for (define, val) in defines.defines.iter() {
        options.add_macro_definition(define, val.as_ref().map_or(None, |s| Some(s.as_str())));
    }

    // debug
    if debug {
        let text = compiler
            .preprocess(source, name, entry, Some(&options))
            .unwrap_or_else(|err| panic!("GLSL preprocessing failed: {}", err))
            .as_text();
        panic!("GLSL: {}", text);
    };

    // output spirv
    compiler.compile_into_spirv(source, kind, name, entry, Some(&options))
}

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
/// ### example
/// ```
/// gears_pipeline::pipeline! {
///     vs: {
///         source: "#version 440\nvoid main(){}"
///         include: "include-path"
///     }
///     fs: {
///         path: "tests/test.glsl"
///         def: [ "FRAGMENT", "VALUE" = "2" ]
///     }
/// }
/// ```
/// will produce
/// ```
/// pub const VERTEX_SPIRV: &[u8] = &[ /* ... */ ];
/// pub const FRAGMENT_SPIRV: &[u8] = &[ /* ... */ ];
/// ```
///
/// ## gears-pipeline defines
///
/// ### for vertex shaders:
///
///  - ```#define GEARS_VERTEX```
///
///  - ```#define GEARS_IN(_location, _data) layout(location = _location) in _data;```
///
///  - ```#define GEARS_INOUT(_location, _data) layout(location = _location) out _data;```
///
///  - ```#define GEARS_OUT(_location, _data) _data;```
///
/// ### for vertex shaders:
///
///  - ```#define GEARS_FRAGMENT```
///
///  - ```#define GEARS_IN(_location, _data) _data;```
///
///  - ```#define GEARS_INOUT(_location, _data) layout(location = _location) in _data;```
///
///  - ```#define GEARS_OUT(_location, _data) layout(location = _location) out _data;```
///
/// ## gears-pipeline default entry points
///
/// - vertex shader: ```vert```
///
/// - fragment shader: ```frag```
#[proc_macro]
pub fn pipeline(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as PipelineInput);

    let pipeline = match Pipeline::new(input) {
        Err(e) => {
            return e.to_compile_error().into();
        }
        Ok(p) => p,
    };

    let expr = match pipeline {
        Pipeline::Graphics(p) => {
            let bin_vs = p.vert.spirv.as_binary_u8();
            let bin_fs = p.frag.spirv.as_binary_u8();
            quote! {
                pub const VERTEX_SPIRV: &[u8] = &[ #(#bin_vs),* ];
                pub const FRAGMENT_SPIRV: &[u8] = &[ #(#bin_fs),* ];
            }
        }
        Pipeline::Compute(_) => {
            quote! {}
        }
    };

    expr.into()
}
