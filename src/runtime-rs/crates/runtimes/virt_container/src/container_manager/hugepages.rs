// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use oci_spec::runtime::{LinuxHugepageLimit, LinuxResources};
use resource::cpu_mem::hugepage_sizing::holdable_guest_bytes;

const MIB: u64 = 1 << 20;

/// What one running container of the pod carries.
#[derive(Clone, Debug, Default)]
pub(crate) struct Reservation {
    pub id: String,
    pub huge_pages: u64,
    pub memory: i64,
}

/// The pod's huge page state kept across its containers.
#[derive(Debug, Default)]
pub(crate) struct HugePageBook {
    /// The memory ceiling each container was given in the guest.
    pub applied: HashMap<String, u64>,
}

/// The guest a pod sized VM boots, zero when not static.
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

/// The bytes of pages of page_size a container reserved.
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

/// The bytes a memory limit carries, zero for none.
pub(crate) fn memory_limit(resources: Option<&LinuxResources>) -> i64 {
    resources
        .and_then(|r| r.memory().as_ref())
        .and_then(|m| m.limit())
        .filter(|l| *l > 0 && *l != i64::MAX)
        .unwrap_or(0)
}

/// The pod level value the kubelet copied: the largest value when the sum is past the VM.
pub(crate) fn copied_reservation(rs: &[Reservation], vm_bytes: u64) -> u64 {
    if vm_bytes == 0 {
        return 0;
    }
    let largest = rs.iter().map(|r| r.huge_pages).max().unwrap_or(0);
    let sum = rs.iter().fold(0u64, |s, r| s.saturating_add(r.huge_pages));
    let below = rs
        .iter()
        .any(|r| r.huge_pages != 0 && r.huge_pages < largest);
    if sum <= vm_bytes || !below || largest > vm_bytes {
        return 0;
    }
    largest
}

/// The value every container carries past the VM, copy or not.
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

pub(crate) fn own(huge_pages: u64, copied: u64) -> u64 {
    if copied != 0 && huge_pages == copied {
        0
    } else {
        huge_pages
    }
}

fn ceiling(r: &Reservation, copied: u64) -> u64 {
    match own(r.huge_pages, copied) {
        0 => r.memory.max(0) as u64,
        own => own,
    }
}

