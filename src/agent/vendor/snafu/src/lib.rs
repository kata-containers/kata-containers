#![deny(missing_docs)]
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(feature = "unstable-backtraces-impl-std", feature(backtrace))]
#![cfg_attr(feature = "unstable-core-error", feature(error_in_core))]
#![cfg_attr(
    feature = "unstable-provider-api",
    feature(error_generic_member_access, provide_any)
)]
#![cfg_attr(feature = "unstable-try-trait", feature(try_trait_v2))]

//! # SNAFU
//!
//! SNAFU is a library to easily generate errors and add information
//! to underlying errors, especially when the same underlying error
//! type can occur in different contexts.
//!
//! For detailed information, please see the [`Snafu`][] macro and the
//! [user's guide](guide).
//!
//! ## Features
//!
//! - [Turnkey errors based on strings](Whatever)
//! - [Custom error types](Snafu)
//!   - Including a conversion path from turnkey errors
//! - [Backtraces](Backtrace)
//! - Extension traits for
//!   - [`Results`](ResultExt)
//!   - [`Options`](OptionExt)
#![cfg_attr(feature = "futures", doc = "   - [`Futures`](futures::TryFutureExt)")]
#![cfg_attr(feature = "futures", doc = "   - [`Streams`](futures::TryStreamExt)")]
//! - Suitable for libraries and applications
//! - `no_std` compatibility
//! - Generic types and lifetimes
//!
//! ## Quick start
//!
//! If you want to report errors without hassle, start with the
//! [`Whatever`][] type and the [`whatever!`][] macro:
//!
//! ```rust
//! use snafu::{prelude::*, Whatever};
//!
//! fn is_valid_id(id: u16) -> Result<(), Whatever> {
//!     if id < 10 {
//!         whatever!("ID may not be less than 10, but it was {}", id);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! You can also use it to wrap any other error:
//!
//! ```rust
//! use snafu::{prelude::*, Whatever};
//!
//! fn read_config_file(path: &str) -> Result<String, Whatever> {
//!     std::fs::read_to_string(path)
//!         .with_whatever_context(|_| format!("Could not read file {}", path))
//! }
//! ```
//!
//! [`Whatever`][] allows for a short message and tracks a
//! [`Backtrace`][] for every error:
//!
//! ```rust
//! use snafu::{prelude::*, ErrorCompat, Whatever};
//!
//! fn main() {
//! # fn returns_an_error() -> Result<(), Whatever> { Ok(()) }
//!     if let Err(e) = returns_an_error() {
//!         eprintln!("An error occurred: {}", e);
//!         if let Some(bt) = ErrorCompat::backtrace(&e) {
//! #           #[cfg(not(feature = "backtraces-impl-backtrace-crate"))]
//!             eprintln!("{}", bt);
//!         }
//!     }
//! }
//! ```
//!
//! ## Custom error types
//!
//! Many projects will hit limitations of the `Whatever` type. When
//! that occurs, it's time to create your own error type by deriving
//! [`Snafu`][]!
//!
//! ### Struct style
//!
//! SNAFU will read your error struct definition and create a *context
//! selector* type (called `InvalidIdSnafu` in this example). These
//! context selectors are used with the [`ensure!`][] macro to provide
//! ergonomic error creation:
//!
//! ```rust
//! use snafu::prelude::*;
//!
//! #[derive(Debug, Snafu)]
//! #[snafu(display("ID may not be less than 10, but it was {id}"))]
//! struct InvalidIdError {
//!     id: u16,
//! }
//!
//! fn is_valid_id(id: u16) -> Result<(), InvalidIdError> {
//!     ensure!(id >= 10, InvalidIdSnafu { id });
//!     Ok(())
//! }
//! ```
//!
//! If you add a `source` field to your error, you can then wrap an
//! underlying error using the [`context`](ResultExt::context)
//! extension method:
//!
//! ```rust
//! use snafu::prelude::*;
//!
//! #[derive(Debug, Snafu)]
//! #[snafu(display("Could not read file {path}"))]
//! struct ConfigFileError {
//!     source: std::io::Error,
//!     path: String,
//! }
//!
//! fn read_config_file(path: &str) -> Result<String, ConfigFileError> {
//!     std::fs::read_to_string(path).context(ConfigFileSnafu { path })
//! }
//! ```
//!
//! ### Enum style
//!
//! While error structs are good for constrained cases, they don't
//! allow for reporting multiple possible kinds of errors at one
//! time. Error enums solve that problem.
//!
//! SNAFU will read your error enum definition and create a *context
//! selector* type for each variant (called `InvalidIdSnafu` in this
//! example). These context selectors are used with the [`ensure!`][]
//! macro to provide ergonomic error creation:
//!
//! ```rust
//! use snafu::prelude::*;
//!
//! #[derive(Debug, Snafu)]
//! enum Error {
//!     #[snafu(display("ID may not be less than 10, but it was {id}"))]
//!     InvalidId { id: u16 },
//! }
//!
//! fn is_valid_id(id: u16) -> Result<(), Error> {
//!     ensure!(id >= 10, InvalidIdSnafu { id });
//!     Ok(())
//! }
//! ```
//!
//! If you add a `source` field to a variant, you can then wrap an
//! underlying error using the [`context`](ResultExt::context)
//! extension method:
//!
//! ```rust
//! use snafu::prelude::*;
//!
//! #[derive(Debug, Snafu)]
//! enum Error {
//!     #[snafu(display("Could not read file {path}"))]
//!     ConfigFile {
//!         source: std::io::Error,
//!         path: String,
//!     },
//! }
//!
//! fn read_config_file(path: &str) -> Result<String, Error> {
//!     std::fs::read_to_string(path).context(ConfigFileSnafu { path })
//! }
//! ```
//!
//! You can combine the power of the [`whatever!`][] macro with an
//! enum error type. This is great if you started out with
//! [`Whatever`][] and are moving to a custom error type:
//!
//! ```rust
//! use snafu::prelude::*;
//!
//! #[derive(Debug, Snafu)]
//! enum Error {
//!     #[snafu(display("ID may not be less than 10, but it was {id}"))]
//!     InvalidId { id: u16 },
//!
//!     #[snafu(whatever, display("{message}"))]
//!     Whatever {
//!         message: String,
//!         #[snafu(source(from(Box<dyn std::error::Error>, Some)))]
//!         source: Option<Box<dyn std::error::Error>>,
//!     },
//! }
//!
//! fn is_valid_id(id: u16) -> Result<(), Error> {
//!     ensure!(id >= 10, InvalidIdSnafu { id });
//!     whatever!("Just kidding... this function always fails!");
//!     Ok(())
//! }
//! ```
//!
//! You may wish to make the type `Send` and/or `Sync`, allowing
//! your error type to be used in multithreaded programs, by changing
//! `dyn std::error::Error` to `dyn std::error::Error + Send + Sync`.
//!
//! ## Next steps
//!
//! Read the documentation for the [`Snafu`][] macro to see all of the
//! capabilities, then read the [user's guide](guide) for deeper
//! understanding.

