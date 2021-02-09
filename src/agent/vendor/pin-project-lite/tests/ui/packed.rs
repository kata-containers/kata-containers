use pin_project_lite::pin_project;

pin_project! { //~ ERROR borrow of packed field is unsafe and requires unsafe function or block
    #[repr(packed, C)]
    struct A {
        #[pin]
        field: u16,
    }
}

pin_project! { //~ ERROR borrow of packed field is unsafe and requires unsafe function or block
    #[repr(packed(2))]
    struct C {
        #[pin]
        field: u32,
    }
}

fn main() {}
