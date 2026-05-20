//! Handler for the LSP `textDocument/foldingRange` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_foldingRange>
//!
//! Clients send this request when they need the foldable regions for a single
//! document. Editors use the response to draw folding controls in the gutter
//! and to support commands such as folding the current block or all foldable
//! blocks in a file.
//!
//! For Achitekfiles, folding ranges are derived from editor document symbols.
//! Multi-line `blueprint`, `prompt`, and nested `validate` symbols become
//! foldable ranges, with the symbol name used as collapsed text when supported
//! by the client.

use crate::{editor, server::ServerState};
use anyhow::Context;
use lsp_types::{FoldingRange, FoldingRangeParams};

/// Handles a `textDocument/foldingRange` request.
///
/// The request URI is used to find the current in-memory document. If the
/// document is known, the handler analyzes its text and returns foldable ranges
/// for multi-line Achitek symbols. If the document is unknown, the handler
/// returns `null`.
pub fn handle(
    state: &ServerState,
    params: FoldingRangeParams,
) -> anyhow::Result<Option<Vec<FoldingRange>>> {
    if let Some(document) = state.documents.get(params.text_document.uri.as_str()) {
        let editor_buffer = editor::from_source(&document.text).with_context(|| {
            format!(
                "failed to analyze document `{:?}`",
                params.text_document.uri
            )
        })?;
        let mut ranges = Vec::new();

        for symbol in editor_buffer.symbols() {
            collect_folding_ranges(symbol, &mut ranges);
        }

        Ok(Some(ranges))
    } else {
        Ok(None)
    }
}

/// Collects foldable ranges from an editor symbol and its children.
fn collect_folding_ranges(symbol: &editor::Symbol, ranges: &mut Vec<FoldingRange>) {
    let range = symbol.range();

    if range.start.line < range.end.line {
        ranges.push(FoldingRange {
            start_line: u32::try_from(range.start.line).expect("line should fit into u32"),
            start_character: Some(
                u32::try_from(range.start.byte).expect("column should fit into u32"),
            ),
            end_line: u32::try_from(range.end.line).expect("line should fit into u32"),
            end_character: Some(u32::try_from(range.end.byte).expect("column should fit into u32")),
            kind: None,
            collapsed_text: Some(symbol.name().to_owned()),
        });
    }

    for child in symbol.children() {
        collect_folding_ranges(child, ranges);
    }
}