use core::fmt;

pub mod prelude {
    //! Traits and macros used by most projects. Add `use
    //! snafu::prelude::*` to your code to quickly get started with
    //! SNAFU.

    pub use crate::{ensure, OptionExt as _, ResultExt as _};

    // https://github.com/rust-lang/rust/issues/89020
    doc_comment::doc_comment! {
        include_str!("Snafu.md"),
        pub use snafu_derive::Snafu;
    }

    #[cfg(any(feature = "std", test))]
    pub use crate::{ensure_whatever, whatever};

    #[cfg(feature = "futures")]
    pub use crate::futures::{TryFutureExt as _, TryStreamExt as _};
}

#[cfg(all(
    not(feature = "backtraces"),
    not(feature = "backtraces-impl-backtrace-crate"),
    not(feature = "backtraces-impl-std"),
))]
mod backtrace_inert;
#[cfg(all(
    not(feature = "backtraces"),
    not(feature = "backtraces-impl-backtrace-crate"),
    not(feature = "backtraces-impl-std"),
))]
pub use crate::backtrace_inert::*;

#[cfg(all(
    feature = "backtraces",
    not(feature = "backtraces-impl-backtrace-crate"),
    not(feature = "backtraces-impl-std"),
))]
mod backtrace_shim;
#[cfg(all(
    feature = "backtraces",
    not(feature = "backtraces-impl-backtrace-crate"),
    not(feature = "backtraces-impl-std"),
))]
pub use crate::backtrace_shim::*;

#[cfg(any(feature = "std", test))]
mod once_bool;

#[cfg(feature = "backtraces-impl-backtrace-crate")]
pub use backtrace::Backtrace;

#[cfg(feature = "backtraces-impl-std")]
pub use std::backtrace::Backtrace;

#[cfg(feature = "futures")]
pub mod futures;

mod error_chain;
pub use crate::error_chain::*;

mod report;
pub use report::{Report, __InternalExtractErrorType};

doc_comment::doc_comment! {
    include_str!("Snafu.md"),
    pub use snafu_derive::Snafu;
}

doc_comment::doc_comment! {
    include_str!("report.md"),
    pub use snafu_derive::report;
}

macro_rules! generate_guide {
    (pub mod $name:ident { $($children:tt)* } $($rest:tt)*) => {
        generate_guide!(@gen ".", pub mod $name { $($children)* } $($rest)*);
    };
    (@gen $prefix:expr, ) => {};
    (@gen $prefix:expr, pub mod $name:ident; $($rest:tt)*) => {
        generate_guide!(@gen $prefix, pub mod $name { } $($rest)*);
    };
    (@gen $prefix:expr, @code pub mod $name:ident; $($rest:tt)*) => {
        #[cfg(feature = "guide")]
        pub mod $name;

        #[cfg(not(feature = "guide"))]
        /// Not currently built; please add the `guide` feature flag.
        pub mod $name {}

        generate_guide!(@gen $prefix, $($rest)*);
    };
    (@gen $prefix:expr, pub mod $name:ident { $($children:tt)* } $($rest:tt)*) => {
        #[cfg(feature = "guide")]
        doc_comment::doc_comment! {
            include_str!(concat!($prefix, "/", stringify!($name), ".md")),
            pub mod $name {
                generate_guide!(@gen concat!($prefix, "/", stringify!($name)), $($children)*);
            }
        }
        #[cfg(not(feature = "guide"))]
        /// Not currently built; please add the `guide` feature flag.
        pub mod $name {
            generate_guide!(@gen concat!($prefix, "/", stringify!($name)), $($children)*);
        }

        generate_guide!(@gen $prefix, $($rest)*);
    };
}

generate_guide! {
    pub mod guide {
        pub mod comparison {
            pub mod failure;
        }
        pub mod compatibility;
        pub mod feature_flags;
        pub mod generics;
        pub mod opaque;
        pub mod philosophy;
        pub mod structs;
        pub mod what_code_is_generated;
        pub mod troubleshooting {
            pub mod missing_field_source;
        }
        pub mod upgrading;

        @code pub mod examples;
    }
}

doc_comment::doctest!("../README.md", readme_tests);

#[cfg(feature = "unstable-core-error")]
#[doc(hidden)]
pub use core::error::Error;

#[cfg(all(not(feature = "unstable-core-error"), any(feature = "std", test)))]
#[doc(hidden)]
pub use std::error::Error;

