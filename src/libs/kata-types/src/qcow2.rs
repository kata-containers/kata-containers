// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
//! qcow2 backing-chain description for direct-assigned volumes.
//!
//! The CSI driver owns image creation. Kata only validates and attaches what it
//! is handed: a writable (unless the mount is read-only) qcow2 top image,
//! optionally layered on top of one or more read-only backing images.
//!
//! The chain is carried in the `metadata` map of `mountInfo.json`:
//!
//! ```json
//! "device": "/var/lib/csi/vol1/top.qcow2",
//! "metadata": {
//!   "io.katacontainers.volume.format": "qcow2",
//!   "io.katacontainers.volume.backing-chain": "[{\"path\":\"/dev/mapper/base\",\"format\":\"raw\"},
//!                      {\"path\":\"/var/lib/csi/l1.qcow2\",\"format\":\"qcow2\"}]",
//!   "io.katacontainers.volume.discard": "unmap"
//! }
//! ```
//!
//! The chain is ordered base first: `backing[0]` is the base image and the last
//! entry is the layer directly below the top image. The format is never probed
//! from image contents, it is always taken from this declaration.

use crate::block_source::open_block_source;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{File, Metadata};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileExt, FileTypeExt, MetadataExt};

/// `metadata` key holding the format of the volume's top image. The keys are
/// namespaced because `metadata` is free-form and CSI drivers may already use
/// short names such as `format` for their own purposes.
pub const DIRECT_VOLUME_FORMAT_KEY: &str = "io.katacontainers.volume.format";
/// `metadata` key holding the JSON encoded backing chain.
pub const DIRECT_VOLUME_BACKING_CHAIN_KEY: &str = "io.katacontainers.volume.backing-chain";
/// `metadata` key selecting what QEMU does with guest discard requests:
/// `"ignore"` (the default) or `"unmap"`.
pub const DIRECT_VOLUME_DISCARD_KEY: &str = "io.katacontainers.volume.discard";
/// Maximum number of backing images below the top image.
/// Raising it later breaks nobody, lowering it would.
pub const MAX_BACKING_CHAIN_DEPTH: usize = 4;

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

    /// Check that the header fits its place in a declared chain.
    ///
    /// `expect_backing` is whether the declared chain puts an image below this
    /// one. QEMU is told the backing images explicitly, so a backing file name
    /// stored in the header is never resolved. The header must still agree
    /// with the declared chain, otherwise reads of unallocated clusters would
    /// silently return zeroes instead of base data. External data files are
    /// rejected because they would make QEMU open yet another host path.
    pub fn check_layer(&self, expect_backing: bool) -> Result<()> {
        match (self.has_backing_file, expect_backing) {
            (true, false) => {
                return Err(anyhow!(
                    "qcow2 header names a backing file but the declared chain has none below this image"
                ))
            }
            (false, true) => {
                return Err(anyhow!(
                    "qcow2 header has no backing file but the declared chain expects one"
                ))
            }
            _ => {}
        }
        if self.incompatible_features & QCOW2_INCOMPAT_DATA_FILE != 0 {
            return Err(anyhow!("qcow2 external data files are not supported"));
        }
        Ok(())
    }
}

/// Format of a single image in the chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    /// Raw image, file or block device. Only valid as the base.
    Raw,
    /// qcow2 image.
    Qcow2,
}

/// A read-only image below the top image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackingImage {
    /// Host path of the image.
    pub path: String,
    /// Declared format of the image.
    pub format: ImageFormat,
}

/// What QEMU does with discard requests the guest sends to the top image.
///
/// Whether the guest sends any is up to the guest: the `discard` mount option
/// or a periodic `fstrim`. Backing images are read-only and never discarded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DiscardMode {
    /// Accept discards but keep the space allocated. This is QEMU's own
    /// default, and the right choice for an image the CSI preallocated.
    #[default]
    Ignore,
    /// Free the discarded clusters, punching holes in the host file, so a thin
    /// image shrinks again when the guest deletes data.
    Unmap,
}

