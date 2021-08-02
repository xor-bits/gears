pub mod dense;
pub mod sparse;

#[cfg(feature = "short_namespaces")]
pub use dense::*;
#[cfg(feature = "short_namespaces")]
pub use sparse::*;
