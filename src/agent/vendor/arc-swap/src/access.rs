//! Abstracting over accessing parts of stored value.
//!
//! Sometimes, there's a big globalish data structure (like a configuration for the whole program).
//! Then there are parts of the program that need access to up-to-date version of their *part* of
//! the configuration, but for reasons of code separation and reusability, it is not desirable to
//! pass the whole configuration to each of the parts.
//!
//! This module provides means to grant the parts access to the relevant subsets of such global
//! data structure while masking the fact it is part of the bigger whole from the component.
//!
//! Note that the [`cache`][::cache] module has its own [`Access`][::cache::Access] trait that
//! serves a similar purpose, but with cached access. The signatures are different, therefore an
//! incompatible trait.
//!
//! # The general idea
//!
//! Each part of the code accepts generic [`Access<T>`][Access] for the `T` of its interest. This
//! provides means to load current version of the structure behind the scenes and get only the
//! relevant part, without knowing what the big structure is.
//!
//! For technical reasons, the [`Access`] trait is not object safe. If type erasure is desired, it
//! is possible use the [`DynAccess`][::access::DynAccess] instead, which is object safe, but
//! slightly slower.
//!
//! For some cases, it is possible to use [`ArcSwapAny::map`]. If that is not flexible enough, the
//! [`Map`] type can be created directly.
//!
//! Note that the [`Access`] trait is also implemented for [`ArcSwapAny`] itself. Additionally,
//! there's the [`Constant`][::access::Constant] helper type, which is useful mostly for testing
//! (it doesn't allow reloading).
//!
//! # Performance
//!
//! In general, these utilities use [`ArcSwapAny::load`] internally and then apply the provided
//! transformation. This has several consequences:
//!
//! * Limitations of the [`load`][ArcSwapAny::load] apply ‒ including the recommendation to not
//!   hold the returned guard object for too long, but long enough to get consistency.
//! * The transformation should be cheap ‒ optimally just borrowing into the structure.
//!
//! # Examples
//!
//! ```rust
//! extern crate arc_swap;
//!
//! use std::sync::Arc;
//! use std::thread;
//! use std::time::Duration;
//!
//! use arc_swap::ArcSwap;
//! use arc_swap::access::{Access, Constant, Map};
//!
//! fn work_with_usize<A: Access<usize> + Send + 'static>(a: A) {
//!     thread::spawn(move || {
//!         loop {
//!             let value = a.load();
//!             println!("{}", *value);
//!             // Not strictly necessary, but dropping the guard can free some resources, like
//!             // slots for tracking what values are still in use. We do it before the sleeping,
//!             // not at the end of the scope.
//!             drop(value);
//!             thread::sleep(Duration::from_millis(50));
//!         }
//!     });
//! }
//!
//! // Passing the whole thing directly
//! // (If we kept another Arc to it, we could change the value behind the scenes)
//! work_with_usize(Arc::new(ArcSwap::from_pointee(42)));
//!
//! // Passing a subset of a structure
//! struct Cfg {
//!     value: usize,
//! }
//!
//! let cfg = Arc::new(ArcSwap::from_pointee(Cfg { value: 0 }));
//! work_with_usize(Map::new(Arc::clone(&cfg), |cfg: &Cfg| &cfg.value));
//! cfg.store(Arc::new(Cfg { value: 42 }));
//!
//! // Passing a constant that can't change. Useful mostly for testing purposes.
//! work_with_usize(Constant(42));
//! ```

use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

use super::gen_lock::LockStorage;
use super::ref_cnt::RefCnt;
use super::{ArcSwapAny, Guard};

/// Abstracts over ways code can get access to a value of type `T`.
///
/// This is the trait that parts of code will use when accessing a subpart of the big data
/// structure. See the [module documentation](index.html) for details.
pub trait Access<T> {
    /// A guard object containing the value and keeping it alive.
    ///
    /// For technical reasons, the library doesn't allow direct access into the stored value. A
    /// temporary guard object must be loaded, that keeps the actual value alive for the time of
    /// use.
    type Guard: Deref<Target = T>;

    /// The loading method.
    ///
    /// This returns the guard that holds the actual value. Should be called anew each time a fresh
    /// value is needed.
    fn load(&self) -> Self::Guard;
}

impl<T, A: Access<T>, P: Deref<Target = A>> Access<T> for P {
    type Guard = A::Guard;
    fn load(&self) -> Self::Guard {
        self.deref().load()
    }
}

impl<T: RefCnt, S: LockStorage> Access<T> for ArcSwapAny<T, S> {
    type Guard = Guard<'static, T>;

