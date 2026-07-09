//! A minimal Language Server Protocol *client* that drives a child language
//! server (rust-analyzer, or an HTML server) over stdio.
//!
//! `rsc-lsp` is itself a server (to the editor), but to give template code real
//! intelligence it becomes a client to a downstream server: it spawns the
//! binary, performs the `initialize` handshake, feeds it documents (overlay
//! `.rs` files or HTML skeletons), and forwards hover / completion / definition
//! requests, mapping positions in and results out.
//!
//! The design is deliberately small: one background task reads framed messages
//! and routes responses to the waiting request by id, buffers
//! `publishDiagnostics` by URI, and auto-replies to any server→client request so
//! the downstream server never stalls waiting on us.

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, mpsc, oneshot};
use tower_lsp::lsp_types::Url;

type Pending = Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>;
type DiagStore = Arc<Mutex<HashMap<Url, Vec<Value>>>>;
type SharedStdin = Arc<Mutex<ChildStdin>>;

/// Sent (best-effort) whenever rust-analyzer publishes diagnostics for a URI, so
/// the owner can map and republish them without polling.
pub type DiagNotify = mpsc::UnboundedSender<Url>;

/// A running downstream language-server child (rust-analyzer or an HTML server)
/// and the plumbing to talk to it.
pub struct LspClient {
    stdin: SharedStdin,
    next_id: AtomicI64,
    pending: Pending,
    diagnostics: DiagStore,
    /// `languageId` sent on `didOpen` — `"rust"` or `"html"`.
    language_id: &'static str,
    // Kept so the child is killed and the reader task aborted on drop.
    child: Child,
    reader: tokio::task::JoinHandle<()>,
}

impl Drop for LspClient {
    fn drop(&mut self) {
        self.reader.abort();
        // Best-effort: ask the child to exit.
        let _ = self.child.start_kill();
    }
}

impl LspClient {
    /// rust-analyzer rooted at `root` (the directory with the workspace/crate
    /// `Cargo.toml`), configured for overlay analysis only — no build scripts,
    /// proc-macro expansion, or check-on-save, so indexing is fast and nothing
    /// runs `cargo`.
    pub async fn rust_analyzer(
        root: &Path,
        on_diagnostics: Option<DiagNotify>,
    ) -> io::Result<Self> {
        let init = json!({
            "cargo": { "buildScripts": { "enable": false } },
            "procMacro": { "enable": false },
            "checkOnSave": false
        });
        Self::spawn("rust-analyzer", &[], "rust", root, init, on_diagnostics).await
    }

    /// An HTML language server (`vscode-html-language-server --stdio`) rooted at
    /// `root`. Used for markup intelligence on the HTML skeleton.
    pub async fn html(root: &Path, on_diagnostics: Option<DiagNotify>) -> io::Result<Self> {
        Self::spawn(
            "vscode-html-language-server",
            &["--stdio"],
            "html",
            root,
            json!({}),
            on_diagnostics,
        )
        .await
    }

    /// Spawn `command args…`, complete the `initialize` handshake at `root` with
    /// the given server-specific `init_options`, and return the ready client.
    /// `on_diagnostics`, if given, receives a URI whenever the server publishes
    /// diagnostics for it.
    async fn spawn(
        command: &str,
        args: &[&str],
        language_id: &'static str,
        root: &Path,
        init_options: Value,
        on_diagnostics: Option<DiagNotify>,
    ) -> io::Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;

        let stdin: SharedStdin = Arc::new(Mutex::new(child.stdin.take().expect("piped stdin")));
        let stdout = child.stdout.take().expect("piped stdout");

        let pending: Pending = Default::default();
        let diagnostics: DiagStore = Default::default();
        let reader = tokio::spawn(reader_loop(
            BufReader::new(stdout),
            pending.clone(),
            diagnostics.clone(),
            stdin.clone(),
            on_diagnostics,
        ));