/// Refuses when the pod's memory ceilings, as given, do not fit in the guest.
pub(crate) fn refuse_pod_ceilings(
    rs: &[Reservation],
    copied: u64,
    applied: &HashMap<String, u64>,
    guest: Guest,
    this: &str,
    name: &str,
) -> Result<()> {
    let holdable = guest.holdable();
    if holdable == 0 {
        return Ok(());
    }
    let ceilings = rs.iter().fold(0u64, |total, r| {
        let c = match applied.get(&r.id) {
            Some(&given) if r.id != this => given,
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

fn refuse_ceiling_past_guest(name: &str, ceiling: u64, guest: Guest) -> Result<()> {
    let holdable = guest.holdable();
    if guest.memory_mib == 0 || ceiling <= holdable {
        return Ok(());
    }
    Err(anyhow!(
        "container {name} declares a {} MiB memory ceiling, but the {} MiB guest holds {} MiB for it: the pod needs {} MiB more of the VM's huge pages, on the pod or as the runtime class's pod overhead",
        ceiling / MIB,
        guest.memory_mib,
        holdable / MIB,
        ((ceiling - holdable) / MIB).max(1)
    ))
}

/// Bounds a container in a guest sized from the pod by its own huge pages.
pub(crate) fn bound_container(
    resources: &mut LinuxResources,
    page_size: &str,
    guest: Guest,
    copied: u64,
    name: &str,
) -> Result<()> {
    if let Some(other) = resources.hugepage_limits().as_ref().and_then(|limits| {
        limits
            .iter()
            .find(|l| l.page_size() != page_size && l.limit() != 0)
            .map(|l| l.page_size().clone())
    }) {
        return Err(anyhow!(
            "container {name} reserves huge pages of {other}, but the VM's memory is on {page_size} pages: only {page_size} huge pages can be used in this runtime class"
        ));
    }
    let own = own(huge_pages_of(Some(resources), page_size), copied);
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
        return refuse_ceiling_past_guest(name, memory_limit(Some(resources)) as u64, guest);
    }
    let mut memory = resources.memory().unwrap_or_default();
    let limit = own.min(i64::MAX as u64) as i64;
    memory.set_limit(Some(limit));
    if memory.reservation().is_some_and(|r| r > limit) {
        memory.set_reservation(Some(limit));
    }
    if memory.swap().is_some_and(|s| s != 0 && s != limit) {
        memory.set_swap(Some(limit));
    }
    resources.set_memory(Some(memory));
    refuse_ceiling_past_guest(name, own, guest)
}

/// Keeps one entry per page size, the last, and the held one where a resize says zero.
pub(crate) fn updated_limits(
    held: Option<&Vec<LinuxHugepageLimit>>,
    update: Option<&Vec<LinuxHugepageLimit>>,
) -> Option<Vec<LinuxHugepageLimit>> {
    let update = match update {
        Some(u) if !u.is_empty() => u,
        _ => return held.cloned(),
    };
    let mut sizes: Vec<String> = Vec::new();
    let mut held_by: HashMap<String, i64> = HashMap::new();
    let mut update_by: HashMap<String, i64> = HashMap::new();
    for l in held.into_iter().flatten() {
        if !sizes.contains(l.page_size()) {
            sizes.push(l.page_size().clone());
        }
        held_by.insert(l.page_size().clone(), l.limit());
    }
    for l in update {
        if !sizes.contains(l.page_size()) {
            sizes.push(l.page_size().clone());
        }
        update_by.insert(l.page_size().clone(), l.limit());
    }
    Some(
        sizes
            .into_iter()
            .map(|size| {
                let limit = match update_by.get(&size) {
                    Some(&v) if v != 0 => v,
                    _ => held_by.get(&size).copied().unwrap_or(0),
                };
                let mut l = LinuxHugepageLimit::default();
                l.set_page_size(size);
                l.set_limit(limit);
                l
            })
            .collect(),
    )
}

/// Puts the held reservation back into an update that names none of the VM's page size.
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
            memory: mem as i64,
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
        let none = HashMap::new();
        let g = |mib, vcpus| Guest {
            memory_mib: mib,
            vcpus,
        };
        let rs = [r("app", 16 * GIB, 1 << 20), r("logging", 2 * GIB, 1 << 20)];
        assert!(refuse_pod_ceilings(&rs, 0, &none, g(20480, 6), "logging", "logging").is_ok());
        let rs = [r("app", 16 * GIB, 2 * GIB), r("logging", 0, 2 * GIB)];
        let err =
            refuse_pod_ceilings(&rs, 0, &none, g(18432, 5), "logging", "logging").unwrap_err();
        assert!(err.to_string().contains("the pod needs"));
        let rs = [r("a", 16 * GIB, 512 << 20), r("b", 10 * GIB, 1 << 20)];
        let applied = HashMap::from([("a".to_string(), 16 * GIB)]);
        assert!(refuse_pod_ceilings(&rs, 16 * GIB, &applied, g(24576, 3), "b", "b").is_err());
        assert!(
            refuse_pod_ceilings(&rs, 16 * GIB, &none, g(24576, 3), "b", "b").is_ok(),
            "without the record the copy reads as 512Mi"
        );
        assert!(
            refuse_pod_ceilings(&[r("a", 64 * GIB, 0)], 0, &none, Guest::default(), "a", "a")
                .is_ok()
        );
    }

    #[test]
    fn test_bound_container() {
        let g = Guest {
            memory_mib: 16384,
            vcpus: 4,
        };
        let mut host = res(8 * GIB, 1 << 20);
        bound_container(&mut host, "1GB", g, 0, "main").unwrap();
        assert_eq!(memory_limit(Some(&host)), (8 * GIB) as i64);
        assert_eq!(
            huge_pages_of(Some(&host), "1GB"),
            0,
            "the guest keeps no huge pages for it"
        );

        let mut copy = res(16 * GIB, 512 << 20);
        bound_container(&mut copy, "1GB", g, 16 * GIB, "s1").unwrap();
        assert_eq!(
            memory_limit(Some(&copy)),
            512 << 20,
            "a copy is not a ceiling"
        );

        let mut too_big = res(16 * GIB, 1 << 20);
        let err = bound_container(&mut too_big, "1GB", g, 0, "app").unwrap_err();
        assert!(err.to_string().contains("1279 MiB more"), "{}", err);

        let mut other = res(0, 1 << 30);
        let mut l = LinuxHugepageLimit::default();
        l.set_page_size("2MB".to_string());
        l.set_limit(2 * GIB as i64);
        other.set_hugepage_limits(Some(vec![l]));
        assert!(bound_container(&mut other, "1GB", g, 0, "x").is_err());
    }

    #[test]
    fn test_updated_limits() {
        let held = res(8 * GIB, 0).hugepage_limits().clone().unwrap();
        let mut zero = LinuxHugepageLimit::default();
        zero.set_page_size("1GB".to_string());
        zero.set_limit(0);
        let update = vec![held[0].clone(), zero];
        let out = updated_limits(Some(&held), Some(&update)).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].limit(),
            (8 * GIB) as i64,
            "a resize keeps the held pages"
        );
        assert_eq!(
            updated_limits(Some(&held), None).unwrap()[0].limit(),
            (8 * GIB) as i64
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
