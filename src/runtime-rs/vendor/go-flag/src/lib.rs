//! A command-line parser with compatibility of Go's `flag` in its main focus.
//!
//! ## Design Goals
//!
//! Go comes with a built-in support for command-line parsing: the `flag` library.
//! This is known to be incompatible with GNU convention, such as:
//!
//! - Short/long flags. POSIX/GNU flags sometimes have a pair of short and long
//!   flags like `-f`/`--force` or `-n`/`--lines`. `flag` doesn't have such
//!   distinction.
//! - Combined short flags. In POSIX/GNU convention, `-fd` means `-f` plus `-d`.
//!   `flag` parses it as a single flag named `fd`.
//! - Flags after arguments. POSIX/GNU allows flags to appear after positional
//!   arguments like `./command arg1 --flag arg2` unless explicitly separated
//!   by `--`. `flag` parses it as a consecutive list of positional arguments.
//!
//! The `go-flag` crate is designed to allow Rust programmers to easily port
//! Go CLI programs written using `flag` without breaking compatibility.
//!
//! Therefore, our priority is the following:
//!
//! 1. **Behavioral compatibility**. It's meant to be compatible with the Go's
//!    built-in `flag` library in its command-line behavior.
//!    Note that API compatibility (similarity) is a different matter.
//! 2. **Migration**. Being unable to use more sophisticated parsers like
//!    `structopt` is painful. Therefore, this library comes with an ability to
//!    check typical incompatible usages to allow gradual migration.
//! 3. **Simplicity**. This library isn't meant to provide full parser
//!    functionality. For example, subcommand parsing is out of scope for
//!    this library. Try to migrate to e.g. `structopt` if you want to extend
//!    your program to accept more complex flags.
//!
//! ## Example
//!
//! Typically you can use the `parse` function.
//!
//! ```rust
//! # if true {
//! #     return;
//! # }
//! let mut force = false;
//! let mut lines = 10_i32;
//! let args: Vec<String> = go_flag::parse(|flags| {
//!     flags.add_flag("f", &mut force);
//!     flags.add_flag("lines", &mut lines);
//! });
//! # drop(args);
//! ```
//!
//! If you want a list of file paths, use `PathBuf` or `OsString` to allow non-UTF8 strings.
//!
//! ```rust
//! # if true {
//! #     return;
//! # }
//! use std::path::PathBuf;
//! let args: Vec<PathBuf> = go_flag::parse(|_| {});
//! # drop(args);
//! ```
//!
//! If an incompatible usage is detected, `parse` issues warnings and continues processing.
//! You can alter the behavior using `parse_with_warnings`.
//!
//! For example, when enough time passed since the first release of your Rust port,
//! you can start to deny the incompatible usages by specifying `WarningMode::Error`:
//!
//! ```rust
//! # if true {
//! #     return;
//! # }
//! use go_flag::WarningMode;
//! let mut force = false;
//! let mut lines = 10_i32;
//! let args: Vec<String> =
//!     go_flag::parse_with_warnings(WarningMode::Error, |flags| {
//!         flags.add_flag("f", &mut force);
//!         flags.add_flag("lines", &mut lines);
//!     });
//! # drop(args);
//! ```

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;

pub use error::{FlagError, FlagParseError, FlagWarning};
pub use flag_value::{FlagSetter, FlagValue};
use unit_parsing::{parse_one, FlagResult};

mod error;
mod flag_value;
mod unit_parsing;

/// A set of flags. Allows fine control over parse procedure.
///
/// Typically you can use the `parse` function.
///
/// ## Example
///
/// ```rust
/// # fn main() -> Result<(), go_flag::FlagError> {
/// # use go_flag::FlagSet;
/// let mut force = false;
/// let mut lines = 10_i32;
/// let args: Vec<String>;
/// {
///     let mut flags = FlagSet::new();
///     flags.add_flag("f", &mut force);
///     flags.add_flag("lines", &mut lines);
///     args = flags.parse(&["-f", "--lines", "20", "--", "foo"])?;
/// }
/// assert_eq!(force, true);
/// assert_eq!(lines, 20);
/// assert_eq!(args, vec![String::from("foo")]);
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct FlagSet<'a> {
    flag_specs: HashMap<&'a str, FlagSpec<'a>>,
}

impl<'a> FlagSet<'a> {
    /// Creates a new set of flags.
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use go_flag::FlagSet;
    /// let mut flags = FlagSet::new();
    /// # flags.add_flag("f", &mut false);
    /// ```
    pub fn new() -> Self {
        Self {
            flag_specs: HashMap::new(),
        }
    }

