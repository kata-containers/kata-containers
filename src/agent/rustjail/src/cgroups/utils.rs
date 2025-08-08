// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs;

use anyhow::Result;
use cgroups::stats::{BlkioStat, MemoryCgroupStats};
use cgroups::CgroupStats;
use protobuf::MessageField;
use protocols::agent::{
    BlkioStats, BlkioStatsEntry, CgroupStats as AgentCgroupStats, CpuStats, CpuUsage, HugetlbStats,
    MemoryData, MemoryStats, PidsStats, ThrottlingData,
};

const GUEST_CPUS_PATH: &str = "/sys/devices/system/cpu/online";

/// Read the guest online CPUs from the system file.
pub fn read_guest_online_cpus() -> Result<String> {
    let c = fs::read_to_string(GUEST_CPUS_PATH)?;
    Ok(c.trim().to_string())
}

fn convert_memory_others_to_hashmap(memory: &MemoryCgroupStats) -> HashMap<String, u64> {
    let mut others = HashMap::new();
    others.insert("cache".to_string(), memory.cache);
    others.insert("rss".to_string(), memory.rss);
    others.insert("rss_huge".to_string(), memory.rss_huge);
    others.insert("shmem".to_string(), memory.shmem);
    others.insert("mapped_file".to_string(), memory.mapped_file);
    others.insert("dirty".to_string(), memory.dirty);
    others.insert("writeback".to_string(), memory.writeback);
    others.insert("swap".to_string(), memory.swap);
    others.insert("pgpgin".to_string(), memory.pgpgin);
    others.insert("pgpgout".to_string(), memory.pgpgout);
    others.insert("pgfault".to_string(), memory.pgfault);
    others.insert("pgmajfault".to_string(), memory.pgmajfault);
    others.insert("inactive_anon".to_string(), memory.inactive_anon);
    others.insert("active_anon".to_string(), memory.active_anon);
    others.insert("inactive_file".to_string(), memory.inactive_file);
    others.insert("active_file".to_string(), memory.active_file);
    others.insert("unevictable".to_string(), memory.unevictable);
    others.insert(
        "hierarchical_memory_limit".to_string(),
        memory.hierarchical_memory_limit as u64,
    );
    others.insert(
        "hierarchical_memsw_limit".to_string(),
        memory.hierarchical_memsw_limit as u64,
    );
    others.insert("total_cache".to_string(), memory.total_cache);
    others.insert("total_rss".to_string(), memory.total_rss);
    others.insert("total_rss_huge".to_string(), memory.total_rss_huge);
    others.insert("total_shmem".to_string(), memory.total_shmem);
    others.insert("total_mapped_file".to_string(), memory.total_mapped_file);
    others.insert("total_dirty".to_string(), memory.total_dirty);
    others.insert("total_writeback".to_string(), memory.total_writeback);
    others.insert("total_swap".to_string(), memory.total_swap);
    others.insert("total_pgpgin".to_string(), memory.total_pgpgin);
    others.insert("total_pgpgout".to_string(), memory.total_pgpgout);
    others.insert("total_pgfault".to_string(), memory.total_pgfault);
    others.insert("total_pgmajfault".to_string(), memory.total_pgmajfault);
    others.insert(
        "total_inactive_anon".to_string(),
        memory.total_inactive_anon,
    );
    others.insert("total_active_anon".to_string(), memory.total_active_anon);
    others.insert(
        "total_inactive_file".to_string(),
        memory.total_inactive_file,
    );
    others.insert("total_active_file".to_string(), memory.total_active_file);
    others.insert("total_unevictable".to_string(), memory.total_unevictable);
    others
}

fn convert_blkio_stats(stats: &[BlkioStat]) -> Vec<BlkioStatsEntry> {
    stats
        .iter()
        .map(|stat| BlkioStatsEntry {
            major: stat.major,
            minor: stat.minor,
            op: stat.op.clone(),
            value: stat.value,
            ..Default::default()
        })
        .collect()
}

