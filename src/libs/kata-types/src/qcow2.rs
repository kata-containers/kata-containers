// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
//! qcow2 description for direct-assigned volumes.
//!
//! The CSI driver owns image creation. Kata only validates and attaches what it
//! is handed: a qcow2 image, writable unless the mount is read-only.
//!
//! The format is carried in the `metadata` map of `mountInfo.json`:
//!
//! ```json
//! "device": "/var/lib/csi/vol1/disk.qcow2",
//! "metadata": {
//!   "io.katacontainers.volume.format": "qcow2"
//! }
//! ```
//!
//! The format is never probed from image contents, it is always taken from
//! this declaration.

use crate::block_source::open_block_source;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::fs::{File, Metadata};
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileExt;

/// `metadata` key holding the format of the volume's image. The keys are
/// namespaced because `metadata` is free-form and CSI drivers may already use
/// short names such as `format` for their own purposes.
pub const DIRECT_VOLUME_FORMAT_KEY: &str = "io.katacontainers.volume.format";

/// qcow2 magic number, "QFI" followed by 0xfb.
pub const QCOW2_MAGIC: [u8; 4] = *b"QFI\xfb";
/// "External data file" bit of the qcow2 incompatible feature flags.
pub const QCOW2_INCOMPAT_DATA_FILE: u64 = 1 << 2;

/// The few qcow2 header fields Kata needs to sanity check an image.
///
/// The on-disk format is specified in the QEMU tree, see
/// <https://www.qemu.org/docs/master/interop/qcow2.html> (source:
/// `docs/interop/qcow2.rst`). All integers are big-endian. The header fields
/// used here are:
///
/// | Offset | Size | Field                                    | Versions |
/// | ------ | ---- | ---------------------------------------- | -------- |
/// | 0      | 4    | magic (`QFI\xfb`)                        | 2, 3     |
/// | 4      | 4    | version                                  | 2, 3     |
/// | 8      | 8    | backing_file_offset (0 = no backing file)| 2, 3     |
/// | 72     | 8    | incompatible_features                    | 3        |
///
/// Version 2 headers end at offset 72, so version 2 images have no feature
/// flags. Kata never reads the backing file *name*, only whether one exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Qcow2Header {
    /// qcow2 version, 2 or 3.
    pub version: u32,
    /// Whether the header records a backing file.
    pub has_backing_file: bool,
    /// Incompatible feature flags, always 0 for version 2.
    pub incompatible_features: u64,
}

impl Qcow2Header {
    /// Number of leading bytes of an image needed by [`Qcow2Header::parse`].
    const PARSE_LEN: usize = 80;

    /// Parse the start of a qcow2 image. `buf` must hold at least
    /// [`Qcow2Header::PARSE_LEN`] bytes.
    fn parse(buf: &[u8]) -> Result<Self> {
        if buf.len() < Self::PARSE_LEN {
            return Err(anyhow!("qcow2 header needs {} bytes", Self::PARSE_LEN));
        }
        if buf[0..4] != QCOW2_MAGIC {
            return Err(anyhow!("no qcow2 magic"));
        }
        let be32 = |o: usize| u32::from_be_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]);
        let be64 = |o: usize| (u64::from(be32(o)) << 32) | u64::from(be32(o + 4));

        let version = be32(4);
        if version != 2 && version != 3 {
            return Err(anyhow!("unsupported qcow2 version {}", version));
        }
        Ok(Self {
            version,
            has_backing_file: be64(8) != 0,
            incompatible_features: if version == 3 { be64(72) } else { 0 },
        })
    }

    /// Read and parse the header of an opened image.
    ///
    /// `file` may be opened with `O_DIRECT`, which fails small unaligned
    /// reads with `EINVAL`. So the header is read from a second, buffered
    /// open of `/proc/self/fd/N`. That opens the same file as `file`, even if
    /// its path now points somewhere else, and leaves `file` unchanged.
    pub fn read(file: &File) -> Result<Self> {
        let reopened = File::open(format!("/proc/self/fd/{}", file.as_raw_fd()))
            .context("reopen image to read its qcow2 header")?;
        let mut buf = [0_u8; Self::PARSE_LEN];
        reopened
            .read_exact_at(&mut buf, 0)
            .context("read qcow2 header")?;
        Self::parse(&buf)
    }

    /// Check that the image is self-contained.
    ///
    /// QEMU is told that the image has no backing, so a backing file name
    /// stored in the header is never resolved. An image that names one is
    /// still rejected: reads of its unallocated clusters would silently return
    /// zeroes instead of the data the image creator expected. External data
    /// files are rejected because they would make QEMU open another host path.
    pub fn check(&self) -> Result<()> {
        if self.has_backing_file {
            return Err(anyhow!("qcow2 header names a backing file"));
        }
        if self.incompatible_features & QCOW2_INCOMPAT_DATA_FILE != 0 {
            return Err(anyhow!("qcow2 external data files are not supported"));
        }
        Ok(())
    }
}

