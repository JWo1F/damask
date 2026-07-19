; A nested brace group inside a tag's code — a multi-line struct literal or
; block indents its contents. A tag's own outer braces are `tag_open` and
; `tag_delimiter`, not these anonymous tokens, so only real nesting matches and
; a one-line `{self.name}` is left alone.
(code
  "{"
  "}" @end) @indent

; Block-tag indentation (`{#if}` … `{/if}`) is not expressible here: the grammar
; makes those sibling nodes rather than a single spanning node. config.toml's
; increase/decrease_indent_pattern handles them. HTML elements indent via the
; injected HTML layer.
