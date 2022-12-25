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

use crate::share_fs::EPHEMERAL_PATH;
use agent::Storage;
use anyhow::{anyhow, Context, Ok, Result};
use async_trait::async_trait;
use byte_unit::Byte;
use hypervisor::HUGETLBFS;
use kata_sys_util::{fs::get_base_name, mount::PROC_MOUNTS_FILE};
use kata_types::mount::KATA_EPHEMERAL_VOLUME_TYPE;

use super::{Volume, BIND};

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
            .map(|limit| format!("pagesize={},size={}", page_size.get_bytes(), limit))
            .context("failed to get hugepage option")?;
        let base_name = get_base_name(mount.source.clone())?
            .into_string()
            .map_err(|e| anyhow!("failed to convert to string{:?}", e))?;
        let mut mount = mount.clone();
        // Set the mount source path to a path that resides inside the VM
        mount.source = format!("{}{}{}", EPHEMERAL_PATH, "/", base_name);
        // Set the mount type to "bind"
        mount.r#type = BIND.to_string();

        // Create a storage struct so that kata agent is able to create
        // hugetlbfs backed volume inside the VM
        let storage = Storage {
            driver: KATA_EPHEMERAL_VOLUME_TYPE.to_string(),
            source: NODEV.to_string(),
            fs_type: HUGETLBFS.to_string(),
            mount_point: mount.source.clone(),
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

    async fn cleanup(&self) -> Result<()> {
        Ok(())
    }
}

pub(crate) fn get_huge_page_option(m: &oci::Mount) -> Result<Option<Vec<String>>> {
    if m.source.is_empty() {
        return Err(anyhow!("empty mount source"));
    }
    let file = File::open(PROC_MOUNTS_FILE).context("failed open file")?;
    let reader = BufReader::new(file);
    for line in reader.lines().flatten() {
        let items: Vec<&str> = line.split(' ').collect();
        if m.source == items[1] && items[2] == HUGETLBFS {
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
    let mut hugepage_limits_map: HashMap<PageSize, Limit> = HashMap::new();
    if let Some(l) = &spec.linux {
        if let Some(r) = &l.resources {
            let hugepage_limits = r.hugepage_limits.clone();
            for hugepage_limit in hugepage_limits {
                // the pagesize send from oci spec is MB or GB, change it to Mi and Gi
                let page_size = hugepage_limit.page_size.replace('B', "i");
                let page_size = Byte::from_str(page_size)
                    .context("failed to create Byte object from String")?;
                hugepage_limits_map.insert(page_size, hugepage_limit.limit);
            }
            return Ok(hugepage_limits_map);
        }
        return Ok(hugepage_limits_map);
    }
    Ok(hugepage_limits_map)
}

fn get_page_size(fs_options: Vec<String>) -> Result<Byte> {
    for fs_option in fs_options {
        if fs_option.starts_with("pagesize=") {
            let page_size = fs_option
                .strip_prefix("pagesize=")
                // the parameters passed are in unit M or G, append i to be Mi and Gi
                .map(|s| format!("{}i", s))
                .context("failed to strip prefix pagesize")?;
            return Byte::from_str(page_size)
                .map_err(|_| anyhow!("failed to convert string to byte"));
        }
    }
    Err(anyhow!("failed to get page size"))
}

#[cfg(test)]
mod tests {

    use std::{collections::HashMap, fs};

    use crate::volume::hugepage::{get_page_size, HUGETLBFS, NODEV};

    use super::{get_huge_page_limits_map, get_huge_page_option};
    use byte_unit::Byte;
    use nix::mount::{mount, umount, MsFlags};
    use oci::{Linux, LinuxHugepageLimit, LinuxResources};
    use test_utils::skip_if_not_root;

    #[test]
    fn test_get_huge_page_option() {
        let format_sizes = ["1GB", "2MB"];
        let mut huge_page_limits: Vec<LinuxHugepageLimit> = vec![];
        for format_size in format_sizes {
            huge_page_limits.push(LinuxHugepageLimit {
                page_size: format_size.to_string(),
                limit: 100000,
            });
        }

        let spec = oci::Spec {
            linux: Some(Linux {
                resources: Some(LinuxResources {
                    hugepage_limits: huge_page_limits,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert!(get_huge_page_limits_map(&spec).is_ok());

        let mut expect_res = HashMap::new();
        expect_res.insert(Byte::from_str("1Gi").ok().unwrap(), 100000);
        expect_res.insert(Byte::from_str("2Mi").ok().unwrap(), 100000);
        assert_eq!(get_huge_page_limits_map(&spec).unwrap(), expect_res);
    }

    #[test]
    fn test_get_huge_page_size() {
        skip_if_not_root!();
        let format_sizes = ["1Gi", "2Mi"];
        for format_size in format_sizes {
            let dir = tempfile::tempdir().unwrap();
            let dst = dir.path().join(format!("hugepages-{}", format_size));
            fs::create_dir_all(&dst).unwrap();
            mount(
                Some(NODEV),
                &dst,
                Some(HUGETLBFS),
                MsFlags::MS_NODEV,
                Some(format!("pagesize={}", format_size).as_str()),
            )
            .unwrap();
            let mount = oci::Mount {
                source: dst.to_str().unwrap().to_string(),
                ..Default::default()
            };
            let option = get_huge_page_option(&mount).unwrap().unwrap();
            let page_size = get_page_size(option).unwrap();
            assert_eq!(page_size, Byte::from_str(format_size).unwrap());
            umount(&dst).unwrap();
            fs::remove_dir(&dst).unwrap();
        }
    }
}
