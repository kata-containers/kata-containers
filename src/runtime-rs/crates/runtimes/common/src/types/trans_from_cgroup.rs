// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::TryFrom;

use cgroups::stats::{
    BlkioCgroupStats, BlkioStat, CpuCgroupStats, HugeTlbStat, MemoryCgroupStats, MemoryStats,
    PidsCgroupStats,
};
use cgroups::CgroupStats;
use containerd_shim_protos::cgroups::metrics as metrics_v1;
use protobuf::Message;

use super::{StatsInfo, StatsInfoValue};

const CGROUP_V1_METRICS_TYPE_URL: &str = "io.containerd.cgroups.v1.Metrics";
impl From<CgroupStats> for StatsInfo {
    fn from(stats: CgroupStats) -> Self {
        let mut metrics = metrics_v1::Metrics::new();
        metrics.set_cpu(cpu_stats(stats.cpu));
        metrics.set_memory(memory_stats(stats.memory));
        metrics.set_pids(pids_stats(stats.pids));
        metrics.set_blkio(blkio_stats(stats.blkio));
        metrics.set_hugetlb(
            stats
                .hugetlb
                .into_iter()
                .map(|(page_size, stat)| hugetlb_stats(page_size, stat))
                .collect(),
        );

        StatsInfo {
            value: Some(StatsInfoValue {
                type_url: CGROUP_V1_METRICS_TYPE_URL.to_string(),
                value: metrics
                    .write_to_bytes()
                    .expect("serializing cgroup metrics cannot fail"),
            }),
        }
    }
}
fn cpu_stats(stats: CpuCgroupStats) -> metrics_v1::CPUStat {
    let mut result = metrics_v1::CPUStat::new();
    if let Some(usage) = stats.cpu_acct {
        let mut value = metrics_v1::CPUUsage::new();
        value.set_total(usage.total_usage);
        value.set_per_cpu(usage.usage_percpu);
        value.set_kernel(usage.system_usage);
        value.set_user(usage.user_usage);
        result.set_usage(value);
    }
    if let Some(throttling) = stats.cpu_throttling {
        let mut value = metrics_v1::Throttle::new();
        value.set_periods(throttling.periods);
        value.set_throttled_periods(throttling.throttled_periods);
        value.set_throttled_time(throttling.throttled_time);
        result.set_throttling(value);
    }
    result
}

fn memory_stats(stats: MemoryCgroupStats) -> metrics_v1::MemoryStat {
    let mut result = metrics_v1::MemoryStat::new();
    result.set_cache(stats.cache);
    result.set_rss(stats.rss);
    result.set_rss_huge(stats.rss_huge);
    result.set_mapped_file(stats.mapped_file);
    result.set_dirty(stats.dirty);
    result.set_writeback(stats.writeback);
    result.set_pg_pg_in(stats.pgpgin);
    result.set_pg_pg_out(stats.pgpgout);
    result.set_pg_fault(stats.pgfault);
    result.set_pg_maj_fault(stats.pgmajfault);
    result.set_inactive_anon(stats.inactive_anon);
    result.set_active_anon(stats.active_anon);
    result.set_inactive_file(stats.inactive_file);
    result.set_active_file(stats.active_file);
    result.set_unevictable(stats.unevictable);
    result.set_hierarchical_memory_limit(limit_to_u64(stats.hierarchical_memory_limit));
    result.set_hierarchical_swap_limit(limit_to_u64(stats.hierarchical_memsw_limit));
    result.set_total_cache(stats.total_cache);
    result.set_total_rss(stats.total_rss);
    result.set_total_rss_huge(stats.total_rss_huge);
    result.set_total_mapped_file(stats.total_mapped_file);
    result.set_total_dirty(stats.total_dirty);
    result.set_total_writeback(stats.total_writeback);
    result.set_total_pg_pg_in(stats.total_pgpgin);
    result.set_total_pg_pg_out(stats.total_pgpgout);
    result.set_total_pg_fault(stats.total_pgfault);
    result.set_total_pg_maj_fault(stats.total_pgmajfault);
    result.set_total_inactive_anon(stats.total_inactive_anon);
    result.set_total_active_anon(stats.total_active_anon);
    result.set_total_inactive_file(stats.total_inactive_file);
    result.set_total_active_file(stats.total_active_file);
    result.set_total_unevictable(stats.total_unevictable);

    if let Some(value) = stats.memory {
        result.set_usage(memory_entry(value));
    }
    if let Some(value) = stats.memory_swap {
        result.set_swap(memory_entry(value));
    }
    if let Some(value) = stats.kernel_memory {
        result.set_kernel(memory_entry(value));
    }
    result
}

