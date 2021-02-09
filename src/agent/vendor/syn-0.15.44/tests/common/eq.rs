extern crate rustc_data_structures;
extern crate rustc_target;
extern crate syntax;
extern crate syntax_pos;

use std::mem;

use self::rustc_data_structures::sync::Lrc;
use self::rustc_data_structures::thin_vec::ThinVec;
use self::rustc_target::abi::FloatTy;
use self::rustc_target::spec::abi::Abi;
use self::syntax::ast::{
    AngleBracketedArgs, AnonConst, Arg, Arm, AsmDialect, AssocTyConstraint, AssocTyConstraintKind,
    AttrId, AttrStyle, Attribute, AwaitOrigin, BareFnTy, BinOpKind, BindingMode, Block,
    BlockCheckMode, CaptureBy, Constness, Crate, CrateSugar, Defaultness, EnumDef, Expr, ExprKind,
    Field, FieldPat, FnDecl, FnHeader, ForeignItem, ForeignItemKind, ForeignMod, FunctionRetTy,
    GenericArg, GenericArgs, GenericBound, GenericParam, GenericParamKind, Generics, GlobalAsm,
    Ident, ImplItem, ImplItemKind, ImplPolarity, InlineAsm, InlineAsmOutput, IntTy, IsAsync,
    IsAuto, Item, ItemKind, Label, Lifetime, Lit, LitIntType, LitKind, Local, MacDelimiter,
    MacStmtStyle, Mac_, MacroDef, MethodSig, Mod, Movability, MutTy, Mutability, NodeId,
    ParenthesizedArgs, Pat, PatKind, Path, PathSegment, PolyTraitRef, QSelf, RangeEnd, RangeLimits,
    RangeSyntax, Stmt, StmtKind, StrStyle, StructField, TraitBoundModifier, TraitItem,
    TraitItemKind, TraitObjectSyntax, TraitRef, Ty, TyKind, UintTy, UnOp, UnsafeSource, Unsafety,
    UseTree, UseTreeKind, VariantData, Variant_, VisibilityKind, WhereBoundPredicate, WhereClause,
    WhereEqPredicate, WherePredicate, WhereRegionPredicate,
};
use self::syntax::parse::lexer::comments;
use self::syntax::parse::token::{self, DelimToken, Token, TokenKind};
use self::syntax::ptr::P;
use self::syntax::source_map::Spanned;
use self::syntax::symbol::{sym, Symbol};
use self::syntax::tokenstream::{DelimSpan, TokenStream, TokenTree};
use self::syntax_pos::{Span, SyntaxContext, DUMMY_SP};

pub trait SpanlessEq {
    fn eq(&self, other: &Self) -> bool;
}

impl<T: SpanlessEq> SpanlessEq for P<T> {
    fn eq(&self, other: &Self) -> bool {
        SpanlessEq::eq(&**self, &**other)
    }
}

impl<T: SpanlessEq> SpanlessEq for Lrc<T> {
    fn eq(&self, other: &Self) -> bool {
        SpanlessEq::eq(&**self, &**other)
    }
}

impl<T: SpanlessEq> SpanlessEq for Option<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (None, None) => true,
            (Some(this), Some(other)) => SpanlessEq::eq(this, other),
            _ => false,
        }
    }
}

impl<T: SpanlessEq> SpanlessEq for Vec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().zip(other).all(|(a, b)| SpanlessEq::eq(a, b))
    }
}

impl<T: SpanlessEq> SpanlessEq for ThinVec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .zip(other.iter())
                .all(|(a, b)| SpanlessEq::eq(a, b))
    }
}

impl<T: SpanlessEq> SpanlessEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        SpanlessEq::eq(&self.node, &other.node)
    }
}

impl<A: SpanlessEq, B: SpanlessEq> SpanlessEq for (A, B) {
    fn eq(&self, other: &Self) -> bool {
        SpanlessEq::eq(&self.0, &other.0) && SpanlessEq::eq(&self.1, &other.1)
    }
}

