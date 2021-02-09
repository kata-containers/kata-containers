#![warn(rust_2018_idioms, single_use_lifetimes)]
#![allow(dead_code)]

#[macro_use]
mod auxiliary;

use core::{marker::PhantomPinned, pin::Pin};
use pin_project_lite::pin_project;

#[test]
fn projection() {
    pin_project! {
        struct Struct<T, U> {
            #[pin]
            f1: T,
            f2: U,
        }
    }

    let mut s = Struct { f1: 1, f2: 2 };
    let mut s_orig = Pin::new(&mut s);
    let s = s_orig.as_mut().project();

    let x: Pin<&mut i32> = s.f1;
    assert_eq!(*x, 1);

    let y: &mut i32 = s.f2;
    assert_eq!(*y, 2);

    assert_eq!(s_orig.as_ref().f1, 1);
    assert_eq!(s_orig.as_ref().f2, 2);

    let mut s = Struct { f1: 1, f2: 2 };

    let s = Pin::new(&mut s).project();

    let _: Pin<&mut i32> = s.f1;
    let _: &mut i32 = s.f2;
}

#[test]
fn where_clause() {
    pin_project! {
        struct Struct<T>
        where
            T: Copy,
        {
            f: T,
        }
    }
}

#[test]
fn where_clause_and_associated_type_field() {
    pin_project! {
        struct Struct1<I>
        where
            I: Iterator,
        {
            #[pin]
            f1: I,
            f2: I::Item,
        }
    }

    pin_project! {
        struct Struct2<I, J>
        where
            I: Iterator<Item = J>,
        {
            #[pin]
            f1: I,
            f2: J,
        }
    }

    pin_project! {
        pub struct Struct3<T>
        where
            T: 'static,
        {
            f: T,
        }
    }

    trait Static: 'static {}

    impl<T> Static for Struct3<T> {}
}

#[test]
fn derive_copy() {
    pin_project! {
        #[derive(Clone, Copy)]
        struct Struct<T> {
            f: T,
        }
    }

    fn is_copy<T: Copy>() {}

    is_copy::<Struct<u8>>();
}

#[test]
fn move_out() {
    struct NotCopy;

    pin_project! {
        struct Struct {
            f: NotCopy,
        }
    }

    let x = Struct { f: NotCopy };
    let _val: NotCopy = x.f;
}

#[test]
fn trait_bounds_on_type_generics() {
    pin_project! {
        pub struct Struct1<'a, T: ?Sized> {
            f: &'a mut T,
        }
    }

    pin_project! {
        pub struct Struct2<'a, T: ::core::fmt::Debug> {
            f: &'a mut T,
        }
    }

    pin_project! {
        pub struct Struct3<'a, T: core::fmt::Debug> {
            f: &'a mut T,
        }
    }

    // pin_project! {
    //     pub struct Struct4<'a, T: core::fmt::Debug + core::fmt::Display> {
    //         f: &'a mut T,
    //     }
    // }

    // pin_project! {
    //     pub struct Struct5<'a, T: core::fmt::Debug + ?Sized> {
    //         f: &'a mut T,
    //     }
    // }

    pin_project! {
        pub struct Struct6<'a, T: core::fmt::Debug = [u8; 16]> {
            f: &'a mut T,
        }
    }

    let _: Struct6<'_> = Struct6 { f: &mut [0u8; 16] };

    pin_project! {
        pub struct Struct7<T: 'static> {
            f: T,
        }
    }

    trait Static: 'static {}

    impl<T> Static for Struct7<T> {}

    pin_project! {
        pub struct Struct8<'a, 'b: 'a> {
            f1: &'a u8,
            f2: &'b u8,
        }
    }
}

#[test]
fn private_type_in_public_type() {
    pin_project! {
        pub struct PublicStruct<T> {
            #[pin]
            inner: PrivateStruct<T>,
        }
    }

    struct PrivateStruct<T>(T);
}

