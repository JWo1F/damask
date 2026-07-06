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
# Re-run it whenever you change grammar.js.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
src="$here/grammars/tree-sitter-rsc"
repo="${RSC_GRAMMAR_REPO:-$HOME/.cache/zed-rsc/tree-sitter-rsc}"

command -v tree-sitter >/dev/null || { echo "error: install the tree-sitter CLI first"; exit 1; }

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
