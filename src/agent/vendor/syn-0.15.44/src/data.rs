use super::*;
use punctuated::Punctuated;

ast_struct! {
    /// An enum variant.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub struct Variant {
        /// Attributes tagged on the variant.
        pub attrs: Vec<Attribute>,

        /// Name of the variant.
        pub ident: Ident,

        /// Content stored in the variant.
        pub fields: Fields,

        /// Explicit discriminant: `Variant = 1`
        pub discriminant: Option<(Token![=], Expr)>,
    }
}

ast_enum_of_structs! {
    /// Data stored within an enum variant or struct.
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
    pub enum Fields {
        /// Named fields of a struct or struct variant such as `Point { x: f64,
        /// y: f64 }`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Named(FieldsNamed {
            pub brace_token: token::Brace,
            pub named: Punctuated<Field, Token![,]>,
        }),

        /// Unnamed fields of a tuple struct or tuple variant such as `Some(T)`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Unnamed(FieldsUnnamed {
            pub paren_token: token::Paren,
            pub unnamed: Punctuated<Field, Token![,]>,
        }),

        /// Unit struct or unit variant such as `None`.
        pub Unit,
    }
}

impl Fields {
    /// Get an iterator over the borrowed [`Field`] items in this object. This
    /// iterator can be used to iterate over a named or unnamed struct or
    /// variant's fields uniformly.
    pub fn iter(&self) -> punctuated::Iter<Field> {
        match *self {
            Fields::Unit => private::empty_punctuated_iter(),
            Fields::Named(ref f) => f.named.iter(),
            Fields::Unnamed(ref f) => f.unnamed.iter(),
        }
    }

    /// Get an iterator over the mutably borrowed [`Field`] items in this
    /// object. This iterator can be used to iterate over a named or unnamed
    /// struct or variant's fields uniformly.
    pub fn iter_mut(&mut self) -> punctuated::IterMut<Field> {
        match *self {
            Fields::Unit => private::empty_punctuated_iter_mut(),
            Fields::Named(ref mut f) => f.named.iter_mut(),
            Fields::Unnamed(ref mut f) => f.unnamed.iter_mut(),
        }
    }
}

impl<'a> IntoIterator for &'a Fields {
    type Item = &'a Field;
    type IntoIter = punctuated::Iter<'a, Field>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Fields {
    type Item = &'a mut Field;
    type IntoIter = punctuated::IterMut<'a, Field>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

ast_struct! {
    /// A field of a struct or enum variant.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub struct Field {
        /// Attributes tagged on the field.
        pub attrs: Vec<Attribute>,

        /// Visibility of the field.
        pub vis: Visibility,

        /// Name of the field, if any.
        ///
        /// Fields of tuple structs have no names.
        pub ident: Option<Ident>,

        pub colon_token: Option<Token![:]>,

        /// Type of the field.
        pub ty: Type,
    }
}

