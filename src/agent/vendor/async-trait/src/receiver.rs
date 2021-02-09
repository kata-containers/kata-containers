use crate::respan::respan;
use proc_macro2::{Group, Spacing, Span, TokenStream, TokenTree};
use quote::{quote, quote_spanned};
use std::iter::FromIterator;
use std::mem;
use syn::punctuated::Punctuated;
use syn::visit_mut::{self, VisitMut};
use syn::{
    parse_quote, Block, Error, ExprPath, ExprStruct, Ident, Item, Macro, PatPath, PatStruct,
    PatTupleStruct, Path, PathArguments, QSelf, Receiver, Signature, Token, Type, TypePath,
    WherePredicate,
};

pub fn has_self_in_sig(sig: &mut Signature) -> bool {
    let mut visitor = HasSelf(false);
    visitor.visit_signature_mut(sig);
    visitor.0
}

pub fn has_self_in_where_predicate(where_predicate: &mut WherePredicate) -> bool {
    let mut visitor = HasSelf(false);
    visitor.visit_where_predicate_mut(where_predicate);
    visitor.0
}

pub fn has_self_in_block(block: &mut Block) -> bool {
    let mut visitor = HasSelf(false);
    visitor.visit_block_mut(block);
    visitor.0
}

fn has_self_in_token_stream(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|tt| match tt {
        TokenTree::Ident(ident) => ident == "Self",
        TokenTree::Group(group) => has_self_in_token_stream(group.stream()),
        _ => false,
    })
}

struct HasSelf(bool);

impl VisitMut for HasSelf {
    fn visit_expr_path_mut(&mut self, expr: &mut ExprPath) {
        self.0 |= expr.path.segments[0].ident == "Self";
        visit_mut::visit_expr_path_mut(self, expr);
    }

    fn visit_pat_path_mut(&mut self, pat: &mut PatPath) {
        self.0 |= pat.path.segments[0].ident == "Self";
        visit_mut::visit_pat_path_mut(self, pat);
    }

    fn visit_type_path_mut(&mut self, ty: &mut TypePath) {
        self.0 |= ty.path.segments[0].ident == "Self";
        visit_mut::visit_type_path_mut(self, ty);
    }

    fn visit_receiver_mut(&mut self, _arg: &mut Receiver) {
        self.0 = true;
    }

    fn visit_item_mut(&mut self, _: &mut Item) {
        // Do not recurse into nested items.
    }

    fn visit_macro_mut(&mut self, mac: &mut Macro) {
        if !contains_fn(mac.tokens.clone()) {
            self.0 |= has_self_in_token_stream(mac.tokens.clone());
        }
    }
}

pub struct ReplaceReceiver {
    pub with: Type,
    pub as_trait: Option<Path>,
}

impl ReplaceReceiver {
    pub fn with(ty: Type) -> Self {
        ReplaceReceiver {
            with: ty,
            as_trait: None,
        }
    }

    pub fn with_as_trait(ty: Type, as_trait: Path) -> Self {
        ReplaceReceiver {
            with: ty,
            as_trait: Some(as_trait),
        }
    }

    fn self_ty(&self, span: Span) -> Type {
        respan(&self.with, span)
    }

    fn self_to_qself_type(&self, qself: &mut Option<QSelf>, path: &mut Path) {
        let include_as_trait = true;
        self.self_to_qself(qself, path, include_as_trait);
    }

    fn self_to_qself_expr(&self, qself: &mut Option<QSelf>, path: &mut Path) {
        let include_as_trait = false;
        self.self_to_qself(qself, path, include_as_trait);
    }

    fn self_to_qself(&self, qself: &mut Option<QSelf>, path: &mut Path, include_as_trait: bool) {
        if path.leading_colon.is_some() {
            return;
        }

        let first = &path.segments[0];
        if first.ident != "Self" || !first.arguments.is_empty() {
            return;
        }

        if path.segments.len() == 1 {
            self.self_to_expr_path(path);
            return;
        }

        let span = first.ident.span();
        *qself = Some(QSelf {
            lt_token: Token![<](span),
            ty: Box::new(self.self_ty(span)),
            position: 0,
            as_token: None,
            gt_token: Token![>](span),
        });

        if include_as_trait && self.as_trait.is_some() {
            let as_trait = self.as_trait.as_ref().unwrap().clone();
            path.leading_colon = as_trait.leading_colon;
            qself.as_mut().unwrap().position = as_trait.segments.len();

            let segments = mem::replace(&mut path.segments, as_trait.segments);
            path.segments.push_punct(Default::default());
            path.segments.extend(segments.into_pairs().skip(1));
        } else {
            path.leading_colon = Some(**path.segments.pairs().next().unwrap().punct().unwrap());

            let segments = mem::replace(&mut path.segments, Punctuated::new());
            path.segments = segments.into_pairs().skip(1).collect();
        }
    }

