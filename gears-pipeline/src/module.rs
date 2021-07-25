use crate::compiler::{compile_shader_module, DefinesInput};
use proc_macro::TokenStream;
use proc_macro2::{Group, Ident, Span};
use quote::{quote, ToTokens, TokenStreamExt};
use regex::Regex;
use shaderc::ShaderKind;
use std::{
    fs::{canonicalize, File},
    io::Read,
    path::PathBuf,
};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
    AttributeArgs, Error, Lit, LitInt, Meta, NestedMeta, Token,
};

pub fn name_to_kind(name: &str) -> Result<ShaderKind, &'static str> {
    match name {
        "vert" | "vertex" => Ok(ShaderKind::Vertex),
        "frag" | "fragment" => Ok(ShaderKind::Fragment),
        "geom" | "geometry" => Ok(ShaderKind::Geometry),
        _ => Err("Invalid shader source kind"),
    }
}

pub fn module(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as AttributeArgs);

    let mut shader_kind = None;
    let mut shader_path = None;
    // TODO: let mut shader_debug = None;
    let mut shader_name = None;
    let mut shader_defines = DefinesInput::new();

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

    compile_module(source, path, kind, debug, name, &shader_defines)
}

#[derive(Debug)]
pub struct LayoutDef {
    location: u32,
    binding: u32,
}

fn parse_second_def(input: ParseStream) -> syn::Result<(Ident, u32)> {
    input.parse::<Token![,]>()?;
    let id = input.parse::<Ident>()?;
    input.parse::<Token![=]>()?;
    Ok((id, input.parse::<LitInt>()?.base10_parse::<u32>()?))
}

impl Parse for LayoutDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        syn::parenthesized!(content in input);

        let mut target = LayoutDef {
            location: 0,
            binding: 0,
        };

        let id0 = content.parse::<Ident>()?;
        content.parse::<Token![=]>()?;
        let n0 = content.parse::<LitInt>()?.base10_parse::<u32>()?;
        let n1 = parse_second_def(&content);

        match id0.to_string().as_str() {
            "location" => {
                target.location = n0;
                if let Ok((id1, n1)) = n1 {
                    if id1.to_string().as_str() != "binding" {
                        Err(Error::new(
                            Span::call_site(),
                            "Only one of location or binding is allowed",
                        ))?
                    }
                    target.binding = n1;
                }
            }
            "binding" => {
                target.binding = n0;
                if let Ok((id1, n1)) = n1 {
                    if id1.to_string().as_str() != "location" {
                        Err(Error::new(
                            Span::call_site(),
                            "Only one of location or binding is allowed",
                        ))?
                    }
                    target.location = n1;
                }
            }
            other => Err(Error::new(
                Span::call_site(),
                format!("Invalid layout attribute: '{}'", other),
            ))?,
        }

        Ok(target)
    }
}

#[derive(Debug)]
pub enum FieldType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
    Int,
    Uint,
}

impl Parse for FieldType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let token = input.parse::<Ident>()?;
        match token.to_string().as_str() {
            "float" => Ok(Self::Float),
            "vec2" => Ok(Self::Vec2),
            "vec3" => Ok(Self::Vec3),
            "vec4" => Ok(Self::Vec4),
            "mat2" => Ok(Self::Mat2),
            "mat3" => Ok(Self::Mat3),
            "mat4" => Ok(Self::Mat4),
            "int" => Ok(Self::Int),
            "uint" => Ok(Self::Uint),
            other => Err(Error::new(
                token.span(),
                format!("invalid type: '{}'", other),
            )),
        }
    }
}

impl ToTokens for FieldType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            &FieldType::Float => tokens.append(Ident::new("f32", Span::call_site())),
            &FieldType::Vec2 => tokens.append(Ident::new("Vec2", Span::call_site())),
            &FieldType::Vec3 => tokens.append(Ident::new("Vec3", Span::call_site())),
            &FieldType::Vec4 => tokens.append(Ident::new("Vec4", Span::call_site())),
            &FieldType::Mat2 => tokens.append(Ident::new("Mat2", Span::call_site())),
            &FieldType::Mat3 => tokens.append(Ident::new("Mat3", Span::call_site())),
            &FieldType::Mat4 => tokens.append(Ident::new("Mat4", Span::call_site())),
            &FieldType::Int => tokens.append(Ident::new("i32", Span::call_site())),
            &FieldType::Uint => tokens.append(Ident::new("u32", Span::call_site())),
        }
    }
}

#[derive(Debug)]
pub enum LayoutType {
    In,
    Out,
    Uniform,
}

impl Parse for LayoutType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        match input.parse::<Token![in]>() {
            Ok(_) => return Ok(Self::In),
            Err(_) => {}
        };

        let token = input.parse::<Ident>()?;
        match token.to_string().as_str() {
            "out" => Ok(Self::Out),
            "uniform" => Ok(Self::Uniform),
            other => Err(Error::new(
                token.span(),
                format!(
                    "Expected identifier out or uniform or keyword in. Got identifier: '{}'",
                    other
                ),
            )),
        }
    }
}

#[derive(Debug)]
pub struct Layout {
    inputs: Vec<(u32, FieldType)>,
    outputs: Vec<(u32, FieldType)>,
    uniforms: Vec<(u32, Vec<FieldType>)>,
}

#[derive(Debug)]
pub struct SortedLayout {
    inputs: Vec<FieldType>,
    outputs: Vec<FieldType>,
    uniforms: Vec<Vec<FieldType>>,
}

#[derive(Debug)]
pub struct LayoutToken {
    pub span: Span,
}

