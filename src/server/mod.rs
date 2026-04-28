//! Server

use crate::{arguments::CommunicationsChannel, capabilities};
use lsp_server::{Connection, IoThreads, Message};
use lsp_types::Uri;
use std::collections::HashMap;

mod handlers;

#[derive(Debug, Clone)]
pub struct Document {
    pub text: String,
}

pub struct Server {
    pub connection: Connection,
    pub io_threads: IoThreads,
    pub documents: HashMap<Uri, Document>,
}

impl Server {
    pub fn new(channel: Option<CommunicationsChannel>) -> Self {
        let (connection, io_threads) = Server::resolve_communications_channel(channel);
        Self {
            connection,
            io_threads,
            documents: HashMap::new(),
        }
    }

    pub fn run(self) -> anyhow::Result<()> {
        let Self {
            connection,
            io_threads,
            documents,
        } = self;

        let init_value = serde_json::json!({
            "capabilities": capabilities::make(),
        });

        let _init_params = match connection.initialize(init_value) {
            Ok(params) => params,
            Err(err) => {
                if err.channel_is_disconnected() {
                    io_threads.join()?;
                }
                return Err(err.into());
            }
        };

        for msg in &connection.receiver {
            match msg {
                Message::Request(req) => {
                    if connection.handle_shutdown(&req)? {
                        break;
                    }

                    match req.method.as_str() {
                        "textDocument/documentSymbol" => {
                            handlers::document_symbol::handle(&connection, &req, &documents)?;
                        }
                        "textDocument/formatting" => {
                            handlers::formatting::handle(&connection, &req, &documents)?;
                        }
                        _ => {}
                    }
                }
                Message::Notification(_note) => {
                    //
                }
                Message::Response(resp) => tracing::error!("[lsp] response: {resp:?}"),
            }
        }

        io_threads.join()?;

        Ok(())
    }

    fn resolve_communications_channel(
        channel: Option<CommunicationsChannel>,
    ) -> (Connection, IoThreads) {
        if let Some(chan) = channel {
            match chan {
                CommunicationsChannel::Stdio => Connection::stdio(),
                _ => {
                    tracing::error!("server does not support communication channel: {}", chan);
                    std::process::exit(0);
                }
            }
        } else {
            tracing::error!("no communication channel provided");
            std::process::exit(0)
        }
    }
}