/// A qcow2 volume.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Qcow2Volume {}

/// Parse the qcow2 description of a direct volume out of its `metadata`.
///
/// Returns `Ok(None)` when the volume is not qcow2 (no `format` key, or
/// `raw`), which is the pre-existing behaviour.
pub fn parse_qcow2_volume(metadata: &HashMap<String, String>) -> Result<Option<Qcow2Volume>> {
    match metadata.get(DIRECT_VOLUME_FORMAT_KEY).map(String::as_str) {
        None | Some("raw") => Ok(None),
        Some("qcow2") => Ok(Some(Qcow2Volume {})),
        Some(other) => Err(anyhow!("unsupported volume format {:?}", other)),
    }
}

impl Qcow2Volume {
    /// Open the image at `path` and check its header.
    ///
    /// The image is opened read-only if `is_readonly`, and with `O_DIRECT` if
    /// `is_direct`.
    pub fn open<'a>(
        &self,
        path: &'a str,
        is_readonly: bool,
        is_direct: bool,
    ) -> Result<OpenedQcow2Volume<'a>> {
        let image = OpenedImage::open(path, is_readonly, is_direct)?;
        image.check_qcow2_header()?;
        Ok(OpenedQcow2Volume { image })
    }
}

/// The image of a qcow2 volume, opened by the shim.
#[derive(Debug)]
pub struct OpenedImage<'a> {
    /// Host path, as declared in `mountInfo.json`. Only used in errors: the
    /// image is read through `file` from here on.
    pub path: &'a str,
    /// The opened image.
    pub file: File,
    /// Metadata of the opened `file`, not of `path`, which could since point
    /// somewhere else.
    pub metadata: Metadata,
}

impl<'a> OpenedImage<'a> {
    fn open(path: &'a str, is_readonly: bool, is_direct: bool) -> Result<Self> {
        // A qcow2 image must be a regular file.
        let (file, metadata) = open_block_source(path, is_readonly, is_direct, true)?;
        Ok(Self {
            path,
            file,
            metadata,
        })
    }

    /// Check the qcow2 header.
    fn check_qcow2_header(&self) -> Result<()> {
        Qcow2Header::read(&self.file)
            .and_then(|header| header.check())
            .with_context(|| format!("check qcow2 header of {}", self.path))
    }
}