        let client = LspClient {
            stdin,
            next_id: AtomicI64::new(1),
            pending,
            diagnostics,
            language_id,
            child,
            reader,
        };
        client.initialize(root, init_options).await?;
        Ok(client)
    }

    async fn initialize(&self, root: &Path, init_options: Value) -> io::Result<()> {
        let root_uri = Url::from_directory_path(root)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "root is not absolute"))?;
        let params = json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "hover": { "contentFormat": ["markdown", "plaintext"] },
                    "completion": { "completionItem": { "snippetSupport": false } },
                    "definition": { "linkSupport": false },
                    "publishDiagnostics": {}
                }
            },
            "initializationOptions": init_options
        });
        self.request("initialize", params).await?;
        self.notify("initialized", json!({})).await?;
        Ok(())
    }

    /// Send a request and await its `result` (or `Value::Null` on error).
    pub async fn request(&self, method: &str, params: Value) -> io::Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        write_message(&mut *self.stdin.lock().await, &msg).await?;
        rx.await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "rust-analyzer closed"))
    }

    /// Send a notification (no response expected).
    pub async fn notify(&self, method: &str, params: Value) -> io::Result<()> {
        let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        write_message(&mut *self.stdin.lock().await, &msg).await
    }

    pub async fn did_open(&self, uri: &Url, version: i64, text: &str) -> io::Result<()> {
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri, "languageId": self.language_id, "version": version, "text": text
                }
            }),
        )
        .await
    }

    pub async fn did_change(&self, uri: &Url, version: i64, text: &str) -> io::Result<()> {
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [ { "text": text } ]
            }),
        )
        .await
    }

    pub async fn hover(&self, uri: &Url, line: u32, character: u32) -> io::Result<Value> {
        self.request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        )
        .await
    }

    pub async fn completion(&self, uri: &Url, line: u32, character: u32) -> io::Result<Value> {
        self.request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        )
        .await
    }

    pub async fn definition(&self, uri: &Url, line: u32, character: u32) -> io::Result<Value> {
        self.request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        )
        .await
    }

    /// The most recent diagnostics rust-analyzer published for `uri`.
    pub async fn diagnostics(&self, uri: &Url) -> Vec<Value> {
        self.diagnostics
            .lock()
            .await
            .get(uri)
            .cloned()
            .unwrap_or_default()
    }
}

async fn reader_loop(
    mut r: BufReader<ChildStdout>,
    pending: Pending,
    diagnostics: DiagStore,
    stdin: SharedStdin,
    on_diagnostics: Option<DiagNotify>,
) {
    while let Ok(Some(msg)) = read_message(&mut r).await {
        handle_message(msg, &pending, &diagnostics, &stdin, on_diagnostics.as_ref()).await;
    }
    // On EOF, fail any in-flight requests so their awaiters don't hang forever.
    pending.lock().await.clear();
}

async fn handle_message(
    msg: Value,
    pending: &Pending,
    diagnostics: &DiagStore,
    stdin: &SharedStdin,
    on_diagnostics: Option<&DiagNotify>,
) {
    let has_method = msg.get("method").is_some();
    if let Some(id) = msg.get("id").and_then(Value::as_i64) {
        if !has_method {
            // A response to one of our requests.
            if let Some(tx) = pending.lock().await.remove(&id) {
                let _ = tx.send(msg.get("result").cloned().unwrap_or(Value::Null));
            }
            return;
        }
        // A server→client request (e.g. workDoneProgress/create,
        // registerCapability). Reply with a null result so rust-analyzer
        // proceeds; we don't need the capability it's negotiating.
        let reply = json!({ "jsonrpc": "2.0", "id": id, "result": null });
        let _ = write_message(&mut *stdin.lock().await, &reply).await;
        return;
    }

    // A notification. Only `publishDiagnostics` is of interest.
    if msg.get("method").and_then(Value::as_str) != Some("textDocument/publishDiagnostics") {
        return;
    }
    let Some(params) = msg.get("params") else {
        return;
    };
    let Some(url) = params
        .get("uri")
        .and_then(Value::as_str)
        .and_then(|u| Url::parse(u).ok())
    else {
        return;
    };
    let diags = params
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    diagnostics.lock().await.insert(url.clone(), diags);
    if let Some(tx) = on_diagnostics {
        let _ = tx.send(url);
    }
}

/// Read one LSP message: `Content-Length` header block, blank line, JSON body.
/// Returns `Ok(None)` at EOF.
async fn read_message(r: &mut BufReader<ChildStdout>) -> io::Result<Option<Value>> {
    let mut content_len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).await? == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_len = rest.trim().parse().ok();
        }
    }
    let len =
        content_len.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing length"))?;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

