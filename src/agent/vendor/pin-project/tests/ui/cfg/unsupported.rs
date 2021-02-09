use pin_project::pin_project;

//~ ERROR may not be used on structs with zero fields
// span is lost.
// Refs: https://github.com/rust-lang/rust/issues/43081
#[pin_project]
struct S {
    #[cfg(any())]
    #[pin]
    f: u8,
}

fn main() {}