    fn load(&self) -> Self::Guard {
        self.load()
    }
}

/// Plumbing type.
///
/// Accessible, but not expected to be used directly in general.
#[derive(Debug)]
pub struct DirectDeref<T: RefCnt>(Guard<'static, T>);

impl<T> Deref for DirectDeref<Arc<T>> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0.deref().deref()
    }
}

impl<T, S: LockStorage> Access<T> for ArcSwapAny<Arc<T>, S> {
    type Guard = DirectDeref<Arc<T>>;
    fn load(&self) -> Self::Guard {
        DirectDeref(self.load())
    }
}

impl<T> Deref for DirectDeref<Rc<T>> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0.deref().deref()
    }
}

impl<T, S: LockStorage> Access<T> for ArcSwapAny<Rc<T>, S> {
    type Guard = DirectDeref<Rc<T>>;
    fn load(&self) -> Self::Guard {
        DirectDeref(self.load())
    }
}

/// Plumbing type.
///
/// This is the guard of [`DynAccess`] trait. It is effectively `Box<Deref<Target = T>>`.
pub struct DynGuard<T: ?Sized>(Box<Deref<Target = T>>);

impl<T: ?Sized> Deref for DynGuard<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

/// An object-safe version of the [`Access`] trait.
///
/// This can be used instead of the [`Access`] trait in case a type erasure is desired. This has
/// the effect of performance hit (due to boxing of the result and due to dynamic dispatch), but
/// makes certain code simpler and possibly makes the executable smaller.
///
/// This is automatically implemented for everything that implements [`Access`].
///
/// # Examples
///
/// ```rust
/// extern crate arc_swap;
///
/// use std::thread;
///
/// use arc_swap::access::{Constant, DynAccess};
///
/// fn do_something(value: Box<dyn DynAccess<usize> + Send>) {
///     thread::spawn(move || {
///         let v = value.load();
///         println!("{}", *v);
///     });
/// }
///
/// do_something(Box::new(Constant(42)));
/// ```
pub trait DynAccess<T> {
    /// The equivalent of [`Access::load`].
    fn load(&self) -> DynGuard<T>;
}

impl<T, A> DynAccess<T> for A
where
    A: Access<T>,
    A::Guard: 'static,
{
    fn load(&self) -> DynGuard<T> {
        DynGuard(Box::new(Access::load(self)))
    }
}

/// A plumbing type.
///
/// This is the guard type for [`Map`]. It is accessible and nameable, but is not expected to be
/// generally used directly.
#[derive(Copy, Clone, Debug)]
pub struct MapGuard<G, T> {
    _guard: G,
    value: *const T,
}

// Why these are safe:
// * The *const T is actually used just as a &const T with 'self lifetime (which can't be done in
//   Rust). So if the reference is Send/Sync, so is the raw pointer.
unsafe impl<G, T> Send for MapGuard<G, T>
where
    G: Send,
    for<'a> &'a T: Send,
{
}

unsafe impl<G, T> Sync for MapGuard<G, T>
where
    G: Sync,
    for<'a> &'a T: Sync,
{
}

impl<G, T> Deref for MapGuard<G, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // Why this is safe:
        // * The pointer is originally converted from a reference. It's not null, it's aligned,
        //   it's the right type, etc.
        // * The pointee couldn't have gone away ‒ the guard keeps the original reference alive, so
        //   must the new one still be alive too. Moving the guard is fine, we assume the RefCnt is
        //   Pin (because it's Arc or Rc or something like that ‒ when that one moves, the data it
        //   points to stay at the same place).
        unsafe { &*self.value }
    }
}

/// An adaptor to provide access to a part of larger structure.
///
/// This is the *active* part of this module. Use the [module documentation](index.html) for the
/// details.
#[derive(Copy, Clone, Debug)]
pub struct Map<A, T, F> {
    access: A,
    projection: F,
    _t: PhantomData<fn() -> T>,
}

impl<A, T, F> Map<A, T, F> {
    /// Creates a new instance.
    ///
    /// # Parameters
    ///
    /// * `access`: Access to the bigger structure. This is usually something like `Arc<ArcSwap>`
    ///   or `&ArcSwap`. It is technically possible to use any other [`Access`] here, though, for
    ///   example to sub-delegate into even smaller structure from a [`Map`] (or generic
    ///   [`Access`]).
    /// * `projection`: A function (or closure) responsible to providing a reference into the
    ///   bigger bigger structure, selecting just subset of it. In general, it is expected to be
    ///   *cheap* (like only taking reference).
    pub fn new<R>(access: A, projection: F) -> Self
    where
        F: Fn(&T) -> &R,
    {
        Map {
            access,
            projection,
            _t: PhantomData,
        }
    }
}