/// A qcow2 volume: the backing chain below its top image, ordered base first,
/// and how discards to the top image are handled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Qcow2Chain {
    /// Read-only images, `backing[0]` being the base.
    pub backing: Vec<BackingImage>,
    /// Handling of guest discard requests.
    pub discard: DiscardMode,
}

/// Parse the qcow2 description of a direct volume out of its `metadata`.
///
/// Returns `Ok(None)` when the volume is not qcow2 (no `format` key, or
/// `raw`), which is the pre-existing behaviour. `top_path` is the path of the
/// top image (`DirectVolumeMountInfo::device`), used to reject cycles.
pub fn parse_qcow2_chain(
    metadata: &HashMap<String, String>,
    top_path: &str,
) -> Result<Option<Qcow2Chain>> {
    let chain = metadata.get(DIRECT_VOLUME_BACKING_CHAIN_KEY);
    let discard = metadata.get(DIRECT_VOLUME_DISCARD_KEY);
    match metadata.get(DIRECT_VOLUME_FORMAT_KEY).map(String::as_str) {
        None | Some("raw") => {
            // Reject rather than ignore keys that only apply to qcow2, so a CSI
            // driver never believes they took effect on a raw volume.
            for (key, value) in [
                (DIRECT_VOLUME_BACKING_CHAIN_KEY, chain),
                (DIRECT_VOLUME_DISCARD_KEY, discard),
            ] {
                if value.is_some() {
                    return Err(anyhow!(
                        "metadata {:?} requires {:?} to be \"qcow2\"",
                        key,
                        DIRECT_VOLUME_FORMAT_KEY
                    ));
                }
            }
            return Ok(None);
        }
        Some("qcow2") => {}
        Some(other) => return Err(anyhow!("unsupported volume format {:?}", other)),
    }

    let backing: Vec<BackingImage> = match chain {
        Some(s) => serde_json::from_str(s).context("parse backing-chain metadata")?,
        None => Vec::new(),
    };
    let discard = match discard.map(String::as_str) {
        None | Some("ignore") => DiscardMode::Ignore,
        Some("unmap") => DiscardMode::Unmap,
        Some(other) => {
            return Err(anyhow!(
                "metadata {:?} must be \"unmap\" or \"ignore\", not {:?}",
                DIRECT_VOLUME_DISCARD_KEY,
                other
            ))
        }
    };
    let chain = Qcow2Chain { backing, discard };
    chain.validate(top_path)?;
    Ok(Some(chain))
}

impl Qcow2Chain {
    /// Validate the chain against the path of the top image.
    fn validate(&self, top_path: &str) -> Result<()> {
        if self.backing.len() > MAX_BACKING_CHAIN_DEPTH {
            return Err(anyhow!(
                "backing chain has {} images, the limit is {}",
                self.backing.len(),
                MAX_BACKING_CHAIN_DEPTH
            ));
        }

        let mut seen = HashSet::new();
        seen.insert(top_path);
        for (i, image) in self.backing.iter().enumerate() {
            if !image.path.starts_with('/') {
                return Err(anyhow!(
                    "backing image path {:?} is not absolute",
                    image.path
                ));
            }
            // Only the base may be raw: a raw image cannot have a backing file.
            if i > 0 && image.format != ImageFormat::Qcow2 {
                return Err(anyhow!(
                    "backing image {:?} sits above the base and must be qcow2",
                    image.path
                ));
            }
            if !seen.insert(image.path.as_str()) {
                return Err(anyhow!(
                    "image {:?} appears more than once in the chain",
                    image.path
                ));
            }
        }
        Ok(())
    }

