/**
 * Tree-sitter grammar for RSC (Rust Smart Components) templates.
 *
 * It recognizes the tag structure only — it does not parse the Rust inside a
 * tag or the host language outside one. Those are handled by injection queries
 * (`injections.scm`), which is why `code` / `comment_text` / `content` are
 * exposed as dedicated nodes.
 */
module.exports = grammar({
  name: 'rsc',

  // Template whitespace is significant (it becomes output), so nothing is
  // skipped between tokens.
  extras: () => [],

  rules: {
    document: $ => repeat($._node),

    _node: $ => choice(
      $.comment_tag,   // <%# … %>  (longest opener first so it wins the lexer)
      $.output_tag,    // <%= … %>
      $.raw_tag,       // <%- … %>
      $.render_tag,    // <%+ … %>
      $.code_tag,      // <%  … %>
      $.content,       // literal host-language text
    ),

    output_tag: $ => seq(alias('<%=', $.tag_delimiter), optional($.code), alias('%>', $.tag_delimiter)),
    raw_tag:    $ => seq(alias('<%-', $.tag_delimiter), optional($.code), alias('%>', $.tag_delimiter)),
    render_tag: $ => seq(alias('<%+', $.tag_delimiter), optional($.code), alias('%>', $.tag_delimiter)),
    code_tag:   $ => seq(alias('<%',  $.tag_delimiter), optional($.code), alias('%>', $.tag_delimiter)),
    comment_tag: $ => seq(alias('<%#', $.tag_delimiter), optional($.comment_text), alias('%>', $.tag_delimiter)),

    // Everything up to the next `%>` (matches the parser's "first %> closes").
    code: () => token(/([^%]|%[^>])+/),
    comment_text: () => token(/([^%]|%[^>])+/),

    // A run of text that is not the start of a tag: any non-`<`, or a `<` not
    // followed by `%`.
    content: () => token(prec(-1, /([^<]|<[^%])+/)),
  },
});
