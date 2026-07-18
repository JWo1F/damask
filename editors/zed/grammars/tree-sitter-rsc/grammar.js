/**
 * Tree-sitter grammar for RSC (Rust Smart Components) templates.
 *
 * RSC templates are HTML with brace `{ … }` tags. This grammar recognizes
 * the tag structure — balancing nested braces and respecting string/char
 * literals so struct literals inside `{@render Card { … }}` don't close the tag
 * early — and exposes each tag's `code` for Rust injection (see injections.scm).
 * It does not parse the Rust itself.
 *
 * It *does* parse an element's angle-bracket tag, because RSC puts its own
 * syntax inside one: an attribute value may hold `{ … }` tags, a class list, or
 * a `{...}` spread. Modelling only text-between-tags cannot colour any of that —
 * a quoted value containing a tag is split across `text` nodes, so an injected
 * HTML parser never sees a complete attribute. Element *content* is still plain
 * text with HTML injected into it, so comments and entities keep their
 * highlighting.
 *
 * A `<` that does not begin a tag name stays text, so `a < b` is prose rather
 * than a broken element.
 */
// A double-quoted string. Named once because a class list's entries hold them —
// Tailwind's arbitrary values put `[` and `]` inside quotes, so the brackets
// that delimit a list must not be looked for there.
const CLASS_STRING = /"([^"\\]|\\.)*"/;

