//! Tree sitter backed semantic parser for .tera files

#![deny(missing_docs)]

mod analysis;
mod diagnostics;
mod parser;
mod tree_sitter_tera;

pub use parser::{ParseError, parse};
