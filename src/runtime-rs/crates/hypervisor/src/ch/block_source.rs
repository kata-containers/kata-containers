// Copyright © 2026, Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::VmdkConfig;

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

fn extent_reference(extent_path: &str, descriptor_dir: &Path) -> String {
    let path = Path::new(extent_path);
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        if parent == descriptor_dir {
            return name.to_string_lossy().into_owned();
        }
    }
    extent_path.to_string()
}

// The VMDK descriptor wraps the extent path in double quotes and has
// no escape mechanism, so a `"`, newline, or carriage return in a path would
// terminate the field early and could inject additional descriptor lines.
// Reject such paths rather than emit a malformed descriptor.
fn validate_extent_reference(reference: &str, extent_path: &str) -> Result<()> {
    if reference.contains(['"', '\n', '\r']) {
        return Err(anyhow!(
            "VMDK extent path {extent_path} contains characters unsupported by the descriptor grammar"
        ));
    }
    Ok(())
}

/// Render a VMDK descriptor for a structured layout.
///
/// This mirrors the QEMU backend's `render_vmdk_descriptor`, but references the
/// backing extents by their real host paths
fn render_vmdk_descriptor(config: &VmdkConfig, descriptor_dir: &Path) -> Result<String> {
    let total_sectors = config
        .total_sectors()
        .ok_or_else(|| anyhow!("VMDK total sector count overflow"))?;
    if total_sectors == 0 {
        return Err(anyhow!("VMDK contains no non-empty extents"));
    }

    let mut descriptor = String::new();
    writeln!(descriptor, "# Disk DescriptorFile")?;
    writeln!(descriptor, "version=1")?;
    writeln!(descriptor, "CID=fffffffe")?;
    writeln!(descriptor, "parentCID=ffffffff")?;
    writeln!(descriptor, "createType=\"twoGbMaxExtentFlat\"")?;
    writeln!(descriptor)?;
    writeln!(descriptor, "# Extent description")?;
    for extent in &config.extents {
        let extent_path = extent_reference(&extent.path_on_host, descriptor_dir);
        validate_extent_reference(&extent_path, &extent.path_on_host)?;
        writeln!(
            descriptor,
            "RW {} FLAT \"{}\" {}",
            extent.sectors, extent_path, extent.file_offset
        )?;
    }

    let cylinders = total_sectors.div_ceil(63 * 16);
    writeln!(descriptor)?;
    writeln!(descriptor, "# The Disk Data Base")?;
    writeln!(descriptor, "#DDB")?;
    writeln!(descriptor)?;
    writeln!(descriptor, "ddb.virtualHWVersion = \"4\"")?;
    writeln!(descriptor, "ddb.geometry.cylinders = \"{cylinders}\"")?;
    writeln!(descriptor, "ddb.geometry.heads = \"16\"")?;
    writeln!(descriptor, "ddb.geometry.sectors = \"63\"")?;
    writeln!(descriptor, "ddb.adapterType = \"ide\"")?;

    Ok(descriptor)
}

/// Validate that every backing extent exists and is large enough for the layout.
fn validate_extents(vmdk: &VmdkConfig) -> Result<()> {
    let mut required_sectors_by_path: HashMap<&str, u64> = HashMap::new();
    for extent in &vmdk.extents {
        let required_sectors = extent
            .file_offset
            .checked_add(extent.sectors)
            .ok_or_else(|| anyhow!("VMDK extent sector count overflow"))?;
        required_sectors_by_path
            .entry(extent.path_on_host.as_str())
            .and_modify(|maximum| *maximum = (*maximum).max(required_sectors))
            .or_insert(required_sectors);
    }

    for (path, required_sectors) in &required_sectors_by_path {
        let metadata = fs::metadata(path).with_context(|| format!("stat VMDK extent {path}"))?;
        if !metadata.is_file() {
            return Err(anyhow!("VMDK extent {path} is not a regular file"));
        }
        if metadata.len().div_ceil(512) < *required_sectors {
            return Err(anyhow!(
                "VMDK extent {path} is shorter than its declared layout"
            ));
        }
    }

    Ok(())
}

