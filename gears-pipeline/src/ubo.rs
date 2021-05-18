use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};

use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream};
use quote::{ToTokens, TokenStreamExt};
use syn::{ext::IdentExt, parse::ParseStream, Error, Token};

use crate::module::ModuleType;

#[derive(Debug)]
enum BindingLocation {
    Binding(Binding),
    Location(Location),
}

pub struct StructRegistry {
    map: HashMap<String, BindingLocation>,
    latest_binding: Binding,
    latest_location_in: Location,
    latest_location_out: Location,
}

#[derive(Debug)]
pub enum StructFieldType {
    Bool(),
    Int(),
    UInt(),

    Float(),
    Float2(),
    Float3(),
    Float4(),

    Mat2(),
    Mat3(),
    Mat4(),
}

pub struct StructField {
    pub field_name: String,
    pub field_type: StructFieldType,
    pub array: bool,

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

#[derive(Debug)]
pub enum BindgenFieldType {
    Uniform(Option<Binding>),
    In(Option<Location>),
    Out(Option<Location>),
}

#[derive(Debug, Clone, Copy)]
pub struct Binding(u32);

#[derive(Debug, Clone, Copy)]
pub struct Location(u32);

impl StructRegistry {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            latest_binding: Binding(0),
            latest_location_in: Location(0),
            latest_location_out: Location(0),
        }
    }

    pub fn next_module(&mut self) {
        self.latest_binding = Binding(0);
        self.latest_location_in = Location(0);
        self.latest_location_out = Location(0);
    }

    pub fn push_binding(&mut self, name: String) -> Binding {
        let res = self.latest_binding;
        self.map.insert(name, BindingLocation::Binding(res));
        self.latest_binding.0 += 1;
        res
    }

    pub fn push_location_in(&mut self, name: String) -> Location {
        let res = self.latest_location_in;
        self.map.insert(name, BindingLocation::Location(res));
        self.latest_location_in.0 += 1;
        res
    }

    pub fn push_location_out(&mut self, name: String) -> Location {
        let res = self.latest_location_out;
        self.map.insert(name, BindingLocation::Location(res));
        self.latest_location_out.0 += 1;
        res
    }
}

impl StructFieldType {
    pub fn size(&self) -> usize {
        match self {
            Self::Bool() => {
                std::mem::size_of::<i32 /* glsl bool is 32 bits unlike rust's 8 bits */>()
            }
            Self::Int() => std::mem::size_of::<i32>(),
            Self::UInt() => std::mem::size_of::<u32>(),

            Self::Float() => std::mem::size_of::<f32>() * 1,
            Self::Float2() => std::mem::size_of::<f32>() * 2,
            Self::Float3() => std::mem::size_of::<f32>() * 3,
            Self::Float4() => std::mem::size_of::<f32>() * 4,

            Self::Mat2() => std::mem::size_of::<f32>() * 2 * 2,
            Self::Mat3() => std::mem::size_of::<f32>() * 3 * 3,
            Self::Mat4() => std::mem::size_of::<f32>() * 4 * 4,
        }
    }

    pub fn format(&self) -> Ident {
        Ident::new(
            match self {
                Self::Bool() => "R32_UINT",
                Self::Int() => "R32_SINT",
                Self::UInt() => "R32_UINT",

                Self::Float() => "R32_SFLOAT",
                Self::Float2() => "R32G32_SFLOAT",
                Self::Float3() => "R32G32B32_SFLOAT",
                Self::Float4() => "R32G32B32A32_SFLOAT",

                Self::Mat2() => "R32G32_SFLOAT",
                Self::Mat3() => "R32G32B32_SFLOAT",
                Self::Mat4() => "R32G32B32A32_SFLOAT",
            },
            Span::call_site(),
        )
    }

    pub fn format_count(&self) -> usize {
        match self {
            Self::Mat2() => 2,
            Self::Mat3() => 3,
            Self::Mat4() => 4,

            _ => 1,
        }
    }

    pub fn format_offset(&self) -> usize {
        match self {
            Self::Mat2() => std::mem::size_of::<f32>() * 2,
            Self::Mat3() => std::mem::size_of::<f32>() * 3,
            Self::Mat4() => std::mem::size_of::<f32>() * 4,

            _ => self.size(),
        }
    }

