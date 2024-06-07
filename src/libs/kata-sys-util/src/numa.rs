// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs::DirEntry;
use std::io::Read;
use std::path::PathBuf;

use kata_types::cpu::CpuSet;
use lazy_static::lazy_static;

use crate::sl;
use std::str::FromStr;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid CPU number {0}")]
    InvalidCpu(u32),
    #[error("Invalid node file name {0}")]
    InvalidNodeFileName(String),
    #[error("Can not read directory {1}: {0}")]
    ReadDirectory(#[source] std::io::Error, String),
    #[error("Can not read from file {0}, {1:?}")]
    ReadFile(String, #[source] std::io::Error),
    #[error("Can not open from file {0}, {1:?}")]
    OpenFile(String, #[source] std::io::Error),
    #[error("Can not parse CPU info, {0:?}")]
    ParseCpuInfo(#[from] kata_types::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

// global config in UT
#[cfg(test)]
lazy_static! {
    static ref SYS_FS_PREFIX: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/texture");
    // numa node file for UT, we can mock data
    static ref NUMA_NODE_PATH: PathBuf = (*SYS_FS_PREFIX).join("sys/devices/system/node");
    // sysfs directory for CPU devices
    static ref NUMA_CPU_PATH: PathBuf = (*SYS_FS_PREFIX).join("sys/devices/system/cpu");
}

// global config in release
#[cfg(not(test))]
lazy_static! {
    // numa node file for UT, we can mock data
    static ref NUMA_NODE_PATH: PathBuf = PathBuf::from("/sys/devices/system/node");
    // sysfs directory for CPU devices
    static ref NUMA_CPU_PATH: PathBuf = PathBuf::from("/sys/devices/system/cpu");
}

const NUMA_NODE_PREFIX: &str = "node";
const NUMA_NODE_CPU_LIST_NAME: &str = "cpulist";

/// Get numa node id for a CPU
pub fn get_node_id(cpu: u32) -> Result<u32> {
    let path = NUMA_CPU_PATH.join(format!("cpu{}", cpu));
    let dirs = path.read_dir().map_err(|_| Error::InvalidCpu(cpu))?;

    for d in dirs {
        let d = d.map_err(|e| Error::ReadDirectory(e, path.to_string_lossy().to_string()))?;
        if let Some(file_name) = d.file_name().to_str() {
            if !file_name.starts_with(NUMA_NODE_PREFIX) {
                continue;
            }
            let index_str = file_name.trim_start_matches(NUMA_NODE_PREFIX);
            if let Ok(i) = index_str.parse::<u32>() {
                return Ok(i);
            }
        }
    }

    // Default to node 0 on UMA systems.
    Ok(0)
}

/// Map cpulist to NUMA node, returns a HashMap<numa_node_id, Vec<cpu_id>>.
pub fn get_node_map(cpus: &str) -> Result<HashMap<u32, Vec<u32>>> {
    // <numa id, Vec<cpu id> >
    let mut node_map: HashMap<u32, Vec<u32>> = HashMap::new();
    let cpuset = CpuSet::from_str(cpus)?;

    for c in cpuset.iter() {
        let node_id = get_node_id(*c)?;
        node_map.entry(node_id).or_default().push(*c);
    }

    Ok(node_map)
}

/// Get CPU to NUMA node mapping by reading `/sys/devices/system/node/nodex/cpulist`.
///
/// Return a HashMap<cpu id, node id>. The hashmap will be empty if NUMA is not enabled on the
/// system.
pub fn get_numa_nodes() -> Result<HashMap<u32, u32>> {
    let mut numa_nodes = HashMap::new();
    let numa_node_path = &*NUMA_NODE_PATH;
    if !numa_node_path.exists() {
        debug!(sl!(), "no numa node available on this system");
        return Ok(numa_nodes);
    }

    let dirs = numa_node_path
        .read_dir()
        .map_err(|e| Error::ReadDirectory(e, numa_node_path.to_string_lossy().to_string()))?;
    for d in dirs {
        match d {
            Err(e) => {
                return Err(Error::ReadDirectory(
                    e,
                    numa_node_path.to_string_lossy().to_string(),
                ))
            }
            Ok(d) => {
                if let Ok(file_name) = d.file_name().into_string() {
                    if file_name.starts_with(NUMA_NODE_PREFIX) {
                        let index_string = file_name.trim_start_matches(NUMA_NODE_PREFIX);
                        info!(
                            sl!(),
                            "get node dir {} node index {}", &file_name, index_string
                        );
                        match index_string.parse::<u32>() {
                            Ok(nid) => read_cpu_info_from_node(&d, nid, &mut numa_nodes)?,
                            Err(_e) => {
                                return Err(Error::InvalidNodeFileName(file_name.to_string()))
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(numa_nodes)
}

fn read_cpu_info_from_node(
    d: &DirEntry,
    node_index: u32,
    numa_nodes: &mut HashMap<u32, u32>,
) -> Result<()> {
    let cpu_list_path = d.path().join(NUMA_NODE_CPU_LIST_NAME);
    let mut file = std::fs::File::open(&cpu_list_path)
        .map_err(|e| Error::OpenFile(cpu_list_path.to_string_lossy().to_string(), e))?;
    let mut cpu_list_string = String::new();
    if let Err(e) = file.read_to_string(&mut cpu_list_string) {
        return Err(Error::ReadFile(
            cpu_list_path.to_string_lossy().to_string(),
            e,
        ));
    }
    let split_cpus = CpuSet::from_str(cpu_list_string.trim())?;
    info!(
        sl!(),
        "node {} list {:?} from {}", node_index, split_cpus, &cpu_list_string
    );
    for split_cpu_id in split_cpus.iter() {
        numa_nodes.insert(*split_cpu_id, node_index);
    }

    Ok(())
}

/// Check whether all specified CPUs have associated NUMA node.
pub fn is_valid_numa_cpu(cpus: &[u32]) -> Result<bool> {
    let numa_nodes = get_numa_nodes()?;

    for cpu in cpus {
        if numa_nodes.get(cpu).is_none() {
            return Ok(false);
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_node_id() {
        assert_eq!(get_node_id(0).unwrap(), 0);
        assert_eq!(get_node_id(1).unwrap(), 0);
        assert_eq!(get_node_id(64).unwrap(), 1);
        get_node_id(65).unwrap_err();
    }

    #[test]
    fn test_get_node_map() {
        let map = get_node_map("0-1,64").unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&0).unwrap().len(), 2);
        assert_eq!(map.get(&1).unwrap().len(), 1);

        get_node_map("0-1,64,65").unwrap_err();
    }

    #[test]
    fn test_get_numa_nodes() {
        let map = get_numa_nodes().unwrap();
        assert_eq!(map.len(), 65);
        assert_eq!(*map.get(&0).unwrap(), 0);
        assert_eq!(*map.get(&1).unwrap(), 0);
        assert_eq!(*map.get(&63).unwrap(), 0);
        assert_eq!(*map.get(&64).unwrap(), 1);
    }

    #[test]
    fn test_is_valid_numa_cpu() {
        assert!(is_valid_numa_cpu(&[0]).unwrap());
        assert!(is_valid_numa_cpu(&[1]).unwrap());
        assert!(is_valid_numa_cpu(&[63]).unwrap());
        assert!(is_valid_numa_cpu(&[64]).unwrap());
        assert!(is_valid_numa_cpu(&[0, 1, 64]).unwrap());
        assert!(!is_valid_numa_cpu(&[0, 1, 64, 65]).unwrap());
        assert!(!is_valid_numa_cpu(&[65]).unwrap());
    }
}
