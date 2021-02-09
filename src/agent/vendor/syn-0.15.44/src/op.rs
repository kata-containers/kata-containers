ast_enum! {
    /// A binary operator: `+`, `+=`, `&`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(feature = "clone-impls", derive(Copy))]
    pub enum BinOp {
        /// The `+` operator (addition)
        Add(Token![+]),
        /// The `-` operator (subtraction)
        Sub(Token![-]),
        /// The `*` operator (multiplication)
        Mul(Token![*]),
        /// The `/` operator (division)
        Div(Token![/]),
        /// The `%` operator (modulus)
        Rem(Token![%]),
        /// The `&&` operator (logical and)
        And(Token![&&]),
        /// The `||` operator (logical or)
        Or(Token![||]),
        /// The `^` operator (bitwise xor)
        BitXor(Token![^]),
        /// The `&` operator (bitwise and)
        BitAnd(Token![&]),
        /// The `|` operator (bitwise or)
        BitOr(Token![|]),
        /// The `<<` operator (shift left)
        Shl(Token![<<]),
        /// The `>>` operator (shift right)
        Shr(Token![>>]),
        /// The `==` operator (equality)
        Eq(Token![==]),
        /// The `<` operator (less than)
        Lt(Token![<]),
        /// The `<=` operator (less than or equal to)
        Le(Token![<=]),
        /// The `!=` operator (not equal to)
        Ne(Token![!=]),
        /// The `>=` operator (greater than or equal to)
        Ge(Token![>=]),
        /// The `>` operator (greater than)
        Gt(Token![>]),
        /// The `+=` operator
        AddEq(Token![+=]),
        /// The `-=` operator
        SubEq(Token![-=]),
        /// The `*=` operator
        MulEq(Token![*=]),
        /// The `/=` operator
        DivEq(Token![/=]),
        /// The `%=` operator
        RemEq(Token![%=]),
        /// The `^=` operator
        BitXorEq(Token![^=]),
        /// The `&=` operator
        BitAndEq(Token![&=]),
        /// The `|=` operator
        BitOrEq(Token![|=]),
        /// The `<<=` operator
        ShlEq(Token![<<=]),
        /// The `>>=` operator
        ShrEq(Token![>>=]),
    }
}

