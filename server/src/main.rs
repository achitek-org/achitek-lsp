use anyhow::Context;
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    Diagnostic as LspDiagnostic, DiagnosticRelatedInformation, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    FoldingRange, FoldingRangeParams, FoldingRangeProviderCapability, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverContents, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, Location, MarkupContent, MarkupKind, OneOf, Position,
    PrepareRenameResponse, PublishDiagnosticsParams, Range, ReferenceParams, RenameOptions,
    RenameParams, SelectionRange, SelectionRangeParams, SelectionRangeProviderCapability,
    ServerCapabilities, ServerInfo, SymbolInformation, SymbolKind as LspSymbolKind,
    TextDocumentContentChangeEvent, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextEdit, Uri, WorkspaceEdit, WorkspaceSymbolParams,
    WorkspaceSymbolResponse,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

fn main() -> anyhow::Result<()> {
    let (connection, io_threads) = Connection::stdio();
    run_server(&connection)?;

    io_threads.join().context("failed to join LSP IO threads")?;
    Ok(())
}

fn run_server(connection: &Connection) -> anyhow::Result<()> {
    let _params = initialize(connection)?;

    let mut server = Server::default();
    server.run(connection)
}

fn initialize(connection: &Connection) -> anyhow::Result<InitializeParams> {
    let (request_id, params) = connection.initialize_start()?;
    let initialize_params: InitializeParams =
        serde_json::from_value(params).context("failed to deserialize initialize params")?;

    let capabilities = ServerCapabilities {
        completion_provider: Some(CompletionOptions::default()),
        definition_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        references_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..TextDocumentSyncOptions::default()
            },
        )),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        ..ServerCapabilities::default()
    };

    let initialize_result = InitializeResult {
        capabilities,
        server_info: Some(ServerInfo {
            name: "achitek".to_owned(),
            version: None,
        }),
    };

    connection.initialize_finish(request_id, serde_json::to_value(initialize_result)?)?;
    Ok(initialize_params)
}

#[derive(Default)]
struct Server {
    documents: HashMap<Uri, Document>,
}

impl Server {
    fn run(&mut self, connection: &Connection) -> anyhow::Result<()> {
        for message in &connection.receiver {
            match message {
                Message::Request(request) => {
                    if connection.handle_shutdown(&request)? {
                        return Ok(());
                    }

                    self.handle_request(connection, &request)?;
                }
                Message::Notification(notification) => {
                    self.handle_notification(connection, &notification)?;
                }
                Message::Response(_) => {}
            }
        }

        Ok(())
    }

    fn handle_request(&mut self, connection: &Connection, request: &Request) -> anyhow::Result<()> {
        if request.method == "textDocument/documentSymbol" {
            let params: DocumentSymbolParams = serde_json::from_value(request.params.clone())
                .context("failed to parse documentSymbol params")?;
            let result = self.document_symbols(&params.text_document.uri)?;
            let response = Response::new_ok(request.id.clone(), result);

            connection
                .sender
                .send(Message::Response(response))
                .context("failed to send documentSymbol response")?;
            return Ok(());
        }

        if request.method == "textDocument/formatting" {
            let params: DocumentFormattingParams = serde_json::from_value(request.params.clone())
                .context("failed to parse formatting params")?;
            let result = self.formatting(&params.text_document.uri)?;
            let response = Response::new_ok(request.id.clone(), result);

            connection
                .sender
                .send(Message::Response(response))
                .context("failed to send formatting response")?;
            return Ok(());
        }

        if request.method == "textDocument/foldingRange" {
            let params: FoldingRangeParams = serde_json::from_value(request.params.clone())
                .context("failed to parse foldingRange params")?;
            let result = self.folding_ranges(&params.text_document.uri)?;
            let response = Response::new_ok(request.id.clone(), result);

            connection
                .sender
                .send(Message::Response(response))
                .context("failed to send foldingRange response")?;
            return Ok(());
        }

        if request.method == "textDocument/selectionRange" {
            let params: SelectionRangeParams = serde_json::from_value(request.params.clone())
                .context("failed to parse selectionRange params")?;
            let result = self.selection_ranges(&params.text_document.uri, &params.positions)?;
            let response = Response::new_ok(request.id.clone(), result);

            connection
                .sender
                .send(Message::Response(response))
                .context("failed to send selectionRange response")?;
            return Ok(());
        }

        if request.method == "textDocument/hover" {
            let params: HoverParams = serde_json::from_value(request.params.clone())
                .context("failed to parse hover params")?;
            let result = self.hover(
                &params.text_document_position_params.text_document.uri,
                params.text_document_position_params.position,
            )?;
            let response = Response::new_ok(request.id.clone(), result);

            connection
                .sender
                .send(Message::Response(response))
                .context("failed to send hover response")?;
            return Ok(());
        }

        if request.method == "textDocument/completion" {
            let params: CompletionParams = serde_json::from_value(request.params.clone())
                .context("failed to parse completion params")?;
            let result = self.completions(
                &params.text_document_position.text_document.uri,
                params.text_document_position.position,
            )?;
            let response = Response::new_ok(request.id.clone(), result);

            connection
                .sender
                .send(Message::Response(response))
                .context("failed to send completion response")?;
            return Ok(());
        }

        if request.method == "textDocument/definition" {
            let params: GotoDefinitionParams = serde_json::from_value(request.params.clone())
                .context("failed to parse definition params")?;
            let result = self.definition(
                &params.text_document_position_params.text_document.uri,
                params.text_document_position_params.position,
            )?;
            let response = Response::new_ok(request.id.clone(), result);

            connection
                .sender
                .send(Message::Response(response))
                .context("failed to send definition response")?;
            return Ok(());
        }

        if request.method == "textDocument/references" {
            let params: ReferenceParams = serde_json::from_value(request.params.clone())
                .context("failed to parse references params")?;
            let result = self.references(
                &params.text_document_position.text_document.uri,
                params.text_document_position.position,
                params.context.include_declaration,
            )?;
            let response = Response::new_ok(request.id.clone(), result);

            connection
                .sender
                .send(Message::Response(response))
                .context("failed to send references response")?;
            return Ok(());
        }

        if request.method == "textDocument/rename" {
            let params: RenameParams = serde_json::from_value(request.params.clone())
                .context("failed to parse rename params")?;
            let result = self.rename(
                &params.text_document_position.text_document.uri,
                params.text_document_position.position,
                &params.new_name,
            )?;
            let response = Response::new_ok(request.id.clone(), result);

            connection
                .sender
                .send(Message::Response(response))
                .context("failed to send rename response")?;
            return Ok(());
        }

        if request.method == "textDocument/prepareRename" {
            let params: lsp_types::TextDocumentPositionParams =
                serde_json::from_value(request.params.clone())
                    .context("failed to parse prepareRename params")?;
            let result = self.prepare_rename(&params.text_document.uri, params.position)?;
            let response = Response::new_ok(request.id.clone(), result);

            connection
                .sender
                .send(Message::Response(response))
                .context("failed to send prepareRename response")?;
            return Ok(());
        }

        if request.method == "workspace/symbol" {
            let params: WorkspaceSymbolParams = serde_json::from_value(request.params.clone())
                .context("failed to parse workspace symbol params")?;
            let result = self.workspace_symbols(&params.query)?;
            let response = Response::new_ok(request.id.clone(), result);

            connection
                .sender
                .send(Message::Response(response))
                .context("failed to send workspace symbol response")?;
            return Ok(());
        }

        let response = Response {
            id: request.id.clone(),
            result: None,
            error: None,
        };

        connection
            .sender
            .send(Message::Response(response))
            .context("failed to send request response")?;

        Ok(())
    }

