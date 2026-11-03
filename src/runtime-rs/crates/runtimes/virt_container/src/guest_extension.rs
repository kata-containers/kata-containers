// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

use std::fs::{self, File};
use std::io;

/// Validate before creating the VMM and again before attaching its devices.
pub(crate) fn validate_guest_apparmor<'a>(
    enabled: bool,
    hypervisor: &str,
    image: &str,
    initrd: &str,
    extensions: impl Iterator<Item = (&'a str, &'a str)>,
) -> io::Result<()> {
    if !enabled {
        return Ok(());
    }
    if hypervisor != "qemu" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "enable_guest_apparmor requires QEMU",
        ));
    }
    if image.trim().is_empty() || !initrd.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "enable_guest_apparmor requires an image-based guest without initrd",
        ));
    }
    let mut apparmor = extensions.filter(|(name, _)| *name == "apparmor");
    let image = apparmor.next();
    if image.is_none() || apparmor.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "enable_guest_apparmor requires exactly one guest_extension_images 'apparmor' entry",
        ));
    }
    if let Some((name, path)) = image {
        validate_apparmor_extension(name, path)?;
    }
    Ok(())
}

/// Old agents report false by omitting the proto3 bool (or all agent details).
pub(crate) fn validate_guest_apparmor_mode(
    expected: bool,
    reported: Option<bool>,
) -> io::Result<()> {
    let actual = reported.unwrap_or(false);
    if expected != actual {
        return Err(io::Error::other(format!(
            "guest AppArmor mode mismatch: configured={expected}, guest_apparmor_enabled={actual}"
        )));
    }
    Ok(())
}

/// Other extensions retain their existing optional-path semantics.
pub(crate) fn validate_apparmor_extension(name: &str, path: &str) -> io::Result<()> {
    if name != "apparmor" {
        return Ok(());
    }

    let validate = || {
        if path.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "AppArmor extension requires a non-empty image path",
            ));
        }
        // Reject special files before opening so a FIFO cannot block startup.
        if !fs::metadata(path)?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "AppArmor extension image must be a regular file",
            ));
        }
        File::open(path)?;
        Ok(())
    };
    validate().map_err(|err: io::Error| {
        io::Error::new(
            err.kind(),
            format!("guest_extension_images 'apparmor' has invalid path {path:?}: {err}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn guest_apparmor_requires_qemu_and_one_policy() {
        for hypervisor in [
            "qemu",
            "clh",
            "firecracker",
            "dragonball",
            "remote",
            "openvmm",
        ] {
            for images in [
                vec![],
                vec![("gpu", "")],
                vec![("apparmor", "/bin/echo")],
                vec![("apparmor", "/bin/echo"), ("apparmor", "/bin/echo")],
                vec![("gpu", ""), ("apparmor", "/bin/echo")],
                vec![("AppArmor", "/bin/echo")],
            ] {
                let count = images
                    .iter()
                    .filter(|(name, _)| *name == "apparmor")
                    .count();
                assert_eq!(
                    validate_guest_apparmor(
                        true,
                        hypervisor,
                        "/guest.img",
                        "",
                        images.iter().copied()
                    )
                    .is_ok(),
                    hypervisor == "qemu" && count == 1
                );
                validate_guest_apparmor(false, hypervisor, "", "/initrd", images.into_iter())
                    .unwrap();
            }
        }
        validate_guest_apparmor(
            false,
            "qemu",
            "",
            "",
            std::iter::once(("apparmor", "bad\0path")),
        )
        .unwrap();
    }

    #[test]
    fn guest_apparmor_requires_image_without_initrd() {
        for (image, initrd, valid) in [
            ("", "", false),
            (" \t", "", false),
            ("", "/initrd", false),
            ("/guest.img", "/initrd", false),
            ("/guest.img", "", true),
        ] {
            assert_eq!(
                validate_guest_apparmor(
                    true,
                    "qemu",
                    image,
                    initrd,
                    std::iter::once(("apparmor", "/bin/echo"))
                )
                .is_ok(),
                valid
            );
        }
    }

    #[test]
    fn guest_apparmor_mode_must_match_including_legacy_agents() {
        for expected in [false, true] {
            for reported in [None, Some(false), Some(true)] {
                assert_eq!(
                    validate_guest_apparmor_mode(expected, reported).is_ok(),
                    expected == reported.unwrap_or(false)
                );
            }
        }
    }

    // Keep these filesystem checks independent of the VMM and its dependencies.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("kata-apparmor-{}-{nonce}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn test_apparmor_requires_path() {
        for path in ["", " ", "\t\n"] {
            let err = validate_apparmor_extension("apparmor", path).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
            assert!(err
                .to_string()
                .contains("guest_extension_images 'apparmor'"));
            assert!(err.to_string().contains("non-empty image path"));
        }
    }

    #[test]
    fn test_apparmor_invalid_paths() {
        let dir = TestDir::new();
        let missing = dir.0.join("missing.img");
        for path in [
            dir.0.to_str().unwrap(),
            missing.to_str().unwrap(),
            "bad\0path",
        ] {
            let err = validate_apparmor_extension("apparmor", path).unwrap_err();
            assert!(err
                .to_string()
                .contains("guest_extension_images 'apparmor' has invalid path"));
        }
    }

    #[test]
    fn test_apparmor_readable_file() {
        let dir = TestDir::new();
        let image = dir.0.join("apparmor.img");
        fs::write(&image, b"policy image").unwrap();
        validate_apparmor_extension("apparmor", image.to_str().unwrap()).unwrap();
        fs::remove_file(&image).unwrap();
        assert!(validate_apparmor_extension("apparmor", image.to_str().unwrap()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_apparmor_symlinks_and_special_files() {
        use std::os::unix::{fs::symlink, net::UnixListener};

        let dir = TestDir::new();
        let image = dir.0.join("apparmor.img");
        let link = dir.0.join("link.img");
        fs::write(&image, b"policy image").unwrap();
        symlink(&image, &link).unwrap();
        validate_apparmor_extension("apparmor", link.to_str().unwrap()).unwrap();
        fs::remove_file(&image).unwrap();
        assert!(validate_apparmor_extension("apparmor", link.to_str().unwrap()).is_err());

        let socket = dir.0.join("socket");
        let _listener = UnixListener::bind(&socket).unwrap();
        let err = validate_apparmor_extension("apparmor", socket.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("regular file"));
    }

    #[cfg(unix)]
    #[test]
    fn test_apparmor_unreadable_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TestDir::new();
        let image = dir.0.join("unreadable.img");
        fs::write(&image, b"policy image").unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o000)).unwrap();
        // A privileged test runner may legitimately be able to read this file.
        if File::open(&image).is_err() {
            let err = validate_apparmor_extension("apparmor", image.to_str().unwrap()).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
            assert!(err.to_string().contains("apparmor"));
        }
    }

    #[test]
    fn test_other_extensions_keep_optional_paths() {
        for name in ["coco", "gpu", "other-extension"] {
            for path in ["", "/nonexistent/extension.img", "bad\0path"] {
                validate_apparmor_extension(name, path).unwrap();
            }
        }
    }
}
