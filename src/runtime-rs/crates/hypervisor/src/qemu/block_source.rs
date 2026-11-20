// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Context, Result};
use kata_types::block_source::open_block_source;
use kata_types::qcow2::{ImageFormat, Qcow2Chain};
use kata_types::vmdk::validate_vmdk_layout;
use nix::sys::memfd::{memfd_create, MFdFlags};
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::virtio_blk_modern::BlockSourceFormat;

const BLOCK_FD_OPAQUE_PREFIX: &str = "kata-block:";

pub(super) fn block_fd_opaque(node_name: &str, label: &str) -> String {
    format!("{BLOCK_FD_OPAQUE_PREFIX}{node_name}:{label}")
}

pub(super) fn block_fd_node_name(opaque: &str) -> Option<&str> {
    opaque
        .strip_prefix(BLOCK_FD_OPAQUE_PREFIX)?
        .split_once(':')
        .map(|(node_name, _)| node_name)
        .filter(|node_name| !node_name.is_empty())
}

#[derive(Debug)]
pub(super) struct PreparedBlockSource {
    pub filename: String,
    pub is_regular_file: bool,
    /// Read-only images below the top image, base first. Empty unless the
    /// source is a qcow2 chain.
    pub backing: Vec<PreparedBackingImage>,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedBackingImage {
    pub filename: String,
    pub format: ImageFormat,
    pub is_regular_file: bool,
}

/// Register the opened images of a qcow2 volume with QEMU.
///
/// `Qcow2Chain::open` opens and checks every image before QEMU sees any of
/// them, so a rejected volume leaves nothing behind in QEMU.
fn prepare_qcow2_source<F>(
    path: &str,
    chain: &Qcow2Chain,
    is_readonly: bool,
    is_direct: bool,
    register: &mut F,
) -> Result<PreparedBlockSource>
where
    F: FnMut(File, &str) -> Result<String>,
{
    let volume = chain.open(path, is_readonly, is_direct)?;

    // The label tells the files of one volume apart, so they can be found and
    // removed again, also after a shim restart (see `block_fd_opaque`). The
    // QEMU node tree is built from the returned names, base first like the
    // chain.
    let filename = register(volume.top.file, "block-source")?;
    let mut backing = Vec::with_capacity(volume.backing.len());
    for (i, image) in volume.backing.into_iter().enumerate() {
        backing.push(PreparedBackingImage {
            filename: register(image.file, &format!("backing-{i}"))?,
            format: image.format,
            is_regular_file: image.metadata.is_file(),
        });
    }

    Ok(PreparedBlockSource {
        filename,
        is_regular_file: true,
        backing,
    })
}

fn create_vmdk_descriptor_file(
    descriptor_path: &Path,
    descriptor: &str,
    is_readonly: bool,
    is_direct: bool,
) -> Result<File> {
    let mut writable = if is_direct {
        let descriptor_dir = descriptor_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| anyhow!("QEMU VMDK descriptor path has no parent directory"))?;
        tempfile::tempfile_in(descriptor_dir).with_context(|| {
            format!(
                "create anonymous QEMU VMDK descriptor file in {}",
                descriptor_dir.display()
            )
        })?
    } else {
        let name = CString::new("kata-qemu-vmdk")?;
        let fd = memfd_create(name.as_c_str(), MFdFlags::MFD_CLOEXEC)
            .context("create QEMU VMDK descriptor memfd")?;
        File::from(fd)
    };
    writable
        .write_all(descriptor.as_bytes())
        .context("write anonymous QEMU VMDK descriptor")?;

    // QEMU's fdset lookup requires the descriptor's access and O_DIRECT flags
    // to match the flags requested by the block layer. Reopen the completed
    // anonymous file with the requested access rather than passing its writable
    // construction descriptor.
    let fd_path = format!("/proc/self/fd/{}", writable.as_raw_fd());
    OpenOptions::new()
        .read(true)
        .write(!is_readonly)
        .custom_flags(if is_direct { libc::O_DIRECT } else { 0 })
        .open(&fd_path)
        .context("reopen anonymous QEMU VMDK descriptor read-only")
}

