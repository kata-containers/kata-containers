// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use super::cvt;
use crate::error::{Result, SeccompError};
use crate::version::ensure_supported_version;
use crate::{check_version, ScmpVersion};
use libseccomp_sys::*;

/// Sets the API level forcibly.
///
/// General use of this function is strongly discouraged.
/// See the [`seccomp_api_get(3)`] man page for details on available API levels.
///
/// [`seccomp_api_get(3)`]: https://www.man7.org/linux/man-pages/man3/seccomp_api_get.3.html
///
/// This function corresponds to
/// [`seccomp_api_set`](https://www.man7.org/linux/man-pages/man3/seccomp_api_set.3.html).
///
/// # Arguments
///
/// * `level` - The API level
///
/// # Errors
///
/// If the API level can not be detected due to the library being older than v2.4.0,
/// an error will be returned.
///
/// # Examples
///
/// ```
/// # use libseccomp::*;
/// set_api(1)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn set_api(level: u32) -> Result<()> {
    cvt(unsafe { seccomp_api_set(level) })?;

    Ok(())
}

/// Gets the API level supported by the system.
///
/// See the [`seccomp_api_get(3)`] man page for details on available API levels.
///
/// [`seccomp_api_get(3)`]: https://www.man7.org/linux/man-pages/man3/seccomp_api_get.3.html
///
/// This function corresponds to
/// [`seccomp_api_get`](https://www.man7.org/linux/man-pages/man3/seccomp_api_get.3.html).
///
/// # Examples
///
/// ```
/// # use libseccomp::*;
/// set_api(1)?;
/// assert_eq!(get_api(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn get_api() -> u32 {
    unsafe { seccomp_api_get() }
}

/// Checks that both the libseccomp API level and the libseccomp version being
/// used are equal to or greater than the specified API level and version.
///
/// This function returns `Ok(true)` if both the libseccomp API level and the
/// libseccomp version are equal to or greater than the specified API level and
/// version, `Ok(false)` otherwise.
///
/// # Arguments
///
/// * `min_level` - The libseccomp API level you want to check
/// * `expected` - The libseccomp version you want to check
///
/// # Errors
///
/// If an issue is encountered getting the current API level or version,
/// an error will be returned.
///
/// # Examples
///
/// ```
/// # use libseccomp::*;
/// assert!(check_api(3, ScmpVersion::from((2, 4, 0)))?);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn check_api(min_level: u32, expected: ScmpVersion) -> Result<bool> {
    let level = get_api();

    if level >= min_level && check_version(expected)? {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Ensures that both the libseccomp API level and the libseccomp version are
/// equal to or greater than the specified API level and version.
///
/// # Arguments
///
/// * `msg` - An arbitrary non-empty operation description, used as a part
/// of the error message returned.
/// * `min_level` - The libseccomp API level you want to check
/// * `expected` - The libseccomp version you want to check
///
/// # Errors
///
/// If the libseccomp API level and the libseccomp version being used are less than
/// the specified version, an error will be returned.
pub(crate) fn ensure_supported_api(msg: &str, min_level: u32, expected: ScmpVersion) -> Result<()> {
    let level = get_api();

    if level >= min_level {
        ensure_supported_version(msg, expected)
    } else {
        let current = ScmpVersion::current()?;
        Err(SeccompError::with_msg(format!(
            "{} requires libseccomp >= {} and API level >= {} (current version: {}, API level: {})",
            msg, expected, min_level, current, level
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_supported_api() {
        assert!(ensure_supported_api("test", 3, ScmpVersion::from((2, 4, 0))).is_ok());
        assert!(ensure_supported_api("test", 100, ScmpVersion::from((2, 4, 0))).is_err());
    }
}
