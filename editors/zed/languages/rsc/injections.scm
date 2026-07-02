; Rust inside a tag — only the tag's top-level `code` (not nested brace groups),
; so the whole expression injects once.
(tag (code) @injection.content
 (#set! injection.language "rust"))

; The HTML between tags.
((text) @injection.content
 (#set! injection.language "html"))
