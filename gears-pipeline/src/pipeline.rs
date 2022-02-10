use gears_spirv::parse::name_to_kind;
use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::quote;
use shaderc::ShaderKind;
use std::collections::{hash_map::Entry, HashMap};
use syn::{parse::Parse, parse_macro_input, Error, LitInt, LitStr, Token};

struct PipelineIO {
    in_struct: TokenTree,
    _arrow: Token![->],
    out_struct: TokenTree,
}

impl Parse for PipelineIO {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            in_struct: input.parse()?,
            _arrow: input.parse()?,
            out_struct: input.parse()?,
        })
    }
}

struct PipelineModule {
    _mod_token: Token![mod],
    module_name: LitStr,
    _as_token: Token![as],
    module_kind: LitStr,
    uniforms: Option<PipelineWhere>,
}

impl Parse for PipelineModule {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            _mod_token: input.parse()?,
            module_name: input.parse()?,
            _as_token: input.parse()?,
            module_kind: input.parse()?,
            uniforms: if input.peek(Token![where]) {
                Some(input.parse()?)
            } else {
                None
            },
        })
    }
}

struct PipelineUniform {
    _in_token: Token![in],
    in_struct: Ident,
    _as_token: Token![as],
    in_binding: LitInt,
}

impl Parse for PipelineUniform {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);

        Ok(Self {
            _in_token: content.parse()?,
            in_struct: content.parse()?,
            _as_token: content.parse()?,
            in_binding: content.parse()?,
        })
    }
}

struct PipelineWhere {
    _where_token: Token![where],
    in_module: PipelineUniform,
}

impl Parse for PipelineWhere {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            _where_token: input.parse()?,
            in_module: input.parse()?,
        })
    }
}

struct PipelineInput {
    name: LitStr,
    input: PipelineIO,

    vertex: PipelineModule,
    fragment: PipelineModule,
    modules: HashMap<usize, PipelineModule>,
}

impl Parse for PipelineInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let mut pipeline_input = None;
        let mut vertex = None;
        let mut fragment = None;
        let mut modules = HashMap::new();

        while !input.is_empty() {
            if input.peek2(Token![->]) {
                if pipeline_input.is_some() {
                    return Err(input.error("Pipeline input already given"));
                } else {
                    pipeline_input = Some(input.parse()?);
                }
            } else if input.peek(Token![mod]) {
                let module: PipelineModule = input.parse()?;
                let kind = match name_to_kind(module.module_kind.value().as_str()) {
                    Ok(kind) => kind,
                    Err(err) => return Err(Error::new(module.module_kind.span(), err)),
                };

                match (kind, &vertex, &fragment) {
                    (ShaderKind::Vertex, None, _) => vertex = Some(module),
                    (ShaderKind::Fragment, _, None) => fragment = Some(module),
                    (ShaderKind::Vertex | ShaderKind::Fragment, _, _) => {
                        return Err(Error::new(
                            module.module_kind.span(),
                            format!("Duplicate '{}' module", module.module_kind.value()),
                        ));
                    }
                    _ => {
                        let kind_id = kind as usize;
                        if let Entry::Vacant(e) = modules.entry(kind_id) {
                            e.insert(module);
                        } else {
                            return Err(Error::new(
                                module.module_kind.span(),
                                format!("Duplicate '{}' module", module.module_kind.value()),
                            ));
                        }
                    }
                }
            } else {
                return Err(input.error("Expected token 'mod' or '->' after a struct name"));
            }
        }

        let pipeline_input = match pipeline_input {
            Some(pipeline_input) => pipeline_input,
            None => return Err(input.error("No pipeline input type given")),
        };

        let vertex = match vertex {
            Some(vertex) => vertex,
            None => return Err(input.error("No vertex module given")),
        };

        let fragment = match fragment {
            Some(fragment) => fragment,
            None => return Err(input.error("No fragment module given")),
        };

        Ok(Self {
            name,
            input: pipeline_input,

            vertex,
            fragment,
            modules,
        })
    }
}

