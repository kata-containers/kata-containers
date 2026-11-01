// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::{Path, PathBuf};

use anyhow::{anyhow, ensure, Context, Result};
use kata_types::config::hypervisor::Hypervisor;

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

/// The host state the sizing reads, so tests can stand in for it.
pub trait HugetlbHost {
    fn vm_huge_page_size(&self) -> Result<u64>;
    fn has_cgroup_v2(&self) -> bool;
    fn pod_hugetlb_limit(&self, pod_cgroup: &str, page_size: u64) -> Result<Option<u64>>;
}

pub struct PodHugetlbHost;

impl HugetlbHost for PodHugetlbHost {
    fn vm_huge_page_size(&self) -> Result<u64> {
        vm_huge_page_size()
    }
    fn has_cgroup_v2(&self) -> bool {
        host_has_cgroup_v2()
    }
    fn pod_hugetlb_limit(&self, pod_cgroup: &str, page_size: u64) -> Result<Option<u64>> {
        pod_hugetlb_limit(pod_cgroup, page_size)
    }
}

// Sizes a huge page backed VM from hugetlb.<size>.max on the pod's cgroup, giving its
// MiB and the page size, or refuses the sandbox when that cannot be done.
pub fn pod_sized_memory_mib(
    hv: &Hypervisor,
    cgroups_path: Option<&str>,
    host: &dyn HugetlbHost,
) -> Result<(u32, String)> {
    let cgroups_path = cgroups_path.filter(|p| !p.is_empty()).context(
        "enable_hugepages_pod_sizing needs the sandbox's cgroup path to find its pod's cgroup",
    )?;
    let pod_cgroup = pod_cgroup_path(cgroups_path).with_context(|| {
        format!("enable_hugepages_pod_sizing cannot find the pod's cgroup from {cgroups_path:?}")
    })?;
    let page_size = host
        .vm_huge_page_size()
        .context("enable_hugepages_pod_sizing cannot tell the VM's huge page size")?;
    let page_name = hugetlb_size_name(page_size)
        .context("enable_hugepages_pod_sizing cannot tell the VM's huge page size")?;
    let resource = hugepages_resource_name(page_size);
    ensure!(
        host.has_cgroup_v2(),
        "enable_hugepages_pod_sizing needs cgroup v2: the pod's huge page reservation is read from the unified hierarchy"
    );
    let reserved = host.pod_hugetlb_limit(&pod_cgroup, page_size).with_context(|| {
        format!("enable_hugepages_pod_sizing cannot read the pod's {resource} reservation (pod cgroup {pod_cgroup})")
    })?;
    // An unlimited cgroup can also read back as a byte count near 2^63.
    let reserved = match reserved {
        Some(r) if r >> 20 <= u32::MAX as u64 => r,
        other => {
            let value = other.map_or("max".to_string(), |r| r.to_string());
            return Err(anyhow!(
                "the pod's cgroup {pod_cgroup} has no finite {resource} limit the VM can be sized from (it reads {value}): set a finite {resource} limit on the pod's cgroup, as the kubelet does"
            ));
        }
    };
    let reserved_mib = reserved >> 20;
    ensure!(
        reserved_mib != 0,
        "the VM is backed by huge pages but its pod reserved none: request {resource} on a container of the pod, or give the runtime class a pod overhead of {resource} (pod cgroup {pod_cgroup})"
    );
    let max_mib = hv.memory_info.default_maxmemory as u64;
    ensure!(
        max_mib == 0 || reserved_mib <= max_mib,
        "the pod reserved {reserved_mib} MiB of {resource}, more than the {max_mib} MiB of default_maxmemory the VM may have: raise default_maxmemory or reserve less (pod cgroup {pod_cgroup})"
    );
    ensure!(
        reserved_mib >= MIN_POD_SIZED_GUEST_MIB,
        "the pod reserved {reserved_mib} MiB of {resource}, less than the {MIN_POD_SIZED_GUEST_MIB} MiB a guest needs: reserve more on a container of the pod or as the runtime class's pod overhead (pod cgroup {pod_cgroup})"
    );
    info!(
        sl!(),
        "sizing the huge page backed VM from the pod's huge page reservation";
        "pod-cgroup" => &pod_cgroup,
        "reserved-mb" => reserved_mib,
        "default-memory-mb" => hv.memory_info.default_memory
    );
    Ok((reserved_mib as u32, page_name))
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
pub(crate) mod tests {
    use super::*;

    pub(crate) struct StubHost {
        page_size: Option<u64>,
        cgroup_v2: bool,
        limit: Option<Option<u64>>,
    }

    impl HugetlbHost for StubHost {
        fn vm_huge_page_size(&self) -> Result<u64> {
            self.page_size.ok_or_else(|| anyhow!("statfs failed"))
        }
        fn has_cgroup_v2(&self) -> bool {
            self.cgroup_v2
        }
        fn pod_hugetlb_limit(&self, _pod_cgroup: &str, _page_size: u64) -> Result<Option<u64>> {
            self.limit.ok_or_else(|| anyhow!("no such file"))
        }
    }

    pub(crate) fn host_reserving(limit: u64) -> StubHost {
        StubHost {
            page_size: Some(1 << 30),
            cgroup_v2: true,
            limit: Some(Some(limit)),
        }
    }

    #[test]
    fn test_pod_sized_memory_mib() {
        let hv = Hypervisor::default();
        let pod = Some("/kubepods/pod1/abc");
        assert_eq!(
            pod_sized_memory_mib(&hv, pod, &host_reserving(2 << 30)).unwrap(),
            (2048, "1GB".to_string())
        );
        let cases: Vec<(&str, Option<&str>, StubHost, &str)> = vec![
            (
                "no sandbox cgroup",
                None,
                host_reserving(2 << 30),
                "cgroup path",
            ),
            (
                "huge page size unknown",
                pod,
                StubHost {
                    page_size: None,
                    ..host_reserving(2 << 30)
                },
                "huge page size",
            ),
            (
                "cgroup v1",
                pod,
                StubHost {
                    cgroup_v2: false,
                    ..host_reserving(2 << 30)
                },
                "cgroup v2",
            ),
            (
                "no limit file",
                pod,
                StubHost {
                    limit: None,
                    ..host_reserving(2 << 30)
                },
                "cannot read the pod's hugepages-1Gi reservation",
            ),
            (
                "unlimited",
                pod,
                StubHost {
                    limit: Some(None),
                    ..host_reserving(2 << 30)
                },
                "no finite hugepages-1Gi limit the VM can be sized from (it reads max)",
            ),
            (
                "unlimited as a number",
                pod,
                StubHost {
                    limit: Some(Some(i64::MAX as u64 & !4095)),
                    ..host_reserving(2 << 30)
                },
                "no finite hugepages-1Gi limit the VM can be sized from (it reads 9223372036854771712)",
            ),
            (
                "below a guest",
                pod,
                host_reserving(512 << 20),
                "less than the 1024 MiB a guest needs",
            ),
        ];
        for (name, cgroups_path, host, want) in cases {
            let err = pod_sized_memory_mib(&hv, cgroups_path, &host).unwrap_err();
            assert!(
                format!("{err:#}").contains(want),
                "{}: {:#} does not mention {:?}",
                name,
                err,
                want
            );
        }
    }

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
