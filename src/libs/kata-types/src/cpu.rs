// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

/// A list of CPU IDs.
#[derive(Debug)]
pub struct CpuSet(Vec<u32>);

impl CpuSet {
    /// Create a new instance of `CpuSet`.
    pub fn new() -> Self {
        CpuSet(vec![])
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
}

impl From<Vec<u32>> for CpuSet {
    fn from(mut cpus: Vec<u32>) -> Self {
        cpus.sort_unstable();
        cpus.dedup();
        CpuSet(cpus)
    }
}

/// Test whether two CPU sets are equal.
impl PartialEq for CpuSet {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_cpu_list_equal() {
        let cpuset1 = CpuSet::from(vec![1, 2, 3]);
        let cpuset2 = CpuSet::from(vec![3, 2, 1]);
        let cpuset3 = CpuSet::from(vec![]);
        let cpuset4 = CpuSet::from(vec![3, 2, 4]);
        let cpuset5 = CpuSet::from(vec![1, 2, 3, 3, 2, 1]);

        assert_eq!(cpuset1.len(), 3);
        assert!(cpuset3.is_empty());
        assert_eq!(cpuset5.len(), 3);

        assert_eq!(cpuset1, cpuset2);
        assert_eq!(cpuset1, cpuset5);
        assert_ne!(cpuset1, cpuset3);
        assert_ne!(cpuset1, cpuset4);
    }
}
