// Copyright (c) 2026 NAVER Cloud Corp.
//
// SPDX-License-Identifier: Apache-2.0
//

//! Cross-source physical-device overlap guard for cold plug.
//!
//! The same physical device can be reachable via a legacy iommu-group cdev
//! from one source and a per-device vfio cdev from the other, so comparing
//! CDI name lists as strings misses a double plug; compare by physical
//! coordinate (PCI BDF or mdev instance UUID) instead.
//!
//! Mirrors the Go runtime's `device_cold_plug_overlap.go`.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::sl;
use crate::{POD_RESOURCE_DEVICE_SOURCE_DEVICE_PLUGIN, POD_RESOURCE_DEVICE_SOURCE_DRA};
use slog::debug;

/// Roots for device-node resolution; parameterized so tests can use fixtures.
const DEFAULT_SYS_ROOT: &str = "/sys";
const DEFAULT_DEV_ROOT: &str = "/dev";

/// Error when a device-plugin CDI device and a DRA CDI device resolve to the
/// same underlying physical device (see the module doc for why names are not
/// compared as strings).
pub(crate) fn check_cross_source_physical_overlap(
    device_plugin_devs: &[String],
    dra_devs: &[String],
    spec_dirs: &[&str],
) -> Result<()> {
    check_cross_source_physical_overlap_in(
        device_plugin_devs,
        dra_devs,
        spec_dirs,
        Path::new(DEFAULT_SYS_ROOT),
        Path::new(DEFAULT_DEV_ROOT),
    )
}

fn check_cross_source_physical_overlap_in(
    device_plugin_devs: &[String],
    dra_devs: &[String],
    spec_dirs: &[&str],
    sys_root: &Path,
    dev_root: &Path,
) -> Result<()> {
    if device_plugin_devs.is_empty() || dra_devs.is_empty() {
        return Ok(());
    }

    let dp_coords = resolve_physical_coords(device_plugin_devs, spec_dirs, sys_root, dev_root)?;
    let dra_coords = resolve_physical_coords(dra_devs, spec_dirs, sys_root, dev_root)?;

    for (coord, dp_name) in &dp_coords {
        if let Some(dra_name) = dra_coords.get(coord) {
            return Err(anyhow!(
                "cold plug: physical device {:?} is reachable via both pod_resource_device_sources: \
                 {:?} (device-plugin CDI device {:?}) and {:?} (dra CDI device {:?}); \
                 this would double cold-plug the same underlying device",
                coord,
                POD_RESOURCE_DEVICE_SOURCE_DEVICE_PLUGIN,
                dp_name,
                POD_RESOURCE_DEVICE_SOURCE_DRA,
                dra_name,
            ));
        }
    }

    Ok(())
}

/// Map CDI devices to physical coordinates (BDF or mdev UUID). Unresolvable
/// names are skipped (handle_cdi_devices already rejects them); a path that
/// cannot be normalized is an error: the guard cannot prove it does not
/// overlap.
fn resolve_physical_coords(
    cdi_devs: &[String],
    spec_dirs: &[&str],
    sys_root: &Path,
    dev_root: &Path,
) -> Result<HashMap<String, String>> {
    let mut coords: HashMap<String, String> = HashMap::new();

    // A cache build/refresh failure here is fail-closed for the overlap guard.
    let name_to_paths = crate::cdi_device_node_host_paths(spec_dirs, cdi_devs)?;

    for name in cdi_devs {
        let paths = match name_to_paths.get(name) {
            Some(paths) => paths,
            None => continue,
        };
        for path in paths {
            let keys = normalize_device_node_path(path, sys_root, dev_root).with_context(|| {
                format!(
                    "cold plug: failed to normalize device node path for overlap check (device {name:?})"
                )
            })?;
            for k in keys {
                coords.entry(k).or_insert_with(|| name.clone());
            }
        }
    }

    Ok(coords)
}

/// Map a device node path to physical coordinate keys. A legacy
/// `<dev_root>/vfio/N` group cdev expands to every BDF in the group, since
/// any member could alias a per-device cdev from the other source; non-vfio
/// paths are their own key.
fn normalize_device_node_path(path: &str, sys_root: &Path, dev_root: &Path) -> Result<Vec<String>> {
    let p = Path::new(path);
    let vfio_devices_dir = dev_root.join("vfio").join("devices");
    let vfio_group_dir = dev_root.join("vfio");

    if let Some(parent) = p.parent() {
        if parent == vfio_devices_dir.as_path() {
            return normalize_vfio_device_cdev(p, sys_root);
        }
        if parent == vfio_group_dir.as_path() {
            if let Some(base) = p.file_name().and_then(|b| b.to_str()) {
                if is_iommu_group(base) {
                    return normalize_iommu_group_cdev(base, sys_root);
                }
            }
        }
    }

    Ok(vec![path.to_string()])
}