    fn handle_notification(
        &mut self,
        connection: &Connection,
        notification: &Notification,
    ) -> anyhow::Result<()> {
        match notification.method.as_str() {
            "textDocument/didOpen" => {
                let params: DidOpenTextDocumentParams =
                    serde_json::from_value(notification.params.clone())
                        .context("failed to parse didOpen params")?;

                let text_document = params.text_document;
                let uri = text_document.uri;
                self.documents.insert(
                    uri.clone(),
                    Document {
                        version: text_document.version,
                        text: text_document.text,
                    },
                );
                self.publish_diagnostics(connection, &uri)?;
                self.publish_template_diagnostics(connection, &uri)?;
            }
            "textDocument/didChange" => {
                let params: DidChangeTextDocumentParams =
                    serde_json::from_value(notification.params.clone())
                        .context("failed to parse didChange params")?;

                let uri = params.text_document.uri;
                if let Some(document) = self.documents.get_mut(&uri) {
                    document.version = params.text_document.version;
                    document.text = apply_content_changes(&document.text, &params.content_changes);
                    self.publish_diagnostics(connection, &uri)?;
                    self.publish_template_diagnostics(connection, &uri)?;
                }
            }
            "textDocument/didClose" => {
                let params: DidCloseTextDocumentParams =
                    serde_json::from_value(notification.params.clone())
                        .context("failed to parse didClose params")?;

                let uri = params.text_document.uri;
                self.documents.remove(&uri);
                self.clear_diagnostics(connection, &uri)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn publish_diagnostics(&self, connection: &Connection, uri: &Uri) -> anyhow::Result<()> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(());
        };

        let analysis = analysis::analyze(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let diagnostics = analysis
            .diagnostics()
            .iter()
            .map(|diagnostic| to_lsp_diagnostic(uri, diagnostic))
            .collect();

        send_notification(
            connection,
            "textDocument/publishDiagnostics",
            PublishDiagnosticsParams {
                uri: uri.clone(),
                diagnostics,
                version: Some(document.version),
            },
        )
    }

    fn clear_diagnostics(&self, connection: &Connection, uri: &Uri) -> anyhow::Result<()> {
        send_notification(
            connection,
            "textDocument/publishDiagnostics",
            PublishDiagnosticsParams {
                uri: uri.clone(),
                diagnostics: Vec::new(),
                version: None,
            },
        )
    }

    fn publish_template_diagnostics(
        &self,
        connection: &Connection,
        uri: &Uri,
    ) -> anyhow::Result<()> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(());
        };
        let Some(blueprint_dir) = blueprint_dir_from_uri(uri) else {
            return Ok(());
        };

        let analysis = analysis::analyze(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let prompt_names = prompt_name_set(&analysis);

        for (template_uri, diagnostics) in scan_template_diagnostics(&blueprint_dir, &prompt_names)?
        {
            send_notification(
                connection,
                "textDocument/publishDiagnostics",
                PublishDiagnosticsParams {
                    uri: template_uri,
                    diagnostics,
                    version: None,
                },
            )?;
        }

        Ok(())
    }

    fn document_symbols(&self, uri: &Uri) -> anyhow::Result<Option<DocumentSymbolResponse>> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(None);
        };

        let analysis = analysis::analyze(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let symbols = analysis
            .symbols()
            .iter()
            .map(to_lsp_document_symbol)
            .collect::<Vec<_>>();

        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    fn formatting(&self, uri: &Uri) -> anyhow::Result<Option<Vec<TextEdit>>> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(None);
        };
        let formatted = format_achitek_source(&document.text);
        if formatted == document.text {
            return Ok(Some(Vec::new()));
        }