#[cfg(not(any(feature = "unstable-core-error", feature = "std", test)))]
mod no_std_error;
#[cfg(not(any(feature = "unstable-core-error", feature = "std", test)))]
#[doc(hidden)]
pub use no_std_error::Error;

/// Ensure a condition is true. If it is not, return from the function
/// with an error.
///
/// ## Examples
///
/// ```rust
/// use snafu::prelude::*;
///
/// #[derive(Debug, Snafu)]
/// enum Error {
///     InvalidUser { user_id: i32 },
/// }
///
/// fn example(user_id: i32) -> Result<(), Error> {
///     ensure!(user_id > 0, InvalidUserSnafu { user_id });
///     // After this point, we know that `user_id` is positive.
///     let user_id = user_id as u32;
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! ensure {
    ($predicate:expr, $context_selector:expr $(,)?) => {
        if !$predicate {
            return $context_selector
                .fail()
                .map_err(::core::convert::Into::into);
        }
    };
}

/// Instantiate and return a stringly-typed error message.
///
/// This can be used with the provided [`Whatever`][] type or with a
/// custom error type that uses `snafu(whatever)`.
///
/// # Without an underlying error
///
/// Provide a format string and any optional arguments. The macro will
/// unconditionally exit the calling function with an error.
///
/// ## Examples
///
/// ```rust
/// use snafu::{Whatever, prelude::*};
///
/// type Result<T, E = Whatever> = std::result::Result<T, E>;
///
/// enum Status {
///     Sleeping,
///     Chilling,
///     Working,
/// }
///
/// # fn stand_up() {}
/// # fn go_downstairs() {}
/// fn do_laundry(status: Status, items: u8) -> Result<()> {
///     match status {
///         Status::Sleeping => whatever!("Cannot launder {items} clothes when I am asleep"),
///         Status::Chilling => {
///             stand_up();
///             go_downstairs();
///         }
///         Status::Working => {
///             go_downstairs();
///         }
///     }
///     Ok(())
/// }
/// ```
///
/// # With an underlying error
///
/// Provide a `Result` as the first argument, followed by a format
/// string and any optional arguments. If the `Result` is an error,
/// the formatted string will be appended to the error and the macro
/// will exit the calling function with an error. If the `Result` is
/// not an error, the macro will evaluate to the `Ok` value of the
/// `Result`.
///
/// ## Examples
///
/// ```rust
/// use snafu::prelude::*;
///
/// #[derive(Debug, Snafu)]
/// #[snafu(whatever, display("Error was: {message}"))]
/// struct Error {
///     message: String,
///     #[snafu(source(from(Box<dyn std::error::Error>, Some)))]
///     source: Option<Box<dyn std::error::Error>>,
/// }
/// type Result<T, E = Error> = std::result::Result<T, E>;
///
/// fn calculate_brightness_factor() -> Result<u8> {
///     let angle = calculate_angle_of_refraction();
///     let angle = whatever!(angle, "There was no angle");
///     Ok(angle * 2)
/// }
///
/// fn calculate_angle_of_refraction() -> Result<u8> {
///     whatever!("The programmer forgot to implement this...");
/// }
/// ```
#[macro_export]
#[cfg(any(feature = "std", test))]
macro_rules! whatever {
    ($fmt:literal$(, $($arg:expr),* $(,)?)?) => {
        return core::result::Result::Err({
            $crate::FromString::without_source(
                format!($fmt$(, $($arg),*)*),
            )
        });
    };
    ($source:expr, $fmt:literal$(, $($arg:expr),* $(,)?)*) => {
        match $source {
            core::result::Result::Ok(v) => v,
            core::result::Result::Err(e) => {
                return core::result::Result::Err({
                    $crate::FromString::with_source(
                        core::convert::Into::into(e),
                        format!($fmt$(, $($arg),*)*),
                    )
                });
            }
        }
    };
}

/// Ensure a condition is true. If it is not, return a stringly-typed
/// error message.
///
/// This can be used with the provided [`Whatever`][] type or with a
/// custom error type that uses `snafu(whatever)`.
///
/// ## Examples
///
/// ```rust
/// use snafu::prelude::*;
///
/// #[derive(Debug, Snafu)]
/// #[snafu(whatever, display("Error was: {message}"))]
/// struct Error {
///     message: String,
/// }
/// type Result<T, E = Error> = std::result::Result<T, E>;
///
/// fn get_bank_account_balance(account_id: &str) -> Result<u8> {
/// # fn moon_is_rising() -> bool { false }
///     ensure_whatever!(
///         moon_is_rising(),
///         "We are recalibrating the dynamos for account {}, sorry",
///         account_id,
///     );
///
///     Ok(100)
/// }
/// ```
#[macro_export]
#[cfg(any(feature = "std", test))]
macro_rules! ensure_whatever {
    ($predicate:expr, $fmt:literal$(, $($arg:expr),* $(,)?)?) => {
        if !$predicate {
            $crate::whatever!($fmt$(, $($arg),*)*);
        }
    };
}

/// Additions to [`Result`](std::result::Result).
pub trait ResultExt<T, E>: Sized {
    /// Extend a [`Result`]'s error with additional context-sensitive information.
    ///
    /// [`Result`]: std::result::Result
    ///
    /// ```rust
    /// use snafu::prelude::*;
    ///
    /// #[derive(Debug, Snafu)]
    /// enum Error {
    ///     Authenticating {
    ///         user_name: String,
    ///         user_id: i32,
    ///         source: ApiError,
    ///     },
    /// }
    ///
    /// fn example() -> Result<(), Error> {
    ///     another_function().context(AuthenticatingSnafu {
    ///         user_name: "admin",
    ///         user_id: 42,
    ///     })?;
    ///     Ok(())
    /// }
    ///
    /// # type ApiError = Box<dyn std::error::Error>;
    /// fn another_function() -> Result<i32, ApiError> {
    ///     /* ... */
    /// # Ok(42)
    /// }
    /// ```
    ///
    /// Note that the context selector will call
    /// [`Into::into`](std::convert::Into::into) on each field, so the types
    /// are not required to exactly match.
    fn context<C, E2>(self, context: C) -> Result<T, E2>
    where
        C: IntoError<E2, Source = E>,
        E2: Error + ErrorCompat;

