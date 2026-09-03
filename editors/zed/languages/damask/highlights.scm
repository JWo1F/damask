; Tag delimiters: {, {#, {@, {:, {/, and }
(tag_open) @punctuation.special
(tag_delimiter) @punctuation.special

; `{# … #}` reaches no output, so it reads as a comment rather than as a tag
; whose body happens to be prose — which is how it looked while the grammar took
; it for a block tag and injected Rust into the sentence inside.
(comment) @comment
(html_comment) @comment
(doctype) @constant

; Element tags. Capitalised names are components and lowercase ones are HTML —
; the same distinction the compiler draws, so it is worth seeing.
(element "<" @punctuation.bracket)
(element ">" @punctuation.bracket)
(element "/" @punctuation.bracket)
(component_name) @type
(element_name) @tag

(attribute_name) @attribute
(attribute "=" @operator)

; The `class:name` directive, whose suffix *is* a class name. Its prefix is one
; token with the colon in it — that is what keeps `attribute_name`, which
; accepts colons, from swallowing the whole of `class:is-loading` — so there is
; no separate `:` to colour here.
(directive_prefix) @attribute
(class_name) @string.special
(class_directive "=" @operator)

; A quoted value's literal runs are string content; the `{ … }` tags inside it
; keep the tag colours above and inject as Rust.
(quoted_value) @string

; The `{@tokens(…)}` and `{@attrs(…)}` helpers. The name carries its `{@` — one
; token, so that its length settles it against the `{@` of `{@html …}` — and it
; is coloured as the call it reads as.
(helper_name) @function.special
(helper "(" @punctuation.bracket)
(helper ")" @punctuation.bracket)
(helper "}" @punctuation.special)
(helper "," @punctuation.delimiter)

; A key is a name, not Rust: the token a `@tokens` entry adds, or the half of an
; attribute name an `@attrs` entry supplies. It carries its own colon.
(helper_key) @string.special

; A quoted *positional* entry takes no colour here: unlike a key, it sits inside
; the `helper_expr` that injections.scm hands to Rust, so the Rust grammar
; colours it as the string literal it is.

; `{...expr}` attribute spread.
(spread "{" @punctuation.special)
(spread "}" @punctuation.special)
(spread "..." @operator)

; The Rust inside a tag and inside a helper's entries is highlighted by an
; injected grammar — see injections.scm. So is the text between elements.
