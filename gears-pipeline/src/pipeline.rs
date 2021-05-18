use proc_macro2::{Group, Ident, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{parse::ParseStream, parse_macro_input::ParseMacroInput, Error, Token};

use crate::{
    module::{CompiledModules, InputModule, InputModules, ModuleType},
    ubo::{BindgenFieldType, BindgenStruct, StructRegistry},
};

// struct/enum

pub struct PipelineInput {
    // name: String,
    modules: InputModules,
    builders: bool,
}

pub struct Pipeline {
    // name: String,
    modules: CompiledModules,
    bindgen_structs: Vec<BindgenStruct>,
    builders: bool,
}

// impl

impl Pipeline {
    pub fn new(input: PipelineInput) -> syn::Result<Self> {
        let mut struct_reg = StructRegistry::new();
        let mut bindgen_structs = Vec::new();
        let modules = input
            .modules
            .into_iter()
            .map(|(module_type, input)| {
                Ok((
                    module_type.clone(),
                    input.compile(module_type.clone(), &mut struct_reg, &mut bindgen_structs)?,
                ))
            })
            .collect::<Result<CompiledModules, Error>>()?;
        let builders = input.builders;

        Ok(Pipeline {
            modules,
            bindgen_structs,
            builders,
        })
    }
}

// trait impl

impl ParseMacroInput for PipelineInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut modules = InputModules::new();
        let mut builders = false;

        while !input.is_empty() {
            let shader: Ident = input.parse()?;
            let shader_type_string = shader.to_string();

            let module_type = match shader_type_string.as_str() {
                "v" | "vertex" | "vert" => ModuleType::Vertex,
                "f" | "fragment" | "frag" => ModuleType::Fragment,
                "g" | "geometry" | "geom" => ModuleType::Geometry,
                "builders" => {
                    builders = true;
                    continue;
                }
                _ => {
                    return Err(Error::new(
                        shader.span(),
                        format!("Unknown shader type: {}", shader_type_string),
                    ));
                }
            };

            input.parse::<Token![:]>()?;
            let group: Group = input.parse()?;
            let group_tokens: proc_macro::TokenStream = group.stream().into();

            if modules.contains_key(&module_type) {
                return Err(Error::new(shader.span(), "Duplicate shader module"));
            } else {
                modules.insert(module_type, syn::parse::<InputModule>(group_tokens)?);
            }
        }

        Ok(PipelineInput { modules, builders })
    }
}

impl ToTokens for Pipeline {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        for (_, module) in self.modules.iter() {
            module.to_tokens(tokens);
        }

        for bindgen_struct in self.bindgen_structs.iter() {
            bindgen_struct.to_tokens(tokens);
        }

        if self.builders {
            let ubos: Vec<Ident> = self
                .bindgen_structs
                .iter()
                .filter_map(|s| match s.meta.bind_type {
                    BindgenFieldType::Uniform(_) => Some(format_ident!("{}", s.struct_name)),
                    _ => None,
                })
                .collect();

            let inputs: Vec<Ident> = self
                .bindgen_structs
                .iter()
                .filter_map(|s| match s.meta.bind_type {
                    BindgenFieldType::In(_) => Some(format_ident!("{}", s.struct_name)),
                    _ => None,
                })
                .collect();

            let modules: Vec<TokenStream> = self
                .modules
                .iter()
                .filter_map(|(t, _)| match t {
                    ModuleType::Geometry => Some(
                        quote! {
                            .with_geometry_module(GEOM_SPIRV_REF)
                        }
                        .into(),
                    ),
                    _ => None,
                })
                .collect();

            let builders = quote! {
                fn _build(renderer: &gears::Renderer, debug: bool) -> gears::Pipeline {
                    gears::PipelineBuilder::new(renderer)
                        #( .with_ubo::<#ubos>() )*
                        .with_graphics_modules(VERT_SPIRV_REF, FRAG_SPIRV_REF)
                        #( #modules )*
                        #( .with_input::<#inputs>() )*
                        .build(debug)
                        .unwrap()
                }

                pub fn build(renderer: &gears::Renderer) -> gears::Pipeline {
                    _build(renderer, false)
                }

                pub fn build_with_debug(renderer: &gears::Renderer) -> gears::Pipeline {
                    _build(renderer, true)
                }
            };

            builders.to_tokens(tokens);
        }
    }
}