    /// Add a flag to be parsed.
    ///
    /// ## Panics
    ///
    /// Panics if the flag of the same name is already registered.
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use go_flag::FlagSet;
    /// let mut force = false;
    /// let mut flags = FlagSet::new();
    /// flags.add_flag("f", &mut force);
    /// ```
    pub fn add_flag(&mut self, name: &'a str, value: &'a mut dyn FlagSetter) {
        let value = FlagSpec { r: value };
        let old = self.flag_specs.insert(name, value);
        if old.is_some() {
            panic!("multiple flags with same name: {}", name);
        }
    }

    /// Parses the given arguments.
    ///
    /// ## Returns
    ///
    /// Returns the list of positional arguments (remaining arguments).
    ///
    /// Positional arguments can also be parsed. You'll typically need
    /// `Vec<String>`, `Vec<OsString>` or `Vec<PathBuf>`.
    ///
    /// ## Errors
    ///
    /// It returns `Err` if the given arguments contains invalid flags.
    ///
    /// ## Example
    ///
    /// ```rust
    /// # fn main() -> Result<(), go_flag::FlagError> {
    /// # use go_flag::FlagSet;
    /// let mut force = false;
    /// let mut flags = FlagSet::new();
    /// flags.add_flag("f", &mut force);
    /// let args: Vec<String> = flags.parse(&["-f", "foo"])?;
    /// assert_eq!(args, vec![String::from("foo")]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn parse<'b, T: FlagValue, S: AsRef<OsStr>>(
        &mut self,
        args: &'b [S],
    ) -> Result<Vec<T>, FlagError> {
        self.parse_with_warnings(args, None)
    }

    /// Parses the given arguments, recording warnings issued.
    ///
    /// ## Returns
    ///
    /// Returns the list of positional arguments (remaining arguments).
    ///
    /// Positional arguments can also be parsed. You'll typically need
    /// `Vec<String>`, `Vec<OsString>` or `Vec<PathBuf>`.
    ///
    /// ## Errors
    ///
    /// It returns `Err` if the given arguments contains invalid flags.
    ///
    /// ## Example
    ///
    /// ```rust
    /// # fn main() -> Result<(), go_flag::FlagError> {
    /// # use go_flag::FlagSet;
    /// let mut warnings = Vec::new();
    /// let mut force = false;
    /// let mut flags = FlagSet::new();
    /// flags.add_flag("f", &mut force);
    /// let args: Vec<String> = flags
    ///     .parse_with_warnings(&["--f", "foo", "-non-flag"], Some(&mut warnings))?;
    /// assert_eq!(args, vec![String::from("foo"), String::from("-non-flag")]);
    /// assert_eq!(warnings[0].to_string(), "short flag with double minuses: --f");
    /// assert_eq!(warnings[1].to_string(), "flag-like syntax appearing after argument: -non-flag");
    /// # Ok(())
    /// # }
    /// ```
    pub fn parse_with_warnings<'b, T: FlagValue, S: AsRef<OsStr>>(
        &mut self,
        mut args: &'b [S],
        mut warnings: Option<&mut Vec<FlagWarning>>,
    ) -> Result<Vec<T>, FlagError> {
        loop {
            let seen = self.process_one(&mut args, reborrow_option_mut(&mut warnings))?;
            if !seen {
                break;
            }
        }
        let args = args
            .iter()
            .map(|x| {
                T::parse(Some(x.as_ref()), reborrow_option_mut(&mut warnings))
                    .map_err(|error| FlagError::ParseError { error })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(args)
    }

    fn process_one<S: AsRef<OsStr>>(
        &mut self,
        args: &mut &[S],
        mut warnings: Option<&mut Vec<FlagWarning>>,
    ) -> Result<bool, FlagError> {
        if args.is_empty() {
            return Ok(false);
        }
        let arg0: &OsStr = args[0].as_ref();
        let (num_minuses, name, value) = match parse_one(arg0) {
            FlagResult::Argument => {
                if let Some(warnings) = reborrow_option_mut(&mut warnings) {
                    for arg in args.iter() {
                        let arg = arg.as_ref();
                        let flag_like = match parse_one(arg) {
                            FlagResult::Argument | FlagResult::EndFlags => false,
                            FlagResult::BadFlag | FlagResult::Flag { .. } => true,
                        };
                        if flag_like {
                            warnings.push(FlagWarning::FlagAfterArg {
                                flag: arg.to_string_lossy().into_owned(),
                            });
                        }
                    }
                }
                return Ok(false);
            }
            FlagResult::EndFlags => {
                *args = &args[1..];
                return Ok(false);
            }
            FlagResult::BadFlag => {
                return Err(FlagError::BadFlag {
                    flag: arg0.to_string_lossy().into_owned(),
                })
            }
            FlagResult::Flag {
                num_minuses,
                name,
                value,
            } => (num_minuses, name, value),
        };
        *args = &args[1..];
        if let Some(warnings) = reborrow_option_mut(&mut warnings) {
            if name.len() > 1 && num_minuses == 1 {
                warnings.push(FlagWarning::ShortLong {
                    flag: arg0.to_string_lossy().into_owned(),
                });
            }
            if name.len() == 1 && num_minuses == 2 {
                warnings.push(FlagWarning::LongShort {
                    flag: arg0.to_string_lossy().into_owned(),
                });
            }
        }
        let name = name.to_str().ok_or_else(|| FlagError::UnknownFlag {
            name: name.to_string_lossy().into_owned(),
        })?;
        let flag_spec = if let Some(flag_spec) = self.flag_specs.get_mut(name) {
            flag_spec
        } else {
            return Err(FlagError::UnknownFlag {
                name: name.to_owned(),
            });
        };
        let value = if !flag_spec.r.is_bool_flag() && value.is_none() {
            if args.is_empty() {
                return Err(FlagError::ArgumentNeeded {
                    name: name.to_owned(),
                });
            }
            let arg1 = args[0].as_ref();
            *args = &args[1..];
            Some(arg1)
        } else {
            value.as_ref().map(|x| x.as_ref())
        };
        flag_spec
            .r
            .set(value, reborrow_option_mut(&mut warnings))
            .map_err(|error| FlagError::ParseError { error })?;
        Ok(true)
    }
}

fn reborrow_option_mut<'a, T>(x: &'a mut Option<&mut T>) -> Option<&'a mut T> {
    if let Some(x) = x {
        Some(x)
    } else {
        None
    }
}

