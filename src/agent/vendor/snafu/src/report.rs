use crate::ChainCompat;
use core::fmt;

#[cfg(all(feature = "std", feature = "rust_1_61"))]
use std::process::{ExitCode, Termination};

/// Opinionated solution to format an error in a user-friendly
/// way. Useful as the return type from `main` and test functions.
///
/// Most users will use the [`snafu::report`][] procedural macro
/// instead of directly using this type, but you can if you do not
/// wish to use the macro.
///
/// [`snafu::report`]: macro@crate::report
///
/// ## Rust 1.61 and up
///
/// Change the return type of the function to [`Report`][] and wrap
/// the body of your function with [`Report::capture`][].
///
/// ## Rust before 1.61
///
/// Use [`Report`][] as the error type inside of [`Result`][] and then
/// call either [`Report::capture_into_result`][] or
/// [`Report::from_error`][].
///
/// ## Nightly Rust
///
/// Enabling the [`unstable-try-trait` feature flag][try-ff] will
/// allow you to use the `?` operator directly:
///
/// ```rust
/// use snafu::{prelude::*, Report};
///
/// # #[cfg(all(feature = "unstable-try-trait", feature = "rust_1_61"))]
/// fn main() -> Report<PlaceholderError> {
///     let _v = may_fail_with_placeholder_error()?;
///
///     Report::ok()
/// }
/// # #[cfg(not(all(feature = "unstable-try-trait", feature = "rust_1_61")))] fn main() {}
/// # #[derive(Debug, Snafu)]
/// # struct PlaceholderError;
/// # fn may_fail_with_placeholder_error() -> Result<u8, PlaceholderError> { Ok(42) }
/// ```
///
/// [try-ff]: crate::guide::feature_flags#unstable-try-trait
///
/// ## Interaction with the Provider API
///
/// If you return a [`Report`][] from your function and enable the
/// [`unstable-provider-api` feature flag][provider-ff], additional
/// capabilities will be added:
///
/// 1. If provided, a [`Backtrace`][] will be included in the output.
/// 1. If provided, a [`ExitCode`][] will be used as the return value.
///
/// [provider-ff]: crate::guide::feature_flags#unstable-provider-api
/// [`Backtrace`]: crate::Backtrace
/// [`ExitCode`]: std::process::ExitCode
///
/// ## Stability of the output
///
/// The exact content and format of a displayed `Report` are not
/// stable, but this type strives to print the error and as much
/// user-relevant information in an easily-consumable manner
pub struct Report<E>(Result<(), E>);

impl<E> Report<E> {
    /// Convert an error into a [`Report`][].
    ///
    /// Recommended if you support versions of Rust before 1.61.
    ///
    /// ```rust
    /// use snafu::{prelude::*, Report};
    ///
    /// #[derive(Debug, Snafu)]
    /// struct PlaceholderError;
    ///
    /// fn main() -> Result<(), Report<PlaceholderError>> {
    ///     let _v = may_fail_with_placeholder_error().map_err(Report::from_error)?;
    ///     Ok(())
    /// }
    ///
    /// fn may_fail_with_placeholder_error() -> Result<u8, PlaceholderError> {
    ///     Ok(42)
    /// }
    /// ```
    pub fn from_error(error: E) -> Self {
        Self(Err(error))
    }

    /// Executes a closure that returns a [`Result`][], converting the
    /// error variant into a [`Report`][].
    ///
    /// Recommended if you support versions of Rust before 1.61.
    ///
    /// ```rust
    /// use snafu::{prelude::*, Report};
    ///
    /// #[derive(Debug, Snafu)]
    /// struct PlaceholderError;
    ///
    /// fn main() -> Result<(), Report<PlaceholderError>> {
    ///     Report::capture_into_result(|| {
    ///         let _v = may_fail_with_placeholder_error()?;
    ///
    ///         Ok(())
    ///     })
    /// }
    ///
    /// fn may_fail_with_placeholder_error() -> Result<u8, PlaceholderError> {
    ///     Ok(42)
    /// }
    /// ```
    pub fn capture_into_result<T>(body: impl FnOnce() -> Result<T, E>) -> Result<T, Self> {
        body().map_err(Self::from_error)
    }

    /// Executes a closure that returns a [`Result`][], converting any
    /// error to a [`Report`][].
    ///
    /// Recommended if you only support Rust version 1.61 or above.
    ///
    /// ```rust
    /// use snafu::{prelude::*, Report};
    ///
    /// #[derive(Debug, Snafu)]
    /// struct PlaceholderError;
    ///
    /// # #[cfg(feature = "rust_1_61")]
    /// fn main() -> Report<PlaceholderError> {
    ///     Report::capture(|| {
    ///         let _v = may_fail_with_placeholder_error()?;
    ///
    ///         Ok(())
    ///     })
    /// }
    /// # #[cfg(not(feature = "rust_1_61"))] fn main() {}
    ///
    /// fn may_fail_with_placeholder_error() -> Result<u8, PlaceholderError> {
    ///     Ok(42)
    /// }
    /// ```
    pub fn capture(body: impl FnOnce() -> Result<(), E>) -> Self {
        Self(body())
    }

    /// A [`Report`][] that indicates no error occurred.
    pub const fn ok() -> Self {
        Self(Ok(()))
    }
}

