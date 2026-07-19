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
//! A setter takes its prop's type exactly, as assigning to the field did, so
//! that coercion and integer inference still work at a call site. The conversion
//! a quoted value needs happens on the value side instead — `damask::props` has the
//! argument, and it is why `detail="…"` reaches an `Option<String>` prop.
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
use quote::{format_ident, quote};
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
}

/// Generate the builder for `input`, or nothing when the struct has no named
/// props to build (a tuple struct's fields cannot be addressed by name, so it is
/// left as it was: constructible from Rust, but not from a template).
pub fn expand(input: &DeriveInput, defaulted: bool) -> TokenStream {
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

    let props: Vec<Prop> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let ident = field.ident.as_ref().expect("named field");
            Prop {
                ident,
                ty: &field.ty,
                vis: &field.vis,
                tracked: (!defaulted && !is_skippable(&field.ty)).then(|| {
                    (
                        format_ident!("__DamaskM{i}"),
                        format_ident!("__Damask{name}_{ident}"),
                    )
                }),
            }
        })
        .collect();

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
                        quote!(::damask::props::Set)
                    } else {
                        quote!(#param)
                    }
                })
            })
            .collect();
        let returns = builder_args(&reached);
        // The parameter is the prop's type exactly, which is what a struct
        // literal field was: an argument position with a known type coerces
        // (`&Vec<T>` to `&[T]`), infers an integer literal to the prop's own
        // integer type, and pins a generic component's parameter. A generic
        // `impl Into<…>` parameter would give up all three, so the conversion a
        // quoted value needs is done on the *value* side instead — see
        // `damask::props::literal`.
        quote! {
            #[doc(hidden)]
            #field_vis fn #ident(mut self, __damask_value: #ty) -> #returns {
                self.__damask_store.#ident = ::core::option::Option::Some(__damask_value);
                #builder {
                    __damask_store: self.__damask_store,
                    __damask_state: ::core::marker::PhantomData,
                }
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
            quote!(where #(#params: ::damask::props::Provided,)*)
        };
        (
            bounds,
            quote! {
                let __damask_store = self.__damask_store;
                #name { #(#values,)* }
            },
        )
    };

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

/// Read `#[component(default)]`, the opt-in that lets a call site skip any prop.
pub fn extract_defaulted(attrs: &[Attribute]) -> syn::Result<bool> {
    let mut found = false;
    for attr in attrs {
        if !attr.path().is_ident("component") {
            continue;
        }
        let mut seen = false;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("default") {
                seen = true;
                Ok(())
            } else {
                Err(meta.error("unknown `component` option; expected `default`"))
            }
        })?;
        if !seen {
            return Err(syn::Error::new_spanned(
                attr,
                "`#[component]` requires an option; the only one is `default`",
            ));
        }
        found = true;
    }
    Ok(found)
}
