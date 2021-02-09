use super::*;
use proc_macro2::{Span, TokenStream};
use punctuated::Punctuated;
#[cfg(feature = "extra-traits")]
use std::hash::{Hash, Hasher};
#[cfg(all(feature = "parsing", feature = "full"))]
use std::mem;
#[cfg(feature = "extra-traits")]
use tt::TokenStreamHelper;

ast_enum_of_structs! {
    /// A Rust expression.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    ///
    /// # Syntax tree enums
    ///
    /// This type is a syntax tree enum. In Syn this and other syntax tree enums
    /// are designed to be traversed using the following rebinding idiom.
    ///
    /// ```edition2018
    /// # use syn::Expr;
    /// #
    /// # fn example(expr: Expr) {
    /// # const IGNORE: &str = stringify! {
    /// let expr: Expr = /* ... */;
    /// # };
    /// match expr {
    ///     Expr::MethodCall(expr) => {
    ///         /* ... */
    ///     }
    ///     Expr::Cast(expr) => {
    ///         /* ... */
    ///     }
    ///     Expr::If(expr) => {
    ///         /* ... */
    ///     }
    ///
    ///     /* ... */
    ///     # _ => {}
    /// # }
    /// # }
    /// ```
    ///
    /// We begin with a variable `expr` of type `Expr` that has no fields
    /// (because it is an enum), and by matching on it and rebinding a variable
    /// with the same name `expr` we effectively imbue our variable with all of
    /// the data fields provided by the variant that it turned out to be. So for
    /// example above if we ended up in the `MethodCall` case then we get to use
    /// `expr.receiver`, `expr.args` etc; if we ended up in the `If` case we get
    /// to use `expr.cond`, `expr.then_branch`, `expr.else_branch`.
    ///
    /// This approach avoids repeating the variant names twice on every line.
    ///
    /// ```edition2018
    /// # use syn::{Expr, ExprMethodCall};
    /// #
    /// # fn example(expr: Expr) {
    /// // Repetitive; recommend not doing this.
    /// match expr {
    ///     Expr::MethodCall(ExprMethodCall { method, args, .. }) => {
    /// # }
    /// # _ => {}
    /// # }
    /// # }
    /// ```
    ///
    /// In general, the name to which a syntax tree enum variant is bound should
    /// be a suitable name for the complete syntax tree enum type.
    ///
    /// ```edition2018
    /// # use syn::{Expr, ExprField};
    /// #
    /// # fn example(discriminant: ExprField) {
    /// // Binding is called `base` which is the name I would use if I were
    /// // assigning `*discriminant.base` without an `if let`.
    /// if let Expr::Tuple(base) = *discriminant.base {
    /// # }
    /// # }
    /// ```
    ///
    /// A sign that you may not be choosing the right variable names is if you
    /// see names getting repeated in your code, like accessing
    /// `receiver.receiver` or `pat.pat` or `cond.cond`.
    pub enum Expr {
        /// A box expression: `box f`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Box(ExprBox #full {
            pub attrs: Vec<Attribute>,
            pub box_token: Token![box],
            pub expr: Box<Expr>,
        }),

        /// A placement expression: `place <- value`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub InPlace(ExprInPlace #full {
            pub attrs: Vec<Attribute>,
            pub place: Box<Expr>,
            pub arrow_token: Token![<-],
            pub value: Box<Expr>,
        }),

        /// A slice literal expression: `[a, b, c, d]`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Array(ExprArray #full {
            pub attrs: Vec<Attribute>,
            pub bracket_token: token::Bracket,
            pub elems: Punctuated<Expr, Token![,]>,
        }),

        /// A function call expression: `invoke(a, b)`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Call(ExprCall {
            pub attrs: Vec<Attribute>,
            pub func: Box<Expr>,
            pub paren_token: token::Paren,
            pub args: Punctuated<Expr, Token![,]>,
        }),

        /// A method call expression: `x.foo::<T>(a, b)`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub MethodCall(ExprMethodCall #full {
            pub attrs: Vec<Attribute>,
            pub receiver: Box<Expr>,
            pub dot_token: Token![.],
            pub method: Ident,
            pub turbofish: Option<MethodTurbofish>,
            pub paren_token: token::Paren,
            pub args: Punctuated<Expr, Token![,]>,
        }),

        /// A tuple expression: `(a, b, c, d)`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Tuple(ExprTuple #full {
            pub attrs: Vec<Attribute>,
            pub paren_token: token::Paren,
            pub elems: Punctuated<Expr, Token![,]>,
        }),

        /// A binary operation: `a + b`, `a * b`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Binary(ExprBinary {
            pub attrs: Vec<Attribute>,
            pub left: Box<Expr>,
            pub op: BinOp,
            pub right: Box<Expr>,
        }),

        /// A unary operation: `!x`, `*x`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Unary(ExprUnary {
            pub attrs: Vec<Attribute>,
            pub op: UnOp,
            pub expr: Box<Expr>,
        }),

        /// A literal in place of an expression: `1`, `"foo"`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Lit(ExprLit {
            pub attrs: Vec<Attribute>,
            pub lit: Lit,
        }),

        /// A cast expression: `foo as f64`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Cast(ExprCast {
            pub attrs: Vec<Attribute>,
            pub expr: Box<Expr>,
            pub as_token: Token![as],
            pub ty: Box<Type>,
        }),

        /// A type ascription expression: `foo: f64`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Type(ExprType #full {
            pub attrs: Vec<Attribute>,
            pub expr: Box<Expr>,
            pub colon_token: Token![:],
            pub ty: Box<Type>,
        }),

        /// A `let` guard: `let Some(x) = opt`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Let(ExprLet #full {
            pub attrs: Vec<Attribute>,
            pub let_token: Token![let],
            pub pats: Punctuated<Pat, Token![|]>,
            pub eq_token: Token![=],
            pub expr: Box<Expr>,
        }),

        /// An `if` expression with an optional `else` block: `if expr { ... }
        /// else { ... }`.
        ///
        /// The `else` branch expression may only be an `If` or `Block`
        /// expression, not any of the other types of expression.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub If(ExprIf #full {
            pub attrs: Vec<Attribute>,
            pub if_token: Token![if],
            pub cond: Box<Expr>,
            pub then_branch: Block,
            pub else_branch: Option<(Token![else], Box<Expr>)>,
        }),

        /// A while loop: `while expr { ... }`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub While(ExprWhile #full {
            pub attrs: Vec<Attribute>,
            pub label: Option<Label>,
            pub while_token: Token![while],
            pub cond: Box<Expr>,
            pub body: Block,
        }),

        /// A for loop: `for pat in expr { ... }`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub ForLoop(ExprForLoop #full {
            pub attrs: Vec<Attribute>,
            pub label: Option<Label>,
            pub for_token: Token![for],
            pub pat: Box<Pat>,
            pub in_token: Token![in],
            pub expr: Box<Expr>,
            pub body: Block,
        }),

        /// Conditionless loop: `loop { ... }`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Loop(ExprLoop #full {
            pub attrs: Vec<Attribute>,
            pub label: Option<Label>,
            pub loop_token: Token![loop],
            pub body: Block,
        }),

        /// A `match` expression: `match n { Some(n) => {}, None => {} }`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Match(ExprMatch #full {
            pub attrs: Vec<Attribute>,
            pub match_token: Token![match],
            pub expr: Box<Expr>,
            pub brace_token: token::Brace,
            pub arms: Vec<Arm>,
        }),

        /// A closure expression: `|a, b| a + b`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Closure(ExprClosure #full {
            pub attrs: Vec<Attribute>,
            pub asyncness: Option<Token![async]>,
            pub movability: Option<Token![static]>,
            pub capture: Option<Token![move]>,
            pub or1_token: Token![|],
            pub inputs: Punctuated<FnArg, Token![,]>,
            pub or2_token: Token![|],
            pub output: ReturnType,
            pub body: Box<Expr>,
        }),

        /// An unsafe block: `unsafe { ... }`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Unsafe(ExprUnsafe #full {
            pub attrs: Vec<Attribute>,
            pub unsafe_token: Token![unsafe],
            pub block: Block,
        }),

        /// A blocked scope: `{ ... }`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Block(ExprBlock #full {
            pub attrs: Vec<Attribute>,
            pub label: Option<Label>,
            pub block: Block,
        }),

        /// An assignment expression: `a = compute()`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Assign(ExprAssign #full {
            pub attrs: Vec<Attribute>,
            pub left: Box<Expr>,
            pub eq_token: Token![=],
            pub right: Box<Expr>,
        }),

        /// A compound assignment expression: `counter += 1`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub AssignOp(ExprAssignOp #full {
            pub attrs: Vec<Attribute>,
            pub left: Box<Expr>,
            pub op: BinOp,
            pub right: Box<Expr>,
        }),

        /// Access of a named struct field (`obj.k`) or unnamed tuple struct
        /// field (`obj.0`).
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Field(ExprField {
            pub attrs: Vec<Attribute>,
            pub base: Box<Expr>,
            pub dot_token: Token![.],
            pub member: Member,
        }),

        /// A square bracketed indexing expression: `vector[2]`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Index(ExprIndex {
            pub attrs: Vec<Attribute>,
            pub expr: Box<Expr>,
            pub bracket_token: token::Bracket,
            pub index: Box<Expr>,
        }),

        /// A range expression: `1..2`, `1..`, `..2`, `1..=2`, `..=2`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Range(ExprRange #full {
            pub attrs: Vec<Attribute>,
            pub from: Option<Box<Expr>>,
            pub limits: RangeLimits,
            pub to: Option<Box<Expr>>,
        }),

        /// A path like `std::mem::replace` possibly containing generic
        /// parameters and a qualified self-type.
        ///
        /// A plain identifier like `x` is a path of length 1.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Path(ExprPath {
            pub attrs: Vec<Attribute>,
            pub qself: Option<QSelf>,
            pub path: Path,
        }),

        /// A referencing operation: `&a` or `&mut a`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Reference(ExprReference #full {
            pub attrs: Vec<Attribute>,
            pub and_token: Token![&],
            pub mutability: Option<Token![mut]>,
            pub expr: Box<Expr>,
        }),

        /// A `break`, with an optional label to break and an optional
        /// expression.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Break(ExprBreak #full {
            pub attrs: Vec<Attribute>,
            pub break_token: Token![break],
            pub label: Option<Lifetime>,
            pub expr: Option<Box<Expr>>,
        }),

        /// A `continue`, with an optional label.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Continue(ExprContinue #full {
            pub attrs: Vec<Attribute>,
            pub continue_token: Token![continue],
            pub label: Option<Lifetime>,
        }),

        /// A `return`, with an optional value to be returned.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Return(ExprReturn #full {
            pub attrs: Vec<Attribute>,
            pub return_token: Token![return],
            pub expr: Option<Box<Expr>>,
        }),

        /// A macro invocation expression: `format!("{}", q)`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Macro(ExprMacro #full {
            pub attrs: Vec<Attribute>,
            pub mac: Macro,
        }),

        /// A struct literal expression: `Point { x: 1, y: 1 }`.
        ///
        /// The `rest` provides the value of the remaining fields as in `S { a:
        /// 1, b: 1, ..rest }`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Struct(ExprStruct #full {
            pub attrs: Vec<Attribute>,
            pub path: Path,
            pub brace_token: token::Brace,
            pub fields: Punctuated<FieldValue, Token![,]>,
            pub dot2_token: Option<Token![..]>,
            pub rest: Option<Box<Expr>>,
        }),

        /// An array literal constructed from one repeated element: `[0u8; N]`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Repeat(ExprRepeat #full {
            pub attrs: Vec<Attribute>,
            pub bracket_token: token::Bracket,
            pub expr: Box<Expr>,
            pub semi_token: Token![;],
            pub len: Box<Expr>,
        }),

        /// A parenthesized expression: `(a + b)`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Paren(ExprParen {
            pub attrs: Vec<Attribute>,
            pub paren_token: token::Paren,
            pub expr: Box<Expr>,
        }),

        /// An expression contained within invisible delimiters.
        ///
        /// This variant is important for faithfully representing the precedence
        /// of expressions and is related to `None`-delimited spans in a
        /// `TokenStream`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Group(ExprGroup #full {
            pub attrs: Vec<Attribute>,
            pub group_token: token::Group,
            pub expr: Box<Expr>,
        }),

        /// A try-expression: `expr?`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Try(ExprTry #full {
            pub attrs: Vec<Attribute>,
            pub expr: Box<Expr>,
            pub question_token: Token![?],
        }),

        /// An async block: `async { ... }`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Async(ExprAsync #full {
            pub attrs: Vec<Attribute>,
            pub async_token: Token![async],
            pub capture: Option<Token![move]>,
            pub block: Block,
        }),

        /// A try block: `try { ... }`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub TryBlock(ExprTryBlock #full {
            pub attrs: Vec<Attribute>,
            pub try_token: Token![try],
            pub block: Block,
        }),

        /// A yield expression: `yield expr`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Yield(ExprYield #full {
            pub attrs: Vec<Attribute>,
            pub yield_token: Token![yield],
            pub expr: Option<Box<Expr>>,
        }),

        /// Tokens in expression position not interpreted by Syn.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Verbatim(ExprVerbatim #manual_extra_traits {
            pub tts: TokenStream,
        }),
    }
}