impl<A: SpanlessEq, B: SpanlessEq, C: SpanlessEq> SpanlessEq for (A, B, C) {
    fn eq(&self, other: &Self) -> bool {
        SpanlessEq::eq(&self.0, &other.0)
            && SpanlessEq::eq(&self.1, &other.1)
            && SpanlessEq::eq(&self.2, &other.2)
    }
}

macro_rules! spanless_eq_true {
    ($name:ident) => {
        impl SpanlessEq for $name {
            fn eq(&self, _other: &Self) -> bool {
                true
            }
        }
    };
}

spanless_eq_true!(Span);
spanless_eq_true!(DelimSpan);
spanless_eq_true!(AttrId);
spanless_eq_true!(NodeId);
spanless_eq_true!(SyntaxContext);

macro_rules! spanless_eq_partial_eq {
    ($name:ident) => {
        impl SpanlessEq for $name {
            fn eq(&self, other: &Self) -> bool {
                PartialEq::eq(self, other)
            }
        }
    };
}

spanless_eq_partial_eq!(bool);
spanless_eq_partial_eq!(u8);
spanless_eq_partial_eq!(u16);
spanless_eq_partial_eq!(u128);
spanless_eq_partial_eq!(usize);
spanless_eq_partial_eq!(char);
spanless_eq_partial_eq!(Symbol);
spanless_eq_partial_eq!(Abi);
spanless_eq_partial_eq!(DelimToken);

macro_rules! spanless_eq_struct {
    {
        $name:ident;
        $([$field:ident $other:ident])*
        $(![$ignore:ident])*
    } => {
        impl SpanlessEq for $name {
            fn eq(&self, other: &Self) -> bool {
                let $name { $($field,)* $($ignore: _,)* } = self;
                let $name { $($field: $other,)* $($ignore: _,)* } = other;
                $(SpanlessEq::eq($field, $other))&&*
            }
        }
    };

    {
        $name:ident;
        $([$field:ident $other:ident])*
        $next:ident
        $($rest:ident)*
        $(!$ignore:ident)*
    } => {
        spanless_eq_struct! {
            $name;
            $([$field $other])*
            [$next other]
            $($rest)*
            $(!$ignore)*
        }
    };

    {
        $name:ident;
        $([$field:ident $other:ident])*
        $(![$ignore:ident])*
        !$next:ident
        $(!$rest:ident)*
    } => {
        spanless_eq_struct! {
            $name;
            $([$field $other])*
            $(![$ignore])*
            ![$next]
            $(!$rest)*
        }
    };
}

macro_rules! spanless_eq_enum {
    {
        $name:ident;
        $([$variant:ident $([$field:tt $this:ident $other:ident])*])*
    } => {
        impl SpanlessEq for $name {
            fn eq(&self, other: &Self) -> bool {
                match self {
                    $(
                        $name::$variant { .. } => {}
                    )*
                }
                #[allow(unreachable_patterns)]
                match (self, other) {
                    $(
                        (
                            $name::$variant { $($field: $this),* },
                            $name::$variant { $($field: $other),* },
                        ) => {
                            true $(&& SpanlessEq::eq($this, $other))*
                        }
                    )*
                    _ => false,
                }
            }
        }
    };

    {
        $name:ident;
        $([$variant:ident $($fields:tt)*])*
        $next:ident [$($named:tt)*] ( $i:tt $($field:tt)* )
        $($rest:tt)*
    } => {
        spanless_eq_enum! {
            $name;
            $([$variant $($fields)*])*
            $next [$($named)* [$i this other]] ( $($field)* )
            $($rest)*
        }
    };

    {
        $name:ident;
        $([$variant:ident $($fields:tt)*])*
        $next:ident [$($named:tt)*] ()
        $($rest:tt)*
    } => {
        spanless_eq_enum! {
            $name;
            $([$variant $($fields)*])*
            [$next $($named)*]
            $($rest)*
        }
    };

    {
        $name:ident;
        $([$variant:ident $($fields:tt)*])*
        $next:ident ( $($field:tt)* )
        $($rest:tt)*
    } => {
        spanless_eq_enum! {
            $name;
            $([$variant $($fields)*])*
            $next [] ( $($field)* )
            $($rest)*
        }
    };

    {
        $name:ident;
        $([$variant:ident $($fields:tt)*])*
        $next:ident
        $($rest:tt)*
    } => {
        spanless_eq_enum! {
            $name;
            $([$variant $($fields)*])*
            [$next]
            $($rest)*
        }
    };
}