/// Resolve `<dev_root>/vfio/devices/vfioN` via its sysfs device symlink; the
/// target basename is the device's own identity (PCI BDF, or mdev instance
/// UUID -- never the parent), so distinct mdev slices of one parent never
/// collide.
fn normalize_vfio_device_cdev(path: &Path, sys_root: &Path) -> Result<Vec<String>> {
    let vfio_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("invalid vfio device node path {:?}", path))?;

    let link = sys_root
        .join("class")
        .join("vfio-dev")
        .join(vfio_name)
        .join("device");
    let target =
        std::fs::read_link(&link).with_context(|| format!("failed to readlink {link:?}"))?;

    let base = target
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("invalid symlink target {:?}", target))?;

    Ok(vec![base.to_string()])
}

/// Return the BDFs of every device in the IOMMU group.
fn normalize_iommu_group_cdev(group: &str, sys_root: &Path) -> Result<Vec<String>> {
    let devices_dir = sys_root
        .join("kernel")
        .join("iommu_groups")
        .join(group)
        .join("devices");
    let entries =
        std::fs::read_dir(&devices_dir).with_context(|| format!("failed to list {devices_dir:?}"))?;

    let mut bdfs = Vec::new();
    for e in entries {
        let e = e.with_context(|| format!("failed to read entry in {devices_dir:?}"))?;
        bdfs.push(e.file_name().to_string_lossy().into_owned());
    }

    debug!(sl!(), "iommu group {} resolved to BDFs {:?}", group, bdfs);
    Ok(bdfs)
}

