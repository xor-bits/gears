use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span};
use quote::{ToTokens, TokenStreamExt};
use syn::{parse::ParseStream, Token};

#[derive(Debug)]
pub enum UBOFieldType {
    Float(),
    Float2(),
    Float3(),
    Float4(),

    Mat2x2(),
    Mat3x3(),
    Mat4x4(),
}

pub struct UBOField {
    field_name: String,
    field_type: UBOFieldType,
}

pub struct UBOFields {
    fields: Vec<UBOField>,
}

pub struct UBOStruct {
    field_name: String,
    struct_name: String,
    fields: UBOFields,
}

// impl parse

impl syn::parse::Parse for UBOStruct {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let field_name = input.parse::<Ident>()?;
        let field_name = field_name.to_string();

        input.parse::<Token![:]>()?;

        let struct_name = input.parse::<Ident>()?;
        let struct_name = struct_name.to_string();

        let group = input.parse::<Group>()?;
        let group_tokens: TokenStream = group.stream().into();
        let fields = syn::parse::<UBOFields>(group_tokens)?;

        Ok(UBOStruct {
            field_name,
            struct_name,
            fields,
        })
    }
}

impl syn::parse::Parse for UBOFields {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut fields = Vec::new();
        while !input.is_empty() {
            let field_name = input.parse::<Ident>()?.to_string();

            input.parse::<Token![:]>()?;

            let field_type = input.parse::<UBOFieldType>()?;

            fields.push(UBOField {
                field_name,
                field_type,
            });
        }

        Ok(UBOFields { fields })
    }
}

impl syn::parse::Parse for UBOFieldType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let field_type = input.parse::<Ident>()?.to_string();

        Ok(match field_type.as_str() {
            "float" | "f32" => UBOFieldType::Float(),
            "vec2" | "Vector2" => UBOFieldType::Float2(),
            "vec3" | "Vector3" => UBOFieldType::Float3(),
            "vec4" | "Vector4" => UBOFieldType::Float4(),

            "mat2" | "Matrix2x2" => UBOFieldType::Mat2x2(),
            "mat3" | "Matrix3x3" => UBOFieldType::Mat3x3(),
            "mat4" | "Matrix4x4" => UBOFieldType::Mat4x4(),

            _ => panic!("Currently unsupported field type: {}", field_type),
        })
    }
}

// impl process

impl UBOStruct {
    pub fn to_glsl(&self) -> String {
        let mut fields = String::new();

        for UBOField {
            field_name,
            field_type,
        } in self.fields.fields.iter()
        {
            fields = format!(
                "{}{} {};",
                fields,
                match field_type {
                    UBOFieldType::Float() => "float",
                    UBOFieldType::Float2() => "vec2",
                    UBOFieldType::Float3() => "vec3",
                    UBOFieldType::Float4() => "vec4",

                    UBOFieldType::Mat2x2() => "mat2",
                    UBOFieldType::Mat3x3() => "mat3",
                    UBOFieldType::Mat4x4() => "mat4",
                },
                field_name,
            );
        }

        format!(
            "uniform {} {{{}}} {}",
            self.struct_name, fields, self.field_name
        )
    }
}

impl ToTokens for UBOStruct {
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

impl ToTokens for UBOField {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.append(Ident::new("pub", Span::call_site()));
        tokens.append(Ident::new(self.field_name.as_str(), Span::call_site()));
        tokens.append(Punct::new(':', Spacing::Alone));
        tokens.append(Ident::new(
            match self.field_type {
                UBOFieldType::Float() => "f32",
                UBOFieldType::Float2() => "Vector2",
                UBOFieldType::Float3() => "Vector3",
                UBOFieldType::Float4() => "Vector4",

                UBOFieldType::Mat2x2() => "Matrix2x2",
                UBOFieldType::Mat3x3() => "Matrix3x3",
                UBOFieldType::Mat4x4() => "Matrix4x4",
            },
            Span::call_site(),
        ));
        tokens.append(Punct::new(',', Spacing::Alone));
    }
}