    /// Extend a [`Result`][]'s error with lazily-generated context-sensitive information.
    ///
    /// [`Result`]: std::result::Result
    ///
    /// ```rust
    /// use snafu::prelude::*;
    ///
    /// #[derive(Debug, Snafu)]
    /// enum Error {
    ///     Authenticating {
    ///         user_name: String,
    ///         user_id: i32,
    ///         source: ApiError,
    ///     },
    /// }
    ///
    /// fn example() -> Result<(), Error> {
    ///     another_function().with_context(|_| AuthenticatingSnafu {
    ///         user_name: "admin".to_string(),
    ///         user_id: 42,
    ///     })?;
    ///     Ok(())
    /// }
    ///
    /// # type ApiError = std::io::Error;
    /// fn another_function() -> Result<i32, ApiError> {
    ///     /* ... */
    /// # Ok(42)
    /// }
    /// ```
    ///
    /// Note that this *may not* be needed in many cases because the context
    /// selector will call [`Into::into`](std::convert::Into::into) on each
    /// field.
    fn with_context<F, C, E2>(self, context: F) -> Result<T, E2>
    where
        F: FnOnce(&mut E) -> C,
        C: IntoError<E2, Source = E>,
        E2: Error + ErrorCompat;

    /// Extend a [`Result`]'s error with information from a string.
    ///
    /// The target error type must implement [`FromString`] by using
    /// the
    /// [`#[snafu(whatever)]`][Snafu#controlling-stringly-typed-errors]
    /// attribute. The premade [`Whatever`] type is also available.
    ///
    /// In many cases, you will want to use
    /// [`with_whatever_context`][Self::with_whatever_context] instead
    /// as it gives you access to the error and is only called in case
    /// of error. This method is best suited for when you have a
    /// string literal.
    ///
    /// ```rust
    /// use snafu::{prelude::*, Whatever};
    ///
    /// fn example() -> Result<(), Whatever> {
    ///     std::fs::read_to_string("/this/does/not/exist")
    ///         .whatever_context("couldn't open the file")?;
    ///     Ok(())
    /// }
    ///
    /// let err = example().unwrap_err();
    /// assert_eq!("couldn't open the file", err.to_string());
    /// ```
    #[cfg(any(feature = "std", test))]
    fn whatever_context<S, E2>(self, context: S) -> Result<T, E2>
    where
        S: Into<String>,
        E2: FromString,
        E: Into<E2::Source>;

    /// Extend a [`Result`]'s error with information from a
    /// lazily-generated string.
    ///
    /// The target error type must implement [`FromString`] by using
    /// the
    /// [`#[snafu(whatever)]`][Snafu#controlling-stringly-typed-errors]
    /// attribute. The premade [`Whatever`] type is also available.
    ///
    /// ```rust
    /// use snafu::{prelude::*, Whatever};
    ///
    /// fn example() -> Result<(), Whatever> {
    ///     let filename = "/this/does/not/exist";
    ///     std::fs::read_to_string(filename)
    ///         .with_whatever_context(|_| format!("couldn't open the file {}", filename))?;
    ///     Ok(())
    /// }
    ///
    /// let err = example().unwrap_err();
    /// assert_eq!(
    ///     "couldn't open the file /this/does/not/exist",
    ///     err.to_string(),
    /// );
    /// ```
    ///
    /// The closure is not called when the `Result` is `Ok`:
    ///
    /// ```rust
    /// use snafu::{prelude::*, Whatever};
    ///
    /// let value: std::io::Result<i32> = Ok(42);
    /// let result = value.with_whatever_context::<_, String, Whatever>(|_| {
    ///     panic!("This block will not be evaluated");
    /// });
    ///
    /// assert!(result.is_ok());
    /// ```
    #[cfg(any(feature = "std", test))]
    fn with_whatever_context<F, S, E2>(self, context: F) -> Result<T, E2>
    where
        F: FnOnce(&mut E) -> S,
        S: Into<String>,
        E2: FromString,
        E: Into<E2::Source>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    #[cfg_attr(feature = "rust_1_46", track_caller)]
    fn context<C, E2>(self, context: C) -> Result<T, E2>
    where
        C: IntoError<E2, Source = E>,
        E2: Error + ErrorCompat,
    {
        // https://github.com/rust-lang/rust/issues/74042
        match self {
            Ok(v) => Ok(v),
            Err(error) => Err(context.into_error(error)),
        }
    }

    #[cfg_attr(feature = "rust_1_46", track_caller)]
    fn with_context<F, C, E2>(self, context: F) -> Result<T, E2>
    where
        F: FnOnce(&mut E) -> C,
        C: IntoError<E2, Source = E>,
        E2: Error + ErrorCompat,
    {
        // https://github.com/rust-lang/rust/issues/74042
        match self {
            Ok(v) => Ok(v),
            Err(mut error) => {
                let context = context(&mut error);
                Err(context.into_error(error))
            }
        }
    }

    #[cfg(any(feature = "std", test))]
    #[cfg_attr(feature = "rust_1_46", track_caller)]
    fn whatever_context<S, E2>(self, context: S) -> Result<T, E2>
    where
        S: Into<String>,
        E2: FromString,
        E: Into<E2::Source>,
    {
        // https://github.com/rust-lang/rust/issues/74042
        match self {
            Ok(v) => Ok(v),
            Err(error) => Err(FromString::with_source(error.into(), context.into())),
        }
    }

