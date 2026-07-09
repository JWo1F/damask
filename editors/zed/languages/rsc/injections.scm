; Rust inside a tag — only the tag's top-level `code` (not nested brace groups),
; so the whole expression injects once.
(tag (code) @injection.content
 (#set! injection.language "rust"))

; The HTML between tags. A `{ … }` tag splits the markup into separate `text`
; nodes, so an element's open and close tags land in different nodes
; (`<main>` / `</main>` around `{@render …}`). Combining every `text` node into
; one injected document lets the HTML grammar see the element as a whole —
; parsed separately, a lone `</main>` is an error and goes unhighlighted.
((text) @injection.content
 (#set! injection.language "html")
 (#set! injection.combined))
