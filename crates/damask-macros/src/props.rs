//! The prop builder a component is constructed through.
//!
//! A template lowers `<Card title={t}/>` to `Card::__damask_props().title(t)
//! .__damask_build()` rather than a struct literal, because the call site knows
//! only the props the author named — it is compiled elsewhere, and cannot see
//! which fields it left out or what they should default to. The derive sees
//! both, so the decision lives here.
//!
//! A prop is *skippable* when its type says what leaving it out means, which is
//! `Option<_>` and nothing else: absent is `None`. A prop that may be skipped is
//! spelled `Option<bool>` rather than `bool` for the same reason a required one
//! is spelled `bool` — the type is where a call site reads whether it has to
//! pass anything.
//!
//! A *required* prop's setter takes its type exactly, as assigning to the field
//! did, so that coercion and integer inference still work at a call site. A
//! *skippable* one takes `impl Into<Option<T>>` instead, so that a call site
//! writes the value and not the `Some` around it — the prop's type has already
//! said that leaving it out is allowed, and saying it a second time at every
//! call site was noise. The conversion a quoted value needs still happens on the
//! value side — `damask::props` has the argument, and it is why `detail="…"`
//! reaches an `Option<String>` prop.
//!
//! Every other prop is required, and carries a type parameter on the builder
//! that starts as a marker named after it and flips to `damask::props::Set` when
//! its setter runs; `__damask_build` demands `Provided` of each, so the diagnostic
//! for a forgotten prop names it.
//!
//! `#[component(default)]` opts the whole struct out of that: the builder starts
//! from `Default::default()` and overwrites what the call site set, so any
//! number of props may be skipped and none are tracked.

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{
    Attribute, DeriveInput, Fields, GenericParam, Generics, Ident, PathArguments, Type, Visibility,
};

/// One field of the struct, seen as a prop.
struct Prop<'a> {
    ident: &'a Ident,
    ty: &'a Type,
    vis: &'a Visibility,
    /// `(parameter, marker)` for a required prop — the builder's type parameter
    /// for it, and the type that stands for "not provided yet".
    tracked: Option<(Ident, Ident)>,
    /// `#[prop(rest)]` — the field every attribute the component does not
    /// declare is collected into. Never tracked, whatever its type says: a bag
    /// a call site fills nothing into is an empty bag, so there is nothing for
    /// a call site to forget.
    rest: bool,
}

/// Everything `#[component(…)]` can say.
pub struct Options {
    /// `#[component(default)]` — every prop may be skipped, filled from `Default`.
    pub defaulted: bool,
    /// `#[component(crate = …)]` — the path generated code reaches this crate
    /// through. A framework that re-exports Damask sets it, because the default
    /// `::damask` only resolves where `damask` is a direct dependency.
    pub krate: syn::Path,
}

impl Options {
    /// The path as the template lowering wants it: a string, since that side
    /// assembles Rust source rather than tokens.
    pub fn krate_str(&self) -> String {
        self.krate.to_token_stream().to_string()
    }
}

