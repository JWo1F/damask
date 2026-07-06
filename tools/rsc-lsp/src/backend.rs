use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use rsc_template::{LineIndex, parse};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, jsonrpc::Result};

use crate::analysis::{Context, cursor_context, in_code_tag, is_self_access};
use crate::introspect;

pub struct Backend {
    client: Client,
    /// Open documents by URI (full-sync text).
    docs: Mutex<HashMap<Url, String>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Backend {
            client,
            docs: Mutex::new(HashMap::new()),
        }
    }

    fn text_of(&self, uri: &Url) -> Option<String> {
        self.docs.lock().unwrap().get(uri).cloned()
    }

    /// Parse the document and publish parse diagnostics (or clear them).
    async fn publish_diagnostics(&self, uri: Url, text: &str) {
        let diagnostics = match parse(text) {
            Ok(_) => Vec::new(),
            Err(err) => {
                let index = LineIndex::new(text);
                let (sl, sc) = index.line_col(text, err.span.start);
                let (el, ec) = index.line_col(text, err.span.end);
                vec![Diagnostic {
                    range: Range {
                        start: Position::new(sl, sc),
                        end: Position::new(el, ec),
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("rsc".to_string()),
                    message: err.message,
                    ..Default::default()
                }]
            }
        };
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "rsc-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "<".to_string(),
                        " ".to_string(),
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "rsc-lsp ready")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        self.docs
            .lock()
            .unwrap()
            .insert(doc.uri.clone(), doc.text.clone());
        self.publish_diagnostics(doc.uri, &doc.text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // FULL sync: the last change carries the whole document.
        if let Some(change) = params.content_changes.into_iter().last() {
            let uri = params.text_document.uri;
            self.docs
                .lock()
                .unwrap()
                .insert(uri.clone(), change.text.clone());
            self.publish_diagnostics(uri, &change.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.docs.lock().unwrap().remove(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let pos = params.text_document_position;
        let uri = pos.text_document.uri;
        let Some(text) = self.text_of(&uri) else {
            return Ok(None);
        };
        let Some(path) = uri.to_file_path().ok() else {
            return Ok(None);
        };

        let index = LineIndex::new(&text);
        let offset = index.offset(&text, pos.position.line, pos.position.character);

        let items = match cursor_context(&text, offset) {
            Context::SelfMember => self_member_items(&path, &text[..offset]),
            Context::ElementName => component_name_items(&path),
            Context::Attribute(name) => attribute_items(&path, &name),
            Context::UsePath => use_path_items(&path),
            Context::None => return Ok(None),
        };

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let pos = params.text_document_position_params;
        let uri = pos.text_document.uri;
        let Some(text) = self.text_of(&uri) else {
            return Ok(None);
        };

        let index = LineIndex::new(&text);
        let offset = index.offset(&text, pos.position.line, pos.position.character);
        if !in_code_tag(&text, offset) {
            return Ok(None);
        }

        let word = word_at(&text, offset);
        if word.is_empty() {
            return Ok(None);
        }

        let Some(info) = uri
            .to_file_path()
            .ok()
            .and_then(|p| introspect::for_template(&p))
        else {
            return Ok(None);
        };

        let Some(member) = info.members.iter().find(|m| m.name == word) else {
            return Ok(None);
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("```rust\n{}\n```", member.detail),
            }),
            range: None,
        }))
    }
}

/// `{ self.… }` — the paired component's fields and methods (plus a `self`
/// entry point outside a `self.` access).
fn self_member_items(path: &Path, before: &str) -> Vec<CompletionItem> {
    let Some(info) = introspect::for_template(path) else {
        return Vec::new();
    };
    let mut items: Vec<CompletionItem> = info
        .members
        .iter()
        .map(|m| CompletionItem {
            label: m.name.clone(),
            kind: Some(if m.is_method {
                CompletionItemKind::METHOD
            } else {
                CompletionItemKind::FIELD
            }),
            detail: Some(m.detail.clone()),
            ..Default::default()
        })
        .collect();
    if !is_self_access(before) {
        items.insert(
            0,
            CompletionItem {
                label: "self".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                insert_text: Some("self.".to_string()),
                detail: Some(format!("the {} component", info.struct_name)),
                ..Default::default()
            },
        );
    }
    items
}

/// `<F…` — component names defined in the crate.
fn component_name_items(path: &Path) -> Vec<CompletionItem> {
    introspect::crate_components(path)
        .into_iter()
        .map(|c| CompletionItem {
            label: c.name,
            kind: Some(CompletionItemKind::CLASS),
            detail: Some(c.module_path),
            ..Default::default()
        })
        .collect()
}

/// `<Frame …` — the component's fields (slot `children` excluded).
fn attribute_items(path: &Path, component: &str) -> Vec<CompletionItem> {
    let components = introspect::crate_components(path);
    let Some(def) = components.iter().find(|c| c.name == component) else {
        return Vec::new();
    };
    def.fields
        .iter()
        .filter(|f| f.name != "children")
        .map(|f| CompletionItem {
            label: f.name.clone(),
            kind: Some(CompletionItemKind::FIELD),
            detail: Some(f.detail.clone()),
            insert_text: Some(format!("{}=", f.name)),
            ..Default::default()
        })
        .collect()
}

/// `{use …}` — component paths in the crate.
fn use_path_items(path: &Path) -> Vec<CompletionItem> {
    introspect::crate_components(path)
        .into_iter()
        .map(|c| CompletionItem {
            label: c.module_path,
            kind: Some(CompletionItemKind::MODULE),
            detail: Some(c.name),
            ..Default::default()
        })
        .collect()
}

/// The identifier under (or just before) the cursor.
fn word_at(text: &str, offset: usize) -> String {
    let offset = offset.min(text.len());
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let start = text[..offset]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map(|(i, _)| i)
        .unwrap_or(offset);
    let end = offset
        + text[offset..]
            .char_indices()
            .take_while(|(_, c)| is_word(*c))
            .map(|(i, c)| i + c.len_utf8())
            .last()
            .unwrap_or(0);
    text[start..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::word_at;

    #[test]
    fn word_at_finds_identifier() {
        let t = "{ self.name }";
        let idx = t.find("name").unwrap() + 2; // inside "name"
        assert_eq!(word_at(t, idx), "name");
    }

    #[test]
    fn word_at_empty_between_symbols() {
        let t = "{  }";
        assert_eq!(word_at(t, 2), "");
    }
}