async fn write_message(w: &mut ChildStdin, v: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(v)?;
    w.write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .await?;
    w.write_all(&body).await?;
    w.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtual_file::VirtualFile;
    use rsc_template::LineIndex;

    /// End-to-end proof of the architecture: spawn a real rust-analyzer against a
    /// throwaway crate, overlay the paired `.rs` with the lowered template, and
    /// confirm a hover at the template's `self.name` resolves to the field's
    /// type. Ignored by default — it builds an index and needs rust-analyzer on
    /// PATH. Run with: `cargo test -p rsc-lsp -- --ignored ra_hover`.
    #[tokio::test]
    #[ignore = "spawns rust-analyzer; slow, needs network-free crate index"]
    async fn ra_hover_resolves_self_field_through_overlay() {
        // A minimal crate that depends on `rsc`, laid out in a temp dir.
        let dir = std::env::temp_dir().join(format!("rsc-lsp-ra-test-{}", std::process::id()));
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let rsc_crate = concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/rsc");
        std::fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"ra_probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
                 [dependencies]\nrsc = {{ path = {rsc_crate:?} }}\n",
            ),
        )
        .unwrap();
        let rs = "use rsc::Component;\n\n#[derive(Component)]\npub struct Greeting {\n    pub name: String,\n}\n";
        std::fs::write(src.join("lib.rs"), rs).unwrap();

        let template = rsc_template::parse("Hello {self.name}!").unwrap();
        let vf = VirtualFile::build(rs, "Greeting", &template).unwrap();
        let rs_uri = Url::from_file_path(src.join("lib.rs")).unwrap();

        let client = LspClient::rust_analyzer(&dir, None).await.unwrap();
        client.did_open(&rs_uri, 1, &vf.text).await.unwrap();

        // Position of `name` in the .rsc, mapped to the overlay, then to LSP
        // line/character on the overlay text.
        let rsc = "Hello {self.name}!";
        let name_at = rsc.find("name").unwrap();
        let ov = vf.source_to_overlay(name_at).unwrap();
        let (line, ch) = LineIndex::new(&vf.text).line_col(&vf.text, ov);

        // rust-analyzer loads the sysroot and crate graph before type inference
        // is available; until then it answers with `{unknown}`. Poll until the
        // field's real type resolves (or we give up).
        let mut text = String::new();
        for _ in 0..60 {
            let hover = client.hover(&rs_uri, line, ch).await.unwrap();
            text = serde_json::to_string(&hover).unwrap();
            if text.contains("String") {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            text.contains("String"),
            "hover on self.name should mention its String type; got: {text}",
        );
    }

    /// The other half of the proxy: completion inside a `{ … }` tag offers the
    /// component's members. Confirms member completion flows through the overlay,
    /// including the fragment-boundary cursor mapping after `self.`.
    #[tokio::test]
    #[ignore = "spawns rust-analyzer; slow, needs network-free crate index"]
    async fn ra_completion_offers_self_members_through_overlay() {
        let dir = std::env::temp_dir().join(format!("rsc-lsp-ra-compl-{}", std::process::id()));
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let rsc_crate = concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/rsc");
        std::fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"ra_probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
                 [dependencies]\nrsc = {{ path = {rsc_crate:?} }}\n",
            ),
        )
        .unwrap();
        let rs = "use rsc::Component;\n\n#[derive(Component)]\npub struct Greeting {\n    pub name: String,\n}\n";
        std::fs::write(src.join("lib.rs"), rs).unwrap();
        let rs_uri = Url::from_file_path(src.join("lib.rs")).unwrap();

        // Cursor at the end of `self.n` — a fragment boundary, as in the editor.
        let rsc = "Hello {self.n}!";
        let vf = VirtualFile::build(rs, "Greeting", &rsc_template::parse(rsc).unwrap()).unwrap();
        let cursor = rsc.find("self.n").unwrap() + "self.n".len();
        let ov = vf.source_to_overlay_boundary(cursor).unwrap();
        let (line, ch) = LineIndex::new(&vf.text).line_col(&vf.text, ov);

        let client = LspClient::rust_analyzer(&dir, None).await.unwrap();
        client.did_open(&rs_uri, 1, &vf.text).await.unwrap();

        let mut found = false;
        for _ in 0..60 {
            let raw = client.completion(&rs_uri, line, ch).await.unwrap();
            if serde_json::to_string(&raw).unwrap().contains("\"name\"") {
                found = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            found,
            "completion after `self.` should offer the `name` field"
        );
    }

    /// The HTML half of the proxy: an HTML server offers tag completion on the
    /// skeleton. Skips gracefully when the server isn't installed; ignored by
    /// default. Run with a server on PATH:
    /// `cargo test -p rsc-lsp -- --ignored html_completion`.
    #[tokio::test]
    #[ignore = "requires vscode-html-language-server on PATH"]
    async fn html_completion_offers_standard_tags() {
        let dir = std::env::temp_dir();
        let uri = Url::from_file_path(dir.join("probe.rsc")).unwrap();
        let Ok(client) = LspClient::html(&dir, None).await else {
            eprintln!("no HTML server installed; skipping");
            return;
        };
        // A document with the cursor just after `<`, where tag completion fires.
        client.did_open(&uri, 1, "<").await.unwrap();

        let mut found = false;
        for _ in 0..20 {
            let raw = client.completion(&uri, 0, 1).await.unwrap();
            if serde_json::to_string(&raw).unwrap().contains("div") {
                found = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        assert!(
            found,
            "HTML completion should offer standard tags like `div`"
        );
    }
}