/// Generate the builder for `input`, or nothing when the struct has no named
/// props to build (a tuple struct's fields cannot be addressed by name, so it is
/// left as it was: constructible from Rust, but not from a template).
pub fn expand(input: &DeriveInput, options: &Options) -> TokenStream {
    let defaulted = options.defaulted;
    let krate = &options.krate;
    let name = &input.ident;
    let vis = &input.vis;

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
            Fields::Unit => Vec::new(),
            Fields::Unnamed(_) => return TokenStream::new(),
        },
        _ => return TokenStream::new(),
    };

    let mut rest_errors = TokenStream::new();
    let props: Vec<Prop> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let ident = field.ident.as_ref().expect("named field");
            let rest = match is_rest(&field.attrs) {
                Ok(rest) => rest,
                Err(error) => {
                    rest_errors.extend(error.to_compile_error());
                    false
                }
            };
            Prop {
                ident,
                ty: &field.ty,
                vis: &field.vis,
                tracked: (!rest && !defaulted && !is_skippable(&field.ty)).then(|| {
                    (
                        format_ident!("__DamaskM{i}"),
                        format_ident!("__Damask{name}_{ident}"),
                    )
                }),
                rest,
            }
        })
        .collect();

    // One bag, or none. Two would leave the generated `Rest` impl choosing
    // between them, and there is no reading of a call site that says which.
    let rest: Vec<&Prop> = props.iter().filter(|prop| prop.rest).collect();
    if rest.len() > 1 {
        for prop in &rest[1..] {
            rest_errors.extend(
                syn::Error::new(
                    prop.ident.span(),
                    "a component has at most one `#[prop(rest)]` field",
                )
                .to_compile_error(),
            );
        }
    }
    let rest = rest.first().map(|prop| prop.ident);

    let builder = format_ident!("__DamaskProps{name}");
    let store = format_ident!("__DamaskPropStore{name}");

    let names: Vec<&Ident> = props.iter().map(|p| p.ident).collect();
    let types: Vec<&Type> = props.iter().map(|p| p.ty).collect();
    let params: Vec<&Ident> = props
        .iter()
        .filter_map(|p| p.tracked.as_ref().map(|(param, _)| param))
        .collect();
    let markers: Vec<&Ident> = props
        .iter()
        .filter_map(|p| p.tracked.as_ref().map(|(_, marker)| marker))
        .collect();

    let (comp_impl, comp_ty, comp_where) = input.generics.split_for_impl();
    let comp_args = generic_args(&input.generics);
    let declared = declared_params(&input.generics);
    let undefaulted = impl_position_params(&input.generics);

    // The builder's generics in their three positions: as declared (the
    // component's, then one defaulted parameter per required prop), in impl
    // position (no defaults allowed there), and as arguments — the component's,
    // then whichever markers the call site has reached.
    let builder_decl = angled(
        declared
            .iter()
            .cloned()
            .chain(
                params
                    .iter()
                    .zip(&markers)
                    .map(|(param, marker)| quote!(#param = #marker)),
            )
            .collect(),
    );
    let builder_args = |reached: &[TokenStream]| {
        let args = angled(
            comp_args
                .iter()
                .cloned()
                .chain(reached.iter().cloned())
                .collect(),
        );
        quote!(#builder #args)
    };

    let unset: Vec<TokenStream> = markers.iter().map(|m| quote!(#m)).collect();
    let held: Vec<TokenStream> = params.iter().map(|p| quote!(#p)).collect();
    let store_decl = angled(declared.clone());
    let store_args = angled(comp_args.clone());
    let unset_builder = builder_args(&unset);
    let held_builder = builder_args(&held);
    let setter_impl = angled(
        undefaulted
            .iter()
            .cloned()
            .chain(held.iter().cloned())
            .collect(),
    );

    // One setter per prop. The store moves through unchanged, so what a setter
    // costs does not grow with the number of props; only the builder's marker
    // changes, and only for a required prop.
    let setters = props.iter().map(|prop| {
        let (ident, ty, field_vis) = (prop.ident, prop.ty, prop.vis);
        let reached: Vec<TokenStream> = props
            .iter()
            .filter_map(|other| {
                other.tracked.as_ref().map(|(param, _)| {
                    if other.ident == prop.ident {
                        quote!(#krate::props::Set)
                    } else {
                        quote!(#param)
                    }
                })
            })
            .collect();
        let returns = builder_args(&reached);
        // A *required* prop's parameter is its type exactly, which is what a
        // struct literal field was: an argument position with a known type
        // coerces (`&Vec<T>` to `&[T]`), infers an integer literal to the prop's
        // own integer type, and pins a generic component's parameter. A generic
        // `impl Into<…>` parameter gives up all three.
        //
        // A *skippable* one takes `impl Into<Option<T>>`, which keeps none of
        // that and buys the thing a call site actually writes: `rows={4}` and
        // `of={form}` rather than `rows={Some(4)}` and `of={Some(form)}`. The
        // `Some` was never information — the prop's own type already said the
        // value may be absent, so writing it again said nothing and read as
        // noise on every optional prop of every component. Both spellings still
        // compile, since `Option<T>` reaches `Option<T>` reflexively.
        //
        // What it costs is inference where the value alone does not say what it
        // is: `class={None}` no longer knows which `None`, and is written
        // `class={None::<String>}` or, better, left out.
        let parameter = match (prop.rest, is_skippable(ty)) {
            // The bag takes a whole set — `attrs={…}` at a call site, and the
            // `{...expr}` spread — through the one conversion that refuses a
            // string, so markup assembled by hand is a build failure.
            (true, _) => quote!(impl #krate::attr::AttrSet),
            (false, true) => quote!(impl ::core::convert::Into<#ty>),
            (false, false) => quote!(#ty),
        };

        // The quoted form of the same prop: `title="…"`, which is static text
        // rather than a value and reaches the prop through `props::literal`.
        //
        // It is a setter of its own because `literal` infers what to build from
        // where it is going, and an `impl Into<…>` parameter is not a
        // destination — it is a set of them. Here the prop's type is written
        // down, so the inference has exactly one answer whatever the setter
        // above accepts.
        let literal = format_ident!("__damask_literal_{ident}");
        let interpolated = format_ident!("__damask_text_{ident}");
        // The bag accumulates rather than replaces, because the attributes
        // reaching it were written one at a time and in an order the page can
        // see. Every other prop assigns, as a struct field did.
        let store = match prop.rest {
            true => quote! {
                let __damask_bag: &mut #krate::attr::Attrs = self
                    .__damask_store
                    .#ident
                    .get_or_insert_with(::core::default::Default::default);
                __damask_bag.merge(&__damask_value);
            },
            false => quote! {
                self.__damask_store.#ident =
                    ::core::option::Option::Some(__damask_value.into());
            },
        };

        quote! {
            #[doc(hidden)]
            #field_vis fn #ident(mut self, __damask_value: #parameter) -> #returns {
                #store
                #builder {
                    __damask_store: self.__damask_store,
                    __damask_state: ::core::marker::PhantomData,
                }
            }

            #[doc(hidden)]
            #field_vis fn #literal<__DamaskLiteral>(
                self,
                __damask_text: &'static str,
            ) -> #returns
            where
                #ty: #krate::props::FromLiteral<__DamaskLiteral>,
            {
                let __damask_value: #ty =
                    #krate::props::FromLiteral::from_literal(__damask_text);
                self.#ident(__damask_value)
            }

            #[doc(hidden)]
            #field_vis fn #interpolated<__DamaskLiteral>(
                self,
                __damask_text: ::std::string::String,
            ) -> #returns
            where
                #ty: #krate::props::FromInterpolated<__DamaskLiteral>,
            {
                let __damask_value: #ty =
                    #krate::props::FromInterpolated::from_interpolated(__damask_text);
                self.#ident(__damask_value)
            }
        }
    });

    // The bounds sit on `__damask_build` itself rather than on its impl block, so
    // that a call site which has not set every required prop gets an
    // unsatisfied-bound error — which the `Provided` trait can phrase — instead
    // of "no such method".
    let (build_where, build_body) = if defaulted {
        // Every prop is skippable, so nothing is tracked and the base is the
        // struct's own `Default`. Overwriting in place keeps each prop's default
        // exactly what `Default` says it is, and asks nothing of the field types
        // themselves.
        (
            quote!(where #name #comp_ty: ::core::default::Default),
            quote! {
                let __damask_store = self.__damask_store;
                let mut __damask_out = <#name #comp_ty as ::core::default::Default>::default();
                #(
                    if let ::core::option::Option::Some(__damask_value) = __damask_store.#names {
                        __damask_out.#names = __damask_value;
                    }
                )*
                __damask_out
            },
        )
    } else {
        let values = props.iter().map(|prop| {
            let ident = prop.ident;
            match prop.tracked {
                // Unreachable: the `Provided` bounds on `__damask_build` are
                // exactly the proof that every required prop's setter has run.
                Some(_) => quote! {
                    #ident: match __damask_store.#ident {
                        ::core::option::Option::Some(__damask_value) => __damask_value,
                        ::core::option::Option::None => ::core::unreachable!(),
                    }
                },
                // A skippable prop is an `Option<_>`, whose `Default` is `None`
                // whatever it wraps.
                None => quote!(#ident: __damask_store.#ident.unwrap_or_default()),
            }
        });
        let bounds = if params.is_empty() {
            // No required prop to prove set, and an empty `where` is a syntax
            // error rather than a no-op.
            TokenStream::new()
        } else {
            quote!(where #(#params: #krate::props::Provided,)*)
        };
        (
            bounds,
            quote! {
                let __damask_store = self.__damask_store;
                #name { #(#values,)* }
            },
        )
    };

    // Only a component with a bag implements `Rest`, which is what makes the
    // bag opt-in: at a call site the trait bound is the thing that fails, and
    // its `on_unimplemented` is the message a component without one gives for
    // an attribute it does not declare.
    //
    // The `&mut Attrs` binding is written out rather than inferred so that a
    // `#[prop(rest)]` field of some other type is a mismatch reported on the
    // field, instead of a missing method reported inside generated code.
    let rest_impl = rest.map(|ident| {
        quote! {
            impl #setter_impl #krate::props::Rest for #held_builder #comp_where {
                fn __damask_rest<__DamaskValue: #krate::attr::IntoAttrValue>(
                    mut self,
                    __damask_name: &'static str,
                    __damask_value: __DamaskValue,
                ) -> Self {
                    let __damask_bag: &mut #krate::attr::Attrs = self
                        .__damask_store
                        .#ident
                        .get_or_insert_with(::core::default::Default::default);
                    __damask_bag.insert(__damask_name, __damask_value);
                    self
                }

                fn __damask_rest_static(
                    mut self,
                    __damask_name: &'static str,
                    __damask_value: &'static str,
                ) -> Self {
                    let __damask_bag: &mut #krate::attr::Attrs = self
                        .__damask_store
                        .#ident
                        .get_or_insert_with(::core::default::Default::default);
                    __damask_bag.insert_static(__damask_name, __damask_value);
                    self
                }

                fn __damask_rest_bare(mut self, __damask_name: &'static str) -> Self {
                    let __damask_bag: &mut #krate::attr::Attrs = self
                        .__damask_store
                        .#ident
                        .get_or_insert_with(::core::default::Default::default);
                    __damask_bag.insert_bare(__damask_name);
                    self
                }

                fn __damask_rest_spread<__DamaskAttrs: #krate::attr::AttrSet + ?Sized>(
                    mut self,
                    __damask_attrs: &__DamaskAttrs,
                ) -> Self {
                    let __damask_bag: &mut #krate::attr::Attrs = self
                        .__damask_store
                        .#ident
                        .get_or_insert_with(::core::default::Default::default);
                    __damask_bag.merge(__damask_attrs);
                    self
                }
            }
        }
    });

    quote! {
        // What the call site has set so far. Split from the builder so moving
        // through a setter does not restate every prop.
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #vis struct #store #store_decl #comp_where {
            #( #names: ::core::option::Option<#types>, )*
            __damask_component: ::core::marker::PhantomData<fn() -> #name #comp_ty>,
        }

        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #vis struct #builder #builder_decl #comp_where {
            __damask_store: #store #store_args,
            __damask_state: ::core::marker::PhantomData<fn() -> (#(#held,)*)>,
        }

        #(
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            #vis struct #markers;
        )*

        impl #comp_impl #name #comp_ty #comp_where {
            #[doc(hidden)]
            #vis fn __damask_props() -> #unset_builder {
                #builder {
                    __damask_store: #store {
                        #( #names: ::core::option::Option::None, )*
                        __damask_component: ::core::marker::PhantomData,
                    },
                    __damask_state: ::core::marker::PhantomData,
                }
            }
        }

        impl #setter_impl #held_builder #comp_where {
            #(#setters)*

            #[doc(hidden)]
            #vis fn __damask_build(self) -> #name #comp_ty #build_where {
                #build_body
            }
        }

        #rest_impl
        #rest_errors
    }
}