#[allow(clippy::needless_lifetimes)]
#[test]
fn lifetime_project() {
    pin_project! {
        struct Struct1<T, U> {
            #[pin]
            pinned: T,
            unpinned: U,
        }
    }

    pin_project! {
        struct Struct2<'a, T, U> {
            #[pin]
            pinned: &'a mut T,
            unpinned: U,
        }
    }

    impl<T, U> Struct1<T, U> {
        fn get_pin_ref<'a>(self: Pin<&'a Self>) -> Pin<&'a T> {
            self.project_ref().pinned
        }
        fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut T> {
            self.project().pinned
        }
        fn get_pin_ref_elided(self: Pin<&Self>) -> Pin<&T> {
            self.project_ref().pinned
        }
        fn get_pin_mut_elided(self: Pin<&mut Self>) -> Pin<&mut T> {
            self.project().pinned
        }
    }

    impl<'b, T, U> Struct2<'b, T, U> {
        fn get_pin_ref<'a>(self: Pin<&'a Self>) -> Pin<&'a &'b mut T> {
            self.project_ref().pinned
        }
        fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut &'b mut T> {
            self.project().pinned
        }
        fn get_pin_ref_elided(self: Pin<&Self>) -> Pin<&&'b mut T> {
            self.project_ref().pinned
        }
        fn get_pin_mut_elided(self: Pin<&mut Self>) -> Pin<&mut &'b mut T> {
            self.project().pinned
        }
    }
}

mod visibility {
    use pin_project_lite::pin_project;

    pin_project! {
        pub(crate) struct S {
            pub f: u8,
        }
    }
}

#[test]
fn visibility() {
    let mut x = visibility::S { f: 0 };
    let x = Pin::new(&mut x);
    let y = x.as_ref().project_ref();
    let _: &u8 = y.f;
    let y = x.project();
    let _: &mut u8 = y.f;
}

#[test]
fn trivial_bounds() {
    pin_project! {
        pub struct NoGenerics {
            #[pin]
            f: PhantomPinned,
        }
    }

    assert_not_unpin!(NoGenerics);
}

#[test]
fn dst() {
    pin_project! {
        pub struct Struct1<T: ?Sized> {
            f: T,
        }
    }

    let mut x = Struct1 { f: 0_u8 };
    let x: Pin<&mut Struct1<dyn core::fmt::Debug>> = Pin::new(&mut x as _);
    let _y: &mut (dyn core::fmt::Debug) = x.project().f;

    pin_project! {
        pub struct Struct2<T: ?Sized> {
            #[pin]
            f: T,
        }
    }

    let mut x = Struct2 { f: 0_u8 };
    let x: Pin<&mut Struct2<dyn core::fmt::Debug + Unpin>> = Pin::new(&mut x as _);
    let _y: Pin<&mut (dyn core::fmt::Debug + Unpin)> = x.project().f;

    pin_project! {
        struct Struct3<T>
        where
            T: ?Sized,
        {
            f: T,
        }
    }

    pin_project! {
        struct Struct4<T>
        where
            T: ?Sized,
        {
            #[pin]
            f: T,
        }
    }

    pin_project! {
        struct Struct11<'a, T: ?Sized, U: ?Sized> {
            f1: &'a mut T,
            f2: U,
        }
    }
}

#[test]
fn dyn_type() {
    pin_project! {
        struct Struct1 {
            f: dyn core::fmt::Debug,
        }
    }

    pin_project! {
        struct Struct2 {
            #[pin]
            f: dyn core::fmt::Debug,
        }
    }

    pin_project! {
        struct Struct3 {
            f: dyn core::fmt::Debug + Send,
        }
    }

    pin_project! {
        struct Struct4 {
            #[pin]
            f: dyn core::fmt::Debug + Send,
        }
    }
}

#[test]
fn no_infer_outlives() {
    trait Trait<X> {
        type Y;
    }

    struct Struct1<A>(A);

    impl<X, T> Trait<X> for Struct1<T> {
        type Y = Option<T>;
    }

    pin_project! {
        struct Struct2<A, B> {
            _f: <Struct1<A> as Trait<B>>::Y,
        }
    }
}

// https://github.com/taiki-e/pin-project-lite/issues/31
#[test]
fn trailing_comma() {
    pub trait T {}

    pin_project! {
        pub struct S1<
            A: T,
            B: T,
        > {
            f: (A, B),
        }
    }

    pin_project! {
        pub struct S2<
            A,
            B,
        >
        where
            A: T,
            B: T,
        {
            f: (A, B),
        }
    }

    pin_project! {
        #[allow(explicit_outlives_requirements)]
        pub struct S3<
            'a,
            A: 'a,
            B: 'a,
        > {
            f: &'a (A, B),
        }
    }

    // pin_project! {
    //     pub struct S4<
    //         'a,
    //         'b: 'a, // <-----
    //     > {
    //         f: &'a &'b (),
    //     }
    // }
}
