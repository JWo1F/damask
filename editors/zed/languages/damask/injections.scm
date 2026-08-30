; Every pattern here sets `injection.include-children`, and none of them work
; without it. An injection defaults to the ranges a captured node owns *itself* —
; the bytes left over once its children are removed — which is not what any of
; these captures mean:
;
;   { "hi".len() }   `code` has a `string` child, so the literal was cut out of
;                    the injected region and came back uncoloured.
;   class={ … }      `class_code`, `class_condition` and `class_expr` are each
;                    wholly covered by the `code` they wrap, so the leftover was
;                    empty and the Rust inside a class value got no colour at all.
;
; With children included, each capture injects the expression it delimits, whole
; and once — which is also what makes a nested brace group (a closure body, a
; struct literal) part of the same expression rather than a hole in it.

; Rust inside a tag.
(tag (code) @injection.content
 (#set! injection.language "rust")
 (#set! injection.include-children))

; The expression a `{...}` spread hands to `AttrSpread`.
(spread (code) @injection.content
 (#set! injection.language "rust")
 (#set! injection.include-children))

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
 (#set! injection.language "rust")
 (#set! injection.include-children))
((class_code) @injection.content
 (#set! injection.language "rust")
 (#set! injection.include-children))
((class_condition) @injection.content
 (#set! injection.language "rust")
 (#set! injection.include-children))

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