spanless_eq_struct!(AngleBracketedArgs; span args constraints);
spanless_eq_struct!(AnonConst; id value);
spanless_eq_struct!(Arg; attrs ty pat id);
spanless_eq_struct!(Arm; attrs pats guard body span);
spanless_eq_struct!(AssocTyConstraint; id ident kind span);
spanless_eq_struct!(Attribute; id style path tokens span !is_sugared_doc);
spanless_eq_struct!(BareFnTy; unsafety abi generic_params decl);
spanless_eq_struct!(Block; stmts id rules span);
spanless_eq_struct!(Crate; module attrs span);
spanless_eq_struct!(EnumDef; variants);
spanless_eq_struct!(Expr; id node span attrs);
spanless_eq_struct!(Field; ident expr span is_shorthand attrs);
spanless_eq_struct!(FieldPat; ident pat is_shorthand attrs);
spanless_eq_struct!(FnDecl; inputs output c_variadic);
spanless_eq_struct!(FnHeader; constness asyncness unsafety abi);
spanless_eq_struct!(ForeignItem; ident attrs node id span vis);
spanless_eq_struct!(ForeignMod; abi items);
spanless_eq_struct!(GenericParam; id ident attrs bounds kind);
spanless_eq_struct!(Generics; params where_clause span);
spanless_eq_struct!(GlobalAsm; asm ctxt);
spanless_eq_struct!(ImplItem; id ident vis defaultness attrs generics node span !tokens);
spanless_eq_struct!(InlineAsm; asm asm_str_style outputs inputs clobbers volatile alignstack dialect ctxt);
spanless_eq_struct!(InlineAsmOutput; constraint expr is_rw is_indirect);
spanless_eq_struct!(Item; ident attrs id node vis span !tokens);
spanless_eq_struct!(Label; ident);
spanless_eq_struct!(Lifetime; id ident);
spanless_eq_struct!(Lit; token node span);
spanless_eq_struct!(Local; pat ty init id span attrs);
spanless_eq_struct!(Mac_; path delim tts);
spanless_eq_struct!(MacroDef; tokens legacy);
spanless_eq_struct!(MethodSig; header decl);
spanless_eq_struct!(Mod; inner items inline);
spanless_eq_struct!(MutTy; ty mutbl);
spanless_eq_struct!(ParenthesizedArgs; span inputs output);
spanless_eq_struct!(Pat; id node span);
spanless_eq_struct!(Path; span segments);
spanless_eq_struct!(PathSegment; ident id args);
spanless_eq_struct!(PolyTraitRef; bound_generic_params trait_ref span);
spanless_eq_struct!(QSelf; ty path_span position);
spanless_eq_struct!(Stmt; id node span);
spanless_eq_struct!(StructField; span ident vis id ty attrs);
spanless_eq_struct!(Token; kind span);
spanless_eq_struct!(TraitItem; id ident attrs generics node span !tokens);
spanless_eq_struct!(TraitRef; path ref_id);
spanless_eq_struct!(Ty; id node span);
spanless_eq_struct!(UseTree; prefix kind span);
spanless_eq_struct!(Variant_; ident attrs id data disr_expr);
spanless_eq_struct!(WhereBoundPredicate; span bound_generic_params bounded_ty bounds);
spanless_eq_struct!(WhereClause; predicates span);
spanless_eq_struct!(WhereEqPredicate; id span lhs_ty rhs_ty);
spanless_eq_struct!(WhereRegionPredicate; span lifetime bounds);
spanless_eq_enum!(AsmDialect; Att Intel);
spanless_eq_enum!(AssocTyConstraintKind; Equality(ty) Bound(bounds));
spanless_eq_enum!(AttrStyle; Outer Inner);
spanless_eq_enum!(AwaitOrigin; FieldLike MacroLike);
spanless_eq_enum!(BinOpKind; Add Sub Mul Div Rem And Or BitXor BitAnd BitOr Shl Shr Eq Lt Le Ne Ge Gt);
spanless_eq_enum!(BindingMode; ByRef(0) ByValue(0));
spanless_eq_enum!(BlockCheckMode; Default Unsafe(0));
spanless_eq_enum!(CaptureBy; Value Ref);
spanless_eq_enum!(Constness; Const NotConst);
spanless_eq_enum!(CrateSugar; PubCrate JustCrate);
spanless_eq_enum!(Defaultness; Default Final);
spanless_eq_enum!(FloatTy; F32 F64);
spanless_eq_enum!(ForeignItemKind; Fn(0 1) Static(0 1) Ty Macro(0));
spanless_eq_enum!(FunctionRetTy; Default(0) Ty(0));
spanless_eq_enum!(GenericArg; Lifetime(0) Type(0) Const(0));
spanless_eq_enum!(GenericArgs; AngleBracketed(0) Parenthesized(0));
spanless_eq_enum!(GenericBound; Trait(0 1) Outlives(0));
spanless_eq_enum!(GenericParamKind; Lifetime Type(default) Const(ty));
spanless_eq_enum!(ImplItemKind; Const(0 1) Method(0 1) Type(0) Existential(0) Macro(0));
spanless_eq_enum!(ImplPolarity; Positive Negative);
spanless_eq_enum!(IntTy; Isize I8 I16 I32 I64 I128);
spanless_eq_enum!(IsAsync; Async(closure_id return_impl_trait_id) NotAsync);
spanless_eq_enum!(IsAuto; Yes No);
spanless_eq_enum!(LitIntType; Signed(0) Unsigned(0) Unsuffixed);
spanless_eq_enum!(MacDelimiter; Parenthesis Bracket Brace);
spanless_eq_enum!(MacStmtStyle; Semicolon Braces NoBraces);
spanless_eq_enum!(Movability; Static Movable);
spanless_eq_enum!(Mutability; Mutable Immutable);
spanless_eq_enum!(RangeEnd; Included(0) Excluded);
spanless_eq_enum!(RangeLimits; HalfOpen Closed);
spanless_eq_enum!(StmtKind; Local(0) Item(0) Expr(0) Semi(0) Mac(0));
spanless_eq_enum!(StrStyle; Cooked Raw(0));
spanless_eq_enum!(TokenTree; Token(0) Delimited(0 1 2));
spanless_eq_enum!(TraitBoundModifier; None Maybe);
spanless_eq_enum!(TraitItemKind; Const(0 1) Method(0 1) Type(0 1) Macro(0));
spanless_eq_enum!(TraitObjectSyntax; Dyn None);
spanless_eq_enum!(UintTy; Usize U8 U16 U32 U64 U128);
spanless_eq_enum!(UnOp; Deref Not Neg);
spanless_eq_enum!(UnsafeSource; CompilerGenerated UserProvided);
spanless_eq_enum!(Unsafety; Unsafe Normal);
spanless_eq_enum!(UseTreeKind; Simple(0 1 2) Nested(0) Glob);
spanless_eq_enum!(VariantData; Struct(0 1) Tuple(0 1) Unit(0));
spanless_eq_enum!(VisibilityKind; Public Crate(0) Restricted(path id) Inherited);
spanless_eq_enum!(WherePredicate; BoundPredicate(0) RegionPredicate(0) EqPredicate(0));
spanless_eq_enum!(ExprKind; Box(0) Array(0) Call(0 1) MethodCall(0 1) Tup(0)
    Binary(0 1 2) Unary(0 1) Lit(0) Cast(0 1) Type(0 1) Let(0 1) If(0 1 2)
    While(0 1 2) ForLoop(0 1 2 3) Loop(0 1) Match(0 1) Closure(0 1 2 3 4 5)
    Block(0 1) Async(0 1 2) Await(0 1) TryBlock(0) Assign(0 1) AssignOp(0 1 2)
    Field(0 1) Index(0 1) Range(0 1 2) Path(0 1) AddrOf(0 1) Break(0 1)
    Continue(0) Ret(0) InlineAsm(0) Mac(0) Struct(0 1 2) Repeat(0 1) Paren(0)
    Try(0) Yield(0) Err);
