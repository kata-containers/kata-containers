use super::*;
use punctuated::{Iter, IterMut, Punctuated};

ast_struct! {
    /// Lifetimes and type parameters attached to a declaration of a function,
    /// enum, trait, etc.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[derive(Default)]
    pub struct Generics {
        pub lt_token: Option<Token![<]>,
        pub params: Punctuated<GenericParam, Token![,]>,
        pub gt_token: Option<Token![>]>,
        pub where_clause: Option<WhereClause>,
    }
}

ast_enum_of_structs! {
    /// A generic type parameter, lifetime, or const generic: `T: Into<String>`,
    /// `'a: 'b`, `const LEN: usize`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: enum.Expr.html#syntax-tree-enums
    //
    // TODO: change syntax-tree-enum link to an intra rustdoc link, currently
    // blocked on https://github.com/rust-lang/rust/issues/62833
    pub enum GenericParam {
        /// A generic type parameter: `T: Into<String>`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Type(TypeParam {
            pub attrs: Vec<Attribute>,
            pub ident: Ident,
            pub colon_token: Option<Token![:]>,
            pub bounds: Punctuated<TypeParamBound, Token![+]>,
            pub eq_token: Option<Token![=]>,
            pub default: Option<Type>,
        }),

        /// A lifetime definition: `'a: 'b + 'c + 'd`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Lifetime(LifetimeDef {
            pub attrs: Vec<Attribute>,
            pub lifetime: Lifetime,
            pub colon_token: Option<Token![:]>,
            pub bounds: Punctuated<Lifetime, Token![+]>,
        }),

        /// A const generic parameter: `const LENGTH: usize`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Const(ConstParam {
            pub attrs: Vec<Attribute>,
            pub const_token: Token![const],
            pub ident: Ident,
            pub colon_token: Token![:],
            pub ty: Type,
            pub eq_token: Option<Token![=]>,
            pub default: Option<Expr>,
        }),
    }
}

impl Generics {
    /// Returns an
    /// <code
    ///   style="padding-right:0;">Iterator&lt;Item = &amp;</code><a
    ///   href="struct.TypeParam.html"><code
    ///   style="padding-left:0;padding-right:0;">TypeParam</code></a><code
    ///   style="padding-left:0;">&gt;</code>
    /// over the type parameters in `self.params`.
    pub fn type_params(&self) -> TypeParams {
        TypeParams(self.params.iter())
    }

    /// Returns an
    /// <code
    ///   style="padding-right:0;">Iterator&lt;Item = &amp;mut </code><a
    ///   href="struct.TypeParam.html"><code
    ///   style="padding-left:0;padding-right:0;">TypeParam</code></a><code
    ///   style="padding-left:0;">&gt;</code>
    /// over the type parameters in `self.params`.
    pub fn type_params_mut(&mut self) -> TypeParamsMut {
        TypeParamsMut(self.params.iter_mut())
    }

    /// Returns an
    /// <code
    ///   style="padding-right:0;">Iterator&lt;Item = &amp;</code><a
    ///   href="struct.LifetimeDef.html"><code
    ///   style="padding-left:0;padding-right:0;">LifetimeDef</code></a><code
    ///   style="padding-left:0;">&gt;</code>
    /// over the lifetime parameters in `self.params`.
    pub fn lifetimes(&self) -> Lifetimes {
        Lifetimes(self.params.iter())
    }

    /// Returns an
    /// <code
    ///   style="padding-right:0;">Iterator&lt;Item = &amp;mut </code><a
    ///   href="struct.LifetimeDef.html"><code
    ///   style="padding-left:0;padding-right:0;">LifetimeDef</code></a><code
    ///   style="padding-left:0;">&gt;</code>
    /// over the lifetime parameters in `self.params`.
    pub fn lifetimes_mut(&mut self) -> LifetimesMut {
        LifetimesMut(self.params.iter_mut())
    }

    /// Returns an
    /// <code
    ///   style="padding-right:0;">Iterator&lt;Item = &amp;</code><a
    ///   href="struct.ConstParam.html"><code
    ///   style="padding-left:0;padding-right:0;">ConstParam</code></a><code
    ///   style="padding-left:0;">&gt;</code>
    /// over the constant parameters in `self.params`.
    pub fn const_params(&self) -> ConstParams {
        ConstParams(self.params.iter())
    }

