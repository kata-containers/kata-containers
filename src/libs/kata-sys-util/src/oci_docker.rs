// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! Helpers to detect Docker-driven containers and to resolve the process network
//! namespace path that Docker (including Docker 26+) places under
//! `/var/run/docker/netns/`. The logic matches the Go runtime helpers
//! in `src/runtime/virtcontainers/utils/utils.go`.

use std::fs;

use oci_spec::runtime as oci;

const LIBNETWORK_SETKEY: &str = "libnetwork-setkey";

const DOCKER_NETNS_PREFIXES: [&str; 2] = ["/var/run/docker/netns/", "/run/docker/netns/"];

/// Returns `true` if the value is a Docker 64-char lowercase-hex sandbox id.
fn valid_sandbox_id(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// Returns if the OCI spec looks like a container managed by Docker, by
/// checking Prestart and CreateRuntime hooks for `libnetwork` arguments.
/// Docker 26+ may use CreateRuntime instead of the deprecated Prestart hooks.
fn iter_docker_hooks(hooks: &oci::Hooks) -> impl Iterator<Item = &oci::Hook> + '_ {
    hooks
        .prestart()
        .iter()
        .flat_map(|v| v.iter())
        .chain(hooks.create_runtime().iter().flat_map(|v| v.iter()))
}

pub fn is_docker_container(spec: &oci::Spec) -> bool {
    let Some(hooks) = spec.hooks().as_ref() else {
        return false;
    };
    for hook in iter_docker_hooks(hooks) {
        if let Some(args) = hook.args() {
            for arg in args {
                if arg.starts_with("libnetwork") {
                    return true;
                }
            }
        }
    }
    false
}

/// Tries to discover Docker’s netns file path from `libnetwork-setkey` hook
/// arguments (Prestart and CreateRuntime). The path is only returned when
/// the target exists as a non-symlink regular file under a well-known prefix.
pub fn docker_netns_path(spec: &oci::Spec) -> Option<String> {
    let hooks = spec.hooks().as_ref()?;
    for hook in iter_docker_hooks(hooks) {
        let Some(args) = hook.args() else {
            continue;
        };
        for (i, arg) in args.iter().enumerate() {
            if arg != LIBNETWORK_SETKEY {
                continue;
            }
            let Some(sandbox_id) = args.get(i + 1) else {
                continue;
            };
            if !valid_sandbox_id(sandbox_id) {
                continue;
            }
            for prefix in DOCKER_NETNS_PREFIXES {
                let ns_path = format!("{prefix}{sandbox_id}");
                if lstat_is_regular_file(&ns_path) {
                    return Some(ns_path);
                }
            }
        }
    }
    None
}

/// Match Go `Lstat` + `IsRegular()`: not a symlink, and a regular data file.
fn lstat_is_regular_file(path: &str) -> bool {
    match fs::symlink_metadata(path) {
        Ok(m) if !m.file_type().is_symlink() => m.is_file(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci::Hook;
    use rstest::rstest;
    use std::fs;

    fn hook_with_setkey(sandbox: &str) -> Hook {
        let mut h = Hook::default();
        h.set_args(Some(vec![
            "/usr/bin/docker-proxy".to_string(),
            "libnetwork-setkey".to_string(),
            sandbox.to_string(),
            "ctrl".to_string(),
        ]));
        h
    }

    #[rstest]
    #[case(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        true
    )]
    #[case(
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        true
    )]
    #[case("tooshort", false)]
    #[case(
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        false
    )]
    #[case("", false)]
    fn test_valid_sandbox_id(#[case] input: &str, #[case] expected: bool) {
        assert_eq!(valid_sandbox_id(input), expected);
    }

    #[test]
    fn is_docker_false_without_hooks() {
        assert!(!is_docker_container(&oci::Spec::default()));
    }

    #[rstest]
    #[case::prestart(true)]
    #[case::create_runtime(false)]
    fn is_docker_true_with_libnetwork(#[case] use_prestart: bool) {
        let mut spec = oci::Spec::default();
        let h = hook_with_setkey(&"ab".repeat(32));
        let mut hs = oci::Hooks::default();
        if use_prestart {
            hs.set_prestart(Some(vec![h]));
        } else {
            hs.set_create_runtime(Some(vec![h]));
        }
        spec.set_hooks(Some(hs));
        assert!(is_docker_container(&spec));
    }

    #[rstest]
    #[case::too_short("tooshort")]
    #[case::uppercase("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")]
    #[case::right_len_non_hex("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz")]
    fn docker_netns_rejects_bad_sandbox_id(#[case] sandbox_id: &str) {
        let mut spec = oci::Spec::default();
        let h = hook_with_setkey(sandbox_id);
        let mut hs = oci::Hooks::default();
        hs.set_prestart(Some(vec![h]));
        spec.set_hooks(Some(hs));
        assert_eq!(docker_netns_path(&spec), None);
    }

    #[test]
    fn lstat_accepts_regular_file_rejects_symlink_and_dir() {
        let dir = tempfile::tempdir().unwrap();
        let regular = dir.path().join("regular");
        fs::write(&regular, b"").unwrap();
        assert!(lstat_is_regular_file(regular.to_str().unwrap()));

        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&regular, &link).unwrap();
        assert!(!lstat_is_regular_file(link.to_str().unwrap()));

        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        assert!(!lstat_is_regular_file(subdir.to_str().unwrap()));

        assert!(!lstat_is_regular_file("/nonexistent/path"));
    }
}
