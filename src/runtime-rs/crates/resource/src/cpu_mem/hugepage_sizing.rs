// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const DEV_HUGEPAGES: &str = "/dev/hugepages";
const MIB: u64 = 1 << 20;

/// The smallest guest sized from the pod's reservation.
pub const MIN_POD_SIZED_GUEST_MIB: u64 = 1024;

/// The size of the pages backing /dev/hugepages, where the guest's RAM comes from.
pub fn vm_huge_page_size() -> Result<u64> {
    let st = nix::sys::statfs::statfs(DEV_HUGEPAGES)
        .with_context(|| format!("statfs {DEV_HUGEPAGES}"))?;
    Ok(st.optimal_transfer_size() as u64)
}

/// Names a size as hugetlb.<name>.max and OCI do: 1 GiB is "1GB", 2 MiB "2MB".
pub fn hugetlb_size_name(size: u64) -> Result<String> {
    const KB: u64 = 1 << 10;
    const GB: u64 = 1 << 30;
    if size == 0 || !size.is_multiple_of(KB) {
        return Err(anyhow!("huge page size {size} is not a whole number of KB"));
    }
    Ok(if size >= GB {
        format!("{}GB", size / GB)
    } else if size >= MIB {
        format!("{}MB", size / MIB)
    } else {
        format!("{}KB", size / KB)
    })
}

/// Names a size the way Kubernetes does: hugepages-1Gi, hugepages-2Mi.
pub fn hugepages_resource_name(size: u64) -> String {
    for (suffix, unit) in [("Gi", 1u64 << 30), ("Mi", 1 << 20), ("Ki", 1 << 10)] {
        if size >= unit && size.is_multiple_of(unit) {
            return format!("hugepages-{}{}", size / unit, suffix);
        }
    }
    format!("hugepages-{size}")
}

// Expands a systemd slice name to its path: "a-b.slice" to "/a.slice/a-b.slice".
fn expand_slice(slice: &str) -> Result<String> {
    let name = slice
        .strip_suffix(".slice")
        .ok_or_else(|| anyhow!("{slice:?} is not a slice"))?;
    if name == "-" {
        return Ok("/".to_string());
    }
    let mut path = String::new();
    let mut prefix = String::new();
    for part in name.split('-') {
        if part.is_empty() {
            return Err(anyhow!("invalid slice name {slice:?}"));
        }
        prefix = if prefix.is_empty() {
            part.to_string()
        } else {
            format!("{prefix}-{part}")
        };
        path.push_str(&format!("/{prefix}.slice"));
    }
    Ok(path)
}

/// The pod's cgroup: the parent of the cgroup an OCI cgroupsPath names.
pub fn pod_cgroup_path(cgroups_path: &str) -> Result<String> {
    let parts: Vec<&str> = cgroups_path.split(':').collect();
    if parts.len() == 3 && !cgroups_path.starts_with('/') {
        return expand_slice(parts[0]);
    }
    let parent = Path::new("/")
        .join(cgroups_path.trim_start_matches('/'))
        .parent()
        .map(PathBuf::from)
        .filter(|p| p != Path::new("/"))
        .ok_or_else(|| anyhow!("cgroup path {cgroups_path:?} has no parent cgroup"))?;
    Ok(parent.display().to_string())
}

/// Whether the host mounts the cgroup v2 unified hierarchy.
pub fn host_has_cgroup_v2() -> bool {
    cgroup_v2_at(CGROUP_ROOT)
}

fn cgroup_v2_at(root: &str) -> bool {
    Path::new(root).join("cgroup.controllers").exists()
}

/// The hugetlb limit for page_size on the pod's cgroup, None when it is "max".
/// A missing file is an error.
pub fn pod_hugetlb_limit(pod_cgroup: &str, page_size: u64) -> Result<Option<u64>> {
    pod_hugetlb_limit_at(CGROUP_ROOT, pod_cgroup, page_size)
}

fn pod_hugetlb_limit_at(root: &str, pod_cgroup: &str, page_size: u64) -> Result<Option<u64>> {
    let file = Path::new(root)
        .join(pod_cgroup.trim_start_matches('/'))
        .join(format!("hugetlb.{}.max", hugetlb_size_name(page_size)?));
    let raw = std::fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
    let value = raw.trim();
    if value == "max" {
        return Ok(None);
    }
    value
        .parse::<u64>()
        .map(Some)
        .with_context(|| format!("parse {}", file.display()))
}

/// The memory a guest of vm_mib reports, less VM/64 for struct pages and 128 MiB.
pub fn guest_mem_total_mib(vm_mib: u64) -> u64 {
    let memmap = vm_mib / 64;
    vm_mib.saturating_sub(memmap + 128)
}

/// What a guest keeps for its kernel, drivers, agent and free pages.
pub fn guest_reserve_mib(mem_total_mib: u64, vcpus: u64) -> u64 {
    (768 + mem_total_mib / 512 + vcpus * 24).min(mem_total_mib / 4)
}

/// The bytes the pod's containers may use in a guest of vm_mib, zero when unknown.
pub fn holdable_guest_bytes(vm_mib: u64, vcpus: u64) -> u64 {
    if vm_mib == 0 {
        return 0;
    }
    let total = guest_mem_total_mib(vm_mib);
    total.saturating_sub(guest_reserve_mib(total, vcpus)) * MIB
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_names() {
        assert_eq!(hugetlb_size_name(1 << 30).unwrap(), "1GB");
        assert_eq!(hugetlb_size_name(2 << 20).unwrap(), "2MB");
        assert!(hugetlb_size_name(0).is_err());
        assert_eq!(hugepages_resource_name(1 << 30), "hugepages-1Gi");
        assert_eq!(hugepages_resource_name(2 << 20), "hugepages-2Mi");
    }

    #[test]
    fn test_pod_cgroup_path() {
        assert_eq!(
            pod_cgroup_path("kubepods-burstable-pod1_2.slice:cri-containerd:abc").unwrap(),
            "/kubepods.slice/kubepods-burstable.slice/kubepods-burstable-pod1_2.slice"
        );
        assert_eq!(
            pod_cgroup_path("/kubepods/burstable/pod12/abc").unwrap(),
            "/kubepods/burstable/pod12"
        );
        assert!(pod_cgroup_path("/abc").is_err());
    }

    #[test]
    fn test_pod_hugetlb_limit() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_str().unwrap();
        assert!(!cgroup_v2_at(root));
        std::fs::write(dir.path().join("cgroup.controllers"), "hugetlb").unwrap();
        assert!(cgroup_v2_at(root));
        std::fs::create_dir_all(dir.path().join("pod")).unwrap();
        assert!(
            pod_hugetlb_limit_at(root, "/pod", 1 << 30).is_err(),
            "no file"
        );
        std::fs::write(dir.path().join("pod/hugetlb.1GB.max"), "max\n").unwrap();
        assert_eq!(pod_hugetlb_limit_at(root, "/pod", 1 << 30).unwrap(), None);
        std::fs::write(dir.path().join("pod/hugetlb.1GB.max"), "17179869184\n").unwrap();
        assert_eq!(
            pod_hugetlb_limit_at(root, "/pod", 1 << 30).unwrap(),
            Some(16 << 30)
        );
    }

    #[test]
    fn test_holdable_matches_the_go_runtime() {
        assert_eq!(holdable_guest_bytes(16384, 4), 15105 * MIB);
        assert_eq!(holdable_guest_bytes(2048, 1), 1416 * MIB);
        assert_eq!(holdable_guest_bytes(0, 4), 0);
    }
}
