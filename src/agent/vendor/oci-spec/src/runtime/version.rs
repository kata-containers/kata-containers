/// API incompatible changes.
pub const VERSION_MAJOR: u32 = 1;

/// Changing functionality in a backwards-compatible manner
pub const VERSION_MINOR: u32 = 0;

/// Backwards-compatible bug fixes.
pub const VERSION_PATCH: u32 = 2;

/// Indicates development branch. Releases will be empty string.
pub const VERSION_DEV: &str = "";

/// Retrieve the version as string representation.
pub fn version() -> String {
    format!(
        "{}.{}.{}{}",
        VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH, VERSION_DEV
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_test() {
        assert_eq!(version(), "1.0.2-dev".to_string())
    }
}