fn is_iommu_group(base: &str) -> bool {
    !base.is_empty() && base.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs as unix_fs;
    use std::path::PathBuf;

    fn write_cdi_spec(dir: &Path, name: &str, kind: &str, devices: &[(&str, &str)]) {
        let mut content = format!("cdiVersion: \"0.5.0\"\nkind: \"{kind}\"\ndevices:\n");
        for (dev_name, path) in devices {
            content.push_str(&format!(
                "  - name: \"{dev_name}\"\n    containerEdits:\n      deviceNodes:\n      - path: \"{path}\"\n"
            ));
        }
        fs::write(dir.join(format!("{name}.yaml")), content).unwrap();
    }

    fn build_vfio_device_cdev(sys_dir: &Path, vfio_name: &str, bdf: &str) {
        let link_dir = sys_dir.join("class").join("vfio-dev").join(vfio_name);
        fs::create_dir_all(&link_dir).unwrap();
        let target_dir = sys_dir.join("devices").join("pci0000:00").join(bdf);
        fs::create_dir_all(&target_dir).unwrap();
        let rel = pathdiff_rel(&target_dir, &link_dir);
        unix_fs::symlink(rel, link_dir.join("device")).unwrap();
    }

    // Nests the mdev UUID under the parent BDF, mirroring real sysfs; the UUID basename is the device identity.
    fn build_vfio_mdev_cdev(sys_dir: &Path, vfio_name: &str, parent_bdf: &str, mdev_uuid: &str) {
        let link_dir = sys_dir.join("class").join("vfio-dev").join(vfio_name);
        fs::create_dir_all(&link_dir).unwrap();
        let target_dir = sys_dir
            .join("devices")
            .join("pci0000:00")
            .join(parent_bdf)
            .join(mdev_uuid);
        fs::create_dir_all(&target_dir).unwrap();
        let rel = pathdiff_rel(&target_dir, &link_dir);
        unix_fs::symlink(rel, link_dir.join("device")).unwrap();
    }

    fn build_iommu_group(sys_dir: &Path, group: &str, bdfs: &[&str]) {
        let devices_dir = sys_dir
            .join("kernel")
            .join("iommu_groups")
            .join(group)
            .join("devices");
        fs::create_dir_all(&devices_dir).unwrap();
        for bdf in bdfs {
            fs::write(devices_dir.join(bdf), b"").unwrap();
        }
    }

    // Hand-rolled to avoid a dev-dependency; fixtures share a common ancestor.
    fn pathdiff_rel(target: &Path, base: &Path) -> PathBuf {
        let t: Vec<_> = target.components().collect();
        let b: Vec<_> = base.components().collect();
        let mut i = 0;
        while i < t.len() && i < b.len() && t[i] == b[i] {
            i += 1;
        }
        let mut rel = PathBuf::new();
        for _ in i..b.len() {
            rel.push("..");
        }
        for comp in &t[i..] {
            rel.push(comp.as_os_str());
        }
        rel
    }

    fn vfio_dev_path(dev_dir: &Path, name: &str) -> String {
        dev_dir
            .join("vfio")
            .join("devices")
            .join(name)
            .to_string_lossy()
            .into_owned()
    }

    fn vfio_group_path(dev_dir: &Path, group: &str) -> String {
        dev_dir
            .join("vfio")
            .join(group)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn test_normalize_device_node_path() {
        let sys_dir = tempfile::tempdir().unwrap();
        let dev_dir = tempfile::tempdir().unwrap();
        let sys = sys_dir.path();
        let dev = dev_dir.path();

        // vfio device cdev resolves to BDF
        build_vfio_device_cdev(sys, "vfio0", "0000:65:00.0");
        let coords = normalize_device_node_path(&vfio_dev_path(dev, "vfio0"), sys, dev).unwrap();
        assert_eq!(coords, vec!["0000:65:00.0".to_string()]);

        // legacy iommu group cdev resolves to member BDFs
        build_iommu_group(sys, "42", &["0000:65:00.0", "0000:65:00.1"]);
        let mut coords = normalize_device_node_path(&vfio_group_path(dev, "42"), sys, dev).unwrap();
        coords.sort();
        assert_eq!(
            coords,
            vec!["0000:65:00.0".to_string(), "0000:65:00.1".to_string()]
        );

        // unrelated path falls back to the path itself
        let coords = normalize_device_node_path("/dev/nvidia0", sys, dev).unwrap();
        assert_eq!(coords, vec!["/dev/nvidia0".to_string()]);

        // mdev cdev resolves to the mdev instance UUID, not the parent BDF
        let mdev_uuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        build_vfio_mdev_cdev(sys, "vfio-mdev0", "0000:65:00.0", mdev_uuid);
        let coords =
            normalize_device_node_path(&vfio_dev_path(dev, "vfio-mdev0"), sys, dev).unwrap();
        assert_eq!(coords, vec![mdev_uuid.to_string()]);

        // vfio device cdev with unresolvable symlink errors
        assert!(normalize_device_node_path(&vfio_dev_path(dev, "vfio-missing"), sys, dev).is_err());
    }

    #[test]
    fn test_check_cross_source_physical_overlap() {
        let sys_dir = tempfile::tempdir().unwrap();
        let dev_dir = tempfile::tempdir().unwrap();
        let sys = sys_dir.path();
        let dev = dev_dir.path();

        // no overlap when sets are disjoint
        {
            let spec_dir = tempfile::tempdir().unwrap();
            let spec = spec_dir.path();
            build_vfio_device_cdev(sys, "vfio0", "0000:65:00.0");
            build_vfio_device_cdev(sys, "vfio1", "0000:66:00.0");
            write_cdi_spec(
                spec,
                "dp",
                "vendor.com/gpu",
                &[("gpu0", &vfio_dev_path(dev, "vfio0"))],
            );
            write_cdi_spec(
                spec,
                "dra",
                "vendor.com/dra",
                &[("gpu1", &vfio_dev_path(dev, "vfio1"))],
            );
            let spec_dirs = [spec.to_str().unwrap()];
            check_cross_source_physical_overlap_in(
                &["vendor.com/gpu=gpu0".to_string()],
                &["vendor.com/dra=gpu1".to_string()],
                &spec_dirs,
                sys,
                dev,
            )
            .unwrap();
        }

        // direct overlap: same BDF via vfio device cdev on both sides
        {
            let spec_dir = tempfile::tempdir().unwrap();
            let spec = spec_dir.path();
            build_vfio_device_cdev(sys, "vfio2", "0000:70:00.0");
            write_cdi_spec(
                spec,
                "dp2",
                "vendor.com/gpu",
                &[("gpu2", &vfio_dev_path(dev, "vfio2"))],
            );
            write_cdi_spec(
                spec,
                "dra2",
                "vendor.com/dra",
                &[("gpu2", &vfio_dev_path(dev, "vfio2"))],
            );
            let spec_dirs = [spec.to_str().unwrap()];
            let err = check_cross_source_physical_overlap_in(
                &["vendor.com/gpu=gpu2".to_string()],
                &["vendor.com/dra=gpu2".to_string()],
                &spec_dirs,
                sys,
                dev,
            )
            .unwrap_err();
            assert!(err.to_string().contains("0000:70:00.0"));
        }

        // aliased overlap: legacy group cdev vs vfio device cdev for the same BDF
        {
            let spec_dir = tempfile::tempdir().unwrap();
            let spec = spec_dir.path();
            let bdf = "0000:99:00.0";
            build_vfio_device_cdev(sys, "vfio9", bdf);
            build_iommu_group(sys, "9", &[bdf]);
            write_cdi_spec(
                spec,
                "dp3",
                "vendor.com/gpu",
                &[("gpu9", &vfio_group_path(dev, "9"))],
            );
            write_cdi_spec(
                spec,
                "dra3",
                "vendor.com/dra",
                &[("gpu9", &vfio_dev_path(dev, "vfio9"))],
            );
            let spec_dirs = [spec.to_str().unwrap()];
            let err = check_cross_source_physical_overlap_in(
                &["vendor.com/gpu=gpu9".to_string()],
                &["vendor.com/dra=gpu9".to_string()],
                &spec_dirs,
                sys,
                dev,
            )
            .unwrap_err();
            assert!(err.to_string().contains(bdf));
        }

        // distinct mdev slices of the same parent GPU never collide
        {
            let spec_dir = tempfile::tempdir().unwrap();
            let spec = spec_dir.path();
            let parent_bdf = "0000:88:00.0";
            build_vfio_mdev_cdev(sys, "vfio-mdev1", parent_bdf, "11111111-1111-1111-1111-111111111111");
            build_vfio_mdev_cdev(sys, "vfio-mdev2", parent_bdf, "22222222-2222-2222-2222-222222222222");
            write_cdi_spec(
                spec,
                "dp-mdev",
                "vendor.com/gpu",
                &[("slice1", &vfio_dev_path(dev, "vfio-mdev1"))],
            );
            write_cdi_spec(
                spec,
                "dra-mdev",
                "vendor.com/dra",
                &[("slice2", &vfio_dev_path(dev, "vfio-mdev2"))],
            );
            let spec_dirs = [spec.to_str().unwrap()];
            check_cross_source_physical_overlap_in(
                &["vendor.com/gpu=slice1".to_string()],
                &["vendor.com/dra=slice2".to_string()],
                &spec_dirs,
                sys,
                dev,
            )
            .unwrap();
        }

        // same mdev instance UUID on both sides collides
        {
            let spec_dir = tempfile::tempdir().unwrap();
            let spec = spec_dir.path();
            let uuid = "33333333-3333-3333-3333-333333333333";
            build_vfio_mdev_cdev(sys, "vfio-mdev3", "0000:89:00.0", uuid);
            write_cdi_spec(
                spec,
                "dp-mdev-same",
                "vendor.com/gpu",
                &[("slice3", &vfio_dev_path(dev, "vfio-mdev3"))],
            );
            write_cdi_spec(
                spec,
                "dra-mdev-same",
                "vendor.com/dra",
                &[("slice3", &vfio_dev_path(dev, "vfio-mdev3"))],
            );
            let spec_dirs = [spec.to_str().unwrap()];
            let err = check_cross_source_physical_overlap_in(
                &["vendor.com/gpu=slice3".to_string()],
                &["vendor.com/dra=slice3".to_string()],
                &spec_dirs,
                sys,
                dev,
            )
            .unwrap_err();
            assert!(err.to_string().contains(uuid));
        }

        // device node normalization failure fails closed instead of being skipped
        {
            let spec_dir = tempfile::tempdir().unwrap();
            let spec = spec_dir.path();
            write_cdi_spec(
                spec,
                "dp-broken",
                "vendor.com/gpu",
                &[("gpu-broken", &vfio_dev_path(dev, "vfio-broken-missing"))],
            );
            write_cdi_spec(
                spec,
                "dra-broken",
                "vendor.com/dra",
                &[("gpu-broken-2", &vfio_dev_path(dev, "vfio-broken-missing-2"))],
            );
            let spec_dirs = [spec.to_str().unwrap()];
            assert!(check_cross_source_physical_overlap_in(
                &["vendor.com/gpu=gpu-broken".to_string()],
                &["vendor.com/dra=gpu-broken-2".to_string()],
                &spec_dirs,
                sys,
                dev,
            )
            .is_err());
        }

        // empty inputs never error
        {
            let spec_dirs = [sys.to_str().unwrap()]; // any dir; not consulted
            check_cross_source_physical_overlap_in(&[], &[], &spec_dirs, sys, dev).unwrap();
            check_cross_source_physical_overlap_in(
                &["vendor.com/gpu=gpu0".to_string()],
                &[],
                &spec_dirs,
                sys,
                dev,
            )
            .unwrap();
            check_cross_source_physical_overlap_in(
                &[],
                &["vendor.com/dra=gpu0".to_string()],
                &spec_dirs,
                sys,
                dev,
            )
            .unwrap();
        }

        // unresolvable CDI device names are skipped silently
        {
            let spec_dir = tempfile::tempdir().unwrap();
            let spec_dirs = [spec_dir.path().to_str().unwrap()];
            check_cross_source_physical_overlap_in(
                &["vendor.com/gpu=does-not-exist".to_string()],
                &["vendor.com/dra=also-does-not-exist".to_string()],
                &spec_dirs,
                sys,
                dev,
            )
            .unwrap();
        }
    }
}