/// Materialize a VMDK descriptor on the host.
pub(super) fn prepare_vmdk_descriptor(descriptor_path: &str, vmdk: &VmdkConfig) -> Result<()> {
    if vmdk.extents.is_empty() {
        return Err(anyhow!("VMDK contains no extents"));
    }

    validate_extents(vmdk)?;

    let path = Path::new(descriptor_path);
    let descriptor_dir = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("VMDK descriptor path has no parent directory"))?;

    let descriptor = render_vmdk_descriptor(vmdk, descriptor_dir)?;

    fs::create_dir_all(descriptor_dir).with_context(|| {
        format!(
            "create VMDK descriptor directory {}",
            descriptor_dir.display()
        )
    })?;

    // Write to a temporary file and rename so a failure never leaves a partial
    // descriptor at the path cloud-hypervisor is about to open.
    let tmp_path = path.with_extension("vmdk.tmp");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp_path)
        .with_context(|| format!("create VMDK descriptor {}", tmp_path.display()))?;
    file.write_all(descriptor.as_bytes())
        .with_context(|| format!("write VMDK descriptor {}", tmp_path.display()))?;
    file.sync_all()
        .with_context(|| format!("flush VMDK descriptor {}", tmp_path.display()))?;
    drop(file);

    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "rename VMDK descriptor {} -> {}",
            tmp_path.display(),
            path.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn renders_structured_vmdk_layout() {
        let mut config = VmdkConfig::default();
        config.push_extent("/images/first.raw", 8, 0);
        config.push_extent("/images/first.raw", 4, 8);
        config.push_extent("/images/second.raw", 16, 0);

        let descriptor = render_vmdk_descriptor(&config, Path::new("/descriptors")).unwrap();
        assert!(descriptor.contains("createType=\"twoGbMaxExtentFlat\""));
        assert!(descriptor.contains("RW 8 FLAT \"/images/first.raw\" 0"));
        assert!(descriptor.contains("RW 4 FLAT \"/images/first.raw\" 8"));
        assert!(descriptor.contains("RW 16 FLAT \"/images/second.raw\" 0"));
    }

    #[test]
    fn references_same_directory_extents_by_file_name() {
        let mut config = VmdkConfig::default();
        config.push_extent("/run/vmdk/gpt-head.img", 2048, 0);
        config.push_extent("/run/vmdk/pad-0.img", 8, 0);
        config.push_extent("/var/lib/containerd/layer.erofs", 16, 0);

        let descriptor = render_vmdk_descriptor(&config, Path::new("/run/vmdk")).unwrap();
        assert!(descriptor.contains("RW 2048 FLAT \"gpt-head.img\" 0"));
        assert!(descriptor.contains("RW 8 FLAT \"pad-0.img\" 0"));
        assert!(descriptor.contains("RW 16 FLAT \"/var/lib/containerd/layer.erofs\" 0"));
    }

    #[test]
    fn writes_descriptor_referencing_host_extents() {
        let dir = tempdir().unwrap();
        let extent = dir.path().join("extent-flat.img");
        fs::write(&extent, vec![0u8; 8 * 512]).unwrap();

        let mut vmdk = VmdkConfig::default();
        vmdk.push_extent(extent.to_str().unwrap(), 8, 0);

        let descriptor_path = dir.path().join("merged.vmdk");
        prepare_vmdk_descriptor(descriptor_path.to_str().unwrap(), &vmdk).unwrap();

        let contents = fs::read_to_string(&descriptor_path).unwrap();
        // The extent lives in the descriptor's directory, so it is referenced by name.
        assert!(contents.contains("RW 8 FLAT \"extent-flat.img\" 0"));
        assert!(!descriptor_path.with_extension("vmdk.tmp").exists());
    }

    #[test]
    fn rejects_extent_paths_with_descriptor_grammar_characters() {
        for bad in [
            "/images/ev\"il.raw",
            "/images/ev\nil.raw",
            "/images/ev\ril.raw",
        ] {
            let mut config = VmdkConfig::default();
            config.push_extent(bad, 8, 0);

            let error = render_vmdk_descriptor(&config, Path::new("/descriptors")).unwrap_err();
            assert!(error
                .to_string()
                .contains("unsupported by the descriptor grammar"));
        }
    }

    #[test]
    fn rejects_short_vmdk_extent() {
        let dir = tempdir().unwrap();
        let extent = dir.path().join("extent-flat.img");
        fs::write(&extent, vec![0u8; 512]).unwrap();

        let mut vmdk = VmdkConfig::default();
        vmdk.push_extent(extent.to_str().unwrap(), 1, 0);
        vmdk.push_extent(extent.to_str().unwrap(), 1, 1);

        let descriptor_path = dir.path().join("merged.vmdk");
        let error = prepare_vmdk_descriptor(descriptor_path.to_str().unwrap(), &vmdk).unwrap_err();
        assert!(error
            .to_string()
            .contains("shorter than its declared layout"));
        assert!(!descriptor_path.exists());
    }
}
