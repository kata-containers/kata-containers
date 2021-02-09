# A custom derive implementation for `#[derive(new)]`

A `derive(new)` attribute creates a `new` constructor function for the annotated
type. That function takes an argument for each field in the type giving a
trivial constructor. This is useful since as your type evolves you can make the
constructor non-trivial (and add or remove fields) without changing client code
(i.e., without breaking backwards compatibility). It is also the most succinct
way to initialise a struct or an enum.

Implementation uses macros 1.1 custom derive (which works in stable Rust from
1.15 onwards).

`#[no_std]` is fully supported if you switch off the default feature `"std"`.

## Examples

Cargo.toml:

```toml
[dependencies]
derive-new = "0.5"
```

Include the macro:

```rust
#[macro_use]
extern crate derive_new;
```

Generating constructor for a simple struct:

```rust
#[derive(new)]
struct Bar {
    a: i32,
    b: String,
}

let _ = Bar::new(42, "Hello".to_owned());
```

Default values can be specified either via `#[new(default)]` attribute which removes
the argument from the constructor and populates the field with `Default::default()`,
or via `#[new(value = "..")]` which initializes the field with a given expression:

```rust
#[derive(new)]
struct Foo {
    x: bool,
    #[new(value = "42")]
    y: i32,
    #[new(default)]
    z: Vec<String>,
}

let _ = Foo::new(true);
```

Generic types are supported; in particular, `PhantomData<T>` fields will be not
included in the argument list and will be intialized automatically:

```rust
use std::marker::PhantomData;

#[derive(new)]
struct Generic<'a, T: Default, P> {
    x: &'a str,
    y: PhantomData<P>,
    #[new(default)]
    z: T,
}

let _ = Generic::<i32, u8>::new("Hello");
```

For enums, one constructor method is generated for each variant, with the type
name being converted to snake case; otherwise, all features supported for
structs work for enum variants as well:

```rust
#[derive(new)]
struct Enum {
    FirstVariant,
    SecondVariant(bool, #[new(default)] u8),
    ThirdVariant { x: i32, #[new(value = "vec![1]")] y: Vec<u8> }
}

let _ = Enum::new_first_variant();
let _ = Enum::new_second_variant(true);
let _ = Enum::new_third_variant(42);
```
