// Copyright 2026 Ant Group.
//
// SPDX-License-Identifier: Apache-2.0

//! Per-queue virtio-blk backend factory.
//!
//! `build_backend` is the single entry point used by the device-manager to
//! turn (detected on-disk format + open parameters) into a concrete
//! `Box<dyn Ufile>`. Keeping this logic here — instead of in
//! `dragonball::device_manager::blk_dev_mgr` — frees the manager from caring
//! about io_uring/aio selection, VMDK quirks, or future formats: adding a
//! new image format only requires editing this file and `format.rs`.

use std::fs::OpenOptions;
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use super::ufile::{aio::Aio, io_uring::IoUring, LocalFile, Ufile};
use super::DiskImageFormat;

#[cfg(feature = "virtio-blk-vmdk")]
use super::ufile::VmdkFile;

/// Open parameters used to construct a single per-queue backend.
///
/// `read_only` / `direct` reflect the *final* policy the manager has already
/// resolved (see `apply_image_format_overrides`); the factory does not
/// second-guess them.
#[derive(Clone, Debug)]
pub struct BackendOpenParams<'a> {
    /// Backing image path on the host filesystem.
    pub path: &'a Path,
    /// Open the underlying file without write access.
    pub read_only: bool,
    /// Open the underlying file with `O_DIRECT`.
    pub direct: bool,
    /// Skip closing the underlying file on drop (live-upgrade scenarios).
    pub no_drop: bool,
    /// Per-queue size to size the IO engine submission ring.
    pub queue_size: u32,
}

/// Build a per-queue `Ufile` backend matching the detected image format.
///
/// `format` is expected to come from a single up-front call to
/// [`detect_disk_format`][super::detect_disk_format] in the caller — this
/// factory does not re-probe the file.
pub fn build_backend(
    format: DiskImageFormat,
    params: BackendOpenParams<'_>,
) -> io::Result<Box<dyn Ufile>> {
    match format {
        DiskImageFormat::Raw => build_raw_backend(params),
        #[cfg(feature = "virtio-blk-vmdk")]
        DiskImageFormat::Vmdk => build_vmdk_backend(params),
        // Defensive: with the feature off `detect_disk_format` returns Raw
        // for VMDK files (see `format.rs`), so this arm is unreachable in
        // practice. If detection is ever tightened we want a hard error
        // rather than silently corrupting the guest by treating VMDK bytes
        // as raw sectors.
        #[cfg(not(feature = "virtio-blk-vmdk"))]
        DiskImageFormat::Vmdk => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "VMDK image detected but the `virtio-blk-vmdk` feature is disabled",
        )),
    }
}

/// Open a raw image and wrap it in a `LocalFile` driven by io_uring (when
/// the host supports it) or libaio as fallback.
fn build_raw_backend(params: BackendOpenParams<'_>) -> io::Result<Box<dyn Ufile>> {
    let custom_flags = if params.direct { libc::O_DIRECT } else { 0 };
    let file = OpenOptions::new()
        .read(true)
        .write(!params.read_only)
        .custom_flags(custom_flags)
        .open(params.path)?;

    if IoUring::is_supported() {
        let io_engine = IoUring::new(file.as_raw_fd(), params.queue_size)?;
        Ok(Box::new(LocalFile::new(file, params.no_drop, io_engine)?))
    } else {
        let io_engine = Aio::new(file.as_raw_fd(), params.queue_size)?;
        Ok(Box::new(LocalFile::new(file, params.no_drop, io_engine)?))
    }
}

/// Open a flat VMDK image via the self-contained VMDK backend..
///
/// The VMDK backend is always read-only, so
/// O_DIRECT and write access are irrelevant — `apply_image_format_overrides`
/// should have already collapsed them on the config side, but we ignore
/// `params.read_only` / `params.direct` here to keep the invariant local.
#[cfg(feature = "virtio-blk-vmdk")]
fn build_vmdk_backend(params: BackendOpenParams<'_>) -> io::Result<Box<dyn Ufile>> {
    let _ = params.queue_size;
    let _ = params.read_only;
    let _ = params.direct;
    let _ = params.no_drop;
    let file = OpenOptions::new().read(true).open(params.path)?;
    Ok(Box::new(VmdkFile::new(file, params.path)?))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use vmm_sys_util::tempfile::TempFile;

    use super::*;

    #[test]
    fn raw_backend_opens_zero_filled_file() {
        let f = TempFile::new().unwrap();
        f.as_file().set_len(4096).unwrap();
        let backend = build_backend(
            DiskImageFormat::Raw,
            BackendOpenParams {
                path: f.as_path(),
                read_only: true,
                direct: false,
                no_drop: false,
                queue_size: 128,
            },
        )
        .unwrap();
        assert_eq!(backend.get_capacity(), 4096);
    }

    #[test]
    fn raw_backend_propagates_missing_file_error() {
        let res = build_backend(
            DiskImageFormat::Raw,
            BackendOpenParams {
                path: Path::new("/nonexistent/dragonball/blk/backend/test/file"),
                read_only: true,
                direct: false,
                no_drop: false,
                queue_size: 128,
            },
        );
        // `Box<dyn Ufile>` is not `Debug`, so we can't `unwrap_err()`.
        match res {
            Err(e) => assert_eq!(e.kind(), io::ErrorKind::NotFound),
            Ok(_) => panic!("expected NotFound, got Ok"),
        }
    }

    #[cfg(feature = "virtio-blk-vmdk")]
    #[test]
    fn vmdk_backend_opens_descriptor() {
        // Build a flat-extent VMDK pair on disk: descriptor + extent.
        let extent_size: u64 = 1024 * 1024;
        let extent_file = TempFile::new().unwrap();
        extent_file.as_file().set_len(extent_size).unwrap();
        let extent_path = extent_file.as_path().to_str().unwrap().to_string();
        let extent_sectors = extent_size / 512;

        let descriptor_file = TempFile::new().unwrap();
        let descriptor = format!(
            r#"# Disk DescriptorFile
version=1
CID=fffffffe
parentCID=ffffffff
createType="monolithicFlat"

# Extent description
RW {extent_sectors} FLAT "{extent_path}" 0

# The Disk Data Base
#DDB
"#
        );
        descriptor_file
            .as_file()
            .write_all(descriptor.as_bytes())
            .unwrap();

        let backend = build_backend(
            DiskImageFormat::Vmdk,
            BackendOpenParams {
                path: descriptor_file.as_path(),
                read_only: true,
                direct: false,
                no_drop: false,
                queue_size: 128,
            },
        )
        .unwrap();
        assert_eq!(backend.get_capacity(), extent_size);
    }

    #[cfg(not(feature = "virtio-blk-vmdk"))]
    #[test]
    fn vmdk_backend_without_feature_returns_unsupported() {
        let f = TempFile::new().unwrap();
        let res = build_backend(
            DiskImageFormat::Vmdk,
            BackendOpenParams {
                path: f.as_path(),
                read_only: true,
                direct: false,
                no_drop: false,
                queue_size: 128,
            },
        );
        match res {
            Err(e) => assert_eq!(e.kind(), io::ErrorKind::Unsupported),
            Ok(_) => panic!("expected Unsupported, got Ok"),
        }
    }
}