        Ok(Some(vec![TextEdit {
            range: full_document_range(&document.text),
            new_text: formatted,
        }]))
    }

    fn folding_ranges(&self, uri: &Uri) -> anyhow::Result<Option<Vec<FoldingRange>>> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(None);
        };
        let analysis = analysis::analyze(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let mut ranges = Vec::new();

        for symbol in analysis.symbols() {
            collect_folding_ranges(symbol, &mut ranges);
        }

        Ok(Some(ranges))
    }

    fn selection_ranges(
        &self,
        uri: &Uri,
        positions: &[Position],
    ) -> anyhow::Result<Option<Vec<SelectionRange>>> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(None);
        };
        let analysis = analysis::analyze(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;

        Ok(Some(
            positions
                .iter()
                .filter_map(|position| selection_range_for_position(&analysis, *position))
                .collect(),
        ))
    }

    fn hover(&self, uri: &Uri, position: Position) -> anyhow::Result<Option<Hover>> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(None);
        };

        let analysis = analysis::analyze(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let hover = analysis.hover(syntax::TextPosition {
            row: usize::try_from(position.line).expect("line should fit into usize"),
            column: usize::try_from(position.character).expect("character should fit into usize"),
        });

        Ok(hover.map(to_lsp_hover))
    }

    fn completions(
        &self,
        uri: &Uri,
        position: Position,
    ) -> anyhow::Result<Option<CompletionResponse>> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(None);
        };

        let analysis = analysis::analyze(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let items = analysis
            .completions(syntax::TextPosition {
                row: usize::try_from(position.line).expect("line should fit into usize"),
                column: usize::try_from(position.character)
                    .expect("character should fit into usize"),
            })
            .into_iter()
            .map(to_lsp_completion_item)
            .collect::<Vec<_>>();

        Ok(Some(CompletionResponse::Array(items)))
    }

    fn definition(
        &self,
        uri: &Uri,
        position: Position,
    ) -> anyhow::Result<Option<GotoDefinitionResponse>> {
        let Some(document) = self.documents.get(uri) else {
            return self.template_definition(uri, position);
        };

        let analysis = analysis::analyze(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let definition = analysis.definition(syntax::TextPosition {
            row: usize::try_from(position.line).expect("line should fit into usize"),
            column: usize::try_from(position.character).expect("character should fit into usize"),
        });

        Ok(definition.map(|target| {
            GotoDefinitionResponse::Scalar(Location::new(
                uri.clone(),
                Range {
                    start: to_lsp_position(target.selection_range().start_position),
                    end: to_lsp_position(target.selection_range().end_position),
                },
            ))
        }))
    }

    fn template_definition(
        &self,
        uri: &Uri,
        position: Position,
    ) -> anyhow::Result<Option<GotoDefinitionResponse>> {
        let Some(template_path) = file_path_from_uri(uri) else {
            return Ok(None);
        };
        if template_path.extension().and_then(|ext| ext.to_str()) != Some("tera") {
            return Ok(None);
        }

        let source = fs::read_to_string(&template_path)
            .with_context(|| format!("failed to read template `{}`", template_path.display()))?;
        let Some(reference_name) = template_reference_at_position(&source, position) else {
            return Ok(None);
        };
        let Some(achitek_path) = find_achitekfile_for_template(&template_path) else {
            return Ok(None);
        };
        let achitek_uri = path_to_uri(&achitek_path)?;
        let achitek_source = self
            .documents
            .get(&achitek_uri)
            .map(|document| document.text.clone())
            .unwrap_or_else(|| fs::read_to_string(&achitek_path).unwrap_or_default());
        let analysis = analysis::analyze(&achitek_source)
            .with_context(|| format!("failed to analyze `{}`", achitek_path.display()))?;
        let Some(symbol) = analysis.symbols().iter().find(|symbol| {
            symbol.kind() == analysis::SymbolKind::Prompt && symbol.name() == reference_name
        }) else {
            return Ok(None);
        };

        Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
            achitek_uri,
            Range {
                start: to_lsp_position(symbol.selection_range().start_position),
                end: to_lsp_position(symbol.selection_range().end_position),
            },
        ))))
    }

    fn references(
        &self,
        uri: &Uri,
        position: Position,
        include_declaration: bool,
    ) -> anyhow::Result<Option<Vec<Location>>> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(None);
        };

        let analysis = analysis::analyze(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let references = analysis.references(
            syntax::TextPosition {
                row: usize::try_from(position.line).expect("line should fit into usize"),
                column: usize::try_from(position.character)
                    .expect("character should fit into usize"),
            },
            include_declaration,
        );
        let prompt_name = analysis.prompt_name(syntax::TextPosition {
            row: usize::try_from(position.line).expect("line should fit into usize"),
            column: usize::try_from(position.character).expect("character should fit into usize"),
        });

        let mut locations: Vec<Location> = references
            .into_iter()
            .map(|target| {
                Location::new(
                    uri.clone(),
                    Range {
                        start: to_lsp_position(target.range().start_position),
                        end: to_lsp_position(target.range().end_position),
                    },
                )
            })
            .collect();

        if let (Some(prompt_name), Some(blueprint_dir)) = (prompt_name, blueprint_dir_from_uri(uri))
        {
            locations.extend(scan_template_references(&blueprint_dir, prompt_name)?);
        }

        Ok(Some(locations))
    }

    #[allow(clippy::mutable_key_type)]
    fn rename(
        &self,
        uri: &Uri,
        position: Position,
        new_name: &str,
    ) -> anyhow::Result<Option<WorkspaceEdit>> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(None);
        };

        let analysis = analysis::analyze(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let cursor_position = syntax::TextPosition {
            row: usize::try_from(position.line).expect("line should fit into usize"),
            column: usize::try_from(position.character).expect("character should fit into usize"),
        };
        let Some(prompt_name) = analysis.prompt_name(cursor_position) else {
            return Ok(None);
        };

        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
        let achitek_references = analysis.references(cursor_position, true);

        for target in achitek_references {
            let range = Range {
                start: to_lsp_position(target.range().start_position),
                end: to_lsp_position(target.range().end_position),
            };
            let replacement = replacement_text_for_range(&document.text, &range, new_name);
            changes.entry(uri.clone()).or_default().push(TextEdit {
                range,
                new_text: replacement,
            });
        }

        if let Some(blueprint_dir) = blueprint_dir_from_uri(uri) {
            let template_locations = scan_template_references(&blueprint_dir, prompt_name)?;

            for location in template_locations {
                let Some(path) = file_path_from_uri(&location.uri) else {
                    continue;
                };
                let source = fs::read_to_string(&path).with_context(|| {
                    format!("failed to read template for rename `{}`", path.display())
                })?;
                let replacement = replacement_text_for_range(&source, &location.range, new_name);
                changes.entry(location.uri).or_default().push(TextEdit {
                    range: location.range,
                    new_text: replacement,
                });
            }
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }))
    }

    fn prepare_rename(
        &self,
        uri: &Uri,
        position: Position,
    ) -> anyhow::Result<Option<PrepareRenameResponse>> {
        let Some(document) = self.documents.get(uri) else {
            return Ok(None);
        };

        let analysis = analysis::analyze(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let target = analysis.prepare_rename(syntax::TextPosition {
            row: usize::try_from(position.line).expect("line should fit into usize"),
            column: usize::try_from(position.character).expect("character should fit into usize"),
        });

        Ok(
            target.map(|target| PrepareRenameResponse::RangeWithPlaceholder {
                range: Range {
                    start: to_lsp_position(target.range().start_position),
                    end: to_lsp_position(target.range().end_position),
                },
                placeholder: target.placeholder().to_owned(),
            }),
        )
    }

    fn workspace_symbols(&self, query: &str) -> anyhow::Result<Option<WorkspaceSymbolResponse>> {
        let query = query.to_lowercase();
        let mut symbols = Vec::new();

        for (uri, document) in &self.documents {
            let analysis = analysis::analyze(&document.text)
                .with_context(|| format!("failed to analyze document `{:?}`", uri))?;

            for symbol in analysis.symbols() {
                if symbol.kind() != analysis::SymbolKind::Prompt {
                    continue;
                }
                if !query.is_empty() && !symbol.name().to_lowercase().contains(&query) {
                    continue;
                }

                symbols.push(to_lsp_symbol_information(uri, symbol));
            }
        }

        Ok(Some(WorkspaceSymbolResponse::Flat(symbols)))
    }
}

#[derive(Debug, Clone)]
struct Document {
    version: i32,
    text: String,
}

fn apply_content_changes(
    current_text: &str,
    content_changes: &[TextDocumentContentChangeEvent],
) -> String {
    content_changes
        .last()
        .map(|change| change.text.clone())
        .unwrap_or_else(|| current_text.to_owned())
}

fn to_lsp_diagnostic(uri: &Uri, diagnostic: &analysis::Diagnostic) -> LspDiagnostic {
    LspDiagnostic {
        range: Range {
            start: to_lsp_position(diagnostic.range().start_position),
            end: to_lsp_position(diagnostic.range().end_position),
        },
        severity: Some(match diagnostic.severity() {
            analysis::Severity::Error => DiagnosticSeverity::ERROR,
            analysis::Severity::Warning => DiagnosticSeverity::WARNING,
            analysis::Severity::Information => DiagnosticSeverity::INFORMATION,
            analysis::Severity::Hint => DiagnosticSeverity::HINT,
        }),
        message: diagnostic.message().to_owned(),
        related_information: Some(
            diagnostic
                .related_information()
                .iter()
                .map(|info| DiagnosticRelatedInformation {
                    location: Location::new(
                        uri.clone(),
                        Range {
                            start: to_lsp_position(info.range().start_position),
                            end: to_lsp_position(info.range().end_position),
                        },
                    ),
                    message: info.message().to_owned(),
                })
                .collect(),
        ),
        ..LspDiagnostic::default()
    }
}

fn to_lsp_position(position: syntax::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.row).expect("line should fit into u32"),
        character: u32::try_from(position.column).expect("column should fit into u32"),
    }
}

#[allow(deprecated)]
fn to_lsp_document_symbol(symbol: &analysis::Symbol) -> DocumentSymbol {
    DocumentSymbol {
        name: symbol.name().to_owned(),
        detail: symbol.detail().map(str::to_owned),
        kind: match symbol.kind() {
            analysis::SymbolKind::Blueprint => LspSymbolKind::MODULE,
            analysis::SymbolKind::Prompt => LspSymbolKind::FIELD,
            analysis::SymbolKind::Validate => LspSymbolKind::OBJECT,
        },
        tags: None,
        deprecated: None,
        range: Range {
            start: to_lsp_position(symbol.range().start_position),
            end: to_lsp_position(symbol.range().end_position),
        },
        selection_range: Range {
            start: to_lsp_position(symbol.selection_range().start_position),
            end: to_lsp_position(symbol.selection_range().end_position),
        },
        children: Some(
            symbol
                .children()
                .iter()
                .map(to_lsp_document_symbol)
                .collect(),
        ),
    }
}

