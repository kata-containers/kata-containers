//! This tiny crate checks that the running or installed `rustc` meets some
//! version requirements. The version is queried by calling the Rust compiler
//! with `--version`. The path to the compiler is determined first via the
//! `RUSTC` environment variable. If it is not set, then `rustc` is used. If
//! that fails, no determination is made, and calls return `None`.
//!
//! # Examples
//!
//! Set a `cfg` flag in `build.rs` if the running compiler was determined to be
//! at least version `1.13.0`:
//!
//! ```rust
//! extern crate version_check as rustc;
//!
//! if rustc::is_min_version("1.13.0").unwrap_or(false) {
//!     println!("cargo:rustc-cfg=question_mark_operator");
//! }
//! ```
//!
//! See [`is_max_version`] or [`is_exact_version`] to check if the compiler
//! is _at most_ or _exactly_ a certain version.
//!
//! Check that the running compiler was released on or after `2018-12-18`:
//!
//! ```rust
//! extern crate version_check as rustc;
//!
//! match rustc::is_min_date("2018-12-18") {
//!     Some(true) => "Yep! It's recent!",
//!     Some(false) => "No, it's older.",
//!     None => "Couldn't determine the rustc version."
//! };
//! ```
//!
//! See [`is_max_date`] or [`is_exact_date`] to check if the compiler was
//! released _prior to_ or _exactly on_ a certain date.
//!
//! Check that the running compiler supports feature flags:
//!
//! ```rust
//! extern crate version_check as rustc;
//!
//! match rustc::is_feature_flaggable() {
//!     Some(true) => "Yes! It's a dev or nightly release!",
//!     Some(false) => "No, it's stable or beta.",
//!     None => "Couldn't determine the rustc version."
//! };
//! ```
//!
//! Check that the running compiler is on the stable channel:
//!
//! ```rust
//! extern crate version_check as rustc;
//!
//! match rustc::Channel::read() {
//!     Some(c) if c.is_stable() => format!("Yes! It's stable."),
//!     Some(c) => format!("No, the channel {} is not stable.", c),
//!     None => format!("Couldn't determine the rustc version.")
//! };
//! ```
//!
//! To interact with the version, release date, and release channel as structs,
//! use [`Version`], [`Date`], and [`Channel`], respectively. The [`triple()`]
//! function returns all three values efficiently.
//!
//! # Alternatives
//!
//! This crate is dead simple with no dependencies. If you need something more
//! and don't care about panicking if the version cannot be obtained, or if you
//! don't mind adding dependencies, see
//! [rustc_version](https://crates.io/crates/rustc_version).

#![allow(deprecated)]

mod version;
mod channel;
mod date;

use std::env;
use std::process::Command;

#[doc(inline)] pub use version::*;
#[doc(inline)] pub use channel::*;
#[doc(inline)] pub use date::*;

/// Parses (version, date) as available from rustc version string.
fn version_and_date_from_rustc_version(s: &str) -> (Option<String>, Option<String>) {
    let last_line = s.lines().last().unwrap_or(s);
    let mut components = last_line.trim().split(" ");
    let version = components.nth(1);
    let date = components.filter(|c| c.ends_with(')')).next()
        .map(|s| s.trim_right().trim_right_matches(")").trim_left().trim_left_matches('('));
    (version.map(|s| s.to_string()), date.map(|s| s.to_string()))
}

/// Returns (version, date) as available from `rustc --version`.
fn get_version_and_date() -> Option<(Option<String>, Option<String>)> {
    env::var("RUSTC").ok()
        .and_then(|rustc| Command::new(rustc).arg("--version").output().ok())
        .or_else(|| Command::new("rustc").arg("--version").output().ok())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| version_and_date_from_rustc_version(&s))
}

/// Reads the triple of [`Version`], [`Channel`], and [`Date`] of the installed
/// or running `rustc`.
///
/// If any attribute cannot be determined (see the [top-level
/// documentation](crate)), returns `None`.
///
/// To obtain only one of three attributes, use [`Version::read()`],
/// [`Channel::read()`], or [`Date::read()`].
pub fn triple() -> Option<(Version, Channel, Date)> {
    let (version_str, date_str) = match get_version_and_date() {
        Some((Some(version), Some(date))) => (version, date),
        _ => return None
    };

    // Can't use `?` or `try!` for `Option` in 1.0.0.
    match Version::parse(&version_str) {
        Some(version) => match Channel::parse(&version_str) {
            Some(channel) => match Date::parse(&date_str) {
                Some(date) => Some((version, channel, date)),
                _ => None,
            },
            _ => None,
        },
        _ => None
    }
}

/// Checks that the running or installed `rustc` was released **on or after**
/// some date.
///
/// The format of `min_date` must be YYYY-MM-DD. For instance: `2016-12-20` or
/// `2017-01-09`.
///
/// If the date cannot be retrieved or parsed, or if `min_date` could not be
/// parsed, returns `None`. Otherwise returns `true` if the installed `rustc`
/// was release on or after `min_date` and `false` otherwise.
pub fn is_min_date(min_date: &str) -> Option<bool> {
    match (Date::read(), Date::parse(min_date)) {
        (Some(rustc_date), Some(min_date)) => Some(rustc_date >= min_date),
        _ => None
    }
}

/// Checks that the running or installed `rustc` was released **on or before**
/// some date.
///
/// The format of `max_date` must be YYYY-MM-DD. For instance: `2016-12-20` or
/// `2017-01-09`.
///
/// If the date cannot be retrieved or parsed, or if `max_date` could not be
/// parsed, returns `None`. Otherwise returns `true` if the installed `rustc`
/// was release on or before `max_date` and `false` otherwise.
pub fn is_max_date(max_date: &str) -> Option<bool> {
    match (Date::read(), Date::parse(max_date)) {
        (Some(rustc_date), Some(max_date)) => Some(rustc_date <= max_date),
        _ => None
    }
}

