use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod derive;
mod modules;
mod pipeline;

/// ## `shader!` macro
/// WIP easier to use AIO shader macro
#[proc_macro]
pub fn shader(_input: TokenStream) -> TokenStream {
    todo!("simple macro that combines pipeline! and module!")
}

/// ## `pipeline!` macro
#[proc_macro]
pub fn pipeline(input: TokenStream) -> TokenStream {
    pipeline::pipeline(input)
}

/// ## `modules!` macro
#[proc_macro]
pub fn modules(input: TokenStream) -> TokenStream {
    modules::modules(input)
}

/// ## Input derive macro
#[proc_macro_derive(Input)]
pub fn derive_input(input: TokenStream) -> TokenStream {
    derive::impl_trait_input(parse_macro_input!(input as DeriveInput)).into()
}

/// ## Output derive macro
/// WIP
#[proc_macro_derive(Output)]
pub fn derive_output(_input: TokenStream) -> TokenStream {
    todo!()
}

/// ## Uniform derive macro
#[proc_macro_derive(Uniform)]
pub fn derive_uniform(_input: TokenStream) -> TokenStream {
    todo!()
}
