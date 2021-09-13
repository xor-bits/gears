use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Data, DeriveInput, Fields};

fn parse_ast(ast: DeriveInput) -> (Ident, Vec<Ident>) {
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
    for field in fields.named.into_iter() {
        token_fields.push(field.ident.expect("Unnamed fields are not allowed"))
    }

    (name, token_fields)
}

pub fn impl_trait_input(ast: DeriveInput) -> TokenStream {
    let (name, token_fields) = parse_ast(ast);

    (quote! {
        gears::vulkano::impl_vertex! { #name, #( #token_fields ),*  }
    })
    .into()
}