    /// Returns an
    /// <code
    ///   style="padding-right:0;">Iterator&lt;Item = &amp;mut </code><a
    ///   href="struct.ConstParam.html"><code
    ///   style="padding-left:0;padding-right:0;">ConstParam</code></a><code
    ///   style="padding-left:0;">&gt;</code>
    /// over the constant parameters in `self.params`.
    pub fn const_params_mut(&mut self) -> ConstParamsMut {
        ConstParamsMut(self.params.iter_mut())
    }

    /// Initializes an empty `where`-clause if there is not one present already.
    pub fn make_where_clause(&mut self) -> &mut WhereClause {
        // This is Option::get_or_insert_with in Rust 1.20.
        if self.where_clause.is_none() {
            self.where_clause = Some(WhereClause {
                where_token: <Token![where]>::default(),
                predicates: Punctuated::new(),
            });
        }
        match self.where_clause {
            Some(ref mut where_clause) => where_clause,
            None => unreachable!(),
        }
    }
}

pub struct TypeParams<'a>(Iter<'a, GenericParam>);

impl<'a> Iterator for TypeParams<'a> {
    type Item = &'a TypeParam;

    fn next(&mut self) -> Option<Self::Item> {
        let next = match self.0.next() {
            Some(item) => item,
            None => return None,
        };
        if let GenericParam::Type(ref type_param) = *next {
            Some(type_param)
        } else {
            self.next()
        }
    }
}

pub struct TypeParamsMut<'a>(IterMut<'a, GenericParam>);

impl<'a> Iterator for TypeParamsMut<'a> {
    type Item = &'a mut TypeParam;

    fn next(&mut self) -> Option<Self::Item> {
        let next = match self.0.next() {
            Some(item) => item,
            None => return None,
        };
        if let GenericParam::Type(ref mut type_param) = *next {
            Some(type_param)
        } else {
            self.next()
        }
    }
}

pub struct Lifetimes<'a>(Iter<'a, GenericParam>);

impl<'a> Iterator for Lifetimes<'a> {
    type Item = &'a LifetimeDef;

    fn next(&mut self) -> Option<Self::Item> {
        let next = match self.0.next() {
            Some(item) => item,
            None => return None,
        };
        if let GenericParam::Lifetime(ref lifetime) = *next {
            Some(lifetime)
        } else {
            self.next()
        }
    }
}

pub struct LifetimesMut<'a>(IterMut<'a, GenericParam>);

impl<'a> Iterator for LifetimesMut<'a> {
    type Item = &'a mut LifetimeDef;

    fn next(&mut self) -> Option<Self::Item> {
        let next = match self.0.next() {
            Some(item) => item,
            None => return None,
        };
        if let GenericParam::Lifetime(ref mut lifetime) = *next {
            Some(lifetime)
        } else {
            self.next()
        }
    }
}

pub struct ConstParams<'a>(Iter<'a, GenericParam>);

impl<'a> Iterator for ConstParams<'a> {
    type Item = &'a ConstParam;

    fn next(&mut self) -> Option<Self::Item> {
        let next = match self.0.next() {
            Some(item) => item,
            None => return None,
        };
        if let GenericParam::Const(ref const_param) = *next {
            Some(const_param)
        } else {
            self.next()
        }
    }
}

pub struct ConstParamsMut<'a>(IterMut<'a, GenericParam>);

impl<'a> Iterator for ConstParamsMut<'a> {
    type Item = &'a mut ConstParam;

    fn next(&mut self) -> Option<Self::Item> {
        let next = match self.0.next() {
            Some(item) => item,
            None => return None,
        };
        if let GenericParam::Const(ref mut const_param) = *next {
            Some(const_param)
        } else {
            self.next()
        }
    }
}

/// Returned by `Generics::split_for_impl`.
///
/// *This type is available if Syn is built with the `"derive"` or `"full"`
/// feature and the `"printing"` feature.*
#[cfg(feature = "printing")]
#[cfg_attr(feature = "extra-traits", derive(Debug, Eq, PartialEq, Hash))]
#[cfg_attr(feature = "clone-impls", derive(Clone))]
pub struct ImplGenerics<'a>(&'a Generics);

/// Returned by `Generics::split_for_impl`.
///
/// *This type is available if Syn is built with the `"derive"` or `"full"`
/// feature and the `"printing"` feature.*
#[cfg(feature = "printing")]
#[cfg_attr(feature = "extra-traits", derive(Debug, Eq, PartialEq, Hash))]
#[cfg_attr(feature = "clone-impls", derive(Clone))]
pub struct TypeGenerics<'a>(&'a Generics);