    #[cfg(any(feature = "std", test))]
    #[cfg_attr(feature = "rust_1_46", track_caller)]
    fn with_whatever_context<F, S, E2>(self, context: F) -> Result<T, E2>
    where
        F: FnOnce(&mut E) -> S,
        S: Into<String>,
        E2: FromString,
        E: Into<E2::Source>,
    {
        // https://github.com/rust-lang/rust/issues/74042
        match self {
            Ok(t) => Ok(t),
            Err(mut e) => {
                let context = context(&mut e);
                Err(FromString::with_source(e.into(), context.into()))
            }
        }
    }
}

/// A temporary error type used when converting an [`Option`][] into a
/// [`Result`][]
///
/// [`Option`]: std::option::Option
/// [`Result`]: std::result::Result
pub struct NoneError;

/// Additions to [`Option`](std::option::Option).
pub trait OptionExt<T>: Sized {
    /// Convert an [`Option`][] into a [`Result`][] with additional
    /// context-sensitive information.
    ///
    /// [Option]: std::option::Option
    /// [Result]: std::option::Result
    ///
    /// ```rust
    /// use snafu::prelude::*;
    ///
    /// #[derive(Debug, Snafu)]
    /// enum Error {
    ///     UserLookup { user_id: i32 },
    /// }
    ///
    /// fn example(user_id: i32) -> Result<(), Error> {
    ///     let name = username(user_id).context(UserLookupSnafu { user_id })?;
    ///     println!("Username was {}", name);
    ///     Ok(())
    /// }
    ///
    /// fn username(user_id: i32) -> Option<String> {
    ///     /* ... */
    /// # None
    /// }
    /// ```
    ///
    /// Note that the context selector will call
    /// [`Into::into`](std::convert::Into::into) on each field, so the types
    /// are not required to exactly match.
    fn context<C, E>(self, context: C) -> Result<T, E>
    where
        C: IntoError<E, Source = NoneError>,
        E: Error + ErrorCompat;

    /// Convert an [`Option`][] into a [`Result`][] with
    /// lazily-generated context-sensitive information.
    ///
    /// [`Option`]: std::option::Option
    /// [`Result`]: std::result::Result
    ///
    /// ```
    /// use snafu::prelude::*;
    ///
    /// #[derive(Debug, Snafu)]
    /// enum Error {
    ///     UserLookup {
    ///         user_id: i32,
    ///         previous_ids: Vec<i32>,
    ///     },
    /// }
    ///
    /// fn example(user_id: i32) -> Result<(), Error> {
    ///     let name = username(user_id).with_context(|| UserLookupSnafu {
    ///         user_id,
    ///         previous_ids: Vec::new(),
    ///     })?;
    ///     println!("Username was {}", name);
    ///     Ok(())
    /// }
    ///
    /// fn username(user_id: i32) -> Option<String> {
    ///     /* ... */
    /// # None
    /// }
    /// ```
    ///
    /// Note that this *may not* be needed in many cases because the context
    /// selector will call [`Into::into`](std::convert::Into::into) on each
    /// field.
    fn with_context<F, C, E>(self, context: F) -> Result<T, E>
    where
        F: FnOnce() -> C,
        C: IntoError<E, Source = NoneError>,
        E: Error + ErrorCompat;

    /// Convert an [`Option`] into a [`Result`] with information
    /// from a string.
    ///
    /// The target error type must implement [`FromString`] by using
    /// the
    /// [`#[snafu(whatever)]`][Snafu#controlling-stringly-typed-errors]
    /// attribute. The premade [`Whatever`] type is also available.
    ///
    /// In many cases, you will want to use
    /// [`with_whatever_context`][Self::with_whatever_context] instead
    /// as it is only called in case of error. This method is best
    /// suited for when you have a string literal.
    ///
    /// ```rust
    /// use snafu::{prelude::*, Whatever};
    ///
    /// fn example(env_var_name: &str) -> Result<(), Whatever> {
    ///     std::env::var_os(env_var_name).whatever_context("couldn't get the environment variable")?;
    ///     Ok(())
    /// }
    ///
    /// let err = example("UNDEFINED_ENVIRONMENT_VARIABLE").unwrap_err();
    /// assert_eq!("couldn't get the environment variable", err.to_string());
    /// ```
    #[cfg(any(feature = "std", test))]
    fn whatever_context<S, E>(self, context: S) -> Result<T, E>
    where
        S: Into<String>,
        E: FromString;

    /// Convert an [`Option`] into a [`Result`][] with information from a
    /// lazily-generated string.
    ///
    /// The target error type must implement [`FromString`][] by using
    /// the
    /// [`#[snafu(whatever)]`][Snafu#controlling-stringly-typed-errors]
    /// attribute. The premade [`Whatever`][] type is also available.
    ///
    /// ```rust
    /// use snafu::{prelude::*, Whatever};
    ///
    /// fn example(env_var_name: &str) -> Result<(), Whatever> {
    ///     std::env::var_os(env_var_name).with_whatever_context(|| {
    ///         format!("couldn't get the environment variable {}", env_var_name)
    ///     })?;
    ///     Ok(())
    /// }
    ///
    /// let err = example("UNDEFINED_ENVIRONMENT_VARIABLE").unwrap_err();
    /// assert_eq!(
    ///     "couldn't get the environment variable UNDEFINED_ENVIRONMENT_VARIABLE",
    ///     err.to_string()
    /// );
    /// ```
    ///
    /// The closure is not called when the `Option` is `Some`:
    ///
    /// ```rust
    /// use snafu::{prelude::*, Whatever};
    ///
    /// let value = Some(42);
    /// let result = value.with_whatever_context::<_, String, Whatever>(|| {
    ///     panic!("This block will not be evaluated");
    /// });
    ///
    /// assert!(result.is_ok());
    /// ```
    #[cfg(any(feature = "std", test))]
    fn with_whatever_context<F, S, E>(self, context: F) -> Result<T, E>
    where
        F: FnOnce() -> S,
        S: Into<String>,
        E: FromString;
}

