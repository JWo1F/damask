//! Zed extension for Damask: registers the `.dmk` language (grammar + queries in
//! `languages/damask/`) and launches the `damask-lsp` language server.

use zed_extension_api::{self as zed, Result};

struct DamaskExtension;

impl zed::Extension for DamaskExtension {
    fn new() -> Self {
        DamaskExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        // `damask-lsp` is installed by the user (e.g. `cargo install damask-lsp`) and
        // found on PATH. Damask has no downloadable prebuilt server binary.
        let path = worktree.which("damask-lsp").ok_or_else(|| {
            "damask-lsp not found on PATH — install it with `cargo install damask-lsp`".to_string()
        })?;

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(DamaskExtension);
