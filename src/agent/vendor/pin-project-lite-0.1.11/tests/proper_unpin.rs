#![warn(rust_2018_idioms, single_use_lifetimes)]
#![allow(dead_code)]

#[macro_use]
mod auxiliary;

pub mod default {
    use pin_project_lite::pin_project;
    use std::marker::PhantomPinned;

    struct Inner<T> {
        f: T,
    }

    assert_unpin!(Inner<()>);
    assert_not_unpin!(Inner<PhantomPinned>);

    pin_project! {
        struct Foo<T, U> {
            #[pin]
            f1: Inner<T>,
            f2: U,
        }
    }

    assert_unpin!(Foo<(), ()>);
    assert_unpin!(Foo<(), PhantomPinned>);
    assert_not_unpin!(Foo<PhantomPinned, ()>);
    assert_not_unpin!(Foo<PhantomPinned, PhantomPinned>);

    pin_project! {
        struct TrivialBounds {
            #[pin]
            f: PhantomPinned,
        }
    }

    assert_not_unpin!(TrivialBounds);

    pin_project! {
        struct Bar<'a, T, U> {
            #[pin]
            f1: &'a mut Inner<T>,
            f2: U,
        }
    }

    assert_unpin!(Bar<'_, PhantomPinned, PhantomPinned>);
}
