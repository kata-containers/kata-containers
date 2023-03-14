/*!
Getset, we're ready to go!

A procedural macro for generating the most basic getters and setters on fields.

Getters are generated as `fn field(&self) -> &type`, while setters are generated as `fn field(&mut self, val: type)`.

These macros are not intended to be used on fields which require custom logic inside of their setters and getters. Just write your own in that case!

```rust
use getset::{CopyGetters, Getters, MutGetters, Setters};

#[derive(Getters, Setters, MutGetters, CopyGetters, Default)]
pub struct Foo<T>
where
    T: Copy + Clone + Default,
{
    /// Doc comments are supported!
    /// Multiline, even.
    #[getset(get, set, get_mut)]
    private: T,

    /// Doc comments are supported!
    /// Multiline, even.
    #[getset(get_copy = "pub", set = "pub", get_mut = "pub")]
    public: T,
}

let mut foo = Foo::default();
foo.set_private(1);
(*foo.private_mut()) += 1;
assert_eq!(*foo.private(), 2);
```

You can use `cargo-expand` to generate the output. Here are the functions that the above generates (Replicate with `cargo expand --example simple`):

```rust,ignore
use getset::{Getters, MutGetters, CopyGetters, Setters};
pub struct Foo<T>
where
    T: Copy + Clone + Default,
{
    /// Doc comments are supported!
    /// Multiline, even.
    #[getset(get, get, get_mut)]
    private: T,
    /// Doc comments are supported!
    /// Multiline, even.
    #[getset(get_copy = "pub", set = "pub", get_mut = "pub")]
    public: T,
}
impl<T> Foo<T>
where
    T: Copy + Clone + Default,
{
    /// Doc comments are supported!
    /// Multiline, even.
    #[inline(always)]
    fn private(&self) -> &T {
        &self.private
    }
}
impl<T> Foo<T>
where
    T: Copy + Clone + Default,
{
    /// Doc comments are supported!
    /// Multiline, even.
    #[inline(always)]
    pub fn set_public(&mut self, val: T) -> &mut Self {
        self.public = val;
        self
    }
}
impl<T> Foo<T>
where
    T: Copy + Clone + Default,
{
    /// Doc comments are supported!
    /// Multiline, even.
    #[inline(always)]
    fn private_mut(&mut self) -> &mut T {
        &mut self.private
    }
    /// Doc comments are supported!
    /// Multiline, even.
    #[inline(always)]
    pub fn public_mut(&mut self) -> &mut T {
        &mut self.public
    }
}
impl<T> Foo<T>
where
    T: Copy + Clone + Default,
{
    /// Doc comments are supported!
    /// Multiline, even.
    #[inline(always)]
    pub fn public(&self) -> T {
        self.public
    }
}
```

Attributes can be set on struct level for all fields in struct as well. Field level attributes take
precedence.

```rust
mod submodule {
    use getset::{Getters, MutGetters, CopyGetters, Setters};
    #[derive(Getters, CopyGetters, Default)]
    #[getset(get_copy = "pub")] // By default add a pub getting for all fields.
    pub struct Foo {
        public: i32,
        #[getset(get_copy)] // Override as private
        private: i32,
    }
    fn demo() {
        let mut foo = Foo::default();
        foo.private();
    }
}

let mut foo = submodule::Foo::default();
foo.public();
```

For some purposes, it's useful to have the `get_` prefix on the getters for
either legacy of compatibility reasons. It is done with `with_prefix`.

```rust
use getset::{Getters, MutGetters, CopyGetters, Setters};

#[derive(Getters, Default)]
pub struct Foo {
    #[getset(get = "pub with_prefix")]
    field: bool,
}


let mut foo = Foo::default();
let val = foo.get_field();
```

Skipping setters and getters generation for a field when struct level attribute is used
is possible with `#[getset(skip)]`.

```rust
use getset::{CopyGetters, Setters};

#[derive(CopyGetters, Setters)]
#[getset(get_copy, set)]
pub struct Foo {
    // If the field was not skipped, the compiler would complain about moving
    // a non-copyable type in copy getter.
    #[getset(skip)]
    skipped: String,

    field1: usize,
    field2: usize,
}

impl Foo {
    // It is possible to write getters and setters manually,
    // possibly with a custom logic.
    fn skipped(&self) -> &str {
        &self.skipped
    }

    fn set_skipped(&mut self, val: &str) -> &mut Self {
        self.skipped = val.to_string();
        self
    }
}
```
*/

extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;
extern crate proc_macro2;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_error::{abort, abort_call_site, proc_macro_error, ResultExt};
use syn::{spanned::Spanned, DataStruct, DeriveInput, Meta};