impl PipelineInput {
    fn get_uniform_tokens(module: &PipelineModule) -> TokenStream {
        match module {
            PipelineModule {
                uniforms: Some(uniform),
                ..
            } => {
                let uniform = &uniform.in_module.in_struct;
                quote! {#uniform}
            }
            _ => quote! {()},
        }
    }

    fn get_uniform_assert_tokens(module: &PipelineModule, module_name: &Ident) -> TokenStream {
        match module {
            PipelineModule {
                uniforms: Some(uniform),
                ..
            } => {
                let uniform = &uniform.in_module.in_struct;
                quote! { gears::static_assertions::assert_type_eq_all!(<#uniform as gears::renderer::pipeline::Uniform>::Fields, #module_name::UNIFORM); }
            }
            _ => quote! {},
        }
    }

    fn get_module(module: &PipelineModule) -> (TokenStream, TokenStream, Ident, Option<u32>) {
        let module_name = Ident::new(
            module.module_name.value().as_str(),
            module.module_name.span(),
        );

        (
            Self::get_uniform_tokens(module),
            Self::get_uniform_assert_tokens(module, &module_name),
            module_name,
            module.uniforms.as_ref().map(|u| {
                u.in_module
                    .in_binding
                    .base10_digits()
                    .parse::<u32>()
                    .expect("Binding must be u32")
            }),
        )
    }

    fn get_module2(
        module: Option<&PipelineModule>,
    ) -> (TokenStream, TokenStream, Option<(Ident, Option<u32>)>) {
        match module {
            Some(module) => {
                let get_module = Self::get_module(module);
                (
                    get_module.0,
                    get_module.1,
                    Some((get_module.2, get_module.3)),
                )
            }
            None => (quote! {}, quote! {}, None),
        }
    }

    fn output(self) -> proc_macro::TokenStream {
        let name = Ident::new(self.name.value().as_str(), self.name.span());
        let input = self.input.in_struct;
        let output = self.input.out_struct;

        let load_spirv = quote! {
            ::load_spirv()?
        };
        let wrap_err = quote! {
            .map_err(|err| gears::renderer::pipeline::PipelineError::BufferError(err))?
        };

        // mandatory modules
        let (vert_uniform, vert_uniform_assert, vert, vert_uniform_binding) =
            Self::get_module(&self.vertex);
        let (frag_uniform, frag_uniform_assert, frag, frag_uniform_binding) =
            Self::get_module(&self.fragment);

        let vert_call = if let Some(binding) = vert_uniform_binding {
            quote! { .vertex_uniform(#vert #load_spirv, #vert_uniform::default(), #binding) }
        } else {
            quote! { .vertex(#vert #load_spirv) }
        };

        let frag_call = if let Some(binding) = frag_uniform_binding {
            quote! { .fragment_uniform(#frag #load_spirv, #frag_uniform::default(), #binding) }
        } else {
            quote! { .fragment(#frag #load_spirv) }
        };

        // optional modules
        let (geom_uniform, geom_uniform_assert, geom) =
            Self::get_module2(self.modules.get(&(ShaderKind::Geometry as usize)));

        let geom_call = match &geom {
            Some((geom, Some(binding))) => {
                quote! { .geometry_uniform(#geom #load_spirv, #geom_uniform::default(), #binding) }
            }
            Some((geom, None)) => {
                quote! { .geometry(#geom #load_spirv) }
            }
            None => quote! {},
        };

        // type list

        let geom_uniform = if geom.is_some() {
            quote! { #geom_uniform }
        } else {
            quote! { () }
        };

        // pipeline stage asserts

        let vert_stage = vert.clone();
        let geom_stage = geom.as_ref().map(|(geom, _)| geom.clone());
        let frag_stage = frag.clone();
        let stages = [Some(vert_stage), geom_stage, Some(frag_stage)]
            .iter()
            .filter_map(|stage| stage.to_owned())
            .collect::<Vec<Ident>>();

        let mut stage_asserts = TokenStream::new();
        for (l, r) in stages.iter().zip(stages.iter().skip(1)) {
            stage_asserts = quote! {
                #stage_asserts
                gears::static_assertions::assert_type_eq_all!(#l::OUTPUT, #r::INPUT);
            };
        }

        // type
        let target_type_generics =
            quote! { #input, #output, #vert_uniform, #geom_uniform, #frag_uniform };
        let target_type =
            quote! { gears::renderer::pipeline::GraphicsPipeline<#target_type_generics> };

        (quote! {
			pub struct #name (#target_type);
            impl #name {
				pub fn build(renderer: &gears::renderer::Renderer) -> Result<Self, gears::renderer::pipeline::PipelineError> {
					gears::static_assertions::assert_type_eq_all!(<#input as gears::renderer::pipeline::Input>::Fields, #vert::INPUT);
					gears::static_assertions::assert_type_eq_all!(<#output as gears::renderer::pipeline::Output>::Fields, #frag::OUTPUT);
					
					#stage_asserts
		
					#vert_uniform_assert
					#geom_uniform_assert
					#frag_uniform_assert
					
					Ok(Self {
						0: gears::renderer::pipeline::factory::Pipeline::builder()
							#vert_call
							#frag_call
							#geom_call
							.input::<#input>()
							.output::<#output>()
							.build(renderer)
							#wrap_err
					})
				}
            }
            impl std::ops::Deref for #name {
                type Target = #target_type;
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }
        })
        .into()
    }
}

pub fn pipeline(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    parse_macro_input!(input as PipelineInput).output()
}
