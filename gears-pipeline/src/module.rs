use crate::{
    compiler::{self, DefinesInput},
    ubo::{BindgenFieldType, BindgenStruct, StructRegistry},
};

use proc_macro::TokenStream;
use proc_macro2::{Group, Ident, Span};
use quote::{format_ident, quote, ToTokens};
use regex::{Captures, Regex};
use shaderc::CompilationArtifact;
use std::{collections::HashMap, env, fs::File, io::Read, path::Path};
use syn::{parse::ParseStream, Error, LitStr, Token};

// struct/enum

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum ModuleType {
    Vertex,
    Fragment,
    Geometry,
}

pub struct InputModule {
    source: String,
    source_file: Option<String>,
    include_path: Option<String>,
    defines: DefinesInput,
    default_defines: bool,
    entry: Option<String>,
    debug: bool,
    span: Span,
}

pub type InputModules = HashMap<ModuleType, InputModule>;
pub type CompiledModules = HashMap<ModuleType, CompiledModule>;

pub struct CompiledModule {
    spirv: CompilationArtifact,
    module_type: ModuleType,
    source_file: Option<String>,
}

// impl

impl ModuleType {
    pub fn name(&self) -> &'static str {
        match self {
            ModuleType::Fragment => "FRAG",
            ModuleType::Vertex => "VERT",
            ModuleType::Geometry => "GEOM",
        }
    }

    pub fn kind(&self) -> shaderc::ShaderKind {
        match self {
            ModuleType::Fragment => shaderc::ShaderKind::Fragment,
            ModuleType::Vertex => shaderc::ShaderKind::Vertex,
            ModuleType::Geometry => shaderc::ShaderKind::Geometry,
        }
    }
}

impl InputModule {
    pub fn compile(
        self,
        module_type: ModuleType,
        struct_reg: &mut StructRegistry,
        bindgen_structs: &mut Vec<BindgenStruct>,
    ) -> Result<CompiledModule, Error> {
        let span = self.span;

        let (source, mut new_bindgen_structs) =
            preprocess_glsl(self.source.as_str(), module_type.clone(), struct_reg);

        bindgen_structs.append(&mut new_bindgen_structs);

        let spirv = compiler::compile_shader_module(
            module_type.kind(),
            source.as_ref(),
            module_type.name(),
            self.entry.as_ref().map_or("main", |e| e.as_str()),
            self.include_path
                .as_ref()
                .map_or(None, |s| Some(Path::new(s))),
            &self.defines,
            self.default_defines,
            self.debug,
        )
        .or_else(|err| Err(Error::new(span, err)))?;

        let source_file = self.source_file;

        Ok(CompiledModule {
            spirv,
            module_type,
            source_file,
        })
    }
}

// trait impl

impl syn::parse::Parse for InputModule {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut end_span = input.span();
        let mut source = None;
        let mut source_file = None;
        let mut include_path = None;
        let mut defines = DefinesInput::new();
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
                    let (s, f) = read_shader_source(path.value(), path.span())?;
                    source = Some(s);
                    source_file = Some(f);

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
            source_file,
            include_path,

            defines,
            default_defines,

            entry,
            debug,
            span: end_span,
        })
    }
}

impl ToTokens for CompiledModule {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let field_name = format_ident!("{}_SPIRV", self.module_type.name());
        let field_ref_name = format_ident!("{}_SPIRV_REF", self.module_type.name());
        let spirv = self.spirv.as_binary_u8();
        let len = spirv.len();

        if let Some(source_file) = self.source_file.as_ref() {
            let field = quote! {
                // recompile on write hack:
                const _: &str = include_str!(#source_file);
            };

            field.to_tokens(tokens);
        }

        let field = quote! {
            // spirv:
            pub const #field_name: [u8; #len] = [ #( #spirv ),* ];
            pub const #field_ref_name: &[u8] = &#field_name;
        };

        field.to_tokens(tokens);
    }
}

// pub fn

// fn

fn preprocess_glsl<'a>(
    source: &'a str,
    module: ModuleType,
    struct_reg: &mut StructRegistry,
) -> (String, Vec<BindgenStruct>) {
    struct_reg.next_module();

    let comment_matcher = Regex::new(r#"(//.*)|(/\*(.|(\r?\n))*?\*/)"#).unwrap();

    let attrib_matcher =
        Regex::new(r#"#\[gears_(bind)?(gen)\(.+\)\]((\r?\n)?.+)\{([^}]+)*(\r?\n)?\}.+;"#).unwrap();

    let mut bindgen_structs = Vec::new();
    let mut ident_renameres = Vec::new();

    let mut output = comment_matcher.replace_all(source, " ").to_string();

    output = attrib_matcher
        .replace_all(&output[..], |caps: &Captures| {
            let cap = &caps[0];
            match syn::parse_str::<BindgenStruct>(cap) {
                Ok(mut s) => {
                    s.meta.in_module = module;
                    s.generate(struct_reg);
                    let glsl = format!("\n{}", s.to_glsl());

                    // uniforms do not have to be renamed
                    match &s.meta.bind_type {
                        BindgenFieldType::Uniform(_) => (),
                        BindgenFieldType::In(_) | BindgenFieldType::Out(_) => {
                            ident_renameres.push(
                                Regex::new(format!("\\b{}\\.\\b", s.field_name).as_str()).unwrap(),
                            );
                        }
                    };

                    // bind only gears_bindgen not gears_gen for ex.
                    if s.meta.bind {
                        bindgen_structs.push(s);
                    }
                    glsl
                }
                Err(e) => {
                    panic!("attrib failed: {:?}, {}", e.to_string(), cap);
                }
            }
        })
        .to_string();

    for ident_renamer in ident_renameres {
        output = ident_renamer
            .replace_all(&output[..], |caps: &Captures| {
                let cap = &caps[0];
                let cap = &cap[..cap.len() - 1];
                format!("_{}_", cap)
            })
            .to_string();
    }

    (output, bindgen_structs)
}

// 0: source, 1: path
fn read_shader_source(path: String, span: Span) -> syn::Result<(String, String)> {
    let root = env::var("CARGO_MANIFEST_DIR").unwrap();

    let error_msg = format!("File not found: '{}' does not exist in '{}'", path, root);
    let full_path = Path::new(&root).join(path);
    let full_path_str = full_path.as_os_str().to_str().unwrap().to_string();

    if !full_path.is_file() {
        Err(Error::new(span, error_msg))
    } else {
        let mut file = File::open(full_path)
            .or_else(|err| Err(Error::new(span, format!("Could not open file: {}", err))))?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .or(Err(Error::new(span, "Could not read from file")))?;

        Ok((buf, full_path_str))
    }
}
