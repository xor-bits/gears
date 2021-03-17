use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream};
use quote::{ToTokens, TokenStreamExt};
use syn::{ext::IdentExt, parse::ParseStream, Error, Token};

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum ModuleType {
    Vertex,
    Fragment,
}

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
    pub field_name: String,
    pub field_type: StructFieldType,

    pub size: usize,
    pub offset: usize,
}

pub struct StructFields {
    pub fields: Vec<StructField>,
    pub size: usize,
}

pub struct BindgenStruct {
    pub field_name: String,
    pub struct_name: String,

    pub fields: StructFields,

    pub meta: BindgenFields,
}

pub struct BindgenFields {
    pub bind: bool,
    pub bind_type: BindgenFieldType,
    pub in_module: ModuleType,
}

pub enum BindgenFieldType {
    Uniform(Binding),
    In,
    Out,
}

pub struct Binding(u32);

impl StructFieldType {
    pub fn size(&self) -> usize {
        match self {
            StructFieldType::Float() => std::mem::size_of::<f32>() * 1,
            StructFieldType::Float2() => std::mem::size_of::<f32>() * 2,
            StructFieldType::Float3() => std::mem::size_of::<f32>() * 3,
            StructFieldType::Float4() => std::mem::size_of::<f32>() * 4,

            StructFieldType::Mat2x2() => std::mem::size_of::<f32>() * 2 * 2,
            StructFieldType::Mat3x3() => std::mem::size_of::<f32>() * 3 * 3,
            StructFieldType::Mat4x4() => std::mem::size_of::<f32>() * 4 * 4,
        }
    }

    pub fn format(&self) -> Ident {
        Ident::new(
            match self {
                StructFieldType::Float() => "R32Sfloat",
                StructFieldType::Float2() => "Rg32Sfloat",
                StructFieldType::Float3() => "Rgb32Sfloat",
                StructFieldType::Float4() => "Rgba32Sfloat",

                StructFieldType::Mat2x2() => "Rg32Sfloat",
                StructFieldType::Mat3x3() => "Rgb32Sfloat",
                StructFieldType::Mat4x4() => "Rgba32Sfloat",
            },
            Span::call_site(),
        )
    }

    pub fn format_count(&self) -> usize {
        match self {
            StructFieldType::Float() => 1,
            StructFieldType::Float2() => 1,
            StructFieldType::Float3() => 1,
            StructFieldType::Float4() => 1,

            StructFieldType::Mat2x2() => 2,
            StructFieldType::Mat3x3() => 3,
            StructFieldType::Mat4x4() => 4,
        }
    }

    pub fn format_offset(&self) -> usize {
        match self {
            StructFieldType::Float() => std::mem::size_of::<f32>() * 1,
            StructFieldType::Float2() => std::mem::size_of::<f32>() * 2,
            StructFieldType::Float3() => std::mem::size_of::<f32>() * 3,
            StructFieldType::Float4() => std::mem::size_of::<f32>() * 4,

            StructFieldType::Mat2x2() => std::mem::size_of::<f32>() * 2,
            StructFieldType::Mat3x3() => std::mem::size_of::<f32>() * 3,
            StructFieldType::Mat4x4() => std::mem::size_of::<f32>() * 4,
        }
    }
}

// impl parse

impl syn::parse::Parse for StructFields {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut fields = Vec::new();
        let mut next_offset = 0;
        while !input.is_empty() {
            let field_type = input.parse::<StructFieldType>()?;
            let field_name = input.parse::<Ident>()?.to_string();

            input.parse::<Token![;]>()?;

            let size = field_type.size();
            let offset = next_offset;
            next_offset += size;

            fields.push(StructField {
                field_name,
                field_type,

                size,
                offset,
            });
        }