    /// Open the top image at `top_path` and every backing image, and check
    /// them against the chain.
    ///
    /// Only the top image can be writable, and only unless `is_readonly`.
    /// Backing images are shared between pods, so they are always opened
    /// read-only. Every image is opened with `O_DIRECT` if `is_direct`.
    ///
    /// All images are open and checked before this returns, so a caller that
    /// hands them to a hypervisor one by one never hands over part of a
    /// rejected volume.
    pub fn open<'a>(
        &'a self,
        top_path: &'a str,
        is_readonly: bool,
        is_direct: bool,
    ) -> Result<OpenedQcow2Volume<'a>> {
        // Like the chain, the list is ordered base first, so the top image
        // comes last.
        let mut images = Vec::with_capacity(self.backing.len() + 1);
        for image in &self.backing {
            images.push(OpenedImage::open(
                &image.path,
                image.format,
                true,
                is_direct,
            )?);
        }
        images.push(OpenedImage::open(
            top_path,
            ImageFormat::Qcow2,
            is_readonly,
            is_direct,
        )?);

        ensure_distinct_files(&images)?;
        for (i, image) in images.iter().enumerate() {
            // Every image but the base has an image below it, so its qcow2
            // header must name a backing file. A raw base has no header to
            // check.
            if image.format == ImageFormat::Qcow2 {
                image.check_qcow2_header(i > 0)?;
            }
        }

        let top = images.pop().context("qcow2 volume without a top image")?;
        Ok(OpenedQcow2Volume {
            top,
            backing: images,
        })
    }
}

/// One image of a qcow2 volume, opened by the shim.
#[derive(Debug)]
pub struct OpenedImage<'a> {
    /// Host path, as declared in `mountInfo.json`. Only used in errors: the
    /// image is read through `file` from here on.
    pub path: &'a str,
    /// Declared format, never probed from the contents.
    pub format: ImageFormat,
    /// The opened image.
    pub file: File,
    /// Metadata of the opened `file`, not of `path`, which could since point
    /// somewhere else.
    pub metadata: Metadata,
}

impl<'a> OpenedImage<'a> {
    fn open(
        path: &'a str,
        format: ImageFormat,
        is_readonly: bool,
        is_direct: bool,
    ) -> Result<Self> {
        // A qcow2 image must be a regular file. Only a raw base may be a block
        // device, such as a device-mapper volume.
        let regular_file_only = format == ImageFormat::Qcow2;
        let (file, metadata) = open_block_source(path, is_readonly, is_direct, regular_file_only)?;
        Ok(Self {
            path,
            format,
            file,
            metadata,
        })
    }

    /// Check the qcow2 header against the declared chain. `expect_backing` is
    /// whether an image sits below this one.
    fn check_qcow2_header(&self, expect_backing: bool) -> Result<()> {
        Qcow2Header::read(&self.file)
            .and_then(|header| header.check_layer(expect_backing))
            .with_context(|| format!("check qcow2 header of {}", self.path))
    }
}

/// A qcow2 volume whose images are all open and checked, see
/// [`Qcow2Chain::open`].
#[derive(Debug)]
pub struct OpenedQcow2Volume<'a> {
    /// The top image.
    pub top: OpenedImage<'a>,
    /// The read-only images below the top image, base first.
    pub backing: Vec<OpenedImage<'a>>,
}