mod generate;
use crate::generate::{GenMode, GenParams};

#[proc_macro_derive(Getters, attributes(get, with_prefix, getset))]
#[proc_macro_error]
pub fn getters(input: TokenStream) -> TokenStream {
    // Parse the string representation
    let ast: DeriveInput = syn::parse(input).expect_or_abort("Couldn't parse for getters");
    let params = GenParams {
        mode: GenMode::Get,
        global_attr: parse_global_attr(&ast.attrs, GenMode::Get),
    };

    // Build the impl
    let gen = produce(&ast, &params);

    // Return the generated impl
    gen.into()
}

#[proc_macro_derive(CopyGetters, attributes(get_copy, with_prefix, getset))]
#[proc_macro_error]
pub fn copy_getters(input: TokenStream) -> TokenStream {
    // Parse the string representation
    let ast: DeriveInput = syn::parse(input).expect_or_abort("Couldn't parse for getters");
    let params = GenParams {
        mode: GenMode::GetCopy,
        global_attr: parse_global_attr(&ast.attrs, GenMode::GetCopy),
    };

    // Build the impl
    let gen = produce(&ast, &params);

    // Return the generated impl
    gen.into()
}

#[proc_macro_derive(MutGetters, attributes(get_mut, getset))]
#[proc_macro_error]
pub fn mut_getters(input: TokenStream) -> TokenStream {
    // Parse the string representation
    let ast: DeriveInput = syn::parse(input).expect_or_abort("Couldn't parse for getters");
    let params = GenParams {
        mode: GenMode::GetMut,
        global_attr: parse_global_attr(&ast.attrs, GenMode::GetMut),
    };

    // Build the impl
    let gen = produce(&ast, &params);
    // Return the generated impl
    gen.into()
}

#[proc_macro_derive(Setters, attributes(set, getset))]
#[proc_macro_error]
pub fn setters(input: TokenStream) -> TokenStream {
    // Parse the string representation
    let ast: DeriveInput = syn::parse(input).expect_or_abort("Couldn't parse for setters");
    let params = GenParams {
        mode: GenMode::Set,
        global_attr: parse_global_attr(&ast.attrs, GenMode::Set),
    };

    // Build the impl
    let gen = produce(&ast, &params);

    // Return the generated impl
    gen.into()
}

fn parse_global_attr(attrs: &[syn::Attribute], mode: GenMode) -> Option<Meta> {
    attrs
        .iter()
        .filter_map(|v| parse_attr(v, mode)) // non "meta" attributes are not our concern
        .last()
}

fn parse_attr(attr: &syn::Attribute, mode: GenMode) -> Option<Meta> {
    use syn::{punctuated::Punctuated, Token};

    if attr.path.is_ident("getset") {
        let (last, skip, mut collected) = attr
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .unwrap_or_abort()
            .into_iter()
            .inspect(|meta| {
                if !(meta.path().is_ident("get")
                    || meta.path().is_ident("get_copy")
                    || meta.path().is_ident("get_mut")
                    || meta.path().is_ident("set")
                    || meta.path().is_ident("skip"))
                {
                    abort!(meta.path().span(), "unknown setter or getter")
                }
            })
            .fold(
                (None, None, Vec::new()),
                |(last, skip, mut collected), meta| {
                    if meta.path().is_ident(mode.name()) {
                        (Some(meta), skip, collected)
                    } else if meta.path().is_ident("skip") {
                        (last, Some(meta), collected)
                    } else {
                        // Store inapplicable item for potential error message
                        // if used with skip.
                        collected.push(meta);
                        (last, skip, collected)
                    }
                },
            );

        if skip.is_some() {
            // Check if there is any setter or getter used with skip, which is
            // forbidden.
            if last.is_none() && collected.is_empty() {
                skip
            } else {
                abort!(
                    last.or_else(|| collected.pop()).unwrap().path().span(),
                    "use of setters and getters with skip is invalid"
                );
            }
        } else {
            // If skip is not used, return the last occurrence of matching
            // setter/getter, if there is any.
            last
        }
    } else {
        attr.parse_meta()
            .ok()
            .filter(|meta| meta.path().is_ident(mode.name()))
    }
}

fn produce(ast: &DeriveInput, params: &GenParams) -> TokenStream2 {
    let name = &ast.ident;
    let generics = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Is it a struct?
    if let syn::Data::Struct(DataStruct { ref fields, .. }) = ast.data {
        let generated = fields.iter().map(|f| generate::implement(f, params));

        quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                #(#generated)*
            }
        }
    } else {
        // Nope. This is an Enum. We cannot handle these!
        abort_call_site!("#[derive(Getters)] is only defined for structs, not for enums!");
    }
}
