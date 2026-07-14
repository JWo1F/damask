#!/usr/bin/env bash
# Prepare the RSC Zed extension for local development.
#
# Zed loads a Tree-sitter grammar by cloning a git repository at a pinned
# revision. Our grammar lives in this monorepo (grammars/tree-sitter-rsc), which
# can't itself be the clone target. This script copies the grammar into a
# standalone git repo, regenerates the parser, and rewrites `[grammars.rsc]` in
# extension.toml to point at that repo via a `file://` URL — after which
# "zed: install dev extension" on this directory works.
#
# It also reinstalls `rsc-lsp` when the copy on PATH is older than its sources:
# the extension launches that installed binary, not this checkout, so a stale one
# keeps serving results from old lowering long after the fix is committed.
#
# Re-run it whenever you change grammar.js or anything the language server
# compiles in (tools/rsc-lsp, crates/rsc-template, crates/rsc).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
src="$here/grammars/tree-sitter-rsc"
repo="${RSC_GRAMMAR_REPO:-$HOME/.cache/zed-rsc/tree-sitter-rsc}"

command -v tree-sitter >/dev/null || { echo "error: install the tree-sitter CLI first"; exit 1; }

# The language server is a separate binary on PATH; the grammar work below does
# nothing for it. Reinstall when anything it is built from is newer than it.
# Set RSC_SKIP_LSP=1 to skip (the release build takes ~30s).
if [ "${RSC_SKIP_LSP:-0}" != "1" ]; then
  installed="$(command -v rsc-lsp || true)"
  newest="$(find "$root/tools/rsc-lsp" "$root/crates" -name '*.rs' -newer "${installed:-/nonexistent}" -print -quit 2>/dev/null || true)"
  if [ -z "$installed" ] || [ -n "$newest" ]; then
    echo "==> installing rsc-lsp (${installed:-not on PATH} is missing or stale)"
    cargo install --path "$root/tools/rsc-lsp" --force
  else
    echo "==> rsc-lsp is up to date ($installed)"
  fi
fi

# Zed clones the grammar into grammars/<name>/ and refuses to reuse a clone of a
# different repo. Clear any stale one so it re-clones from the current file:// URL.
rm -rf "$here/grammars/rsc"

echo "==> generating parser in $src (ABI 14 for Zed compatibility)"
( cd "$src" && tree-sitter generate --abi 14 >/dev/null )

echo "==> syncing grammar into standalone repo $repo"
rm -rf "$repo"
mkdir -p "$repo"
cp -R "$src/grammar.js" "$src/package.json" "$src/tree-sitter.json" "$src/src" "$src/test" "$repo/"

echo "==> committing"
git -C "$repo" init -q
git -C "$repo" add -A
git -C "$repo" -c user.email=dev@rsc.local -c user.name="RSC dev" commit -qm "tree-sitter-rsc grammar"
rev="$(git -C "$repo" rev-parse HEAD)"

echo "==> pointing extension.toml at file://$repo @ $rev"
ext="$here/extension.toml"
python3 - "$ext" "file://$repo" "$rev" <<'PY'
import re, sys
path, url, rev = sys.argv[1:4]
s = open(path).read()
s = re.sub(r'(\[grammars\.rsc\]\nrepository = ")[^"]*(")', lambda m: m.group(1)+url+m.group(2), s)
s = re.sub(r'(rev = ")[0-9a-f]*(")', lambda m: m.group(1)+rev+m.group(2), s)
open(path, "w").write(s)
PY

echo
echo "Done. In Zed run: zed: install dev extension  ->  select $here"
echo "(extension.toml now references your local grammar; don't commit that change.)"