ast_enum! {
    /// A unary operator: `*`, `!`, `-`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(feature = "clone-impls", derive(Copy))]
    pub enum UnOp {
        /// The `*` operator for dereferencing
        Deref(Token![*]),
        /// The `!` operator for logical inversion
        Not(Token![!]),
        /// The `-` operator for negation
        Neg(Token![-]),
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;

    use parse::{Parse, ParseStream, Result};

    fn parse_binop(input: ParseStream) -> Result<BinOp> {
        if input.peek(Token![&&]) {
            input.parse().map(BinOp::And)
        } else if input.peek(Token![||]) {
            input.parse().map(BinOp::Or)
        } else if input.peek(Token![<<]) {
            input.parse().map(BinOp::Shl)
        } else if input.peek(Token![>>]) {
            input.parse().map(BinOp::Shr)
        } else if input.peek(Token![==]) {
            input.parse().map(BinOp::Eq)
        } else if input.peek(Token![<=]) {
            input.parse().map(BinOp::Le)
        } else if input.peek(Token![!=]) {
            input.parse().map(BinOp::Ne)
        } else if input.peek(Token![>=]) {
            input.parse().map(BinOp::Ge)
        } else if input.peek(Token![+]) {
            input.parse().map(BinOp::Add)
        } else if input.peek(Token![-]) {
            input.parse().map(BinOp::Sub)
        } else if input.peek(Token![*]) {
            input.parse().map(BinOp::Mul)
        } else if input.peek(Token![/]) {
            input.parse().map(BinOp::Div)
        } else if input.peek(Token![%]) {
            input.parse().map(BinOp::Rem)
        } else if input.peek(Token![^]) {
            input.parse().map(BinOp::BitXor)
        } else if input.peek(Token![&]) {
            input.parse().map(BinOp::BitAnd)
        } else if input.peek(Token![|]) {
            input.parse().map(BinOp::BitOr)
        } else if input.peek(Token![<]) {
            input.parse().map(BinOp::Lt)
        } else if input.peek(Token![>]) {
            input.parse().map(BinOp::Gt)
        } else {
            Err(input.error("expected binary operator"))
        }
    }

    impl Parse for BinOp {
        #[cfg(not(feature = "full"))]
        fn parse(input: ParseStream) -> Result<Self> {
            parse_binop(input)
        }

        #[cfg(feature = "full")]
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Token![+=]) {
                input.parse().map(BinOp::AddEq)
            } else if input.peek(Token![-=]) {
                input.parse().map(BinOp::SubEq)
            } else if input.peek(Token![*=]) {
                input.parse().map(BinOp::MulEq)
            } else if input.peek(Token![/=]) {
                input.parse().map(BinOp::DivEq)
            } else if input.peek(Token![%=]) {
                input.parse().map(BinOp::RemEq)
            } else if input.peek(Token![^=]) {
                input.parse().map(BinOp::BitXorEq)
            } else if input.peek(Token![&=]) {
                input.parse().map(BinOp::BitAndEq)
            } else if input.peek(Token![|=]) {
                input.parse().map(BinOp::BitOrEq)
            } else if input.peek(Token![<<=]) {
                input.parse().map(BinOp::ShlEq)
            } else if input.peek(Token![>>=]) {
                input.parse().map(BinOp::ShrEq)
            } else {
                parse_binop(input)
            }
        }
    }

    impl Parse for UnOp {
        fn parse(input: ParseStream) -> Result<Self> {
            let lookahead = input.lookahead1();
            if lookahead.peek(Token![*]) {
                input.parse().map(UnOp::Deref)
            } else if lookahead.peek(Token![!]) {
                input.parse().map(UnOp::Not)
            } else if lookahead.peek(Token![-]) {
                input.parse().map(UnOp::Neg)
            } else {
                Err(lookahead.error())
            }
        }
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::ToTokens;

    impl ToTokens for BinOp {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match *self {
                BinOp::Add(ref t) => t.to_tokens(tokens),
                BinOp::Sub(ref t) => t.to_tokens(tokens),
                BinOp::Mul(ref t) => t.to_tokens(tokens),
                BinOp::Div(ref t) => t.to_tokens(tokens),
                BinOp::Rem(ref t) => t.to_tokens(tokens),
                BinOp::And(ref t) => t.to_tokens(tokens),
                BinOp::Or(ref t) => t.to_tokens(tokens),
                BinOp::BitXor(ref t) => t.to_tokens(tokens),
                BinOp::BitAnd(ref t) => t.to_tokens(tokens),
                BinOp::BitOr(ref t) => t.to_tokens(tokens),
                BinOp::Shl(ref t) => t.to_tokens(tokens),
                BinOp::Shr(ref t) => t.to_tokens(tokens),
                BinOp::Eq(ref t) => t.to_tokens(tokens),
                BinOp::Lt(ref t) => t.to_tokens(tokens),
                BinOp::Le(ref t) => t.to_tokens(tokens),
                BinOp::Ne(ref t) => t.to_tokens(tokens),
                BinOp::Ge(ref t) => t.to_tokens(tokens),
                BinOp::Gt(ref t) => t.to_tokens(tokens),
                BinOp::AddEq(ref t) => t.to_tokens(tokens),
                BinOp::SubEq(ref t) => t.to_tokens(tokens),
                BinOp::MulEq(ref t) => t.to_tokens(tokens),
                BinOp::DivEq(ref t) => t.to_tokens(tokens),
                BinOp::RemEq(ref t) => t.to_tokens(tokens),
                BinOp::BitXorEq(ref t) => t.to_tokens(tokens),
                BinOp::BitAndEq(ref t) => t.to_tokens(tokens),
                BinOp::BitOrEq(ref t) => t.to_tokens(tokens),
                BinOp::ShlEq(ref t) => t.to_tokens(tokens),
                BinOp::ShrEq(ref t) => t.to_tokens(tokens),
            }
        }
    }

    impl ToTokens for UnOp {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match *self {
                UnOp::Deref(ref t) => t.to_tokens(tokens),
                UnOp::Not(ref t) => t.to_tokens(tokens),
                UnOp::Neg(ref t) => t.to_tokens(tokens),
            }
        }
    }
}
