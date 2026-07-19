; Rust inside a tag — only the tag's top-level `code` (not nested brace groups),
; so the whole expression injects once.
(tag (code) @injection.content
 (#set! injection.language "rust"))

; The expression a `{...}` spread hands to `AttrSpread`.
(spread (code) @injection.content
 (#set! injection.language "rust"))

; The Rust parts of a class value: a list entry that is not a class name, a
; plain braced value, and a map's condition. Each injects on its own, so
; `class={ "a": cond }` is never handed to the Rust grammar as one lump — it is
; not an expression, and while the grammar took it for an ordinary tag its `:`
; came back an error.
;
; The `#set!` belongs *inside* the pattern's parens. Outside, it parses as a
; pattern of its own and the language is attached to nothing, which leaves the
; captured text with no injection and no colour at all.
((class_expr) @injection.content
 (#set! injection.language "rust"))
((class_code (code)) @injection.content
 (#set! injection.language "rust"))
((class_condition (code)) @injection.content
 (#set! injection.language "rust"))

; The text between elements. A `{ … }` tag splits it into separate `text` nodes,
; so combining them into one injected document lets the HTML grammar see a run
; of content as a whole.
;
; Only element *content* reaches here: the grammar owns the angle-bracket tags
; themselves (see highlights.scm), because an attribute value holding a tag is
; not something an injected HTML parser can ever see the whole of.
((text) @injection.content
 (#set! injection.language "html")
 (#set! injection.combined))
