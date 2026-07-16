/**
 * Tree-sitter grammar for RSC (Rust Smart Components) templates.
 *
 * RSC templates are HTML with brace `{ … }` tags. This grammar recognizes
 * the tag structure — balancing nested braces and respecting string/char
 * literals so struct literals inside `{@render Card { … }}` don't close the tag
 * early — and exposes the tag `code` for Rust injection (see injections.scm). It
 * does not parse the Rust itself or the surrounding HTML.
 *
 * Text between tags is one contiguous run (so injected HTML sees whole tags),
 * including `<!-- … -->` comments, which the injected HTML grammar highlights.
 *
 * Because no HTML is modelled here, a `{ … }` anywhere is a tag — which is why
 * interpolation inside a quoted attribute value (`class="p-3 {expr}"`) and the
 * `class:on={expr}` directives highlight correctly with no rule of their own.
 *
 * Two edges this doesn't handle: a `{` inside an HTML comment is still read as a
 * tag, and so is the conditional-class map (`class={ "c": cond }`), whose
 * contents are then injected as Rust despite not being Rust. Both degrade to odd
 * highlighting, never to a broken parse.
 */
module.exports = grammar({
  name: 'rsc',

  extras: () => [],

  rules: {
    document: $ => repeat($._node),

    _node: $ => choice($.tag, $.text),

    tag: $ => seq(
      field('open', $.tag_open),
      optional($.code),
      field('close', alias('}', $.tag_delimiter)),
    ),

    // Longest-match ordering so `{#`, `{@`, `{:`, `{/` win over a bare `{`.
    tag_open: () => alias(token(choice('{#', '{@', '{:', '{/', '{')), 'tag_delimiter'),

    // Balanced tag content: text, nested brace groups, and literals.
    code: $ => repeat1(choice(
      $._code_text,
      $._braces,
      $.string,
      $.char,
      $.lifetime,
    )),
    _braces: $ => seq('{', optional($.code), '}'),
    _code_text: () => token(prec(-1, /[^{}"']+/)),
    string: () => token(/"([^"\\]|\\.)*"/),
    char: () => token(/'([^'\\]|\\.)'/),
    lifetime: () => token(/'[a-zA-Z_][a-zA-Z0-9_]*/),

    // HTML text: everything up to the next tag.
    text: () => token(prec(-2, /[^{]+/)),
  },
});
