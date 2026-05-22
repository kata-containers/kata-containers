// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! Helpers for reading `version = N` from containerd `config.toml`.

/// Reads the schema `version = N` from the containerd config root table only (before any `[` TOML table header).
///
/// Containerd keeps `version` as a top-level key; keys under `[plugins]` or other tables are ignored.
/// Ignores `#` comments on the same line. Malformed `version` lines are skipped so a later valid line can still match.
/// Returns `None` if no valid root `version` key is found.
pub fn major_version_from_config_toml(content: &str) -> Option<u32> {
    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            break;
        }
        let mut parts = line.splitn(2, '=');
        let key = parts.next().unwrap_or("").trim();
        if key != "version" {
            continue;
        }
        let Some(rhs) = parts.next() else {
            continue;
        };
        let value = rhs.trim();
        let num_str: String = value.chars().take_while(|c| c.is_ascii_digit()).collect();
        if num_str.is_empty() {
            continue;
        }
        if let Ok(n) = num_str.parse::<u32>() {
            return Some(n);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("version = 4\n", Some(4))]
    #[case("version=3\n", Some(3))]
    #[case("  version  =  2  \n", Some(2))]
    #[case("version = 4 # comment\n", Some(4))]
    // Other root keys may precede `version`
    #[case("root = '/foo'\nversion = 3\n", Some(3))]
    // Only root table: ignore `version` under [plugins]
    #[case("version = 2\n\n[plugins]\n  version = 999\n", Some(2))]
    #[case("[plugins]\nversion = 3\n", None)]
    // Malformed lines are skipped until a valid `version = N`
    #[case("version = abc\nversion = 4\n", Some(4))]
    #[case("version\nversion = 3\n", Some(3))]
    #[case("root = '/foo'\n", None)]
    #[case("", None)]
    fn test_major_version_from_config_toml(#[case] content: &str, #[case] expected: Option<u32>) {
        assert_eq!(major_version_from_config_toml(content), expected);
    }
}