fn memory_entry(stats: MemoryStats) -> metrics_v1::MemoryEntry {
    let mut result = metrics_v1::MemoryEntry::new();
    result.set_usage(stats.usage);
    result.set_max(stats.max_usage);
    result.set_limit(limit_to_u64(stats.limit));
    result.set_failcnt(stats.fail_cnt);
    result
}

fn pids_stats(stats: PidsCgroupStats) -> metrics_v1::PidsStat {
    let mut result = metrics_v1::PidsStat::new();
    result.set_current(stats.current);
    result.set_limit(limit_to_u64(stats.limit));
    result
}

fn blkio_stats(stats: BlkioCgroupStats) -> metrics_v1::BlkIOStat {
    let mut result = metrics_v1::BlkIOStat::new();
    result.set_io_service_bytes_recursive(blkio_entries(stats.io_service_bytes_recursive));
    result.set_io_serviced_recursive(blkio_entries(stats.io_serviced_recursive));
    result.set_io_queued_recursive(blkio_entries(stats.io_queued_recursive));
    result.set_io_service_time_recursive(blkio_entries(stats.io_service_time_recursive));
    result.set_io_wait_time_recursive(blkio_entries(stats.io_wait_time_recursive));
    result.set_io_merged_recursive(blkio_entries(stats.io_merged_recursive));
    result.set_io_time_recursive(blkio_entries(stats.io_time_recursive));
    result.set_sectors_recursive(blkio_entries(stats.sectors_recursive));
    result
}

fn blkio_entries(entries: Vec<BlkioStat>) -> Vec<metrics_v1::BlkIOEntry> {
    entries
        .into_iter()
        .map(|entry| {
            let mut result = metrics_v1::BlkIOEntry::new();
            result.set_major(entry.major);
            result.set_minor(entry.minor);
            result.set_op(entry.op);
            result.set_value(entry.value);
            result
        })
        .collect()
}

fn hugetlb_stats(page_size: String, stats: HugeTlbStat) -> metrics_v1::HugetlbStat {
    let mut result = metrics_v1::HugetlbStat::new();
    result.set_pagesize(page_size);
    result.set_usage(stats.usage);
    result.set_max(stats.max_usage);
    result.set_failcnt(stats.fail_cnt);
    result
}

fn limit_to_u64(limit: i64) -> u64 {
    u64::try_from(limit).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use cgroups::stats::{CpuAcctStats, CpuCgroupStats, MemoryCgroupStats, MemoryStats};
    use cgroups::CgroupStats;
    use containerd_shim_protos::cgroups::metrics as metrics_v1;
    use protobuf::Message;

    use super::{StatsInfo, CGROUP_V1_METRICS_TYPE_URL};

    #[test]
    fn converts_sandbox_cgroup_stats() {
        let stats = CgroupStats {
            cpu: CpuCgroupStats {
                cpu_acct: Some(CpuAcctStats {
                    total_usage: 42,
                    ..Default::default()
                }),
                ..Default::default()
            },
            memory: MemoryCgroupStats {
                memory: Some(MemoryStats {
                    usage: 1024,
                    limit: -1,
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let value = StatsInfo::from(stats).value.expect("metrics payload");
        assert_eq!(value.type_url, CGROUP_V1_METRICS_TYPE_URL);
        let decoded = metrics_v1::Metrics::parse_from_bytes(&value.value).expect("decode metrics");
        assert_eq!(decoded.cpu().usage().total(), 42);
        assert_eq!(decoded.memory().usage().usage(), 1024);
        assert_eq!(decoded.memory().usage().limit(), u64::MAX);
    }
}
