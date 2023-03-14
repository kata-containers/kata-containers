//! get the IANA time zone for the current system
//!
//! This small utility crate provides the
//! [`get_timezone()`](fn.get_timezone.html) function.
//!
//! ```rust
//! // Get the current time zone as a string.
//! let tz_str = iana_time_zone::get_timezone()?;
//! println!("The current time zone is: {}", tz_str);
//! # Ok::<(), iana_time_zone::GetTimezoneError>(())
//! ```
//!
//! The resulting string can be parsed to a
//! [`chrono-tz::Tz`](https://docs.rs/chrono-tz/latest/chrono_tz/enum.Tz.html)
//! variant like this:
//! ```ignore
//! let tz_str = iana_time_zone::get_timezone()?;
//! let tz: chrono_tz::Tz = tz_str.parse()?;
//! ```

#[cfg_attr(target_os = "linux", path = "tz_linux.rs")]
#[cfg_attr(target_os = "windows", path = "tz_windows.rs")]
#[cfg_attr(any(target_os = "macos", target_os = "ios"), path = "tz_macos.rs")]
#[cfg_attr(
    all(target_arch = "wasm32", not(target_os = "wasi")),
    path = "tz_wasm32.rs"
)]
#[cfg_attr(
    any(target_os = "freebsd", target_os = "dragonfly"),
    path = "tz_freebsd.rs"
)]
#[cfg_attr(
    any(target_os = "netbsd", target_os = "openbsd"),
    path = "tz_netbsd.rs"
)]
#[cfg_attr(
    any(target_os = "illumos", target_os = "solaris"),
    path = "tz_illumos.rs"
)]
#[cfg_attr(target_os = "android", path = "tz_android.rs")]
mod platform;

/// Error types
#[derive(Debug)]
pub enum GetTimezoneError {
    /// Failed to parse
    FailedParsingString,
    /// Wrapped IO error
    IoError(std::io::Error),
    /// Platform-specific error from the operating system
    OsError,
}

impl std::error::Error for GetTimezoneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GetTimezoneError::FailedParsingString => None,
            GetTimezoneError::IoError(err) => Some(err),
            GetTimezoneError::OsError => None,
        }
    }
}

impl std::fmt::Display for GetTimezoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        f.write_str(match self {
            Self::FailedParsingString => "GetTimezoneError::FailedParsingString",
            Self::IoError(err) => return err.fmt(f),
            Self::OsError => "OsError",
        })
    }
}

impl std::convert::From<std::io::Error> for GetTimezoneError {
    fn from(orig: std::io::Error) -> Self {
        GetTimezoneError::IoError(orig)
    }
}

/// Get the current IANA time zone as a string.
///
/// See the module-level documentatation for a usage example and more details
/// about this function.
#[inline]
pub fn get_timezone() -> std::result::Result<String, crate::GetTimezoneError> {
    platform::get_timezone_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_current() {
        println!("current: {}", get_timezone().unwrap());
    }
}
