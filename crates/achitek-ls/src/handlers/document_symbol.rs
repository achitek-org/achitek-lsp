//! Handler for the LSP `textDocument/documentSymbol` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_documentSymbol>
//!
//! Clients send this request when they need an outline of a single open
//! document. Editors commonly use the response to power outline panes,
//! breadcrumb navigation, and quick symbol pickers scoped to the current file.
//!
//! For Achitekfiles, this handler returns nested symbols for language
//! structures such as the top-level `blueprint` block, `prompt` blocks, and
//! nested `validate` blocks.
use crate::{editor, server::ServerState};
use anyhow::Context;
use lsp_types::{
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, Position, Range,
    SymbolKind as LspSymbolKind,
};

/// Handles a `textDocument/documentSymbol` request.
///
/// The request URI is used to find the current in-memory document. If the
/// document is known, the handler analyzes its text and converts Achitek
/// symbols into LSP document symbols. If the document is unknown, the handler
/// returns `null`, which tells the client there are no symbols available for
/// that request.
pub fn handle(
    state: &ServerState,
    params: DocumentSymbolParams,
) -> anyhow::Result<Option<DocumentSymbolResponse>> {
    if let Some(document) = state.documents.get(params.text_document.uri.as_str()) {
        let editor_buffer = editor::from_source(&document.text).with_context(|| {
            format!(
                "failed to analyze document `{:?}`",
                params.text_document.uri
            )
        })?;
        let symbols = editor_buffer
            .symbols()
            .iter()
            .map(to_lsp_document_symbol)
            .collect::<Vec<_>>();

        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    } else {
        Ok(None)
    }
}

/// Converts an editor symbol into the nested LSP document-symbol shape.
#[allow(deprecated)]
fn to_lsp_document_symbol(symbol: &editor::Symbol) -> DocumentSymbol {
    DocumentSymbol {
        name: symbol.name().to_owned(),
        detail: symbol.detail().map(str::to_owned),
        kind: match symbol.kind() {
            editor::SymbolKind::Blueprint => LspSymbolKind::MODULE,
            editor::SymbolKind::Prompt => LspSymbolKind::FIELD,
            editor::SymbolKind::Validate => LspSymbolKind::OBJECT,
        },
        tags: None,
        deprecated: None,
        range: to_lsp_range(symbol.range()),
        selection_range: to_lsp_range(symbol.selection_range()),
        children: Some(
            symbol
                .children()
                .iter()
                .map(to_lsp_document_symbol)
                .collect(),
        ),
    }
}

/// Converts an editor text range into an LSP range.
fn to_lsp_range(range: achitekfile::TextRange) -> Range {
    Range {
        start: to_lsp_position(range.start),
        end: to_lsp_position(range.end),
    }
}

/// Converts a zero-based editor text position into an LSP position.
fn to_lsp_position(position: achitekfile::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.line).expect("line should fit into u32"),
        character: u32::try_from(position.byte).expect("column should fit into u32"),
    }
}
