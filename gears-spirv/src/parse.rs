use proc_macro2::{Group, Ident, Span};
use quote::{quote, ToTokens};
use regex::Regex;
use syn::{
    parse::{Parse, ParseStream},
    Error, LitInt, Token,
};

pub use shaderc::ShaderKind;

pub fn name_to_kind(name: &str) -> Result<ShaderKind, &'static str> {
    match name {
        "vert" | "vertex" => Ok(ShaderKind::Vertex),
        "frag" | "fragment" => Ok(ShaderKind::Fragment),
        "geom" | "geometry" => Ok(ShaderKind::Geometry),
        _ => Err("Invalid shader source kind"),
    }
}

pub fn kind_to_name(kind: ShaderKind) -> Result<&'static str, &'static str> {
    match kind {
        ShaderKind::Vertex => Ok("Vertex"),
        ShaderKind::Fragment => Ok("Fragment"),
        ShaderKind::Geometry => Ok("Geometry"),
        _ => Err("Invalid shader source kind"),
    }
}

#[derive(Debug)]
pub struct LayoutDef {
    pub location: u32,
    pub binding: u32,
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
    pub ty: FieldType,
    pub name: Ident,
    pub array: Option<Group>,
    pub semicolon: Token![;],
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
    pub layout_token: LayoutToken,
    pub layout_def: LayoutDef,
    pub layout_type: LayoutType,

    pub data: DataField,
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
pub struct Fields {
    pub data: Vec<DataField>,
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
    pub layout_token: LayoutToken,
    pub layout_def: LayoutDef,
    pub layout_type: LayoutType,

    pub struct_name: Ident,
    pub fields: Fields,
    pub instance_name: Ident,
    pub semicolon: Token![;],
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
#[derive(Debug, PartialEq, Eq)]
pub enum FieldType {
    Float,
    Double,
    Vec2,
    Vec3,
    Vec4,
    DVec2,
    DVec3,
    DVec4,
    Mat2,
    Mat3,
    Mat4,
    DMat2,
    DMat3,
    DMat4,
    Int,
    Uint,
}

impl FieldType {
    pub fn to_ident(&self) -> Ident {
        match self {
            &FieldType::Float => Ident::new("f32", Span::call_site()),
            &FieldType::Double => Ident::new("f64", Span::call_site()),
            &FieldType::Vec2 => Ident::new("Vec2", Span::call_site()),
            &FieldType::Vec3 => Ident::new("Vec3", Span::call_site()),
            &FieldType::Vec4 => Ident::new("Vec4", Span::call_site()),
            &FieldType::DVec2 => Ident::new("DVec2", Span::call_site()),
            &FieldType::DVec3 => Ident::new("DVec3", Span::call_site()),
            &FieldType::DVec4 => Ident::new("DVec4", Span::call_site()),
            &FieldType::Mat2 => Ident::new("Mat2", Span::call_site()),
            &FieldType::Mat3 => Ident::new("Mat3", Span::call_site()),
            &FieldType::Mat4 => Ident::new("Mat4", Span::call_site()),
            &FieldType::DMat2 => Ident::new("DMat2", Span::call_site()),
            &FieldType::DMat3 => Ident::new("DMat3", Span::call_site()),
            &FieldType::DMat4 => Ident::new("DMat4", Span::call_site()),
            &FieldType::Int => Ident::new("i32", Span::call_site()),
            &FieldType::Uint => Ident::new("u32", Span::call_site()),
        }
    }
}

impl Parse for FieldType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let token = input.parse::<Ident>()?;
        match token.to_string().as_str() {
            "float" => Ok(Self::Float),
            "double" => Ok(Self::Double),
            "vec2" => Ok(Self::Vec2),
            "vec3" => Ok(Self::Vec3),
            "vec4" => Ok(Self::Vec4),
            "dvec2" => Ok(Self::DVec2),
            "dvec3" => Ok(Self::DVec3),
            "dvec4" => Ok(Self::DVec4),
            "dmat2" => Ok(Self::DMat2),
            "dmat3" => Ok(Self::DMat3),
            "dmat4" => Ok(Self::DMat4),
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
        let res = match self {
            &FieldType::Float => quote! { gears::gears_spirv::parse::FieldType::Float },
            &FieldType::Double => quote! { gears::gears_spirv::parse::FieldType::Double },
            &FieldType::Vec2 => quote! { gears::gears_spirv::parse::FieldType::Vec2 },
            &FieldType::Vec3 => quote! { gears::gears_spirv::parse::FieldType::Vec3 },
            &FieldType::Vec4 => quote! { gears::gears_spirv::parse::FieldType::Vec4 },
            &FieldType::DVec2 => quote! { gears::gears_spirv::parse::FieldType::DVec2 },
            &FieldType::DVec3 => quote! { gears::gears_spirv::parse::FieldType::DVec3 },
            &FieldType::DVec4 => quote! { gears::gears_spirv::parse::FieldType::DVec4 },
            &FieldType::Mat2 => quote! { gears::gears_spirv::parse::FieldType::Mat2 },
            &FieldType::Mat3 => quote! { gears::gears_spirv::parse::FieldType::Mat3 },
            &FieldType::Mat4 => quote! { gears::gears_spirv::parse::FieldType::Mat4 },
            &FieldType::DMat2 => quote! { gears::gears_spirv::parse::FieldType::DMat2 },
            &FieldType::DMat3 => quote! { gears::gears_spirv::parse::FieldType::DMat3 },
            &FieldType::DMat4 => quote! { gears::gears_spirv::parse::FieldType::DMat4 },
            &FieldType::Int => quote! { gears::gears_spirv::parse::FieldType::Int },
            &FieldType::Uint => quote! { gears::gears_spirv::parse::FieldType::Uint },
        };

        tokens.extend(res);
    }
}

#[derive(Debug)]
pub struct Layout {
    pub inputs: Vec<(u32, FieldType)>,
    pub outputs: Vec<(u32, FieldType)>,
    pub uniforms: Vec<(u32, Vec<FieldType>)>,
}

#[derive(Debug)]
pub struct SortedLayout {
    pub inputs: Vec<FieldType>,
    pub outputs: Vec<FieldType>,
    pub uniforms: Vec<Vec<FieldType>>,
}

pub fn get_layout(source: &str) -> SortedLayout {
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