#[allow(deprecated)]
fn to_lsp_symbol_information(uri: &Uri, symbol: &analysis::Symbol) -> SymbolInformation {
    SymbolInformation {
        name: symbol.name().to_owned(),
        kind: match symbol.kind() {
            analysis::SymbolKind::Blueprint => LspSymbolKind::MODULE,
            analysis::SymbolKind::Prompt => LspSymbolKind::FIELD,
            analysis::SymbolKind::Validate => LspSymbolKind::OBJECT,
        },
        tags: None,
        deprecated: None,
        location: Location::new(
            uri.clone(),
            Range {
                start: to_lsp_position(symbol.selection_range().start_position),
                end: to_lsp_position(symbol.selection_range().end_position),
            },
        ),
        container_name: Some("Achitekfile".to_owned()),
    }
}

fn to_lsp_hover(hover: analysis::Hover) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: hover.contents().to_owned(),
        }),
        range: Some(Range {
            start: to_lsp_position(hover.range().start_position),
            end: to_lsp_position(hover.range().end_position),
        }),
    }
}

fn to_lsp_completion_item(item: analysis::Completion) -> CompletionItem {
    CompletionItem {
        label: item.label().to_owned(),
        detail: item.detail().map(str::to_owned),
        kind: Some(match item.kind() {
            analysis::CompletionKind::Keyword => CompletionItemKind::KEYWORD,
            analysis::CompletionKind::Property => CompletionItemKind::PROPERTY,
            analysis::CompletionKind::Value => CompletionItemKind::VALUE,
            analysis::CompletionKind::Reference => CompletionItemKind::REFERENCE,
            analysis::CompletionKind::Function => CompletionItemKind::FUNCTION,
        }),
        ..CompletionItem::default()
    }
}

fn format_achitek_source(source: &str) -> String {
    let mut formatted = String::new();
    let mut indent = 0usize;

    for raw_line in source.lines() {
        let line = raw_line.trim();

        if line.starts_with('}') {
            indent = indent.saturating_sub(1);
        }

        if line.is_empty() {
            formatted.push('\n');
        } else {
            formatted.push_str(&"  ".repeat(indent));
            formatted.push_str(line);
            formatted.push('\n');
        }

        if line.ends_with('{') {
            indent += 1;
        }
    }

    formatted
}

fn full_document_range(source: &str) -> Range {
    let last_line = source.lines().last().unwrap_or("");
    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: u32::try_from(source.lines().count()).expect("line count should fit into u32"),
            character: u32::try_from(last_line.len()).expect("line length should fit into u32"),
        },
    }
}

fn collect_folding_ranges(symbol: &analysis::Symbol, ranges: &mut Vec<FoldingRange>) {
    let range = symbol.range();
    if range.start_position.row < range.end_position.row {
        ranges.push(FoldingRange {
            start_line: u32::try_from(range.start_position.row).expect("line should fit into u32"),
            start_character: Some(
                u32::try_from(range.start_position.column).expect("column should fit into u32"),
            ),
            end_line: u32::try_from(range.end_position.row).expect("line should fit into u32"),
            end_character: Some(
                u32::try_from(range.end_position.column).expect("column should fit into u32"),
            ),
            kind: None,
            collapsed_text: Some(symbol.name().to_owned()),
        });
    }

    for child in symbol.children() {
        collect_folding_ranges(child, ranges);
    }
}

fn selection_range_for_position(
    analysis: &analysis::Analysis,
    position: Position,
) -> Option<SelectionRange> {
    let position = syntax::TextPosition {
        row: usize::try_from(position.line).ok()?,
        column: usize::try_from(position.character).ok()?,
    };
    let mut candidates = Vec::new();

    for symbol in analysis.symbols() {
        collect_selection_candidates(symbol, position, &mut candidates);
    }

    candidates.sort_by_key(|range| {
        (
            range
                .end_position
                .row
                .saturating_sub(range.start_position.row),
            range
                .end_position
                .column
                .saturating_sub(range.start_position.column),
        )
    });

    let mut current = None;
    for range in candidates.into_iter().rev() {
        current = Some(SelectionRange {
            range: Range {
                start: to_lsp_position(range.start_position),
                end: to_lsp_position(range.end_position),
            },
            parent: current.map(Box::new),
        });
    }

    current
}

fn collect_selection_candidates(
    symbol: &analysis::Symbol,
    position: syntax::TextPosition,
    candidates: &mut Vec<syntax::TextRange>,
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

fn contains_position(range: syntax::TextRange, position: syntax::TextPosition) -> bool {
    (position.row > range.start_position.row
        || (position.row == range.start_position.row
            && position.column >= range.start_position.column))
        && (position.row < range.end_position.row
            || (position.row == range.end_position.row
                && position.column <= range.end_position.column))
}

fn send_notification<P: serde::Serialize>(
    connection: &Connection,
    method: &'static str,
    params: P,
) -> anyhow::Result<()> {
    let notification = Notification::new(method.to_owned(), params);
    connection
        .sender
        .send(Message::Notification(notification))
        .with_context(|| format!("failed to send `{method}` notification"))?;
    Ok(())
}

fn blueprint_dir_from_uri(uri: &Uri) -> Option<PathBuf> {
    let raw = uri.as_str();
    let path = raw.strip_prefix("file://")?;
    let path = if cfg!(windows) && path.starts_with('/') {
        &path[1..]
    } else {
        path
    };
    Path::new(path).parent().map(Path::to_path_buf)
}

fn scan_template_references(root: &Path, prompt_name: &str) -> anyhow::Result<Vec<Location>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut locations = Vec::new();
    collect_template_references(root, prompt_name, &mut locations)?;
    Ok(locations)
}

fn scan_template_diagnostics(
    root: &Path,
    prompt_names: &HashSet<String>,
) -> anyhow::Result<Vec<(Uri, Vec<LspDiagnostic>)>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut diagnostics = Vec::new();
    collect_template_diagnostics(root, prompt_names, &mut diagnostics)?;
    Ok(diagnostics)
}