impl Parse for LayoutToken {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let token = input.parse::<Ident>()?;
        if token.to_string().as_str() == "layout" {
            Ok(Self { span: token.span() })
        } else {
            Err(Error::new(token.span(), "layout token expected"))
        }
    }
}

#[derive(Debug)]
pub struct DataField {
    ty: FieldType,
    name: Ident,
    array: Option<Group>,
    semicolon: Token![;],
}

impl Parse for DataField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            ty: input.parse()?,
            name: input.parse()?,
            array: input.parse()?,
            semicolon: input.parse()?,
        })
    }
}

#[derive(Debug)]
pub struct IOLayoutField {
    layout_token: LayoutToken,
    layout_def: LayoutDef,
    layout_type: LayoutType,

    data: DataField,
}

impl Parse for IOLayoutField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            layout_token: input.parse()?,
            layout_def: input.parse()?,
            layout_type: input.parse()?,
            data: input.parse()?,
        })
    }
}

#[derive(Debug)]
struct Fields {
    data: Vec<DataField>,
}

impl Parse for Fields {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);

        let mut data = Vec::new();
        while !content.is_empty() {
            data.push(content.parse::<DataField>()?);
        }

        Ok(Self { data })
    }
}

#[derive(Debug)]
pub struct UniformLayoutField {
    layout_token: LayoutToken,
    layout_def: LayoutDef,
    layout_type: LayoutType,

    struct_name: Ident,
    fields: Fields,
    instance_name: Ident,
    semicolon: Token![;],
}

impl Parse for UniformLayoutField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            layout_token: input.parse()?,
            layout_def: input.parse()?,
            layout_type: input.parse()?,

            struct_name: input.parse()?,
            fields: input.parse()?,
            instance_name: input.parse()?,
            semicolon: input.parse()?,
        })
    }
}

fn get_layout(source: &str) -> SortedLayout {
    // warning: regex monsters
    let comment_remover = Regex::new(r#"//.*(.|\n)"#).unwrap();
    let comment_block_remover = Regex::new(r#"/\*(.|\n)*?\*/"#).unwrap();
    let io_layout_finder =
        Regex::new(r#"layout(\s|)*?\(.*?\)(\s|)*?(in|out)(\s|)*?[a-zA-Z0-9_].*?;"#).unwrap();
    let uniform_layout_finder = Regex::new(
        r#"layout(\s|)*?\(.*?\)(\s|)*?uniform(\s|)*?(.|\n)*?(\s|)*?\{(.|\n)*?\}(\s|)*?(.|\n)*?;"#,
    )
    .unwrap();

    let source = comment_remover.replace_all(&source, "");
    let source = comment_block_remover.replace_all(&source, "");

    let mut layout = Layout {
        inputs: Vec::new(),
        outputs: Vec::new(),
        uniforms: Vec::new(),
    };

    for m in io_layout_finder.find_iter(&source) {
        let f: IOLayoutField = syn::parse_str(m.as_str()).unwrap();
        match f.layout_type {
            LayoutType::In => layout.inputs.push((f.layout_def.location, f.data.ty)),
            LayoutType::Out => layout.outputs.push((f.layout_def.location, f.data.ty)),
            LayoutType::Uniform => unreachable!(),
        };
    }

    for m in uniform_layout_finder.find_iter(&source) {
        let f: UniformLayoutField = syn::parse_str(m.as_str()).unwrap();
        match f.layout_type {
            LayoutType::In => unreachable!(),
            LayoutType::Out => unreachable!(),
            LayoutType::Uniform => {
                let mut uniform = Vec::new();
                for field in f.fields.data {
                    uniform.push(field.ty);
                }
                layout.uniforms.push((f.layout_def.location, uniform));
            }
        };
    }

    layout.inputs.sort_by(|a, b| a.0.cmp(&b.0));
    layout.outputs.sort_by(|a, b| a.0.cmp(&b.0));
    layout.uniforms.sort_by(|a, b| a.0.cmp(&b.0));

    SortedLayout {
        inputs: layout.inputs.into_iter().map(|(_, f)| f).collect(),
        outputs: layout.outputs.into_iter().map(|(_, f)| f).collect(),
        uniforms: layout.uniforms.into_iter().map(|(_, f)| f).collect(),
    }
}

fn compile_module(
    source: String,
    path: PathBuf,
    kind: ShaderKind,
    debug: bool,
    mod_name: String,
    shader_defines: &DefinesInput,
) -> TokenStream {
    // preprocess source
    let layout = get_layout(source.as_str());
    let inputs = &layout.inputs[..];
    let outputs = &layout.outputs[..];
    let uniforms = layout.uniforms.first().map(|v| &v[..]).unwrap_or(&[]); // only one for now
    let name = Ident::new(mod_name.as_str(), Span::call_site());

    // compile module
    let artifact = compile_shader_module(
        kind,
        &source,
        "module",
        "main",
        path.clone(),
        &shader_defines,
        debug,
    );

    // tokens
    match (artifact, debug) {
        (Ok(spirv), _) => {
            let spirv = spirv.as_binary_u8();
            let path = path.to_str().unwrap();
            (quote! {
                pub mod #name {
                    use gears::glam::{Vec2, Vec3, Vec4, Mat2, Mat3, Mat4};

                    pub const SOURCE: &'static str = include_str!(#path);
                    pub const SPIRV: &'static [u8] = &[ #(#spirv),* ];
                    pub type INPUT = ( #(#inputs,)* );
                    pub type OUTPUT = ( #(#outputs,)* );
                    pub type UNIFORM = ( #(#uniforms,)* );
                }
            })
            .into()
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
