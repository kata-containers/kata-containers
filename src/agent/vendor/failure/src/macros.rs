/// Exits a function early with an `Error`.
///
/// The `bail!` macro provides an easy way to exit a function. `bail!(X)` is
/// equivalent to writing:
///
/// ```rust,ignore
/// return Err(format_err!(X))
/// ```
#[macro_export]
macro_rules! bail {
    ($e:expr) => {
        return Err($crate::err_msg($e));
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err($crate::err_msg(format!($fmt, $($arg)*)));
    };
}

/// Exits a function early with an `Error` if the condition is not satisfied.
///
/// Similar to `assert!`, `ensure!` takes a condition and exits the function
/// if the condition fails. Unlike `assert!`, `ensure!` returns an `Error`,
/// it does not panic.
#[macro_export(local_inner_macros)]
macro_rules! ensure {
    ($cond:expr) => {
        if !($cond) {
            bail!("{}", _failure__stringify!($cond));
        }
    };
    ($cond:expr, $e:expr) => {
        if !($cond) {
            bail!($e);
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !($cond) {
            bail!($fmt, $($arg)*);
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! _failure__stringify {
    ($($inner:tt)*) => {
        stringify! { $($inner)* }
    }
}

/// Constructs an `Error` using the standard string interpolation syntax.
///
/// ```rust
/// #[macro_use] extern crate failure;
///
/// fn main() {
///     let code = 101;
///     let err = format_err!("Error code: {}", code);
/// }
/// ```
#[macro_export]
macro_rules! format_err {
    ($($arg:tt)*) => { $crate::err_msg(format!($($arg)*)) }
}
