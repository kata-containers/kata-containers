//! An experimental new error-handling library. Guide-style introduction
//! is available [here](https://boats.gitlab.io/failure/).
//!
//! The primary items exported by this library are:
//!
//! - `Fail`: a new trait for custom error types in Rust.
//! - `Error`: a wrapper around `Fail` types to make it easy to coalesce them
//!   at higher levels.
//!
//! As a general rule, library authors should create their own error types and
//! implement `Fail` for them, whereas application authors should primarily
//! deal with the `Error` type. There are exceptions to this rule, though, in
//! both directions, and users should do whatever seems most appropriate to
//! their situation.
//!
//! ## Backtraces
//!
//! Backtraces are disabled by default. To turn backtraces on, enable
//! the `backtrace` Cargo feature and set the `RUST_BACKTRACE` environment
//! variable to a non-zero value (this also enables backtraces for panics).
//! Use the `RUST_FAILURE_BACKTRACE` variable to enable or disable backtraces
//! for `failure` specifically.
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
#![deny(warnings)]
#![cfg_attr(feature = "small-error", feature(extern_types, allocator_api))]

macro_rules! with_std { ($($i:item)*) => ($(#[cfg(feature = "std")]$i)*) }
macro_rules! without_std { ($($i:item)*) => ($(#[cfg(not(feature = "std"))]$i)*) }

// Re-export libcore using an alias so that the macros can work without
// requiring `extern crate core` downstream.
#[doc(hidden)]
pub extern crate core as _core;

mod as_fail;
mod backtrace;
#[cfg(feature = "std")]
mod box_std;
mod compat;
mod context;
mod result_ext;

use core::any::TypeId;
use core::fmt::{Debug, Display};

pub use as_fail::AsFail;
pub use backtrace::Backtrace;
pub use compat::Compat;
pub use context::Context;
pub use result_ext::ResultExt;

#[cfg(feature = "failure_derive")]
#[allow(unused_imports)]
#[macro_use]
extern crate failure_derive;

#[cfg(feature = "failure_derive")]
#[doc(hidden)]
pub use failure_derive::*;

with_std! {
    extern crate core;

    mod sync_failure;
    pub use sync_failure::SyncFailure;

    mod error;

    use std::error::Error as StdError;

    pub use error::Error;

    /// A common result with an `Error`.
    pub type Fallible<T> = Result<T, Error>;

    mod macros;
    mod error_message;
    pub use error_message::err_msg;
}

/// The `Fail` trait.
///
/// Implementors of this trait are called 'failures'.
///
/// All error types should implement `Fail`, which provides a baseline of
/// functionality that they all share.
///
/// `Fail` has no required methods, but it does require that your type
/// implement several other traits:
///
/// - `Display`: to print a user-friendly representation of the error.
/// - `Debug`: to print a verbose, developer-focused representation of the
///   error.
/// - `Send + Sync`: Your error type is required to be safe to transfer to and
///   reference from another thread
///
/// Additionally, all failures must be `'static`. This enables downcasting.
///
/// `Fail` provides several methods with default implementations. Two of these
/// may be appropriate to override depending on the definition of your
/// particular failure: the `cause` and `backtrace` methods.
///
/// The `failure_derive` crate provides a way to derive the `Fail` trait for
/// your type. Additionally, all types that already implement
/// `std::error::Error`, and are also `Send`, `Sync`, and `'static`, implement
/// `Fail` by a blanket impl.
pub trait Fail: Display + Debug + Send + Sync + 'static {
    /// Returns the "name" of the error.
    /// 
    /// This is typically the type name. Not all errors will implement
    /// this. This method is expected to be most useful in situations
    /// where errors need to be reported to external instrumentation systems 
    /// such as crash reporters.
    fn name(&self) -> Option<&str> {
        None
    }

    /// Returns a reference to the underlying cause of this failure, if it
    /// is an error that wraps other errors.
    ///
    /// Returns `None` if this failure does not have another error as its
    /// underlying cause. By default, this returns `None`.
    ///
    /// This should **never** return a reference to `self`, but only return
    /// `Some` when it can return a **different** failure. Users may loop
    /// over the cause chain, and returning `self` would result in an infinite
    /// loop.
    fn cause(&self) -> Option<&dyn Fail> {
        None
    }

    /// Returns a reference to the `Backtrace` carried by this failure, if it
    /// carries one.
    ///
    /// Returns `None` if this failure does not carry a backtrace. By
    /// default, this returns `None`.
    fn backtrace(&self) -> Option<&Backtrace> {
        None
    }

    /// Provides context for this failure.
    ///
    /// This can provide additional information about this error, appropriate
    /// to the semantics of the current layer. That is, if you have a
    /// lower-level error, such as an IO error, you can provide additional context
    /// about what that error means in the context of your function. This
    /// gives users of this function more information about what has gone
    /// wrong.
    ///
    /// This takes any type that implements `Display`, as well as
    /// `Send`/`Sync`/`'static`. In practice, this means it can take a `String`
    /// or a string literal, or another failure, or some other custom context-carrying
    /// type.
    fn context<D>(self, context: D) -> Context<D>
    where
        D: Display + Send + Sync + 'static,
        Self: Sized,
    {
        Context::with_err(context, self)
    }

    /// Wraps this failure in a compatibility wrapper that implements
    /// `std::error::Error`.
    ///
    /// This allows failures  to be compatible with older crates that
    /// expect types that implement the `Error` trait from `std::error`.
    fn compat(self) -> Compat<Self>
    where
        Self: Sized,
    {
        Compat { error: self }
    }

    #[doc(hidden)]
    #[deprecated(since = "0.1.2", note = "please use the 'iter_chain()' method instead")]
    fn causes(&self) -> Causes
    where
        Self: Sized,
    {
        Causes { fail: Some(self) }
    }

    #[doc(hidden)]
    #[deprecated(
        since = "0.1.2",
        note = "please use the 'find_root_cause()' method instead"
    )]
    fn root_cause(&self) -> &dyn Fail
    where
        Self: Sized,
    {
        find_root_cause(self)
    }

    #[doc(hidden)]
    fn __private_get_type_id__(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

impl dyn Fail {
    /// Attempts to downcast this failure to a concrete type by reference.
    ///
    /// If the underlying error is not of type `T`, this will return `None`.
    pub fn downcast_ref<T: Fail>(&self) -> Option<&T> {
        if self.__private_get_type_id__() == TypeId::of::<T>() {
            unsafe { Some(&*(self as *const dyn Fail as *const T)) }
        } else {
            None
        }
    }

    /// Attempts to downcast this failure to a concrete type by mutable
    /// reference.
    ///
    /// If the underlying error is not of type `T`, this will return `None`.
    pub fn downcast_mut<T: Fail>(&mut self) -> Option<&mut T> {
        if self.__private_get_type_id__() == TypeId::of::<T>() {
            unsafe { Some(&mut *(self as *mut dyn Fail as *mut T)) }
        } else {
            None
        }
    }

    /// Returns the "root cause" of this `Fail` - the last value in the
    /// cause chain which does not return an underlying `cause`.
    ///
    /// If this type does not have a cause, `self` is returned, because
    /// it is its own root cause.
    ///
    /// This is equivalent to iterating over `iter_causes()` and taking
    /// the last item.
    pub fn find_root_cause(&self) -> &dyn Fail {
        find_root_cause(self)
    }

    /// Returns a iterator over the causes of this `Fail` with the cause
    /// of this fail as the first item and the `root_cause` as the final item.
    ///
    /// Use `iter_chain` to also include the fail itself.
    pub fn iter_causes(&self) -> Causes {
        Causes { fail: self.cause() }
    }

    /// Returns a iterator over all fails up the chain from the current
    /// as the first item up to the `root_cause` as the final item.
    ///
    /// This means that the chain also includes the fail itself which
    /// means that it does *not* start with `cause`.  To skip the outermost
    /// fail use `iter_causes` instead.
    pub fn iter_chain(&self) -> Causes {
        Causes { fail: Some(self) }
    }

    /// Deprecated alias to `find_root_cause`.
    #[deprecated(
        since = "0.1.2",
        note = "please use the 'find_root_cause()' method instead"
    )]
    pub fn root_cause(&self) -> &dyn Fail {
        find_root_cause(self)
    }

    /// Deprecated alias to `iter_chain`.
    #[deprecated(since = "0.1.2", note = "please use the 'iter_chain()' method instead")]
    pub fn causes(&self) -> Causes {
        Causes { fail: Some(self) }
    }
}

#[cfg(feature = "std")]
impl<E: StdError + Send + Sync + 'static> Fail for E {}

#[cfg(feature = "std")]
impl Fail for Box<dyn Fail> {
    fn cause(&self) -> Option<&dyn Fail> {
        (**self).cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        (**self).backtrace()
    }
}

/// A iterator over the causes of a `Fail`
pub struct Causes<'f> {
    fail: Option<&'f dyn Fail>,
}

impl<'f> Iterator for Causes<'f> {
    type Item = &'f dyn Fail;
    fn next(&mut self) -> Option<&'f dyn Fail> {
        self.fail.map(|fail| {
            self.fail = fail.cause();
            fail
        })
    }
}

fn find_root_cause(mut fail: &dyn Fail) -> &dyn Fail {
    while let Some(cause) = fail.cause() {
        fail = cause;
    }

    fail
}
