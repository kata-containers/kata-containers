use super::super::common::{CgroupHierarchy, Properties};
use super::transformer::Transformer;

use anyhow::Result;
use oci::{LinuxCpu, LinuxResources};
use zbus::zvariant::Value;

const BASIC_SYSTEMD_VERSION: &str = "242";
const DEFAULT_CPUQUOTAPERIOD: u64 = 100 * 1000;
const SEC2MICROSEC: u64 = 1000 * 1000;
const BASIC_INTERVAL: u64 = 10 * 1000;

pub struct Cpu {}

impl Transformer for Cpu {
    fn apply(
        r: &LinuxResources,
        properties: &mut Properties,
        cgroup_hierarchy: &CgroupHierarchy,
        systemd_version: &str,
    ) -> Result<()> {
        if let Some(cpu_resources) = &r.cpu {
            match cgroup_hierarchy {
                CgroupHierarchy::Legacy => Self::legacy_apply(cpu_resources, properties)?,
                CgroupHierarchy::Unified => {
                    Self::unified_apply(cpu_resources, properties, systemd_version)?
                }
            }
        }

        Ok(())
    }
}

impl Cpu {
    // v1:
    // cpu.shares <-> CPUShares
    fn legacy_apply(cpu_resources: &LinuxCpu, properties: &mut Properties) -> Result<()> {
        if let Some(shares) = cpu_resources.shares {
            properties.push(("CPUShares", Value::U64(shares)));
        }

        Ok(())
    }

    // v2:
    // cpu.shares <-> CPUShares
    // cpu.period <-> CPUQuotaPeriodUSec
    // cpu.period & cpu.quota <-> CPUQuotaPerSecUSec
    fn unified_apply(
        cpu_resources: &LinuxCpu,
        properties: &mut Properties,
        systemd_version: &str,
    ) -> Result<()> {
        if let Some(shares) = cpu_resources.shares {
            let unified_shares = get_unified_cpushares(shares);
            properties.push(("CPUShares", Value::U64(unified_shares)));
        }
        let quota = cpu_resources.quota.unwrap_or(i64::MAX);
        let period = cpu_resources.period.unwrap_or(u64::MAX);
        if period != 0 && systemd_version >= BASIC_SYSTEMD_VERSION {
            properties.push(("CPUQuotaPeriodUSec", Value::U64(period)));
        }
        if quota != 0 || period != 0 {
            let cpu_quota_per_sec_usec = get_unified_cpuquota(quota, period);
            properties.push(("CPUQuotaPerSecUSec", Value::U64(cpu_quota_per_sec_usec)));
        }
        Ok(())
    }
}

// [2-262144] to [1-10000]
fn get_unified_cpushares(shares: u64) -> u64 {
    if shares == 0 {
        return 100;
    }

    1 + ((shares - 2) * 9999) / 262142
}

fn get_unified_cpuquota(quota: i64, period: u64) -> u64 {
    let mut cpu_quota_per_sec_usec = u64::MAX;
    if quota > 0 {
        let adjusted_period = if period != 0 {
            period
        } else {
            DEFAULT_CPUQUOTAPERIOD
        };
        cpu_quota_per_sec_usec = (quota as u64) * SEC2MICROSEC / adjusted_period;
        if cpu_quota_per_sec_usec % BASIC_INTERVAL != 0 {
            cpu_quota_per_sec_usec =
                ((cpu_quota_per_sec_usec / BASIC_INTERVAL) + 1) * BASIC_INTERVAL;
        }
    }
    cpu_quota_per_sec_usec
}

#[cfg(test)]
mod tests {
    use crate::cgroups::systemd::subsystem::cpu::get_unified_cpuquota;

    #[test]
    fn test_unified_cpuquota() {
        let quota: i64 = 1000000;
        let period: u64 = 500000;
        let cpu_quota_per_sec_usec = get_unified_cpuquota(quota, period);

        assert_eq!(2000000, cpu_quota_per_sec_usec);
    }
}
