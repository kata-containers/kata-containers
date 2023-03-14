//! This module provides a scoped API, allowing for taking an arbitrary number of `&mut T` into `T` within one closure.
//! The references are all required to outlive the closure.
//!
//! # Example
//! ```
//! use take_mut::scoped;
//! struct Foo;
//! let mut foo = Foo; // Must outlive scope
//! scoped::scope(|scope| {
//!     let (t, hole) = scope.take(&mut foo);
//!     drop(t);
//!     hole.fill(Foo); // If not called before the closure ends, causes an abort.
//! });
//! ```
//!
//! # Invalid Example (does not compile)
//! ```ignore
//! use take_mut::scoped;
//! struct Foo;
//! scoped::scope(|scope| {
//!     let mut foo = Foo; // Invalid because foo must come from outside the scope.
//!     let (t, hole) = scope.take(&mut foo);
//!     drop(t);
//!     hole.fill(Foo);
//! });
//! ```
//! 
//! `Scope` also offers `take_or_recover`, which takes a function to call in the event the hole isn't filled.

#![warn(missing_docs)]


use std;
use std::panic;
use std::cell::Cell;
use std::marker::PhantomData;

/// Represents a scope within which, it is possible to take a `T` from a `&mut T` as long as the `&mut T` outlives the scope.
pub struct Scope<'s> {
    active_holes: Cell<usize>,
    marker: PhantomData<Cell<&'s mut ()>>
}

impl<'s> Scope<'s> {

    /// Takes a `(T, Hole<'c, 'm, T, F>)` from an `&'m mut T`.
    ///
    /// If the `Hole` is dropped without being filled, either due to panic or forgetting to fill, will run the `recovery` function to obtain a `T` to fill itself with.
    pub fn take_or_recover<'c, 'm: 's, T: 'm, F: FnOnce() -> T>(&'c self, mut_ref: &'m mut T, recovery: F) -> (T, Hole<'c, 'm, T, F>) {
        use std::ptr;
        
        let t: T;
        let hole: Hole<'c, 'm, T, F>;
        let num_of_holes = self.active_holes.get();
        if num_of_holes == std::usize::MAX {
            panic!("Too many holes!");
        }
        self.active_holes.set(num_of_holes + 1);
        unsafe {
            t = ptr::read(mut_ref as *mut T);
            hole = Hole {
                active_holes: &self.active_holes,
                hole: mut_ref as *mut T,
                phantom: PhantomData,
                recovery: Some(recovery)
            };
        };
        (t, hole)
    }
    
    /// Takes a `(T, Hole<'c, 'm, T, F>)` from an `&'m mut T`.
    pub fn take<'c, 'm: 's, T: 'm>(&'c self, mut_ref: &'m mut T) -> (T, Hole<'c, 'm, T, fn() -> T>) {
        #[allow(missing_docs)]
        fn panic<T>() -> T {
            panic!("Failed to recover a Hole!")
        }
        self.take_or_recover(mut_ref, panic)
    }
}

/// Main function to create a `Scope`.
///
/// If the given closure ends without all Holes filled, will abort the program.
pub fn scope<'s, F, R>(f: F) -> R
    where F: FnOnce(&Scope<'s>) -> R {
    let this = Scope { active_holes: Cell::new(0), marker: PhantomData };
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        f(&this)
    }));
    if this.active_holes.get() != 0 {
        std::process::abort();
    }
    match result {
        Ok(r) => r,
        Err(p) => panic::resume_unwind(p),
    }
    
}

/// A `Hole<'c, 'm, T, F>` represents an unfilled `&'m mut T` which must be filled before the end of the `Scope` with lifetime `'c` and recovery closure `F`.
///
/// An unfilled `Hole<'c, 'm, T, F> that is destructed will try to use `F` to fill the hole.
///
/// If the scope ends without the `Hole` being filled, the program will `std::process::abort()`.
#[must_use]
pub struct Hole<'c, 'm, T: 'm, F: FnOnce() -> T> {
    active_holes: &'c Cell<usize>,
    hole: *mut T,
    phantom: PhantomData<&'m mut T>,
    recovery: Option<F>,
}

impl<'c, 'm, T: 'm, F: FnOnce() -> T> Hole<'c, 'm, T, F> {
    /// Fills the Hole.
    pub fn fill(self, t: T) {
        use std::ptr;
        use std::mem;
        
        unsafe {
            ptr::write(self.hole, t);
        }
        let num_holes = self.active_holes.get();
        self.active_holes.set(num_holes - 1);
        mem::forget(self);
    }
}

impl<'c, 'm, T: 'm, F: FnOnce() -> T> Drop for Hole<'c, 'm, T, F> {
    fn drop(&mut self) {
        use std::ptr;
        
        let t = (self.recovery.take().expect("No recovery function in Hole!"))();
        unsafe {
            ptr::write(self.hole, t);
        }
        let num_holes = self.active_holes.get();
        self.active_holes.set(num_holes - 1);
    }
}

#[test]
fn scope_based_take() {
    #[derive(Debug)]
    struct Foo;
    
    #[derive(Debug)]
    struct Bar {
        a: Foo,
        b: Foo
    }
    let mut bar = Bar { a: Foo, b: Foo };
    scope(|scope| {
        let (a, a_hole) = scope.take(&mut bar.a);
        let (b, b_hole) = scope.take(&mut bar.b);
        // Imagine consuming a and b
        a_hole.fill(Foo);
        b_hole.fill(Foo);
    });
    println!("{:?}", &bar);
}

#[test]
fn panic_on_recovered_panic() {
    use std::panic;
    
    struct Foo;
    let mut foo = Foo;
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        scope(|scope| {
            let (t, hole) = scope.take_or_recover(&mut foo, || Foo);
            panic!("Oops!");
        });
    }));
    assert!(result.is_err());
}