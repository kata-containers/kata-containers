//! This crate provides several functions for handling `&mut T` including `take()`.
//!
//! `take()` allows for taking `T` out of a `&mut T`, doing anything with it including consuming it, and producing another `T` to put back in the `&mut T`.
//!
//! During `take()`, if a panic occurs, the entire process will be aborted, as there's no valid `T` to put back into the `&mut T`.
//! Use `take_or_recover()` to replace the `&mut T` with a recovery value before continuing the panic.
//!
//! Contrast with `std::mem::replace()`, which allows for putting a different `T` into a `&mut T`, but requiring the new `T` to be available before being able to consume the old `T`.

use std::panic;

pub mod scoped;

/// Allows use of a value pointed to by `&mut T` as though it was owned, as long as a `T` is made available afterwards.
///
/// The closure must return a valid T.
/// # Important
/// Will abort the program if the closure panics.
///
/// # Example
/// ```
/// struct Foo;
/// let mut foo = Foo;
/// take_mut::take(&mut foo, |foo| {
///     // Can now consume the Foo, and provide a new value later
///     drop(foo);
///     // Do more stuff
///     Foo // Return new Foo from closure, which goes back into the &mut Foo
/// });
/// ```
pub fn take<T, F>(mut_ref: &mut T, closure: F)
  where F: FnOnce(T) -> T {
    use std::ptr;

    unsafe {
        let old_t = ptr::read(mut_ref);
        let new_t = panic::catch_unwind(panic::AssertUnwindSafe(|| closure(old_t)))
            .unwrap_or_else(|_| ::std::process::abort());
        ptr::write(mut_ref, new_t);
    }
}

#[test]
fn it_works() {
    #[derive(PartialEq, Eq, Debug)]
    enum Foo {A, B};
    impl Drop for Foo {
        fn drop(&mut self) {
            match *self {
                Foo::A => println!("Foo::A dropped"),
                Foo::B => println!("Foo::B dropped")
            }
        }
    }
    let mut foo = Foo::A;
    take(&mut foo, |f| {
       drop(f);
       Foo::B
    });
    assert_eq!(&foo, &Foo::B);
}


/// Allows use of a value pointed to by `&mut T` as though it was owned, as long as a `T` is made available afterwards.
///
/// The closure must return a valid T.
/// # Important
/// Will replace `&mut T` with `recover` if the closure panics, then continues the panic.
///
/// # Example
/// ```
/// struct Foo;
/// let mut foo = Foo;
/// take_mut::take_or_recover(&mut foo, || Foo, |foo| {
///     // Can now consume the Foo, and provide a new value later
///     drop(foo);
///     // Do more stuff
///     Foo // Return new Foo from closure, which goes back into the &mut Foo
/// });
/// ```
pub fn take_or_recover<T, F, R>(mut_ref: &mut T, recover: R, closure: F)
  where F: FnOnce(T) -> T, R: FnOnce() -> T {
    use std::ptr;
    unsafe {
        let old_t = ptr::read(mut_ref);
        let new_t = panic::catch_unwind(panic::AssertUnwindSafe(|| closure(old_t)));
        match new_t {
            Err(err) => {
                let r = panic::catch_unwind(panic::AssertUnwindSafe(|| recover()))
                    .unwrap_or_else(|_| ::std::process::abort());
                ptr::write(mut_ref, r);
                panic::resume_unwind(err);
            }
            Ok(new_t) => ptr::write(mut_ref, new_t),
        }
    }
}




#[test]
fn it_works_recover() {
    #[derive(PartialEq, Eq, Debug)]
    enum Foo {A, B};
    impl Drop for Foo {
        fn drop(&mut self) {
            match *self {
                Foo::A => println!("Foo::A dropped"),
                Foo::B => println!("Foo::B dropped")
            }
        }
    }
    let mut foo = Foo::A;
    take_or_recover(&mut foo, || Foo::A, |f| {
       drop(f);
       Foo::B
    });
    assert_eq!(&foo, &Foo::B);
}

#[test]
fn it_works_recover_panic() {
    #[derive(PartialEq, Eq, Debug)]
    enum Foo {A, B, C};
    impl Drop for Foo {
        fn drop(&mut self) {
            match *self {
                Foo::A => println!("Foo::A dropped"),
                Foo::B => println!("Foo::B dropped"),
                Foo::C => println!("Foo::C dropped")
            }
        }
    }
    let mut foo = Foo::A;

    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        take_or_recover(&mut foo, || Foo::C, |f| {
            drop(f);
            panic!("panic");
            Foo::B
        });
    }));

    assert!(res.is_err());
    assert_eq!(&foo, &Foo::C);
}



