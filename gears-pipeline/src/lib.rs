use proc_macro::TokenStream;
use proc_macro2::{Group, Span};
use quote::quote;
use shaderc::CompilationArtifact;
use std::{env, fs::File, io::Read, path::Path};
use syn::{parse::ParseStream, parse_macro_input, Error, Ident, LitStr, Token};

// input

struct ModuleInput {
    source: String,
    include_path: Option<String>,
    span: Span,
}

struct GraphicsPipelineInput {
    name: String,
    vert_source: ModuleInput,
    frag_source: ModuleInput,
    // geom_source: Option<ModuleInput>,
    /* ... */
}

struct ComputePipelineInput {
    name: String,
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
impl parse_macro_input::ParseMacroInput for PipelineInput
// impl syn::parse::Parse for PipelineInput
{
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut vert_source = None;
        let mut frag_source = None;

        let pipeline_name: Ident = input.parse()?;

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
                name: pipeline_name.to_string(),
                vert_source,
                frag_source,
            }))
        } else {
            Ok(PipelineInput::Compute(ComputePipelineInput {
                name: pipeline_name.to_string(),
                // comp_source: ModuleInput::new(),
            }))
        }
    }
}

impl syn::parse::Parse for ModuleInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let start_span = input.span();
        let mut end_span = input.span();
        let mut source = None;
        let mut include_path = None;

        while !input.is_empty() {
            let field_type: Ident = input.parse()?;
            let field_type_string = field_type.to_string();

            input.parse::<Token![:]>()?;

            match field_type_string.as_str() {
                "o" | "path" => {
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
                                .unwrap()
                                .into(),
                        );
                    }
                }
                "s" | "src" | "source" => {
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
                    let path: LitStr = input.parse()?;
                    end_span = path.span();
                    include_path = Some(path.value());
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
            start_span.join(end_span).unwrap(),
            "Missing shader source add either 'source' or 'path' field",
        ))?;

        Ok(Self {
            source,
            include_path,
            span: start_span.join(end_span).unwrap(),
        })
    }
}

fn read_shader_source(path: String, span: Span) -> syn::Result<String> {
    let root = // Span::call_site().source_file();
						env::var("CARGO_MANIFEST_DIR").unwrap_or(".".into());
    let root_path = Path::new(root.as_str());

    let full_path = root_path.join(path);

    if !full_path.is_file() {
        Err(Error::new(span, "Path must be a file path"))
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
                    format!("{}-vert", p.name).as_str(),
                    "main",
                    p.vert_source
                        .include_path
                        .as_ref()
                        .map_or(None, |s| Some(Path::new(s))),
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
                    format!("{}-frag", p.name).as_str(),
                    "main",
                    p.frag_source
                        .include_path
                        .as_ref()
                        .map_or(None, |s| Some(Path::new(s))),
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
) -> shaderc::Result<shaderc::CompilationArtifact> {
    let mut compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.set_optimization_level(shaderc::OptimizationLevel::Performance);
    options.set_include_callback(
        |name: &str, _include_type: shaderc::IncludeType, _source: &str, _depth: usize| {
            let full_path = include_path.ok_or("No include path")?.join(name);
            let mut file = File::open(&full_path).or(Err(format!(
                "Could not open file '{}'",
                full_path.to_str().unwrap()
            )))?;

            let mut content = String::new();
            file.read_to_string(&mut content).or(Err(format!(
                "Could not read from file '{}'",
                full_path.to_str().unwrap()
            )))?;

            Ok(shaderc::ResolvedInclude {
                content,
                resolved_name: String::from(full_path.to_str().unwrap()),
            })
        },
    );

    match kind {
        shaderc::ShaderKind::Vertex => {
            options.add_macro_definition("SHADER_MODULE_VERTEX", None);
        }
        shaderc::ShaderKind::Fragment => {
            options.add_macro_definition("SHADER_MODULE_FRAGMENT", None);
        }
        _ => (),
    };

    // output spirv
    compiler.compile_into_spirv(source, kind, name, entry, Some(&options))
}

// main macro

#[proc_macro]
pub fn pipeline(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as PipelineInput);
    let name = Ident::new(
        match &input {
            PipelineInput::Graphics(p) => p.name.as_str(),
            PipelineInput::Compute(p) => p.name.as_str(),
        },
        Span::call_site(),
    );

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
                mod #name {
                    pub const VERTEX_SPIRV: &[u8] = &[ #(#bin_vs),* ];
                    pub const FRAGMENT_SPIRV: &[u8] = &[ #(#bin_fs),* ];
                }
            }
        }
        Pipeline::Compute(_) => {
            quote! {
                mod #name {}
            }
        }
    };

    expr.into()
}