impl<A, T, F, R> Access<R> for Map<A, T, F>
where
    A: Access<T>,
    F: Fn(&T) -> &R,
{
    type Guard = MapGuard<A::Guard, R>;
    fn load(&self) -> Self::Guard {
        let guard = self.access.load();
        let value: *const _ = (self.projection)(&guard);
        MapGuard {
            _guard: guard,
            value,
        }
    }
}

/// A plumbing type.
///
/// This is the guard type for [`Constant`]. It is accessible, but is not expected to be generally
/// used directly.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ConstantDeref<T>(T);

impl<T> Deref for ConstantDeref<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

/// Access to an constant.
///
/// This wraps a constant value to provide [`Access`] to it. It is constant in the sense that,
/// unlike [`ArcSwapAny`] and [`Map`], the loaded value will always stay the same (there's no
/// remote `store`).
///
/// The purpose is mostly testing and plugging a parameter that works generically from code that
/// doesn't need the updating functionality.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Constant<T>(pub T);

impl<T: Clone> Access<T> for Constant<T> {
    type Guard = ConstantDeref<T>;
    fn load(&self) -> Self::Guard {
        ConstantDeref(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ArcSwap, ArcSwapOption};

    use super::*;

    fn check_static_dispatch_direct<A: Access<usize>>(a: A) {
        assert_eq!(42, *a.load());
    }

    fn check_static_dispatch<A: Access<Arc<usize>>>(a: A) {
        assert_eq!(42, **a.load());
    }

    /// Tests dispatching statically from arc-swap works
    #[test]
    fn static_dispatch() {
        let a = ArcSwap::from_pointee(42);
        check_static_dispatch_direct(&a);
        check_static_dispatch(&a);
        check_static_dispatch(a);
    }

    fn check_dyn_dispatch_direct(a: &DynAccess<usize>) {
        assert_eq!(42, *a.load());
    }

    fn check_dyn_dispatch(a: &DynAccess<Arc<usize>>) {
        assert_eq!(42, **a.load());
    }

    /// Tests we can also do a dynamic dispatch of the companion trait
    #[test]
    fn dyn_dispatch() {
        let a = ArcSwap::from_pointee(42);
        check_dyn_dispatch_direct(&a);
        check_dyn_dispatch(&a);
    }

    fn check_transition<A>(a: A)
    where
        A: Access<usize>,
        A::Guard: 'static,
    {
        check_dyn_dispatch_direct(&a)
    }

    /// Tests we can easily transition from the static dispatch trait to the dynamic one
    #[test]
    fn transition() {
        let a = ArcSwap::from_pointee(42);
        check_transition(&a);
        check_transition(a);
    }

    /// Test we can dispatch from Arc<ArcSwap<_>> or similar.
    #[test]
    fn indirect() {
        let a = Arc::new(ArcSwap::from_pointee(42));
        check_static_dispatch(&a);
        check_dyn_dispatch(&a);
    }

    struct Cfg {
        value: usize,
    }

    #[test]
    fn map() {
        let a = ArcSwap::from_pointee(Cfg { value: 42 });
        let map = a.map(|a: &Cfg| &a.value);
        check_static_dispatch_direct(&map);
        check_dyn_dispatch_direct(&map);
    }

    #[test]
    fn map_option_some() {
        let a = ArcSwapOption::from_pointee(Cfg { value: 42 });
        let map = a.map(|a: &Option<Arc<Cfg>>| a.as_ref().map(|c| &c.value).unwrap());
        check_static_dispatch_direct(&map);
        check_dyn_dispatch_direct(&map);
    }

    #[test]
    fn map_option_none() {
        let a = ArcSwapOption::empty();
        let map = a.map(|a: &Option<Arc<Cfg>>| a.as_ref().map(|c| &c.value).unwrap_or(&42));
        check_static_dispatch_direct(&map);
        check_dyn_dispatch_direct(&map);
    }

    #[test]
    fn constant() {
        let c = Constant(42);
        check_static_dispatch_direct(&c);
        check_dyn_dispatch_direct(&c);
        check_static_dispatch_direct(c);
    }

    #[test]
    fn map_reload() {
        let a = ArcSwap::from_pointee(Cfg { value: 0 });
        let map = a.map(|cfg: &Cfg| &cfg.value);
        assert_eq!(0, *Access::load(&map));
        a.store(Arc::new(Cfg { value: 42 }));
        assert_eq!(42, *Access::load(&map));
    }
}