#[cfg(feature = "extra-traits")]
impl Eq for ExprVerbatim {}

#[cfg(feature = "extra-traits")]
impl PartialEq for ExprVerbatim {
    fn eq(&self, other: &Self) -> bool {
        TokenStreamHelper(&self.tts) == TokenStreamHelper(&other.tts)
    }
}

#[cfg(feature = "extra-traits")]
impl Hash for ExprVerbatim {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        TokenStreamHelper(&self.tts).hash(state);
    }
}

impl Expr {
    #[cfg(all(feature = "parsing", feature = "full"))]
    fn replace_attrs(&mut self, new: Vec<Attribute>) -> Vec<Attribute> {
        match *self {
            Expr::Box(ExprBox { ref mut attrs, .. })
            | Expr::InPlace(ExprInPlace { ref mut attrs, .. })
            | Expr::Array(ExprArray { ref mut attrs, .. })
            | Expr::Call(ExprCall { ref mut attrs, .. })
            | Expr::MethodCall(ExprMethodCall { ref mut attrs, .. })
            | Expr::Tuple(ExprTuple { ref mut attrs, .. })
            | Expr::Binary(ExprBinary { ref mut attrs, .. })
            | Expr::Unary(ExprUnary { ref mut attrs, .. })
            | Expr::Lit(ExprLit { ref mut attrs, .. })
            | Expr::Cast(ExprCast { ref mut attrs, .. })
            | Expr::Type(ExprType { ref mut attrs, .. })
            | Expr::Let(ExprLet { ref mut attrs, .. })
            | Expr::If(ExprIf { ref mut attrs, .. })
            | Expr::While(ExprWhile { ref mut attrs, .. })
            | Expr::ForLoop(ExprForLoop { ref mut attrs, .. })
            | Expr::Loop(ExprLoop { ref mut attrs, .. })
            | Expr::Match(ExprMatch { ref mut attrs, .. })
            | Expr::Closure(ExprClosure { ref mut attrs, .. })
            | Expr::Unsafe(ExprUnsafe { ref mut attrs, .. })
            | Expr::Block(ExprBlock { ref mut attrs, .. })
            | Expr::Assign(ExprAssign { ref mut attrs, .. })
            | Expr::AssignOp(ExprAssignOp { ref mut attrs, .. })
            | Expr::Field(ExprField { ref mut attrs, .. })
            | Expr::Index(ExprIndex { ref mut attrs, .. })
            | Expr::Range(ExprRange { ref mut attrs, .. })
            | Expr::Path(ExprPath { ref mut attrs, .. })
            | Expr::Reference(ExprReference { ref mut attrs, .. })
            | Expr::Break(ExprBreak { ref mut attrs, .. })
            | Expr::Continue(ExprContinue { ref mut attrs, .. })
            | Expr::Return(ExprReturn { ref mut attrs, .. })
            | Expr::Macro(ExprMacro { ref mut attrs, .. })
            | Expr::Struct(ExprStruct { ref mut attrs, .. })
            | Expr::Repeat(ExprRepeat { ref mut attrs, .. })
            | Expr::Paren(ExprParen { ref mut attrs, .. })
            | Expr::Group(ExprGroup { ref mut attrs, .. })
            | Expr::Try(ExprTry { ref mut attrs, .. })
            | Expr::Async(ExprAsync { ref mut attrs, .. })
            | Expr::TryBlock(ExprTryBlock { ref mut attrs, .. })
            | Expr::Yield(ExprYield { ref mut attrs, .. }) => mem::replace(attrs, new),
            Expr::Verbatim(_) => Vec::new(),
        }
    }
}

ast_enum! {
    /// A struct or tuple struct field accessed in a struct literal or field
    /// expression.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub enum Member {
        /// A named field like `self.x`.
        Named(Ident),
        /// An unnamed field like `self.0`.
        Unnamed(Index),
    }
}

ast_struct! {
    /// The index of an unnamed tuple struct field.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub struct Index #manual_extra_traits {
        pub index: u32,
        pub span: Span,
    }
}

impl From<usize> for Index {
    fn from(index: usize) -> Index {
        assert!(index < u32::max_value() as usize);
        Index {
            index: index as u32,
            span: Span::call_site(),
        }
    }
}

#[cfg(feature = "extra-traits")]
impl Eq for Index {}

#[cfg(feature = "extra-traits")]
impl PartialEq for Index {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

#[cfg(feature = "extra-traits")]
impl Hash for Index {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

#[cfg(feature = "full")]
ast_struct! {
    /// The `::<>` explicit type parameters passed to a method call:
    /// `parse::<u64>()`.
    ///
    /// *This type is available if Syn is built with the `"full"` feature.*
    pub struct MethodTurbofish {
        pub colon2_token: Token![::],
        pub lt_token: Token![<],
        pub args: Punctuated<GenericMethodArgument, Token![,]>,
        pub gt_token: Token![>],
    }
}

#[cfg(feature = "full")]
ast_enum! {
    /// An individual generic argument to a method, like `T`.
    ///
    /// *This type is available if Syn is built with the `"full"` feature.*
    pub enum GenericMethodArgument {
        /// A type argument.
        Type(Type),
        /// A const expression. Must be inside of a block.
        ///
        /// NOTE: Identity expressions are represented as Type arguments, as
        /// they are indistinguishable syntactically.
        Const(Expr),
    }
}

#[cfg(feature = "full")]
ast_struct! {
    /// A field-value pair in a struct literal.
    ///
    /// *This type is available if Syn is built with the `"full"` feature.*
    pub struct FieldValue {
        /// Attributes tagged on the field.
        pub attrs: Vec<Attribute>,

        /// Name or index of the field.
        pub member: Member,

        /// The colon in `Struct { x: x }`. If written in shorthand like
        /// `Struct { x }`, there is no colon.
        pub colon_token: Option<Token![:]>,

        /// Value of the field.
        pub expr: Expr,
    }
}

#[cfg(feature = "full")]
ast_struct! {
    /// A lifetime labeling a `for`, `while`, or `loop`.
    ///
    /// *This type is available if Syn is built with the `"full"` feature.*
    pub struct Label {
        pub name: Lifetime,
        pub colon_token: Token![:],
    }
}

#[cfg(feature = "full")]
ast_struct! {
    /// A braced block containing Rust statements.
    ///
    /// *This type is available if Syn is built with the `"full"` feature.*
    pub struct Block {
        pub brace_token: token::Brace,
        /// Statements in a block
        pub stmts: Vec<Stmt>,
    }
}

#[cfg(feature = "full")]
ast_enum! {
    /// A statement, usually ending in a semicolon.
    ///
    /// *This type is available if Syn is built with the `"full"` feature.*
    pub enum Stmt {
        /// A local (let) binding.
        Local(Local),

        /// An item definition.
        Item(Item),

        /// Expr without trailing semicolon.
        Expr(Expr),

        /// Expression with trailing semicolon.
        Semi(Expr, Token![;]),
    }
}

#[cfg(feature = "full")]
ast_struct! {
    /// A local `let` binding: `let x: u64 = s.parse()?`.
    ///
    /// *This type is available if Syn is built with the `"full"` feature.*
    pub struct Local {
        pub attrs: Vec<Attribute>,
        pub let_token: Token![let],
        pub pats: Punctuated<Pat, Token![|]>,
        pub ty: Option<(Token![:], Box<Type>)>,
        pub init: Option<(Token![=], Box<Expr>)>,
        pub semi_token: Token![;],
    }
}

#[cfg(feature = "full")]
ast_enum_of_structs! {
    /// A pattern in a local binding, function signature, match expression, or
    /// various other places.
    ///
    /// *This type is available if Syn is built with the `"full"` feature.*
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: enum.Expr.html#syntax-tree-enums
    //
    // TODO: change syntax-tree-enum link to an intra rustdoc link, currently
    // blocked on https://github.com/rust-lang/rust/issues/62833
    pub enum Pat {
        /// A pattern that matches any value: `_`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Wild(PatWild {
            pub underscore_token: Token![_],
        }),

        /// A pattern that binds a new variable: `ref mut binding @ SUBPATTERN`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Ident(PatIdent {
            pub by_ref: Option<Token![ref]>,
            pub mutability: Option<Token![mut]>,
            pub ident: Ident,
            pub subpat: Option<(Token![@], Box<Pat>)>,
        }),

        /// A struct or struct variant pattern: `Variant { x, y, .. }`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Struct(PatStruct {
            pub path: Path,
            pub brace_token: token::Brace,
            pub fields: Punctuated<FieldPat, Token![,]>,
            pub dot2_token: Option<Token![..]>,
        }),

        /// A tuple struct or tuple variant pattern: `Variant(x, y, .., z)`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub TupleStruct(PatTupleStruct {
            pub path: Path,
            pub pat: PatTuple,
        }),

        /// A path pattern like `Color::Red`, optionally qualified with a
        /// self-type.
        ///
        /// Unqualified path patterns can legally refer to variants, structs,
        /// constants or associated constants. Qualified path patterns like
        /// `<A>::B::C` and `<A as Trait>::B::C` can only legally refer to
        /// associated constants.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Path(PatPath {
            pub qself: Option<QSelf>,
            pub path: Path,
        }),

        /// A tuple pattern: `(a, b)`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Tuple(PatTuple {
            pub paren_token: token::Paren,
            pub front: Punctuated<Pat, Token![,]>,
            pub dot2_token: Option<Token![..]>,
            pub comma_token: Option<Token![,]>,
            pub back: Punctuated<Pat, Token![,]>,
        }),

        /// A box pattern: `box v`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Box(PatBox {
            pub box_token: Token![box],
            pub pat: Box<Pat>,
        }),

        /// A reference pattern: `&mut (first, second)`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Ref(PatRef {
            pub and_token: Token![&],
            pub mutability: Option<Token![mut]>,
            pub pat: Box<Pat>,
        }),

        /// A literal pattern: `0`.
        ///
        /// This holds an `Expr` rather than a `Lit` because negative numbers
        /// are represented as an `Expr::Unary`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Lit(PatLit {
            pub expr: Box<Expr>,
        }),

        /// A range pattern: `1..=2`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Range(PatRange {
            pub lo: Box<Expr>,
            pub limits: RangeLimits,
            pub hi: Box<Expr>,
        }),

        /// A dynamically sized slice pattern: `[a, b, i.., y, z]`.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Slice(PatSlice {
            pub bracket_token: token::Bracket,
            pub front: Punctuated<Pat, Token![,]>,
            pub middle: Option<Box<Pat>>,
            pub dot2_token: Option<Token![..]>,
            pub comma_token: Option<Token![,]>,
            pub back: Punctuated<Pat, Token![,]>,
        }),

        /// A macro in expression position.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Macro(PatMacro {
            pub mac: Macro,
        }),

        /// Tokens in pattern position not interpreted by Syn.
        ///
        /// *This type is available if Syn is built with the `"full"` feature.*
        pub Verbatim(PatVerbatim #manual_extra_traits {
            pub tts: TokenStream,
        }),
    }
}

#[cfg(all(feature = "full", feature = "extra-traits"))]
impl Eq for PatVerbatim {}

#[cfg(all(feature = "full", feature = "extra-traits"))]
impl PartialEq for PatVerbatim {
    fn eq(&self, other: &Self) -> bool {
        TokenStreamHelper(&self.tts) == TokenStreamHelper(&other.tts)
    }
}

#[cfg(all(feature = "full", feature = "extra-traits"))]
impl Hash for PatVerbatim {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        TokenStreamHelper(&self.tts).hash(state);
    }
}

#[cfg(feature = "full")]
ast_struct! {
    /// One arm of a `match` expression: `0...10 => { return true; }`.
    ///
    /// As in:
    ///
    /// ```edition2018
    /// # fn f() -> bool {
    /// #     let n = 0;
    /// match n {
    ///     0...10 => {
    ///         return true;
    ///     }
    ///     // ...
    ///     # _ => {}
    /// }
    /// #   false
    /// # }
    /// ```
    ///
    /// *This type is available if Syn is built with the `"full"` feature.*
    pub struct Arm {
        pub attrs: Vec<Attribute>,
        pub leading_vert: Option<Token![|]>,
        pub pats: Punctuated<Pat, Token![|]>,
        pub guard: Option<(Token![if], Box<Expr>)>,
        pub fat_arrow_token: Token![=>],
        pub body: Box<Expr>,
        pub comma: Option<Token![,]>,
    }
}

