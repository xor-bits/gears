use derive::impl_trait_input;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod compiler;
mod derive;
mod module;

use derive::impl_trait_uniform;

#[proc_macro]
pub fn module(input: TokenStream) -> TokenStream {
    module::module(input)
}

/// ```
/// # use gears::{ModuleInput, Vec2, Input};
/// # use gears_pipeline::Input;
/// # use static_assertions::assert_type_eq_all;
/// #[derive(Input)]
/// pub struct VertexData {
///     light: f32,
///     dir: Vec2,
///     active: i32,
/// }
///
/// assert_type_eq_all!(<VertexData as Input>::FIELDS, (f32, Vec2, i32));
/// ```
/// ```
/// # use gears::{ModuleInput, Vec2, Input};
/// # use gears_pipeline::Input;
/// # use static_assertions::assert_type_eq_all;
/// #[derive(Input)]
/// pub struct VertexData {}
///
/// assert_type_eq_all!(<VertexData as Input>::FIELDS, ());
/// ```
#[proc_macro_derive(Input)]
pub fn derive_input(input: TokenStream) -> TokenStream {
    impl_trait_input(parse_macro_input!(input as DeriveInput)).into()
}

/// ```
/// # use gears::{ModuleInput, Vec2, Uniform};
/// # use gears_pipeline::Uniform;
/// # use static_assertions::assert_type_eq_all;
/// #[derive(Uniform)]
/// pub struct UniformData {
///     light: f32,
///     dir: Vec2,
///     active: i32,
/// }
///
/// assert_type_eq_all!(<UniformData as Uniform>::FIELDS, (f32, Vec2, i32));
/// ```
/// ```
/// # use gears::{ModuleInput, Vec2, Uniform};
/// # use gears_pipeline::Uniform;
/// # use static_assertions::assert_type_eq_all;
/// #[derive(Uniform)]
/// pub struct UniformData {}
///
/// assert_type_eq_all!(<UniformData as Uniform>::FIELDS, ());
/// ```
#[proc_macro_derive(Uniform)]
pub fn derive_uniform(input: TokenStream) -> TokenStream {
    impl_trait_uniform(parse_macro_input!(input as DeriveInput)).into()
}