/// Fail if two of the opened `images` are the same file.
///
/// [`parse_qcow2_chain`] only rejects duplicate paths. This compares the
/// opened files, so it also catches one file reached through two paths: a
/// symlink, a hard link, a bind mount, or `/dev/dm-N` and `/dev/mapper/...`.
/// Otherwise one file could be both the writable top and a read-only backing
/// image.
fn ensure_distinct_files(images: &[OpenedImage]) -> Result<()> {
    let mut seen = HashMap::new();
    for image in images {
        // Block devices are compared by device number, since two device nodes
        // for one disk are different inodes.
        let identity = if image.metadata.file_type().is_block_device() {
            (u64::MAX, image.metadata.rdev())
        } else {
            (image.metadata.dev(), image.metadata.ino())
        };
        if let Some(first) = seen.insert(identity, image.path) {
            return Err(anyhow!("{} is the same file as {}", image.path, first));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::fcntl::{fcntl, FcntlArg, OFlag};
    use std::fs::OpenOptions;
    use std::os::unix::fs::{symlink, OpenOptionsExt};
    use std::path::Path;
    use tempfile::tempdir;

    const TOP: &str = "/csi/top.qcow2";
    const FMT: &str = DIRECT_VOLUME_FORMAT_KEY;
    const CHAIN: &str = DIRECT_VOLUME_BACKING_CHAIN_KEY;
    const DISCARD: &str = DIRECT_VOLUME_DISCARD_KEY;

    fn image(path: &str, format: ImageFormat) -> BackingImage {
        BackingImage {
            path: path.to_string(),
            format,
        }
    }

    fn qcow2(backing: Vec<BackingImage>, discard: DiscardMode) -> Option<Qcow2Chain> {
        Some(Qcow2Chain { backing, discard })
    }

    fn layers(count: usize) -> Vec<BackingImage> {
        (0..count)
            .map(|i| image(&format!("/l{i}"), ImageFormat::Qcow2))
            .collect()
    }

    #[test]
    fn test_parse_qcow2_chain() {
        #[derive(Debug)]
        struct TestData {
            name: &'static str,
            metadata: Vec<(&'static str, String)>,
            // Ok: the parsed volume. Err: text the error must contain.
            result: std::result::Result<Option<Qcow2Chain>, String>,
        }
        let case = |name, metadata: &[(&'static str, &str)], result| TestData {
            name,
            metadata: metadata.iter().map(|(k, v)| (*k, v.to_string())).collect(),
            result,
        };
        let requires_qcow2 =
            |key: &str| Err(format!("metadata {key:?} requires {FMT:?} to be \"qcow2\""));
        let err = |text: &str| Err(text.to_string());
        let max = serde_json::to_string(&layers(MAX_BACKING_CHAIN_DEPTH)).unwrap();
        let too_deep = serde_json::to_string(&layers(MAX_BACKING_CHAIN_DEPTH + 1)).unwrap();
        let unusual = "/csi/my vol/b\u{e4}se \u{2713}.img";
        let unusual_chain = format!(r#"[{{"path":"{unusual}","format":"raw"}}]"#);
        let top_in_chain = format!(r#"[{{"path":"{TOP}","format":"raw"}}]"#);

        let tests = vec![
            // Not qcow2: the pre-existing raw behaviour.
            case("no metadata", &[], Ok(None)),
            case("explicit raw", &[(FMT, "raw")], Ok(None)),
            // A CSI driver's own short keys are not ours, even with our values.
            case(
                "foreign short keys",
                &[
                    ("format", "qcow2"),
                    ("backing-chain", "[]"),
                    ("discard", "on"),
                ],
                Ok(None),
            ),
            // qcow2.
            case(
                "thin qcow2",
                &[(FMT, "qcow2")],
                Ok(qcow2(vec![], DiscardMode::Ignore)),
            ),
            case(
                "explicit empty chain",
                &[(FMT, "qcow2"), (CHAIN, "[]")],
                Ok(qcow2(vec![], DiscardMode::Ignore)),
            ),
            case(
                "multi-layer chain keeps its order",
                &[
                    (FMT, "qcow2"),
                    (
                        CHAIN,
                        r#"[{"path":"/dev/base","format":"raw"},
                            {"path":"/csi/l1.qcow2","format":"qcow2"},
                            {"path":"/csi/l2.qcow2","format":"qcow2"}]"#,
                    ),
                ],
                Ok(qcow2(
                    vec![
                        image("/dev/base", ImageFormat::Raw),
                        image("/csi/l1.qcow2", ImageFormat::Qcow2),
                        image("/csi/l2.qcow2", ImageFormat::Qcow2),
                    ],
                    DiscardMode::Ignore,
                )),
            ),
            case(
                "qcow2 base",
                &[
                    (FMT, "qcow2"),
                    (CHAIN, r#"[{"path":"/b","format":"qcow2"}]"#),
                ],
                Ok(qcow2(
                    vec![image("/b", ImageFormat::Qcow2)],
                    DiscardMode::Ignore,
                )),
            ),
            case(
                "chain at the depth limit",
                &[(FMT, "qcow2"), (CHAIN, &max)],
                Ok(qcow2(layers(MAX_BACKING_CHAIN_DEPTH), DiscardMode::Ignore)),
            ),
            case(
                "path with spaces and unicode",
                &[(FMT, "qcow2"), (CHAIN, &unusual_chain)],
                Ok(qcow2(
                    vec![image(unusual, ImageFormat::Raw)],
                    DiscardMode::Ignore,
                )),
            ),
            case(
                "discard unmap",
                &[(FMT, "qcow2"), (DISCARD, "unmap")],
                Ok(qcow2(vec![], DiscardMode::Unmap)),
            ),
            case(
                "discard ignore",
                &[(FMT, "qcow2"), (DISCARD, "ignore")],
                Ok(qcow2(vec![], DiscardMode::Ignore)),
            ),
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
            // qcow2-only keys on a raw volume.
            case(
                "chain without format",
                &[(CHAIN, "[]")],
                requires_qcow2(CHAIN),
            ),
            case(
                "chain on raw",
                &[(FMT, "raw"), (CHAIN, "[]")],
                requires_qcow2(CHAIN),
            ),
            case(
                "discard without format",
                &[(DISCARD, "unmap")],
                requires_qcow2(DISCARD),
            ),
            case(
                "discard on raw",
                &[(FMT, "raw"), (DISCARD, "ignore")],
                requires_qcow2(DISCARD),
            ),
            // Chain JSON.
            case(
                "empty chain string",
                &[(FMT, "qcow2"), (CHAIN, "")],
                err("parse backing-chain metadata: EOF while parsing"),
            ),
            case(
                "malformed chain",
                &[(FMT, "qcow2"), (CHAIN, "nope")],
                err("parse backing-chain metadata: expected ident"),
            ),
            case(
                "chain is not an array",
                &[(FMT, "qcow2"), (CHAIN, r#"{"path":"/a","format":"raw"}"#)],
                err("invalid type: map, expected a sequence"),
            ),
            case(
                "unknown image field",
                &[
                    (FMT, "qcow2"),
                    (CHAIN, r#"[{"path":"/a","format":"raw","x":1}]"#),
                ],
                err("unknown field `x`"),
            ),
            case(
                "missing image format",
                &[(FMT, "qcow2"), (CHAIN, r#"[{"path":"/a"}]"#)],
                err("missing field `format`"),
            ),
            case(
                "unknown image format",
                &[(FMT, "qcow2"), (CHAIN, r#"[{"path":"/a","format":"vhd"}]"#)],
                err("unknown variant `vhd`"),
            ),
            // Chain rules.
            case(
                "relative path",
                &[(FMT, "qcow2"), (CHAIN, r#"[{"path":"a","format":"raw"}]"#)],
                err(r#"backing image path "a" is not absolute"#),
            ),
            case(
                "empty path",
                &[(FMT, "qcow2"), (CHAIN, r#"[{"path":"","format":"raw"}]"#)],
                err(r#"backing image path "" is not absolute"#),
            ),
            case(
                "raw above the base",
                &[
                    (FMT, "qcow2"),
                    (
                        CHAIN,
                        r#"[{"path":"/a","format":"raw"},{"path":"/b","format":"raw"}]"#,
                    ),
                ],
                err(r#"backing image "/b" sits above the base and must be qcow2"#),
            ),
            case(
                "duplicate image",
                &[
                    (FMT, "qcow2"),
                    (
                        CHAIN,
                        r#"[{"path":"/a","format":"raw"},{"path":"/a","format":"qcow2"}]"#,
                    ),
                ],
                err(r#"image "/a" appears more than once in the chain"#),
            ),
            case(
                "top image in its own chain",
                &[(FMT, "qcow2"), (CHAIN, &top_in_chain)],
                err(&format!(
                    "image {TOP:?} appears more than once in the chain"
                )),
            ),
            case(
                "chain over the depth limit",
                &[(FMT, "qcow2"), (CHAIN, &too_deep)],
                err("backing chain has 5 images, the limit is 4"),
            ),
            // Discard.
            case(
                "unknown discard",
                &[(FMT, "qcow2"), (DISCARD, "on")],
                err(r#"must be "unmap" or "ignore", not "on""#),
            ),
            case(
                "empty discard",
                &[(FMT, "qcow2"), (DISCARD, "")],
                err(r#"must be "unmap" or "ignore", not """#),
            ),
            case(
                "upper-case discard",
                &[(FMT, "qcow2"), (DISCARD, "Unmap")],
                err(r#"must be "unmap" or "ignore", not "Unmap""#),
            ),
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}] {}: {:?}", i, d.name, d);
            let metadata = d
                .metadata
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();

            let result = parse_qcow2_chain(&metadata, TOP);
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
    fn test_qcow2_header_check_layer() {
        let names =
            "qcow2 header names a backing file but the declared chain has none below this image";
        let missing = "qcow2 header has no backing file but the declared chain expects one";
        let data_file = "qcow2 external data files are not supported";

        // (name, header, expect_backing, expected error)
        let tests = [
            ("base without backing", header(3, false, 0), false, None),
            ("layer with backing", header(3, true, 0), true, None),
            ("v2 layer with backing", header(2, true, 0), true, None),
            (
                "unrelated feature bits",
                header(3, true, !QCOW2_INCOMPAT_DATA_FILE),
                true,
                None,
            ),
            (
                "backing not declared",
                header(3, true, 0),
                false,
                Some(names),
            ),
            (
                "declared backing missing",
                header(3, false, 0),
                true,
                Some(missing),
            ),
            (
                "data file",
                header(3, false, QCOW2_INCOMPAT_DATA_FILE),
                false,
                Some(data_file),
            ),
            (
                "data file on a layer",
                header(3, true, QCOW2_INCOMPAT_DATA_FILE),
                true,
                Some(data_file),
            ),
        ];

        for (i, (name, header, expect_backing, expected)) in tests.iter().enumerate() {
            let result = header.check_layer(*expect_backing);
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

    fn chain(layers: &[(&str, ImageFormat)]) -> Qcow2Chain {
        Qcow2Chain {
            backing: layers
                .iter()
                .map(|(path, format)| image(path, *format))
                .collect(),
            ..Default::default()
        }
    }

    fn open_flags(file: &File) -> OFlag {
        OFlag::from_bits_truncate(fcntl(file, FcntlArg::F_GETFL).unwrap())
    }

    #[test]
    fn test_qcow2_chain_open() {
        let dir = tempdir().unwrap();
        let base = write_image(dir.path(), "base.raw", &[0; 8192]);
        let mid = qcow2_image(dir.path(), "mid.qcow2", 3, true, 0);
        let top = qcow2_image(dir.path(), "top.qcow2", 3, true, 0);
        let thin = qcow2_image(dir.path(), "thin.qcow2", 3, false, 0);

        // (name, top, backing, is_readonly, is_direct)
        let tests = [
            ("thin, writable", &thin, vec![], false, false),
            ("thin, read-only", &thin, vec![], true, false),
            (
                "chain, writable",
                &top,
                vec![
                    (base.as_str(), ImageFormat::Raw),
                    (mid.as_str(), ImageFormat::Qcow2),
                ],
                false,
                false,
            ),
            // O_DIRECT fails the small header read, which must still work, and
            // every image must keep O_DIRECT for the hypervisor.
            (
                "chain, O_DIRECT",
                &top,
                vec![
                    (base.as_str(), ImageFormat::Raw),
                    (mid.as_str(), ImageFormat::Qcow2),
                ],
                false,
                true,
            ),
        ];

        for (i, (name, top, layers, is_readonly, is_direct)) in tests.iter().enumerate() {
            let chain = chain(layers);
            let result = chain.open(top, *is_readonly, *is_direct);
            let msg = format!("test[{}] {}: result: {:?}", i, name, result);
            let volume = result.unwrap_or_else(|_| panic!("{}", msg));

            assert_eq!(volume.top.path, top.as_str(), "{}", msg);
            assert_eq!(volume.top.format, ImageFormat::Qcow2, "{}", msg);
            let access = if *is_readonly {
                OFlag::O_RDONLY
            } else {
                OFlag::O_RDWR
            };
            let top_flags = open_flags(&volume.top.file);
            assert_eq!(top_flags & OFlag::O_ACCMODE, access, "{}", msg);
            assert_eq!(top_flags.contains(OFlag::O_DIRECT), *is_direct, "{}", msg);

            // Backing images keep the chain order and are always read-only.
            let backing: Vec<_> = volume.backing.iter().map(|b| (b.path, b.format)).collect();
            assert_eq!(&backing, layers, "{}", msg);
            for image in &volume.backing {
                let flags = open_flags(&image.file);
                assert_eq!(flags & OFlag::O_ACCMODE, OFlag::O_RDONLY, "{}", msg);
                assert_eq!(flags.contains(OFlag::O_DIRECT), *is_direct, "{}", msg);
            }
        }
    }

    #[test]
    fn test_qcow2_chain_open_checks_every_header() {
        let dir = tempdir().unwrap();
        let base = write_image(dir.path(), "base.raw", &[0; 4096]);
        let short = write_image(dir.path(), "short.qcow2", &QCOW2_MAGIC);
        let img = |name, version, backing, incompat| {
            qcow2_image(dir.path(), name, version, backing, incompat)
        };
        let thin = img("thin.qcow2", 3, false, 0);
        let layer = img("layer.qcow2", 3, true, 0);
        let v2_base = img("v2-base.qcow2", 2, false, 0);
        let based_base = img("based-base.qcow2", 3, true, 0);
        let data_file = img("data-file.qcow2", 3, false, QCOW2_INCOMPAT_DATA_FILE);
        let v9 = img("v9.qcow2", 9, false, 0);
        let mid = img("mid.qcow2", 3, true, 0);
        let flat_mid = img("flat-mid.qcow2", 3, false, 0);

        let names =
            "qcow2 header names a backing file but the declared chain has none below this image";
        let missing = "qcow2 header has no backing file but the declared chain expects one";
        let raw = ImageFormat::Raw;
        let qcow2 = ImageFormat::Qcow2;

        struct TestData<'a> {
            name: &'a str,
            top: &'a str,
            layers: Vec<(&'a str, ImageFormat)>,
            // The image expected to be rejected, and why.
            error: Option<(&'a str, &'a str)>,
        }
        let case = |name, top, layers, error| TestData {
            name,
            top,
            layers,
            error,
        };

        let tests = vec![
            case("thin top without a chain", &thin, vec![], None),
            case("top over a raw base", &layer, vec![(&base, raw)], None),
            case(
                "top over a v2 qcow2 base",
                &layer,
                vec![(&v2_base, qcow2)],
                None,
            ),
            case(
                "top over a middle layer",
                &layer,
                vec![(&base, raw), (&mid, qcow2)],
                None,
            ),
            case(
                "chain declared, top has no backing",
                &thin,
                vec![(&base, raw)],
                Some((&thin, missing)),
            ),
            case(
                "top has backing, no chain declared",
                &layer,
                vec![],
                Some((&layer, names)),
            ),
            case(
                "qcow2 base has backing",
                &layer,
                vec![(&based_base, qcow2)],
                Some((&based_base, names)),
            ),
            case(
                "middle layer has no backing",
                &layer,
                vec![(&base, raw), (&flat_mid, qcow2)],
                Some((&flat_mid, missing)),
            ),
            case(
                "external data file",
                &data_file,
                vec![],
                Some((&data_file, "qcow2 external data files are not supported")),
            ),
            case(
                "unsupported version",
                &v9,
                vec![],
                Some((&v9, "unsupported qcow2 version 9")),
            ),
            case(
                "raw image declared qcow2",
                &base,
                vec![],
                Some((&base, "no qcow2 magic")),
            ),
            case(
                "image shorter than the header",
                &short,
                vec![],
                Some((&short, "read qcow2 header: failed to fill whole buffer")),
            ),
            // A raw base is never read, whatever it contains.
            case(
                "raw base holding a qcow2 header",
                &layer,
                vec![(&data_file, raw)],
                None,
            ),
        ];

        for (i, d) in tests.iter().enumerate() {
            let result = chain(&d.layers).open(d.top, true, false).map(|_| ());
            let msg = format!("test[{}] {}: result: {:?}", i, d.name, result);
            match d.error {
                None => assert!(result.is_ok(), "{}", msg),
                Some((image, error)) => {
                    let actual = format!("{:#}", result.unwrap_err());
                    let expected = format!("check qcow2 header of {}: {}", image, error);
                    assert!(actual.contains(&expected), "{}", msg);
                }
            }
        }
    }

    #[test]
    fn test_qcow2_chain_open_needs_a_regular_file_top() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap();
        // (name, top image)
        let tests = [("character device", "/dev/null"), ("directory", dir_path)];

        for (i, (name, top)) in tests.iter().enumerate() {
            let result = Qcow2Chain::default().open(top, true, false).map(|_| ());
            let msg = format!("test[{}] {}: result: {:?}", i, name, result);
            let expected = format!("block source {} is not a regular file", top);
            assert_eq!(format!("{:#}", result.unwrap_err()), expected, "{}", msg);
        }
    }

    #[test]
    fn test_qcow2_chain_open_rejects_the_same_file_through_another_path() {
        let dir = tempdir().unwrap();
        let path = |name: &str| dir.path().join(name).to_str().unwrap().to_string();
        let base = write_image(dir.path(), "base.raw", &[0; 4096]);
        let base_symlink = path("base-symlink.raw");
        symlink(&base, &base_symlink).unwrap();
        let mid = qcow2_image(dir.path(), "mid.qcow2", 3, true, 0);
        let mid_hard_link = path("mid-hard-link.qcow2");
        std::fs::hard_link(&mid, &mid_hard_link).unwrap();
        let top = qcow2_image(dir.path(), "top.qcow2", 3, true, 0);
        let top_symlink = path("top-symlink.qcow2");
        symlink(&top, &top_symlink).unwrap();

        struct TestData<'a> {
            name: &'a str,
            layers: Vec<(&'a str, ImageFormat)>,
            // (image, the image it duplicates)
            duplicate: Option<(&'a str, &'a str)>,
        }
        fn raw(path: &str) -> (&str, ImageFormat) {
            (path, ImageFormat::Raw)
        }
        fn qcow2(path: &str) -> (&str, ImageFormat) {
            (path, ImageFormat::Qcow2)
        }
        let tests = [
            TestData {
                name: "distinct images",
                layers: vec![raw(&base), qcow2(&mid)],
                duplicate: None,
            },
            TestData {
                name: "base through a symlink",
                layers: vec![raw(&base), qcow2(&base_symlink)],
                duplicate: Some((&base_symlink, &base)),
            },
            TestData {
                name: "middle layer through a hard link",
                layers: vec![raw(&base), qcow2(&mid), qcow2(&mid_hard_link)],
                duplicate: Some((&mid_hard_link, &mid)),
            },
            TestData {
                name: "writable top as a backing image",
                layers: vec![raw(&base), qcow2(&top_symlink)],
                // Images are compared base first, so the top comes last.
                duplicate: Some((&top, &top_symlink)),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let result = chain(&d.layers).open(&top, false, false).map(|_| ());
            let msg = format!("test[{}] {}: result: {:?}", i, d.name, result);
            match d.duplicate {
                None => assert!(result.is_ok(), "{}", msg),
                Some((image, first)) => {
                    let expected = format!("{} is the same file as {}", image, first);
                    assert_eq!(format!("{:#}", result.unwrap_err()), expected, "{}", msg);
                }
            }
        }
    }
}