spanless_eq_enum!(ItemKind; ExternCrate(0) Use(0) Static(0 1 2) Const(0 1)
    Fn(0 1 2 3) Mod(0) ForeignMod(0) GlobalAsm(0) Ty(0 1) Existential(0 1)
    Enum(0 1) Struct(0 1) Union(0 1) Trait(0 1 2 3 4) TraitAlias(0 1)
    Impl(0 1 2 3 4 5 6) Mac(0) MacroDef(0));
spanless_eq_enum!(LitKind; Str(0 1) ByteStr(0) Byte(0) Char(0) Int(0 1)
    Float(0 1) FloatUnsuffixed(0) Bool(0) Err(0));
spanless_eq_enum!(PatKind; Wild Ident(0 1 2) Struct(0 1 2) TupleStruct(0 1 2)
    Path(0 1) Tuple(0 1) Box(0) Ref(0 1) Lit(0) Range(0 1 2) Slice(0 1 2)
    Paren(0) Mac(0));
spanless_eq_enum!(TyKind; Slice(0) Array(0 1) Ptr(0) Rptr(0 1) BareFn(0) Never
    Tup(0) Path(0 1) TraitObject(0 1) ImplTrait(0 1) Paren(0) Typeof(0) Infer
    ImplicitSelf Mac(0) Err CVarArgs);

