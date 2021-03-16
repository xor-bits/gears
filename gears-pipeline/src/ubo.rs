use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span};
use quote::{ToTokens, TokenStreamExt};
use syn::{ext::IdentExt, parse::ParseStream, Error, Token};

#[derive(Debug)]
pub enum StructFieldType {
    Float(),
    Float2(),
    Float3(),
    Float4(),

    Mat2x2(),
    Mat3x3(),
    Mat4x4(),
}

pub struct StructField {
    field_name: String,
    field_type: StructFieldType,
}

pub struct StructFields {
    fields: Vec<StructField>,
}

pub struct BindgenStruct {
    field_name: String,
    struct_name: String,
    fields: StructFields,

    meta: BindgenFields,
}

pub struct BindgenFields {
    bind: bool,
    bind_type: BindgenFieldType,
}

pub enum BindgenFieldType {
    Uniform(Binding),
    In,
    Out,
}

pub struct Binding(u32);

// impl parse

impl syn::parse::Parse for StructFields {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut fields = Vec::new();
        while !input.is_empty() {
            let field_type = input.parse::<StructFieldType>()?;
            let field_name = input.parse::<Ident>()?.to_string();

            input.parse::<Token![;]>()?;

            fields.push(StructField {
                field_name,
                field_type,
            });
        }

        Ok(StructFields { fields })
    }
}

impl syn::parse::Parse for BindgenStruct {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![#]>()?;
        let group: Group = input.parse()?;
        let meta = syn::parse::<BindgenFields>(group.stream().into())?;

        /* let ident = input // this one fails for some really odd reason even if the next token is ident
        .parse::<Ident>()?; */
        let ident = input.call(Ident::parse_any)?; // this one doesnt do that
        let span = ident.span();
        let string = ident.to_string();

        if string != "struct" {
            Err(Error::new(
                span,
                format!("expected identifier 'struct', found '{}'", string),
            ))
        } else {
            let struct_name = input.parse::<Ident>()?.to_string();
            let group: Group = input.parse()?;
            let fields = syn::parse::<StructFields>(group.stream().into())?;
            let field_name = input.parse::<Ident>()?.to_string();

            input.parse::<Token![;]>()?;

            Ok(BindgenStruct {
                struct_name,
                field_name,
                fields,
                meta,
            })
        }
    }
}

impl syn::parse::Parse for BindgenFields {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<Ident>()?.to_string();
        let group: Group = input.parse()?;
        let bind_type = syn::parse::<BindgenFieldType>(group.stream().into())?;

        Ok(Self {
            bind: match ident.as_str() {
                "gears_bindgen" => true,
                "gears_gen" => true,
                _ => panic!("Unknown BindgenFields: {}", ident),
            },
            bind_type,
        })
    }
}

impl syn::parse::Parse for BindgenFieldType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.call(Ident::parse_any)?.to_string();
        Ok(match ident.as_str() {
            "in" => Self::In,
            "out" => Self::Out,
            "uniform" => Self::Uniform(syn::parse::<Binding>(
                input.parse::<Group>()?.stream().into(),
            )?),
            _ => panic!("Unknown BindgenFieldType: {}", ident),
        })
    }
}

impl syn::parse::Parse for Binding {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<Ident>()?;
        let string = ident.to_string();
        if string != "binding" {
            Err(Error::new(
                ident.span(),
                format!("expected identifier 'binding', found '{}'", string),
            ))
        } else {
            input.parse::<Token![=]>()?;

            let index_lit = input.parse::<Literal>()?;
            let index = index_lit.to_string().parse::<u32>().or(Err(Error::new(
                index_lit.span(),
                format!("Could not parse '{}' as u32", index_lit.to_string()),
            )))?;

            Ok(Binding(index))
        }
    }
}