fn collect_template_diagnostics(
    root: &Path,
    prompt_names: &HashSet<String>,
    diagnostics: &mut Vec<(Uri, Vec<LspDiagnostic>)>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read blueprint directory `{}`", root.display()))?
    {
        let entry = entry.context("failed to read blueprint directory entry")?;
        let path = entry.path();

        if path.is_dir() {
            collect_template_diagnostics(&path, prompt_names, diagnostics)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("tera") {
            continue;
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read template `{}`", path.display()))?;
        let uri = path_to_uri(&path)
            .with_context(|| format!("failed to convert `{}` to a file URI", path.display()))?;
        let template_diagnostics = find_unknown_template_references(&source, &uri, prompt_names);
        diagnostics.push((uri, template_diagnostics));
    }

    Ok(())
}

fn find_unknown_template_references(
    source: &str,
    uri: &Uri,
    prompt_names: &HashSet<String>,
) -> Vec<LspDiagnostic> {
    find_template_identifiers_in_source(source, uri)
        .into_iter()
        .filter(|reference| !prompt_names.contains(&reference.name))
        .map(|reference| LspDiagnostic {
            range: reference.location.range,
            severity: Some(DiagnosticSeverity::ERROR),
            message: format!("unknown prompt reference `{}`", reference.name),
            ..LspDiagnostic::default()
        })
        .collect()
}

fn collect_template_references(
    root: &Path,
    prompt_name: &str,
    locations: &mut Vec<Location>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read blueprint directory `{}`", root.display()))?
    {
        let entry = entry.context("failed to read blueprint directory entry")?;
        let path = entry.path();

        if path.is_dir() {
            collect_template_references(&path, prompt_name, locations)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("tera") {
            continue;
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read template `{}`", path.display()))?;
        let uri = path_to_uri(&path)
            .with_context(|| format!("failed to convert `{}` to a file URI", path.display()))?;
        locations.extend(find_template_references_in_source(
            &source,
            &uri,
            prompt_name,
        ));
    }

    Ok(())
}

fn find_template_references_in_source(source: &str, uri: &Uri, prompt_name: &str) -> Vec<Location> {
    find_template_identifiers_in_source(source, uri)
        .into_iter()
        .filter(|reference| reference.name == prompt_name)
        .map(|reference| reference.location)
        .collect()
}

fn tera_reference_context(line: &str, start: usize, end: usize) -> bool {
    let before = &line[..start];
    let after = &line[end..];

    let in_output = before
        .rfind("{{")
        .is_some_and(|open| before[open..].find("}}").is_none())
        && after.contains("}}");
    let in_tag = before
        .rfind("{%")
        .is_some_and(|open| before[open..].find("%}").is_none())
        && after.contains("%}");

    in_output || in_tag
}

#[derive(Debug, Clone)]
struct TemplateReference {
    name: String,
    location: Location,
}

fn find_template_identifiers_in_source(source: &str, uri: &Uri) -> Vec<TemplateReference> {
    let mut references = Vec::new();

    for (row, line) in source.lines().enumerate() {
        let mut index = 0;
        while index < line.len() {
            let Some((offset, ch)) = line[index..].char_indices().next() else {
                break;
            };
            let start = index + offset;

            if !is_identifier_start(ch) {
                index = start + ch.len_utf8();
                continue;
            }

            let mut end = start + ch.len_utf8();
            while end < line.len() {
                let Some(next) = line[end..].chars().next() else {
                    break;
                };
                if !is_identifier_continue(next) {
                    break;
                }
                end += next.len_utf8();
            }

            let name = &line[start..end];
            if tera_reference_context(line, start, end)
                && !is_tera_keyword(name)
                && !(tera_tag_context(line, start, end)
                    && is_inside_quoted_template_string(line, start, end))
            {
                references.push(TemplateReference {
                    name: name.to_owned(),
                    location: Location::new(
                        uri.clone(),
                        Range {
                            start: Position {
                                line: u32::try_from(row).expect("row should fit into u32"),
                                character: u32::try_from(start)
                                    .expect("column should fit into u32"),
                            },
                            end: Position {
                                line: u32::try_from(row).expect("row should fit into u32"),
                                character: u32::try_from(end).expect("column should fit into u32"),
                            },
                        },
                    ),
                });
            }

            index = end;
        }
    }

    references
}

fn template_reference_at_position(source: &str, position: Position) -> Option<String> {
    let column = usize::try_from(position.character).ok()?;

    find_template_identifiers_in_source(source, &"file:///template.tera".parse().ok()?)
        .into_iter()
        .find(|reference| {
            let range = reference.location.range;
            range.start.line == position.line
                && usize::try_from(range.start.character)
                    .ok()
                    .is_some_and(|start| start <= column)
                && usize::try_from(range.end.character)
                    .ok()
                    .is_some_and(|end| column <= end)
        })
        .map(|reference| reference.name)
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_tera_keyword(name: &str) -> bool {
    matches!(
        name,
        "and"
            | "as"
            | "block"
            | "elif"
            | "else"
            | "endblock"
            | "endfor"
            | "endif"
            | "extends"
            | "false"
            | "filter"
            | "for"
            | "if"
            | "in"
            | "include"
            | "loop"
            | "not"
            | "or"
            | "set"
            | "true"
            | "with"
    )
}

fn is_inside_quoted_template_string(line: &str, start: usize, end: usize) -> bool {
    line[..start].chars().filter(|ch| *ch == '"').count() % 2 == 1 && line[end..].contains('"')
}

fn tera_tag_context(line: &str, start: usize, end: usize) -> bool {
    let before = &line[..start];
    let after = &line[end..];
    before
        .rfind("{%")
        .is_some_and(|open| before[open..].find("%}").is_none())
        && after.contains("%}")
}

fn prompt_name_set(analysis: &analysis::Analysis) -> HashSet<String> {
    analysis
        .symbols()
        .iter()
        .filter(|symbol| symbol.kind() == analysis::SymbolKind::Prompt)
        .map(|symbol| symbol.name().to_owned())
        .collect()
}

fn find_achitekfile_for_template(template_path: &Path) -> Option<PathBuf> {
    let mut dir = template_path.parent()?;
    loop {
        let candidate = dir.join("Achitekfile");
        if candidate.exists() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}

fn path_to_uri(path: &Path) -> anyhow::Result<Uri> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize `{}`", path.display()))?;
    let value = format!("file://{}", path.to_string_lossy());
    value
        .parse()
        .with_context(|| format!("failed to parse `{value}` as a URI"))
}

fn file_path_from_uri(uri: &Uri) -> Option<PathBuf> {
    let raw = uri.as_str();
    let path = raw.strip_prefix("file://")?;
    let path = if cfg!(windows) && path.starts_with('/') {
        &path[1..]
    } else {
        path
    };
    Some(PathBuf::from(path))
}

fn replacement_text_for_range(source: &str, range: &Range, new_name: &str) -> String {
    if selected_text(source, range).is_some_and(|text| text.starts_with('"') && text.ends_with('"'))
    {
        format!("\"{new_name}\"")
    } else {
        new_name.to_owned()
    }
}

fn selected_text<'a>(source: &'a str, range: &Range) -> Option<&'a str> {
    if range.start.line != range.end.line {
        return None;
    }

    let line = source
        .lines()
        .nth(usize::try_from(range.start.line).ok()?)?;
    let start = usize::try_from(range.start.character).ok()?;
    let end = usize::try_from(range.end.character).ok()?;
    line.get(start..end)
}

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use lsp_server::{Request, RequestId};
    use lsp_types::{
        notification::{
            DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Initialized,
            Notification as _,
        },
        request::{
            Completion, DocumentSymbolRequest, GotoDefinition, HoverRequest, Initialize,
            PrepareRenameRequest, References, Rename, Request as _, WorkspaceSymbolRequest,
        },
        notification::{
            DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Initialized,
            Notification as _,
        },
        request::{
            Completion, DocumentSymbolRequest, GotoDefinition, HoverRequest, Initialize,
            PrepareRenameRequest, References, Rename, Request as _, WorkspaceSymbolRequest,
        },
    };
    use serde_json::json;
    use std::{
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn server_smoke_test_publishes_and_clears_diagnostics() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();

        let server_thread = thread::spawn(move || run_server(&server_connection));

        let initialize_id = RequestId::from(1_i32);
        send_request(
            &client_connection,
            initialize_id.clone(),
            Initialize::METHOD,
            InitializeParams {
                capabilities: ClientCapabilities::default(),
                ..InitializeParams::default()
            },
        )?;

        let initialize_response = recv_response(&client_connection)?;
        assert_eq!(initialize_response.id, initialize_id);
        assert!(initialize_response.error.is_none());

        send_notification_message(
            &client_connection,
            Initialized::METHOD,
            InitializedParams {},
        )?;

        let uri: Uri = "file:///workspace/Achitekfile"
            .parse()
            .context("failed to parse test URI")?;

        send_notification_message(
            &client_connection,
            DidOpenTextDocument::METHOD,
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "achitek".to_owned(),
                    version: 1,
                    text: invalid_source(),
                },
            },
        )?;

        let open_diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(open_diagnostics.uri, uri);
        assert_eq!(open_diagnostics.version, Some(1));
        assert!(!open_diagnostics.diagnostics.is_empty());

        send_notification_message(
            &client_connection,
            DidChangeTextDocument::METHOD,
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: valid_source(),
                }],
            },
        )?;

        let changed_diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(changed_diagnostics.uri, uri);
        assert_eq!(changed_diagnostics.version, Some(2));
        assert!(changed_diagnostics.diagnostics.is_empty());

        send_request(
            &client_connection,
            RequestId::from(3_i32),
            DocumentSymbolRequest::METHOD,
            DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )?;

        let symbol_response = recv_response(&client_connection)?;
        assert!(symbol_response.error.is_none());
        let symbols: Option<DocumentSymbolResponse> = serde_json::from_value(
            symbol_response
                .result
                .expect("document symbols should have a result"),
        )
        .context("failed to deserialize documentSymbol response")?;
        let DocumentSymbolResponse::Nested(symbols) =
            symbols.expect("document symbols should exist")
        else {
            panic!("expected nested document symbols");
        };
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "blueprint");
        assert_eq!(symbols[1].name, "project_name");
        assert_eq!(symbols[1].children.as_ref().map(Vec::len), Some(0));

        send_request(
            &client_connection,
            RequestId::from(4_i32),
            HoverRequest::METHOD,
            HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position {
                        line: 6,
                        character: 10,
                    },
                },
                work_done_progress_params: Default::default(),
            },
        )?;

        let hover_response = recv_response(&client_connection)?;
        assert!(hover_response.error.is_none());
        let hover: Option<Hover> =
            serde_json::from_value(hover_response.result.expect("hover should have a result"))
                .context("failed to deserialize hover response")?;
        let hover = hover.expect("hover should exist");
        let HoverContents::Markup(MarkupContent { value, .. }) = hover.contents else {
            panic!("expected markdown hover contents");
        };
        assert!(value.contains("`string`"));

        send_request(
            &client_connection,
            RequestId::from(5_i32),
            Completion::METHOD,
            CompletionParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position {
                        line: 6,
                        character: 21,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        )?;

        let completion_response = recv_response(&client_connection)?;
        assert!(completion_response.error.is_none());
        let completion: Option<CompletionResponse> = serde_json::from_value(
            completion_response
                .result
                .expect("completion should have a result"),
        )
        .context("failed to deserialize completion response")?;
        let CompletionResponse::Array(items) = completion.expect("completion should exist") else {
            panic!("expected array completion response");
        };
        assert!(items.iter().any(|item| item.label == "string"));

        send_notification_message(
            &client_connection,
            DidChangeTextDocument::METHOD,
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 3,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: definition_source(),
                }],
            },
        )?;

        let definition_ready_diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(definition_ready_diagnostics.uri, uri);
        assert_eq!(definition_ready_diagnostics.version, Some(3));
        assert!(definition_ready_diagnostics.diagnostics.is_empty());

        send_request(
            &client_connection,
            RequestId::from(6_i32),
            GotoDefinition::METHOD,
            GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position {
                        line: 13,
                        character: 17,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )?;

        let definition_response = recv_response(&client_connection)?;
        assert!(definition_response.error.is_none());
        let definition: Option<GotoDefinitionResponse> = serde_json::from_value(
            definition_response
                .result
                .expect("definition should have a result"),
        )
        .context("failed to deserialize definition response")?;
        let GotoDefinitionResponse::Scalar(location) = definition.expect("definition should exist")
        else {
            panic!("expected scalar definition response");
        };
        assert_eq!(location.uri, uri);
        assert_eq!(location.range.start.line, 5);

        send_request(
            &client_connection,
            RequestId::from(7_i32),
            References::METHOD,
            ReferenceParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position {
                        line: 5,
                        character: 9,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: lsp_types::ReferenceContext {
                    include_declaration: true,
                },
            },
        )?;

        let references_response = recv_response(&client_connection)?;
        assert!(references_response.error.is_none());
        let references: Option<Vec<Location>> = serde_json::from_value(
            references_response
                .result
                .expect("references should have a result"),
        )
        .context("failed to deserialize references response")?;
        let references = references.expect("references should exist");
        assert_eq!(references.len(), 2);
        assert_eq!(references[0].range.start.line, 5);
        assert_eq!(references[1].range.start.line, 13);

        send_request(
            &client_connection,
            RequestId::from(8_i32),
            PrepareRenameRequest::METHOD,
            TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 13,
                    character: 16,
                },
            },
        )?;

        let prepare_rename_response = recv_response(&client_connection)?;
        assert!(prepare_rename_response.error.is_none());
        let prepare_rename: Option<PrepareRenameResponse> = serde_json::from_value(
            prepare_rename_response
                .result
                .expect("prepareRename should have a result"),
        )
        .context("failed to deserialize prepareRename response")?;
        let PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } =
            prepare_rename.expect("prepareRename should exist")
        else {
            panic!("expected prepareRename range with placeholder");
        };
        assert_eq!(placeholder, "project_name");
        assert_eq!(range.start.line, 13);
        assert_eq!(range.start.character, 15);

        send_request(
            &client_connection,
            RequestId::from(9_i32),
            PrepareRenameRequest::METHOD,
            TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 1,
                    character: 3,
                },
            },
        )?;

        let invalid_prepare_rename_response = recv_response(&client_connection)?;
        assert!(invalid_prepare_rename_response.error.is_none());
        let invalid_prepare_rename: Option<PrepareRenameResponse> = serde_json::from_value(
            invalid_prepare_rename_response
                .result
                .expect("prepareRename should have a result"),
        )
        .context("failed to deserialize empty prepareRename response")?;
        assert!(invalid_prepare_rename.is_none());

        send_notification_message(
            &client_connection,
            DidCloseTextDocument::METHOD,
            DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
            },
        )?;

        let closed_diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(closed_diagnostics.uri, uri);
        assert_eq!(closed_diagnostics.version, None);
        assert!(closed_diagnostics.diagnostics.is_empty());

        send_request(
            &client_connection,
            RequestId::from(2_i32),
            "shutdown",
            json!({}),
        )?;

        let shutdown_response = recv_response(&client_connection)?;
        assert!(shutdown_response.error.is_none());

        send_notification_message(&client_connection, "exit", json!({}))?;

        server_thread
            .join()
            .expect("server thread should not panic")
            .context("server loop should exit cleanly")?;

        Ok(())
    }

    #[test]
    fn scans_template_references_in_blueprint_directory() -> anyhow::Result<()> {
        let temp_root = std::env::temp_dir().join(format!(
            "achitek-references-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_root)
            .with_context(|| format!("failed to create `{}`", temp_root.display()))?;

        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(
            &template_path,
            r#"[package]
name = "{{project}}"
description = "{{description}}"
repository = "{{repo}}"

{% if dev_profile == "FastCompile" -%}
[profile.dev]
debug = 0
{% endif %}
"#,
        )
        .with_context(|| format!("failed to write `{}`", template_path.display()))?;

        let references = scan_template_references(&temp_root, "repo")?;
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].range.start.line, 3);

        fs::remove_dir_all(&temp_root)
            .with_context(|| format!("failed to clean up `{}`", temp_root.display()))?;
        Ok(())
    }

    #[test]
    #[allow(clippy::mutable_key_type)]
    fn rename_returns_workspace_edits_for_achitekfile_and_templates() -> anyhow::Result<()> {
        let temp_root = std::env::temp_dir().join(format!(
            "achitek-rename-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_root)
            .with_context(|| format!("failed to create `{}`", temp_root.display()))?;

        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(
            &template_path,
            r#"[package]
name = "{{project_name}}"
repository = "{{project_name}}"
"#,
        )
        .with_context(|| format!("failed to write `{}`", template_path.display()))?;

        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, definition_source())
            .with_context(|| format!("failed to write `{}`", achitek_path.display()))?;
        let uri = path_to_uri(&achitek_path)?;

        let (server_connection, client_connection) = Connection::memory();
        let server_thread = thread::spawn(move || run_server(&server_connection));

        let initialize_id = RequestId::from(10_i32);
        send_request(
            &client_connection,
            initialize_id.clone(),
            Initialize::METHOD,
            InitializeParams {
                capabilities: ClientCapabilities::default(),
                ..InitializeParams::default()
            },
        )?;
        let _ = recv_response(&client_connection)?;

        send_notification_message(
            &client_connection,
            Initialized::METHOD,
            InitializedParams {},
        )?;

        send_notification_message(
            &client_connection,
            DidOpenTextDocument::METHOD,
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "achitek".to_owned(),
                    version: 1,
                    text: definition_source(),
                },
            },
        )?;

        let diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert!(diagnostics.diagnostics.is_empty());
        let template_diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(template_diagnostics.uri, path_to_uri(&template_path)?);
        assert!(template_diagnostics.diagnostics.is_empty());

        send_request(
            &client_connection,
            RequestId::from(11_i32),
            Rename::METHOD,
            RenameParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position {
                        line: 5,
                        character: 10,
                    },
                },
                new_name: "repository_name".to_owned(),
                work_done_progress_params: Default::default(),
            },
        )?;

        let rename_response = recv_response(&client_connection)?;
        assert!(rename_response.error.is_none());
        let edit: Option<WorkspaceEdit> =
            serde_json::from_value(rename_response.result.expect("rename should have a result"))
                .context("failed to deserialize rename response")?;
        let edit = edit.expect("rename edit should exist");
        let changes = edit.changes.expect("rename should include file changes");

        let achitek_changes = changes
            .get(&uri)
            .expect("rename should edit the Achitekfile");
        assert!((|edit| edit.new_text == "\"repository_name\""));
        assetek_changes    .iter()
            let late_uri = path_to_uri(&template_path)?;
        let late.expect("rename should edit matching templates");
        asseq!(template_changes.len(), 2);
        assetemplate_changes    .iter()
            .all(|edit| edit.new_text == "repository_name"));

        send_req&client_connection,
            estId::from(12_i32),
            tdown",    json!({}),
        )?;
        let _ = _notification_message(&client_connection, "exit", json!({}))?;

        servhread    .join()
            .expect("server thread should not panic")
            .context("server loop should exit cleanly")?;

        fs::remove_dir_all(&temp_root)
            .with_context(|| format!("failed to clean up `{}`", temp_root.display()))?;
        Ok(())
    }

    #[test]
    fn publishes_template_diagnostics_for_unknown_prompt_references() -> anyhow::Result<()> {
        let temp_root = std::env::temp_dir().join(format!(
            "achitek-template-diagnostics-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_root)
            .with_context(|| format!("failed to create `{}`", temp_root.display()))?;

        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, valid_source())
            .with_context(|| format!("failed to write `{}`", achitek_path.display()))?;
        let achitek_uri = path_to_uri(&achitek_path)?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(
            &template_path,
            r#"[package]
name = "{{project_name}}"
description = "{{missing_prompt}}"
"#,
        )
        .with_context(|| format!("failed to write `{}`", template_path.display()))?;
        let template_uri = path_to_uri(&template_path)?;

        let (server_connection, client_connection) = Connection::memory();
        let server_thread = thread::spawn(move || run_server(&server_connection));

        send_request(
            &client_connection,
            RequestId::from(30_i32),
            Initialize::METHOD,
            InitializeParams {
                capabilities: ClientCapabilities::default(),
                ..InitializeParams::default()
            },
        )?;
        let _ = recv_response(&client_connection)?;
        send_notification_message(
            &client_connection,
            Initialized::METHOD,
            InitializedParams {},
        )?;

        send_notification_message(
            &client_connection,
            DidOpenTextDocument::METHOD,
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: achitek_uri,
                    language_id: "achitek".to_owned(),
                    version: 1,
                    text: valid_source(),
                },
            },
        )?;

        let achitek_diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert!(achitek_diagnostics.diagnostics.is_empty());
        let template_diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(template_diagnostics.uri, template_uri);
        assert_eq!(template_diagnostics.diagnostics.len(), 1);
        assert_eq!(
            template_diagnostics.diagnostics[0].message,
            "unknown prompt reference `missing_prompt`"
        );

        shutdown_server(&client_connection, RequestId::from(31_i32))?;
        server_thread
            .join()
            .expect("server thread should not panic")
            .context("server loop should exit cleanly")?;
        fs::remove_dir_all(&temp_root)
            .with_context(|| format!("failed to clean up `{}`", temp_root.display()))?;
        Ok(())
    }

    #[test]
    fn resolves_template_definition_to_prompt() -> anyhow::Result<()> {
        let temp_root = std::env::temp_dir().join(format!(
            "achitek-template-definition-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_root)
            .with_context(|| format!("failed to create `{}`", temp_root.display()))?;

        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, valid_source())
            .with_context(|| format!("failed to write `{}`", achitek_path.display()))?;
        let achitek_uri = path_to_uri(&achitek_path)?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(&template_path, "name = \"{{project_name}}\"\n")
            .with_context(|| format!("failed to write `{}`", template_path.display()))?;
        let template_uri = path_to_uri(&template_path)?;

        let (server_connection, client_connection) = Connection::memory();
        let server_thread = thread::spawn(move || run_server(&server_connection));

        send_request(
            &client_connection,
            RequestId::from(32_i32),
            Initialize::METHOD,
            InitializeParams {
                capabilities: ClientCapabilities::default(),
                ..InitializeParams::default()
            },
        )?;
        let _ = recv_response(&client_connection)?;
        send_notification_message(
            &client_connection,
            Initialized::METHOD,
            InitializedParams {},
        )?;

        send_request(
            &client_connection,
            RequestId::from(33_i32),
            GotoDefinition::METHOD,
            GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: template_uri },
                    position: Position {
                        line: 0,
                        character: 13,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )?;

        let response = recv_response(&client_connection)?;
        assert!(response.error.is_none());
        let definition: Option<GotoDefinitionResponse> =
            serde_json::from_value(response.result.expect("definition should have a result"))
                .context("failed to deserialize template definition response")?;
        let GotoDefinitionResponse::Scalar(location) =
            definition.expect("template definition should exist")
        else {
            panic!("expected scalar definition response");
        };
        assert_eq!(location.uri, achitek_uri);
        assert_eq!(location.range.start.line, 5);

        shutdown_server(&client_connection, RequestId::from(34_i32))?;
        server_thread
            .join()
            .expect("server thread should not panic")
            .context("server loop should exit cleanly")?;
        fs::remove_dir_all(&temp_root)
            .with_context(|| format!("failed to clean up `{}`", temp_root.display()))?;
        Ok(())
    }

    #[test]
    fn returns_workspace_symbols_for_open_achitekfiles() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let server_thread = thread::spawn(move || run_server(&server_connection));

        send_request(
            &client_connection,
            RequestId::from(35_i32),
            Initialize::METHOD,
            InitializeParams {
                capabilities: ClientCapabilities::default(),
                ..InitializeParams::default()
            },
        )?;
        let _ = recv_response(&client_connection)?;
        send_notification_message(
            &client_connection,
            Initialized::METHOD,
            InitializedParams {},
        )?;

        let uri: Uri = "file:///workspace/Achitekfile"
            .parse()
            .context("failed to parse test URI")?;
        send_notification_message(
            &client_connection,
            DidOpenTextDocument::METHOD,
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "achitek".to_owned(),
                    version: 1,
                    text: valid_source(),
                },
            },
        )?;
        let _ = recv_publish_diagnostics(&client_connection)?;

        send_request(
            &client_connection,
            RequestId::from(36_i32),
            WorkspaceSymbolRequest::METHOD,
            WorkspaceSymbolParams {
                query: "project".to_owned(),
                partial_result_params: Default::default(),
                work_done_progress_params: Default::default(),
            },
        )?;

        let response = recv_response(&client_connection)?;
        assert!(response.error.is_none());
        let symbols: Option<WorkspaceSymbolResponse> = serde_json::from_value(
            response
                .result
                .expect("workspace symbols should have a result"),
        )
        .context("failed to deserialize workspaceSymbol response")?;
        let WorkspaceSymbolResponse::Flat(symbols) =
            symbols.expect("workspace symbols should exist")
        else {
            panic!("expected flat workspace symbols");
        };
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "project_name");
        assert_eq!(symbols[0].location.uri, uri);

        shutdown_server(&client_connection, RequestId::from(37_i32))?;
        server_thread
            .join()
            .expect("server thread should not panic")
            .context("server loop should exit cleanly")?;
        Ok(())
    }

    #[test]
    fn formats_achitek_source_with_consistent_indentation() {
        let source = r#"blueprint {
version = "1.0.0"
}

