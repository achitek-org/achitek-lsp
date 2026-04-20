//! Syntax support for Achitek source files.
//!
//! This crate is the parsing boundary for the workspace. It is responsible for
//! turning raw source text into a tree that higher layers can inspect without
//! knowing about LSP transport details.
//!
//! # Responsibilities
//!
//! `syntax` should own:
//! - configuring and invoking Tree-sitter with the Achitek grammar
//! - building the initial concrete syntax tree (CST)
//! - exposing parsing entry points and syntax-facing data structures
//! - reporting syntax-level errors such as malformed or incomplete input
//! - source-position and range helpers needed by later analysis layers
//!
//! # Non-responsibilities
//!
//! This crate should not own:
//! - LSP request handling or editor protocol concerns
//! - semantic analysis such as symbol resolution or definitions
//! - workspace-wide project state
//!
//! Those concerns belong in the `server` and `analysis` workspace members.
//!
//! # Layering
//!
//! The intended dependency direction in this workspace is:
//!
//! `server -> analysis -> syntax -> tree-sitter-achitekfile`
//!
//! Keeping parsing isolated in this crate makes it easier to test the language
//! layer independently and prevents protocol concerns from leaking into syntax
//! code.
//!
//! # Design Notes
//!
//! Tree-sitter provides a concrete syntax tree and low-level node APIs. This
//! crate can wrap those primitives in project-specific types that are easier
//! for the rest of the workspace to consume. Over time, this crate will likely
//! grow types such as:
//! - a syntax tree wrapper
//! - parse results and syntax diagnostics
//! - typed node helpers or CST adapters
//! - source text and offset conversion utilities
//!
//! The public entry point today is [`parse`], which will evolve into the main
//! constructor for syntax data used by `analysis`.

#![deny(missing_docs)]

mod parser;

pub use parser::{
    ParseError, SyntaxError, SyntaxErrorKind, SyntaxTree, TextPosition, TextRange, parse,
};
