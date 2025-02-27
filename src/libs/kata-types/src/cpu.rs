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
#[derive(Clone, Default, Debug)]
pub struct LinuxContainerCpuResources {
    shares: u64,
    period: u64,
    quota: i64,
    cpuset: CpuSet,
    nodeset: NumaNodeSet,
    calculated_vcpu_time_ms: Option<u64>,
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

    /// Get number of vCPUs to fulfill the CPU resource request, `None` means unconstrained.
    pub fn get_vcpus(&self) -> Option<u64> {
        self.calculated_vcpu_time_ms
            .map(|v| v.saturating_add(999) / 1000)
    }
}

impl TryFrom<&oci::LinuxCpu> for LinuxContainerCpuResources {
    type Error = Error;

    // Unhandled fields: realtime_runtime, realtime_period, mems
    fn try_from(value: &oci::LinuxCpu) -> Result<Self, Self::Error> {
        let period = value.period().unwrap_or(0);
        let quota = value.quota().unwrap_or(-1);
        let value_cpus = value.cpus().as_ref().map_or("", |cpus| cpus);
        let cpuset = CpuSet::from_str(value_cpus).map_err(Error::InvalidCpuSet)?;
        let value_mems = value.mems().as_ref().map_or("", |mems| mems);
        let nodeset = NumaNodeSet::from_str(value_mems).map_err(Error::InvalidNodeSet)?;

        // If quota is -1, it means the CPU resource request is unconstrained. In that case,
        // we don't currently assign additional CPUs.
        let milli_sec = if quota >= 0 && period != 0 {
            Some((quota as u64).saturating_mul(1000) / period)
        } else {
            None
        };

        Ok(LinuxContainerCpuResources {
            shares: value.shares().unwrap_or(0),
            period,
            quota,
            cpuset,
            nodeset,
            calculated_vcpu_time_ms: milli_sec,
        })
    }
}

/// Assigned CPU resources for a Linux sandbox/pod.
#[derive(Default, Debug)]
pub struct LinuxSandboxCpuResources {
    shares: u64,
    calculated_vcpu_time_ms: u64,
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

    /// Get assigned vCPU time in ms.
    pub fn calculated_vcpu_time_ms(&self) -> u64 {
        self.calculated_vcpu_time_ms
    }

    /// Get the CPU set.
    pub fn cpuset(&self) -> &CpuSet {
        &self.cpuset
    }

    /// Get the NUMA memory node set.
    pub fn nodeset(&self) -> &NumaNodeSet {
        &self.nodeset
    }

    /// Get number of vCPUs to fulfill the CPU resource request.
    pub fn get_vcpus(&self) -> u64 {
        if self.calculated_vcpu_time_ms == 0 && !self.cpuset.is_empty() {
            self.cpuset.len() as u64
        } else {
            self.calculated_vcpu_time_ms.saturating_add(999) / 1000
        }
    }

    /// Merge resources assigned to a container into the sandbox/pod resources.
    pub fn merge(&mut self, container_resource: &LinuxContainerCpuResources) -> &mut Self {
        if let Some(v) = container_resource.calculated_vcpu_time_ms.as_ref() {
            self.calculated_vcpu_time_ms += v;
        }
        self.cpuset.extend(&container_resource.cpuset);
        self.nodeset.extend(&container_resource.nodeset);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_container_cpu_resources() {
        let resources = LinuxContainerCpuResources::default();

        assert_eq!(resources.shares(), 0);
        assert_eq!(resources.calculated_vcpu_time_ms, None);
        assert!(resources.cpuset.is_empty());
        assert!(resources.nodeset.is_empty());
        assert!(resources.calculated_vcpu_time_ms.is_none());

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
        assert_eq!(resources.calculated_vcpu_time_ms, Some(10010));
        assert_eq!(resources.get_vcpus().unwrap(), 11);
        assert_eq!(resources.cpuset().len(), 3);
        assert_eq!(resources.nodeset().len(), 1);

        let mut linux_cpu = oci::LinuxCpu::default();
        linux_cpu.set_shares(Some(2048));
        linux_cpu.set_cpus(Some("1".to_string()));
        linux_cpu.set_mems(Some("1-2".to_string()));

        let resources = LinuxContainerCpuResources::try_from(&linux_cpu).unwrap();
        assert_eq!(resources.shares(), 2048);
        assert_eq!(resources.period(), 0);
        assert_eq!(resources.quota(), -1);
        assert_eq!(resources.calculated_vcpu_time_ms, None);
        assert!(resources.get_vcpus().is_none());
        assert_eq!(resources.cpuset().len(), 1);
        assert_eq!(resources.nodeset().len(), 2);
    }

    #[test]
    fn test_linux_sandbox_cpu_resources() {
        let mut sandbox = LinuxSandboxCpuResources::new(1024);

        assert_eq!(sandbox.shares(), 1024);
        assert_eq!(sandbox.get_vcpus(), 0);
        assert_eq!(sandbox.calculated_vcpu_time_ms(), 0);
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
        assert_eq!(sandbox.get_vcpus(), 11);
        assert_eq!(sandbox.calculated_vcpu_time_ms(), 10010);
        assert_eq!(sandbox.cpuset().len(), 3);
        assert_eq!(sandbox.nodeset().len(), 1);

        let mut linux_cpu = oci::LinuxCpu::default();
        linux_cpu.set_shares(Some(2048));
        linux_cpu.set_cpus(Some("1,4".to_string()));
        linux_cpu.set_mems(Some("1-2".to_string()));

        let resources = LinuxContainerCpuResources::try_from(&linux_cpu).unwrap();
        sandbox.merge(&resources);

        assert_eq!(sandbox.shares(), 1024);
        assert_eq!(sandbox.get_vcpus(), 11);
        assert_eq!(sandbox.calculated_vcpu_time_ms(), 10010);
        assert_eq!(sandbox.cpuset().len(), 4);
        assert_eq!(sandbox.nodeset().len(), 2);
    }
}
