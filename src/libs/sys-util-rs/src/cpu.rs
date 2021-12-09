// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashSet;

use crate::sl;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid CPU list {0}")]
    InvalidCpuList(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Parse cpuset in form of "1,3,5" and "1-4,6,8".
pub fn split_cpus(cpus_string: &str) -> Result<Vec<u32>> {
    let mut cpu_list = HashSet::new();

    if !cpus_string.is_empty() {
        'next_token: for split_cpu in cpus_string.split(',') {
            let cpus: Vec<&str> = split_cpu.split('-').collect();
            if cpus.len() == 1 {
                let value = cpus[0];
                if !value.is_empty() {
                    if let Ok(cpu_id) = value.parse::<u32>() {
                        cpu_list.insert(cpu_id);
                        continue 'next_token;
                    }
                }
            } else if cpus.len() == 2 {
                if let Ok(start) = cpus[0].parse::<u32>() {
                    if let Ok(end) = cpus[1].parse::<u32>() {
                        for cpu in start..=end {
                            cpu_list.insert(cpu);
                        }
                        continue 'next_token;
                    }
                }
            }
            return Err(Error::InvalidCpuList(cpus_string.to_string()));
        }
    }

    let mut result = cpu_list.into_iter().collect::<Vec<_>>();
    result.sort_unstable();
    info!(sl!(), "get cpu list {:?} from {}", result, cpus_string);

    Ok(result)
}

/// Test whether two CPU sets are equal.
pub fn is_cpu_list_equal(left: &[u32], right: &[u32]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut l = left.to_vec();
    let mut r = right.to_vec();
    l.sort_unstable();
    r.sort_unstable();
    l == r
}

/// Convert CPU quota and period into milli-CPUs
///
/// If quota is -1, it means the CPU resource request is unconstrained.
pub fn calculate_milli_cpus(quota: i64, period: u64) -> (u32, bool) {
    if quota >= 0 && period != 0 {
        ((quota as u64 * 1000 / period) as u32, false)
    } else {
        (0, quota == -1)
    }
}

/// Convert from mCPU to CPU, taking the ceiling value.
pub fn calculate_vcpus_from_milli_cpus(m_cpu: u32) -> u32 {
    (m_cpu + 999) / 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_cpus() {
        assert_eq!(0, split_cpus("").unwrap().len());

        let support_cpus1 = vec![1, 2, 3];
        assert_eq!(support_cpus1, split_cpus("1,2,3").unwrap());
        assert_eq!(support_cpus1, split_cpus("1-2,3").unwrap());

        let support_cpus2 = vec![1, 3, 4, 6, 7, 8];
        assert_eq!(support_cpus2, split_cpus("1,3,4,6,7,8").unwrap());
        assert_eq!(support_cpus2, split_cpus("1,3-4,6-8").unwrap());

        assert!(split_cpus("1-2-3,3").is_err());
        assert!(split_cpus("1-2,,3").is_err());
        assert!(split_cpus("1-2.5,3").is_err());
    }

    #[test]
    fn test_is_cpu_list_equal() {
        let cpu_list1 = vec![1, 2, 3];
        let cpu_list2 = vec![3, 2, 1];
        let cpu_list3 = vec![];
        let cpu_list4 = vec![3, 2, 4];

        assert!(is_cpu_list_equal(&cpu_list1, &cpu_list2));
        assert!(!is_cpu_list_equal(&cpu_list1, &cpu_list3));
        assert!(!is_cpu_list_equal(&cpu_list1, &cpu_list4));
    }

    #[test]
    fn test_calculate_milli_cpus() {
        let (r, no_limit) = calculate_milli_cpus(2000, 1000);
        assert_eq!(r, 2000);
        assert!(!no_limit);

        let (r, no_limit) = calculate_milli_cpus(250, 100);
        assert_eq!(r, 2500);
        assert!(!no_limit);

        let (r, no_limit) = calculate_milli_cpus(0, 0);
        assert_eq!(r, 0);
        assert!(!no_limit);

        let (r, no_limit) = calculate_milli_cpus(-1, 0);
        assert_eq!(r, 0);
        assert!(no_limit);

        let (r, no_limit) = calculate_milli_cpus(-1, 100);
        assert_eq!(r, 0);
        assert!(no_limit);
    }

    #[test]
    fn test_calculate_vcpus_from_milli_cpus() {
        assert_eq!(calculate_vcpus_from_milli_cpus(0), 0);
        assert_eq!(calculate_vcpus_from_milli_cpus(1), 1);
        assert_eq!(calculate_vcpus_from_milli_cpus(999), 1);
        assert_eq!(calculate_vcpus_from_milli_cpus(1000), 1);
        assert_eq!(calculate_vcpus_from_milli_cpus(1001), 2);
    }
}