        Ok(StructFields {
            fields,
            size: next_offset,
        })
    }
}
impl syn::parse::Parse for BindgenStruct {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut hasher = DefaultHasher::new();
        input.to_string().hash(&mut hasher);
        let hash = hasher.finish();

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
            let struct_name = input
                .parse::<Ident>()
                .map_or_else(|_| format!("STRUCT_{}", hash), |i| i.to_string());
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
                "gears_gen" => false,
                _ => panic!("Unknown BindgenFields: {}", ident),
            },
            bind_type,
            in_module: ModuleType::Vertex,
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
        match &self.meta.bind_type {
            BindgenFieldType::Uniform(i) => {
                let mut fields = String::new();
                for field in self.fields.fields.iter() {
                    fields = format!(
                        "{}{} {};",
                        fields,
                        field.field_type.to_glsl(),
                        field.field_name
                    );
                }

                format!(
                    "layout(binding = {}) uniform {} {{{}}} {};",
                    i.0, self.struct_name, fields, self.field_name
                )
            }
            BindgenFieldType::In | BindgenFieldType::Out => {
                let mut first_i = 0;
                let mut layouts = String::new();
                let is_in = match self.meta.bind_type {
                    BindgenFieldType::In => true,
                    _ => false,
                };

                for field in self.fields.fields.iter() {
                    layouts += format!(
                        "layout(location = {}) {} {} _{}_{};",
                        first_i,
                        if is_in { "in" } else { "out" },
                        field.field_type.to_glsl(),
                        self.field_name,
                        field.field_name
                    )
                    .as_str();
                    first_i += 1;
                }

                layouts
            }
        }
    }

    fn in_out_to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append(Ident::new("impl", Span::call_site()));
        namespacer("gears_traits", tokens);
        tokens.append(Ident::new("Vertex", Span::call_site()));
        tokens.append(Ident::new("for", Span::call_site()));
        tokens.append(Ident::new(self.struct_name.as_str(), Span::call_site()));

        let empty_tokens = TokenStream::new();

        let impl_tokens = {
            // fn binding_desc()
            let mut impl_tokens = TokenStream::new();
            impl_tokens.append(Ident::new("fn", Span::call_site()));
            impl_tokens.append(Ident::new("binding_desc", Span::call_site()));
            impl_tokens.append(Group::new(Delimiter::Parenthesis, empty_tokens.clone()));

            impl_tokens.append(Punct::new('-', Spacing::Joint));
            impl_tokens.append(Punct::new('>', Spacing::Alone));

            impl_tokens.append(Ident::new("Vec", Span::call_site()));
            impl_tokens.append(Punct::new('<', Spacing::Joint));
            namespacer("gears_traits", &mut impl_tokens);
            impl_tokens.append(Ident::new("VertexBufferDesc", Span::call_site()));
            impl_tokens.append(Punct::new('>', Spacing::Alone));

            let binding_desc = {
                // vec!
                let mut binding_desc = TokenStream::new();
                binding_desc.append(Ident::new("vec", Span::call_site()));
                binding_desc.append(Punct::new('!', Spacing::Alone));
                let contents = {
                    // ...
                    let mut fields = TokenStream::new();
                    fields.append(Ident::new("binding", Span::call_site()));
                    fields.append(Punct::new(':', Spacing::Alone));
                    fields.append(Literal::u32_unsuffixed(0));
                    fields.append(Punct::new(',', Spacing::Alone));

                    fields.append(Ident::new("rate", Span::call_site()));
                    fields.append(Punct::new(':', Spacing::Alone));
                    namespacer("gears_traits", &mut fields);
                    namespacer("VertexInputRate", &mut fields);
                    fields.append(Ident::new("Vertex", Span::call_site()));
                    fields.append(Punct::new(',', Spacing::Alone));

                    fields.append(Ident::new("stride", Span::call_site()));
                    fields.append(Punct::new(':', Spacing::Alone));
                    fields.append(Literal::u32_unsuffixed(self.fields.size as u32));
                    fields.append(Punct::new(',', Spacing::Alone));

                    // VertexBufferDesc { ... }
                    let mut contents = TokenStream::new();
                    namespacer("gears_traits", &mut contents);
                    contents.append(Ident::new("VertexBufferDesc", Span::call_site()));
                    contents.append(Group::new(Delimiter::Brace, fields));
                    contents.append(Punct::new(',', Spacing::Alone));
                    contents
                };
                binding_desc.append(Group::new(Delimiter::Bracket, contents));
                binding_desc
            };
            impl_tokens.append(Group::new(Delimiter::Brace, binding_desc));

            // fn attribute_desc()
            impl_tokens.append(Ident::new("fn", Span::call_site()));
            impl_tokens.append(Ident::new("attribute_desc", Span::call_site()));
            impl_tokens.append(Group::new(Delimiter::Parenthesis, empty_tokens.clone()));

            impl_tokens.append(Punct::new('-', Spacing::Joint));
            impl_tokens.append(Punct::new('>', Spacing::Alone));

            impl_tokens.append(Ident::new("Vec", Span::call_site()));
            impl_tokens.append(Punct::new('<', Spacing::Joint));
            namespacer("gears_traits", &mut impl_tokens);
            impl_tokens.append(Ident::new("AttributeDesc", Span::call_site()));
            impl_tokens.append(Punct::new('>', Spacing::Alone));

            let attribute_desc = {
                // vec!
                let mut attribute_desc = TokenStream::new();
                attribute_desc.append(Ident::new("vec", Span::call_site()));
                attribute_desc.append(Punct::new('!', Spacing::Alone));
                let contents = {
                    let mut contents = TokenStream::new();
                    for (index, field) in self.fields.fields.iter().enumerate() {
                        for i in 0..field.field_type.format_count() {
                            // ...
                            let mut fields = TokenStream::new();
                            fields.append(Ident::new("binding", Span::call_site()));
                            fields.append(Punct::new(':', Spacing::Alone));
                            fields.append(Literal::u32_unsuffixed(0));
                            fields.append(Punct::new(',', Spacing::Alone));

                            fields.append(Ident::new("location", Span::call_site()));
                            fields.append(Punct::new(':', Spacing::Alone));
                            fields.append(Literal::u32_unsuffixed(index as u32));
                            fields.append(Punct::new(',', Spacing::Alone));

                            fields.append(Ident::new("element", Span::call_site()));
                            fields.append(Punct::new(':', Spacing::Alone));
                            namespacer("gears_traits", &mut fields);
                            fields.append(Ident::new("Element", Span::call_site()));
                            let element = {
                                let mut element = TokenStream::new();
                                element.append(Ident::new("format", Span::call_site()));
                                element.append(Punct::new(':', Spacing::Alone));
                                namespacer("gears_traits", &mut element);
                                namespacer("Format", &mut element);
                                element.append(field.field_type.format());
                                element.append(Punct::new(',', Spacing::Alone));

                                element.append(Ident::new("offset", Span::call_site()));
                                element.append(Punct::new(':', Spacing::Alone));
                                element.append(Literal::u32_unsuffixed(
                                    (field.offset + i * field.field_type.format_offset()) as u32,
                                ));
                                element
                            };
                            fields.append(Group::new(Delimiter::Brace, element));

                            // AttributeDesc { ..., ... }
                            namespacer("gears_traits", &mut contents);
                            contents.append(Ident::new("AttributeDesc", Span::call_site()));
                            contents.append(Group::new(Delimiter::Brace, fields));
                            contents.append(Punct::new(',', Spacing::Alone));
                        }
                    }
                    contents
                };
                attribute_desc.append(Group::new(Delimiter::Bracket, contents));
                attribute_desc
            };
            impl_tokens.append(Group::new(Delimiter::Brace, attribute_desc));

            impl_tokens
        };

        tokens.append(Group::new(Delimiter::Brace, impl_tokens));
    }

    fn uniform_to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append(Ident::new("impl", Span::call_site()));
        namespacer("gears_traits", tokens);
        tokens.append(Ident::new("UBO", Span::call_site()));
        tokens.append(Ident::new("for", Span::call_site()));
        tokens.append(Ident::new(self.struct_name.as_str(), Span::call_site()));

        let impl_tokens = {
            let mut impl_tokens = TokenStream::new();
            impl_tokens.append(Ident::new("const", Span::call_site()));
            impl_tokens.append(Ident::new("STAGE", Span::call_site()));
            impl_tokens.append(Punct::new(':', Spacing::Alone));

            namespacer("gears_traits", &mut impl_tokens);
            impl_tokens.append(Ident::new("ShaderStageFlags", Span::call_site()));

            impl_tokens.append(Punct::new('=', Spacing::Alone));

            namespacer("gears_traits", &mut impl_tokens);
            namespacer("ShaderStageFlags", &mut impl_tokens);
            impl_tokens.append(Ident::new(
                match self.meta.in_module {
                    ModuleType::Vertex => "VERTEX",
                    ModuleType::Fragment => "FRAGMENT",
                },
                Span::call_site(),
            ));

            impl_tokens.append(Punct::new(';', Spacing::Alone));

            impl_tokens
        };
        tokens.append(Group::new(Delimiter::Brace, impl_tokens));
    }
}