impl<T> OptionExt<T> for Option<T> {
    #[cfg_attr(feature = "rust_1_46", track_caller)]
    fn context<C, E>(self, context: C) -> Result<T, E>
    where
        C: IntoError<E, Source = NoneError>,
        E: Error + ErrorCompat,
    {
        // https://github.com/rust-lang/rust/issues/74042
        match self {
            Some(v) => Ok(v),
            None => Err(context.into_error(NoneError)),
        }
    }

    #[cfg_attr(feature = "rust_1_46", track_caller)]
    fn with_context<F, C, E>(self, context: F) -> Result<T, E>
    where
        F: FnOnce() -> C,
        C: IntoError<E, Source = NoneError>,
        E: Error + ErrorCompat,
    {
        // https://github.com/rust-lang/rust/issues/74042
        match self {
            Some(v) => Ok(v),
            None => Err(context().into_error(NoneError)),
        }
    }

    #[cfg(any(feature = "std", test))]
    #[cfg_attr(feature = "rust_1_46", track_caller)]
    fn whatever_context<S, E>(self, context: S) -> Result<T, E>
    where
        S: Into<String>,
        E: FromString,
    {
        match self {
            Some(v) => Ok(v),
            None => Err(FromString::without_source(context.into())),
        }
    }

    #[cfg(any(feature = "std", test))]
    #[cfg_attr(feature = "rust_1_46", track_caller)]
    fn with_whatever_context<F, S, E>(self, context: F) -> Result<T, E>
    where
        F: FnOnce() -> S,
        S: Into<String>,
        E: FromString,
    {
        match self {
            Some(v) => Ok(v),
            None => {
                let context = context();
                Err(FromString::without_source(context.into()))
            }
        }
    }
}

/// Backports changes to the [`Error`](std::error::Error) trait to
/// versions of Rust lacking them.
///
/// It is recommended to always call these methods explicitly so that
/// it is easy to replace usages of this trait when you start
/// supporting a newer version of Rust.
///
/// ```
/// # use snafu::{prelude::*, ErrorCompat};
/// # #[derive(Debug, Snafu)] enum Example {};
/// # fn example(error: Example) {
/// ErrorCompat::backtrace(&error); // Recommended
/// error.backtrace();              // Discouraged
/// # }
/// ```
pub trait ErrorCompat {
    /// Returns a [`Backtrace`](Backtrace) that may be printed.
    fn backtrace(&self) -> Option<&Backtrace> {
        None
    }

    /// Returns an iterator for traversing the chain of errors,
    /// starting with the current error
    /// and continuing with recursive calls to `Error::source`.
    ///
    /// To omit the current error and only traverse its sources,
    /// use `skip(1)`.
    fn iter_chain(&self) -> ChainCompat
    where
        Self: AsErrorSource,
    {
        ChainCompat::new(self.as_error_source())
    }
}

impl<'a, E> ErrorCompat for &'a E
where
    E: ErrorCompat,
{
    fn backtrace(&self) -> Option<&Backtrace> {
        (**self).backtrace()
    }
}

#[cfg(any(feature = "std", test))]
impl<E> ErrorCompat for Box<E>
where
    E: ErrorCompat,
{
    fn backtrace(&self) -> Option<&Backtrace> {
        (**self).backtrace()
    }
}

/// Converts the receiver into an [`Error`][] trait object, suitable
/// for use in [`Error::source`][].
///
/// It is expected that most users of SNAFU will not directly interact
/// with this trait.
///
/// [`Error`]: std::error::Error
/// [`Error::source`]: std::error::Error::source
//
// Given an error enum with multiple types of underlying causes:
//
// ```rust
// enum Error {
//     BoxTraitObjectSendSync(Box<dyn error::Error + Send + Sync + 'static>),
//     BoxTraitObject(Box<dyn error::Error + 'static>),
//     Boxed(Box<io::Error>),
//     Unboxed(io::Error),
// }
// ```
//
// This trait provides the answer to what consistent expression can go
// in each match arm:
//
// ```rust
// impl error::Error for Error {
//     fn source(&self) -> Option<&(dyn error::Error + 'static)> {
//         use Error::*;
//
//         let v = match *self {
//             BoxTraitObjectSendSync(ref e) => ...,
//             BoxTraitObject(ref e) => ...,
//             Boxed(ref e) => ...,
//             Unboxed(ref e) => ...,
//         };
//
//         Some(v)
//     }
// }
//
// Existing methods like returning `e`, `&**e`, `Borrow::borrow(e)`,
// `Deref::deref(e)`, and `AsRef::as_ref(e)` do not work for various
// reasons.
pub trait AsErrorSource {
    /// For maximum effectiveness, this needs to be called as a method
    /// to benefit from Rust's automatic dereferencing of method
    /// receivers.
    fn as_error_source(&self) -> &(dyn Error + 'static);
}

impl AsErrorSource for dyn Error + 'static {
    fn as_error_source(&self) -> &(dyn Error + 'static) {
        self
    }
}

impl AsErrorSource for dyn Error + Send + 'static {
    fn as_error_source(&self) -> &(dyn Error + 'static) {
        self
    }
}

impl AsErrorSource for dyn Error + Sync + 'static {
    fn as_error_source(&self) -> &(dyn Error + 'static) {
        self
    }
}