/// A qcow2 volume whose image is open and checked, see [`Qcow2Volume::open`].
#[derive(Debug)]
pub struct OpenedQcow2Volume<'a> {
    /// The image.
    pub image: OpenedImage<'a>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::fcntl::{fcntl, FcntlArg, OFlag};
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::Path;
    use tempfile::tempdir;

    const FMT: &str = DIRECT_VOLUME_FORMAT_KEY;

    #[test]
    fn test_parse_qcow2_volume() {
        #[derive(Debug)]
        struct TestData {
            name: &'static str,
            metadata: Vec<(&'static str, String)>,
            // Ok: the parsed volume. Err: text the error must contain.
            result: std::result::Result<Option<Qcow2Volume>, String>,
        }
        let case = |name, metadata: &[(&'static str, &str)], result| TestData {
            name,
            metadata: metadata.iter().map(|(k, v)| (*k, v.to_string())).collect(),
            result,
        };
        let err = |text: &str| Err(text.to_string());

        let tests = [
            // Not qcow2: the pre-existing raw behaviour.
            case("no metadata", &[], Ok(None)),
            case("explicit raw", &[(FMT, "raw")], Ok(None)),
            // A CSI driver's own short keys are not ours, even with our values.
            case("foreign short keys", &[("format", "qcow2")], Ok(None)),
            // qcow2.
            case("qcow2", &[(FMT, "qcow2")], Ok(Some(Qcow2Volume {}))),
            // Format.
            case(
                "unknown format",
                &[(FMT, "vhd")],
                err(r#"unsupported volume format "vhd""#),
            ),
            case(
                "empty format",
                &[(FMT, "")],
                err(r#"unsupported volume format """#),
            ),
            case(
                "upper-case format",
                &[(FMT, "QCOW2")],
                err(r#"unsupported volume format "QCOW2""#),
            ),
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}] {}: {:?}", i, d.name, d);
            let metadata = d
                .metadata
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();

            let result = parse_qcow2_volume(&metadata);
            let msg = format!("{}, result: {:?}", msg, result);

            match &d.result {
                Ok(expected) => assert_eq!(result.as_ref().ok(), Some(expected), "{}", msg),
                Err(expected) => {
                    let actual = format!("{:#}", result.as_ref().unwrap_err());
                    assert!(actual.contains(expected.as_str()), "{}", msg);
                }
            }
        }
    }

    fn header_bytes(version: u32, backing: bool, incompat: u64) -> Vec<u8> {
        let mut h = vec![0_u8; Qcow2Header::PARSE_LEN];
        h[0..4].copy_from_slice(&QCOW2_MAGIC);
        h[4..8].copy_from_slice(&version.to_be_bytes());
        if backing {
            h[8..16].copy_from_slice(&512_u64.to_be_bytes());
        }
        h[72..80].copy_from_slice(&incompat.to_be_bytes());
        h
    }

    fn header(version: u32, has_backing_file: bool, incompatible_features: u64) -> Qcow2Header {
        Qcow2Header {
            version,
            has_backing_file,
            incompatible_features,
        }
    }

    #[test]
    fn test_qcow2_header_parse() {
        let mut no_magic = header_bytes(3, false, 0);
        no_magic[3] = 0xfa;
        let mut longer = header_bytes(3, true, 0);
        longer.resize(4096, 0xff);
        let short = format!("qcow2 header needs {} bytes", Qcow2Header::PARSE_LEN);

        let tests: Vec<(&str, Vec<u8>, std::result::Result<Qcow2Header, String>)> = vec![
            ("v3", header_bytes(3, false, 0), Ok(header(3, false, 0))),
            (
                "v3 with backing",
                header_bytes(3, true, 0),
                Ok(header(3, true, 0)),
            ),
            (
                "v3 with a data file",
                header_bytes(3, false, QCOW2_INCOMPAT_DATA_FILE),
                Ok(header(3, false, QCOW2_INCOMPAT_DATA_FILE)),
            ),
            (
                "v3 with every feature bit",
                header_bytes(3, false, u64::MAX),
                Ok(header(3, false, u64::MAX)),
            ),
            // Version 2 headers end at offset 72: whatever follows is not flags.
            (
                "v2",
                header_bytes(2, true, u64::MAX),
                Ok(header(2, true, 0)),
            ),
            ("trailing image data", longer, Ok(header(3, true, 0))),
            ("empty", vec![], Err(short.clone())),
            (
                "one byte short",
                header_bytes(3, false, 0)[..79].to_vec(),
                Err(short),
            ),
            ("no magic", no_magic, Err("no qcow2 magic".to_string())),
            ("all zeroes", vec![0; 80], Err("no qcow2 magic".to_string())),
            (
                "version 0",
                header_bytes(0, false, 0),
                Err("unsupported qcow2 version 0".to_string()),
            ),
            (
                "version 1",
                header_bytes(1, false, 0),
                Err("unsupported qcow2 version 1".to_string()),
            ),
            (
                "version 4",
                header_bytes(4, false, 0),
                Err("unsupported qcow2 version 4".to_string()),
            ),
            (
                "version u32::MAX",
                header_bytes(u32::MAX, false, 0),
                Err(format!("unsupported qcow2 version {}", u32::MAX)),
            ),
        ];

        for (i, (name, bytes, expected)) in tests.iter().enumerate() {
            let result = Qcow2Header::parse(bytes);
            let msg = format!("test[{}] {}: result: {:?}", i, name, result);
            match expected {
                Ok(expected) => assert_eq!(result.as_ref().ok(), Some(expected), "{}", msg),
                Err(expected) => {
                    let actual = format!("{:#}", result.as_ref().unwrap_err());
                    assert!(actual.contains(expected.as_str()), "{}", msg);
                }
            }
        }
    }

    #[test]
    fn test_qcow2_header_check() {
        let names = "qcow2 header names a backing file";
        let data_file = "qcow2 external data files are not supported";

        // (name, header, expected error)
        let tests = [
            ("v3", header(3, false, 0), None),
            ("v2", header(2, false, 0), None),
            (
                "unrelated feature bits",
                header(3, false, !QCOW2_INCOMPAT_DATA_FILE),
                None,
            ),
            ("backing file", header(3, true, 0), Some(names)),
            (
                "data file",
                header(3, false, QCOW2_INCOMPAT_DATA_FILE),
                Some(data_file),
            ),
        ];

        for (i, (name, header, expected)) in tests.iter().enumerate() {
            let result = header.check();
            let msg = format!("test[{}] {}: result: {:?}", i, name, result);
            match expected {
                None => assert!(result.is_ok(), "{}", msg),
                Some(expected) => {
                    assert_eq!(result.unwrap_err().to_string(), *expected, "{}", msg)
                }
            }
        }
    }

    #[test]
    fn test_qcow2_header_read() {
        let dir = tempdir().unwrap();
        let write = |name: &str, bytes: &[u8]| {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        };
        let mut padded = header_bytes(3, true, 0);
        padded.resize(8192, 0);
        let image = write("image.qcow2", &padded);
        let short = write("short.qcow2", &header_bytes(3, true, 0)[..10]);
        let empty = write("empty.qcow2", &[]);

        // (name, path, O_DIRECT, expected error)
        let tests = [
            ("buffered", &image, false, None),
            // O_DIRECT rejects the small unaligned read itself with EINVAL on
            // most filesystems (tmpfs does not enforce alignment).
            ("O_DIRECT", &image, true, None),
            (
                "shorter than the header",
                &short,
                false,
                Some("read qcow2 header: failed to fill whole buffer"),
            ),
            (
                "empty",
                &empty,
                true,
                Some("read qcow2 header: failed to fill whole buffer"),
            ),
        ];

        for (i, (name, path, direct, expected)) in tests.iter().enumerate() {
            let flags = if *direct { OFlag::O_DIRECT.bits() } else { 0 };
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(flags)
                .open(path)
                .unwrap();

            let result = Qcow2Header::read(&file);
            let msg = format!("test[{}] {}: result: {:?}", i, name, result);
            match expected {
                None => assert_eq!(result.as_ref().ok(), Some(&header(3, true, 0)), "{}", msg),
                Some(expected) => {
                    let actual = format!("{:#}", result.as_ref().unwrap_err());
                    assert!(actual.contains(expected), "{}", msg);
                }
            }
        }
    }

    /// Write a qcow2 image holding only a header, padded so that it can also
    /// be read with `O_DIRECT`.
    fn qcow2_image(dir: &Path, name: &str, version: u32, backing: bool, incompat: u64) -> String {
        let mut bytes = header_bytes(version, backing, incompat);
        bytes.resize(8192, 0);
        write_image(dir, name, &bytes)
    }

    fn write_image(dir: &Path, name: &str, bytes: &[u8]) -> String {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path.to_str().unwrap().to_string()
    }

    fn open_flags(file: &File) -> OFlag {
        OFlag::from_bits_truncate(fcntl(file, FcntlArg::F_GETFL).unwrap())
    }

    #[test]
    fn test_qcow2_volume_open() {
        let dir = tempdir().unwrap();
        let thin = qcow2_image(dir.path(), "thin.qcow2", 3, false, 0);

        // (is_readonly, is_direct)
        let tests = [
            (false, false),
            (true, false),
            // O_DIRECT fails the small header read, which must still work, and
            // the image must keep O_DIRECT for the hypervisor.
            (false, true),
        ];

        for (i, (is_readonly, is_direct)) in tests.iter().enumerate() {
            let result = Qcow2Volume::default().open(&thin, *is_readonly, *is_direct);
            let msg = format!("test[{}]: result: {:?}", i, result);
            let volume = result.unwrap_or_else(|_| panic!("{}", msg));

            assert_eq!(volume.image.path, thin.as_str(), "{}", msg);
            let access = if *is_readonly {
                OFlag::O_RDONLY
            } else {
                OFlag::O_RDWR
            };
            let flags = open_flags(&volume.image.file);
            assert_eq!(flags & OFlag::O_ACCMODE, access, "{}", msg);
            assert_eq!(flags.contains(OFlag::O_DIRECT), *is_direct, "{}", msg);
        }
    }

    #[test]
    fn test_qcow2_volume_open_checks_the_header() {
        let dir = tempdir().unwrap();
        let raw = write_image(dir.path(), "raw.img", &[0; 4096]);
        let short = write_image(dir.path(), "short.qcow2", &QCOW2_MAGIC);
        let img = |name, version, backing, incompat| {
            qcow2_image(dir.path(), name, version, backing, incompat)
        };
        let thin = img("thin.qcow2", 3, false, 0);
        let v2 = img("v2.qcow2", 2, false, 0);
        let backing = img("backing.qcow2", 3, true, 0);
        let data_file = img("data-file.qcow2", 3, false, QCOW2_INCOMPAT_DATA_FILE);
        let v9 = img("v9.qcow2", 9, false, 0);

        let names = "qcow2 header names a backing file";

        // (name, image, expected error)
        let tests = [
            ("v3", &thin, None),
            ("v2", &v2, None),
            ("backing file", &backing, Some(names)),
            (
                "external data file",
                &data_file,
                Some("qcow2 external data files are not supported"),
            ),
            (
                "unsupported version",
                &v9,
                Some("unsupported qcow2 version 9"),
            ),
            ("raw image declared qcow2", &raw, Some("no qcow2 magic")),
            (
                "image shorter than the header",
                &short,
                Some("read qcow2 header: failed to fill whole buffer"),
            ),
        ];

        for (i, (name, image, error)) in tests.iter().enumerate() {
            let result = Qcow2Volume::default().open(image, true, false).map(|_| ());
            let msg = format!("test[{}] {}: result: {:?}", i, name, result);
            match error {
                None => assert!(result.is_ok(), "{}", msg),
                Some(error) => {
                    let actual = format!("{:#}", result.unwrap_err());
                    let expected = format!("check qcow2 header of {}: {}", image, error);
                    assert!(actual.contains(&expected), "{}", msg);
                }
            }
        }
    }

    #[test]
    fn test_qcow2_volume_open_needs_a_regular_file() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap();
        // (name, image)
        let tests = [("character device", "/dev/null"), ("directory", dir_path)];

        for (i, (name, image)) in tests.iter().enumerate() {
            let result = Qcow2Volume::default().open(image, true, false).map(|_| ());
            let msg = format!("test[{}] {}: result: {:?}", i, name, result);
            let expected = format!("block source {} is not a regular file", image);
            assert_eq!(format!("{:#}", result.unwrap_err()), expected, "{}", msg);
        }
    }
}
