//!# A custom derive implementation for `#[derive(new)]`
//!
//!A `derive(new)` attribute creates a `new` constructor function for the annotated
//!type. That function takes an argument for each field in the type giving a
//!trivial constructor. This is useful since as your type evolves you can make the
//!constructor non-trivial (and add or remove fields) without changing client code
//!(i.e., without breaking backwards compatibility). It is also the most succinct
//!way to initialise a struct or an enum.
//!
//!Implementation uses macros 1.1 custom derive (which works in stable Rust from
//!1.15 onwards).
//!
//!## Examples
//!
//!Cargo.toml:
//!
//!```toml
//![dependencies]
//!derive-new = "0.5"
//!```
//!
//!Include the macro:
//!
//!```rust
//!#[macro_use]
//!extern crate derive_new;
//!fn main() {}
//!```
//!
//!Generating constructor for a simple struct:
//!
//!```rust
//!#[macro_use]
//!extern crate derive_new;
//!#[derive(new)]
//!struct Bar {
//!    a: i32,
//!    b: String,
//!}
//!
//!fn main() {
//!  let _ = Bar::new(42, "Hello".to_owned());
//!}
//!```
//!
//!Default values can be specified either via `#[new(default)]` attribute which removes
//!the argument from the constructor and populates the field with `Default::default()`,
//!or via `#[new(value = "..")]` which initializes the field with a given expression:
//!
//!```rust
//!#[macro_use]
//!extern crate derive_new;
//!#[derive(new)]
//!struct Foo {
//!    x: bool,
//!    #[new(value = "42")]
//!    y: i32,
//!    #[new(default)]
//!    z: Vec<String>,
//!}
//!
//!fn main() {
//!  let _ = Foo::new(true);
//!}
//!```
//!
//!Generic types are supported; in particular, `PhantomData<T>` fields will be not
//!included in the argument list and will be intialized automatically:
//!
//!```rust
//!#[macro_use]
//!extern crate derive_new;
//!use std::marker::PhantomData;
//!
//!#[derive(new)]
//!struct Generic<'a, T: Default, P> {
//!    x: &'a str,
//!    y: PhantomData<P>,
//!    #[new(default)]
//!    z: T,
//!}
//!
//!fn main() {
//!  let _ = Generic::<i32, u8>::new("Hello");
//!}
//!```
//!
//!For enums, one constructor method is generated for each variant, with the type
//!name being converted to snake case; otherwise, all features supported for
//!structs work for enum variants as well:
//!
//!```rust
//!#[macro_use]
//!extern crate derive_new;
//!#[derive(new)]
//!enum Enum {
//!    FirstVariant,
//!    SecondVariant(bool, #[new(default)] u8),
//!    ThirdVariant { x: i32, #[new(value = "vec![1]")] y: Vec<u8> }
//!}
//!
//!fn main() {
//!  let _ = Enum::new_first_variant();
//!  let _ = Enum::new_second_variant(true);
//!  let _ = Enum::new_third_variant(42);
//!}
//!```
#![crate_type = "proc-macro"]
#![recursion_limit = "192"]

extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate quote;
extern crate syn;

macro_rules! my_quote {
    ($($t:tt)*) => (quote_spanned!(proc_macro2::Span::call_site() => $($t)*))
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<String>>().join("::")
}

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use syn::Token;

#[proc_macro_derive(new, attributes(new))]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).expect("Couldn't parse item");
    let result = match ast.data {
        syn::Data::Enum(ref e) => new_for_enum(&ast, e),
        syn::Data::Struct(ref s) => new_for_struct(&ast, &s.fields, None),
        syn::Data::Union(_) => panic!("doesn't work with unions yet"),
    };
    result.into()
}

fn new_for_struct(
    ast: &syn::DeriveInput,
    fields: &syn::Fields,
    variant: Option<&syn::Ident>,
) -> proc_macro2::TokenStream {
    match *fields {
        syn::Fields::Named(ref fields) => new_impl(&ast, Some(&fields.named), true, variant),
        syn::Fields::Unit => new_impl(&ast, None, false, variant),
        syn::Fields::Unnamed(ref fields) => new_impl(&ast, Some(&fields.unnamed), false, variant),
    }
}

