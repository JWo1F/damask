; Tag delimiters: {, {#, {@, {:, {/, and }
(tag_open) @punctuation.special
(tag_delimiter) @punctuation.special

; HTML comments
(comment) @comment

; The Rust inside a tag and the HTML around it are highlighted by injected
; grammars — see injections.scm.
