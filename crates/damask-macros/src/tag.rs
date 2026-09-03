//! The `tag!` macro: one element, built in Rust rather than in a template.
//!
//! What it is for is the markup a `.dmk` is the wrong shape for — a helper in a
//! service, a fragment a controller assembles, a `<style>` element whose CSS is
//! full of the `{` a template reserves. It writes the same runtime calls the
//! lowerer writes, through the same [`Attr`], [`TokenList`] and [`Attrs`], and
//! it takes the same two helpers — `class: @tokens(…)`, `data: @attrs(…)` — on
//! any attribute, so a tag means here exactly what it means there.
//!
//! [`Attr`]: damask::Attr
//! [`TokenList`]: damask::TokenList
//! [`Attrs`]: damask::Attrs

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, LitStr, Path, Token, parenthesized};

/// One argument of `@tokens(…)`.
enum TokenEntry {
    /// A [`TokenItem`](damask::TokenItem): a name, a list of them, an `Option`.
    /// Each is added on its own, so the entries need no common type.
    Item(Expr),
    /// `name: cond` — a token that is there while its condition holds.
    Cond(LitStr, Expr),
}

/// One argument of `@attrs(…)`.
enum AttrEntry {
    /// An [`AttrSet`](damask::AttrSet): a map, a pair list, an `Attrs`.
    Set(Expr),
    /// `key: value` — one attribute under the prefix.
    Pair(LitStr, Expr),
}

enum AttrSpec {
    Plain {
        name: LitStr,
        value: Expr,
    },
    /// `name: @tokens(…)`, on any attribute.
    Tokens {
        name: LitStr,
        entries: Vec<TokenEntry>,
    },
    /// `name: @attrs(…)`, on any attribute — a run of `name-*`.
    Attrs {
        name: LitStr,
        entries: Vec<AttrEntry>,
    },
}

pub struct TagInput {
    krate: Path,
    name: String,
    name_span: Span,
    /// The `#id` in the head, already a string because it is written as one.
    id: Option<LitStr>,
    attrs: Vec<AttrSpec>,
    children: Option<Expr>,
}

impl Parse for TagInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // The path Damask is reachable by, threaded in by the `macro_rules!`
        // wrapper so that `$crate` resolves for a crate that re-exports this.
        let krate;
        parenthesized!(krate in input);
        let krate: Path = krate.parse()?;

        let (name, id, name_span) = parse_head(input)?;

        let mut attrs = Vec::new();
        let mut children = None;
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break; // a trailing comma
            }
            match parse_attr(input)? {
                Some(attr) => attrs.push(attr),
                // Not a `name:` pair, so this is the content, and it is last.
                None => {
                    children = Some(input.parse()?);
                    if !input.is_empty() {
                        input.parse::<Token![,]>()?;
                        if !input.is_empty() {
                            return Err(input.error(
                                "content comes last: everything after the element is \
                                 `name: value`, and the one argument without a name is what \
                                 goes inside",
                            ));
                        }
                    }
                    break;
                }
            }
        }

        Ok(TagInput {
            krate,
            name,
            name_span,
            id,
            attrs,
            children,
        })
    }
}

/// Read `div` or `div#some-id`.
///
/// The head is taken as raw tokens and put back together with the whitespace
/// removed, because `some-id` is three tokens to Rust and one name to HTML.
/// Nothing is lost by that: neither an element name nor an id may contain a
/// space, so any whitespace between the tokens was the lexer's rather than the
/// author's.
fn parse_head(input: ParseStream) -> syn::Result<(String, Option<LitStr>, Span)> {
    let span = input.span();
    let mut head = TokenStream::new();
    while !input.is_empty() && !input.peek(Token![,]) {
        let tree: TokenTree = input.parse()?;
        head.extend(std::iter::once(tree));
    }
    let head = head.to_string().replace([' ', '\n', '\t'], "");
    if head.is_empty() {
        return Err(syn::Error::new(
            span,
            "`tag!` needs an element, as `tag!(div)`",
        ));
    }

    let (name, id) = match head.split_once('#') {
        Some((name, id)) => (name, Some(id)),
        None => (head.as_str(), None),
    };

    if !is_element_name(name) {
        return Err(syn::Error::new(
            span,
            format!(
                "`{name}` is not an element name: a name starts with a letter and continues \
                 with letters, digits or `-`"
            ),
        ));
    }
    let id = match id {
        None => None,
        Some("") => {
            return Err(syn::Error::new(
                span,
                "`#` in the head needs an id after it, as `tag!(div#main)`",
            ));
        }
        Some(id) => Some(LitStr::new(id, span)),
    };

    Ok((name.to_owned(), id, span))
}