    pub fn to_glsl(&self) -> &'static str {
        match self {
            Self::Bool() => "bool",
            Self::Int() => "int",
            Self::UInt() => "uint",

            Self::Float() => "float",
            Self::Float2() => "vec2",
            Self::Float3() => "vec3",
            Self::Float4() => "vec4",

            Self::Mat2() => "mat2",
            Self::Mat3() => "mat3",
            Self::Mat4() => "mat4",
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

            let array = input
                .parse::<Group>()
                .map_or(false, |g| g.delimiter() == Delimiter::Bracket);

            input.parse::<Token![;]>()?;

            let size = field_type.size();
            let offset = next_offset;
            next_offset += size;

            fields.push(StructField {
                field_name,
                field_type,
                array,

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
            "in" => Self::In(None),
            "out" => Self::Out(None),
            "uniform" => Self::Uniform(None),
            _ => panic!("Unknown BindgenFieldType: {}", ident),
        })
    }
}

impl syn::parse::Parse for StructFieldType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let field_type = input.parse::<Ident>()?.to_string();

        Ok(match field_type.as_str() {
            "bool" => StructFieldType::Bool(),
            "int" => StructFieldType::Int(),
            "uint" => StructFieldType::UInt(),

            "float" => StructFieldType::Float(),
            "vec2" => StructFieldType::Float2(),
            "vec3" => StructFieldType::Float3(),
            "vec4" => StructFieldType::Float4(),

            "mat2" => StructFieldType::Mat2(),
            "mat3" => StructFieldType::Mat3(),
            "mat4" => StructFieldType::Mat4(),

            _ => panic!("Currently unsupported field type: {}", field_type),
        })
    }
}

// impl process

