use proc_macro::TokenStream;
use proc_macro2::{Group, Ident, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Token,
};

#[derive(Debug)]
struct SingleModuleInput {
    name: Ident,
    _colon_token: Token![:],
    others: Group,
}

#[derive(Debug)]
struct ModuleInput {
    inputs: Vec<SingleModuleInput>,
}

impl Parse for SingleModuleInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            name: input.parse()?,
            _colon_token: input.parse()?,
            others: input.parse()?,
        })
    }
}

impl Parse for ModuleInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut inputs = Vec::new();
        while !input.is_empty() {
            inputs.push(input.parse()?);
            let _ = input.parse::<Token![,]>();
        }

        Ok(Self { inputs })
    }
}

pub fn modules(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ModuleInput);

    let mut modules = TokenStream2::new();

    for input in input.inputs {
        let name = input.name;
        let tokens = input.others.stream();

        modules = quote! {
            #modules
            pub mod #name {
                vulkano_shaders::shader! {
                    #tokens
                }
            }
        };
    }

    modules.into()
}