fn is_element_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Read one `name: value`, or `None` if what is next has no name and is
/// therefore the content.
fn parse_attr(input: ParseStream) -> syn::Result<Option<AttrSpec>> {
    // `::` is a path, not a name and a value; and an ident followed by anything
    // else is an expression that happens to start with one.
    let named = (input.peek(syn::Ident::peek_any) || input.peek(LitStr))
        && input.peek2(Token![:])
        && !input.peek2(Token![::]);
    if !named {
        return Ok(None);
    }

    let name = parse_attr_name(input)?;
    input.parse::<Token![:]>()?;

    // `@tokens(…)` and `@attrs(…)` are the template's helpers, spelled the same
    // here. Nothing is keyed to the attribute's name: `class`, `rel` and `data`
    // are read exactly alike, and the helper says what happens to the value.
    if input.peek(Token![@]) {
        input.parse::<Token![@]>()?;
        let helper = input.call(syn::Ident::parse_any)?;
        let args;
        parenthesized!(args in input);
        return Ok(Some(match helper.to_string().as_str() {
            "tokens" => AttrSpec::Tokens {
                name,
                entries: parse_token_entries(&args)?,
            },
            "attrs" => AttrSpec::Attrs {
                name,
                entries: parse_attr_entries(&args)?,
            },
            other => {
                return Err(syn::Error::new(
                    helper.span(),
                    format!(
                        "unknown attribute helper `@{other}`; the helpers are `@tokens` and \
                         `@attrs`"
                    ),
                ));
            }
        }));
    }

    Ok(Some(AttrSpec::Plain {
        name,
        value: input.parse()?,
    }))
}

/// An attribute's name: an ident, whose underscores become hyphens, or a string
/// for anything an ident cannot spell.
///
/// No HTML attribute contains an underscore, so `aria_label` is unambiguous and
/// saves quoting the common case. `"http-equiv"` is there for the rest —
/// including the handful of names that are Rust keywords in every position.
fn parse_attr_name(input: ParseStream) -> syn::Result<LitStr> {
    if input.peek(LitStr) {
        return input.parse();
    }
    let ident = input.call(syn::Ident::parse_any)?;
    let name = ident.to_string();
    // A raw ident is how `type:` and `for:` are written; the `r#` is Rust's and
    // is not part of the name.
    let name = name.strip_prefix("r#").unwrap_or(&name).replace('_', "-");
    Ok(LitStr::new(&name, ident.span()))
}

fn parse_token_entries(input: ParseStream) -> syn::Result<Vec<TokenEntry>> {
    let mut entries = Vec::new();
    while !input.is_empty() {
        entries.push(match parse_key(input)? {
            Some(key) => TokenEntry::Cond(key, input.parse()?),
            None => TokenEntry::Item(input.parse()?),
        });
        if input.is_empty() {
            break;
        }
        input.parse::<Token![,]>()?;
    }
    Ok(entries)
}

fn parse_attr_entries(input: ParseStream) -> syn::Result<Vec<AttrEntry>> {
    let mut entries = Vec::new();
    while !input.is_empty() {
        entries.push(match parse_key(input)? {
            Some(key) => AttrEntry::Pair(key, input.parse()?),
            None => AttrEntry::Set(input.parse()?),
        });
        if input.is_empty() {
            break;
        }
        input.parse::<Token![,]>()?;
    }
    Ok(entries)
}