/// Checks that the running or installed `rustc` was released **exactly** on
/// some date.
///
/// The format of `date` must be YYYY-MM-DD. For instance: `2016-12-20` or
/// `2017-01-09`.
///
/// If the date cannot be retrieved or parsed, or if `date` could not be parsed,
/// returns `None`. Otherwise returns `true` if the installed `rustc` was
/// release on `date` and `false` otherwise.
pub fn is_exact_date(date: &str) -> Option<bool> {
    match (Date::read(), Date::parse(date)) {
        (Some(rustc_date), Some(date)) => Some(rustc_date == date),
        _ => None
    }
}

/// Checks that the running or installed `rustc` is **at least** some minimum
/// version.
///
/// The format of `min_version` is a semantic version: `1.3.0`, `1.15.0-beta`,
/// `1.14.0`, `1.16.0-nightly`, etc.
///
/// If the version cannot be retrieved or parsed, or if `min_version` could not
/// be parsed, returns `None`. Otherwise returns `true` if the installed `rustc`
/// is at least `min_version` and `false` otherwise.
pub fn is_min_version(min_version: &str) -> Option<bool> {
    match (Version::read(), Version::parse(min_version)) {
        (Some(rustc_ver), Some(min_ver)) => Some(rustc_ver >= min_ver),
        _ => None
    }
}

/// Checks that the running or installed `rustc` is **at most** some maximum
/// version.
///
/// The format of `max_version` is a semantic version: `1.3.0`, `1.15.0-beta`,
/// `1.14.0`, `1.16.0-nightly`, etc.
///
/// If the version cannot be retrieved or parsed, or if `max_version` could not
/// be parsed, returns `None`. Otherwise returns `true` if the installed `rustc`
/// is at most `max_version` and `false` otherwise.
pub fn is_max_version(max_version: &str) -> Option<bool> {
    match (Version::read(), Version::parse(max_version)) {
        (Some(rustc_ver), Some(max_ver)) => Some(rustc_ver <= max_ver),
        _ => None
    }
}

/// Checks that the running or installed `rustc` is **exactly** some version.
///
/// The format of `version` is a semantic version: `1.3.0`, `1.15.0-beta`,
/// `1.14.0`, `1.16.0-nightly`, etc.
///
/// If the version cannot be retrieved or parsed, or if `version` could not be
/// parsed, returns `None`. Otherwise returns `true` if the installed `rustc` is
/// exactly `version` and `false` otherwise.
pub fn is_exact_version(version: &str) -> Option<bool> {
    match (Version::read(), Version::parse(version)) {
        (Some(rustc_ver), Some(version)) => Some(rustc_ver == version),
        _ => None
    }
}

/// Checks whether the running or installed `rustc` supports feature flags.
///
/// In other words, if the channel is either "nightly" or "dev".
///
/// If the version could not be determined, returns `None`. Otherwise returns
/// `true` if the running version supports feature flags and `false` otherwise.
pub fn is_feature_flaggable() -> Option<bool> {
    Channel::read().map(|c| c.supports_features())
}

#[cfg(test)]
mod tests {
    use super::version_and_date_from_rustc_version;

    macro_rules! check_parse {
        ($s:expr => $v:expr, $d:expr) => (
            if let (Some(v), d) = version_and_date_from_rustc_version($s) {
                let e_d: Option<&str> = $d.into();
                assert_eq!((v, d), ($v.into(), e_d.map(|s| s.into())));
            } else {
                panic!("{:?} didn't parse for version testing.", $s);
            }
        )
    }

    #[test]
    fn test_version_parse() {
        check_parse!("rustc 1.18.0" => "1.18.0", None);
        check_parse!("rustc 1.8.0" => "1.8.0", None);
        check_parse!("rustc 1.20.0-nightly" => "1.20.0-nightly", None);
        check_parse!("rustc 1.20" => "1.20", None);
        check_parse!("rustc 1.3" => "1.3", None);
        check_parse!("rustc 1" => "1", None);
        check_parse!("rustc 1.5.1-beta" => "1.5.1-beta", None);

        // Because of 1.0.0, we can't use Option<T>: From<T>.
        check_parse!("rustc 1.20.0 (2017-07-09)"
                     => "1.20.0", Some("2017-07-09"));

        check_parse!("rustc 1.20.0-dev (2017-07-09)"
                     => "1.20.0-dev", Some("2017-07-09"));

        check_parse!("rustc 1.20.0-nightly (d84693b93 2017-07-09)"
                       => "1.20.0-nightly", Some("2017-07-09"));

        check_parse!("rustc 1.20.0 (d84693b93 2017-07-09)"
                       => "1.20.0", Some("2017-07-09"));

        check_parse!("warning: invalid logging spec 'warning', ignoring it
                      rustc 1.30.0-nightly (3bc2ca7e4 2018-09-20)"
                      => "1.30.0-nightly", Some("2018-09-20"));

        check_parse!("warning: invalid logging spec 'warning', ignoring it\n
                      rustc 1.30.0-nightly (3bc2ca7e4 2018-09-20)"
                      => "1.30.0-nightly", Some("2018-09-20"));

        check_parse!("warning: invalid logging spec 'warning', ignoring it
                      warning: something else went wrong
                      rustc 1.30.0-nightly (3bc2ca7e4 2018-09-20)"
                      => "1.30.0-nightly", Some("2018-09-20"));
    }
}
