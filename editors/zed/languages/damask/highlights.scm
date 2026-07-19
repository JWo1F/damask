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

; `class`, and the `class:name` directive whose suffix *is* a class name.
(directive_prefix) @attribute
(class_directive_name ":" @punctuation.delimiter)
(class_name) @string.special
(class_attribute "=" @operator)
(class_directive "=" @operator)

; A quoted value's literal runs are string content; the `{ … }` tags inside it
; keep the tag colours above and inject as Rust.
(quoted_value) @string

; Class lists and conditional maps.
(class_list "[" @punctuation.bracket)
(class_list "]" @punctuation.bracket)
(class_list "," @punctuation.delimiter)
(class_brace "{" @punctuation.bracket)
(class_brace "}" @punctuation.bracket)
(class_map "{" @punctuation.bracket)
(class_map "}" @punctuation.bracket)
(class_pair ":" @punctuation.delimiter)

; A class name — a map's key, or a quoted entry in a list. Not Rust: it is the
; thing the class list is made of, and both spellings of it take one colour.
(class_key) @string.special
(class_string) @string.special

; `{...expr}` attribute spread.
(spread "{" @punctuation.special)
(spread "}" @punctuation.special)
(spread "..." @operator)

; The Rust inside a tag, a class entry and a map condition is highlighted by an
; injected grammar — see injections.scm. So is the text between elements.
