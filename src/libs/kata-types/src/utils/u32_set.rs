// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::ops::Deref;
use std::slice::Iter;
use std::str::FromStr;

use crate::Error;

/// A set of unique `u32` IDs.
///
/// The `U32Set` may be used to save CPUs parsed from a CPU list file or NUMA nodes parsed from
/// a NUMA node list file.
#[derive(Clone, Default, Debug)]
pub struct U32Set(Vec<u32>);

impl U32Set {
    /// Create a new instance of `U32Set`.
    pub fn new() -> Self {
        U32Set(vec![])
    }

    /// Add the `cpu` to the CPU set.
    pub fn add(&mut self, cpu: u32) {
        self.0.push(cpu);
        self.0.sort_unstable();
        self.0.dedup();
    }

    /// Add new CPUs into the set.
    pub fn extend(&mut self, cpus: &[u32]) {
        self.0.extend_from_slice(cpus);
        self.0.sort_unstable();
        self.0.dedup();
    }

    /// Returns true if the CPU set contains elements.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get number of elements in the CPU set.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Get an iterator over the CPU set.
    pub fn iter(&self) -> Iter<u32> {
        self.0.iter()
    }
}

impl From<Vec<u32>> for U32Set {
    fn from(mut cpus: Vec<u32>) -> Self {
        cpus.sort_unstable();
        cpus.dedup();
        U32Set(cpus)
    }
}

impl FromStr for U32Set {
    type Err = Error;

    fn from_str(cpus_str: &str) -> Result<Self, Self::Err> {
        if cpus_str.is_empty() {
            return Ok(U32Set::new());
        }

        let mut cpus = Vec::new();
        for split_cpu in cpus_str.split(',') {
            if !split_cpu.contains('-') {
                if !split_cpu.is_empty() {
                    if let Ok(cpu_id) = split_cpu.parse::<u32>() {
                        cpus.push(cpu_id);
                        continue;
                    }
                }
            } else {
                let fields: Vec<&str> = split_cpu.split('-').collect();
                if fields.len() == 2 {
                    if let Ok(start) = fields[0].parse::<u32>() {
                        if let Ok(end) = fields[1].parse::<u32>() {
                            if start < end {
                                for cpu in start..=end {
                                    cpus.push(cpu);
                                }
                                continue;
                            }
                        }
                    }
                }
            }

            return Err(Error::InvalidList(cpus_str.to_string()));
        }

        Ok(U32Set::from(cpus))
    }
}

impl Deref for U32Set {
    type Target = [u32];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Test whether two CPU sets are equal.
impl PartialEq for U32Set {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_cpuset_equal() {
        let cpuset1 = U32Set::from(vec![1, 2, 3]);
        let cpuset2 = U32Set::from(vec![3, 2, 1]);
        let cpuset3 = U32Set::from(vec![]);
        let cpuset4 = U32Set::from(vec![3, 2, 4]);
        let cpuset5 = U32Set::from(vec![1, 2, 3, 3, 2, 1]);

        assert_eq!(cpuset1.len(), 3);
        assert!(cpuset3.is_empty());
        assert_eq!(cpuset5.len(), 3);

        assert_eq!(cpuset1, cpuset2);
        assert_eq!(cpuset1, cpuset5);
        assert_ne!(cpuset1, cpuset3);
        assert_ne!(cpuset1, cpuset4);
    }

    #[test]
    fn test_cpuset_from_str() {
        assert!(U32Set::from_str("").unwrap().is_empty());

        let support_cpus1 = U32Set::from(vec![1, 2, 3]);
        assert_eq!(support_cpus1, U32Set::from_str("1,2,3").unwrap());
        assert_eq!(support_cpus1, U32Set::from_str("1-2,3").unwrap());

        let support_cpus2 = U32Set::from(vec![1, 3, 4, 6, 7, 8]);
        assert_eq!(support_cpus2, U32Set::from_str("1,3,4,6,7,8").unwrap());
        assert_eq!(support_cpus2, U32Set::from_str("1,3-4,6-8").unwrap());

        assert!(U32Set::from_str("1-2-3,3").is_err());
        assert!(U32Set::from_str("1-2,,3").is_err());
        assert!(U32Set::from_str("1-2.5,3").is_err());
        assert!(U32Set::from_str("1-1").is_err());
        assert!(U32Set::from_str("2-1").is_err());
        assert!(U32Set::from_str("0,,1").is_err());
        assert!(U32Set::from_str("-1").is_err());
        assert!(U32Set::from_str("1-").is_err());
        assert!(U32Set::from_str("-1--2").is_err());
        assert!(U32Set::from_str("999999999999999999999999999999999999999999999").is_err());
    }
}
