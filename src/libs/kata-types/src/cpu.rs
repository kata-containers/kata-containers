// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use oci_spec::runtime as oci;
use std::convert::TryFrom;
use std::str::FromStr;

/// A set of CPU ids.
pub type CpuSet = crate::utils::u32_set::U32Set;

/// A set of NUMA memory nodes.
pub type NumaNodeSet = crate::utils::u32_set::U32Set;

/// Error code for CPU related operations.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Invalid CPU list.
    #[error("Invalid CPU list: {0}")]
    InvalidCpuSet(crate::Error),
    /// Invalid NUMA memory node list.
    #[error("Invalid NUMA memory node list: {0}")]
    InvalidNodeSet(crate::Error),
}

/// Assigned CPU resources for a Linux container.
/// Stores fractional vCPU allocation for more precise resource tracking.
#[derive(Clone, Default, Debug)]
pub struct LinuxContainerCpuResources {
    shares: u64,
    period: u64,
    quota: i64,
    cpuset: CpuSet,
    nodeset: NumaNodeSet,
    /// Calculated fractional vCPU allocation, e.g., 0.25 means 1/4 of a CPU.
    calculated_vcpu: Option<f64>,
}

impl LinuxContainerCpuResources {
    /// Get the CPU shares.
    pub fn shares(&self) -> u64 {
        self.shares
    }

    /// Get the CPU schedule period.
    pub fn period(&self) -> u64 {
        self.period
    }

    /// Get the CPU schedule quota.
    pub fn quota(&self) -> i64 {
        self.quota
    }

    /// Get the CPU set.
    pub fn cpuset(&self) -> &CpuSet {
        &self.cpuset
    }

    /// Get the NUMA memory node set.
    pub fn nodeset(&self) -> &NumaNodeSet {
        &self.nodeset
    }

    /// Get the number of vCPUs assigned to the container as a fractional value.
    /// Returns `None` if unconstrained (no limit).
    pub fn get_vcpus(&self) -> Option<f64> {
        self.calculated_vcpu
    }
}

impl TryFrom<&oci::LinuxCpu> for LinuxContainerCpuResources {
    type Error = Error;

    // Unhandled fields: realtime_runtime, realtime_period, mems
    fn try_from(value: &oci::LinuxCpu) -> Result<Self, Self::Error> {
        let period = value.period().unwrap_or(0);
        let quota = value.quota().unwrap_or(-1);
        let value_cpus = value.cpus().as_deref().unwrap_or("");
        let cpuset = CpuSet::from_str(value_cpus).map_err(Error::InvalidCpuSet)?;
        let value_mems = value.mems().as_deref().unwrap_or("");
        let nodeset = NumaNodeSet::from_str(value_mems).map_err(Error::InvalidNodeSet)?;

        // Calculate fractional vCPUs:
        // If quota >= 0 and period > 0, vCPUs = quota / period.
        // Otherwise, if cpuset is non-empty, derive from cpuset length.
        let vcpu_fraction = if quota >= 0 && period > 0 {
            Some(quota as f64 / period as f64)
        } else if !cpuset.is_empty() {
            Some(cpuset.len() as f64)
        } else {
            None
        };

        Ok(LinuxContainerCpuResources {
            shares: value.shares().unwrap_or(0),
            period,
            quota,
            cpuset,
            nodeset,
            calculated_vcpu: vcpu_fraction,
        })
    }
}

/// Aggregated CPU resources for a Linux sandbox/pod.
/// Tracks cumulative fractional vCPU allocation across all containers in the pod.
#[derive(Default, Debug)]
pub struct LinuxSandboxCpuResources {
    shares: u64,
    /// Total fractional vCPU allocation for the sandbox.
    calculated_vcpu: f64,
    cpuset: CpuSet,
    nodeset: NumaNodeSet,
}

impl LinuxSandboxCpuResources {
    /// Create a new instance of `LinuxSandboxCpuResources`.
    pub fn new(shares: u64) -> Self {
        Self {
            shares,
            ..Default::default()
        }
    }

    /// Get the CPU shares.
    pub fn shares(&self) -> u64 {
        self.shares
    }

    /// Return the cumulative fractional vCPU allocation for the sandbox.
    pub fn calculated_vcpu(&self) -> f64 {
        self.calculated_vcpu
    }

    /// Get the CPU set.
    pub fn cpuset(&self) -> &CpuSet {
        &self.cpuset
    }

    /// Get the NUMA memory node set.
    pub fn nodeset(&self) -> &NumaNodeSet {
        &self.nodeset
    }

    /// Get the number of vCPUs for the sandbox as a fractional value.
    /// If no quota and cpuset is defined, return cpuset length as float.
    pub fn get_vcpus(&self) -> f64 {
        if self.calculated_vcpu == 0.0 {
            if !self.cpuset.is_empty() {
                return self.cpuset.len() as f64;
            }
            return 0.0;
        }
        self.calculated_vcpu
    }