impl AsErrorSource for dyn Error + Send + Sync + 'static {
    fn as_error_source(&self) -> &(dyn Error + 'static) {
        self
    }
}

impl<T> AsErrorSource for T
where
    T: Error + 'static,
{
    fn as_error_source(&self) -> &(dyn Error + 'static) {
        self
    }
}

/// Combines an underlying error with additional information
/// about the error.
///
/// It is expected that most users of SNAFU will not directly interact
/// with this trait.
pub trait IntoError<E>
where
    E: Error + ErrorCompat,
{
    /// The underlying error
    type Source;

    /// Combine the information to produce the error
    fn into_error(self, source: Self::Source) -> E;
}

/// Takes a string message and builds the corresponding error.
///
/// It is expected that most users of SNAFU will not directly interact
/// with this trait.
#[cfg(any(feature = "std", test))]
pub trait FromString {
    /// The underlying error
    type Source;

    /// Create a brand new error from the given string
    fn without_source(message: String) -> Self;

    /// Wrap an existing error with the given string
    fn with_source(source: Self::Source, message: String) -> Self;
}

/// Construct data to be included as part of an error. The data must
/// require no arguments to be created.
pub trait GenerateImplicitData {
    /// Build the data.
    fn generate() -> Self;

    /// Build the data using the given source
    #[cfg_attr(feature = "rust_1_46", track_caller)]
    fn generate_with_source(source: &dyn crate::Error) -> Self
    where
        Self: Sized,
    {
        let _source = source;
        Self::generate()
    }
}

/// View a backtrace-like value as an optional backtrace.
pub trait AsBacktrace {
    /// Retrieve the optional backtrace
    fn as_backtrace(&self) -> Option<&Backtrace>;
}

/// Only create a backtrace when an environment variable is set.
///
/// This looks first for the value of `RUST_LIB_BACKTRACE` then
/// `RUST_BACKTRACE`. If the value is set to `1`, backtraces will be
/// enabled.
///
/// This value will be tested only once per program execution;
/// changing the environment variable after it has been checked will
/// have no effect.
///
/// ## Interaction with the Provider API
///
/// If you enable the [`unstable-provider-api` feature
/// flag][provider-ff], a backtrace will not be captured if the
/// original error is able to provide a `Backtrace`, even if the
/// appropriate environment variables are set. This prevents capturing
/// a redundant backtrace.
///
/// [provider-ff]: crate::guide::feature_flags#unstable-provider-api
#[cfg(any(feature = "std", test))]
impl GenerateImplicitData for Option<Backtrace> {
    fn generate() -> Self {
        if backtrace_collection_enabled() {
            Some(Backtrace::generate())
        } else {
            None
        }
    }

    fn generate_with_source(source: &dyn crate::Error) -> Self {
        #[cfg(feature = "unstable-provider-api")]
        {
            use core::any;

            if !backtrace_collection_enabled() {
                None
            } else if any::request_ref::<Backtrace>(source).is_some() {
                None
            } else {
                Some(Backtrace::generate_with_source(source))
            }
        }

        #[cfg(not(feature = "unstable-provider-api"))]
        {
            let _source = source;
            Self::generate()
        }
    }
}

#[cfg(any(feature = "std", test))]
impl AsBacktrace for Option<Backtrace> {
    fn as_backtrace(&self) -> Option<&Backtrace> {
        self.as_ref()
    }
}

#[cfg(any(feature = "std", test))]
fn backtrace_collection_enabled() -> bool {
    use crate::once_bool::OnceBool;
    use std::env;

    static ENABLED: OnceBool = OnceBool::new();

    ENABLED.get(|| {
        // TODO: What values count as "true"?
        env::var_os("RUST_LIB_BACKTRACE")
            .or_else(|| env::var_os("RUST_BACKTRACE"))
            .map_or(false, |v| v == "1")
    })
}

#[cfg(feature = "backtraces-impl-backtrace-crate")]
impl GenerateImplicitData for Backtrace {
    fn generate() -> Self {
        Backtrace::new()
    }
}

#[cfg(feature = "backtraces-impl-backtrace-crate")]
impl AsBacktrace for Backtrace {
    fn as_backtrace(&self) -> Option<&Backtrace> {
        Some(self)
    }
}

#[cfg(feature = "backtraces-impl-std")]
impl GenerateImplicitData for Backtrace {
    fn generate() -> Self {
        Backtrace::force_capture()
    }
}

#[cfg(feature = "backtraces-impl-std")]
impl AsBacktrace for Backtrace {
    fn as_backtrace(&self) -> Option<&Backtrace> {
        Some(self)
    }
}