/// Does the type say what leaving the prop out means? `Option<_>` does — absent
/// is `None` — and nothing else does.
fn is_skippable(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    if path.qself.is_some() {
        return false;
    }
    let Some(last) = path.path.segments.last() else {
        return false;
    };
    // Matched on the last segment so `std::option::Option<_>` counts too.
    last.ident == "Option" && matches!(last.arguments, PathArguments::AngleBracketed(_))
}

/// `<a, b>` — or nothing at all, since an empty parameter list reads better
/// absent and components without generics are the common case.
fn angled(items: Vec<TokenStream>) -> TokenStream {
    if items.is_empty() {
        TokenStream::new()
    } else {
        quote!(< #(#items),* >)
    }
}

fn declared_params(generics: &Generics) -> Vec<TokenStream> {
    generics.params.iter().map(|param| quote!(#param)).collect()
}

/// The same parameters with their defaults dropped: a default belongs to a
/// type's declaration and is rejected in impl position.
fn impl_position_params(generics: &Generics) -> Vec<TokenStream> {
    generics
        .params
        .iter()
        .map(|param| {
            let mut param = param.clone();
            match &mut param {
                GenericParam::Type(ty) => {
                    ty.eq_token = None;
                    ty.default = None;
                }
                GenericParam::Const(konst) => {
                    konst.eq_token = None;
                    konst.default = None;
                }
                GenericParam::Lifetime(_) => {}
            }
            quote!(#param)
        })
        .collect()
}

fn generic_args(generics: &Generics) -> Vec<TokenStream> {
    generics
        .params
        .iter()
        .map(|param| match param {
            GenericParam::Lifetime(def) => {
                let lifetime = &def.lifetime;
                quote!(#lifetime)
            }
            GenericParam::Type(ty) => {
                let ident = &ty.ident;
                quote!(#ident)
            }
            GenericParam::Const(konst) => {
                let ident = &konst.ident;
                quote!(#ident)
            }
        })
        .collect()
}

/// Read `#[component(…)]`.
pub fn extract_options(attrs: &[Attribute]) -> syn::Result<Options> {
    let mut defaulted = false;
    let mut krate: Option<syn::Path> = None;

    for attr in attrs {
        if !attr.path().is_ident("component") {
            continue;
        }
        let mut seen = false;
        attr.parse_nested_meta(|meta| {
            seen = true;
            if meta.path.is_ident("default") {
                defaulted = true;
                Ok(())
            } else if meta.path.is_ident("crate") {
                krate = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("unknown `component` option; expected `default` or `crate = …`"))
            }
        })?;
        if !seen {
            return Err(syn::Error::new_spanned(
                attr,
                "`#[component]` requires an option; they are `default` and `crate = …`",
            ));
        }
    }

    Ok(Options {
        defaulted,
        krate: krate.unwrap_or_else(|| syn::parse_quote!(::damask)),
    })
}

/// Read `#[prop(…)]` on a field. `rest` is the only word it takes, and it is
/// what marks the bag every attribute the component does not declare lands in.
fn is_rest(attrs: &[Attribute]) -> syn::Result<bool> {
    let mut rest = false;
    for attr in attrs {
        if !attr.path().is_ident("prop") {
            continue;
        }
        let mut seen = false;
        attr.parse_nested_meta(|meta| {
            seen = true;
            if meta.path.is_ident("rest") {
                rest = true;
                Ok(())
            } else {
                Err(meta.error("unknown `prop` option; the only one is `rest`"))
            }
        })?;
        if !seen {
            return Err(syn::Error::new_spanned(
                attr,
                "`#[prop]` requires an option; the only one is `rest`",
            ));
        }
    }
    Ok(rest)
}
