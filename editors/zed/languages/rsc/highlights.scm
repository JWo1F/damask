; Tag delimiters (<%=, <%-, <%+, <%, <%#, %>)
(tag_delimiter) @punctuation.special

; The body of a <%# … %> comment tag
(comment_text) @comment

; The Rust inside code tags and the host language inside text are highlighted
; by injected grammars — see injections.scm.