fn new_for_enum(ast: &syn::DeriveInput, data: &syn::DataEnum) -> proc_macro2::TokenStream {
    if data.variants.is_empty() {
        panic!("#[derive(new)] cannot be implemented for enums with zero variants");
    }
    let impls = data.variants.iter().map(|v| {
        if v.discriminant.is_some() {
            panic!("#[derive(new)] cannot be implemented for enums with discriminants");
        }
        new_for_struct(ast, &v.fields, Some(&v.ident))
    });
    my_quote!(#(#impls)*)
}

fn new_impl(
    ast: &syn::DeriveInput,
    fields: Option<&syn::punctuated::Punctuated<syn::Field, Token![,]>>,
    named: bool,
    variant: Option<&syn::Ident>,
) -> proc_macro2::TokenStream {
    let name = &ast.ident;
    let unit = fields.is_none();
    let empty = Default::default();
    let fields: Vec<_> = fields
        .unwrap_or(&empty)
        .iter()
        .enumerate()
        .map(|(i, f)| FieldExt::new(f, i, named))
        .collect();
    let args = fields.iter().filter(|f| f.needs_arg()).map(|f| f.as_arg());
    let inits = fields.iter().map(|f| f.as_init());
    let inits = if unit {
        my_quote!()
    } else if named {
        my_quote![{ #(#inits),* }]
    } else {
        my_quote![( #(#inits),* )]
    };
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let (mut new, qual, doc) = match variant {
        None => (
            syn::Ident::new("new", proc_macro2::Span::call_site()),
            my_quote!(),
            format!("Constructs a new `{}`.", name),
        ),
        Some(ref variant) => (
            syn::Ident::new(
                &format!("new_{}", to_snake_case(&variant.to_string())),
                proc_macro2::Span::call_site(),
            ),
            my_quote!(::#variant),
            format!("Constructs a new `{}::{}`.", name, variant),
        ),
    };
    new.set_span(proc_macro2::Span::call_site());
    let lint_attrs = collect_parent_lint_attrs(&ast.attrs);
    let lint_attrs = my_quote![#(#lint_attrs),*];
    my_quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #[doc = #doc]
            #lint_attrs
            pub fn #new(#(#args),*) -> Self {
                #name #qual #inits
            }
        }
    }
}

fn collect_parent_lint_attrs(attrs: &[syn::Attribute]) -> Vec<syn::Attribute> {
    fn is_lint(item: &syn::Meta) -> bool {
        if let syn::Meta::List(ref l) = *item {
            let path = &l.path;
            return path.is_ident("allow") || path.is_ident("deny") || path.is_ident("forbid") || path.is_ident("warn")
        }
        false
    }

    fn is_cfg_attr_lint(item: &syn::Meta) -> bool {
        if let syn::Meta::List(ref l) = *item {
            if l.path.is_ident("cfg_attr") && l.nested.len() == 2 {
                if let syn::NestedMeta::Meta(ref item) = l.nested[1] {
                    return is_lint(item);
                }
            }
        }
        false
    }

    attrs
        .iter()
        .filter_map(|a| a.parse_meta().ok().map(|m| (m, a)))
        .filter(|&(ref m, _)| is_lint(m) || is_cfg_attr_lint(m))
        .map(|p| p.1)
        .cloned()
        .collect()
}

enum FieldAttr {
    Default,
    Value(proc_macro2::TokenStream),
}

impl FieldAttr {
    pub fn as_tokens(&self) -> proc_macro2::TokenStream {
        match *self {
            FieldAttr::Default => {
                if cfg!(feature = "std") {
                    my_quote!(::std::default::Default::default())
                } else {
                    my_quote!(::core::default::Default::default())
                }
            }
            FieldAttr::Value(ref s) => my_quote!(#s),
        }
    }

    pub fn parse(attrs: &[syn::Attribute]) -> Option<FieldAttr> {
        use syn::{AttrStyle, Meta, NestedMeta};

        let mut result = None;
        for attr in attrs.iter() {
            match attr.style {
                AttrStyle::Outer => {}
                _ => continue,
            }
            let last_attr_path = attr
                .path
                .segments
                .iter()
                .last()
                .expect("Expected at least one segment where #[segment[::segment*](..)]");
            if (*last_attr_path).ident != "new" {
                continue;
            }
            let meta = match attr.parse_meta() {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            let list = match meta {
                Meta::List(l) => l,
                _ if meta.path().is_ident("new") => {
                    panic!("Invalid #[new] attribute, expected #[new(..)]")
                }
                _ => continue,
            };
            if result.is_some() {
                panic!("Expected at most one #[new] attribute");
            }
            for item in list.nested.iter() {
                match *item {
                    NestedMeta::Meta(Meta::Path(ref path)) => {
                        if path.is_ident("default") {
                            result = Some(FieldAttr::Default);
                        } else {
                            panic!("Invalid #[new] attribute: #[new({})]", path_to_string(&path));
                        }
                    }
                    NestedMeta::Meta(Meta::NameValue(ref kv)) => {
                        if let syn::Lit::Str(ref s) = kv.lit {
                            if kv.path.is_ident("value") {
                                let tokens = lit_str_to_token_stream(s).ok().expect(&format!(
                                    "Invalid expression in #[new]: `{}`",
                                    s.value()
                                ));
                                result = Some(FieldAttr::Value(tokens));
                            } else {
                                panic!("Invalid #[new] attribute: #[new({} = ..)]", path_to_string(&kv.path));
                            }
                        } else {
                            panic!("Non-string literal value in #[new] attribute");
                        }
                    }
                    NestedMeta::Meta(Meta::List(ref l)) => {
                        panic!("Invalid #[new] attribute: #[new({}(..))]", path_to_string(&l.path));
                    }
                    NestedMeta::Lit(_) => {
                        panic!("Invalid #[new] attribute: literal value in #[new(..)]");
                    }
                }
            }
        }
        result
    }
}

struct FieldExt<'a> {
    ty: &'a syn::Type,
    attr: Option<FieldAttr>,
    ident: syn::Ident,
    named: bool,
}

impl<'a> FieldExt<'a> {
    pub fn new(field: &'a syn::Field, idx: usize, named: bool) -> FieldExt<'a> {
        FieldExt {
            ty: &field.ty,
            attr: FieldAttr::parse(&field.attrs),
            ident: if named {
                field.ident.clone().unwrap()
            } else {
                syn::Ident::new(&format!("f{}", idx), proc_macro2::Span::call_site())
            },
            named: named,
        }
    }

    pub fn has_attr(&self) -> bool {
        self.attr.is_some()
    }

    pub fn is_phantom_data(&self) -> bool {
        match *self.ty {
            syn::Type::Path(syn::TypePath {
                qself: None,
                ref path,
            }) => path
                .segments
                .last()
                .map(|x| x.ident == "PhantomData")
                .unwrap_or(false),
            _ => false,
        }
    }

    pub fn needs_arg(&self) -> bool {
        !self.has_attr() && !self.is_phantom_data()
    }

    pub fn as_arg(&self) -> proc_macro2::TokenStream {
        let f_name = &self.ident;
        let ty = &self.ty;
        my_quote!(#f_name: #ty)
    }

    pub fn as_init(&self) -> proc_macro2::TokenStream {
        let f_name = &self.ident;
        let init = if self.is_phantom_data() {
            if cfg!(feature = "std") {
                my_quote!(::std::marker::PhantomData)
            } else {
                my_quote!(::core::marker::PhantomData)
            }
        } else {
            match self.attr {
                None => my_quote!(#f_name),
                Some(ref attr) => attr.as_tokens(),
            }
        };
        if self.named {
            my_quote!(#f_name: #init)
        } else {
            my_quote!(#init)
        }
    }
}

fn lit_str_to_token_stream(s: &syn::LitStr) -> Result<TokenStream2, proc_macro2::LexError> {
    let code = s.value();
    let ts: TokenStream2 = code.parse()?;
    Ok(set_ts_span_recursive(ts, &s.span()))
}

fn set_ts_span_recursive(ts: TokenStream2, span: &proc_macro2::Span) -> TokenStream2 {
    ts.into_iter().map(|mut tt| {
        tt.set_span(span.clone());
        if let proc_macro2::TokenTree::Group(group) = &mut tt {
            let stream = set_ts_span_recursive(group.stream(), span);
            *group = proc_macro2::Group::new(group.delimiter(), stream);
        }
        tt
    }).collect()
}

fn to_snake_case(s: &str) -> String {
    let (ch, next, mut acc): (Option<char>, Option<char>, String) =
        s.chars()
            .fold((None, None, String::new()), |(prev, ch, mut acc), next| {
                if let Some(ch) = ch {
                    if let Some(prev) = prev {
                        if ch.is_uppercase() {
                            if prev.is_lowercase()
                                || prev.is_numeric()
                                || (prev.is_uppercase() && next.is_lowercase())
                            {
                                acc.push('_');
                            }
                        }
                    }
                    acc.extend(ch.to_lowercase());
                }
                (ch, Some(next), acc)
            });
    if let Some(next) = next {
        if let Some(ch) = ch {
            if (ch.is_lowercase() || ch.is_numeric()) && next.is_uppercase() {
                acc.push('_');
            }
        }
        acc.extend(next.to_lowercase());
    }
    acc
}

#[test]
fn test_to_snake_case() {
    assert_eq!(to_snake_case(""), "");
    assert_eq!(to_snake_case("a"), "a");
    assert_eq!(to_snake_case("B"), "b");
    assert_eq!(to_snake_case("BC"), "bc");
    assert_eq!(to_snake_case("Bc"), "bc");
    assert_eq!(to_snake_case("bC"), "b_c");
    assert_eq!(to_snake_case("Fred"), "fred");
    assert_eq!(to_snake_case("CARGO"), "cargo");
    assert_eq!(to_snake_case("_Hello"), "_hello");
    assert_eq!(to_snake_case("QuxBaz"), "qux_baz");
    assert_eq!(to_snake_case("FreeBSD"), "free_bsd");
    assert_eq!(to_snake_case("specialK"), "special_k");
    assert_eq!(to_snake_case("hello1World"), "hello1_world");
    assert_eq!(to_snake_case("Keep_underscore"), "keep_underscore");
    assert_eq!(to_snake_case("ThisISNotADrill"), "this_is_not_a_drill");
}
