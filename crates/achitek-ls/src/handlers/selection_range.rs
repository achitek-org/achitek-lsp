//! Handler for the LSP `textDocument/selectionRange` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_selectionRange>
//!
//! Clients send this request to expand a cursor position into progressively
//! larger source ranges. Editors use the response for "expand selection" and
//! similar structural-selection commands.
//!
//! For Achitekfiles, selection ranges are built from analyzed symbols. A cursor
//! inside a prompt name can expand from the prompt name, to the whole prompt
//! block, and then to larger containing symbols when available.

use crate::{editor, server::ServerState};
use anyhow::Context;
use lsp_types::{Position, Range, SelectionRange, SelectionRangeParams};

/// Handles a `textDocument/selectionRange` request.
///
/// The request URI is used to find the current in-memory document. If the
/// document is known, the handler analyzes its text and returns a selection
/// range chain for each requested position that falls inside a known symbol. If
/// the document is unknown, the handler returns `null`.
pub fn handle(
    state: &ServerState,
    params: SelectionRangeParams,
) -> anyhow::Result<Option<Vec<SelectionRange>>> {
    if let Some(document) = state.documents.get(params.text_document.uri.as_str()) {
        let editor_buffer = editor::from_source(&document.text).with_context(|| {
            format!(
                "failed to analyze document `{:?}`",
                params.text_document.uri
            )
        })?;

        Ok(Some(
            params
                .positions
                .iter()
                .filter_map(|position| selection_range_for_position(&editor_buffer, *position))
                .collect::<Vec<_>>(),
        ))
    } else {
        Ok(None)
    }
}

/// Builds the nested LSP selection range for a single position.
fn selection_range_for_position(
    analysis: &editor::EditorBuffer,
    position: Position,
) -> Option<SelectionRange> {
    let position = achitekfile::TextPosition {
        line: usize::try_from(position.line).ok()?,
        byte: usize::try_from(position.character).ok()?,
    };
    let mut candidates = Vec::new();

    for symbol in analysis.symbols() {
        collect_selection_candidates(symbol, position, &mut candidates);
    }

    candidates.sort_by_key(|range| {
        (
            range.end.line.saturating_sub(range.start.line),
            range.end.byte.saturating_sub(range.start.byte),
        )
    });

    let mut current = None;
    for range in candidates.into_iter().rev() {
        current = Some(SelectionRange {
            range: to_lsp_range(range),
            parent: current.map(Box::new),
        });
    }

    current
}

/// Collects symbol ranges that contain the requested position.
fn collect_selection_candidates(
    symbol: &editor::Symbol,
    position: achitekfile::TextPosition,
    candidates: &mut Vec<achitekfile::TextRange>,
) {
    if contains_position(symbol.selection_range(), position) {
        candidates.push(symbol.selection_range());
    }
    if contains_position(symbol.range(), position) {
        candidates.push(symbol.range());
    }

    for child in symbol.children() {
        collect_selection_candidates(child, position, candidates);
    }
}

/// Returns true when a position is inside a source range.
fn contains_position(range: achitekfile::TextRange, position: achitekfile::TextPosition) -> bool {
    (position.line > range.start.line
        || (position.line == range.start.line && position.byte >= range.start.byte))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.byte <= range.end.byte))
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