/// Open a block source while the shim still has host privileges and register
/// the resulting descriptors with QEMU. Structured VMDK layouts are serialized
/// once after their backing extents have been registered.
pub(super) fn prepare_block_source<F>(
    path: &str,
    source: &BlockSourceFormat,
    is_readonly: bool,
    is_direct: bool,
    mut register: F,
) -> Result<PreparedBlockSource>
where
    F: FnMut(File, &str) -> Result<String>,
{
    if let BlockSourceFormat::Qcow2(chain) = source {
        return prepare_qcow2_source(path, chain, is_readonly, is_direct, &mut register);
    }

    let BlockSourceFormat::Vmdk(vmdk) = source else {
        let (file, metadata) = open_block_source(path, is_readonly, is_direct, false)?;
        let is_regular_file = metadata.is_file();
        let filename = register(file, "block-source")?;
        return Ok(PreparedBlockSource {
            filename,
            is_regular_file,
            backing: Vec::new(),
        });
    };

    let layout = validate_vmdk_layout(vmdk)?;

    let mut backing_paths = HashMap::new();
    for extent in &vmdk.extents {
        if backing_paths.contains_key(&extent.path_on_host) {
            continue;
        }
        // QEMU applies cache.direct to the file node containing the VMDK
        // descriptor, but the VMDK driver opens its flat extents without
        // O_DIRECT. QEMU fdsets require an exact O_DIRECT match, so preserve
        // that existing reopen behavior for delegated extent descriptors.
        let (file, metadata) = open_block_source(&extent.path_on_host, is_readonly, false, true)?;
        let required_sectors = layout
            .required_sectors_for(&extent.path_on_host)
            .ok_or_else(|| anyhow!("missing VMDK extent size for {}", extent.path_on_host))?;
        if metadata.len().div_ceil(512) < required_sectors {
            return Err(anyhow!(
                "VMDK extent {} is shorter than its declared layout",
                extent.path_on_host
            ));
        }
        let index = backing_paths.len();
        let fd_path = register(file, &format!("vmdk-extent-{index}"))?;
        backing_paths.insert(extent.path_on_host.clone(), fd_path);
    }

    let descriptor = layout.render_descriptor_with(|extent| {
        backing_paths
            .get(&extent.path_on_host)
            .cloned()
            .ok_or_else(|| anyhow!("missing prepared VMDK extent {}", extent.path_on_host))
    })?;
    let descriptor =
        create_vmdk_descriptor_file(Path::new(path), &descriptor, is_readonly, is_direct)?;
    let filename = register(descriptor, "vmdk-descriptor")?;

    Ok(PreparedBlockSource {
        filename,
        is_regular_file: true,
        backing: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kata_types::qcow2::QCOW2_MAGIC;
    use kata_types::vmdk::VmdkConfig;
    use nix::fcntl::{fcntl, FcntlArg, OFlag};
    use std::cell::RefCell;
    use std::io::{Read, Seek, SeekFrom};
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn prepares_raw_source_with_requested_access() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("disk.img");
        std::fs::write(&path, b"disk").unwrap();

        let prepared = prepare_block_source(
            path.to_str().unwrap(),
            &BlockSourceFormat::Raw,
            true,
            false,
            |file, _| {
                assert_eq!(file.metadata()?.len(), 4);
                Ok("/dev/fdset/7".to_string())
            },
        )
        .unwrap();

        assert_eq!(prepared.filename, "/dev/fdset/7");
        assert!(prepared.is_regular_file);
    }

    #[test]
    fn opens_writable_raw_source_read_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("writable.raw");
        std::fs::write(&path, b"disk").unwrap();

        prepare_block_source(
            path.to_str().unwrap(),
            &BlockSourceFormat::Raw,
            false,
            false,
            |mut file, _| {
                file.seek(SeekFrom::End(0))?;
                file.write_all(b"-writable")?;
                Ok("/dev/fdset/8".to_string())
            },
        )
        .unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"disk-writable");
    }

    #[test]
    fn opens_readonly_raw_source_without_write_access() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("readonly.raw");
        std::fs::write(&path, b"readonly").unwrap();

        prepare_block_source(
            path.to_str().unwrap(),
            &BlockSourceFormat::Raw,
            true,
            false,
            |mut file, _| {
                assert!(file.write_all(b"no").is_err());
                Ok("/dev/fdset/8".to_string())
            },
        )
        .unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"readonly");
    }

    #[test]
    fn prepares_every_structured_vmdk_extent() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first.raw");
        let second = dir.path().join("second.raw");
        std::fs::write(&first, vec![0u8; 512]).unwrap();
        std::fs::write(&second, vec![0u8; 1024]).unwrap();

        let mut vmdk = VmdkConfig::default();
        vmdk.push_extent(first.to_str().unwrap(), 1, 0);
        vmdk.push_extent(second.to_str().unwrap(), 2, 0);

        let registered = RefCell::new(Vec::new());
        let prepared = prepare_block_source(
            "merged.vmdk",
            &BlockSourceFormat::Vmdk(vmdk),
            true,
            false,
            |mut file, label| {
                let index = registered.borrow().len() + 10;
                if label == "vmdk-descriptor" {
                    let flags = OFlag::from_bits_truncate(fcntl(&file, FcntlArg::F_GETFL)?);
                    assert_eq!(flags & OFlag::O_ACCMODE, OFlag::O_RDONLY);
                    let mut descriptor = String::new();
                    file.read_to_string(&mut descriptor)?;
                    assert!(descriptor.contains("\"/dev/fdset/10\""));
                    assert!(descriptor.contains("\"/dev/fdset/11\""));
                    assert!(!descriptor.contains(first.to_str().unwrap()));
                    assert!(!descriptor.contains(second.to_str().unwrap()));
                }
                registered.borrow_mut().push(label.to_string());
                Ok(format!("/dev/fdset/{index}"))
            },
        )
        .unwrap();

        assert_eq!(prepared.filename, "/dev/fdset/12");
        assert_eq!(
            registered.into_inner(),
            vec!["vmdk-extent-0", "vmdk-extent-1", "vmdk-descriptor"]
        );
    }

    #[test]
    fn preserves_direct_io_on_registered_files() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("disk.img");
        std::fs::write(&path, vec![0u8; 4096]).unwrap();

        prepare_block_source(
            path.to_str().unwrap(),
            &BlockSourceFormat::Raw,
            true,
            true,
            |file, _| {
                let flags = OFlag::from_bits_truncate(fcntl(&file, FcntlArg::F_GETFL)?);
                assert!(flags.contains(OFlag::O_DIRECT));
                Ok("/dev/fdset/8".to_string())
            },
        )
        .unwrap();
    }

    #[test]
    fn preserves_qemu_direct_io_flags_on_vmdk_files() {
        let dir = tempdir().unwrap();
        let extent = dir.path().join("extent.raw");
        let descriptor = dir.path().join("merged.vmdk");
        std::fs::write(&extent, vec![0u8; 4096]).unwrap();
        let mut vmdk = VmdkConfig::default();
        vmdk.push_extent(extent.to_str().unwrap(), 8, 0);

        prepare_block_source(
            descriptor.to_str().unwrap(),
            &BlockSourceFormat::Vmdk(vmdk),
            true,
            true,
            |file, label| {
                let flags = OFlag::from_bits_truncate(fcntl(&file, FcntlArg::F_GETFL)?);
                if label == "vmdk-descriptor" {
                    assert!(flags.contains(OFlag::O_DIRECT));
                    let fd_path = format!("/proc/self/fd/{}", file.as_raw_fd());
                    assert!(std::fs::read_link(fd_path)?.starts_with(dir.path()));
                } else {
                    assert!(!flags.contains(OFlag::O_DIRECT));
                }
                Ok("/dev/fdset/9".to_string())
            },
        )
        .unwrap();
    }

    #[test]
    fn follows_final_component_symlink() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target.raw");
        let link = dir.path().join("source.raw");
        std::fs::write(&target, b"disk").unwrap();
        symlink(&target, &link).unwrap();

        let prepared = prepare_block_source(
            link.to_str().unwrap(),
            &BlockSourceFormat::Raw,
            true,
            false,
            |file, _| {
                assert!(file.metadata()?.is_file());
                Ok("/dev/fdset/1".to_string())
            },
        )
        .unwrap();

        assert_eq!(prepared.filename, "/dev/fdset/1");
        assert!(prepared.is_regular_file);
    }

    #[test]
    fn rejects_short_vmdk_extent() {
        let dir = tempdir().unwrap();
        let extent = dir.path().join("extent.raw");
        std::fs::write(&extent, vec![0_u8; 512]).unwrap();
        let mut vmdk = VmdkConfig::default();
        vmdk.push_extent(extent.to_str().unwrap(), 1, 0);
        vmdk.push_extent(extent.to_str().unwrap(), 1, 1);

        let error = prepare_block_source(
            "disk.vmdk",
            &BlockSourceFormat::Vmdk(vmdk),
            true,
            false,
            |_, _| Ok("/dev/fdset/1".to_string()),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("shorter than its declared layout"));
    }

    #[test]
    fn registered_source_survives_path_removal() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("source.raw");
        std::fs::write(&path, b"persistent").unwrap();
        let received = RefCell::new(None);

        prepare_block_source(
            path.to_str().unwrap(),
            &BlockSourceFormat::Raw,
            true,
            false,
            |file, _| {
                *received.borrow_mut() = Some(file.try_clone()?);
                Ok("/dev/fdset/1".to_string())
            },
        )
        .unwrap();
        std::fs::remove_file(&path).unwrap();

        let mut contents = String::new();
        received
            .borrow_mut()
            .as_mut()
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert_eq!(contents, "persistent");
    }

    #[test]
    fn identifies_owned_block_fdsets() {
        let opaque = block_fd_opaque("drive-3", "vmdk-extent-0");

        assert_eq!(block_fd_node_name(&opaque), Some("drive-3"));
        assert_eq!(block_fd_node_name("unrelated"), None);
        assert_eq!(block_fd_node_name("kata-block::source"), None);
    }

    fn qcow2_image(dir: &Path, name: &str, backing: bool) -> String {
        let mut header = vec![0_u8; 104];
        header[0..4].copy_from_slice(&QCOW2_MAGIC);
        header[4..8].copy_from_slice(&3_u32.to_be_bytes());
        if backing {
            header[8..16].copy_from_slice(&512_u64.to_be_bytes());
        }
        let path = dir.join(name);
        std::fs::write(&path, header).unwrap();
        path.to_str().unwrap().to_string()
    }

    fn chain(layers: &[(&str, ImageFormat)]) -> Qcow2Chain {
        Qcow2Chain {
            backing: layers
                .iter()
                .map(|(path, format)| kata_types::qcow2::BackingImage {
                    path: path.to_string(),
                    format: *format,
                })
                .collect(),
        }
    }

    #[test]
    fn qcow2_chain_registers_every_layer() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base.raw");
        std::fs::write(&base, vec![0_u8; 4096]).unwrap();
        let base = base.to_str().unwrap().to_string();
        let mid = qcow2_image(dir.path(), "mid.qcow2", true);
        let top = qcow2_image(dir.path(), "top.qcow2", true);

        let labels = RefCell::new(Vec::new());
        let prepared = prepare_block_source(
            &top,
            &BlockSourceFormat::Qcow2(chain(&[
                (&base, ImageFormat::Raw),
                (&mid, ImageFormat::Qcow2),
            ])),
            false,
            false,
            |file, label| {
                let flags = OFlag::from_bits_truncate(fcntl(&file, FcntlArg::F_GETFL)?);
                let writable = flags.contains(OFlag::O_RDWR) || flags.contains(OFlag::O_WRONLY);
                // Only the top image may be writable.
                assert_eq!(writable, label == "block-source", "label {label}");
                labels.borrow_mut().push(label.to_string());
                Ok(format!("/dev/fdset/{}", labels.borrow().len()))
            },
        )
        .unwrap();

        assert_eq!(*labels.borrow(), ["block-source", "backing-0", "backing-1"]);
        assert_eq!(prepared.filename, "/dev/fdset/1");
        assert!(prepared.is_regular_file);
        let backing: Vec<_> = prepared
            .backing
            .iter()
            .map(|b| (b.filename.as_str(), b.format))
            .collect();
        assert_eq!(
            backing,
            [
                ("/dev/fdset/2", ImageFormat::Raw),
                ("/dev/fdset/3", ImageFormat::Qcow2)
            ]
        );
    }

    #[test]
    fn thin_qcow2_registers_only_the_top_image() {
        let dir = tempdir().unwrap();
        let top = qcow2_image(dir.path(), "top.qcow2", false);

        // (is_readonly, expect a writable descriptor)
        let tests = [(false, true), (true, false)];

        for (i, (is_readonly, expect_writable)) in tests.iter().enumerate() {
            let labels = RefCell::new(Vec::new());
            let prepared = prepare_block_source(
                &top,
                &BlockSourceFormat::Qcow2(Qcow2Chain::default()),
                *is_readonly,
                false,
                |file, label| {
                    let flags = OFlag::from_bits_truncate(fcntl(&file, FcntlArg::F_GETFL)?);
                    let writable = flags.contains(OFlag::O_RDWR);
                    assert_eq!(writable, *expect_writable, "test[{}]", i);
                    labels.borrow_mut().push(label.to_string());
                    Ok("/dev/fdset/1".to_string())
                },
            );
            let msg = format!("test[{}] read-only {}: {:?}", i, is_readonly, prepared);
            let prepared = prepared.unwrap_or_else(|_| panic!("{}", msg));

            assert_eq!(*labels.borrow(), ["block-source"], "{}", msg);
            assert_eq!(prepared.filename, "/dev/fdset/1", "{}", msg);
            assert!(prepared.is_regular_file, "{}", msg);
            assert!(prepared.backing.is_empty(), "{}", msg);
        }
    }

    #[test]
    fn rejected_qcow2_chain_registers_nothing() {
        let dir = tempdir().unwrap();
        let top = qcow2_image(dir.path(), "top.qcow2", true);
        let top_symlink = dir.path().join("top-symlink.qcow2");
        symlink(&top, &top_symlink).unwrap();
        let top_symlink = top_symlink.to_str().unwrap();

        // (name, backing images, expected error)
        let tests = [
            (
                "header names a backing file, none declared",
                vec![],
                format!("check qcow2 header of {top}"),
            ),
            (
                "writable top as a backing image",
                vec![(top_symlink, ImageFormat::Qcow2)],
                format!("{top} is the same file as {top_symlink}"),
            ),
        ];

        for (i, (name, layers, expected)) in tests.iter().enumerate() {
            let registered = RefCell::new(0);
            let result = prepare_block_source(
                &top,
                &BlockSourceFormat::Qcow2(chain(layers)),
                false,
                false,
                |_, _| {
                    *registered.borrow_mut() += 1;
                    Ok("/dev/fdset/1".to_string())
                },
            );
            let msg = format!("test[{}] {}: result: {:?}", i, name, result);
            assert!(
                format!("{:#}", result.unwrap_err()).contains(expected.as_str()),
                "{}",
                msg
            );
            // A rejected chain hands QEMU nothing, not even the top.
            assert_eq!(*registered.borrow(), 0, "{}", msg);
        }
    }
}