impl StructFieldType {
    pub fn to_glsl(&self) -> &'static str {
        match self {
            StructFieldType::Float() => "float",
            StructFieldType::Float2() => "vec2",
            StructFieldType::Float3() => "vec3",
            StructFieldType::Float4() => "vec4",

            StructFieldType::Mat2x2() => "mat2",
            StructFieldType::Mat3x3() => "mat3",
            StructFieldType::Mat4x4() => "mat4",
        }
    }
}

impl ToTokens for BindgenStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append(Ident::new("pub", Span::call_site()));
        tokens.append(Ident::new("struct", Span::call_site()));

        tokens.append(Ident::new(self.struct_name.as_str(), Span::call_site()));

        let mut struct_tokens = TokenStream::new();
        for field in self.fields.fields.iter() {
            field.to_tokens(&mut struct_tokens);
        }
        tokens.append(Group::new(Delimiter::Brace, struct_tokens));

        // impls

        match &self.meta.bind_type {
            BindgenFieldType::Uniform(_) => self.uniform_to_tokens(tokens),
            BindgenFieldType::In | BindgenFieldType::Out => self.in_out_to_tokens(tokens),
        }
    }
}

fn namespacer(namespace: &'static str, tokens: &mut TokenStream) {
    tokens.append(Ident::new(namespace, Span::call_site()));
    tokens.append(Punct::new(':', Spacing::Joint));
    tokens.append(Punct::new(':', Spacing::Joint));
}

