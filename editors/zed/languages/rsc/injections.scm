; Rust inside expression, raw, render, and statement tags.
((code) @injection.content
 (#set! injection.language "rust"))

; Host language in the literal text between tags.
;
; A `.rsc` file's real host language is named by its middle extension
; (foo.html.rsc, foo.js.rsc, foo.css.rsc). Tree-sitter injection queries choose a
; single fixed language, so v1 injects HTML — the common case. Per-suffix
; selection (registering html.rsc / js.rsc / css.rsc as distinct languages that
; share this grammar) is a documented follow-up.
((content) @injection.content
 (#set! injection.language "html"))
