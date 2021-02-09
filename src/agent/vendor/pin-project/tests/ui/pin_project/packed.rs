use pin_project::pin_project;

#[pin_project]
#[repr(packed, C)] //~ ERROR may not be used on #[repr(packed)] types
struct A {
    #[pin]
    f: u8,
}

// Test putting 'repr' before the 'pin_project' attribute
#[repr(packed, C)] //~ ERROR may not be used on #[repr(packed)] types
#[pin_project]
struct B {
    #[pin]
    f: u8,
}

#[pin_project]
#[repr(packed(2))] //~ ERROR may not be used on #[repr(packed)] types
struct C {
    #[pin]
    f: u32,
}

fn main() {}