ast_enum_of_structs! {
    /// The visibility level of an item: inherited or `pub` or
    /// `pub(restricted)`.
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
    pub enum Visibility {
        /// A public visibility level: `pub`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Public(VisPublic {
            pub pub_token: Token![pub],
        }),

        /// A crate-level visibility: `crate`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Crate(VisCrate {
            pub crate_token: Token![crate],
        }),

        /// A visibility level restricted to some path: `pub(self)` or
        /// `pub(super)` or `pub(crate)` or `pub(in some::module)`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Restricted(VisRestricted {
            pub pub_token: Token![pub],
            pub paren_token: token::Paren,
            pub in_token: Option<Token![in]>,
            pub path: Box<Path>,
        }),

        /// An inherited visibility, which usually means private.
        pub Inherited,
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;

    use ext::IdentExt;
    use parse::{Parse, ParseStream, Result};

    impl Parse for Variant {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(Variant {
                attrs: input.call(Attribute::parse_outer)?,
                ident: input.parse()?,
                fields: {
                    if input.peek(token::Brace) {
                        Fields::Named(input.parse()?)
                    } else if input.peek(token::Paren) {
                        Fields::Unnamed(input.parse()?)
                    } else {
                        Fields::Unit
                    }
                },
                discriminant: {
                    if input.peek(Token![=]) {
                        let eq_token: Token![=] = input.parse()?;
                        let discriminant: Expr = input.parse()?;
                        Some((eq_token, discriminant))
                    } else {
                        None
                    }
                },
            })
        }
    }

    impl Parse for FieldsNamed {
        fn parse(input: ParseStream) -> Result<Self> {
            let content;
            Ok(FieldsNamed {
                brace_token: braced!(content in input),
                named: content.parse_terminated(Field::parse_named)?,
            })
        }
    }

    impl Parse for FieldsUnnamed {
        fn parse(input: ParseStream) -> Result<Self> {
            let content;
            Ok(FieldsUnnamed {
                paren_token: parenthesized!(content in input),
                unnamed: content.parse_terminated(Field::parse_unnamed)?,
            })
        }
    }

    impl Field {
        /// Parses a named (braced struct) field.
        pub fn parse_named(input: ParseStream) -> Result<Self> {
            Ok(Field {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                ident: Some(input.parse()?),
                colon_token: Some(input.parse()?),
                ty: input.parse()?,
            })
        }

        /// Parses an unnamed (tuple struct) field.
        pub fn parse_unnamed(input: ParseStream) -> Result<Self> {
            Ok(Field {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                ident: None,
                colon_token: None,
                ty: input.parse()?,
            })
        }
    }

    impl Parse for Visibility {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Token![pub]) {
                Self::parse_pub(input)
            } else if input.peek(Token![crate]) {
                Self::parse_crate(input)
            } else {
                Ok(Visibility::Inherited)
            }
        }
    }

    impl Visibility {
        fn parse_pub(input: ParseStream) -> Result<Self> {
            let pub_token = input.parse::<Token![pub]>()?;

            if input.peek(token::Paren) {
                // TODO: optimize using advance_to
                let ahead = input.fork();
                let mut content;
                parenthesized!(content in ahead);

                if content.peek(Token![crate])
                    || content.peek(Token![self])
                    || content.peek(Token![super])
                {
                    return Ok(Visibility::Restricted(VisRestricted {
                        pub_token: pub_token,
                        paren_token: parenthesized!(content in input),
                        in_token: None,
                        path: Box::new(Path::from(content.call(Ident::parse_any)?)),
                    }));
                } else if content.peek(Token![in]) {
                    return Ok(Visibility::Restricted(VisRestricted {
                        pub_token: pub_token,
                        paren_token: parenthesized!(content in input),
                        in_token: Some(content.parse()?),
                        path: Box::new(content.call(Path::parse_mod_style)?),
                    }));
                }
            }

            Ok(Visibility::Public(VisPublic {
                pub_token: pub_token,
            }))
        }

        fn parse_crate(input: ParseStream) -> Result<Self> {
            if input.peek2(Token![::]) {
                Ok(Visibility::Inherited)
            } else {
                Ok(Visibility::Crate(VisCrate {
                    crate_token: input.parse()?,
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

    use print::TokensOrDefault;

    impl ToTokens for Variant {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(&self.attrs);
            self.ident.to_tokens(tokens);
            self.fields.to_tokens(tokens);
            if let Some((ref eq_token, ref disc)) = self.discriminant {
                eq_token.to_tokens(tokens);
                disc.to_tokens(tokens);
            }
        }
    }

    impl ToTokens for FieldsNamed {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.brace_token.surround(tokens, |tokens| {
                self.named.to_tokens(tokens);
            });
        }
    }

    impl ToTokens for FieldsUnnamed {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.paren_token.surround(tokens, |tokens| {
                self.unnamed.to_tokens(tokens);
            });
        }
    }

    impl ToTokens for Field {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(&self.attrs);
            self.vis.to_tokens(tokens);
            if let Some(ref ident) = self.ident {
                ident.to_tokens(tokens);
                TokensOrDefault(&self.colon_token).to_tokens(tokens);
            }
            self.ty.to_tokens(tokens);
        }
    }

    impl ToTokens for VisPublic {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.pub_token.to_tokens(tokens)
        }
    }

    impl ToTokens for VisCrate {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.crate_token.to_tokens(tokens);
        }
    }

    impl ToTokens for VisRestricted {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.pub_token.to_tokens(tokens);
            self.paren_token.surround(tokens, |tokens| {
                // TODO: If we have a path which is not "self" or "super" or
                // "crate", automatically add the "in" token.
                self.in_token.to_tokens(tokens);
                self.path.to_tokens(tokens);
            });
        }
    }
}