module.exports = grammar({
  name: 'rsc',

  extras: () => [],

  // A `{ … }` opening a class-list entry could be a nested brace group inside a
  // Rust expression or the conditional-class map; only its contents tell them
  // apart, so the parser explores both.
  // Everything that can open with `{` in a value position: an ordinary tag, a
  // nested brace group inside a Rust expression, and the conditional-class map.
  // Only the contents tell them apart, so the parser explores each.
  // A `{` opening a class-list entry could begin a nested brace group inside a
  // Rust expression or the conditional-class map; only the contents tell them
  // apart, so the parser explores both. (A map must hold at least one
  // `"name": cond` pair, which is what keeps `class={some_expr}` an ordinary
  // tag rather than an empty map.)
  // Inside a braced class value the parser cannot tell `"name": cond` pairs
  // from a Rust expression until it has read past the first token, so it
  // explores both.

  rules: {
    document: $ => repeat($._node),

    _node: $ => choice($.comment, $.html_comment, $.doctype, $.tag, $.element, $.text),

    // `{# … #}` — dropped entirely by the compiler, so it is a comment here too.
    // The whitespace after `{#` is what tells it from a `{#if}` block tag: a
    // block keyword cannot begin with one.
    comment: () =>
      token(seq('{#', /\s/, repeat(choice(/[^#]/, seq('#', /[^}]/))), '#}')),

    // ---------------------------------------------------------------- tags

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

    // `<!DOCTYPE html>`. The character after `<!` must not be a `-`, so this
    // never competes with a comment.
    doctype: () => token(seq('<!', /[^->][^>]*/, '>')),

    // `<!-- … -->`. Modelled rather than left to the injected HTML, because the
    // element rule now claims `<` and would otherwise try to read one here.
    html_comment: () =>
      token(seq('<!--', repeat(choice(/[^-]/, seq('-', /[^-]/), seq('--', /[^>]/))), '-->')),

    // ------------------------------------------------------------ elements

    // Only the angle-bracket tag itself; what it encloses is ordinary content.
    // The name must follow `<` with no gap, which is what keeps prose like
    // `a < b` from being read as the start of an element.
    element: $ => seq(
      '<',
      optional(token.immediate('/')),
      field('name', $.tag_name),
      repeat(seq($._ws, $._attribute)),
      optional($._ws),
      optional('/'),
      '>',
    ),

    // Capitalised names are components, lowercase are HTML — a distinction
    // highlights.scm draws, so the two are separate nodes.
    tag_name: $ => choice($.component_name, $.element_name),
    component_name: () => token.immediate(/[A-Z][A-Za-z0-9_]*/),
    element_name: () => token.immediate(/[a-z][A-Za-z0-9_.:-]*/),

    _ws: () => /\s+/,

    _attribute: $ => choice($.spread, $.class_directive, $.class_attribute, $.attribute),

    // `{...expr}` — a run of attributes prepared elsewhere.
    spread: $ => seq('{', '...', optional($.code), '}'),

    // `class` is its own rule because only it takes a list or a conditional
    // map; on any other attribute a `{ … }` value is an ordinary tag.
    class_attribute: $ => seq(
      field('name', $.directive_prefix),
      '=',
      field('value', choice($.quoted_value, $.class_list, $.class_brace)),
    ),

    // `class:name={cond}` — a directive naming one class. Its value is a plain
    // condition, so unlike `class` it takes no list or map.
    class_directive: $ => seq(
      field('name', $.class_directive_name),
      optional(seq('=', field('value', choice($.quoted_value, $.tag)))),
    ),
    class_directive_name: $ => seq($.directive_prefix, ':', $.class_name),

    directive_prefix: () => token(prec(1, 'class')),
    class_name: () => /[A-Za-z0-9_.:\/\[\]%-]+/,

    attribute: $ => seq(
      field('name', $.attribute_name),
      optional(seq('=', field('value', choice($.quoted_value, $.tag)))),
    ),
    attribute_name: () => /[A-Za-z_][A-Za-z0-9_.:-]*/,

    // A quoted value interpolates, so it holds tags as well as text. Modelled
    // here rather than left to an injected HTML parser, which would only ever
    // see the fragments either side of a tag.
    quoted_value: $ => choice(
      seq('"', repeat(choice($.tag, alias(token(prec(-1, /[^"{]+/)), $.text))), '"'),
      seq("'", repeat(choice($.tag, alias(token(prec(-1, /[^'{]+/)), $.text))), "'"),
    ),

    // `class=[…]` — entries are Rust expressions, or maps of conditionals.
    //
    // Flat: whitespace, commas, braced entries and plain entries are siblings.
    // Nesting them made whitespace ambiguous between a separator and the start
    // of an entry, which the lexer had to settle before the parser could.
    class_list: $ => seq(
      '[',
      repeat(choice($._ws, ',', $.class_map, $.class_string, $.class_expr)),
      ']',
    ),

    // A quoted run of class names — the same thing a map's key is, so it is its
    // own node and takes the same colour. As part of the code run below it was
    // left uncoloured while the map's key was not, which is the inconsistency
    // that made a list look half-highlighted.
    //
    // A sibling of `class_expr` rather than nested inside it: nested, a run of
    // strings and code could be grouped more than one way and the parser had no
    // way to choose. Flat, `Some("a")` is code, a class name, then code — which
    // is exactly what it is.
    class_string: () => token(CLASS_STRING),

    // The rest of an entry: a path, a call, an operator. Starts with a
    // non-space so it never competes with the whitespace between entries, and
    // holds no quotes so it never competes with the string above.
    class_expr: () => alias(token(prec(-1, /[^,\[\]{}"'\s][^,\[\]{}"']*/)), 'code'),

    // A braced entry *inside a list* is always a map — the compiler reads it
    // that way too — so unlike the top-level value below it needs no Rust
    // alternative, and with none there is nothing for the parser to guess.
    class_map: $ => seq('{', $._class_pairs, '}'),

    // A braced class value: either `"name": cond` pairs, or an ordinary Rust
    // expression. One rule, so `{` is a single token — as two rules the lexer
    // had to commit to one of them before the parser could see which it was,
    // and `class={SOME_CONST}` was read as a malformed map.
    class_brace: $ => seq(
      '{',
      optional(choice(prec.dynamic(1, $._class_pairs), $.class_code)),
      '}',
    ),
    // The expression form of a braced class value. Its own rule, with quotes
    // inside the text run rather than a `string` token, because that token and
    // a map's `class_key` are the same shape — and the lexer has to pick one
    // before the parser knows which form it is reading.
    class_code: $ => repeat1(choice(
      alias(token(prec(-1, /[^{}]+/)), $.code),
      $._braces,
    )),

    // Flat, for the reason the list is: nesting whitespace inside a pair made
    // it ambiguous between a separator and the start of the next key.
    _class_pairs: $ => repeat1(choice($._ws, ',', $.class_pair)),
    class_pair: $ => seq(
      field('key', $.class_key),
      optional($._ws),
      ':',
      field('condition', $.class_condition),
    ),

    class_key: () => token(/"([^"\\]|\\.)*"/),
    // A condition carries its own quotes rather than a `string` token: that
    // token and `class_key` are identical, so with both live the parser could
    // not tell a condition continuing from the next pair's key beginning.
    // Its run stops at a comma, which is what separates the pairs.
    class_condition: $ => repeat1(choice(
      alias(token(prec(-1, /[^,{}]+/)), $.code),
      $._braces,
    )),

    // ---------------------------------------------------------------- text

    // Everything that is not a tag, an element or a comment.
    //
    // A `<` is part of the text unless a name, a `/` or a `!` follows it
    // immediately — the same test the compiler's parser applies — so prose like
    // `a < b` and `3<4` stays prose instead of opening an element.
    text: () => token(prec(-2, choice(
      /([^{<]|<[^A-Za-z\/!])+/,
      /</,
    ))),
  },
});