impl BindgenStruct {
    pub fn generate(&mut self, reg: &mut StructRegistry) {
        match (&mut self.meta.bind_type, reg.map.get(&self.struct_name)) {
            (BindgenFieldType::Uniform(i), Some(BindingLocation::Binding(new_i))) => {
                *i = Some(*new_i);
            }
            (BindgenFieldType::Uniform(i), None) => {
                let binding = reg.push_binding(self.struct_name.clone());
                *i = Some(binding);
            }
            (BindgenFieldType::In(i), Some(BindingLocation::Location(new_i))) => {
                *i = Some(*new_i);
            }
            (BindgenFieldType::In(i), None) => {
                let binding = reg.push_location_in(self.struct_name.clone());
                *i = Some(binding);
            }
            (BindgenFieldType::Out(i), Some(BindingLocation::Location(new_i))) => {
                *i = Some(*new_i);
            }
            (BindgenFieldType::Out(i), None) => {
                let binding = reg.push_location_out(self.struct_name.clone());
                *i = Some(binding);
            }

            _ => panic!(
                "Gen struct {} expected {:?} but got {:?}",
                self.struct_name,
                self.meta.bind_type,
                reg.map.get(&self.struct_name)
            ),
        };
    }

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
                    i.as_ref().expect("BindgenStruct bindings not generated").0,
                    self.struct_name,
                    fields,
                    self.field_name
                )
            }
            BindgenFieldType::In(l) | BindgenFieldType::Out(l) => {
                let mut first_i = l.as_ref().expect("BindgenStruct locations not generated").0;
                let mut layouts = String::new();
                let is_in = match self.meta.bind_type {
                    BindgenFieldType::In(_) => true,
                    _ => false,
                };

                for field in self.fields.fields.iter() {
                    layouts += format!(
                        "layout(location = {}) {} {} _{}_{}{};",
                        first_i,
                        if is_in { "in" } else { "out" },
                        field.field_type.to_glsl(),
                        self.field_name,
                        field.field_name,
                        if field.array { "[]" } else { "" }
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
            namespacer("vk", &mut impl_tokens);
            impl_tokens.append(Ident::new(
                "VertexInputBindingDescription",
                Span::call_site(),
            ));
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

                    fields.append(Ident::new("input_rate", Span::call_site()));
                    fields.append(Punct::new(':', Spacing::Alone));
                    namespacer("gears_traits", &mut fields);
                    namespacer("vk", &mut fields);
                    namespacer("VertexInputRate", &mut fields);
                    fields.append(Ident::new("VERTEX", Span::call_site()));
                    fields.append(Punct::new(',', Spacing::Alone));

                    fields.append(Ident::new("stride", Span::call_site()));
                    fields.append(Punct::new(':', Spacing::Alone));
                    fields.append(Literal::u32_unsuffixed(self.fields.size as u32));
                    fields.append(Punct::new(',', Spacing::Alone));

                    // VertexInputBindingDescription { ... }
                    let mut contents = TokenStream::new();
                    namespacer("gears_traits", &mut contents);
                    namespacer("vk", &mut contents);
                    contents.append(Ident::new(
                        "VertexInputBindingDescription",
                        Span::call_site(),
                    ));
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
            namespacer("vk", &mut impl_tokens);
            impl_tokens.append(Ident::new(
                "VertexInputAttributeDescription",
                Span::call_site(),
            ));
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

                            fields.append(Ident::new("format", Span::call_site()));
                            fields.append(Punct::new(':', Spacing::Alone));
                            namespacer("gears_traits", &mut fields);
                            namespacer("vk", &mut fields);
                            namespacer("Format", &mut fields);
                            fields.append(field.field_type.format());
                            fields.append(Punct::new(',', Spacing::Alone));

                            fields.append(Ident::new("offset", Span::call_site()));
                            fields.append(Punct::new(':', Spacing::Alone));
                            fields.append(Literal::u32_unsuffixed(
                                (field.offset + i * field.field_type.format_offset()) as u32,
                            ));

                            // VertexInputAttributeDescription { ..., ... }
                            namespacer("gears_traits", &mut contents);
                            namespacer("vk", &mut contents);
                            contents.append(Ident::new(
                                "VertexInputAttributeDescription",
                                Span::call_site(),
                            ));
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
        // impl UBO
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
            namespacer("vk", &mut impl_tokens);
            impl_tokens.append(Ident::new("ShaderStageFlags", Span::call_site()));

            impl_tokens.append(Punct::new('=', Spacing::Alone));

            namespacer("gears_traits", &mut impl_tokens);
            namespacer("vk", &mut impl_tokens);
            namespacer("ShaderStageFlags", &mut impl_tokens);
            impl_tokens.append(Ident::new(
                match self.meta.in_module {
                    ModuleType::Vertex => "VERTEX",
                    ModuleType::Fragment => "FRAGMENT",
                    ModuleType::Geometry => "GEOMETRY",
                },
                Span::call_site(),
            ));

            impl_tokens.append(Punct::new(';', Spacing::Alone));

            impl_tokens
        };
        tokens.append(Group::new(Delimiter::Brace, impl_tokens));

        // impl Default
        tokens.append(Ident::new("impl", Span::call_site()));
        tokens.append(Ident::new("Default", Span::call_site()));
        tokens.append(Ident::new("for", Span::call_site()));
        tokens.append(Ident::new(self.struct_name.as_str(), Span::call_site()));

        let impl_tokens = {
            let empty_tokens = TokenStream::new();
            let mut impl_tokens = TokenStream::new();

            impl_tokens.append(Ident::new("fn", Span::call_site()));
            impl_tokens.append(Ident::new("default", Span::call_site()));
            impl_tokens.append(Group::new(Delimiter::Parenthesis, empty_tokens));

            impl_tokens.append(Punct::new('-', Spacing::Joint));
            impl_tokens.append(Punct::new('>', Spacing::Alone));

            let fn_tokens = {
                let self_tokens = {
                    let mut self_tokens = TokenStream::new();

                    for field in self.fields.fields.iter() {
                        self_tokens
                            .append(Ident::new(field.field_name.as_str(), Span::call_site()));
                        self_tokens.append(Punct::new(':', Spacing::Alone));
                        match &field.field_type {
                            StructFieldType::Bool() => {
                                self_tokens.append(Ident::new("false", Span::call_site()))
                            }
                            StructFieldType::Int() => self_tokens.append(Literal::i32_suffixed(0)),
                            StructFieldType::UInt() => self_tokens.append(Literal::u32_suffixed(0)),
                            StructFieldType::Float() => {
                                self_tokens.append(Literal::f32_suffixed(0.0))
                            }
                            StructFieldType::Float2() => {
                                namespacer("cgmath", &mut self_tokens);
                                namespacer("Vector2", &mut self_tokens);
                                self_tokens.append(Ident::new("new", Span::call_site()));
                                let mut value_tokens = TokenStream::new();
                                value_tokens.append(Literal::f32_suffixed(0.0));
                                value_tokens.append(Punct::new(',', Spacing::Alone));
                                value_tokens.append(Literal::f32_suffixed(0.0));
                                self_tokens
                                    .append(Group::new(Delimiter::Parenthesis, value_tokens));
                            }
                            StructFieldType::Float3() => {
                                namespacer("cgmath", &mut self_tokens);
                                namespacer("Vector3", &mut self_tokens);
                                self_tokens.append(Ident::new("new", Span::call_site()));
                                let mut value_tokens = TokenStream::new();
                                value_tokens.append(Literal::f32_suffixed(0.0));
                                value_tokens.append(Punct::new(',', Spacing::Alone));
                                value_tokens.append(Literal::f32_suffixed(0.0));
                                value_tokens.append(Punct::new(',', Spacing::Alone));
                                value_tokens.append(Literal::f32_suffixed(0.0));
                                self_tokens
                                    .append(Group::new(Delimiter::Parenthesis, value_tokens));
                            }
                            StructFieldType::Float4() => {
                                namespacer("cgmath", &mut self_tokens);
                                namespacer("Vector4", &mut self_tokens);
                                self_tokens.append(Ident::new("new", Span::call_site()));
                                let mut value_tokens = TokenStream::new();
                                value_tokens.append(Literal::f32_suffixed(0.0));
                                value_tokens.append(Punct::new(',', Spacing::Alone));
                                value_tokens.append(Literal::f32_suffixed(0.0));
                                value_tokens.append(Punct::new(',', Spacing::Alone));
                                value_tokens.append(Literal::f32_suffixed(0.0));
                                value_tokens.append(Punct::new(',', Spacing::Alone));
                                value_tokens.append(Literal::f32_suffixed(0.0));
                                self_tokens
                                    .append(Group::new(Delimiter::Parenthesis, value_tokens));
                            }

                            StructFieldType::Mat2() => {
                                namespacer("cgmath", &mut self_tokens);
                                namespacer("Matrix2", &mut self_tokens);
                                self_tokens.append(Ident::new("from_scale", Span::call_site()));
                                let mut value_tokens = TokenStream::new();
                                value_tokens.append(Literal::f32_suffixed(1.0));
                                self_tokens
                                    .append(Group::new(Delimiter::Parenthesis, value_tokens));
                            }
                            StructFieldType::Mat3() => {
                                namespacer("cgmath", &mut self_tokens);
                                namespacer("Matrix3", &mut self_tokens);
                                self_tokens.append(Ident::new("from_scale", Span::call_site()));
                                let mut value_tokens = TokenStream::new();
                                value_tokens.append(Literal::f32_suffixed(1.0));
                                self_tokens
                                    .append(Group::new(Delimiter::Parenthesis, value_tokens));
                            }
                            StructFieldType::Mat4() => {
                                namespacer("cgmath", &mut self_tokens);
                                namespacer("Matrix4", &mut self_tokens);
                                self_tokens.append(Ident::new("from_scale", Span::call_site()));
                                let mut value_tokens = TokenStream::new();
                                value_tokens.append(Literal::f32_suffixed(1.0));
                                self_tokens
                                    .append(Group::new(Delimiter::Parenthesis, value_tokens));
                            }
                        };
                        self_tokens.append(Punct::new(',', Spacing::Alone));
                    }

                    self_tokens
                };

                let mut fn_tokens = TokenStream::new();
                fn_tokens.append(Ident::new("Self", Span::call_site()));
                fn_tokens.append(Group::new(Delimiter::Brace, self_tokens));
                fn_tokens
            };
            impl_tokens.append(Ident::new("Self", Span::call_site()));
            impl_tokens.append(Group::new(Delimiter::Brace, fn_tokens));

            impl_tokens
        };
        tokens.append(Group::new(Delimiter::Brace, impl_tokens));
    }
}