prompt "project_name" {
type = string
validate {
min_length = 2
}
}
"#;

        let formatted = format_achitek_source(source);

        assert!(formatted.contains("  version = \"1.0.0\""));
        assert!(formatted.contains("  type = string"));
        assert!(formatted.contains("    min_length = 2"));
    }

    #[test]
    fn builds_folding_ranges_from_symbols() {
        let analysis = analysis::analyze(&valid_source()).expect("valid source should analyze");
        let mut ranges = Vec::new();

        for symbol in analysis.symbols() {
            collect_folding_ranges(symbol, &mut ranges);
        }

        assert!(ranges.iter().any(|range| range.start_line == 0));
        assert!(ranges.iter().any(|range| range.start_line == 5));
    }

    #[test]
    fn builds_selection_ranges_from_symbols() {
        let analysis = analysis::analyze(&valid_source()).expect("valid source should analyze");
        let selection = selection_range_for_position(
            &analysis,
            Position {
                line: 5,
                character: 10,
            },
        )
        .expect("selection range should exist");

        assert_eq!(selection.range.start.line, 5);
        assert!(selection.parent.is_some());
    }

    #[test]
    fn publish_diagnostics_includes_related_information_for_duplicates() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let server_thread = thread::spawn(move || run_server(&server_connection));

        send_request(
            &client_connection,
            RequestId::from(20_i32),
            Initialize::METHOD,
            InitializeParams {
                capabilities: ClientCapabilities::default(),
                ..InitializeParams::default()
            },
        )?;
        let _ = recv_response(&client_connection)?;

        send_notification_message(
            &client_connection,
            Initialized::METHOD,
            InitializedParams {},
        )?;

        let uri: Uri = "file:///workspace/Achitekfile"
            .parse()
            .context("failed to parse test URI")?;

        send_notification_message(
            &client_connection,
            DidOpenTextDocument::METHOD,
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "achitek".to_owned(),
                    version: 1,
                    text: duplicate_prompt_source(),
                },
            },
        )?;

        let diagnostics = recv_publish_diagnostics(&client_connection)?;
        let duplicate = diagnostics
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains("duplicate prompt"))
            .expect("expected duplicate diagnostic");
        let original = diagnostics
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains("previous definition of prompt"))
            .expect("expected original diagnostic");
        assert_eq!(
            duplicate.message,
            "duplicate prompt `project_name`; first defined at line 6"
        );
        assert_eq!(
            original.message,
            "previous definition of prompt `project_name` here"
        );
        assert_eq!(original.severity, Some(DiagnosticSeverity::HINT));
        assert_eq!(original.range.start.line, 5);
        let related = duplicate
            .related_information
            .as_ref()
            .expect("duplicate diagnostic should include related information");
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].location.uri, uri);
        assert_eq!(related[0].location.range.start.line, 5);
        let original_related = original
            .related_information
            .as_ref()
            .expect("original diagnostic should include related information");
        assert_eq!(original_related.len(), 1);
        assert_eq!(original_related[0].location.uri, uri);
        assert_eq!(original_related[0].location.range.start.line, 9);

        send_request(
            &client_connection,
            RequestId::from(21_i32),
            "shutdown",
            json!({}),
        )?;
        let _ = recv_response(&client_connection)?;
        send_notification_message(&client_connection, "exit", json!({}))?;

        server_thread
            .join()
            .expect("server thread should not panic")
            .context("server loop should exit cleanly")?;

        Ok(())
    }

    fn send_request<P: serde::Serialize>(
        connection: &Connection,
        id: RequestId,
        method: &'static str,
        params: P,
    ) -> anyhow::Result<()> {
        let request = Request::new(id, method.to_owned(), params);
        connection
            .sender
            .send(Message::Request(request))
            .with_context(|| format!("failed to send `{method}` request"))?;
        Ok(())
    }

    fn send_notification_message<P: serde::Serialize>(
        connection: &Connection,
        method: &'static str,
        params: P,
    ) -> anyhow::Result<()> {
        let notification = Notification::new(method.to_owned(), params);
        connection
            .sender
            .send(Message::Notification(notification))
            .with_context(|| format!("failed to send `{method}` notification"))?;
        Ok(())
    }

    fn recv_response(connection: &Connection) -> anyhow::Result<Response> {
        match recv_message(connection)? {
            Message::Response(response) => Ok(response),
            message => anyhow::bail!("expected response, got {message:?}"),
        }
    }

    fn recv_publish_diagnostics(
        connection: &Connection,
    ) -> anyhow::Result<PublishDiagnosticsParams> {
        match recv_message(connection)? {
            Message::Notification(notification)
                if notification.method == "textDocument/publishDiagnostics" =>
            {
                serde_json::from_value(notification.params)
                    .context("failed to deserialize publishDiagnostics params")
            }
            message => anyhow::bail!("expected publishDiagnostics notification, got {message:?}"),
        }
    }

    fn recv_message(connection: &Connection) -> anyhow::Result<Message> {
        connection
            .receiver
            .recv_timeout(Duration::from_secs(2))
            .context("timed out waiting for server message")
    }

    fn shutdown_server(connection: &Connection, request_id: RequestId) -> anyhow::Result<()> {
        send_request(connection, request_id, "shutdown", json!({}))?;
        let response = recv_response(connection)?;
        assert!(response.error.is_none());
        send_notification_message(connection, "exit", json!({}))
    }

    fn invalid_source() -> String {
        r#"blueprint {
  version = "1.0.0"
  name = "broken"

prompt "project_name" {
  type = string
}
"#
        .to_owned()
    }

    fn valid_source() -> String {
        r#"blueprint {
  version = "1.0.0"
  name = "minimal"
}

prompt "project_name" {
  type = string
  help = "Project name"
}
"#
        .to_owned()
    }

    fn definition_source() -> String {
        r#"blueprint {
  version = "1.0.0"
  name = "minimal"
}

prompt "project_name" {
  type = string
  help = "Project name"
}

prompt "kind" {
  type = string
  help = "Kind"
  depends_on = project_name
}
"#
        .to_owned()
    }

    fn duplicate_prompt_source() -> String {
        r#"blueprint {
  version = "1.0.0"
  name = "minimal"
}

prompt "project_name" {
  type = string
}

prompt "project_name" {
  type = string
}
"#
        .to_owned()
    }
}
