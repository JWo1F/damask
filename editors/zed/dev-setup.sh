#!/usr/bin/env bash
# Prepare the Damask Zed extension for local development.
#
# The Tree-sitter grammar is its own repository, which extension.toml pins by
# revision; Zed clones it directly, so nothing here has to stage it. Grammar
# changes are made and released there, then adopted by bumping that `rev`.
#
# What still needs doing locally is the language server. The extension launches
# the `damask-lsp` installed on PATH, not this checkout, and that binary compiles
# the template lowering in — so a stale one keeps serving results from old
# lowering long after the fix is committed, and restarting the server only
# restarts the old binary.
#
# Re-run whenever you change anything the language server is built from
# (tools/damask-lsp, crates/damask-template, crates/damask).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"

# Reinstall when anything the server is built from is newer than the binary.
# Set DAMASK_SKIP_LSP=1 to skip (the release build takes ~30s).
if [ "${DAMASK_SKIP_LSP:-0}" != "1" ]; then
  installed="$(command -v damask-lsp || true)"
  newest="$(find "$root/tools/damask-lsp" "$root/crates" -name '*.rs' -newer "${installed:-/nonexistent}" -print -quit 2>/dev/null || true)"
  if [ -z "$installed" ] || [ -n "$newest" ]; then
    echo "==> installing damask-lsp (${installed:-not on PATH} is missing or stale)"
    cargo install --path "$root/tools/damask-lsp" --force
  else
    echo "==> damask-lsp is up to date ($installed)"
  fi
fi

# Zed clones the grammar into grammars/<name>/ and refuses to reuse a clone of a
# different repository. Clear any stale one so it re-clones from the pinned URL.
if [ -d "$here/grammars/damask" ]; then
  echo "==> clearing stale grammar clone"
  rm -rf "$here/grammars/damask"
fi

echo
echo "Done. In Zed run: zed: install dev extension  ->  select $here"