#[cfg(feature = "full")]
ast_enum! {
    /// Limit types of a range, inclusive or exclusive.
    ///
    /// *This type is available if Syn is built with the `"full"` feature.*
    #[cfg_attr(feature = "clone-impls", derive(Copy))]
    pub enum RangeLimits {
        /// Inclusive at the beginning, exclusive at the end.
        HalfOpen(Token![..]),
        /// Inclusive at the beginning and end.
        Closed(Token![..=]),
    }
}

#[cfg(feature = "full")]
ast_struct! {
    /// A single field in a struct pattern.
    ///
    /// Patterns like the fields of Foo `{ x, ref y, ref mut z }` are treated
    /// the same as `x: x, y: ref y, z: ref mut z` but there is no colon token.
    ///
    /// *This type is available if Syn is built with the `"full"` feature.*
    pub struct FieldPat {
        pub attrs: Vec<Attribute>,
        pub member: Member,
        pub colon_token: Option<Token![:]>,
        pub pat: Box<Pat>,
    }
}

#[cfg(any(feature = "parsing", feature = "printing"))]
#[cfg(feature = "full")]
fn requires_terminator(expr: &Expr) -> bool {
    // see https://github.com/rust-lang/rust/blob/eb8f2586e/src/libsyntax/parse/classify.rs#L17-L37
    match *expr {
        Expr::Unsafe(..)
        | Expr::Block(..)
        | Expr::If(..)
        | Expr::Match(..)
        | Expr::While(..)
        | Expr::Loop(..)
        | Expr::ForLoop(..)
        | Expr::Async(..)
        | Expr::TryBlock(..) => false,
        _ => true,
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;

    #[cfg(feature = "full")]
    use ext::IdentExt;
    use parse::{Parse, ParseStream, Result};
    use path;

    // When we're parsing expressions which occur before blocks, like in an if
    // statement's condition, we cannot parse a struct literal.
    //
    // Struct literals are ambiguous in certain positions
    // https://github.com/rust-lang/rfcs/pull/92
    #[derive(Copy, Clone)]
    pub struct AllowStruct(bool);

    #[derive(Copy, Clone, PartialEq, PartialOrd)]
    enum Precedence {
        Any,
        Assign,
        Range,
        Or,
        And,
        Compare,
        BitOr,
        BitXor,
        BitAnd,
        Shift,
        Arithmetic,
        Term,
        Cast,
    }

    impl Precedence {
        fn of(op: &BinOp) -> Self {
            match *op {
                BinOp::Add(_) | BinOp::Sub(_) => Precedence::Arithmetic,
                BinOp::Mul(_) | BinOp::Div(_) | BinOp::Rem(_) => Precedence::Term,
                BinOp::And(_) => Precedence::And,
                BinOp::Or(_) => Precedence::Or,
                BinOp::BitXor(_) => Precedence::BitXor,
                BinOp::BitAnd(_) => Precedence::BitAnd,
                BinOp::BitOr(_) => Precedence::BitOr,
                BinOp::Shl(_) | BinOp::Shr(_) => Precedence::Shift,
                BinOp::Eq(_)
                | BinOp::Lt(_)
                | BinOp::Le(_)
                | BinOp::Ne(_)
                | BinOp::Ge(_)
                | BinOp::Gt(_) => Precedence::Compare,
                BinOp::AddEq(_)
                | BinOp::SubEq(_)
                | BinOp::MulEq(_)
                | BinOp::DivEq(_)
                | BinOp::RemEq(_)
                | BinOp::BitXorEq(_)
                | BinOp::BitAndEq(_)
                | BinOp::BitOrEq(_)
                | BinOp::ShlEq(_)
                | BinOp::ShrEq(_) => Precedence::Assign,
            }
        }
    }

    impl Parse for Expr {
        fn parse(input: ParseStream) -> Result<Self> {
            ambiguous_expr(input, AllowStruct(true))
        }
    }

    #[cfg(feature = "full")]
    fn expr_no_struct(input: ParseStream) -> Result<Expr> {
        ambiguous_expr(input, AllowStruct(false))
    }

    #[cfg(feature = "full")]
    fn parse_expr(
        input: ParseStream,
        mut lhs: Expr,
        allow_struct: AllowStruct,
        base: Precedence,
    ) -> Result<Expr> {
        loop {
            if input
                .fork()
                .parse::<BinOp>()
                .ok()
                .map_or(false, |op| Precedence::of(&op) >= base)
            {
                let op: BinOp = input.parse()?;
                let precedence = Precedence::of(&op);
                let mut rhs = unary_expr(input, allow_struct)?;
                loop {
                    let next = peek_precedence(input);
                    if next > precedence || next == precedence && precedence == Precedence::Assign {
                        rhs = parse_expr(input, rhs, allow_struct, next)?;
                    } else {
                        break;
                    }
                }
                lhs = if precedence == Precedence::Assign {
                    Expr::AssignOp(ExprAssignOp {
                        attrs: Vec::new(),
                        left: Box::new(lhs),
                        op: op,
                        right: Box::new(rhs),
                    })
                } else {
                    Expr::Binary(ExprBinary {
                        attrs: Vec::new(),
                        left: Box::new(lhs),
                        op: op,
                        right: Box::new(rhs),
                    })
                };
            } else if Precedence::Assign >= base
                && input.peek(Token![=])
                && !input.peek(Token![==])
                && !input.peek(Token![=>])
            {
                let eq_token: Token![=] = input.parse()?;
                let mut rhs = unary_expr(input, allow_struct)?;
                loop {
                    let next = peek_precedence(input);
                    if next >= Precedence::Assign {
                        rhs = parse_expr(input, rhs, allow_struct, next)?;
                    } else {
                        break;
                    }
                }
                lhs = Expr::Assign(ExprAssign {
                    attrs: Vec::new(),
                    left: Box::new(lhs),
                    eq_token: eq_token,
                    right: Box::new(rhs),
                });
            } else if Precedence::Range >= base && input.peek(Token![..]) {
                let limits: RangeLimits = input.parse()?;
                let rhs = if input.is_empty()
                    || input.peek(Token![,])
                    || input.peek(Token![;])
                    || !allow_struct.0 && input.peek(token::Brace)
                {
                    None
                } else {
                    let mut rhs = unary_expr(input, allow_struct)?;
                    loop {
                        let next = peek_precedence(input);
                        if next > Precedence::Range {
                            rhs = parse_expr(input, rhs, allow_struct, next)?;
                        } else {
                            break;
                        }
                    }
                    Some(rhs)
                };
                lhs = Expr::Range(ExprRange {
                    attrs: Vec::new(),
                    from: Some(Box::new(lhs)),
                    limits: limits,
                    to: rhs.map(Box::new),
                });
            } else if Precedence::Cast >= base && input.peek(Token![as]) {
                let as_token: Token![as] = input.parse()?;
                let ty = input.call(Type::without_plus)?;
                lhs = Expr::Cast(ExprCast {
                    attrs: Vec::new(),
                    expr: Box::new(lhs),
                    as_token: as_token,
                    ty: Box::new(ty),
                });
            } else if Precedence::Cast >= base && input.peek(Token![:]) && !input.peek(Token![::]) {
                let colon_token: Token![:] = input.parse()?;
                let ty = input.call(Type::without_plus)?;
                lhs = Expr::Type(ExprType {
                    attrs: Vec::new(),
                    expr: Box::new(lhs),
                    colon_token: colon_token,
                    ty: Box::new(ty),
                });
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    #[cfg(not(feature = "full"))]
    fn parse_expr(
        input: ParseStream,
        mut lhs: Expr,
        allow_struct: AllowStruct,
        base: Precedence,
    ) -> Result<Expr> {
        loop {
            if input
                .fork()
                .parse::<BinOp>()
                .ok()
                .map_or(false, |op| Precedence::of(&op) >= base)
            {
                let op: BinOp = input.parse()?;
                let precedence = Precedence::of(&op);
                let mut rhs = unary_expr(input, allow_struct)?;
                loop {
                    let next = peek_precedence(input);
                    if next > precedence || next == precedence && precedence == Precedence::Assign {
                        rhs = parse_expr(input, rhs, allow_struct, next)?;
                    } else {
                        break;
                    }
                }
                lhs = Expr::Binary(ExprBinary {
                    attrs: Vec::new(),
                    left: Box::new(lhs),
                    op: op,
                    right: Box::new(rhs),
                });
            } else if Precedence::Cast >= base && input.peek(Token![as]) {
                let as_token: Token![as] = input.parse()?;
                let ty = input.call(Type::without_plus)?;
                lhs = Expr::Cast(ExprCast {
                    attrs: Vec::new(),
                    expr: Box::new(lhs),
                    as_token: as_token,
                    ty: Box::new(ty),
                });
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn peek_precedence(input: ParseStream) -> Precedence {
        if let Ok(op) = input.fork().parse() {
            Precedence::of(&op)
        } else if input.peek(Token![=]) && !input.peek(Token![=>]) {
            Precedence::Assign
        } else if input.peek(Token![..]) {
            Precedence::Range
        } else if input.peek(Token![as]) || input.peek(Token![:]) && !input.peek(Token![::]) {
            Precedence::Cast
        } else {
            Precedence::Any
        }
    }

    // Parse an arbitrary expression.
    fn ambiguous_expr(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        let lhs = unary_expr(input, allow_struct)?;
        parse_expr(input, lhs, allow_struct, Precedence::Any)
    }

    // <UnOp> <trailer>
    // & <trailer>
    // &mut <trailer>
    // box <trailer>
    #[cfg(feature = "full")]
    fn unary_expr(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        // TODO: optimize using advance_to
        let ahead = input.fork();
        ahead.call(Attribute::parse_outer)?;
        if ahead.peek(Token![&])
            || ahead.peek(Token![box])
            || ahead.peek(Token![*])
            || ahead.peek(Token![!])
            || ahead.peek(Token![-])
        {
            let attrs = input.call(Attribute::parse_outer)?;
            if input.peek(Token![&]) {
                Ok(Expr::Reference(ExprReference {
                    attrs: attrs,
                    and_token: input.parse()?,
                    mutability: input.parse()?,
                    expr: Box::new(unary_expr(input, allow_struct)?),
                }))
            } else if input.peek(Token![box]) {
                Ok(Expr::Box(ExprBox {
                    attrs: attrs,
                    box_token: input.parse()?,
                    expr: Box::new(unary_expr(input, allow_struct)?),
                }))
            } else {
                Ok(Expr::Unary(ExprUnary {
                    attrs: attrs,
                    op: input.parse()?,
                    expr: Box::new(unary_expr(input, allow_struct)?),
                }))
            }
        } else {
            trailer_expr(input, allow_struct)
        }
    }

    #[cfg(not(feature = "full"))]
    fn unary_expr(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        // TODO: optimize using advance_to
        let ahead = input.fork();
        ahead.call(Attribute::parse_outer)?;
        if ahead.peek(Token![*]) || ahead.peek(Token![!]) || ahead.peek(Token![-]) {
            Ok(Expr::Unary(ExprUnary {
                attrs: input.call(Attribute::parse_outer)?,
                op: input.parse()?,
                expr: Box::new(unary_expr(input, allow_struct)?),
            }))
        } else {
            trailer_expr(input, allow_struct)
        }
    }

    // <atom> (..<args>) ...
    // <atom> . <ident> (..<args>) ...
    // <atom> . <ident> ...
    // <atom> . <lit> ...
    // <atom> [ <expr> ] ...
    // <atom> ? ...
    #[cfg(feature = "full")]
    fn trailer_expr(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        if input.peek(token::Group) {
            return input.call(expr_group).map(Expr::Group);
        }

        let outer_attrs = input.call(Attribute::parse_outer)?;

        let atom = atom_expr(input, allow_struct)?;
        let mut e = trailer_helper(input, atom)?;

        let inner_attrs = e.replace_attrs(Vec::new());
        let attrs = private::attrs(outer_attrs, inner_attrs);
        e.replace_attrs(attrs);
        Ok(e)
    }

    #[cfg(feature = "full")]
    fn trailer_helper(input: ParseStream, mut e: Expr) -> Result<Expr> {
        loop {
            if input.peek(token::Paren) {
                let content;
                e = Expr::Call(ExprCall {
                    attrs: Vec::new(),
                    func: Box::new(e),
                    paren_token: parenthesized!(content in input),
                    args: content.parse_terminated(Expr::parse)?,
                });
            } else if input.peek(Token![.]) && !input.peek(Token![..]) {
                let dot_token: Token![.] = input.parse()?;
                let member: Member = input.parse()?;
                let turbofish = if member.is_named() && input.peek(Token![::]) {
                    Some(MethodTurbofish {
                        colon2_token: input.parse()?,
                        lt_token: input.parse()?,
                        args: {
                            let mut args = Punctuated::new();
                            loop {
                                if input.peek(Token![>]) {
                                    break;
                                }
                                let value = input.call(generic_method_argument)?;
                                args.push_value(value);
                                if input.peek(Token![>]) {
                                    break;
                                }
                                let punct = input.parse()?;
                                args.push_punct(punct);
                            }
                            args
                        },
                        gt_token: input.parse()?,
                    })
                } else {
                    None
                };

                if turbofish.is_some() || input.peek(token::Paren) {
                    if let Member::Named(method) = member {
                        let content;
                        e = Expr::MethodCall(ExprMethodCall {
                            attrs: Vec::new(),
                            receiver: Box::new(e),
                            dot_token: dot_token,
                            method: method,
                            turbofish: turbofish,
                            paren_token: parenthesized!(content in input),
                            args: content.parse_terminated(Expr::parse)?,
                        });
                        continue;
                    }
                }

                e = Expr::Field(ExprField {
                    attrs: Vec::new(),
                    base: Box::new(e),
                    dot_token: dot_token,
                    member: member,
                });
            } else if input.peek(token::Bracket) {
                let content;
                e = Expr::Index(ExprIndex {
                    attrs: Vec::new(),
                    expr: Box::new(e),
                    bracket_token: bracketed!(content in input),
                    index: content.parse()?,
                });
            } else if input.peek(Token![?]) {
                e = Expr::Try(ExprTry {
                    attrs: Vec::new(),
                    expr: Box::new(e),
                    question_token: input.parse()?,
                });
            } else {
                break;
            }
        }
        Ok(e)
    }

    #[cfg(not(feature = "full"))]
    fn trailer_expr(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        let mut e = atom_expr(input, allow_struct)?;

        loop {
            if input.peek(token::Paren) {
                let content;
                e = Expr::Call(ExprCall {
                    attrs: Vec::new(),
                    func: Box::new(e),
                    paren_token: parenthesized!(content in input),
                    args: content.parse_terminated(Expr::parse)?,
                });
            } else if input.peek(Token![.]) {
                e = Expr::Field(ExprField {
                    attrs: Vec::new(),
                    base: Box::new(e),
                    dot_token: input.parse()?,
                    member: input.parse()?,
                });
            } else if input.peek(token::Bracket) {
                let content;
                e = Expr::Index(ExprIndex {
                    attrs: Vec::new(),
                    expr: Box::new(e),
                    bracket_token: bracketed!(content in input),
                    index: content.parse()?,
                });
            } else {
                break;
            }
        }

        Ok(e)
    }

    // Parse all atomic expressions which don't have to worry about precedence
    // interactions, as they are fully contained.
    #[cfg(feature = "full")]
    fn atom_expr(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        if input.peek(token::Group) {
            input.call(expr_group).map(Expr::Group)
        } else if input.peek(Lit) {
            input.parse().map(Expr::Lit)
        } else if input.peek(Token![async])
            && (input.peek2(token::Brace) || input.peek2(Token![move]) && input.peek3(token::Brace))
        {
            input.call(expr_async).map(Expr::Async)
        } else if input.peek(Token![try]) && input.peek2(token::Brace) {
            input.call(expr_try_block).map(Expr::TryBlock)
        } else if input.peek(Token![|])
            || input.peek(Token![async]) && (input.peek2(Token![|]) || input.peek2(Token![move]))
            || input.peek(Token![static])
            || input.peek(Token![move])
        {
            expr_closure(input, allow_struct).map(Expr::Closure)
        } else if input.peek(Ident)
            || input.peek(Token![::])
            || input.peek(Token![<])
            || input.peek(Token![self])
            || input.peek(Token![Self])
            || input.peek(Token![super])
            || input.peek(Token![extern])
            || input.peek(Token![crate])
        {
            path_or_macro_or_struct(input, allow_struct)
        } else if input.peek(token::Paren) {
            paren_or_tuple(input)
        } else if input.peek(Token![break]) {
            expr_break(input, allow_struct).map(Expr::Break)
        } else if input.peek(Token![continue]) {
            input.call(expr_continue).map(Expr::Continue)
        } else if input.peek(Token![return]) {
            expr_ret(input, allow_struct).map(Expr::Return)
        } else if input.peek(token::Bracket) {
            array_or_repeat(input)
        } else if input.peek(Token![let]) {
            input.call(expr_let).map(Expr::Let)
        } else if input.peek(Token![if]) {
            input.parse().map(Expr::If)
        } else if input.peek(Token![while]) {
            input.parse().map(Expr::While)
        } else if input.peek(Token![for]) {
            input.parse().map(Expr::ForLoop)
        } else if input.peek(Token![loop]) {
            input.parse().map(Expr::Loop)
        } else if input.peek(Token![match]) {
            input.parse().map(Expr::Match)
        } else if input.peek(Token![yield]) {
            input.call(expr_yield).map(Expr::Yield)
        } else if input.peek(Token![unsafe]) {
            input.call(expr_unsafe).map(Expr::Unsafe)
        } else if input.peek(token::Brace) {
            input.call(expr_block).map(Expr::Block)
        } else if input.peek(Token![..]) {
            expr_range(input, allow_struct).map(Expr::Range)
        } else if input.peek(Lifetime) {
            let the_label: Label = input.parse()?;
            let mut expr = if input.peek(Token![while]) {
                Expr::While(input.parse()?)
            } else if input.peek(Token![for]) {
                Expr::ForLoop(input.parse()?)
            } else if input.peek(Token![loop]) {
                Expr::Loop(input.parse()?)
            } else if input.peek(token::Brace) {
                Expr::Block(input.call(expr_block)?)
            } else {
                return Err(input.error("expected loop or block expression"));
            };
            match expr {
                Expr::While(ExprWhile { ref mut label, .. })
                | Expr::ForLoop(ExprForLoop { ref mut label, .. })
                | Expr::Loop(ExprLoop { ref mut label, .. })
                | Expr::Block(ExprBlock { ref mut label, .. }) => *label = Some(the_label),
                _ => unreachable!(),
            }
            Ok(expr)
        } else {
            Err(input.error("expected expression"))
        }
    }

    #[cfg(not(feature = "full"))]
    fn atom_expr(input: ParseStream, _allow_struct: AllowStruct) -> Result<Expr> {
        if input.peek(Lit) {
            input.parse().map(Expr::Lit)
        } else if input.peek(token::Paren) {
            input.call(expr_paren).map(Expr::Paren)
        } else if input.peek(Ident)
            || input.peek(Token![::])
            || input.peek(Token![<])
            || input.peek(Token![self])
            || input.peek(Token![Self])
            || input.peek(Token![super])
            || input.peek(Token![extern])
            || input.peek(Token![crate])
        {
            input.parse().map(Expr::Path)
        } else {
            Err(input.error("unsupported expression; enable syn's features=[\"full\"]"))
        }
    }

    #[cfg(feature = "full")]
    fn path_or_macro_or_struct(input: ParseStream, allow_struct: AllowStruct) -> Result<Expr> {
        let expr: ExprPath = input.parse()?;
        if expr.qself.is_some() {
            return Ok(Expr::Path(expr));
        }

        if input.peek(Token![!]) && !input.peek(Token![!=]) {
            let mut contains_arguments = false;
            for segment in &expr.path.segments {
                match segment.arguments {
                    PathArguments::None => {}
                    PathArguments::AngleBracketed(_) | PathArguments::Parenthesized(_) => {
                        contains_arguments = true;
                    }
                }
            }

            if !contains_arguments {
                let bang_token: Token![!] = input.parse()?;
                let (delimiter, tts) = mac::parse_delimiter(input)?;
                return Ok(Expr::Macro(ExprMacro {
                    attrs: Vec::new(),
                    mac: Macro {
                        path: expr.path,
                        bang_token: bang_token,
                        delimiter: delimiter,
                        tts: tts,
                    },
                }));
            }
        }

        if allow_struct.0 && input.peek(token::Brace) {
            let outer_attrs = Vec::new();
            expr_struct_helper(input, outer_attrs, expr.path).map(Expr::Struct)
        } else {
            Ok(Expr::Path(expr))
        }
    }

    #[cfg(feature = "full")]
    fn paren_or_tuple(input: ParseStream) -> Result<Expr> {
        let content;
        let paren_token = parenthesized!(content in input);
        let inner_attrs = content.call(Attribute::parse_inner)?;
        if content.is_empty() {
            return Ok(Expr::Tuple(ExprTuple {
                attrs: inner_attrs,
                paren_token: paren_token,
                elems: Punctuated::new(),
            }));
        }

        let first: Expr = content.parse()?;
        if content.is_empty() {
            return Ok(Expr::Paren(ExprParen {
                attrs: inner_attrs,
                paren_token: paren_token,
                expr: Box::new(first),
            }));
        }

        let mut elems = Punctuated::new();
        elems.push_value(first);
        while !content.is_empty() {
            let punct = content.parse()?;
            elems.push_punct(punct);
            if content.is_empty() {
                break;
            }
            let value = content.parse()?;
            elems.push_value(value);
        }
        Ok(Expr::Tuple(ExprTuple {
            attrs: inner_attrs,
            paren_token: paren_token,
            elems: elems,
        }))
    }

    #[cfg(feature = "full")]
    fn array_or_repeat(input: ParseStream) -> Result<Expr> {
        let content;
        let bracket_token = bracketed!(content in input);
        let inner_attrs = content.call(Attribute::parse_inner)?;
        if content.is_empty() {
            return Ok(Expr::Array(ExprArray {
                attrs: inner_attrs,
                bracket_token: bracket_token,
                elems: Punctuated::new(),
            }));
        }

        let first: Expr = content.parse()?;
        if content.is_empty() || content.peek(Token![,]) {
            let mut elems = Punctuated::new();
            elems.push_value(first);
            while !content.is_empty() {
                let punct = content.parse()?;
                elems.push_punct(punct);
                if content.is_empty() {
                    break;
                }
                let value = content.parse()?;
                elems.push_value(value);
            }
            Ok(Expr::Array(ExprArray {
                attrs: inner_attrs,
                bracket_token: bracket_token,
                elems: elems,
            }))
        } else if content.peek(Token![;]) {
            let semi_token: Token![;] = content.parse()?;
            let len: Expr = content.parse()?;
            Ok(Expr::Repeat(ExprRepeat {
                attrs: inner_attrs,
                bracket_token: bracket_token,
                expr: Box::new(first),
                semi_token: semi_token,
                len: Box::new(len),
            }))
        } else {
            Err(content.error("expected `,` or `;`"))
        }
    }

    #[cfg(feature = "full")]
    fn expr_early(input: ParseStream) -> Result<Expr> {
        let mut attrs = input.call(Attribute::parse_outer)?;
        let mut expr = if input.peek(Token![if]) {
            Expr::If(input.parse()?)
        } else if input.peek(Token![while]) {
            Expr::While(input.parse()?)
        } else if input.peek(Token![for]) {
            Expr::ForLoop(input.parse()?)
        } else if input.peek(Token![loop]) {
            Expr::Loop(input.parse()?)
        } else if input.peek(Token![match]) {
            Expr::Match(input.parse()?)
        } else if input.peek(Token![try]) && input.peek2(token::Brace) {
            Expr::TryBlock(input.call(expr_try_block)?)
        } else if input.peek(Token![unsafe]) {
            Expr::Unsafe(input.call(expr_unsafe)?)
        } else if input.peek(token::Brace) {
            Expr::Block(input.call(expr_block)?)
        } else {
            let allow_struct = AllowStruct(true);
            let mut expr = unary_expr(input, allow_struct)?;

            attrs.extend(expr.replace_attrs(Vec::new()));
            expr.replace_attrs(attrs);

            return parse_expr(input, expr, allow_struct, Precedence::Any);
        };

        if input.peek(Token![.]) || input.peek(Token![?]) {
            expr = trailer_helper(input, expr)?;

            attrs.extend(expr.replace_attrs(Vec::new()));
            expr.replace_attrs(attrs);

            let allow_struct = AllowStruct(true);
            return parse_expr(input, expr, allow_struct, Precedence::Any);
        }

        attrs.extend(expr.replace_attrs(Vec::new()));
        expr.replace_attrs(attrs);
        Ok(expr)
    }

    impl Parse for ExprLit {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ExprLit {
                attrs: Vec::new(),
                lit: input.parse()?,
            })
        }
    }

    #[cfg(feature = "full")]
    fn expr_group(input: ParseStream) -> Result<ExprGroup> {
        let group = private::parse_group(input)?;
        Ok(ExprGroup {
            attrs: Vec::new(),
            group_token: group.token,
            expr: group.content.parse()?,
        })
    }

    #[cfg(not(feature = "full"))]
    fn expr_paren(input: ParseStream) -> Result<ExprParen> {
        let content;
        Ok(ExprParen {
            attrs: Vec::new(),
            paren_token: parenthesized!(content in input),
            expr: content.parse()?,
        })
    }

    #[cfg(feature = "full")]
    fn generic_method_argument(input: ParseStream) -> Result<GenericMethodArgument> {
        // TODO parse const generics as well
        input.parse().map(GenericMethodArgument::Type)
    }

    #[cfg(feature = "full")]
    fn expr_let(input: ParseStream) -> Result<ExprLet> {
        Ok(ExprLet {
            attrs: Vec::new(),
            let_token: input.parse()?,
            pats: {
                let mut pats = Punctuated::new();
                input.parse::<Option<Token![|]>>()?;
                let value: Pat = input.parse()?;
                pats.push_value(value);
                while input.peek(Token![|]) && !input.peek(Token![||]) && !input.peek(Token![|=]) {
                    let punct = input.parse()?;
                    pats.push_punct(punct);
                    let value: Pat = input.parse()?;
                    pats.push_value(value);
                }
                pats
            },
            eq_token: input.parse()?,
            expr: Box::new(input.call(expr_no_struct)?),
        })
    }

    #[cfg(feature = "full")]
    impl Parse for ExprIf {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ExprIf {
                attrs: Vec::new(),
                if_token: input.parse()?,
                cond: Box::new(input.call(expr_no_struct)?),
                then_branch: input.parse()?,
                else_branch: {
                    if input.peek(Token![else]) {
                        Some(input.call(else_block)?)
                    } else {
                        None
                    }
                },
            })
        }
    }

    #[cfg(feature = "full")]
    fn else_block(input: ParseStream) -> Result<(Token![else], Box<Expr>)> {
        let else_token: Token![else] = input.parse()?;

        let lookahead = input.lookahead1();
        let else_branch = if input.peek(Token![if]) {
            input.parse().map(Expr::If)?
        } else if input.peek(token::Brace) {
            Expr::Block(ExprBlock {
                attrs: Vec::new(),
                label: None,
                block: input.parse()?,
            })
        } else {
            return Err(lookahead.error());
        };

        Ok((else_token, Box::new(else_branch)))
    }

    #[cfg(feature = "full")]
    impl Parse for ExprForLoop {
        fn parse(input: ParseStream) -> Result<Self> {
            let label: Option<Label> = input.parse()?;
            let for_token: Token![for] = input.parse()?;
            let pat: Pat = input.parse()?;
            let in_token: Token![in] = input.parse()?;
            let expr: Expr = input.call(expr_no_struct)?;

            let content;
            let brace_token = braced!(content in input);
            let inner_attrs = content.call(Attribute::parse_inner)?;
            let stmts = content.call(Block::parse_within)?;

            Ok(ExprForLoop {
                attrs: inner_attrs,
                label: label,
                for_token: for_token,
                pat: Box::new(pat),
                in_token: in_token,
                expr: Box::new(expr),
                body: Block {
                    brace_token: brace_token,
                    stmts: stmts,
                },
            })
        }
    }

    #[cfg(feature = "full")]
    impl Parse for ExprLoop {
        fn parse(input: ParseStream) -> Result<Self> {
            let label: Option<Label> = input.parse()?;
            let loop_token: Token![loop] = input.parse()?;

            let content;
            let brace_token = braced!(content in input);
            let inner_attrs = content.call(Attribute::parse_inner)?;
            let stmts = content.call(Block::parse_within)?;

            Ok(ExprLoop {
                attrs: inner_attrs,
                label: label,
                loop_token: loop_token,
                body: Block {
                    brace_token: brace_token,
                    stmts: stmts,
                },
            })
        }
    }

    #[cfg(feature = "full")]
    impl Parse for ExprMatch {
        fn parse(input: ParseStream) -> Result<Self> {
            let match_token: Token![match] = input.parse()?;
            let expr = expr_no_struct(input)?;

            let content;
            let brace_token = braced!(content in input);
            let inner_attrs = content.call(Attribute::parse_inner)?;

            let mut arms = Vec::new();
            while !content.is_empty() {
                arms.push(content.call(Arm::parse)?);
            }

            Ok(ExprMatch {
                attrs: inner_attrs,
                match_token: match_token,
                expr: Box::new(expr),
                brace_token: brace_token,
                arms: arms,
            })
        }
    }

    #[cfg(all(feature = "full", feature = "printing"))]
    impl Parse for ExprInPlace {
        fn parse(input: ParseStream) -> Result<Self> {
            let msg = "placement expression has been removed from Rust and is no longer parsed";
            Err(input.error(msg))
        }
    }

    macro_rules! impl_by_parsing_expr {
        (
            $(
                $expr_type:ty, $variant:ident, $msg:expr,
            )*
        ) => {
            $(
                #[cfg(all(feature = "full", feature = "printing"))]
                impl Parse for $expr_type {
                    fn parse(input: ParseStream) -> Result<Self> {
                        let mut expr: Expr = input.parse()?;
                        loop {
                            match expr {
                                Expr::$variant(inner) => return Ok(inner),
                                Expr::Group(next) => expr = *next.expr,
                                _ => return Err(Error::new_spanned(expr, $msg)),
                            }
                        }
                    }
                }
            )*
        };
    }

    impl_by_parsing_expr! {
        ExprBox, Box, "expected box expression",
        ExprArray, Array, "expected slice literal expression",
        ExprCall, Call, "expected function call expression",
        ExprMethodCall, MethodCall, "expected method call expression",
        ExprTuple, Tuple, "expected tuple expression",
        ExprBinary, Binary, "expected binary operation",
        ExprUnary, Unary, "expected unary operation",
        ExprCast, Cast, "expected cast expression",
        ExprType, Type, "expected type ascription expression",
        ExprLet, Let, "expected let guard",
        ExprClosure, Closure, "expected closure expression",
        ExprUnsafe, Unsafe, "expected unsafe block",
        ExprBlock, Block, "expected blocked scope",
        ExprAssign, Assign, "expected assignment expression",
        ExprAssignOp, AssignOp, "expected compound assignment expression",
        ExprField, Field, "expected struct field access",
        ExprIndex, Index, "expected indexing expression",
        ExprRange, Range, "expected range expression",
        ExprReference, Reference, "expected referencing operation",
        ExprBreak, Break, "expected break expression",
        ExprContinue, Continue, "expected continue expression",
        ExprReturn, Return, "expected return expression",
        ExprMacro, Macro, "expected macro invocation expression",
        ExprStruct, Struct, "expected struct literal expression",
        ExprRepeat, Repeat, "expected array literal constructed from one repeated element",
        ExprParen, Paren, "expected parenthesized expression",
        ExprTry, Try, "expected try expression",
        ExprAsync, Async, "expected async block",
        ExprTryBlock, TryBlock, "expected try block",
        ExprYield, Yield, "expected yield expression",
    }

    #[cfg(feature = "full")]
    fn expr_try_block(input: ParseStream) -> Result<ExprTryBlock> {
        Ok(ExprTryBlock {
            attrs: Vec::new(),
            try_token: input.parse()?,
            block: input.parse()?,
        })
    }

    #[cfg(feature = "full")]
    fn expr_yield(input: ParseStream) -> Result<ExprYield> {
        Ok(ExprYield {
            attrs: Vec::new(),
            yield_token: input.parse()?,
            expr: {
                if !input.is_empty() && !input.peek(Token![,]) && !input.peek(Token![;]) {
                    Some(input.parse()?)
                } else {
                    None
                }
            },
        })
    }

    #[cfg(feature = "full")]
    fn expr_closure(input: ParseStream, allow_struct: AllowStruct) -> Result<ExprClosure> {
        let asyncness: Option<Token![async]> = input.parse()?;
        let movability: Option<Token![static]> = if asyncness.is_none() {
            input.parse()?
        } else {
            None
        };
        let capture: Option<Token![move]> = input.parse()?;
        let or1_token: Token![|] = input.parse()?;

        let mut inputs = Punctuated::new();
        loop {
            if input.peek(Token![|]) {
                break;
            }
            let value = fn_arg(input)?;
            inputs.push_value(value);
            if input.peek(Token![|]) {
                break;
            }
            let punct: Token![,] = input.parse()?;
            inputs.push_punct(punct);
        }

        let or2_token: Token![|] = input.parse()?;

        let (output, body) = if input.peek(Token![->]) {
            let arrow_token: Token![->] = input.parse()?;
            let ty: Type = input.parse()?;
            let body: Block = input.parse()?;
            let output = ReturnType::Type(arrow_token, Box::new(ty));
            let block = Expr::Block(ExprBlock {
                attrs: Vec::new(),
                label: None,
                block: body,
            });
            (output, block)
        } else {
            let body = ambiguous_expr(input, allow_struct)?;
            (ReturnType::Default, body)
        };

        Ok(ExprClosure {
            attrs: Vec::new(),
            asyncness: asyncness,
            movability: movability,
            capture: capture,
            or1_token: or1_token,
            inputs: inputs,
            or2_token: or2_token,
            output: output,
            body: Box::new(body),
        })
    }

    #[cfg(feature = "full")]
    fn expr_async(input: ParseStream) -> Result<ExprAsync> {
        Ok(ExprAsync {
            attrs: Vec::new(),
            async_token: input.parse()?,
            capture: input.parse()?,
            block: input.parse()?,
        })
    }

    #[cfg(feature = "full")]
    fn fn_arg(input: ParseStream) -> Result<FnArg> {
        let pat: Pat = input.parse()?;

        if input.peek(Token![:]) {
            Ok(FnArg::Captured(ArgCaptured {
                pat: pat,
                colon_token: input.parse()?,
                ty: input.parse()?,
            }))
        } else {
            Ok(FnArg::Inferred(pat))
        }
    }

    #[cfg(feature = "full")]
    impl Parse for ExprWhile {
        fn parse(input: ParseStream) -> Result<Self> {
            let label: Option<Label> = input.parse()?;
            let while_token: Token![while] = input.parse()?;
            let cond = expr_no_struct(input)?;

            let content;
            let brace_token = braced!(content in input);
            let inner_attrs = content.call(Attribute::parse_inner)?;
            let stmts = content.call(Block::parse_within)?;

            Ok(ExprWhile {
                attrs: inner_attrs,
                label: label,
                while_token: while_token,
                cond: Box::new(cond),
                body: Block {
                    brace_token: brace_token,
                    stmts: stmts,
                },
            })
        }
    }

    #[cfg(feature = "full")]
    impl Parse for Label {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(Label {
                name: input.parse()?,
                colon_token: input.parse()?,
            })
        }
    }

    #[cfg(feature = "full")]
    impl Parse for Option<Label> {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Lifetime) {
                input.parse().map(Some)
            } else {
                Ok(None)
            }
        }
    }

    #[cfg(feature = "full")]
    fn expr_continue(input: ParseStream) -> Result<ExprContinue> {
        Ok(ExprContinue {
            attrs: Vec::new(),
            continue_token: input.parse()?,
            label: input.parse()?,
        })
    }

    #[cfg(feature = "full")]
    fn expr_break(input: ParseStream, allow_struct: AllowStruct) -> Result<ExprBreak> {
        Ok(ExprBreak {
            attrs: Vec::new(),
            break_token: input.parse()?,
            label: input.parse()?,
            expr: {
                if input.is_empty()
                    || input.peek(Token![,])
                    || input.peek(Token![;])
                    || !allow_struct.0 && input.peek(token::Brace)
                {
                    None
                } else {
                    let expr = ambiguous_expr(input, allow_struct)?;
                    Some(Box::new(expr))
                }
            },
        })
    }

    #[cfg(feature = "full")]
    fn expr_ret(input: ParseStream, allow_struct: AllowStruct) -> Result<ExprReturn> {
        Ok(ExprReturn {
            attrs: Vec::new(),
            return_token: input.parse()?,
            expr: {
                if input.is_empty() || input.peek(Token![,]) || input.peek(Token![;]) {
                    None
                } else {
                    // NOTE: return is greedy and eats blocks after it even when in a
                    // position where structs are not allowed, such as in if statement
                    // conditions. For example:
                    //
                    // if return { println!("A") } {} // Prints "A"
                    let expr = ambiguous_expr(input, allow_struct)?;
                    Some(Box::new(expr))
                }
            },
        })
    }

    #[cfg(feature = "full")]
    impl Parse for FieldValue {
        fn parse(input: ParseStream) -> Result<Self> {
            let member: Member = input.parse()?;
            let (colon_token, value) = if input.peek(Token![:]) || !member.is_named() {
                let colon_token: Token![:] = input.parse()?;
                let value: Expr = input.parse()?;
                (Some(colon_token), value)
            } else if let Member::Named(ref ident) = member {
                let value = Expr::Path(ExprPath {
                    attrs: Vec::new(),
                    qself: None,
                    path: Path::from(ident.clone()),
                });
                (None, value)
            } else {
                unreachable!()
            };

            Ok(FieldValue {
                attrs: Vec::new(),
                member: member,
                colon_token: colon_token,
                expr: value,
            })
        }
    }

    #[cfg(feature = "full")]
    fn expr_struct_helper(
        input: ParseStream,
        outer_attrs: Vec<Attribute>,
        path: Path,
    ) -> Result<ExprStruct> {
        let content;
        let brace_token = braced!(content in input);
        let inner_attrs = content.call(Attribute::parse_inner)?;

        let mut fields = Punctuated::new();
        loop {
            let attrs = content.call(Attribute::parse_outer)?;
            // TODO: optimize using advance_to
            if content.fork().parse::<Member>().is_err() {
                if attrs.is_empty() {
                    break;
                } else {
                    return Err(content.error("expected struct field"));
                }
            }

            fields.push(FieldValue {
                attrs: attrs,
                ..content.parse()?
            });

            if !content.peek(Token![,]) {
                break;
            }
            let punct: Token![,] = content.parse()?;
            fields.push_punct(punct);
        }

        let (dot2_token, rest) = if fields.empty_or_trailing() && content.peek(Token![..]) {
            let dot2_token: Token![..] = content.parse()?;
            let rest: Expr = content.parse()?;
            (Some(dot2_token), Some(Box::new(rest)))
        } else {
            (None, None)
        };

        Ok(ExprStruct {
            attrs: private::attrs(outer_attrs, inner_attrs),
            brace_token: brace_token,
            path: path,
            fields: fields,
            dot2_token: dot2_token,
            rest: rest,
        })
    }

    #[cfg(feature = "full")]
    fn expr_unsafe(input: ParseStream) -> Result<ExprUnsafe> {
        let unsafe_token: Token![unsafe] = input.parse()?;

        let content;
        let brace_token = braced!(content in input);
        let inner_attrs = content.call(Attribute::parse_inner)?;
        let stmts = content.call(Block::parse_within)?;

        Ok(ExprUnsafe {
            attrs: inner_attrs,
            unsafe_token: unsafe_token,
            block: Block {
                brace_token: brace_token,
                stmts: stmts,
            },
        })
    }

    #[cfg(feature = "full")]
    pub fn expr_block(input: ParseStream) -> Result<ExprBlock> {
        let label: Option<Label> = input.parse()?;

        let content;
        let brace_token = braced!(content in input);
        let inner_attrs = content.call(Attribute::parse_inner)?;
        let stmts = content.call(Block::parse_within)?;

        Ok(ExprBlock {
            attrs: inner_attrs,
            label: label,
            block: Block {
                brace_token: brace_token,
                stmts: stmts,
            },
        })
    }

    #[cfg(feature = "full")]
    fn expr_range(input: ParseStream, allow_struct: AllowStruct) -> Result<ExprRange> {
        Ok(ExprRange {
            attrs: Vec::new(),
            from: None,
            limits: input.parse()?,
            to: {
                if input.is_empty()
                    || input.peek(Token![,])
                    || input.peek(Token![;])
                    || !allow_struct.0 && input.peek(token::Brace)
                {
                    None
                } else {
                    let to = ambiguous_expr(input, allow_struct)?;
                    Some(Box::new(to))
                }
            },
        })
    }

    #[cfg(feature = "full")]
    impl Parse for RangeLimits {
        fn parse(input: ParseStream) -> Result<Self> {
            let lookahead = input.lookahead1();
            if lookahead.peek(Token![..=]) {
                input.parse().map(RangeLimits::Closed)
            } else if lookahead.peek(Token![...]) {
                let dot3: Token![...] = input.parse()?;
                Ok(RangeLimits::Closed(Token![..=](dot3.spans)))
            } else if lookahead.peek(Token![..]) {
                input.parse().map(RangeLimits::HalfOpen)
            } else {
                Err(lookahead.error())
            }
        }
    }

    impl Parse for ExprPath {
        fn parse(input: ParseStream) -> Result<Self> {
            #[cfg(not(feature = "full"))]
            let attrs = Vec::new();
            #[cfg(feature = "full")]
            let attrs = input.call(Attribute::parse_outer)?;

            let (qself, path) = path::parsing::qpath(input, true)?;

            Ok(ExprPath {
                attrs: attrs,
                qself: qself,
                path: path,
            })
        }
    }

    #[cfg(feature = "full")]
    impl Parse for Block {
        fn parse(input: ParseStream) -> Result<Self> {
            let content;
            Ok(Block {
                brace_token: braced!(content in input),
                stmts: content.call(Block::parse_within)?,
            })
        }
    }

    #[cfg(feature = "full")]
    impl Block {
        /// Parse the body of a block as zero or more statements, possibly
        /// including one trailing expression.
        ///
        /// *This function is available if Syn is built with the `"parsing"`
        /// feature.*
        ///
        /// # Example
        ///
        /// ```edition2018
        /// use syn::{braced, token, Attribute, Block, Ident, Result, Stmt, Token};
        /// use syn::parse::{Parse, ParseStream};
        ///
        /// // Parse a function with no generics or parameter list.
        /// //
        /// //     fn playground {
        /// //         let mut x = 1;
        /// //         x += 1;
        /// //         println!("{}", x);
        /// //     }
        /// struct MiniFunction {
        ///     attrs: Vec<Attribute>,
        ///     fn_token: Token![fn],
        ///     name: Ident,
        ///     brace_token: token::Brace,
        ///     stmts: Vec<Stmt>,
        /// }
        ///
        /// impl Parse for MiniFunction {
        ///     fn parse(input: ParseStream) -> Result<Self> {
        ///         let outer_attrs = input.call(Attribute::parse_outer)?;
        ///         let fn_token: Token![fn] = input.parse()?;
        ///         let name: Ident = input.parse()?;
        ///
        ///         let content;
        ///         let brace_token = braced!(content in input);
        ///         let inner_attrs = content.call(Attribute::parse_inner)?;
        ///         let stmts = content.call(Block::parse_within)?;
        ///
        ///         Ok(MiniFunction {
        ///             attrs: {
        ///                 let mut attrs = outer_attrs;
        ///                 attrs.extend(inner_attrs);
        ///                 attrs
        ///             },
        ///             fn_token: fn_token,
        ///             name: name,
        ///             brace_token: brace_token,
        ///             stmts: stmts,
        ///         })
        ///     }
        /// }
        /// ```
        pub fn parse_within(input: ParseStream) -> Result<Vec<Stmt>> {
            let mut stmts = Vec::new();
            loop {
                while input.peek(Token![;]) {
                    input.parse::<Token![;]>()?;
                }
                if input.is_empty() {
                    break;
                }
                let s = parse_stmt(input, true)?;
                let requires_semicolon = if let Stmt::Expr(ref s) = s {
                    requires_terminator(s)
                } else {
                    false
                };
                stmts.push(s);
                if input.is_empty() {
                    break;
                } else if requires_semicolon {
                    return Err(input.error("unexpected token"));
                }
            }
            Ok(stmts)
        }
    }

    #[cfg(feature = "full")]
    impl Parse for Stmt {
        fn parse(input: ParseStream) -> Result<Self> {
            parse_stmt(input, false)
        }
    }

    #[cfg(feature = "full")]
    fn parse_stmt(input: ParseStream, allow_nosemi: bool) -> Result<Stmt> {
        // TODO: optimize using advance_to
        let ahead = input.fork();
        ahead.call(Attribute::parse_outer)?;

        if {
            let ahead = ahead.fork();
            // Only parse braces here; paren and bracket will get parsed as
            // expression statements
            ahead.call(Path::parse_mod_style).is_ok()
                && ahead.parse::<Token![!]>().is_ok()
                && (ahead.peek(token::Brace) || ahead.peek(Ident))
        } {
            stmt_mac(input)
        } else if ahead.peek(Token![let]) {
            stmt_local(input).map(Stmt::Local)
        } else if ahead.peek(Token![pub])
            || ahead.peek(Token![crate]) && !ahead.peek2(Token![::])
            || ahead.peek(Token![extern]) && !ahead.peek2(Token![::])
            || ahead.peek(Token![use])
            || ahead.peek(Token![static]) && (ahead.peek2(Token![mut]) || ahead.peek2(Ident))
            || ahead.peek(Token![const])
            || ahead.peek(Token![unsafe]) && !ahead.peek2(token::Brace)
            || ahead.peek(Token![async])
                && (ahead.peek2(Token![unsafe])
                    || ahead.peek2(Token![extern])
                    || ahead.peek2(Token![fn]))
            || ahead.peek(Token![fn])
            || ahead.peek(Token![mod])
            || ahead.peek(Token![type])
            || ahead.peek(Token![existential]) && ahead.peek2(Token![type])
            || ahead.peek(Token![struct])
            || ahead.peek(Token![enum])
            || ahead.peek(Token![union]) && ahead.peek2(Ident)
            || ahead.peek(Token![auto]) && ahead.peek2(Token![trait])
            || ahead.peek(Token![trait])
            || ahead.peek(Token![default])
                && (ahead.peek2(Token![unsafe]) || ahead.peek2(Token![impl]))
            || ahead.peek(Token![impl])
            || ahead.peek(Token![macro])
        {
            input.parse().map(Stmt::Item)
        } else {
            stmt_expr(input, allow_nosemi)
        }
    }

    #[cfg(feature = "full")]
    fn stmt_mac(input: ParseStream) -> Result<Stmt> {
        let attrs = input.call(Attribute::parse_outer)?;
        let path = input.call(Path::parse_mod_style)?;
        let bang_token: Token![!] = input.parse()?;
        let ident: Option<Ident> = input.parse()?;
        let (delimiter, tts) = mac::parse_delimiter(input)?;
        let semi_token: Option<Token![;]> = input.parse()?;

        Ok(Stmt::Item(Item::Macro(ItemMacro {
            attrs: attrs,
            ident: ident,
            mac: Macro {
                path: path,
                bang_token: bang_token,
                delimiter: delimiter,
                tts: tts,
            },
            semi_token: semi_token,
        })))
    }

    #[cfg(feature = "full")]
    fn stmt_local(input: ParseStream) -> Result<Local> {
        Ok(Local {
            attrs: input.call(Attribute::parse_outer)?,
            let_token: input.parse()?,
            pats: {
                let mut pats = Punctuated::new();
                let value: Pat = input.parse()?;
                pats.push_value(value);
                while input.peek(Token![|]) && !input.peek(Token![||]) && !input.peek(Token![|=]) {
                    let punct = input.parse()?;
                    pats.push_punct(punct);
                    let value: Pat = input.parse()?;
                    pats.push_value(value);
                }
                pats
            },
            ty: {
                if input.peek(Token![:]) {
                    let colon_token: Token![:] = input.parse()?;
                    let ty: Type = input.parse()?;
                    Some((colon_token, Box::new(ty)))
                } else {
                    None
                }
            },
            init: {
                if input.peek(Token![=]) {
                    let eq_token: Token![=] = input.parse()?;
                    let init: Expr = input.parse()?;
                    Some((eq_token, Box::new(init)))
                } else {
                    None
                }
            },
            semi_token: input.parse()?,
        })
    }

    #[cfg(feature = "full")]
    fn stmt_expr(input: ParseStream, allow_nosemi: bool) -> Result<Stmt> {
        let mut attrs = input.call(Attribute::parse_outer)?;
        let mut e = expr_early(input)?;

        attrs.extend(e.replace_attrs(Vec::new()));
        e.replace_attrs(attrs);

        if input.peek(Token![;]) {
            return Ok(Stmt::Semi(e, input.parse()?));
        }

        if allow_nosemi || !requires_terminator(&e) {
            Ok(Stmt::Expr(e))
        } else {
            Err(input.error("expected semicolon"))
        }
    }

    #[cfg(feature = "full")]
    impl Parse for Pat {
        fn parse(input: ParseStream) -> Result<Self> {
            let lookahead = input.lookahead1();
            if lookahead.peek(Token![_]) {
                input.call(pat_wild).map(Pat::Wild)
            } else if lookahead.peek(Token![box]) {
                input.call(pat_box).map(Pat::Box)
            } else if lookahead.peek(Token![-]) || lookahead.peek(Lit) {
                pat_lit_or_range(input)
            } else if input.peek(Ident)
                && ({
                    input.peek2(Token![::])
                        || input.peek2(Token![!])
                        || input.peek2(token::Brace)
                        || input.peek2(token::Paren)
                        || input.peek2(Token![..])
                            && !{
                                let ahead = input.fork();
                                ahead.parse::<Ident>()?;
                                ahead.parse::<RangeLimits>()?;
                                ahead.is_empty() || ahead.peek(Token![,])
                            }
                })
                || input.peek(Token![self]) && input.peek2(Token![::])
                || input.peek(Token![::])
                || input.peek(Token![<])
                || input.peek(Token![Self])
                || input.peek(Token![super])
                || input.peek(Token![extern])
                || input.peek(Token![crate])
            {
                pat_path_or_macro_or_struct_or_range(input)
            } else if input.peek(Token![ref])
                || input.peek(Token![mut])
                || input.peek(Token![self])
                || input.peek(Ident)
            {
                input.call(pat_ident).map(Pat::Ident)
            } else if lookahead.peek(token::Paren) {
                input.call(pat_tuple).map(Pat::Tuple)
            } else if lookahead.peek(Token![&]) {
                input.call(pat_ref).map(Pat::Ref)
            } else if lookahead.peek(token::Bracket) {
                input.call(pat_slice).map(Pat::Slice)
            } else {
                Err(lookahead.error())
            }
        }
    }

    #[cfg(feature = "full")]
    fn pat_path_or_macro_or_struct_or_range(input: ParseStream) -> Result<Pat> {
        let (qself, path) = path::parsing::qpath(input, true)?;

        if input.peek(Token![..]) {
            return pat_range(input, qself, path).map(Pat::Range);
        }

        if qself.is_some() {
            return Ok(Pat::Path(PatPath {
                qself: qself,
                path: path,
            }));
        }

        if input.peek(Token![!]) && !input.peek(Token![!=]) {
            let mut contains_arguments = false;
            for segment in &path.segments {
                match segment.arguments {
                    PathArguments::None => {}
                    PathArguments::AngleBracketed(_) | PathArguments::Parenthesized(_) => {
                        contains_arguments = true;
                    }
                }
            }

            if !contains_arguments {
                let bang_token: Token![!] = input.parse()?;
                let (delimiter, tts) = mac::parse_delimiter(input)?;
                return Ok(Pat::Macro(PatMacro {
                    mac: Macro {
                        path: path,
                        bang_token: bang_token,
                        delimiter: delimiter,
                        tts: tts,
                    },
                }));
            }
        }

        if input.peek(token::Brace) {
            pat_struct(input, path).map(Pat::Struct)
        } else if input.peek(token::Paren) {
            pat_tuple_struct(input, path).map(Pat::TupleStruct)
        } else if input.peek(Token![..]) {
            pat_range(input, qself, path).map(Pat::Range)
        } else {
            Ok(Pat::Path(PatPath {
                qself: qself,
                path: path,
            }))
        }
    }

    #[cfg(feature = "full")]
    fn pat_wild(input: ParseStream) -> Result<PatWild> {
        Ok(PatWild {
            underscore_token: input.parse()?,
        })
    }

    #[cfg(feature = "full")]
    fn pat_box(input: ParseStream) -> Result<PatBox> {
        Ok(PatBox {
            box_token: input.parse()?,
            pat: input.parse()?,
        })
    }

    #[cfg(feature = "full")]
    fn pat_ident(input: ParseStream) -> Result<PatIdent> {
        Ok(PatIdent {
            by_ref: input.parse()?,
            mutability: input.parse()?,
            ident: input.call(Ident::parse_any)?,
            subpat: {
                if input.peek(Token![@]) {
                    let at_token: Token![@] = input.parse()?;
                    let subpat: Pat = input.parse()?;
                    Some((at_token, Box::new(subpat)))
                } else {
                    None
                }
            },
        })
    }

    #[cfg(feature = "full")]
    fn pat_tuple_struct(input: ParseStream, path: Path) -> Result<PatTupleStruct> {
        Ok(PatTupleStruct {
            path: path,
            pat: input.call(pat_tuple)?,
        })
    }

    #[cfg(feature = "full")]
    fn pat_struct(input: ParseStream, path: Path) -> Result<PatStruct> {
        let content;
        let brace_token = braced!(content in input);

        let mut fields = Punctuated::new();
        while !content.is_empty() && !content.peek(Token![..]) {
            let value = content.call(field_pat)?;
            fields.push_value(value);
            if !content.peek(Token![,]) {
                break;
            }
            let punct: Token![,] = content.parse()?;
            fields.push_punct(punct);
        }

        let dot2_token = if fields.empty_or_trailing() && content.peek(Token![..]) {
            Some(content.parse()?)
        } else {
            None
        };

        Ok(PatStruct {
            path: path,
            brace_token: brace_token,
            fields: fields,
            dot2_token: dot2_token,
        })
    }

    #[cfg(feature = "full")]
    fn field_pat(input: ParseStream) -> Result<FieldPat> {
        let boxed: Option<Token![box]> = input.parse()?;
        let by_ref: Option<Token![ref]> = input.parse()?;
        let mutability: Option<Token![mut]> = input.parse()?;
        let member: Member = input.parse()?;

        if boxed.is_none() && by_ref.is_none() && mutability.is_none() && input.peek(Token![:])
            || member.is_unnamed()
        {
            return Ok(FieldPat {
                attrs: Vec::new(),
                member: member,
                colon_token: input.parse()?,
                pat: input.parse()?,
            });
        }

        let ident = match member {
            Member::Named(ident) => ident,
            Member::Unnamed(_) => unreachable!(),
        };

        let mut pat = Pat::Ident(PatIdent {
            by_ref: by_ref,
            mutability: mutability,
            ident: ident.clone(),
            subpat: None,
        });

        if let Some(boxed) = boxed {
            pat = Pat::Box(PatBox {
                pat: Box::new(pat),
                box_token: boxed,
            });
        }

        Ok(FieldPat {
            member: Member::Named(ident),
            pat: Box::new(pat),
            attrs: Vec::new(),
            colon_token: None,
        })
    }

    impl Parse for Member {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Ident) {
                input.parse().map(Member::Named)
            } else if input.peek(LitInt) {
                input.parse().map(Member::Unnamed)
            } else {
                Err(input.error("expected identifier or integer"))
            }
        }
    }

    #[cfg(feature = "full")]
    impl Parse for Arm {
        fn parse(input: ParseStream) -> Result<Arm> {
            let requires_comma;
            Ok(Arm {
                attrs: input.call(Attribute::parse_outer)?,
                leading_vert: input.parse()?,
                pats: {
                    let mut pats = Punctuated::new();
                    let value: Pat = input.parse()?;
                    pats.push_value(value);
                    loop {
                        if !input.peek(Token![|]) {
                            break;
                        }
                        let punct = input.parse()?;
                        pats.push_punct(punct);
                        let value: Pat = input.parse()?;
                        pats.push_value(value);
                    }
                    pats
                },
                guard: {
                    if input.peek(Token![if]) {
                        let if_token: Token![if] = input.parse()?;
                        let guard: Expr = input.parse()?;
                        Some((if_token, Box::new(guard)))
                    } else {
                        None
                    }
                },
                fat_arrow_token: input.parse()?,
                body: {
                    let body = input.call(expr_early)?;
                    requires_comma = requires_terminator(&body);
                    Box::new(body)
                },
                comma: {
                    if requires_comma && !input.is_empty() {
                        Some(input.parse()?)
                    } else {
                        input.parse()?
                    }
                },
            })
        }
    }

    impl Parse for Index {
        fn parse(input: ParseStream) -> Result<Self> {
            let lit: LitInt = input.parse()?;
            if let IntSuffix::None = lit.suffix() {
                Ok(Index {
                    index: lit.value() as u32,
                    span: lit.span(),
                })
            } else {
                Err(Error::new(lit.span(), "expected unsuffixed integer"))
            }
        }
    }

    #[cfg(feature = "full")]
    fn pat_range(input: ParseStream, qself: Option<QSelf>, path: Path) -> Result<PatRange> {
        Ok(PatRange {
            lo: Box::new(Expr::Path(ExprPath {
                attrs: Vec::new(),
                qself: qself,
                path: path,
            })),
            limits: input.parse()?,
            hi: input.call(pat_lit_expr)?,
        })
    }

    #[cfg(feature = "full")]
    fn pat_tuple(input: ParseStream) -> Result<PatTuple> {
        let content;
        let paren_token = parenthesized!(content in input);

        let mut front = Punctuated::new();
        let mut dot2_token = None::<Token![..]>;
        let mut comma_token = None::<Token![,]>;
        loop {
            if content.is_empty() {
                break;
            }
            if content.peek(Token![..]) {
                dot2_token = Some(content.parse()?);
                comma_token = content.parse()?;
                break;
            }
            let value: Pat = content.parse()?;
            front.push_value(value);
            if content.is_empty() {
                break;
            }
            let punct = content.parse()?;
            front.push_punct(punct);
        }

        let mut back = Punctuated::new();
        while !content.is_empty() {
            let value: Pat = content.parse()?;
            back.push_value(value);
            if content.is_empty() {
                break;
            }
            let punct = content.parse()?;
            back.push_punct(punct);
        }

        Ok(PatTuple {
            paren_token: paren_token,
            front: front,
            dot2_token: dot2_token,
            comma_token: comma_token,
            back: back,
        })
    }

    #[cfg(feature = "full")]
    fn pat_ref(input: ParseStream) -> Result<PatRef> {
        Ok(PatRef {
            and_token: input.parse()?,
            mutability: input.parse()?,
            pat: input.parse()?,
        })
    }

    #[cfg(feature = "full")]
    fn pat_lit_or_range(input: ParseStream) -> Result<Pat> {
        let lo = input.call(pat_lit_expr)?;
        if input.peek(Token![..]) {
            Ok(Pat::Range(PatRange {
                lo: lo,
                limits: input.parse()?,
                hi: input.call(pat_lit_expr)?,
            }))
        } else {
            Ok(Pat::Lit(PatLit { expr: lo }))
        }
    }

    #[cfg(feature = "full")]
    fn pat_lit_expr(input: ParseStream) -> Result<Box<Expr>> {
        let neg: Option<Token![-]> = input.parse()?;

        let lookahead = input.lookahead1();
        let expr = if lookahead.peek(Lit) {
            Expr::Lit(input.parse()?)
        } else if lookahead.peek(Ident)
            || lookahead.peek(Token![::])
            || lookahead.peek(Token![<])
            || lookahead.peek(Token![self])
            || lookahead.peek(Token![Self])
            || lookahead.peek(Token![super])
            || lookahead.peek(Token![extern])
            || lookahead.peek(Token![crate])
        {
            Expr::Path(input.parse()?)
        } else {
            return Err(lookahead.error());
        };

        Ok(Box::new(if let Some(neg) = neg {
            Expr::Unary(ExprUnary {
                attrs: Vec::new(),
                op: UnOp::Neg(neg),
                expr: Box::new(expr),
            })
        } else {
            expr
        }))
    }

    #[cfg(feature = "full")]
    fn pat_slice(input: ParseStream) -> Result<PatSlice> {
        let content;
        let bracket_token = bracketed!(content in input);

        let mut front = Punctuated::new();
        let mut middle = None;
        loop {
            if content.is_empty() || content.peek(Token![..]) {
                break;
            }
            let value: Pat = content.parse()?;
            if content.peek(Token![..]) {
                middle = Some(Box::new(value));
                break;
            }
            front.push_value(value);
            if content.is_empty() {
                break;
            }
            let punct = content.parse()?;
            front.push_punct(punct);
        }

        let dot2_token: Option<Token![..]> = content.parse()?;
        let mut comma_token = None::<Token![,]>;
        let mut back = Punctuated::new();
        if dot2_token.is_some() {
            comma_token = content.parse()?;
            if comma_token.is_some() {
                loop {
                    if content.is_empty() {
                        break;
                    }
                    let value: Pat = content.parse()?;
                    back.push_value(value);
                    if content.is_empty() {
                        break;
                    }
                    let punct = content.parse()?;
                    back.push_punct(punct);
                }
            }
        }

        Ok(PatSlice {
            bracket_token: bracket_token,
            front: front,
            middle: middle,
            dot2_token: dot2_token,
            comma_token: comma_token,
            back: back,
        })
    }

    #[cfg(feature = "full")]
    impl Member {
        fn is_named(&self) -> bool {
            match *self {
                Member::Named(_) => true,
                Member::Unnamed(_) => false,
            }
        }

        fn is_unnamed(&self) -> bool {
            match *self {
                Member::Named(_) => false,
                Member::Unnamed(_) => true,
            }
        }
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;

    use proc_macro2::{Literal, TokenStream};
    use quote::{ToTokens, TokenStreamExt};

    #[cfg(feature = "full")]
    use attr::FilterAttrs;
    #[cfg(feature = "full")]
    use print::TokensOrDefault;

    // If the given expression is a bare `ExprStruct`, wraps it in parenthesis
    // before appending it to `TokenStream`.
    #[cfg(feature = "full")]
    fn wrap_bare_struct(tokens: &mut TokenStream, e: &Expr) {
        if let Expr::Struct(_) = *e {
            token::Paren::default().surround(tokens, |tokens| {
                e.to_tokens(tokens);
            });
        } else {
            e.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    fn outer_attrs_to_tokens(attrs: &[Attribute], tokens: &mut TokenStream) {
        tokens.append_all(attrs.outer());
    }

    #[cfg(feature = "full")]
    fn inner_attrs_to_tokens(attrs: &[Attribute], tokens: &mut TokenStream) {
        tokens.append_all(attrs.inner());
    }

    #[cfg(not(feature = "full"))]
    fn outer_attrs_to_tokens(_attrs: &[Attribute], _tokens: &mut TokenStream) {}

    #[cfg(not(feature = "full"))]
    fn inner_attrs_to_tokens(_attrs: &[Attribute], _tokens: &mut TokenStream) {}

    #[cfg(feature = "full")]
    impl ToTokens for ExprBox {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.box_token.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprInPlace {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.place.to_tokens(tokens);
            self.arrow_token.to_tokens(tokens);
            self.value.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprArray {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.bracket_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                self.elems.to_tokens(tokens);
            })
        }
    }

    impl ToTokens for ExprCall {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.func.to_tokens(tokens);
            self.paren_token.surround(tokens, |tokens| {
                self.args.to_tokens(tokens);
            })
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprMethodCall {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.receiver.to_tokens(tokens);
            self.dot_token.to_tokens(tokens);
            self.method.to_tokens(tokens);
            self.turbofish.to_tokens(tokens);
            self.paren_token.surround(tokens, |tokens| {
                self.args.to_tokens(tokens);
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for MethodTurbofish {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.colon2_token.to_tokens(tokens);
            self.lt_token.to_tokens(tokens);
            self.args.to_tokens(tokens);
            self.gt_token.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for GenericMethodArgument {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match *self {
                GenericMethodArgument::Type(ref t) => t.to_tokens(tokens),
                GenericMethodArgument::Const(ref c) => c.to_tokens(tokens),
            }
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprTuple {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.paren_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                self.elems.to_tokens(tokens);
                // If we only have one argument, we need a trailing comma to
                // distinguish ExprTuple from ExprParen.
                if self.elems.len() == 1 && !self.elems.trailing_punct() {
                    <Token![,]>::default().to_tokens(tokens);
                }
            })
        }
    }

    impl ToTokens for ExprBinary {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.left.to_tokens(tokens);
            self.op.to_tokens(tokens);
            self.right.to_tokens(tokens);
        }
    }

    impl ToTokens for ExprUnary {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.op.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    impl ToTokens for ExprLit {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.lit.to_tokens(tokens);
        }
    }

    impl ToTokens for ExprCast {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.expr.to_tokens(tokens);
            self.as_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprType {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.expr.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    fn maybe_wrap_else(tokens: &mut TokenStream, else_: &Option<(Token![else], Box<Expr>)>) {
        if let Some((ref else_token, ref else_)) = *else_ {
            else_token.to_tokens(tokens);

            // If we are not one of the valid expressions to exist in an else
            // clause, wrap ourselves in a block.
            match **else_ {
                Expr::If(_) | Expr::Block(_) => {
                    else_.to_tokens(tokens);
                }
                _ => {
                    token::Brace::default().surround(tokens, |tokens| {
                        else_.to_tokens(tokens);
                    });
                }
            }
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprLet {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.let_token.to_tokens(tokens);
            self.pats.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            wrap_bare_struct(tokens, &self.expr);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprIf {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.if_token.to_tokens(tokens);
            wrap_bare_struct(tokens, &self.cond);
            self.then_branch.to_tokens(tokens);
            maybe_wrap_else(tokens, &self.else_branch);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprWhile {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.label.to_tokens(tokens);
            self.while_token.to_tokens(tokens);
            wrap_bare_struct(tokens, &self.cond);
            self.body.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                tokens.append_all(&self.body.stmts);
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprForLoop {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.label.to_tokens(tokens);
            self.for_token.to_tokens(tokens);
            self.pat.to_tokens(tokens);
            self.in_token.to_tokens(tokens);
            wrap_bare_struct(tokens, &self.expr);
            self.body.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                tokens.append_all(&self.body.stmts);
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprLoop {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.label.to_tokens(tokens);
            self.loop_token.to_tokens(tokens);
            self.body.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                tokens.append_all(&self.body.stmts);
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprMatch {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.match_token.to_tokens(tokens);
            wrap_bare_struct(tokens, &self.expr);
            self.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                for (i, arm) in self.arms.iter().enumerate() {
                    arm.to_tokens(tokens);
                    // Ensure that we have a comma after a non-block arm, except
                    // for the last one.
                    let is_last = i == self.arms.len() - 1;
                    if !is_last && requires_terminator(&arm.body) && arm.comma.is_none() {
                        <Token![,]>::default().to_tokens(tokens);
                    }
                }
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprAsync {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.async_token.to_tokens(tokens);
            self.capture.to_tokens(tokens);
            self.block.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprTryBlock {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.try_token.to_tokens(tokens);
            self.block.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprYield {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.yield_token.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprClosure {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.asyncness.to_tokens(tokens);
            self.movability.to_tokens(tokens);
            self.capture.to_tokens(tokens);
            self.or1_token.to_tokens(tokens);
            for input in self.inputs.pairs() {
                match **input.value() {
                    FnArg::Captured(ArgCaptured {
                        ref pat,
                        ty: Type::Infer(_),
                        ..
                    }) => {
                        pat.to_tokens(tokens);
                    }
                    _ => input.value().to_tokens(tokens),
                }
                input.punct().to_tokens(tokens);
            }
            self.or2_token.to_tokens(tokens);
            self.output.to_tokens(tokens);
            self.body.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprUnsafe {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.unsafe_token.to_tokens(tokens);
            self.block.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                tokens.append_all(&self.block.stmts);
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprBlock {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.label.to_tokens(tokens);
            self.block.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                tokens.append_all(&self.block.stmts);
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprAssign {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.left.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            self.right.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprAssignOp {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.left.to_tokens(tokens);
            self.op.to_tokens(tokens);
            self.right.to_tokens(tokens);
        }
    }

    impl ToTokens for ExprField {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.base.to_tokens(tokens);
            self.dot_token.to_tokens(tokens);
            self.member.to_tokens(tokens);
        }
    }

    impl ToTokens for Member {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match *self {
                Member::Named(ref ident) => ident.to_tokens(tokens),
                Member::Unnamed(ref index) => index.to_tokens(tokens),
            }
        }
    }

    impl ToTokens for Index {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let mut lit = Literal::i64_unsuffixed(i64::from(self.index));
            lit.set_span(self.span);
            tokens.append(lit);
        }
    }

    impl ToTokens for ExprIndex {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.expr.to_tokens(tokens);
            self.bracket_token.surround(tokens, |tokens| {
                self.index.to_tokens(tokens);
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprRange {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.from.to_tokens(tokens);
            match self.limits {
                RangeLimits::HalfOpen(ref t) => t.to_tokens(tokens),
                RangeLimits::Closed(ref t) => t.to_tokens(tokens),
            }
            self.to.to_tokens(tokens);
        }
    }

    impl ToTokens for ExprPath {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            private::print_path(tokens, &self.qself, &self.path);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprReference {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.and_token.to_tokens(tokens);
            self.mutability.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprBreak {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.break_token.to_tokens(tokens);
            self.label.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprContinue {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.continue_token.to_tokens(tokens);
            self.label.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprReturn {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.return_token.to_tokens(tokens);
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprMacro {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.mac.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprStruct {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.path.to_tokens(tokens);
            self.brace_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                self.fields.to_tokens(tokens);
                if self.rest.is_some() {
                    TokensOrDefault(&self.dot2_token).to_tokens(tokens);
                    self.rest.to_tokens(tokens);
                }
            })
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprRepeat {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.bracket_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                self.expr.to_tokens(tokens);
                self.semi_token.to_tokens(tokens);
                self.len.to_tokens(tokens);
            })
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprGroup {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.group_token.surround(tokens, |tokens| {
                self.expr.to_tokens(tokens);
            });
        }
    }

    impl ToTokens for ExprParen {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.paren_token.surround(tokens, |tokens| {
                inner_attrs_to_tokens(&self.attrs, tokens);
                self.expr.to_tokens(tokens);
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for ExprTry {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.expr.to_tokens(tokens);
            self.question_token.to_tokens(tokens);
        }
    }

    impl ToTokens for ExprVerbatim {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.tts.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for Label {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.name.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for FieldValue {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.member.to_tokens(tokens);
            if let Some(ref colon_token) = self.colon_token {
                colon_token.to_tokens(tokens);
                self.expr.to_tokens(tokens);
            }
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for Arm {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(&self.attrs);
            self.leading_vert.to_tokens(tokens);
            self.pats.to_tokens(tokens);
            if let Some((ref if_token, ref guard)) = self.guard {
                if_token.to_tokens(tokens);
                guard.to_tokens(tokens);
            }
            self.fat_arrow_token.to_tokens(tokens);
            self.body.to_tokens(tokens);
            self.comma.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatWild {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.underscore_token.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatIdent {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.by_ref.to_tokens(tokens);
            self.mutability.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            if let Some((ref at_token, ref subpat)) = self.subpat {
                at_token.to_tokens(tokens);
                subpat.to_tokens(tokens);
            }
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatStruct {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.path.to_tokens(tokens);
            self.brace_token.surround(tokens, |tokens| {
                self.fields.to_tokens(tokens);
                // NOTE: We need a comma before the dot2 token if it is present.
                if !self.fields.empty_or_trailing() && self.dot2_token.is_some() {
                    <Token![,]>::default().to_tokens(tokens);
                }
                self.dot2_token.to_tokens(tokens);
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatTupleStruct {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.path.to_tokens(tokens);
            self.pat.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatPath {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            private::print_path(tokens, &self.qself, &self.path);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatTuple {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.paren_token.surround(tokens, |tokens| {
                self.front.to_tokens(tokens);
                if let Some(ref dot2_token) = self.dot2_token {
                    if !self.front.empty_or_trailing() {
                        // Ensure there is a comma before the .. token.
                        <Token![,]>::default().to_tokens(tokens);
                    }
                    dot2_token.to_tokens(tokens);
                    self.comma_token.to_tokens(tokens);
                    if self.comma_token.is_none() && !self.back.is_empty() {
                        // Ensure there is a comma after the .. token.
                        <Token![,]>::default().to_tokens(tokens);
                    }
                }
                self.back.to_tokens(tokens);
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatBox {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.box_token.to_tokens(tokens);
            self.pat.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatRef {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.and_token.to_tokens(tokens);
            self.mutability.to_tokens(tokens);
            self.pat.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatLit {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatRange {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.lo.to_tokens(tokens);
            match self.limits {
                RangeLimits::HalfOpen(ref t) => t.to_tokens(tokens),
                RangeLimits::Closed(ref t) => t.to_tokens(tokens),
            }
            self.hi.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatSlice {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.bracket_token.surround(tokens, |tokens| {
                self.front.to_tokens(tokens);

                // If we need a comma before the middle or standalone .. token,
                // then make sure it's present.
                if !self.front.empty_or_trailing()
                    && (self.middle.is_some() || self.dot2_token.is_some())
                {
                    <Token![,]>::default().to_tokens(tokens);
                }

                // If we have an identifier, we always need a .. token.
                if self.middle.is_some() {
                    self.middle.to_tokens(tokens);
                    TokensOrDefault(&self.dot2_token).to_tokens(tokens);
                } else if self.dot2_token.is_some() {
                    self.dot2_token.to_tokens(tokens);
                }

                // Make sure we have a comma before the back half.
                if !self.back.is_empty() {
                    TokensOrDefault(&self.comma_token).to_tokens(tokens);
                    self.back.to_tokens(tokens);
                } else {
                    self.comma_token.to_tokens(tokens);
                }
            })
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatMacro {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.mac.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for PatVerbatim {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.tts.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for FieldPat {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            if let Some(ref colon_token) = self.colon_token {
                self.member.to_tokens(tokens);
                colon_token.to_tokens(tokens);
            }
            self.pat.to_tokens(tokens);
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for Block {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.brace_token.surround(tokens, |tokens| {
                tokens.append_all(&self.stmts);
            });
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for Stmt {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match *self {
                Stmt::Local(ref local) => local.to_tokens(tokens),
                Stmt::Item(ref item) => item.to_tokens(tokens),
                Stmt::Expr(ref expr) => expr.to_tokens(tokens),
                Stmt::Semi(ref expr, ref semi) => {
                    expr.to_tokens(tokens);
                    semi.to_tokens(tokens);
                }
            }
        }
    }

    #[cfg(feature = "full")]
    impl ToTokens for Local {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            outer_attrs_to_tokens(&self.attrs, tokens);
            self.let_token.to_tokens(tokens);
            self.pats.to_tokens(tokens);
            if let Some((ref colon_token, ref ty)) = self.ty {
                colon_token.to_tokens(tokens);
                ty.to_tokens(tokens);
            }
            if let Some((ref eq_token, ref init)) = self.init {
                eq_token.to_tokens(tokens);
                init.to_tokens(tokens);
            }
            self.semi_token.to_tokens(tokens);
        }
    }
}
