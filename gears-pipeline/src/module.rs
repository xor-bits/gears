use gears_spirv::{
    compiler::{compile_shader_module, preprocess_shader_module, DefinesInput},
    parse::{get_layout, kind_to_name, name_to_kind, SortedLayout},
};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use shaderc::ShaderKind;
use std::{
    fs::{canonicalize, File},
    io::Read,
    path::{Path, PathBuf},
};
use syn::{parse_macro_input, spanned::Spanned, AttributeArgs, Error, Lit, Meta, NestedMeta};

pub fn module(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as AttributeArgs);

    let mut shader_kind = None;
    let mut shader_path = None;
    // TODO: let mut shader_debug = None;
    let mut shader_name = None;
    let mut shader_defines = DefinesInput::new();
    let mut shader_runtime = None;

    for nm in input {
        let meta = match nm {
            NestedMeta::Meta(meta) => meta,
            NestedMeta::Lit(lit) => {
                return Error::new(lit.span(), "Expected meta attribute but got literal")
                    .to_compile_error()
                    .into()
            }
        };

        let name_value = match meta {
            Meta::NameValue(name_value) => name_value,
            meta => {
                return Error::new(meta.span(), "Expected NameValue attribute")
                    .to_compile_error()
                    .into()
            }
        };

        let name = name_value
            .path
            .segments
            .iter()
            .map(|seg| format!("::{}", seg.ident.to_string()))
            .collect::<String>();

        let value_span = name_value.lit.span();
        let value = match name_value.lit {
            Lit::Str(s) => s,
            _ => {
                return Error::new(value_span, "Invalid literal string")
                    .to_compile_error()
                    .into()
            }
        };

        match name.as_str() {
            "::kind" => {
                shader_kind = Some(match name_to_kind(value.value().as_str()) {
                    Ok(kind) => kind,
                    Err(err) => return Error::new(value_span, err).to_compile_error().into(),
                })
            }
            "::path" => shader_path = Some(value.value()),
            "::name" => shader_name = Some(value.value()),
            "::define" => {
                if let Some((l, r)) = value.value().split_once('=') {
                    shader_defines.push((l.into(), Some(r.into())));
                } else {
                    shader_defines.push((value.value(), None));
                }
            }
            "::runtime" => shader_runtime = Some(value.value()),
            _ => {
                return Error::new(name_value.path.span(), "Invalid item name")
                    .to_compile_error()
                    .into()
            }
        }
    }

    let path = match shader_path {
        Some(path) => match canonicalize(path) {
            Ok(path) => path,
            Err(err) => {
                return Error::new(Span::call_site(), err.to_string())
                    .to_compile_error()
                    .into()
            }
        },
        None => {
            return Error::new(Span::call_site(), "No source path given")
                .to_compile_error()
                .into()
        }
    };

    let source = match File::open(path.clone()) {
        Ok(mut file) => {
            let mut source = String::new();
            match file.read_to_string(&mut source) {
                Ok(_) => source,
                Err(err) => {
                    return Error::new(Span::call_site(), err.to_string())
                        .to_compile_error()
                        .into()
                }
            }
        }
        Err(err) => {
            return Error::new(Span::call_site(), err.to_string())
                .to_compile_error()
                .into()
        }
    };

    let kind = match shader_kind {
        Some(kind) => kind,
        None => {
            return Error::new(Span::call_site(), "No source kind given")
                .to_compile_error()
                .into()
        }
    };

    let debug = false; /* TODO: match shader_debug {
                           Some(debug) => debug,
                           None => false,
                       }; */

    let name = match shader_name {
        Some(name) => name,
        None => {
            return Error::new(Span::call_site(), "No shader name given")
                .to_compile_error()
                .into()
        }
    };

    if let Some(shader_runtime) = shader_runtime {
        post_compile_module(
            source,
            path,
            kind,
            debug,
            name,
            &shader_defines,
            shader_runtime,
        )
    } else {
        pre_compile_module(source, path, kind, debug, name, &shader_defines)
    }
}

fn layout_to_ident(layout: &SortedLayout) -> (Vec<Ident>, Vec<Ident>, Vec<Ident>) {
    let inputs = layout
        .inputs
        .iter()
        .map(|f| f.to_ident())
        .collect::<Vec<_>>();
    let outputs = layout
        .outputs
        .iter()
        .map(|f| f.to_ident())
        .collect::<Vec<_>>();
    let uniforms = layout
        .uniforms
        .first()
        .map(|v| &v[..])
        .unwrap_or(&[])
        .iter()
        .map(|f| f.to_ident())
        .collect::<Vec<_>>(); // only one for now

    (inputs, outputs, uniforms)
}