/// Convert cgroups-rs `CgroupStats` to agent ttrpc `CgroupStats`.
pub fn convert_cgroup_stats(stats: CgroupStats) -> Result<AgentCgroupStats> {
    // CPU
    let cpu_usage = stats.cpu.cpu_acct.map(|acct| CpuUsage {
        total_usage: acct.total_usage,
        usage_in_usermode: acct.user_usage,
        usage_in_kernelmode: acct.system_usage,
        percpu_usage: acct.usage_percpu.clone(),
        ..Default::default()
    });

    let cpu_throttling = stats.cpu.cpu_throttling.map(|throttling| ThrottlingData {
        periods: throttling.periods,
        throttled_periods: throttling.throttled_periods,
        throttled_time: throttling.throttled_time,
        ..Default::default()
    });

    let cpu_stats = CpuStats {
        cpu_usage: MessageField::from_option(cpu_usage),
        throttling_data: MessageField::from_option(cpu_throttling),
        ..Default::default()
    };

    // Memory
    let memory_usage = stats.memory.memory.as_ref().map(|mem| MemoryData {
        usage: mem.usage,
        max_usage: mem.max_usage,
        limit: mem.limit as u64,
        failcnt: mem.fail_cnt,
        ..Default::default()
    });

    let swap_usage = stats.memory.memory_swap.as_ref().map(|swap| MemoryData {
        usage: swap.usage,
        max_usage: swap.max_usage,
        limit: swap.limit as u64,
        failcnt: swap.fail_cnt,
        ..Default::default()
    });

    let kernel_usage = stats
        .memory
        .kernel_memory
        .as_ref()
        .map(|kernel| MemoryData {
            usage: kernel.usage,
            max_usage: kernel.max_usage,
            limit: kernel.limit as u64,
            failcnt: kernel.fail_cnt,
            ..Default::default()
        });

    let memory_stats = MemoryStats {
        cache: stats.memory.cache,
        usage: MessageField::from_option(memory_usage),
        swap_usage: MessageField::from_option(swap_usage),
        kernel_usage: MessageField::from_option(kernel_usage),
        use_hierarchy: stats.memory.use_hierarchy,
        stats: convert_memory_others_to_hashmap(&stats.memory),
        ..Default::default()
    };

    // Pids
    let pids_stats = PidsStats {
        current: stats.pids.current,
        limit: stats.pids.limit as u64,
        ..Default::default()
    };

    // Blkio
    let blkio_stats = BlkioStats {
        io_service_bytes_recursive: convert_blkio_stats(&stats.blkio.io_service_bytes_recursive),
        io_serviced_recursive: convert_blkio_stats(&stats.blkio.io_serviced_recursive),
        io_queued_recursive: convert_blkio_stats(&stats.blkio.io_queued_recursive),
        io_service_time_recursive: convert_blkio_stats(&stats.blkio.io_service_time_recursive),
        io_wait_time_recursive: convert_blkio_stats(&stats.blkio.io_wait_time_recursive),
        io_merged_recursive: convert_blkio_stats(&stats.blkio.io_merged_recursive),
        io_time_recursive: convert_blkio_stats(&stats.blkio.io_time_recursive),
        sectors_recursive: convert_blkio_stats(&stats.blkio.sectors_recursive),
        ..Default::default()
    };

    // HugeTLB
    let hugetlb_stats: HashMap<_, _> = stats
        .hugetlb
        .iter()
        .map(|(page_size, stat)| {
            (
                page_size.to_string(),
                HugetlbStats {
                    usage: stat.usage,
                    max_usage: stat.max_usage,
                    failcnt: stat.fail_cnt,
                    ..Default::default()
                },
            )
        })
        .collect();

    Ok(AgentCgroupStats {
        cpu_stats: MessageField::some(cpu_stats),
        memory_stats: MessageField::some(memory_stats),
        pids_stats: MessageField::some(pids_stats),
        blkio_stats: MessageField::some(blkio_stats),
        hugetlb_stats,
        ..Default::default()
    })
}