impl ToTokens for StructField {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append(Ident::new("pub", Span::call_site()));
        tokens.append(Ident::new(self.field_name.as_str(), Span::call_site()));
        tokens.append(Punct::new(':', Spacing::Alone));

        let append_cgmath = |tokens: &mut TokenStream| {
            tokens.append(Ident::new("cgmath", Span::call_site()));
            tokens.append(Punct::new(':', Spacing::Joint));
            tokens.append(Punct::new(':', Spacing::Joint));
        };
        let append_f32 = |tokens: &mut TokenStream| {
            tokens.append(Punct::new('<', Spacing::Joint));
            tokens.append(Ident::new("f32", Span::call_site()));
            tokens.append(Punct::new('>', Spacing::Alone));
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
                tokens.append(Ident::new("Matrix2", Span::call_site()));
                append_f32(tokens);
            }
            StructFieldType::Mat3x3() => {
                append_cgmath(tokens);
                tokens.append(Ident::new("Matrix3", Span::call_site()));
                append_f32(tokens);
            }
            StructFieldType::Mat4x4() => {
                append_cgmath(tokens);
                tokens.append(Ident::new("Matrix4", Span::call_site()));
                append_f32(tokens);
            }
        };

        tokens.append(Punct::new(',', Spacing::Alone));
    }
}