impl syn::parse::Parse for StructFieldType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let field_type = input.parse::<Ident>()?.to_string();

        Ok(match field_type.as_str() {
            "float" => StructFieldType::Float(),
            "vec2" => StructFieldType::Float2(),
            "vec3" => StructFieldType::Float3(),
            "vec4" => StructFieldType::Float4(),

            "mat2" => StructFieldType::Mat2x2(),
            "mat3" => StructFieldType::Mat3x3(),
            "mat4" => StructFieldType::Mat4x4(),

            _ => panic!("Currently unsupported field type: {}", field_type),
        })
    }
}

// impl process

impl BindgenStruct {
    pub fn to_glsl(&self) -> String {
        let mut fields = String::new();

        for StructField {
            field_name,
            field_type,
        } in self.fields.fields.iter()
        {
            fields = format!(
                "{}{} {};",
                fields,
                match field_type {
                    StructFieldType::Float() => "float",
                    StructFieldType::Float2() => "vec2",
                    StructFieldType::Float3() => "vec3",
                    StructFieldType::Float4() => "vec4",

                    StructFieldType::Mat2x2() => "mat2",
                    StructFieldType::Mat3x3() => "mat3",
                    StructFieldType::Mat4x4() => "mat4",
                },
                field_name,
            );
        }

        format!(
            "layout(binding = {}) {} {} {{{}}} {};",
            match &self.meta.bind_type {
                BindgenFieldType::Uniform(i) => i.0,
                _ => 0,
            },
            if self.meta.bind { "uniform" } else { "struct" },
            self.struct_name,
            fields,
            self.field_name
        )
    }

    pub fn bound(&self) -> bool {
        self.meta.bind
    }
}

impl ToTokens for BindgenStruct {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.append(Ident::new("pub", Span::call_site()));
        tokens.append(Ident::new("struct", Span::call_site()));

        tokens.append(Ident::new(self.struct_name.as_str(), Span::call_site()));

        let mut struct_tokens = proc_macro2::TokenStream::new();
        for field in self.fields.fields.iter() {
            field.to_tokens(&mut struct_tokens);
        }
        tokens.append(Group::new(Delimiter::Brace, struct_tokens));
    }
}

impl ToTokens for StructField {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.append(Ident::new("pub", Span::call_site()));
        tokens.append(Ident::new(self.field_name.as_str(), Span::call_site()));
        tokens.append(Punct::new(':', Spacing::Alone));

        let append_cgmath = |tokens: &mut proc_macro2::TokenStream| {
            tokens.append(Ident::new("cgmath", Span::call_site()));
            tokens.append(Punct::new(':', Spacing::Joint));
            tokens.append(Punct::new(':', Spacing::Joint));
        };
        let append_f32 = |tokens: &mut proc_macro2::TokenStream| {
            tokens.append(Punct::new('<', Spacing::Joint));
            tokens.append(Ident::new("f32", Span::call_site()));
            tokens.append(Punct::new('>', Spacing::Joint));
        };

        match self.field_type {
            StructFieldType::Float() => tokens.append(Ident::new("f32", Span::call_site())),
            StructFieldType::Float2() => {
                append_cgmath(tokens);
                tokens.append(Ident::new("Vector2", Span::call_site()));
                append_f32(tokens);
            }
            StructFieldType::Float3() => {
                append_cgmath(tokens);
                tokens.append(Ident::new("Vector3", Span::call_site()));
                append_f32(tokens);
            }
            StructFieldType::Float4() => {
                append_cgmath(tokens);
                tokens.append(Ident::new("Vector4", Span::call_site()));
                append_f32(tokens);
            }

            StructFieldType::Mat2x2() => {
                append_cgmath(tokens);
                tokens.append(Ident::new("Matrix2x2", Span::call_site()));
                append_f32(tokens);
            }
            StructFieldType::Mat3x3() => {
                append_cgmath(tokens);
                tokens.append(Ident::new("Matrix3x3", Span::call_site()));
                append_f32(tokens);
            }
            StructFieldType::Mat4x4() => {
                append_cgmath(tokens);
                tokens.append(Ident::new("Matrix4x4", Span::call_site()));
                append_f32(tokens);
            }
        };

        tokens.append(Punct::new(',', Spacing::Alone));
    }
}