/// Returned by `TypeGenerics::as_turbofish`.
///
/// *This type is available if Syn is built with the `"derive"` or `"full"`
/// feature and the `"printing"` feature.*
#[cfg(feature = "printing")]
#[cfg_attr(feature = "extra-traits", derive(Debug, Eq, PartialEq, Hash))]
#[cfg_attr(feature = "clone-impls", derive(Clone))]
pub struct Turbofish<'a>(&'a Generics);

#[cfg(feature = "printing")]
impl Generics {
    /// Split a type's generics into the pieces required for impl'ing a trait
    /// for that type.
    ///
    /// ```edition2018
    /// # use proc_macro2::{Span, Ident};
    /// # use quote::quote;
    /// #
    /// # fn main() {
    /// #     let generics: syn::Generics = Default::default();
    /// #     let name = Ident::new("MyType", Span::call_site());
    /// #
    /// let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    /// quote! {
    ///     impl #impl_generics MyTrait for #name #ty_generics #where_clause {
    ///         // ...
    ///     }
    /// }
    /// #     ;
    /// # }
    /// ```
    ///
    /// *This method is available if Syn is built with the `"derive"` or
    /// `"full"` feature and the `"printing"` feature.*
    pub fn split_for_impl(&self) -> (ImplGenerics, TypeGenerics, Option<&WhereClause>) {
        (
            ImplGenerics(self),
            TypeGenerics(self),
            self.where_clause.as_ref(),
        )
    }
}

#[cfg(feature = "printing")]
impl<'a> TypeGenerics<'a> {
    /// Turn a type's generics like `<X, Y>` into a turbofish like `::<X, Y>`.
    ///
    /// *This method is available if Syn is built with the `"derive"` or
    /// `"full"` feature and the `"printing"` feature.*
    pub fn as_turbofish(&self) -> Turbofish {
        Turbofish(self.0)
    }
}

ast_struct! {
    /// A set of bound lifetimes: `for<'a, 'b, 'c>`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[derive(Default)]
    pub struct BoundLifetimes {
        pub for_token: Token![for],
        pub lt_token: Token![<],
        pub lifetimes: Punctuated<LifetimeDef, Token![,]>,
        pub gt_token: Token![>],
    }
}

impl LifetimeDef {
    pub fn new(lifetime: Lifetime) -> Self {
        LifetimeDef {
            attrs: Vec::new(),
            lifetime: lifetime,
            colon_token: None,
            bounds: Punctuated::new(),
        }
    }
}

impl From<Ident> for TypeParam {
    fn from(ident: Ident) -> Self {
        TypeParam {
            attrs: vec![],
            ident: ident,
            colon_token: None,
            bounds: Punctuated::new(),
            eq_token: None,
            default: None,
        }
    }
}

ast_enum_of_structs! {
    /// A trait or lifetime used as a bound on a type parameter.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub enum TypeParamBound {
        pub Trait(TraitBound),
        pub Lifetime(Lifetime),
    }
}

ast_struct! {
    /// A trait used as a bound on a type parameter.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub struct TraitBound {
        pub paren_token: Option<token::Paren>,
        pub modifier: TraitBoundModifier,
        /// The `for<'a>` in `for<'a> Foo<&'a T>`
        pub lifetimes: Option<BoundLifetimes>,
        /// The `Foo<&'a T>` in `for<'a> Foo<&'a T>`
        pub path: Path,
    }
}

ast_enum! {
    /// A modifier on a trait bound, currently only used for the `?` in
    /// `?Sized`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(feature = "clone-impls", derive(Copy))]
    pub enum TraitBoundModifier {
        None,
        Maybe(Token![?]),
    }
}

ast_struct! {
    /// A `where` clause in a definition: `where T: Deserialize<'de>, D:
    /// 'static`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub struct WhereClause {
        pub where_token: Token![where],
        pub predicates: Punctuated<WherePredicate, Token![,]>,
    }
}

