use core::fmt::{self, Display, Debug};

use {Causes, Fail};
use backtrace::Backtrace;
use context::Context;
use compat::Compat;

#[cfg(feature = "std")]
use box_std::BoxStd;

#[cfg_attr(feature = "small-error", path = "./error_impl_small.rs")]
mod error_impl;
use self::error_impl::ErrorImpl;

#[cfg(feature = "std")]
use std::error::Error as StdError;


/// The `Error` type, which can contain any failure.
///
/// Functions which accumulate many kinds of errors should return this type.
/// All failures can be converted into it, so functions which catch those
/// errors can be tried with `?` inside of a function that returns this kind
/// of error.
///
/// In addition to implementing `Debug` and `Display`, this type carries `Backtrace`
/// information, and can be downcast into the failure that underlies it for
/// more detailed inspection.
pub struct Error {
    imp: ErrorImpl,
}

impl<F: Fail> From<F> for Error {
    fn from(failure: F) -> Error {
        Error {
            imp: ErrorImpl::from(failure)
        }
    }
}

impl Error {
    /// Creates an `Error` from `Box<std::error::Error>`.
    ///
    /// This method is useful for comparability with code,
    /// which does not use the `Fail` trait.
    ///
    /// # Example
    ///
    /// ```
    /// use std::error::Error as StdError;
    /// use failure::Error;
    ///
    /// fn app_fn() -> Result<i32, Error> {
    ///     let x = library_fn().map_err(Error::from_boxed_compat)?;
    ///     Ok(x * 2)
    /// }
    ///
    /// fn library_fn() -> Result<i32, Box<StdError + Sync + Send + 'static>> {
    ///     Ok(92)
    /// }
    /// ```
    #[cfg(feature = "std")]
    pub fn from_boxed_compat(err: Box<dyn StdError + Sync + Send + 'static>) -> Error {
        Error::from(BoxStd(err))
    }

    /// Return a reference to the underlying failure that this `Error`
    /// contains.
    pub fn as_fail(&self) -> &dyn Fail {
        self.imp.failure()
    }

    /// Returns the name of the underlying fail.
    pub fn name(&self) -> Option<&str> {
        self.as_fail().name()
    }

    /// Returns a reference to the underlying cause of this `Error`. Unlike the
    /// method on `Fail`, this does not return an `Option`. The `Error` type
    /// always has an underlying failure.
    ///
    /// This method has been deprecated in favor of the [Error::as_fail] method,
    /// which does the same thing.
    #[deprecated(since = "0.1.2", note = "please use 'as_fail()' method instead")]
    pub fn cause(&self) -> &dyn Fail {
        self.as_fail()
    }

    /// Gets a reference to the `Backtrace` for this `Error`.
    ///
    /// If the failure this wrapped carried a backtrace, that backtrace will
    /// be returned. Otherwise, the backtrace will have been constructed at
    /// the point that failure was cast into the `Error` type.
    pub fn backtrace(&self) -> &Backtrace {
        self.imp.failure().backtrace().unwrap_or(&self.imp.backtrace())
    }

