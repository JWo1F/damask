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
#
# It also clears Zed's local clone of the grammar so Zed re-clones it at the
# pinned revision — unless that clone has work in it, in which case it says so
# and stops. DAMASK_FORCE_GRAMMAR=1 deletes it anyway; DAMASK_SKIP_LSP=1 skips
# the language-server build.
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
# different repository — or to check out a new revision over local edits. Clear
# any stale one so it re-clones from the pinned URL.
#
# But look before deleting. That directory is git-ignored, so nothing ever shows
# what is in it, and it is a perfectly ordinary clone of the grammar — which
# makes it an easy place to edit the grammar by mistake and a silent place to
# lose the edit. It has happened: a correction to the README and the test corpus
# lived there for weeks, invisible, until a checkout refused to run over it.
clone="$here/grammars/damask"
if [ -d "$clone" ]; then
  # Whether it is a clone at all, asked as "is it its *own* repository". Testing
  # for a repository plainly would say yes to any directory here, since this one
  # sits inside the Damask checkout and git would answer about that instead —
  # and then the checks below would report the parent's dirty files as work in a
  # clone that does not exist. Anything that is not its own repository is a
  # half-finished or corrupted clone, with nothing in it to protect.
  top="$(git -C "$clone" rev-parse --show-toplevel 2>/dev/null || true)"
  if [ "$top" != "$clone" ]; then
    echo "==> clearing grammar clone (not a clone of the grammar)"
    rm -rf "$clone"
  else
    # Uncommitted or untracked files, stashes, and commits that are on no
    # remote — three ways for work to exist only here.
    dirty="$(git -C "$clone" status --porcelain)"
    stashed="$(git -C "$clone" stash list)"
    unpushed="$(git -C "$clone" log --oneline HEAD --not --remotes 2>/dev/null)"

    if [ -n "$dirty$stashed$unpushed" ] && [ "${DAMASK_FORCE_GRAMMAR:-0}" != "1" ]; then
      echo
      echo "REFUSING to clear the grammar clone: it has work in it." >&2
      echo "  $clone" >&2
      echo >&2
      [ -n "$dirty" ] && { echo "uncommitted:" >&2; echo "$dirty" >&2; }
      [ -n "$stashed" ] && { echo "stashed:" >&2; echo "$stashed" >&2; }
      [ -n "$unpushed" ] && { echo "on no remote:" >&2; echo "$unpushed" >&2; }
      echo >&2
      echo "This is a throwaway clone Zed manages, so nothing here is backed up." >&2
      echo "Move the work to the grammar repository, which is where it belongs:" >&2
      echo >&2
      echo "  git -C $clone diff > /tmp/grammar.patch" >&2
      echo >&2
      echo "Then apply it to a real checkout of" >&2
      echo "  $(git -C "$clone" remote get-url origin 2>/dev/null || echo 'the grammar repository')" >&2
      echo "and re-run this script. To delete it regardless:" >&2
      echo >&2
      echo "  DAMASK_FORCE_GRAMMAR=1 $0" >&2
      exit 1
    fi

    echo "==> clearing stale grammar clone"
    rm -rf "$clone"
  fi
fi

echo
echo "Done. In Zed run: zed: install dev extension  ->  select $here"
