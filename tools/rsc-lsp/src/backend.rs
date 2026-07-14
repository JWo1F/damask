use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rsc_template::{LineIndex, parse};
use tokio::sync::{Mutex as AsyncMutex, mpsc};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, jsonrpc::Result};

use crate::analysis::{Context, cursor_context, in_code_tag, is_self_access};
use crate::introspect;
use crate::lsp_client::LspClient;
use crate::virtual_file::VirtualFile;

/// A component's overlay as currently synced to rust-analyzer, kept so the
/// diagnostics consumer can map published ranges back to the `.rsc`.
struct OverlayState {
    vf: Arc<VirtualFile>,
    rsc_uri: Url,
    rsc_text: String,
    version: i64,
}

/// Everything needed to forward one request through the proxy.
struct OverlayHandle {
    client: Arc<LspClient>,
    vf: Arc<VirtualFile>,
    rs_uri: Url,
}

/// The shared HTML language server, spawned lazily. Once we've learned it isn't
/// installed we stop retrying (spawning is slow) and simply skip HTML features.
enum HtmlSlot {
    Untried,
    Unavailable,
    Ready(Arc<LspClient>),
}

pub struct Backend {
    client: Client,
    /// Open documents by URI (full-sync text).
    docs: Mutex<HashMap<Url, String>>,
    /// One rust-analyzer per workspace root, spawned on demand.
    ra: AsyncMutex<HashMap<PathBuf, Arc<LspClient>>>,
    /// Synced overlays keyed by the paired `.rs` URI. Shared so the per-client
    /// diagnostics task can translate ranges without holding `&self`.
    overlays: Arc<AsyncMutex<HashMap<Url, OverlayState>>>,
    /// The HTML language server (one for the whole session — it's stateless).
    html: AsyncMutex<HtmlSlot>,
    /// HTML skeleton document versions, keyed by `.rsc` URI.
    html_versions: AsyncMutex<HashMap<Url, i64>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Backend {
            client,
            docs: Mutex::new(HashMap::new()),
            ra: AsyncMutex::new(HashMap::new()),
            overlays: Arc::new(AsyncMutex::new(HashMap::new())),
            html: AsyncMutex::new(HtmlSlot::Untried),
            html_versions: AsyncMutex::new(HashMap::new()),
        }
    }

    fn text_of(&self, uri: &Url) -> Option<String> {
        self.docs.lock().unwrap().get(uri).cloned()
    }

    /// Publish parse diagnostics for a template (rust-analyzer diagnostics are
    /// added later, when the overlay is analysed — see the consumer in
    /// [`ra_client_for`](Self::ra_client_for)).
    async fn publish_parse_diagnostics(&self, uri: Url, text: &str) {
        self.client
            .publish_diagnostics(uri, parse_diagnostics(text), None)
            .await;
    }

    /// Get (or spawn) the rust-analyzer for `root`, wiring its diagnostics back
    /// to the editor through a background task.
    async fn ra_client_for(&self, root: &Path) -> Option<Arc<LspClient>> {
        let mut map = self.ra.lock().await;
        if let Some(existing) = map.get(root) {
            return Some(existing.clone());
        }
        let (tx, mut rx) = mpsc::unbounded_channel::<Url>();
        let client = Arc::new(LspClient::rust_analyzer(root, Some(tx)).await.ok()?);
        map.insert(root.to_path_buf(), client.clone());
        drop(map);

        // Republish rust-analyzer's diagnostics onto the matching `.rsc`.
        let editor = self.client.clone();
        let overlays = self.overlays.clone();
        let ra = client.clone();
        tokio::spawn(async move {
            while let Some(rs_uri) = rx.recv().await {
                publish_ra_diagnostics(&editor, &overlays, &ra, &rs_uri).await;
            }
        });
        Some(client)
    }

    /// Build and sync the overlay for the template at `rsc_uri`, returning a
    /// handle for forwarding a request. Returns `None` when the document has no
    /// paired component, isn't in a cargo project, or fails to lower.
    ///
    /// `require_mapped` is a source offset the request is about. It is checked
    /// against the freshly built overlay *before* rust-analyzer is spawned or
    /// synced, so a request over plain markup costs a parse and nothing more.
    async fn ensure_overlay(
        &self,
        rsc_uri: &Url,
        rsc_text: &str,
        require_mapped: Option<usize>,
    ) -> Option<OverlayHandle> {
        let rsc_path = rsc_uri.to_file_path().ok()?;
        let (rs_path, struct_name) = introspect::paired_rs(&rsc_path)?;
        let root = introspect::project_root(&rs_path)?;
        let rs_uri = Url::from_file_path(&rs_path).ok()?;
        let rs_src = std::fs::read_to_string(&rs_path).ok()?;
        let template = parse(rsc_text).ok()?;
        let vf = Arc::new(VirtualFile::build(&rs_src, &struct_name, &template).ok()?);

        // Not a position that lowers to Rust — leave it to the HTML server.
        if let Some(offset) = require_mapped {
            vf.source_to_overlay(offset)?;
        }

        let client = self.ra_client_for(&root).await?;

        // Sync the overlay: open the first time, change thereafter.
        let mut overlays = self.overlays.lock().await;
        let first_open = !overlays.contains_key(&rs_uri);
        let version = overlays.get(&rs_uri).map(|s| s.version + 1).unwrap_or(1);
        overlays.insert(
            rs_uri.clone(),
            OverlayState {
                vf: vf.clone(),
                rsc_uri: rsc_uri.clone(),
                rsc_text: rsc_text.to_string(),
                version,
            },
        );
        drop(overlays);

        if first_open {
            client.did_open(&rs_uri, version, &vf.text).await.ok()?;
        } else {
            client.did_change(&rs_uri, version, &vf.text).await.ok()?;
        }

        Some(OverlayHandle { client, vf, rs_uri })
    }

    /// Hover via rust-analyzer, with the range mapped back to the template.
    async fn proxy_hover(&self, rsc_uri: &Url, pos: Position) -> Option<Hover> {
        let rsc_text = self.text_of(rsc_uri)?;
        let offset = offset_at(&rsc_text, pos);
        let h = self
            .ensure_overlay(rsc_uri, &rsc_text, Some(offset))
            .await?;
        let ov = map_pos_to_overlay(&h.vf, &rsc_text, pos)?;
        let raw = h
            .client
            .hover(&h.rs_uri, ov.line, ov.character)
            .await
            .ok()?;
        let mut hover: Hover = serde_json::from_value(raw).ok()?;
        if let Some(range) = hover.range {
            hover.range = map_range_to_rsc(&h.vf, &rsc_text, range);
        }
        Some(hover)
    }

    /// Completion via rust-analyzer for code inside a `{ … }` tag.
    async fn proxy_completion(&self, rsc_uri: &Url, pos: Position) -> Option<Vec<CompletionItem>> {
        let rsc_text = self.text_of(rsc_uri)?;
        let h = self.ensure_overlay(rsc_uri, &rsc_text, None).await?;
        // Completion fires with the cursor at a fragment boundary (after `.`),
        // so use the boundary-aware mapping.
        let ov_off = h.vf.source_to_overlay_boundary(offset_at(&rsc_text, pos))?;
        let ov = position_at(&h.vf.text, ov_off);
        let raw = h
            .client
            .completion(&h.rs_uri, ov.line, ov.character)
            .await
            .ok()?;
        let items = match serde_json::from_value::<CompletionResponse>(raw).ok()? {
            CompletionResponse::Array(items) => items,
            CompletionResponse::List(list) => list.items,
        };
        // Strip overlay-coordinate text edits; a clean insert is always safe and
        // avoids leaking virtual-file positions into the editor.
        Some(items.into_iter().map(sanitize_completion).collect())
    }

    /// Go-to-definition via rust-analyzer, mapping any target that lands back in
    /// the appended template body onto the `.rsc`.
    async fn proxy_definition(
        &self,
        rsc_uri: &Url,
        pos: Position,
    ) -> Option<GotoDefinitionResponse> {
        let rsc_text = self.text_of(rsc_uri)?;
        let offset = offset_at(&rsc_text, pos);
        let h = self
            .ensure_overlay(rsc_uri, &rsc_text, Some(offset))
            .await?;
        let ov = map_pos_to_overlay(&h.vf, &rsc_text, pos)?;
        let raw = h
            .client
            .definition(&h.rs_uri, ov.line, ov.character)
            .await
            .ok()?;
        if raw.is_null() {
            return None;
        }
        let resp: GotoDefinitionResponse = serde_json::from_value(raw).ok()?;
        Some(remap_definition(resp, &h, rsc_uri, &rsc_text))
    }

    /// The HTML language server, spawned on first use. Returns `None` (and stops
    /// trying) if it isn't installed.
    async fn html_client(&self, root: &Path) -> Option<Arc<LspClient>> {
        let mut slot = self.html.lock().await;
        match &*slot {
            HtmlSlot::Ready(c) => return Some(c.clone()),
            HtmlSlot::Unavailable => return None,
            HtmlSlot::Untried => {}
        }
        match LspClient::html(root, None).await {
            Ok(c) => {
                let c = Arc::new(c);
                *slot = HtmlSlot::Ready(c.clone());
                Some(c)
            }
            Err(_) => {
                *slot = HtmlSlot::Unavailable;
                None
            }
        }
    }

    /// Sync the HTML skeleton for `rsc_uri` to the HTML server (open, then
    /// change). The skeleton shares the template's offsets, so no map is kept.
    async fn sync_html(&self, client: &LspClient, rsc_uri: &Url, rsc_text: &str) {
        let mut versions = self.html_versions.lock().await;
        let first_open = !versions.contains_key(rsc_uri);
        let version = versions.get(rsc_uri).map(|v| v + 1).unwrap_or(1);
        versions.insert(rsc_uri.clone(), version);
        drop(versions);

        let skeleton = crate::html_doc::html_skeleton(rsc_text);
        let _ = if first_open {
            client.did_open(rsc_uri, version, &skeleton).await
        } else {
            client.did_change(rsc_uri, version, &skeleton).await
        };
    }

    /// A handle plus everything an HTML request needs. `None` in a code tag
    /// (rust-analyzer handles those) or when no HTML server is available.
    async fn html_handle(&self, rsc_uri: &Url, pos: Position) -> Option<Arc<LspClient>> {
        let rsc_text = self.text_of(rsc_uri)?;
        if in_code_tag(&rsc_text, offset_at(&rsc_text, pos)) {
            return None;
        }
        let root = rsc_uri.to_file_path().ok()?.parent()?.to_path_buf();
        let client = self.html_client(&root).await?;
        self.sync_html(&client, rsc_uri, &rsc_text).await;
        Some(client)
    }

    /// Hover on markup via the HTML server. Positions are the identity, so the
    /// result (including its range) is already in `.rsc` coordinates.
    async fn proxy_html_hover(&self, rsc_uri: &Url, pos: Position) -> Option<Hover> {
        let client = self.html_handle(rsc_uri, pos).await?;
        let raw = client.hover(rsc_uri, pos.line, pos.character).await.ok()?;
        serde_json::from_value(raw).ok()
    }

    /// Completion on markup (HTML tags and attributes) via the HTML server.
    async fn proxy_html_completion(
        &self,
        rsc_uri: &Url,
        pos: Position,
    ) -> Option<Vec<CompletionItem>> {
        let client = self.html_handle(rsc_uri, pos).await?;
        let raw = client
            .completion(rsc_uri, pos.line, pos.character)
            .await
            .ok()?;
        let items = match serde_json::from_value::<CompletionResponse>(raw).ok()? {
            CompletionResponse::Array(items) => items,
            CompletionResponse::List(list) => list.items,
        };
        Some(items.into_iter().map(sanitize_completion).collect())
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
                        ":".to_string(),
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
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
        self.publish_parse_diagnostics(doc.uri.clone(), &doc.text)
            .await;
        // Warm the overlay so rust-analyzer starts analysing immediately.
        self.ensure_overlay(&doc.uri, &doc.text, None).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // FULL sync: the last change carries the whole document.
        if let Some(change) = params.content_changes.into_iter().last() {
            let uri = params.text_document.uri;
            self.docs
                .lock()
                .unwrap()
                .insert(uri.clone(), change.text.clone());
            self.publish_parse_diagnostics(uri.clone(), &change.text)
                .await;
            self.ensure_overlay(&uri, &change.text, None).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.docs.lock().unwrap().remove(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let pos = params.text_document_position;
        let uri = pos.text_document.uri;
        let position = pos.position;
        let Some(text) = self.text_of(&uri) else {
            return Ok(None);
        };
        let offset = offset_at(&text, position);

        // Inside a `{ … }` tag, rust-analyzer gives far richer results than the
        // static introspection can; elsewhere (element names, component
        // attributes) the completions are RSC-specific and stay local.
        match cursor_context(&text, offset) {
            Context::SelfMember | Context::UsePath => {
                if let Some(items) = self.proxy_completion(&uri, position).await {
                    return Ok(Some(CompletionResponse::Array(items)));
                }
                // Fall back to the static self-member list if the proxy is
                // unavailable (no cargo project, rust-analyzer missing, …).
                let Some(path) = uri.to_file_path().ok() else {
                    return Ok(None);
                };
                let items = match cursor_context(&text, offset) {
                    Context::UsePath => use_path_items(&path),
                    _ => self_member_items(&path, &text[..offset]),
                };
                Ok(Some(CompletionResponse::Array(items)))
            }
            // After `<` both an RSC component and a plain HTML element are
            // valid, so the local component list is merged with the HTML
            // server's tags rather than replacing it. The `sort_text` prefixes
            // rank components above HTML tags while leaving each group's own
            // ordering intact.
            Context::ElementName => {
                let Some(path) = uri.to_file_path().ok() else {
                    return Ok(None);
                };
                let mut items = component_name_items(&path);
                for (i, item) in items.iter_mut().enumerate() {
                    item.sort_text = Some(format!("0{i:04}"));
                }
                if let Some(html) = self.proxy_html_completion(&uri, position).await {
                    items.extend(html.into_iter().map(|mut item| {
                        item.sort_text = Some(format!(
                            "1{}",
                            item.sort_text.as_deref().unwrap_or(&item.label)
                        ));
                        item
                    }));
                }
                Ok(Some(CompletionResponse::Array(items)))
            }
            Context::Attribute(name) => {
                let Some(path) = uri.to_file_path().ok() else {
                    return Ok(None);
                };
                Ok(Some(CompletionResponse::Array(attribute_items(
                    &path, &name,
                ))))
            }
            // Plain markup (HTML element attributes, text) — the HTML server
            // handles tag/attribute completion here.
            Context::None => {
                if let Some(items) = self.proxy_html_completion(&uri, position).await {
                    return Ok(Some(CompletionResponse::Array(items)));
                }
                Ok(None)
            }
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let pos = params.text_document_position_params;
        let uri = pos.text_document.uri;
        // Code inside `{ … }` → rust-analyzer; markup → the HTML server.
        if let Some(hover) = self.proxy_hover(&uri, pos.position).await {
            return Ok(Some(hover));
        }
        if let Some(hover) = self.proxy_html_hover(&uri, pos.position).await {
            return Ok(Some(hover));
        }
        // Fallback: static introspection of the paired component.
        Ok(fallback_hover(&uri, pos.position, self.text_of(&uri)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let pos = params.text_document_position_params;
        Ok(self
            .proxy_definition(&pos.text_document.uri, pos.position)
            .await)
    }
}

// ---------------------------------------------------------------------------
// Position mapping helpers.
// ---------------------------------------------------------------------------

fn offset_at(text: &str, pos: Position) -> usize {
    LineIndex::new(text).offset(text, pos.line, pos.character)
}

fn position_at(text: &str, offset: usize) -> Position {
    let (line, character) = LineIndex::new(text).line_col(text, offset);
    Position { line, character }
}

fn map_pos_to_overlay(vf: &VirtualFile, rsc_text: &str, pos: Position) -> Option<Position> {
    let off = offset_at(rsc_text, pos);
    let ov = vf.source_to_overlay(off)?;
    Some(position_at(&vf.text, ov))
}

/// Map an overlay range back to the template. Ranges from rust-analyzer sit
/// within a single identifier, hence a single mapped fragment, so the length is
/// preserved and the end follows the start by the same delta.
fn map_range_to_rsc(vf: &VirtualFile, rsc_text: &str, range: Range) -> Option<Range> {
    let start_off = offset_at(&vf.text, range.start);
    let end_off = offset_at(&vf.text, range.end);
    let s = vf.overlay_to_source(start_off)?;
    let e = s + end_off.saturating_sub(start_off);
    Some(Range {
        start: position_at(rsc_text, s),
        end: position_at(rsc_text, e),
    })
}

/// Reduce a rust-analyzer completion item to a form with no overlay-relative
/// edits: a plain insert of its label (or the edit's replacement text).
fn sanitize_completion(item: CompletionItem) -> CompletionItem {
    let insert_text = item.insert_text.clone().or_else(|| match &item.text_edit {
        Some(CompletionTextEdit::Edit(e)) => Some(e.new_text.clone()),
        Some(CompletionTextEdit::InsertAndReplace(e)) => Some(e.new_text.clone()),
        None => None,
    });
    CompletionItem {
        label: item.label,
        kind: item.kind,
        detail: item.detail,
        documentation: item.documentation,
        sort_text: item.sort_text,
        filter_text: item.filter_text,
        preselect: item.preselect,
        insert_text,
        text_edit: None,
        additional_text_edits: None,
        ..Default::default()
    }
}

/// Rewrite definition targets: any location pointing into the overlay's appended
/// body is remapped onto the `.rsc`; targets in the user's real code (before the
/// appended region, or in other files) are already valid and pass through.
fn remap_definition(
    resp: GotoDefinitionResponse,
    h: &OverlayHandle,
    rsc_uri: &Url,
    rsc_text: &str,
) -> GotoDefinitionResponse {
    let remap_location = |mut loc: Location| -> Location {
        let start_off = offset_at(&h.vf.text, loc.range.start);
        let in_appended_body = loc.uri == h.rs_uri && h.vf.in_body(start_off);
        if let Some(range) = in_appended_body
            .then(|| map_range_to_rsc(&h.vf, rsc_text, loc.range))
            .flatten()
        {
            loc.uri = rsc_uri.clone();
            loc.range = range;
        }
        loc
    };
    match resp {
        GotoDefinitionResponse::Scalar(loc) => GotoDefinitionResponse::Scalar(remap_location(loc)),
        GotoDefinitionResponse::Array(locs) => {
            GotoDefinitionResponse::Array(locs.into_iter().map(remap_location).collect())
        }
        // Definition links carry target ranges we don't remap yet; return as-is.
        GotoDefinitionResponse::Link(links) => GotoDefinitionResponse::Link(links),
    }
}

/// Translate rust-analyzer's diagnostics for an overlay into `.rsc` diagnostics
/// (only those landing in the appended body) and republish them alongside the
/// template's own parse diagnostics.
async fn publish_ra_diagnostics(
    editor: &Client,
    overlays: &Arc<AsyncMutex<HashMap<Url, OverlayState>>>,
    ra: &LspClient,
    rs_uri: &Url,
) {
    let raw = ra.diagnostics(rs_uri).await;
    let (vf, rsc_uri, rsc_text) = {
        let guard = overlays.lock().await;
        let Some(state) = guard.get(rs_uri) else {
            return;
        };
        (
            state.vf.clone(),
            state.rsc_uri.clone(),
            state.rsc_text.clone(),
        )
    };

    let mut diagnostics = parse_diagnostics(&rsc_text);
    for value in raw {
        let Ok(mut diag) = serde_json::from_value::<Diagnostic>(value) else {
            continue;
        };
        let start_off = offset_at(&vf.text, diag.range.start);
        if !vf.in_body(start_off) {
            continue; // belongs to the user's real code, not the template
        }
        let Some(range) = map_range_to_rsc(&vf, &rsc_text, diag.range) else {
            continue;
        };
        diag.range = range;
        diag.source = Some("rust-analyzer".to_string());
        diagnostics.push(diag);
    }
    editor.publish_diagnostics(rsc_uri, diagnostics, None).await;
}

/// Parse `text` and return its parse-error diagnostics (empty when it parses).
fn parse_diagnostics(text: &str) -> Vec<Diagnostic> {
    match parse(text) {
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
    }
}

// ---------------------------------------------------------------------------
// Static (syn-based) fallbacks — used when the proxy is unavailable.
// ---------------------------------------------------------------------------

fn fallback_hover(uri: &Url, position: Position, text: Option<String>) -> Option<Hover> {
    let text = text?;
    let offset = offset_at(&text, position);
    if !in_code_tag(&text, offset) {
        return None;
    }
    let word = word_at(&text, offset);
    if word.is_empty() {
        return None;
    }
    let info = uri
        .to_file_path()
        .ok()
        .and_then(|p| crate::introspect::for_template(&p))?;
    let member = info.members.iter().find(|m| m.name == word)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("```rust\n{}\n```", member.detail),
        }),
        range: None,
    })
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

/// `<Frame …` — the component's fields. Slots are not fields, so every field is
/// an attribute.
fn attribute_items(path: &Path, component: &str) -> Vec<CompletionItem> {
    let components = introspect::crate_components(path);
    let Some(def) = components.iter().find(|c| c.name == component) else {
        return Vec::new();
    };
    def.fields
        .iter()
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
    use super::*;
    use crate::virtual_file::VirtualFile;

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

    #[test]
    fn sanitize_completion_drops_overlay_edit_keeps_insert() {
        let item = CompletionItem {
            label: "name".to_string(),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range: Range {
                    // An overlay-relative range that must not leak to the editor.
                    start: Position::new(14, 27),
                    end: Position::new(14, 27),
                },
                new_text: "name".to_string(),
            })),
            ..Default::default()
        };
        let clean = sanitize_completion(item);
        assert!(clean.text_edit.is_none());
        assert_eq!(clean.insert_text.as_deref(), Some("name"));
        assert_eq!(clean.label, "name");
    }

    /// An overlay range over a mapped fragment round-trips back to the same span
    /// in the template.
    #[test]
    fn overlay_range_maps_back_to_template() {
        let rs = "pub struct Greeting {\n    pub name: String,\n}\n";
        let rsc = "Hello {self.name}!";
        let vf = VirtualFile::build(rs, "Greeting", &parse(rsc).unwrap()).unwrap();

        let name_at = rsc.find("name").unwrap();
        let ov = vf.source_to_overlay(name_at).unwrap();
        let range = Range {
            start: position_at(&vf.text, ov),
            end: position_at(&vf.text, ov + "name".len()),
        };
        let back = map_range_to_rsc(&vf, rsc, range).unwrap();
        assert_eq!(offset_at(rsc, back.start), name_at);
        assert_eq!(offset_at(rsc, back.end), name_at + "name".len());
    }
}