    fn self_to_expr_path(&self, path: &mut Path) {
        if path.leading_colon.is_some() {
            return;
        }

        let first = &path.segments[0];
        if first.ident != "Self" || !first.arguments.is_empty() {
            return;
        }

        if let Type::Path(self_ty) = self.self_ty(first.ident.span()) {
            let variant = mem::replace(path, self_ty.path);
            for segment in &mut path.segments {
                if let PathArguments::AngleBracketed(bracketed) = &mut segment.arguments {
                    if bracketed.colon2_token.is_none() && !bracketed.args.is_empty() {
                        bracketed.colon2_token = Some(Default::default());
                    }
                }
            }
            if variant.segments.len() > 1 {
                path.segments.push_punct(Default::default());
                path.segments.extend(variant.segments.into_pairs().skip(1));
            }
        } else {
            let span = path.segments[0].ident.span();
            let msg = "Self type of this impl is unsupported in expression position";
            let error = Error::new(span, msg).to_compile_error();
            *path = parse_quote!(::core::marker::PhantomData::<#error>);
        }
    }

    fn visit_token_stream(&self, tokens: &mut TokenStream) -> bool {
        let mut out = Vec::new();
        let mut modified = false;
        let mut iter = tokens.clone().into_iter().peekable();
        while let Some(tt) = iter.next() {
            match tt {
                TokenTree::Ident(mut ident) => {
                    modified |= prepend_underscore_to_self(&mut ident);
                    if ident == "Self" {
                        modified = true;
                        if self.as_trait.is_none() {
                            let ident = Ident::new("AsyncTrait", ident.span());
                            out.push(TokenTree::Ident(ident));
                        } else {
                            let self_ty = self.self_ty(ident.span());
                            match iter.peek() {
                                Some(TokenTree::Punct(p))
                                    if p.as_char() == ':' && p.spacing() == Spacing::Joint =>
                                {
                                    let next = iter.next().unwrap();
                                    match iter.peek() {
                                        Some(TokenTree::Punct(p)) if p.as_char() == ':' => {
                                            let span = ident.span();
                                            out.extend(quote_spanned!(span=> <#self_ty>));
                                        }
                                        _ => out.extend(quote!(#self_ty)),
                                    }
                                    out.push(next);
                                }
                                _ => out.extend(quote!(#self_ty)),
                            }
                        }
                    } else {
                        out.push(TokenTree::Ident(ident));
                    }
                }
                TokenTree::Group(group) => {
                    let mut content = group.stream();
                    modified |= self.visit_token_stream(&mut content);
                    let mut new = Group::new(group.delimiter(), content);
                    new.set_span(group.span());
                    out.push(TokenTree::Group(new));
                }
                other => out.push(other),
            }
        }
        if modified {
            *tokens = TokenStream::from_iter(out);
        }
        modified
    }
}

impl VisitMut for ReplaceReceiver {
    // `Self` -> `Receiver`
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(node) = ty {
            if node.qself.is_none() && node.path.is_ident("Self") {
                *ty = self.self_ty(node.path.segments[0].ident.span());
            } else {
                self.visit_type_path_mut(node);
            }
        } else {
            visit_mut::visit_type_mut(self, ty);
        }
    }

    // `Self::Assoc` -> `<Receiver>::Assoc`
    fn visit_type_path_mut(&mut self, ty: &mut TypePath) {
        if ty.qself.is_none() {
            self.self_to_qself_type(&mut ty.qself, &mut ty.path);
        }
        visit_mut::visit_type_path_mut(self, ty);
    }

    // `Self::method` -> `<Receiver>::method`
    fn visit_expr_path_mut(&mut self, expr: &mut ExprPath) {
        if expr.qself.is_none() {
            prepend_underscore_to_self(&mut expr.path.segments[0].ident);
            self.self_to_qself_expr(&mut expr.qself, &mut expr.path);
        }
        visit_mut::visit_expr_path_mut(self, expr);
    }

    fn visit_expr_struct_mut(&mut self, expr: &mut ExprStruct) {
        self.self_to_expr_path(&mut expr.path);
        visit_mut::visit_expr_struct_mut(self, expr);
    }

    fn visit_pat_path_mut(&mut self, pat: &mut PatPath) {
        if pat.qself.is_none() {
            self.self_to_qself_expr(&mut pat.qself, &mut pat.path);
        }
        visit_mut::visit_pat_path_mut(self, pat);
    }

    fn visit_pat_struct_mut(&mut self, pat: &mut PatStruct) {
        self.self_to_expr_path(&mut pat.path);
        visit_mut::visit_pat_struct_mut(self, pat);
    }

    fn visit_pat_tuple_struct_mut(&mut self, pat: &mut PatTupleStruct) {
        self.self_to_expr_path(&mut pat.path);
        visit_mut::visit_pat_tuple_struct_mut(self, pat);
    }

    fn visit_item_mut(&mut self, i: &mut Item) {
        match i {
            // Visit `macro_rules!` because locally defined macros can refer to `self`.
            Item::Macro(i) if i.mac.path.is_ident("macro_rules") => {
                self.visit_macro_mut(&mut i.mac)
            }
            // Otherwise, do not recurse into nested items.
            _ => {}
        }
    }

    fn visit_macro_mut(&mut self, mac: &mut Macro) {
        // We can't tell in general whether `self` inside a macro invocation
        // refers to the self in the argument list or a different self
        // introduced within the macro. Heuristic: if the macro input contains
        // `fn`, then `self` is more likely to refer to something other than the
        // outer function's self argument.
        if !contains_fn(mac.tokens.clone()) {
            self.visit_token_stream(&mut mac.tokens);
        }
    }
}

fn contains_fn(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|tt| match tt {
        TokenTree::Ident(ident) => ident == "fn",
        TokenTree::Group(group) => contains_fn(group.stream()),
        _ => false,
    })
}

fn prepend_underscore_to_self(ident: &mut Ident) -> bool {
    let modified = ident == "self";
    if modified {
        *ident = Ident::new("_self", ident.span());
    }
    modified
}
