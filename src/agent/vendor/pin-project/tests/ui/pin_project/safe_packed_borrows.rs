#![forbid(safe_packed_borrows)]

// Refs: https://github.com/rust-lang/rust/issues/46043

#[repr(packed)]
struct A {
    f: u32,
}

#[repr(packed(2))]
struct B {
    f: u32,
}

fn main() {
    let a = A { f: 1 };
    &a.f; //~ ERROR borrow of packed field is unsafe and requires unsafe function or block

    let b = B { f: 1 };
    &b.f; //~ ERROR borrow of packed field is unsafe and requires unsafe function or block
}