    /// Merge container CPU resources into this sandbox CPU resource object.
    /// Aggregates fractional vCPU allocation and extends cpuset/nodeset.
    pub fn merge(&mut self, container_resource: &LinuxContainerCpuResources) -> &mut Self {
        if let Some(v) = container_resource.calculated_vcpu {
            self.calculated_vcpu += v;
        }
        self.cpuset.extend(&container_resource.cpuset);
        self.nodeset.extend(&container_resource.nodeset);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const EPSILON: f64 = 0.0001;

    #[test]
    fn test_linux_container_cpu_resources() {
        let resources = LinuxContainerCpuResources::default();

        assert_eq!(resources.shares(), 0);
        assert!(resources.cpuset.is_empty());
        assert!(resources.nodeset.is_empty());
        assert!(resources.get_vcpus().is_none());

        let mut linux_cpu = oci::LinuxCpu::default();
        linux_cpu.set_shares(Some(2048));
        linux_cpu.set_quota(Some(1001));
        linux_cpu.set_period(Some(100));
        linux_cpu.set_cpus(Some("1,2,3".to_string()));
        linux_cpu.set_mems(Some("1".to_string()));

        let resources = LinuxContainerCpuResources::try_from(&linux_cpu).unwrap();
        assert_eq!(resources.shares(), 2048);
        assert_eq!(resources.period(), 100);
        assert_eq!(resources.quota(), 1001);

        // Expected fractional vCPUs = quota / period
        let expected_vcpus = 1001.0 / 100.0;
        assert!(
            (resources.get_vcpus().unwrap() - expected_vcpus).abs() < EPSILON,
            "got {}, expect {}",
            resources.get_vcpus().unwrap(),
            expected_vcpus
        );

        assert_eq!(resources.cpuset().len(), 3);
        assert_eq!(resources.nodeset().len(), 1);

        // Test cpuset-only path (no quota)
        let mut linux_cpu = oci::LinuxCpu::default();
        linux_cpu.set_shares(Some(2048));
        linux_cpu.set_cpus(Some("1".to_string()));
        linux_cpu.set_mems(Some("1-2".to_string()));

        let resources = LinuxContainerCpuResources::try_from(&linux_cpu).unwrap();
        assert_eq!(resources.shares(), 2048);
        assert_eq!(resources.period(), 0);
        assert_eq!(resources.quota(), -1);
        assert!(
            (resources.get_vcpus().unwrap() - 1.0).abs() < EPSILON,
            "cpuset size vCPU mismatch"
        );
        assert_eq!(resources.cpuset().len(), 1);
        assert_eq!(resources.nodeset().len(), 2);
    }

    #[test]
    fn test_linux_sandbox_cpu_resources() {
        let mut sandbox = LinuxSandboxCpuResources::new(1024);

        assert_eq!(sandbox.shares(), 1024);
        assert_eq!(sandbox.get_vcpus(), 0.0);
        assert!(sandbox.cpuset().is_empty());
        assert!(sandbox.nodeset().is_empty());

        let mut linux_cpu = oci::LinuxCpu::default();
        linux_cpu.set_shares(Some(2048));
        linux_cpu.set_quota(Some(1001));
        linux_cpu.set_period(Some(100));
        linux_cpu.set_cpus(Some("1,2,3".to_string()));
        linux_cpu.set_mems(Some("1".to_string()));

        let resources = LinuxContainerCpuResources::try_from(&linux_cpu).unwrap();
        sandbox.merge(&resources);
        assert_eq!(sandbox.shares(), 1024);

        // vCPUs after merge = quota / period
        let expected_vcpus = 1001.0 / 100.0;
        assert!(
            (sandbox.get_vcpus() - expected_vcpus).abs() < EPSILON,
            "sandbox vCPU mismatch: got {}, expect {}",
            sandbox.get_vcpus(),
            expected_vcpus
        );

        assert_eq!(sandbox.cpuset().len(), 3);
        assert_eq!(sandbox.nodeset().len(), 1);

        // Merge cpuset-only container
        let mut linux_cpu = oci::LinuxCpu::default();
        linux_cpu.set_shares(Some(2048));
        linux_cpu.set_cpus(Some("1,4".to_string()));
        linux_cpu.set_mems(Some("1-2".to_string()));

        let resources = LinuxContainerCpuResources::try_from(&linux_cpu).unwrap();
        sandbox.merge(&resources);

        assert_eq!(sandbox.shares(), 1024);

        // Expect quota-based + cpuset len (since cpuset is treated as allocation)
        let expected_after_merge = expected_vcpus + resources.get_vcpus().unwrap();
        assert!(
            (sandbox.get_vcpus() - expected_after_merge).abs() < EPSILON,
            "sandbox vCPU mismatch after cpuset merge: got {}, expect {}",
            sandbox.get_vcpus(),
            expected_after_merge
        );
        assert_eq!(sandbox.cpuset().len(), 4);
        assert_eq!(sandbox.nodeset().len(), 2);
    }
}