impl SpanlessEq for Ident {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

// Give up on comparing literals inside of macros because there are so many
// equivalent representations of the same literal; they are tested elsewhere
impl SpanlessEq for token::Lit {
    fn eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

impl SpanlessEq for RangeSyntax {
    fn eq(&self, _other: &Self) -> bool {
        match self {
            RangeSyntax::DotDotDot | RangeSyntax::DotDotEq => true,
        }
    }
}

impl SpanlessEq for TokenKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TokenKind::Literal(this), TokenKind::Literal(other)) => SpanlessEq::eq(this, other),
            (TokenKind::DotDotEq, _) | (TokenKind::DotDotDot, _) => match other {
                TokenKind::DotDotEq | TokenKind::DotDotDot => true,
                _ => false,
            },
            _ => self == other,
        }
    }
}

impl SpanlessEq for TokenStream {
    fn eq(&self, other: &Self) -> bool {
        SpanlessEq::eq(&expand_tts(self), &expand_tts(other))
    }
}

fn expand_tts(tts: &TokenStream) -> Vec<TokenTree> {
    let mut tokens = Vec::new();
    for tt in tts.clone().into_trees() {
        let c = match tt {
            TokenTree::Token(Token {
                kind: TokenKind::DocComment(c),
                ..
            }) => c,
            _ => {
                tokens.push(tt);
                continue;
            }
        };
        let contents = comments::strip_doc_comment_decoration(&c.as_str());
        let style = comments::doc_comment_style(&c.as_str());
        tokens.push(TokenTree::token(TokenKind::Pound, DUMMY_SP));
        if style == AttrStyle::Inner {
            tokens.push(TokenTree::token(TokenKind::Not, DUMMY_SP));
        }
        let lit = token::Lit {
            kind: token::LitKind::Str,
            symbol: Symbol::intern(&contents),
            suffix: None,
        };
        let tts = vec![
            TokenTree::token(TokenKind::Ident(sym::doc, false), DUMMY_SP),
            TokenTree::token(TokenKind::Eq, DUMMY_SP),
            TokenTree::token(TokenKind::Literal(lit), DUMMY_SP),
        ];
        tokens.push(TokenTree::Delimited(
            DelimSpan::dummy(),
            DelimToken::Bracket,
            tts.into_iter().collect::<TokenStream>().into(),
        ));
    }
    tokens
}
