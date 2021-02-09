use auxiliary_macro::add_pin_attr;
use pin_project::pin_project;
use std::marker::PhantomPinned;

#[pin_project]
#[add_pin_attr(struct)] //~ ERROR duplicate #[pin] attribute
struct Foo {
    #[pin]
    f: PhantomPinned,
}

#[add_pin_attr(struct)] //~ ERROR #[pin] attribute may only be used on fields of structs or variants
#[pin_project]
struct Bar {
    #[pin]
    f: PhantomPinned,
}

fn main() {}
