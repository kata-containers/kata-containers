// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
};

use super::{Volume, BIND};
use crate::share_fs::ephemeral_path;
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use byte_unit::{Byte, Unit};
use hypervisor::{device::device_manager::DeviceManager, HUGETLBFS};
use kata_sys_util::{
    fs::get_base_name,
    mount::{get_mount_path, PROC_MOUNTS_FILE},
};
use kata_types::mount::KATA_EPHEMERAL_VOLUME_TYPE;
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

type PageSize = Byte;
type Limit = u64;

const NODEV: &str = "nodev";

// container hugepage
pub(crate) struct Hugepage {
    // storage info
    storage: Option<Storage>,
    // mount info
    mount: oci::Mount,
}

// handle hugepage
impl Hugepage {
    pub(crate) fn new(
        mount: &oci::Mount,
        hugepage_limits_map: HashMap<PageSize, Limit>,
        fs_options: Vec<String>,
    ) -> Result<Self> {
        // Create mount option string
        let page_size = get_page_size(fs_options).context("failed to get page size")?;
        let option = hugepage_limits_map
            .get(&page_size)
            .map(|limit| {
                format!(
                    "pagesize={},size={}",
                    page_size.get_adjusted_unit(Unit::B).get_value(),
                    limit
                )
            })
            .context("failed to get hugepage option")?;
        let base_name = get_base_name(get_mount_path(mount.source()).clone())?
            .into_string()
            .map_err(|e| anyhow!("failed to convert to string{:?}", e))?;
        let mut mount = mount.clone();
        // Set the mount source path to a path that resides inside the VM
        mount.set_source(Some(
            format!("{}{}{}", ephemeral_path(), "/", base_name).into(),
        ));
        // Set the mount type to "bind"
        mount.set_typ(Some(BIND.to_string()));

        // Create a storage struct so that kata agent is able to create
        // hugetlbfs backed volume inside the VM
        let storage = Storage {
            driver: KATA_EPHEMERAL_VOLUME_TYPE.to_string(),
            source: NODEV.to_string(),
            fs_type: HUGETLBFS.to_string(),
            mount_point: get_mount_path(mount.source()).clone(),
            options: vec![option],
            ..Default::default()
        };
        Ok(Self {
            storage: Some(storage),
            mount,
        })
    }
}

#[async_trait]
impl Volume for Hugepage {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        let s = if let Some(s) = self.storage.as_ref() {
            vec![s.clone()]
        } else {
            vec![]
        };
        Ok(s)
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        Ok(())
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

pub(crate) fn get_huge_page_option(m: &oci::Mount) -> Result<Option<Vec<String>>> {
    if m.source().is_none() {
        return Err(anyhow!("empty mount source"));
    }
    let file = File::open(PROC_MOUNTS_FILE).context("failed open file")?;
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        let items: Vec<&str> = line.split(' ').collect();
        if get_mount_path(m.source()).as_str() == items[1] && items[2] == HUGETLBFS {
            let fs_options: Vec<&str> = items[3].split(',').collect();
            return Ok(Some(
                fs_options
                    .iter()
                    .map(|&s| s.to_string())
                    .collect::<Vec<String>>(),
            ));
        }
    }
    Ok(None)
}

// TODO add hugepage limit to sandbox memory once memory hotplug is enabled
// https://github.com/kata-containers/kata-containers/issues/5880
pub(crate) fn get_huge_page_limits_map(spec: &oci::Spec) -> Result<HashMap<PageSize, Limit>> {
    let hugepage_limits_map = spec
        .linux()
        .as_ref()
        .and_then(|linux| linux.resources().as_ref())
        .map(|resources| {
            resources
                .hugepage_limits()
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|hugepage_limit| {
                    // the pagesize send from oci spec is MB or GB, change it to Mi and Gi
                    let page_size_str = hugepage_limit.page_size().replace('B', "i");
                    let page_size = Byte::parse_str(page_size_str, true)
                        .context("failed to create Byte object from String")?;
                    Ok((page_size, hugepage_limit.limit() as u64))
                })
                .collect::<Result<HashMap<_, _>>>()
        })
        .unwrap_or_else(|| Ok(HashMap::new()))?;

    Ok(hugepage_limits_map)
}

