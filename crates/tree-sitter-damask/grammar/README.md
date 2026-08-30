# Vendored grammar

Copied verbatim from
[JWo1F/tree-sitter-damask](https://github.com/JWo1F/tree-sitter-damask) at

    30406e9d412da0a6d005fea865f9705baa34db93

which is the same revision `editors/zed/extension.toml` pins. **Bump the two
together**: Zed clones the grammar and the website compiles this copy, so a
revision that only moves in one place is a grammar the editor and the site
disagree about.

To adopt a new revision, replace `src/` with the upstream `src/` at that
revision and update the hash above — nothing here is edited by hand. The parser
is generated at ABI 14 and has no external scanner, so `src/parser.c` and the
headers beside it are the whole of it.

Licensed MIT by the upstream project; its licence is in `LICENSE`.
