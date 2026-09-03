/**
 * Tree-sitter grammar for Damask templates.
 *
 * Damask templates are HTML with brace `{ … }` tags. This grammar recognizes
 * the tag structure — balancing nested braces and respecting string/char
 * literals so struct literals inside `{@render Card { … }}` don't close the tag
 * early — and exposes each tag's `code` for Rust injection (see injections.scm).
 * It does not parse the Rust itself.
 *
 * It *does* parse an element's angle-bracket tag, because Damask puts its own
 * syntax inside one: an attribute value may hold `{ … }` tags, an `@tokens` or
 * `@attrs` helper, or a `{...}` spread. Modelling only text-between-tags cannot colour any of that —
 * a quoted value containing a tag is split across `text` nodes, so an injected
 * HTML parser never sees a complete attribute. Element *content* is still plain
 * text with HTML injected into it, so comments and entities keep their
 * highlighting.
 *
 * A `<` that does not begin a tag name stays text, so `a < b` is prose rather
 * than a broken element.
 */
module.exports = grammar({
  name: 'damask',

  extras: () => [],

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

    _attribute: $ => choice($.spread, $.class_directive, $.attribute),

    // `{...expr}` — a run of attributes prepared elsewhere.
    spread: $ => seq('{', '...', optional($.code), '}'),

    // `class:name={cond}` — a directive naming one class. The only attribute
    // whose *name* carries meaning, and the last of them: `class` and `data`
    // used to have list and map forms of their own, and now take the same two
    // helpers every other attribute takes.
    class_directive: $ => seq(
      field('name', $.class_directive_name),
      optional(seq('=', field('value', choice($.quoted_value, $.tag)))),
    ),

    // The `class:` is one token, colon included, so that it out-matches
    // `attribute_name` — which accepts colons, and would otherwise swallow
    // `class:is-loading` whole. Taking the colon into the prefix is what lets
    // the precedence be narrow enough to be safe: it fires only on a name that
    // is *followed* by a colon, so it can never bite a `class-*` attribute.
    class_directive_name: $ => seq(
      alias(token(prec(1, /class:/)), $.directive_prefix),
      $.class_name,
    ),
    class_name: () => /[A-Za-z0-9_.:\/\[\]%-]+/,

    attribute: $ => seq(
      field('name', $.attribute_name),
      optional(seq('=', field('value', choice($.quoted_value, $.helper, $.tag)))),
    ),
    attribute_name: () => /[A-Za-z_][A-Za-z0-9_.:-]*/,

    // A quoted value interpolates, so it holds tags as well as text. Modelled
    // here rather than left to an injected HTML parser, which would only ever
    // see the fragments either side of a tag.
    quoted_value: $ => choice(
      seq('"', repeat(choice($.tag, alias(token(prec(-1, /[^"{]+/)), $.text))), '"'),
      seq("'", repeat(choice($.tag, alias(token(prec(-1, /[^'{]+/)), $.text))), "'"),
    ),

    // ------------------------------------------------------------- helpers

    // `{@tokens(…)}` builds one space-separated value; `{@attrs(…)}` expands
    // into a run of `<name>-*` attributes. Both are written on any attribute,
    // so nothing here is keyed to a name.
    //
    // The opener is one token, brace and `@` included, so that its length
    // settles it against the `{@` that opens `{@html …}` — the lexer prefers
    // the longer match and needs no precedence to do it.
    helper: $ => seq(
      field('name', alias(token(choice('{@tokens', '{@attrs')), $.helper_name)),
      optional($._ws),
      '(',
      // No whitespace of its own: an entry takes the space around it, which is
      // what keeps `self.n > 2` one entry rather than three.
      repeat(choice(',', $.helper_pair, $.helper_expr)),
      ')',
      optional($._ws),
      '}',
    ),

    // `name: value` — a conditional token, or one attribute under the prefix.
    //
    // The key takes its colon with it. Without it the key and the head of an
    // expression are the same shape, and the lexer would have to pick one
    // before the parser could see which was there; with it the key is the
    // longer match, and an expression — whose run stops at a lone colon — is
    // what is left when there is no colon to take.
    helper_pair: $ => seq(
      field('key', $.helper_key),
      field('value', alias($._entry, $.helper_value)),
    ),
    helper_key: () => token(seq(
      /\s*/,
      choice(/"([^"\\]|\\.)*"/, /[A-Za-z_][A-Za-z0-9_-]*/),
      /\s*/,
      ':',
    )),

    // A positional entry: a whole `TokenItem` or `AttrSet`.
    helper_expr: $ => $._entry,

    // The Rust of one entry, stopping where the next begins. Bracket groups are
    // balanced rather than scanned, so `self.count()` and `[a, b]` are entries
    // and their delimiters cannot end the helper.
    // Whitespace belongs to the entry around it — the space in `self.n > 2`, and
    // the one after a key's colon — because the helper itself has none to give:
    // an entry that could be nothing but whitespace would leave the parser
    // choosing between an empty entry and a separator at every gap.
    _entry: $ => prec.right(seq(
      optional($._ws),
      $._atom,
      repeat(choice($._ws, $._atom)),
    )),

    _atom: $ => choice(
      alias($._entry_text, $.code),
      $.string,
      $.char,
      $._group_parens,
      $._group_brackets,
      $._group_braces,
    ),

    // Leading whitespace is taken into the run rather than left to the
    // separator between entries, so that ` > 2` continues the expression it
    // belongs to. It stops at a lone `:`, which is a key's, and continues
    // through `::`, which is a path's.
    _entry_text: () => token(prec(-1, seq(
      /\s*/,
      /[^,()\[\]{}"'\s:]/,
      repeat(choice(/[^,()\[\]{}"':]/, '::')),
    ))),

    // Inside a bracket group nothing is a separator, so the text run there
    // keeps its commas and colons — a struct literal and a call's arguments are
    // one piece of Rust, not several entries.
    _group: $ => repeat1(choice(
      alias($._group_text, $.code),
      $.string,
      $.char,
      $._group_parens,
      $._group_brackets,
      $._group_braces,
    )),
    _group_text: () => token(prec(-1, /[^()\[\]{}"']+/)),
    _group_parens: $ => seq('(', optional($._group), ')'),
    _group_brackets: $ => seq('[', optional($._group), ']'),
    _group_braces: $ => seq('{', optional($._group), '}'),

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