fn get_page_size(fs_options: Vec<String>) -> Result<Byte> {
    for fs_option in fs_options {
        if fs_option.starts_with("pagesize=") {
            let page_size = fs_option
                .strip_prefix("pagesize=")
                // the parameters passed are in unit M or G, append i to be Mi and Gi
                .map(|s| format!("{s}i"))
                .context("failed to strip prefix pagesize")?;
            return Byte::parse_str(page_size, true)
                .map_err(|_| anyhow!("failed to convert string to byte"));
        }
    }
    Err(anyhow!("failed to get page size"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, fs, path::PathBuf};

    use crate::volume::hugepage::{get_page_size, HUGETLBFS, NODEV};

    use super::{get_huge_page_limits_map, get_huge_page_option};
    use byte_unit::Byte;
    use nix::mount::{mount, umount, MsFlags};
    use oci::{
        LinuxBuilder, LinuxHugepageLimit, LinuxHugepageLimitBuilder, LinuxResourcesBuilder,
        SpecBuilder,
    };
    use test_utils::skip_if_not_root;

    /// List the huge page sizes the running kernel actually exposes via
    /// `/sys/kernel/mm/hugepages/hugepages-NkB`, rendered as binary-unit
    /// strings (e.g. "2Mi", "1Gi") that are accepted both by the kernel's
    /// `pagesize=...` mount option and by `byte_unit::Byte::parse_str(s,
    /// /*allow_binary=*/ true)`.
    ///
    /// This test was historically hard-coded to `["1Gi", "2Mi"]`, which
    /// happens to match what x86_64 Ubuntu kernels expose by default, but
    /// other architectures use different page sizes (s390x typically
    /// exposes "1Mi", ppc64le with 64K base pages typically exposes "16Mi"
    /// and/or "16Gi", etc.). Discovering them at runtime keeps the test
    /// arch-portable.
    fn supported_hugetlbfs_page_sizes() -> Vec<String> {
        let Ok(entries) = fs::read_dir("/sys/kernel/mm/hugepages") else {
            return Vec::new();
        };
        let mut sizes = Vec::new();
        for entry in entries.flatten() {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Some(kib) = name
                .strip_prefix("hugepages-")
                .and_then(|s| s.strip_suffix("kB"))
                .and_then(|s| s.parse::<u64>().ok())
            else {
                continue;
            };
            let s = if kib % (1024 * 1024) == 0 {
                format!("{}Gi", kib / (1024 * 1024))
            } else if kib % 1024 == 0 {
                format!("{}Mi", kib / 1024)
            } else {
                format!("{}Ki", kib)
            };
            sizes.push(s);
        }
        sizes
    }

    #[test]
    fn test_get_huge_page_option() {
        let format_sizes = ["1GB", "2MB"];
        let mut huge_page_limits: Vec<LinuxHugepageLimit> = vec![];
        for format_size in format_sizes {
            let hugetlb = LinuxHugepageLimitBuilder::default()
                .page_size(format_size.to_string())
                .limit(100000)
                .build()
                .unwrap();
            huge_page_limits.push(hugetlb);
        }

        let spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .resources(
                        LinuxResourcesBuilder::default()
                            .hugepage_limits(huge_page_limits)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        assert!(get_huge_page_limits_map(&spec).is_ok());

        let mut expect_res = HashMap::new();
        expect_res.insert(Byte::parse_str("1Gi", false).ok().unwrap(), 100000);
        expect_res.insert(Byte::parse_str("2Mi", false).ok().unwrap(), 100000);
        assert_eq!(get_huge_page_limits_map(&spec).unwrap(), expect_res);
    }

    #[test]
    fn test_get_huge_page_size() {
        skip_if_not_root!();
        let format_sizes = supported_hugetlbfs_page_sizes();
        if format_sizes.is_empty() {
            // No hugetlbfs pools on this kernel (e.g. hugetlbfs is
            // unconfigured or /sys isn't mounted in the test environment);
            // nothing meaningful to round-trip.
            return;
        }
        // Probe once before iterating: some CI runners (e.g. the
        // ubuntu-24.04-s390x GHA runner) report supported huge-page sizes via
        // /sys but execute the test inside a user/mount namespace where
        // mount(2) of hugetlbfs is forbidden (EPERM) even when running as
        // root. There's no portable capability bit we can sniff for that, so
        // just try once and bail out cleanly if the kernel won't let us mount
        // hugetlbfs at all -- skipping is more honest than failing on
        // something this test can't control. A real regression on a host
        // where mount() *does* work will still surface inside the loop below.
        // Hugetlbfs's `pagesize=` mount option expects the kernel-native
        // shorthand ("2M", "1G"), not byte_unit's IEC form ("2Mi", "1Gi"):
        // it parses the value with `memparse()`, and `/proc/mounts` always
        // renders it back as `pagesize=<N>{K,M,G}` regardless of input. Pass
        // the trimmed form to mount(2) so the test doesn't rely on the
        // kernel silently ignoring the trailing `i`, and keep the IEC form
        // for the `Byte::parse_str(_, /*allow_binary=*/ true)` comparison.
        let probe_dir = tempfile::tempdir().unwrap();
        let probe_dst = probe_dir
            .path()
            .join(format!("hugepages-probe-{}", format_sizes[0]));
        fs::create_dir_all(&probe_dst).unwrap();
        let probe_kernel_size = format_sizes[0].trim_end_matches('i');
        if let Err(e) = mount(
            Some(NODEV),
            &probe_dst,
            Some(HUGETLBFS),
            MsFlags::MS_NODEV,
            Some(format!("pagesize={}", probe_kernel_size).as_str()),
        ) {
            eprintln!(
                "test_get_huge_page_size: skipping, hugetlbfs mount probe failed \
                 (pagesize={}): {}",
                probe_kernel_size, e
            );
            return;
        }
        umount(&probe_dst).unwrap();

        for format_size in format_sizes {
            let dir = tempfile::tempdir().unwrap();
            let dst = dir.path().join(format!("hugepages-{}", format_size));
            fs::create_dir_all(&dst).unwrap();
            let kernel_size = format_size.trim_end_matches('i');
            mount(
                Some(NODEV),
                &dst,
                Some(HUGETLBFS),
                MsFlags::MS_NODEV,
                Some(format!("pagesize={}", kernel_size).as_str()),
            )
            .unwrap();
            let mut mount = oci::Mount::default();
            mount.set_source(Some(PathBuf::from(dst.to_str().unwrap())));

            let option = get_huge_page_option(&mount).unwrap().unwrap();
            let page_size = get_page_size(option).unwrap();
            assert_eq!(page_size, Byte::parse_str(&format_size, true).unwrap());
            umount(&dst).unwrap();
            fs::remove_dir(&dst).unwrap();
        }
    }
}
