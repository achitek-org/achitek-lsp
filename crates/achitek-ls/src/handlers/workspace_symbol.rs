//! Handler for the LSP `workspace/symbol` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#workspace_symbol>
//!
//! Clients send this request when the user searches for symbols across the
//! workspace. This handler searches the server's in-memory Achitek documents
//! and returns prompt symbols whose names match the query.
use crate::{editor, server::ServerState};
use anyhow::Context;
use lsp_types::{
    Location, Position, Range, SymbolInformation, SymbolKind as LspSymbolKind, Uri,
    WorkspaceSymbolParams, WorkspaceSymbolResponse,
};

/// Handles a `workspace/symbol` request.
pub fn handle(
    state: &ServerState,
    params: WorkspaceSymbolParams,
) -> anyhow::Result<Option<WorkspaceSymbolResponse>> {
    let query = params.query.to_lowercase();
    let mut symbols = Vec::new();

    for (uri, document) in &state.documents {
        let uri = uri
            .parse::<Uri>()
            .with_context(|| format!("failed to parse document URI `{uri}`"))?;
        let editor_buffer = editor::from_source(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;

        for symbol in editor_buffer.symbols() {
            if symbol.kind() != editor::SymbolKind::Prompt {
                continue;
            }
            if !query.is_empty() && !symbol.name().to_lowercase().contains(&query) {
                continue;
            }

            symbols.push(to_lsp_symbol_information(&uri, symbol));
        }
    }

    Ok(Some(WorkspaceSymbolResponse::Flat(symbols)))
}

#[allow(deprecated)]
fn to_lsp_symbol_information(uri: &Uri, symbol: &editor::Symbol) -> SymbolInformation {
    SymbolInformation {
        name: symbol.name().to_owned(),
        kind: match symbol.kind() {
            editor::SymbolKind::Blueprint => LspSymbolKind::MODULE,
            editor::SymbolKind::Prompt => LspSymbolKind::FIELD,
            editor::SymbolKind::Validate => LspSymbolKind::OBJECT,
        },
        tags: None,
        deprecated: None,
        location: Location::new(uri.clone(), to_lsp_range(symbol.selection_range())),
        container_name: Some("Achitekfile".to_owned()),
    }
}

fn to_lsp_range(range: achitekfile::TextRange) -> Range {
    Range {
        start: to_lsp_position(range.start),
        end: to_lsp_position(range.end),
    }
}

fn to_lsp_position(position: achitekfile::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.line).expect("line should fit into u32"),
        character: u32::try_from(position.byte).expect("column should fit into u32"),
    }
}