impl ToTokens for BindgenStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append(Punct::new('#', Spacing::Joint));
        let mut derive_tokens = TokenStream::new();
        derive_tokens.append(Ident::new("Debug", Span::call_site()));
        derive_tokens.append(Punct::new(',', Spacing::Alone));
        derive_tokens.append(Ident::new("Copy", Span::call_site()));
        derive_tokens.append(Punct::new(',', Spacing::Alone));
        derive_tokens.append(Ident::new("Clone", Span::call_site()));
        let mut attrib_tokens = TokenStream::new();
        attrib_tokens.append(Ident::new("derive", Span::call_site()));
        attrib_tokens.append(Group::new(Delimiter::Parenthesis, derive_tokens));
        tokens.append(Group::new(Delimiter::Bracket, attrib_tokens));

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
            BindgenFieldType::In(_) | BindgenFieldType::Out(_) => self.in_out_to_tokens(tokens),
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
            StructFieldType::Bool() => tokens.append(Ident::new("bool", Span::call_site())),
            StructFieldType::Int() => tokens.append(Ident::new("i32", Span::call_site())),
            StructFieldType::UInt() => tokens.append(Ident::new("u32", Span::call_site())),
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

            StructFieldType::Mat2() => {
                append_cgmath(tokens);
                tokens.append(Ident::new("Matrix2", Span::call_site()));
                append_f32(tokens);
            }
            StructFieldType::Mat3() => {
                append_cgmath(tokens);
                tokens.append(Ident::new("Matrix3", Span::call_site()));
                append_f32(tokens);
            }
            StructFieldType::Mat4() => {
                append_cgmath(tokens);
                tokens.append(Ident::new("Matrix4", Span::call_site()));
                append_f32(tokens);
            }
        };

        tokens.append(Punct::new(',', Spacing::Alone));
    }
}