fn make_module(
    layout: &SortedLayout,
    mod_name: &str,
    path: &Path,
    load_spirv: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let (inputs, outputs, uniforms) = layout_to_ident(&layout);
    let name = Ident::new(mod_name, Span::call_site());
    let path = path.to_str().unwrap();

    quote! {
        pub mod #name {
            use gears::glam::{Vec2, Vec3, Vec4, DVec2, DVec3, DVec4, Mat2, Mat3, Mat4, DMat2, DMat3, DMat4};

            #load_spirv

            pub const SOURCE: &'static str = include_str!(#path);
            pub type INPUT = ( #(#inputs,)* );
            pub type OUTPUT = ( #(#outputs,)* );
            pub type UNIFORM = ( #(#uniforms,)* );
        }
    }
}

fn post_compile_module(
    source: String,
    path: PathBuf,
    kind: ShaderKind,
    debug: bool,
    mod_name: String,
    shader_defines: &DefinesInput,
    shader_runtime: String,
) -> TokenStream {
    let layout = get_layout(source.as_str());
    let load_spirv = make_load_spirv(
        &layout,
        &mod_name,
        &path,
        kind,
        shader_runtime,
        shader_defines,
        debug,
    );

    (make_module(&layout, &mod_name, &path, load_spirv)).into()
}

fn pre_compile_module(
    source: String,
    path: PathBuf,
    kind: ShaderKind,
    debug: bool,
    mod_name: String,
    shader_defines: &DefinesInput,
) -> TokenStream {
    // preprocess module
    let layout = match preprocess_shader_module(
        &source,
        mod_name.as_str(),
        "main",
        Some(path.clone()),
        &shader_defines,
    ) {
        Err(err) => {
            return Error::new(Span::call_site(), err)
                .into_compile_error()
                .into()
        }
        Ok(source) => {
            if debug {
                return Error::new(Span::call_site(), source)
                    .into_compile_error()
                    .into();
            } else {
                get_layout(source.as_str())
            }
        }
    };

    // compile module
    let artifact = compile_shader_module(
        kind,
        &source,
        mod_name.as_str(),
        "main",
        Some(path.clone()),
        &shader_defines,
    );

    // tokens
    match (artifact, debug) {
        (Ok(spirv), _) => {
            let spirv = spirv.as_binary_u8();
            let load_spirv = make_const_load_spirv(spirv);
            make_module(&layout, &mod_name, &path, load_spirv).into()
        }
        (Err(error), true) => (quote! {
            #error
        })
        .into(),
        (Err(error), false) => {
            panic!("{}", error)
        }
    }
}

fn make_load_spirv(
    layout: &SortedLayout,
    name_str: &str,
    path: &Path,
    kind: ShaderKind,
    shader_runtime: String,
    shader_defines: &DefinesInput,
    debug: bool,
) -> proc_macro2::TokenStream {
    let layout_inputs = &layout.inputs[..];
    let layout_outputs = &layout.outputs[..];
    let layout_uniforms = layout.uniforms.first().map(|v| &v[..]).unwrap_or(&[]);

    let path = path.to_str().unwrap();
    let kind_str = Ident::new(kind_to_name(kind).unwrap(), Span::call_site());
    let shader_runtime = Ident::new(&shader_runtime, Span::call_site());

    let mut defines =
        quote! { let mut defines = gears::gears_spirv::compiler::DefinesInput::new(); };
    for (define, value) in shader_defines {
        defines = quote! { #defines defines.push((#define, #value)); };
    }

    quote! {
        fn layout_assert(
            l: &[gears::gears_spirv::parse::FieldType],
            r: &[gears::gears_spirv::parse::FieldType],
        ) -> Result<(), gears::renderer::pipeline::PipelineError> {
            if l != r {
                Err(gears::renderer::pipeline::PipelineError::LayoutMismatch(
                    format!("left != right\n  left: '{:?}'\n right: '{:?}'", l, r),
                ))
            } else {
                Ok(())
            }
        }
        pub fn load_spirv() -> Result<std::borrow::Cow<'static, [u8]>, gears::renderer::pipeline::PipelineError> {
            let source = super:: #shader_runtime (#name_str);
            let layout = gears::gears_spirv::parse::get_layout(&source);

            #defines

            let inputs = &layout.inputs[..];
            let outputs = &layout.outputs[..];
            let uniforms = layout.uniforms.first().map(|v| &v[..]).unwrap_or(&[]);

            layout_assert(inputs, &[ #(#layout_inputs),* ])?;
            layout_assert(outputs, &[ #(#layout_outputs),* ])?;
            layout_assert(uniforms, &[ #(#layout_uniforms),* ])?;

            let artifact: Vec<u8> = gears::gears_spirv::compiler::compile_shader_module(
                gears::gears_spirv::parse::ShaderKind::#kind_str,
                &source,
                #name_str,
                "main",
                Some(std::path::Path::new( #path ).into()),
                &defines,
                #debug,
            )
            .map_err(|err| gears::renderer::pipeline::PipelineError::CompileError(err))?
            .as_binary_u8()
            .into();

            Ok(std::borrow::Cow::Owned(artifact))
        }
    }
}

fn make_const_load_spirv(spirv: &[u8]) -> proc_macro2::TokenStream {
    quote! {
        pub const fn load_spirv() -> Result<std::borrow::Cow<'static, [u8]>, gears::renderer::pipeline::PipelineError> { Ok(std::borrow::Cow::Borrowed(SPIRV)) }
        pub const SPIRV: &'static [u8] = &[ #(#spirv),* ];
    }
}