ast_enum_of_structs! {
    /// A single predicate in a `where` clause: `T: Deserialize<'de>`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: enum.Expr.html#syntax-tree-enums
    //
    // TODO: change syntax-tree-enum link to an intra rustdoc link, currently
    // blocked on https://github.com/rust-lang/rust/issues/62833
    pub enum WherePredicate {
        /// A type predicate in a `where` clause: `for<'c> Foo<'c>: Trait<'c>`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Type(PredicateType {
            /// Any lifetimes from a `for` binding
            pub lifetimes: Option<BoundLifetimes>,
            /// The type being bounded
            pub bounded_ty: Type,
            pub colon_token: Token![:],
            /// Trait and lifetime bounds (`Clone+Send+'static`)
            pub bounds: Punctuated<TypeParamBound, Token![+]>,
        }),

        /// A lifetime predicate in a `where` clause: `'a: 'b + 'c`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Lifetime(PredicateLifetime {
            pub lifetime: Lifetime,
            pub colon_token: Token![:],
            pub bounds: Punctuated<Lifetime, Token![+]>,
        }),

        /// An equality predicate in a `where` clause (unsupported).
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Eq(PredicateEq {
            pub lhs_ty: Type,
            pub eq_token: Token![=],
            pub rhs_ty: Type,
        }),
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;

    use parse::{Parse, ParseStream, Result};

    impl Parse for Generics {
        fn parse(input: ParseStream) -> Result<Self> {
            if !input.peek(Token![<]) {
                return Ok(Generics::default());
            }

            let lt_token: Token![<] = input.parse()?;

            let mut params = Punctuated::new();
            let mut allow_lifetime_param = true;
            let mut allow_type_param = true;
            loop {
                if input.peek(Token![>]) {
                    break;
                }

                let attrs = input.call(Attribute::parse_outer)?;
                let lookahead = input.lookahead1();
                if allow_lifetime_param && lookahead.peek(Lifetime) {
                    params.push_value(GenericParam::Lifetime(LifetimeDef {
                        attrs: attrs,
                        ..input.parse()?
                    }));
                } else if allow_type_param && lookahead.peek(Ident) {
                    allow_lifetime_param = false;
                    params.push_value(GenericParam::Type(TypeParam {
                        attrs: attrs,
                        ..input.parse()?
                    }));
                } else if lookahead.peek(Token![const]) {
                    allow_lifetime_param = false;
                    allow_type_param = false;
                    params.push_value(GenericParam::Const(ConstParam {
                        attrs: attrs,
                        ..input.parse()?
                    }));
                } else {
                    return Err(lookahead.error());
                }

                if input.peek(Token![>]) {
                    break;
                }
                let punct = input.parse()?;
                params.push_punct(punct);
            }

            let gt_token: Token![>] = input.parse()?;

            Ok(Generics {
                lt_token: Some(lt_token),
                params: params,
                gt_token: Some(gt_token),
                where_clause: None,
            })
        }
    }

    impl Parse for GenericParam {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;

            let lookahead = input.lookahead1();
            if lookahead.peek(Ident) {
                Ok(GenericParam::Type(TypeParam {
                    attrs: attrs,
                    ..input.parse()?
                }))
            } else if lookahead.peek(Lifetime) {
                Ok(GenericParam::Lifetime(LifetimeDef {
                    attrs: attrs,
                    ..input.parse()?
                }))
            } else if lookahead.peek(Token![const]) {
                Ok(GenericParam::Const(ConstParam {
                    attrs: attrs,
                    ..input.parse()?
                }))
            } else {
                Err(lookahead.error())
            }
        }
    }

    impl Parse for LifetimeDef {
        fn parse(input: ParseStream) -> Result<Self> {
            let has_colon;
            Ok(LifetimeDef {
                attrs: input.call(Attribute::parse_outer)?,
                lifetime: input.parse()?,
                colon_token: {
                    if input.peek(Token![:]) {
                        has_colon = true;
                        Some(input.parse()?)
                    } else {
                        has_colon = false;
                        None
                    }
                },
                bounds: {
                    let mut bounds = Punctuated::new();
                    if has_colon {
                        loop {
                            if input.peek(Token![,]) || input.peek(Token![>]) {
                                break;
                            }
                            let value = input.parse()?;
                            bounds.push_value(value);
                            if !input.peek(Token![+]) {
                                break;
                            }
                            let punct = input.parse()?;
                            bounds.push_punct(punct);
                        }
                    }
                    bounds
                },
            })
        }
    }

    impl Parse for BoundLifetimes {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(BoundLifetimes {
                for_token: input.parse()?,
                lt_token: input.parse()?,
                lifetimes: {
                    let mut lifetimes = Punctuated::new();
                    while !input.peek(Token![>]) {
                        lifetimes.push_value(input.parse()?);
                        if input.peek(Token![>]) {
                            break;
                        }
                        lifetimes.push_punct(input.parse()?);
                    }
                    lifetimes
                },
                gt_token: input.parse()?,
            })
        }
    }

    impl Parse for Option<BoundLifetimes> {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Token![for]) {
                input.parse().map(Some)
            } else {
                Ok(None)
            }
        }
    }

    impl Parse for TypeParam {
        fn parse(input: ParseStream) -> Result<Self> {
            let has_colon;
            let has_default;
            Ok(TypeParam {
                attrs: input.call(Attribute::parse_outer)?,
                ident: input.parse()?,
                colon_token: {
                    if input.peek(Token![:]) {
                        has_colon = true;
                        Some(input.parse()?)
                    } else {
                        has_colon = false;
                        None
                    }
                },
                bounds: {
                    let mut bounds = Punctuated::new();
                    if has_colon {
                        loop {
                            if input.peek(Token![,])
                                || input.peek(Token![>])
                                || input.peek(Token![=])
                            {
                                break;
                            }
                            let value = input.parse()?;
                            bounds.push_value(value);
                            if !input.peek(Token![+]) {
                                break;
                            }
                            let punct = input.parse()?;
                            bounds.push_punct(punct);
                        }
                    }
                    bounds
                },
                eq_token: {
                    if input.peek(Token![=]) {
                        has_default = true;
                        Some(input.parse()?)
                    } else {
                        has_default = false;
                        None
                    }
                },
                default: {
                    if has_default {
                        Some(input.parse()?)
                    } else {
                        None
                    }
                },
            })
        }
    }

    impl Parse for TypeParamBound {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Lifetime) {
                return input.parse().map(TypeParamBound::Lifetime);
            }

            if input.peek(token::Paren) {
                let content;
                let paren_token = parenthesized!(content in input);
                let mut bound: TraitBound = content.parse()?;
                bound.paren_token = Some(paren_token);
                return Ok(TypeParamBound::Trait(bound));
            }

            input.parse().map(TypeParamBound::Trait)
        }
    }

    impl Parse for TraitBound {
        fn parse(input: ParseStream) -> Result<Self> {
            let modifier: TraitBoundModifier = input.parse()?;
            let lifetimes: Option<BoundLifetimes> = input.parse()?;

            let mut path: Path = input.parse()?;
            if path.segments.last().unwrap().value().arguments.is_empty()
                && input.peek(token::Paren)
            {
                let parenthesized = PathArguments::Parenthesized(input.parse()?);
                path.segments.last_mut().unwrap().value_mut().arguments = parenthesized;
            }

            Ok(TraitBound {
                paren_token: None,
                modifier: modifier,
                lifetimes: lifetimes,
                path: path,
            })
        }
    }

    impl Parse for TraitBoundModifier {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Token![?]) {
                input.parse().map(TraitBoundModifier::Maybe)
            } else {
                Ok(TraitBoundModifier::None)
            }
        }
    }

    impl Parse for ConstParam {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut default = None;
            Ok(ConstParam {
                attrs: input.call(Attribute::parse_outer)?,
                const_token: input.parse()?,
                ident: input.parse()?,
                colon_token: input.parse()?,
                ty: input.parse()?,
                eq_token: {
                    if input.peek(Token![=]) {
                        let eq_token = input.parse()?;
                        default = Some(input.parse::<Expr>()?);
                        Some(eq_token)
                    } else {
                        None
                    }
                },
                default: default,
            })
        }
    }

    impl Parse for WhereClause {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(WhereClause {
                where_token: input.parse()?,
                predicates: {
                    let mut predicates = Punctuated::new();
                    loop {
                        if input.is_empty()
                            || input.peek(token::Brace)
                            || input.peek(Token![,])
                            || input.peek(Token![;])
                            || input.peek(Token![:]) && !input.peek(Token![::])
                            || input.peek(Token![=])
                        {
                            break;
                        }
                        let value = input.parse()?;
                        predicates.push_value(value);
                        if !input.peek(Token![,]) {
                            break;
                        }
                        let punct = input.parse()?;
                        predicates.push_punct(punct);
                    }
                    predicates
                },
            })
        }
    }

    impl Parse for Option<WhereClause> {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Token![where]) {
                input.parse().map(Some)
            } else {
                Ok(None)
            }
        }
    }

    impl Parse for WherePredicate {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Lifetime) && input.peek2(Token![:]) {
                Ok(WherePredicate::Lifetime(PredicateLifetime {
                    lifetime: input.parse()?,
                    colon_token: input.parse()?,
                    bounds: {
                        let mut bounds = Punctuated::new();
                        loop {
                            if input.is_empty()
                                || input.peek(token::Brace)
                                || input.peek(Token![,])
                                || input.peek(Token![;])
                                || input.peek(Token![:])
                                || input.peek(Token![=])
                            {
                                break;
                            }
                            let value = input.parse()?;
                            bounds.push_value(value);
                            if !input.peek(Token![+]) {
                                break;
                            }
                            let punct = input.parse()?;
                            bounds.push_punct(punct);
                        }
                        bounds
                    },
                }))
            } else {
                Ok(WherePredicate::Type(PredicateType {
                    lifetimes: input.parse()?,
                    bounded_ty: input.parse()?,
                    colon_token: input.parse()?,
                    bounds: {
                        let mut bounds = Punctuated::new();
                        loop {
                            if input.is_empty()
                                || input.peek(token::Brace)
                                || input.peek(Token![,])
                                || input.peek(Token![;])
                                || input.peek(Token![:]) && !input.peek(Token![::])
                                || input.peek(Token![=])
                            {
                                break;
                            }
                            let value = input.parse()?;
                            bounds.push_value(value);
                            if !input.peek(Token![+]) {
                                break;
                            }
                            let punct = input.parse()?;
                            bounds.push_punct(punct);
                        }
                        bounds
                    },
                }))
            }
        }
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;

    use proc_macro2::TokenStream;
    use quote::{ToTokens, TokenStreamExt};

    use attr::FilterAttrs;
    use print::TokensOrDefault;

    impl ToTokens for Generics {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            if self.params.is_empty() {
                return;
            }

            TokensOrDefault(&self.lt_token).to_tokens(tokens);

            // Print lifetimes before types and consts, regardless of their
            // order in self.params.
            //
            // TODO: ordering rules for const parameters vs type parameters have
            // not been settled yet. https://github.com/rust-lang/rust/issues/44580
            let mut trailing_or_empty = true;
            for param in self.params.pairs() {
                if let GenericParam::Lifetime(_) = **param.value() {
                    param.to_tokens(tokens);
                    trailing_or_empty = param.punct().is_some();
                }
            }
            for param in self.params.pairs() {
                match **param.value() {
                    GenericParam::Type(_) | GenericParam::Const(_) => {
                        if !trailing_or_empty {
                            <Token![,]>::default().to_tokens(tokens);
                            trailing_or_empty = true;
                        }
                        param.to_tokens(tokens);
                    }
                    GenericParam::Lifetime(_) => {}
                }
            }

            TokensOrDefault(&self.gt_token).to_tokens(tokens);
        }
    }

    impl<'a> ToTokens for ImplGenerics<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            if self.0.params.is_empty() {
                return;
            }

            TokensOrDefault(&self.0.lt_token).to_tokens(tokens);

            // Print lifetimes before types and consts, regardless of their
            // order in self.params.
            //
            // TODO: ordering rules for const parameters vs type parameters have
            // not been settled yet. https://github.com/rust-lang/rust/issues/44580
            let mut trailing_or_empty = true;
            for param in self.0.params.pairs() {
                if let GenericParam::Lifetime(_) = **param.value() {
                    param.to_tokens(tokens);
                    trailing_or_empty = param.punct().is_some();
                }
            }
            for param in self.0.params.pairs() {
                if let GenericParam::Lifetime(_) = **param.value() {
                    continue;
                }
                if !trailing_or_empty {
                    <Token![,]>::default().to_tokens(tokens);
                    trailing_or_empty = true;
                }
                match **param.value() {
                    GenericParam::Lifetime(_) => unreachable!(),
                    GenericParam::Type(ref param) => {
                        // Leave off the type parameter defaults
                        tokens.append_all(param.attrs.outer());
                        param.ident.to_tokens(tokens);
                        if !param.bounds.is_empty() {
                            TokensOrDefault(&param.colon_token).to_tokens(tokens);
                            param.bounds.to_tokens(tokens);
                        }
                    }
                    GenericParam::Const(ref param) => {
                        // Leave off the const parameter defaults
                        tokens.append_all(param.attrs.outer());
                        param.const_token.to_tokens(tokens);
                        param.ident.to_tokens(tokens);
                        param.colon_token.to_tokens(tokens);
                        param.ty.to_tokens(tokens);
                    }
                }
                param.punct().to_tokens(tokens);
            }

            TokensOrDefault(&self.0.gt_token).to_tokens(tokens);
        }
    }

    impl<'a> ToTokens for TypeGenerics<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            if self.0.params.is_empty() {
                return;
            }

            TokensOrDefault(&self.0.lt_token).to_tokens(tokens);

            // Print lifetimes before types and consts, regardless of their
            // order in self.params.
            //
            // TODO: ordering rules for const parameters vs type parameters have
            // not been settled yet. https://github.com/rust-lang/rust/issues/44580
            let mut trailing_or_empty = true;
            for param in self.0.params.pairs() {
                if let GenericParam::Lifetime(ref def) = **param.value() {
                    // Leave off the lifetime bounds and attributes
                    def.lifetime.to_tokens(tokens);
                    param.punct().to_tokens(tokens);
                    trailing_or_empty = param.punct().is_some();
                }
            }
            for param in self.0.params.pairs() {
                if let GenericParam::Lifetime(_) = **param.value() {
                    continue;
                }
                if !trailing_or_empty {
                    <Token![,]>::default().to_tokens(tokens);
                    trailing_or_empty = true;
                }
                match **param.value() {
                    GenericParam::Lifetime(_) => unreachable!(),
                    GenericParam::Type(ref param) => {
                        // Leave off the type parameter defaults
                        param.ident.to_tokens(tokens);
                    }
                    GenericParam::Const(ref param) => {
                        // Leave off the const parameter defaults
                        param.ident.to_tokens(tokens);
                    }
                }
                param.punct().to_tokens(tokens);
            }

            TokensOrDefault(&self.0.gt_token).to_tokens(tokens);
        }
    }

    impl<'a> ToTokens for Turbofish<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            if !self.0.params.is_empty() {
                <Token![::]>::default().to_tokens(tokens);
                TypeGenerics(self.0).to_tokens(tokens);
            }
        }
    }

    impl ToTokens for BoundLifetimes {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.for_token.to_tokens(tokens);
            self.lt_token.to_tokens(tokens);
            self.lifetimes.to_tokens(tokens);
            self.gt_token.to_tokens(tokens);
        }
    }

    impl ToTokens for LifetimeDef {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.lifetime.to_tokens(tokens);
            if !self.bounds.is_empty() {
                TokensOrDefault(&self.colon_token).to_tokens(tokens);
                self.bounds.to_tokens(tokens);
            }
        }
    }

    impl ToTokens for TypeParam {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.ident.to_tokens(tokens);
            if !self.bounds.is_empty() {
                TokensOrDefault(&self.colon_token).to_tokens(tokens);
                self.bounds.to_tokens(tokens);
            }
            if self.default.is_some() {
                TokensOrDefault(&self.eq_token).to_tokens(tokens);
                self.default.to_tokens(tokens);
            }
        }
    }

    impl ToTokens for TraitBound {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let to_tokens = |tokens: &mut TokenStream| {
                self.modifier.to_tokens(tokens);
                self.lifetimes.to_tokens(tokens);
                self.path.to_tokens(tokens);
            };
            match self.paren_token {
                Some(ref paren) => paren.surround(tokens, to_tokens),
                None => to_tokens(tokens),
            }
        }
    }

    impl ToTokens for TraitBoundModifier {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match *self {
                TraitBoundModifier::None => {}
                TraitBoundModifier::Maybe(ref t) => t.to_tokens(tokens),
            }
        }
    }

    impl ToTokens for ConstParam {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.const_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
            if self.default.is_some() {
                TokensOrDefault(&self.eq_token).to_tokens(tokens);
                self.default.to_tokens(tokens);
            }
        }
    }

    impl ToTokens for WhereClause {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            if !self.predicates.is_empty() {
                self.where_token.to_tokens(tokens);
                self.predicates.to_tokens(tokens);
            }
        }
    }

    impl ToTokens for PredicateType {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.lifetimes.to_tokens(tokens);
            self.bounded_ty.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.bounds.to_tokens(tokens);
        }
    }

    impl ToTokens for PredicateLifetime {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.lifetime.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.bounds.to_tokens(tokens);
        }
    }

    impl ToTokens for PredicateEq {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.lhs_ty.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            self.rhs_ty.to_tokens(tokens);
        }
    }
}