impl<E> From<Result<(), E>> for Report<E> {
    fn from(other: Result<(), E>) -> Self {
        Self(other)
    }
}

impl<E> fmt::Debug for Report<E>
where
    E: crate::Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<E> fmt::Display for Report<E>
where
    E: crate::Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Err(e) => fmt::Display::fmt(&ReportFormatter(e), f),
            _ => Ok(()),
        }
    }
}

#[cfg(all(feature = "std", feature = "rust_1_61"))]
impl<E> Termination for Report<E>
where
    E: crate::Error,
{
    fn report(self) -> ExitCode {
        match self.0 {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{}", ReportFormatter(&e));

                #[cfg(feature = "unstable-provider-api")]
                {
                    use core::any;

                    any::request_value::<ExitCode>(&e)
                        .or_else(|| any::request_ref::<ExitCode>(&e).copied())
                        .unwrap_or(ExitCode::FAILURE)
                }

                #[cfg(not(feature = "unstable-provider-api"))]
                {
                    ExitCode::FAILURE
                }
            }
        }
    }
}

#[cfg(feature = "unstable-try-trait")]
impl<T, E> core::ops::FromResidual<Result<T, E>> for Report<E> {
    fn from_residual(residual: Result<T, E>) -> Self {
        Self(residual.map(drop))
    }
}

struct ReportFormatter<'a>(&'a dyn crate::Error);

impl<'a> fmt::Display for ReportFormatter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "std")]
        {
            if trace_cleaning_enabled() {
                self.cleaned_error_trace(f)?;
            } else {
                self.error_trace(f)?;
            }
        }

        #[cfg(not(feature = "std"))]
        {
            self.error_trace(f)?;
        }

        #[cfg(feature = "unstable-provider-api")]
        {
            use core::any;

            if let Some(bt) = any::request_ref::<crate::Backtrace>(self.0) {
                writeln!(f, "\nBacktrace:\n{}", bt)?;
            }
        }

        Ok(())
    }
}

impl<'a> ReportFormatter<'a> {
    fn error_trace(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "{}", self.0)?;

        let sources = ChainCompat::new(self.0).skip(1);
        let plurality = sources.clone().take(2).count();

        match plurality {
            0 => {}
            1 => writeln!(f, "\nCaused by this error:")?,
            _ => writeln!(f, "\nCaused by these errors (recent errors listed first):")?,
        }

        for (i, source) in sources.enumerate() {
            // Let's use 1-based indexing for presentation
            let i = i + 1;
            writeln!(f, "{:3}: {}", i, source)?;
        }

        Ok(())
    }

    #[cfg(feature = "std")]
    fn cleaned_error_trace(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        const NOTE: char = '*';

        let mut original_messages = ChainCompat::new(self.0).map(ToString::to_string);
        let mut prev = original_messages.next();

        let mut cleaned_messages = vec![];
        let mut any_cleaned = false;
        let mut any_removed = false;
        for msg in original_messages {
            if let Some(mut prev) = prev {
                let cleaned = prev.trim_end_matches(&msg).trim_end().trim_end_matches(':');
                if cleaned.is_empty() {
                    any_removed = true;
                    // Do not add this to the output list
                } else if cleaned != prev {
                    any_cleaned = true;
                    let cleaned_len = cleaned.len();
                    prev.truncate(cleaned_len);
                    prev.push(' ');
                    prev.push(NOTE);
                    cleaned_messages.push(prev);
                } else {
                    cleaned_messages.push(prev);
                }
            }

            prev = Some(msg);
        }
        cleaned_messages.extend(prev);

        let mut visible_messages = cleaned_messages.iter();

        let head = match visible_messages.next() {
            Some(v) => v,
            None => return Ok(()),
        };

        writeln!(f, "{}", head)?;

        match cleaned_messages.len() {
            0 | 1 => {}
            2 => writeln!(f, "\nCaused by this error:")?,
            _ => writeln!(f, "\nCaused by these errors (recent errors listed first):")?,
        }

        for (i, msg) in visible_messages.enumerate() {
            // Let's use 1-based indexing for presentation
            let i = i + 1;
            writeln!(f, "{:3}: {}", i, msg)?;
        }

        if any_cleaned || any_removed {
            write!(f, "\nNOTE: ")?;

            if any_cleaned {
                write!(
                    f,
                    "Some redundant information has been removed from the lines marked with {}. ",
                    NOTE,
                )?;
            } else {
                write!(f, "Some redundant information has been removed. ")?;
            }

            writeln!(
                f,
                "Set {}=1 to disable this behavior.",
                SNAFU_RAW_ERROR_MESSAGES,
            )?;
        }

        Ok(())
    }
}

#[cfg(feature = "std")]
const SNAFU_RAW_ERROR_MESSAGES: &str = "SNAFU_RAW_ERROR_MESSAGES";

#[cfg(feature = "std")]
fn trace_cleaning_enabled() -> bool {
    use crate::once_bool::OnceBool;
    use std::env;

    static DISABLED: OnceBool = OnceBool::new();
    !DISABLED.get(|| env::var_os(SNAFU_RAW_ERROR_MESSAGES).map_or(false, |v| v == "1"))
}

#[doc(hidden)]
pub trait __InternalExtractErrorType {
    type Err;
}

impl<T, E> __InternalExtractErrorType for core::result::Result<T, E> {
    type Err = E;
}
