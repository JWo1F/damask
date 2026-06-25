use proc_macro2::TokenStream;
use syn::parse::{Parse, ParseStream};
use syn::{Field, Ident, LitStr, Token, Visibility, braced};

mod kw {
    syn::custom_keyword!(schema);
    syn::custom_keyword!(template);
}

/// The parsed body of a `component! { … }` invocation.
///
/// Grammar (blocks are order-independent and each optional):
///
/// ```text
/// component! {
///     [visibility] Name
///     [ template = "path"; ]
///     [ schema { [pub] field: Type; … } ]
///     [ impl { <items> } ]
/// }
/// ```
///
/// A `!`-macro must be followed directly by a delimiter, so the whole component
/// is wrapped in one `{ … }` group (the brief's `component! Name { … }` is not
/// valid Rust macro-call syntax).
pub struct ComponentInput {
    pub vis: Visibility,
    pub name: Ident,
    pub template: Option<LitStr>,
    pub fields: Vec<Field>,
    pub impl_body: TokenStream,
}

impl Parse for ComponentInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let vis: Visibility = input.parse()?;
        let name: Ident = input.parse()?;

        let mut template: Option<LitStr> = None;
        let mut fields: Option<Vec<Field>> = None;
        let mut impl_body: Option<TokenStream> = None;

        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::template) {
                let key = input.parse::<kw::template>()?;
                // Accept `template = "…"` or `template: "…"`.
                if input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                } else if input.peek(Token![:]) {
                    input.parse::<Token![:]>()?;
                } else {
                    return Err(input.error("expected `=` or `:` after `template`"));
                }
                let lit: LitStr = input.parse()?;
                if template.is_some() {
                    return Err(syn::Error::new_spanned(key, "duplicate `template`"));
                }
                template = Some(lit);
                // Optional trailing separator.
                if input.peek(Token![;]) {
                    input.parse::<Token![;]>()?;
                } else if input.peek(Token![,]) {
                    input.parse::<Token![,]>()?;
                }
            } else if lookahead.peek(kw::schema) {
                let key = input.parse::<kw::schema>()?;
                let content;
                braced!(content in input);
                if fields.is_some() {
                    return Err(syn::Error::new_spanned(key, "duplicate `schema` block"));
                }
                fields = Some(parse_fields(&content)?);
            } else if lookahead.peek(Token![impl]) {
                let key = input.parse::<Token![impl]>()?;
                let content;
                braced!(content in input);
                if impl_body.is_some() {
                    return Err(syn::Error::new_spanned(key, "duplicate `impl` block"));
                }
                impl_body = Some(content.parse()?);
            } else {
                return Err(lookahead.error());
            }
        }

        Ok(ComponentInput {
            vis,
            name,
            template,
            fields: fields.unwrap_or_default(),
            impl_body: impl_body.unwrap_or_default(),
        })
    }
}

/// Parse `schema { … }` field declarations, separated by `;` or `,`.
fn parse_fields(input: ParseStream) -> syn::Result<Vec<Field>> {
    let mut fields = Vec::new();
    while !input.is_empty() {
        fields.push(input.call(Field::parse_named)?);
        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
        } else if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else if !input.is_empty() {
            return Err(input.error("expected `;` or `,` between schema fields"));
        }
    }
    Ok(fields)
}
