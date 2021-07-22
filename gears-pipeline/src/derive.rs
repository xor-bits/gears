use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{Data, DeriveInput, Fields};

fn parse_ast(ast: DeriveInput) -> (Ident, Group, Vec<TokenStream>) {
    let name = ast.ident;
    let data = match ast.data {
        Data::Struct(s) => s,
        _ => panic!("Union or enum inputs are not allowed."),
    };
    let fields = match data.fields {
        Fields::Named(f) => f,
        _ => panic!("Unnamed fields or unit struct are not allowed"),
    };

    let mut token_fields = Vec::new();
    let mut tuple = TokenStream::new();
    for field in fields.named.iter() {
        field.ty.to_tokens(&mut tuple);
        tuple.append(Punct::new(',', Spacing::Alone));

        let mut token_field = TokenStream::new();
        field.ty.to_tokens(&mut token_field);
        token_fields.push(token_field);
    }
    let tuple = Group::new(Delimiter::Parenthesis, tuple);

    (name, tuple, token_fields)
}

pub fn impl_trait_input(ast: DeriveInput) -> TokenStream {
    let (name, tuple, token_fields) = parse_ast(ast);

    let mut attributes = TokenStream::new();
    let mut last_field = quote! { (0) };
    for (i, field) in token_fields.into_iter().enumerate() {
        let i = i as u32;
        attributes.extend(quote! {
            gears::vk::VertexInputAttributeDescription {
                binding: 0,
                location: #i,
                offset: #last_field,
                format: <#field as FormatOf>::FORMAT_OF,
            },
        });
        last_field = quote! {
            (#last_field + <#field as FormatOf>::OFFSET_OF)
        };
    }

    (quote! {
        impl Input for #name {
            type FIELDS = #tuple;
            const BINDING_DESCRIPTION: &'static [gears::vk::VertexInputBindingDescription] = &[
                gears::vk::VertexInputBindingDescription {
                    binding: 0,
                    stride: std::mem::size_of::<#name>() as u32,
                    input_rate: gears::vk::VertexInputRate::VERTEX,
                }
            ];
            const ATTRIBUTE_DESCRIPTION: &'static [gears::vk::VertexInputAttributeDescription] = &[
                #attributes
            ];
        }
    })
    .into()
}

pub fn impl_trait_uniform(ast: DeriveInput) -> TokenStream {
    let (name, tuple, _) = parse_ast(ast);

    (quote! {
        impl Uniform for #name {
            type FIELDS = #tuple;
        }
    })
    .into()
}