struct FlagSpec<'a> {
    r: &'a mut dyn FlagSetter,
}

impl<'a> fmt::Debug for FlagSpec<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct FlagSetterPlaceholder<'a>(&'a dyn FlagSetter);
        impl<'a> fmt::Debug for FlagSetterPlaceholder<'a> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "<mutable reference {:p}>", self.0)
            }
        }

        f.debug_struct("FlagSpec")
            .field("r", &FlagSetterPlaceholder(self.r))
            .finish()
    }
}

/// Parses the given arguments into flags.
///
/// Flags are registered in the given closure.
///
/// ## Returns
///
/// Returns the list of positional arguments (remaining arguments).
///
/// Positional arguments can also be parsed. You'll typically need
/// `Vec<String>`, `Vec<OsString>` or `Vec<PathBuf>`.
///
/// ## Errors
///
/// It returns `Err` if the given arguments contains invalid flags.
///
/// ## Example
///
/// ```rust
/// # fn main() -> Result<(), go_flag::FlagError> {
/// let mut force = false;
/// let mut lines = 10_i32;
/// let args = ["-f", "--", "foo"];
/// let args: Vec<String> = go_flag::parse_args(&args, |flags| {
///     flags.add_flag("f", &mut force);
///     flags.add_flag("lines", &mut lines);
/// })?;
/// assert_eq!(force, true);
/// assert_eq!(lines, 10);
/// assert_eq!(args, vec![String::from("foo")]);
/// # Ok(())
/// # }
/// ```
pub fn parse_args<'a, T, S: AsRef<OsStr>, F>(args: &[S], f: F) -> Result<Vec<T>, FlagError>
where
    T: FlagValue,
    F: FnOnce(&mut FlagSet<'a>),
{
    parse_args_with_warnings(args, None, f)
}

/// Parses the given arguments into flags, recording warnings issued.
///
/// Flags are registered in the given closure.
///
/// ## Returns
///
/// Returns the list of positional arguments (remaining arguments).
///
/// Positional arguments can also be parsed. You'll typically need
/// `Vec<String>`, `Vec<OsString>` or `Vec<PathBuf>`.
///
/// ## Errors
///
/// It returns `Err` if the given arguments contains invalid flags.
///
/// ## Example
///
/// ```rust
/// # fn main() -> Result<(), go_flag::FlagError> {
/// let mut warnings = Vec::new();
/// let mut force = false;
/// let mut lines = 10_i32;
/// let args = ["--f", "--", "foo"];
/// let args: Vec<String> =
///     go_flag::parse_args_with_warnings(&args, Some(&mut warnings), |flags| {
///         flags.add_flag("f", &mut force);
///         flags.add_flag("lines", &mut lines);
///     })?;
/// assert_eq!(force, true);
/// assert_eq!(lines, 10);
/// assert_eq!(args, vec![String::from("foo")]);
/// assert_eq!(warnings[0].to_string(), "short flag with double minuses: --f");
/// # Ok(())
/// # }
/// ```
pub fn parse_args_with_warnings<'a, T, S: AsRef<OsStr>, F>(
    args: &[S],
    mut warnings: Option<&mut Vec<FlagWarning>>,
    f: F,
) -> Result<Vec<T>, FlagError>
where
    T: FlagValue,
    F: FnOnce(&mut FlagSet<'a>),
{
    let mut flag_set = FlagSet::new();
    f(&mut flag_set);
    let remain = flag_set.parse_with_warnings(args, reborrow_option_mut(&mut warnings))?;
    Ok(remain)
}

/// Parses the command-line arguments into flags.
///
/// Flags are registered in the given closure.
///
/// ## Returns
///
/// Returns the list of positional arguments (remaining arguments).
///
/// Positional arguments can also be parsed. You'll typically need
/// `Vec<String>`, `Vec<OsString>` or `Vec<PathBuf>`.
///
/// ## Exits
///
/// It exits if the command-line arguments contain invalid flags.
///
/// ## Outputs
///
/// It prints errors and warnings to the standard error stream (stderr).
///
/// ## Example
///
/// ```rust
/// # if true {
/// #     return;
/// # }
/// let mut force = false;
/// let mut lines = 10_i32;
/// let args: Vec<String> = go_flag::parse(|flags| {
///     flags.add_flag("f", &mut force);
///     flags.add_flag("lines", &mut lines);
/// });
/// # drop(args);
/// ```
pub fn parse<'a, T, F>(f: F) -> Vec<T>
where
    T: FlagValue,
    F: FnOnce(&mut FlagSet<'a>),
{
    parse_with_warnings(WarningMode::Report, f)
}

/// Parses the command-line arguments into flags, handling warnings as specified.
///
/// Flags are registered in the given closure.
///
/// ## Returns
///
/// Returns the list of positional arguments (remaining arguments).
///
/// Positional arguments can also be parsed. You'll typically need
/// `Vec<String>`, `Vec<OsString>` or `Vec<PathBuf>`.
///
/// ## Exits
///
/// It exits if:
///
/// - the command-line arguments contain invalid flags, or
/// - `mode` is `WarningMode::Error` and we have compatibility warnings.
///
/// ## Outputs
///
/// It prints errors and warnings to the standard error stream (stderr).
///
/// If `WarningMode::Ignore` is set, we'll throw warnings away.
///
/// ## Example
///
/// ```rust
/// # if true {
/// #     return;
/// # }
/// use go_flag::WarningMode;
/// let mut force = false;
/// let mut lines = 10_i32;
/// let args: Vec<String> =
///     go_flag::parse_with_warnings(WarningMode::Error, |flags| {
///         flags.add_flag("f", &mut force);
///         flags.add_flag("lines", &mut lines);
///     });
/// # drop(args);
/// ```
pub fn parse_with_warnings<'a, T, F>(mode: WarningMode, f: F) -> Vec<T>
where
    T: FlagValue,
    F: FnOnce(&mut FlagSet<'a>),
{
    let mut warnings = if mode == WarningMode::Ignore {
        None
    } else {
        Some(Vec::new())
    };
    let args = std::env::args_os().collect::<Vec<_>>();
    match parse_args_with_warnings(&args[1..], warnings.as_mut(), f) {
        Ok(x) => {
            if let Some(warnings) = &warnings {
                for w in warnings {
                    eprintln!("{}", w);
                }
                if !warnings.is_empty() && mode == WarningMode::Error {
                    std::process::exit(1);
                }
            }
            x
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

/// How `parse_with_warnings` treats compatibility warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WarningMode {
    /// Throw warnings away.
    Ignore,
    /// Report warnings to stderr and continue processing.
    Report,
    /// Report warnings to stderr and abort the program.
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args() {
        let parse = |args: &[&str]| -> Result<(bool, i32, Vec<String>), FlagError> {
            let mut force = false;
            let mut lines = 10_i32;
            let args = parse_args(args, |flags| {
                flags.add_flag("f", &mut force);
                flags.add_flag("lines", &mut lines);
            })?;
            Ok((force, lines, args))
        };
        assert_eq!(parse(&[]).unwrap(), (false, 10, vec![]));
        assert_eq!(parse(&["-f"]).unwrap(), (true, 10, vec![]));
        assert_eq!(parse(&["-f", "--lines=20"]).unwrap(), (true, 20, vec![]));
    }
}
