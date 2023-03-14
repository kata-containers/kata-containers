# getset

[![Download](https://img.shields.io/crates/d/getset)](https://crates.io/crates/getset)
[![License](https://img.shields.io/crates/l/getset)](https://github.com/Hoverbear/getset/blob/master/LICENSE)
[![Docs](https://docs.rs/getset/badge.svg)](https://docs.rs/getset/)
[![Coverage Status](https://coveralls.io/repos/github/Hoverbear/getset/badge.svg)](https://coveralls.io/github/Hoverbear/getset)

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

fn main() {
    let mut foo = Foo::default();
    foo.set_private(1);
    (*foo.private_mut()) += 1;
    assert_eq!(*foo.private(), 2);
}
```

You can use `cargo-expand` to generate the output. Here are the functions that the above generates (Replicate with `cargo expand --example simple`):

```rust
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
#[macro_use]
extern crate getset;

mod submodule {
    #[derive(Getters, CopyGetters, Default)]
    #[get_copy = "pub"] // By default add a pub getting for all fields.
    pub struct Foo {
        public: i32,
        #[get_copy] // Override as private
        private: i32,
    }
    fn demo() {
        let mut foo = Foo::default();
        foo.private();
    }
}
fn main() {
    let mut foo = submodule::Foo::default();
    foo.public();
}
```

For some purposes, it's useful to have the `get_` prefix on the getters for
either legacy of compatability reasons. It is done with `with_prefix`.

```rust
#[macro_use]
extern crate getset;

#[derive(Getters, Default)]
pub struct Foo {
    #[get = "pub with_prefix"]
    field: bool,
}

fn main() {
    let mut foo = Foo::default();
    let val = foo.get_field();
}
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
