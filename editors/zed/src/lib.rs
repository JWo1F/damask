//! Zed extension for RSC: registers the `.rsc` language (grammar + queries in
//! `languages/rsc/`) and launches the `rsc-lsp` language server.

use zed_extension_api::{self as zed, Result};

struct RscExtension;

impl zed::Extension for RscExtension {
    fn new() -> Self {
        RscExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        // `rsc-lsp` is installed by the user (e.g. `cargo install rsc-lsp`) and
        // found on PATH. RSC has no downloadable prebuilt server binary.
        let path = worktree.which("rsc-lsp").ok_or_else(|| {
            "rsc-lsp not found on PATH — install it with `cargo install rsc-lsp`".to_string()
        })?;

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(RscExtension);
