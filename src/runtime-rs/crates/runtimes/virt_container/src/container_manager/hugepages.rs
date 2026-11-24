// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use kata_types::config::{hypervisor::HugePageType, TomlConfig};
use oci_spec::runtime::{LinuxHugepageLimit, LinuxResources};
use resource::cpu_mem::hugepage_sizing::holdable_guest_bytes;

const MIB: u64 = 1 << 20;

#[derive(Clone, Debug, Default)]
pub(crate) struct Reservation {
    pub id: String,
    /// In the VM's page size. It can be the container's own declaration or the
    /// kubelet's copy of the pod level value.
    pub huge_pages: u64,
    pub memory: u64,
    /// The ceiling a running container was given. It is not taken back when a
    /// later container changes what counts as a copy.
    pub applied: Option<u64>,
}

pub(crate) struct PodHugePages {
    pub page: String,
    pub others: Vec<Reservation>,
}

pub(crate) struct HugePagePlan {
    pub page: String,
    pub copied: u64,
    pub held: u64,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Guest {
    pub memory_mib: u64,
    pub vcpus: u64,
}

impl Guest {
    fn holdable(&self) -> u64 {
        holdable_guest_bytes(self.memory_mib, self.vcpus)
    }
}

pub(crate) fn is_pod_sized(toml_config: &TomlConfig) -> bool {
    toml_config
        .hypervisor
        .get(&toml_config.runtime.hypervisor_name)
        .is_some_and(|hv| {
            hv.memory_info.enable_hugepages
                && hv.memory_info.enable_hugepages_pod_sizing
                && matches!(hv.memory_info.hugepage_type, HugePageType::Hugetlbfs)
        })
}

// A dynamic guest has no fixed memory budget for admission checks.
fn static_guest(toml_config: &TomlConfig) -> Guest {
    if !toml_config.runtime.static_sandbox_resource_mgmt {
        return Guest::default();
    }
    toml_config
        .hypervisor
        .get(&toml_config.runtime.hypervisor_name)
        .map(|hv| Guest {
            memory_mib: hv.memory_info.default_memory as u64,
            vcpus: hv.cpu_info.default_vcpus.ceil() as u64,
        })
        .unwrap_or_default()
}

// Messages name the resource as Kubernetes spells it: "1GB" is hugepages-1Gi.
fn resource_name(page: &str) -> String {
    let size = page
        .strip_suffix("GB")
        .map(|n| format!("{n}Gi"))
        .or_else(|| page.strip_suffix("MB").map(|n| format!("{n}Mi")))
        .or_else(|| page.strip_suffix("KB").map(|n| format!("{n}Ki")))
        .unwrap_or_else(|| page.to_string());
    format!("hugepages-{size}")
}

pub(crate) fn huge_pages_of(resources: Option<&LinuxResources>, page_size: &str) -> u64 {
    resources
        .and_then(|r| r.hugepage_limits().as_ref())
        .map(|limits| {
            limits
                .iter()
                .filter(|l| l.page_size() == page_size)
                .map(|l| l.limit().max(0) as u64)
                .sum()
        })
        .unwrap_or(0)
}

/// A memory limit of zero, a negative one and i64::MAX all mean none.
pub(crate) fn memory_limit(resources: Option<&LinuxResources>) -> u64 {
    resources
        .and_then(|r| r.memory().as_ref())
        .and_then(|m| m.limit())
        .filter(|l| *l > 0 && *l != i64::MAX)
        .map_or(0, |l| l as u64)
}

pub(crate) fn reservation(
    id: &str,
    resources: Option<&LinuxResources>,
    page_size: &str,
    applied: Option<u64>,
) -> Reservation {
    Reservation {
        id: id.to_string(),
        huge_pages: huge_pages_of(resources, page_size),
        memory: memory_limit(resources),
        applied,
    }
}

/// The kubelet copies a pod level hugepages-<size> into each container that
/// declares none, and nothing marks the copy (kubernetes/enhancements#4113).
/// Returns the copied value, or zero when the values do not reveal one.
pub(crate) fn copied_reservation(rs: &[Reservation], vm_bytes: u64) -> u64 {
    if vm_bytes == 0 {
        return 0;
    }
    let largest = rs.iter().map(|r| r.huge_pages).max().unwrap_or(0);
    let sum = rs.iter().fold(0u64, |s, r| s.saturating_add(r.huge_pages));
    let below = rs
        .iter()
        .any(|r| r.huge_pages != 0 && r.huge_pages < largest);
    // Declarations alone fit the VM, so a sum past it means some are copies.
    // The copy is the largest value, if a smaller one exists and it fits the VM.
    if sum <= vm_bytes || !below || largest > vm_bytes {
        return 0;
    }
    largest
}

/// When every container carries the same value and together they run past the
/// VM, the values cannot tell a copy from a declaration.
pub(crate) fn ambiguous_copy(rs: &[Reservation], vm_bytes: u64) -> Option<u64> {
    if vm_bytes == 0 {
        return None;
    }
    let carriers: Vec<u64> = rs
        .iter()
        .map(|r| r.huge_pages)
        .filter(|h| *h != 0)
        .collect();
    let value = *carriers.first()?;
    let sum = carriers.iter().fold(0u64, |s, h| s.saturating_add(*h));
    (carriers.len() > 1
        && carriers.iter().all(|h| *h == value)
        && sum > vm_bytes
        && value <= vm_bytes)
        .then_some(value)
}

/// A container that carries the copied value declared nothing, so it holds no
/// huge pages of its own.
pub(crate) fn own(huge_pages: u64, copied: u64) -> u64 {
    if copied != 0 && huge_pages == copied {
        0
    } else {
        huge_pages
    }
}

fn ceiling(r: &Reservation, copied: u64) -> u64 {
    match own(r.huge_pages, copied) {
        0 => r.memory,
        own => own,
    }
}

/// Report memory shortfalls at admission to avoid overcommitting the guest.
pub(crate) fn refuse_pod_ceilings(
    rs: &[Reservation],
    copied: u64,
    guest: Guest,
    this: &str,
    name: &str,
) -> Result<()> {
    let holdable = guest.holdable();
    if holdable == 0 {
        return Ok(());
    }
    let ceilings = rs.iter().fold(0u64, |total, r| {
        // A running container counts with what it was given, even if a copy
        // found since then would give it less.
        let c = match r.applied {
            Some(given) if r.id != this => given,
            _ => ceiling(r, copied),
        };
        total.saturating_add(c)
    });
    if ceilings <= holdable {
        return Ok(());
    }
    Err(anyhow!(
        "container {name} brings the memory ceilings of the pod's containers to {} MiB, but the {} MiB guest holds {} MiB for them: the pod needs {} MiB more of the VM's huge pages, on the pod or as the runtime class's pod overhead",
        ceilings / MIB,
        guest.memory_mib,
        holdable / MIB,
        ((ceilings - holdable) / MIB).max(1)
    ))
}

/// Pod sizing accounts for the VM's backing page size only.
fn refuse_other_page_size(
    resources: Option<&LinuxResources>,
    page_size: &str,
    name: &str,
) -> Result<()> {
    match resources
        .and_then(|r| r.hugepage_limits().as_ref())
        .and_then(|limits| limits.iter().find(|l| l.page_size() != page_size && l.limit() != 0))
    {
        Some(other) => Err(anyhow!(
            "container {name} reserves huge pages of {}, but the VM's memory is on {page_size} pages: only {page_size} huge pages can be used in this runtime class",
            other.page_size()
        )),
        None => Ok(()),
    }
}

/// Changes nothing, so create and update both run it before their state
/// changes and a refusal needs no rollback.
pub(crate) fn plan(
    toml_config: &TomlConfig,
    pod: &PodHugePages,
    id: &str,
    name: &str,
    resources: Option<&LinuxResources>,
    creating: bool,
) -> Result<HugePagePlan> {
    let page = pod.page.clone();
    let guest = static_guest(toml_config);
    let vm_bytes = guest.memory_mib << 20;
    refuse_other_page_size(resources, &page, name)?;
    let this = reservation(id, resources, &page, None);
    let mut rs = pod.others.clone();
    rs.push(this.clone());
    // Only a create is refused for this. On an update the value counts as the
    // container's own.
    if let Some(value) = ambiguous_copy(&rs, vm_bytes).filter(|_| creating) {
        let resource = resource_name(&page);
        return Err(anyhow!(
            "the pod level {resource} is copied into every container that does not declare it, and each container of this pod carries {} MiB of it, so a copy cannot be told from a declaration: declare {resource} on every container of the pod",
            value >> 20
        ));
    }
    // Once declaring containers stop, a copied value may be treated as a
    // container reservation.
    let copied = copied_reservation(&rs, vm_bytes);
    refuse_pod_ceilings(&rs, copied, guest, id, name)?;
    Ok(HugePagePlan {
        page,
        copied,
        held: this.huge_pages,
        name: name.to_string(),
    })
}

/// An update carries only what changes, so the checks run on it merged with
/// what the container holds.
pub(crate) fn merged_update(
    held: Option<&LinuxResources>,
    update: &LinuxResources,
) -> LinuxResources {
    let mut merged = update.clone();
    // Kubernetes cannot change huge page limits, and the kubelet rebuilds them
    // for a resize, from the pod level value when there is one.
    merged.set_hugepage_limits(held.and_then(|l| l.hugepage_limits().clone()));
    let held_memory = held.and_then(|l| *l.memory());
    // The agent applies only a positive limit, so zero changes nothing in the guest.
    let new_limit = update.memory().and_then(|m| m.limit()).filter(|l| *l > 0);
    if new_limit.is_none() {
        let mut m = update.memory().or(held_memory).unwrap_or_default();
        m.set_limit(held_memory.and_then(|h| h.limit()));
        merged.set_memory(Some(m));
    }
    merged
}

/// Host huge pages back ordinary guest RAM, so the reservation belongs in
/// the guest memory limit.
pub(crate) fn bound_container(resources: &mut LinuxResources, plan: &HugePagePlan) {
    let page_size = plan.page.as_str();
    let own = own(huge_pages_of(Some(resources), page_size), plan.copied);
    if let Some(limits) = resources.hugepage_limits().clone() {
        let stripped: Vec<LinuxHugepageLimit> = limits
            .into_iter()
            .map(|mut l| {
                if l.page_size() == page_size {
                    l.set_limit(0);
                }
                l
            })
            .collect();
        resources.set_hugepage_limits(Some(stripped));
    }
    if own == 0 {
        return;
    }
    let mut memory = resources.memory().unwrap_or_default();
    let limit = own.min(i64::MAX as u64) as i64;
    memory.set_limit(Some(limit));
    if memory.reservation().is_some_and(|r| r > limit) {
        memory.set_reservation(Some(limit));
    }
    // OCI swap includes memory. Matching the guest limit leaves no swap allowance.
    if memory.swap().is_some_and(|s| s != 0 && s != limit) {
        memory.set_swap(Some(limit));
    }
    resources.set_memory(Some(memory));
}

/// An update without the VM's page size would otherwise reach the agent with
/// the memory limit alone, and the container would lose its ceiling.
pub(crate) fn fold_held(resources: &mut LinuxResources, page_size: &str, held: u64) {
    if held == 0 {
        return;
    }
    let mut limits = resources.hugepage_limits().clone().unwrap_or_default();
    if limits.iter().any(|l| l.page_size() == page_size) {
        return;
    }
    let mut l = LinuxHugepageLimit::default();
    l.set_page_size(page_size.to_string());
    l.set_limit(held.min(i64::MAX as u64) as i64);
    limits.push(l);
    resources.set_hugepage_limits(Some(limits));
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci_spec::runtime::LinuxMemory;

    const GIB: u64 = 1 << 30;

    fn r(id: &str, huge: u64, mem: u64) -> Reservation {
        Reservation {
            id: id.to_string(),
            huge_pages: huge,
            memory: mem,
            applied: None,
        }
    }

    fn res(huge: u64, mem: u64) -> LinuxResources {
        let mut l = LinuxHugepageLimit::default();
        l.set_page_size("1GB".to_string());
        l.set_limit(huge as i64);
        let mut m = LinuxMemory::default();
        m.set_limit(Some(mem as i64));
        let mut out = LinuxResources::default();
        out.set_hugepage_limits(Some(vec![l]));
        out.set_memory(Some(m));
        out
    }

    fn plan_for(page: &str, copied: u64) -> HugePagePlan {
        HugePagePlan {
            page: page.to_string(),
            copied,
            held: 0,
            name: String::new(),
        }
    }

    #[test]
    fn test_copied_reservation() {
        let rs = [
            r("main", 10 * GIB, 0),
            r("s1", 16 * GIB, 0),
            r("s2", 16 * GIB, 0),
        ];
        assert_eq!(copied_reservation(&rs, 18 * GIB), 16 * GIB);
        assert_eq!(
            copied_reservation(&[r("a", 12 * GIB, 0)], 12 * GIB),
            0,
            "lone container"
        );
        assert_eq!(
            copied_reservation(&[r("a", 8 * GIB, 0), r("b", GIB, 0)], 13 * GIB),
            0,
            "declarations"
        );
        assert_eq!(
            copied_reservation(&[r("a", 16 * GIB, 0), r("b", 16 * GIB, 0)], 18 * GIB),
            0,
            "cannot tell"
        );
        assert_eq!(
            ambiguous_copy(&[r("a", 16 * GIB, 0), r("b", 16 * GIB, 0)], 18 * GIB),
            Some(16 * GIB)
        );
        assert_eq!(
            ambiguous_copy(&[r("a", 4 * GIB, 0), r("b", 4 * GIB, 0)], 2 * GIB),
            None,
            "not sized from the pod"
        );
        assert_eq!(
            ambiguous_copy(&[r("a", 8 * GIB, 0), r("b", 8 * GIB, 0)], 18 * GIB),
            None,
            "fits"
        );
    }

    #[test]
    fn test_refuse_pod_ceilings() {
        let g = |mib, vcpus| Guest {
            memory_mib: mib,
            vcpus,
        };
        let rs = [r("app", 16 * GIB, 1 << 20), r("logging", 2 * GIB, 1 << 20)];
        assert!(refuse_pod_ceilings(&rs, 0, g(20480, 6), "logging", "logging").is_ok());
        let rs = [r("app", 16 * GIB, 2 * GIB), r("logging", 0, 2 * GIB)];
        let err = refuse_pod_ceilings(&rs, 0, g(18432, 5), "logging", "logging").unwrap_err();
        assert!(err.to_string().contains("the pod needs"));
        let unrecorded = [r("a", 16 * GIB, 512 << 20), r("b", 10 * GIB, 1 << 20)];
        let mut recorded = unrecorded.clone();
        recorded[0].applied = Some(16 * GIB);
        assert!(refuse_pod_ceilings(&recorded, 16 * GIB, g(24576, 3), "b", "b").is_err());
        assert!(
            refuse_pod_ceilings(&unrecorded, 16 * GIB, g(24576, 3), "b", "b").is_ok(),
            "without the record the copy reads as 512Mi"
        );
        assert!(refuse_pod_ceilings(&[r("a", 64 * GIB, 0)], 0, Guest::default(), "a", "a").is_ok());
        let err = refuse_pod_ceilings(&[r("app", 16 * GIB, 1 << 20)], 0, g(16384, 4), "app", "app")
            .unwrap_err();
        assert!(err.to_string().contains("1279 MiB more"), "{}", err);
    }

    #[test]
    fn test_refuse_other_page_size() {
        let mut other = res(0, 1 << 30);
        let mut l = LinuxHugepageLimit::default();
        l.set_page_size("2MB".to_string());
        l.set_limit(2 * GIB as i64);
        other.set_hugepage_limits(Some(vec![l]));
        assert!(refuse_other_page_size(Some(&other), "1GB", "x").is_err());
        assert!(refuse_other_page_size(Some(&res(8 * GIB, 0)), "1GB", "x").is_ok());
        assert_eq!(resource_name("1GB"), "hugepages-1Gi");
        assert_eq!(resource_name("2MB"), "hugepages-2Mi");
    }

    #[test]
    fn test_bound_container() {
        let mut host = res(8 * GIB, 1 << 20);
        bound_container(&mut host, &plan_for("1GB", 0));
        assert_eq!(memory_limit(Some(&host)), 8 * GIB);
        assert_eq!(
            huge_pages_of(Some(&host), "1GB"),
            0,
            "the guest keeps no huge pages for it"
        );

        let mut copy = res(16 * GIB, 512 << 20);
        bound_container(&mut copy, &plan_for("1GB", 16 * GIB));
        assert_eq!(
            memory_limit(Some(&copy)),
            512 << 20,
            "a copy sets no ceiling"
        );
    }

    #[test]
    fn test_merged_update_keeps_the_huge_pages() {
        let held = res(8 * GIB, 0);
        for update in [res(16 * GIB, 0), res(4 * GIB, 0), res(0, 0)] {
            let merged = merged_update(Some(&held), &update);
            assert_eq!(huge_pages_of(Some(&merged), "1GB"), 8 * GIB);
        }
        let merged = merged_update(Some(&held), &LinuxResources::default());
        assert_eq!(huge_pages_of(Some(&merged), "1GB"), 8 * GIB);
    }

    #[test]
    fn test_merged_update_keeps_the_limit_on_zero() {
        let mut held = LinuxResources::default();
        let mut m = LinuxMemory::default();
        m.set_limit(Some((8 * GIB) as i64));
        held.set_memory(Some(m));
        let mut update = LinuxResources::default();
        let mut zero = LinuxMemory::default();
        zero.set_limit(Some(0));
        update.set_memory(Some(zero));
        let merged = merged_update(Some(&held), &update);
        assert_eq!(memory_limit(Some(&merged)), 8 * GIB);
        assert_eq!(
            memory_limit(Some(&merged_update(
                Some(&held),
                &LinuxResources::default()
            ))),
            8 * GIB
        );
    }

    #[test]
    fn test_fold_held() {
        let mut update = LinuxResources::default();
        let mut m = LinuxMemory::default();
        m.set_limit(Some(16 << 20));
        update.set_memory(Some(m));
        fold_held(&mut update, "1GB", 8 * GIB);
        assert_eq!(huge_pages_of(Some(&update), "1GB"), 8 * GIB);
        let mut named = res(4 * GIB, 0);
        fold_held(&mut named, "1GB", 8 * GIB);
        assert_eq!(huge_pages_of(Some(&named), "1GB"), 4 * GIB);
    }
}
