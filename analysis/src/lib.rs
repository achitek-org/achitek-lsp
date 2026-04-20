//! Semantic-facing analysis for Achitek documents.
//!
//! This crate sits between the LSP server and the low-level syntax layer. Its
//! job is to turn parsed source into editor-friendly results such as
//! diagnostics, symbols, definitions, and other language features over time.
//!
//! The first implementation is intentionally small: it delegates parsing to the
//! `syntax` crate and lifts syntax issues into a crate-local diagnostics model
//! that higher layers can consume without depending on Tree-sitter details.

#![deny(missing_docs)]

mod analyzer;

pub use analyzer::{
    Analysis, Completion, CompletionKind, DefinitionTarget, Diagnostic, Hover, PrepareRenameTarget,
    ReferenceTarget, Severity, Symbol, SymbolKind, analyze,
};

// TODO:
// 1. type should be required in prompt block