/// The `key:` of a `key: value` entry, or `None` when the entry is an
/// expression instead.
///
/// A key is taken as written, with no `_`→`-` rewriting: an attribute key is
/// the part after the prefix and Damask never rewrites one, so `user_id` stays
/// `data-user_id`. Quote the key to write a hyphen, or anything else an ident
/// cannot spell.
fn parse_key(input: ParseStream) -> syn::Result<Option<LitStr>> {
    let keyed = (input.peek(syn::Ident::peek_any) || input.peek(LitStr))
        && input.peek2(Token![:])
        && !input.peek2(Token![::]);
    if !keyed {
        return Ok(None);
    }
    let key = if input.peek(LitStr) {
        input.parse()?
    } else {
        let ident = input.call(syn::Ident::parse_any)?;
        let name = ident.to_string();
        LitStr::new(name.strip_prefix("r#").unwrap_or(&name), ident.span())
    };
    input.parse::<Token![:]>()?;
    Ok(Some(key))
}

pub fn expand(input: TagInput) -> TokenStream {
    let TagInput {
        krate,
        name,
        name_span,
        id,
        attrs,
        children,
    } = input;

    let void = damask_template::is_void_element(&name);
    if void && let Some(children) = &children {
        return syn::Error::new_spanned(
            children,
            format!("`<{name}>` is a void element, so it has no content"),
        )
        .to_compile_error();
    }

    if id.is_some()
        && attrs
            .iter()
            .any(|attr| matches!(attr, AttrSpec::Plain { name, .. } if name.value() == "id"))
    {
        return syn::Error::new(
            name_span,
            "the id is given twice — once in the head and once as `id:`",
        )
        .to_compile_error();
    }

    let open = format!("<{name}");
    let close = format!("</{name}>");

    let id = id.map(|id| {
        quote! { #krate::Attr::write_attr(&(#id), "id", __tag_r); }
    });

    let attrs = attrs.iter().map(|attr| match attr {
        AttrSpec::Plain { name, value } => quote! {
            #krate::Attr::write_attr(&(#value), #name, __tag_r);
        },
        AttrSpec::Tokens { name, entries } => {
            let entries = entries.iter().map(|entry| match entry {
                TokenEntry::Item(expr) => quote! {
                    #krate::TokenItem::add_to(&(#expr), &mut __tag_tokens);
                },
                TokenEntry::Cond(token, when) => quote! {
                    if #when { __tag_tokens.add(#token); }
                },
            });
            quote! {
                {
                    let mut __tag_tokens = #krate::TokenList::new();
                    #(#entries)*
                    __tag_tokens.write_attr(#name, __tag_r);
                }
            }
        }
        AttrSpec::Attrs { name, entries } => {
            let entries = entries.iter().map(|entry| match entry {
                AttrEntry::Set(expr) => quote! {
                    #krate::AttrSet::add_attrs(&(#expr), &mut __tag_group);
                },
                AttrEntry::Pair(key, value) => quote! {
                    __tag_group.insert(#key, (#value));
                },
            });
            quote! {
                {
                    let mut __tag_group = #krate::Attrs::new();
                    #(#entries)*
                    __tag_group.write_attrs_prefixed(#name, __tag_r);
                }
            }
        }
    });

    // The content is written after the sink is given up, because it appends to
    // the document directly — `Content` is about escaping, which is a decision
    // about a value, and needs no renderer to make it.
    let content = match (void, children) {
        (true, _) => quote! {},
        (false, None) => quote! { __tag_out.push_markup(#close); },
        (false, Some(children)) => quote! {
            #krate::Content::write_content(&(#children), &mut __tag_out);
            __tag_out.push_markup(#close);
        },
    };

    quote! {{
        let mut __tag_out = #krate::Trusted::new();
        {
            let mut __tag_sink = #krate::Trusted::sink(&mut __tag_out);
            let __tag_r: &mut dyn #krate::Renderer = &mut __tag_sink;
            __tag_r.write_raw(#open);
            #id
            #(#attrs)*
            __tag_r.write_raw(">");
        }
        #content
        __tag_out
    }}
}