    /// Provides context for this `Error`.
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
    /// or a string literal, or a failure, or some other custom context-carrying
    /// type.
    pub fn context<D: Display + Send + Sync + 'static>(self, context: D) -> Context<D> {
        Context::with_err(context, self)
    }

    /// Wraps `Error` in a compatibility type.
    ///
    /// This type implements the `Error` trait from `std::error`. If you need
    /// to pass failure's `Error` to an interface that takes any `Error`, you
    /// can use this method to get a compatible type.
    pub fn compat(self) -> Compat<Error> {
        Compat { error: self }
    }

    /// Attempts to downcast this `Error` to a particular `Fail` type.
    ///
    /// This downcasts by value, returning an owned `T` if the underlying
    /// failure is of the type `T`. For this reason it returns a `Result` - in
    /// the case that the underlying error is of a different type, the
    /// original `Error` is returned.
    pub fn downcast<T: Fail>(self) -> Result<T, Error> {
        self.imp.downcast().map_err(|imp| Error { imp })
    }

    /// Returns the "root cause" of this error - the last value in the
    /// cause chain which does not return an underlying `cause`.
    pub fn find_root_cause(&self) -> &dyn Fail {
        self.as_fail().find_root_cause()
    }

    /// Returns a iterator over the causes of this error with the cause
    /// of the fail as the first item and the `root_cause` as the final item.
    ///
    /// Use `iter_chain` to also include the fail of this error itself.
    pub fn iter_causes(&self) -> Causes {
        self.as_fail().iter_causes()
    }

    /// Returns a iterator over all fails up the chain from the current
    /// as the first item up to the `root_cause` as the final item.
    ///
    /// This means that the chain also includes the fail itself which
    /// means that it does *not* start with `cause`.  To skip the outermost
    /// fail use `iter_causes` instead.
    pub fn iter_chain(&self) -> Causes {
        self.as_fail().iter_chain()
    }

    /// Attempts to downcast this `Error` to a particular `Fail` type by
    /// reference.
    ///
    /// If the underlying error is not of type `T`, this will return `None`.
    pub fn downcast_ref<T: Fail>(&self) -> Option<&T> {
        self.imp.failure().downcast_ref()
    }

    /// Attempts to downcast this `Error` to a particular `Fail` type by
    /// mutable reference.
    ///
    /// If the underlying error is not of type `T`, this will return `None`.
    pub fn downcast_mut<T: Fail>(&mut self) -> Option<&mut T> {
        self.imp.failure_mut().downcast_mut()
    }

    /// Deprecated alias to `find_root_cause`.
    #[deprecated(since = "0.1.2", note = "please use the 'find_root_cause()' method instead")]
    pub fn root_cause(&self) -> &dyn Fail {
        ::find_root_cause(self.as_fail())
    }

    /// Deprecated alias to `iter_causes`.
    #[deprecated(since = "0.1.2", note = "please use the 'iter_chain()' method instead")]
    pub fn causes(&self) -> Causes {
        Causes { fail: Some(self.as_fail()) }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.imp.failure(), f)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let backtrace = self.imp.backtrace();
        if backtrace.is_none() {
            Debug::fmt(&self.imp.failure(), f)
        } else {
            write!(f, "{:?}\n\n{:?}", &self.imp.failure(), backtrace)
        }
    }
}

impl AsRef<dyn Fail> for Error {
    fn as_ref(&self) -> &dyn Fail {
        self.as_fail()
    }
}

#[cfg(test)]
mod test {
    use std::io;
    use super::Error;

    fn assert_just_data<T: Send + Sync + 'static>() { }

    #[test]
    fn assert_error_is_just_data() {
        assert_just_data::<Error>();
    }

    #[test]
    fn methods_seem_to_work() {
        let io_error: io::Error = io::Error::new(io::ErrorKind::NotFound, "test");
        let error: Error = io::Error::new(io::ErrorKind::NotFound, "test").into();
        assert!(error.downcast_ref::<io::Error>().is_some());
        let _: ::Backtrace = *error.backtrace();
        assert_eq!(format!("{:?}", io_error), format!("{:?}", error));
        assert_eq!(format!("{}", io_error), format!("{}", error));
        drop(error);
        assert!(true);
    }

    #[test]
    fn downcast_can_be_used() {
        let mut error: Error = io::Error::new(io::ErrorKind::NotFound, "test").into();
        {
            let real_io_error_ref = error.downcast_ref::<io::Error>().unwrap();
            assert_eq!(real_io_error_ref.to_string(), "test");
        }
        {
            let real_io_error_mut = error.downcast_mut::<io::Error>().unwrap();
            assert_eq!(real_io_error_mut.to_string(), "test");
        }
        let real_io_error = error.downcast::<io::Error>().unwrap();
        assert_eq!(real_io_error.to_string(), "test");
    }
}