/// The source code location where the error was reported.
///
/// To use it, add a field `location: Location` to your error. This
/// will automatically register it as [implicitly generated
/// data][implicit].
///
/// [implicit]: Snafu#controlling-implicitly-generated-data
///
/// ## Limitations
///
/// ### Rust 1.46
///
/// You need to enable the [`rust_1_46` feature flag][flag] for
/// implicit location capture. If you cannot enable that, you can
/// still use the [`location!`] macro at the expense of more typing.
///
/// [flag]: guide::compatibility#rust_1_46
///
/// ### Disabled context selectors
///
/// If you have [disabled the context selector][disabled], SNAFU will
/// not be able to capture an accurate location.
///
/// As a workaround, re-enable the context selector.
///
/// [disabled]: Snafu#disabling-the-context-selector
///
/// ### Asynchronous code
///
/// When using SNAFU's
#[cfg_attr(feature = "futures", doc = " [`TryFutureExt`][futures::TryFutureExt]")]
#[cfg_attr(not(feature = "futures"), doc = " `TryFutureExt`")]
/// or
#[cfg_attr(feature = "futures", doc = " [`TryStreamExt`][futures::TryStreamExt]")]
#[cfg_attr(not(feature = "futures"), doc = " `TryStreamExt`")]
/// extension traits, the automatically captured location will
/// correspond to where the future or stream was **polled**, not where
/// it was created. Additionally, many `Future` or `Stream`
/// combinators do not forward the caller's location to their
/// closures, causing the recorded location to be inside of the future
/// combinator's library.
///
/// There are two workarounds:
/// 1. Use the [`location!`] macro
/// 1. Use [`ResultExt`] instead
///
/// ```rust
/// # #[cfg(feature = "futures")] {
/// # use snafu::{prelude::*, Location, location};
/// // Non-ideal: will report where `wrapped_error_future` is `.await`ed.
/// # let error_future = async { AnotherSnafu.fail::<()>() };
/// let wrapped_error_future = error_future.context(ImplicitLocationSnafu);
///
/// // Better: will report the location of `.context`.
/// # let error_future = async { AnotherSnafu.fail::<()>() };
/// let wrapped_error_future = async { error_future.await.context(ImplicitLocationSnafu) };
///
/// // Better: Will report the location of `location!`
/// # let error_future = async { AnotherSnafu.fail::<()>() };
/// let wrapped_error_future = error_future.with_context(|_| ExplicitLocationSnafu {
///     location: location!(),
/// });
///
/// # #[derive(Debug, Snafu)] struct AnotherError;
/// #[derive(Debug, Snafu)]
/// struct ImplicitLocationError {
///     source: AnotherError,
///     location: Location,
/// }
///
/// #[derive(Debug, Snafu)]
/// struct ExplicitLocationError {
///     source: AnotherError,
///     #[snafu(implicit(false))]
///     location: Location,
/// }
/// # }
/// ```
#[derive(Copy, Clone)]
pub struct Location {
    /// The file where the error was reported
    pub file: &'static str,
    /// The line where the error was reported
    pub line: u32,
    /// The column where the error was reported
    pub column: u32,

    // Use `#[non_exhaustive]` when we upgrade to Rust 1.40
    _other: (),
}

impl Location {
    /// Constructs a `Location` using the given information
    pub fn new(file: &'static str, line: u32, column: u32) -> Self {
        Self {
            file,
            line,
            column,
            _other: (),
        }
    }
}

#[cfg(feature = "rust_1_46")]
impl Default for Location {
    #[track_caller]
    fn default() -> Self {
        let loc = core::panic::Location::caller();
        Self {
            file: loc.file(),
            line: loc.line(),
            column: loc.column(),
            _other: (),
        }
    }
}

#[cfg(feature = "rust_1_46")]
impl GenerateImplicitData for Location {
    #[inline]
    #[track_caller]
    fn generate() -> Self {
        Self::default()
    }
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Location")
            .field("file", &self.file)
            .field("line", &self.line)
            .field("column", &self.column)
            .finish()
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{file}:{line}:{column}",
            file = self.file,
            line = self.line,
            column = self.column,
        )
    }
}

/// Constructs a [`Location`] using the current file, line, and column.
#[macro_export]
macro_rules! location {
    () => {
        Location::new(file!(), line!(), column!())
    };
}

/// A basic error type that you can use as a first step to better
/// error handling.
///
/// You can use this type in your own application as a quick way to
/// create errors or add basic context to another error. This can also
/// be used in a library, but consider wrapping it in an
/// [opaque](guide::opaque) error to avoid putting the SNAFU crate in
/// your public API.
///
/// ## Examples
///
/// ```rust
/// use snafu::prelude::*;
///
/// type Result<T, E = snafu::Whatever> = std::result::Result<T, E>;
///
/// fn subtract_numbers(a: u32, b: u32) -> Result<u32> {
///     if a > b {
///         Ok(a - b)
///     } else {
///         whatever!("Can't subtract {} - {}", a, b)
///     }
/// }
///
/// fn complicated_math(a: u32, b: u32) -> Result<u32> {
///     let val = subtract_numbers(a, b).whatever_context("Can't do the math")?;
///     Ok(val * 2)
/// }
/// ```
///
/// See [`whatever!`][] for detailed usage instructions.
///
/// ## Limitations
///
/// When wrapping errors, only the backtrace from the shallowest
/// function is guaranteed to be available. If you need the deepest
/// possible trace, consider creating a custom error type and [using
/// `#[snafu(backtrace)]` on the `source`
/// field](Snafu#controlling-backtraces). If a best-effort attempt is
/// sufficient, see the [`backtrace`][Self::backtrace] method.
///
/// When the standard library stabilizes backtrace support, this
/// behavior may change.
#[derive(Debug, Snafu)]
#[snafu(crate_root(crate))]
#[snafu(whatever)]
#[snafu(display("{message}"))]
#[snafu(provide(opt, ref, chain, dyn std::error::Error => source.as_deref()))]
#[cfg(any(feature = "std", test))]
pub struct Whatever {
    #[snafu(source(from(Box<dyn std::error::Error>, Some)))]
    #[snafu(provide(false))]
    source: Option<Box<dyn std::error::Error>>,
    message: String,
    backtrace: Backtrace,
}

#[cfg(any(feature = "std", test))]
impl Whatever {
    /// Gets the backtrace from the deepest `Whatever` error. If none
    /// of the underlying errors are `Whatever`, returns the backtrace
    /// from when this instance was created.
    pub fn backtrace(&self) -> Option<&Backtrace> {
        let mut best_backtrace = &self.backtrace;

        let mut source = self.source();
        while let Some(s) = source {
            if let Some(this) = s.downcast_ref::<Self>() {
                best_backtrace = &this.backtrace;
            }
            source = s.source();
        }

        Some(best_backtrace)
    }
}
