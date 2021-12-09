// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

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
