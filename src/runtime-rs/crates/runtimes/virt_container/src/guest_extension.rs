// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

use std::fs::{self, File};
use std::io;

/// AppArmor is opt-in, but an explicitly configured policy image must be usable.
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
