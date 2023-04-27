// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::From;

use containerd_shim_protos::cgroups::metrics;
use protobuf::Message;

use super::{StatsInfo, StatsInfoValue};

// TODO: trans from agent proto?
impl From<Option<agent::StatsContainerResponse>> for StatsInfo {
    fn from(c_stats: Option<agent::StatsContainerResponse>) -> Self {
        let mut metric = metrics::Metrics::new();
        let stats = match c_stats {
            None => {
                return StatsInfo { value: None };
            }
            Some(stats) => stats,
        };

        if let Some(cg_stats) = stats.cgroup_stats {
            if let Some(cpu) = cg_stats.cpu_stats {
                // set protobuf cpu stat
                let mut p_cpu = metrics::CPUStat::new();
                if let Some(usage) = cpu.cpu_usage {
                    let mut p_usage = metrics::CPUUsage::new();
                    p_usage.set_total(usage.total_usage);
                    p_usage.set_per_cpu(usage.percpu_usage);
                    p_usage.set_kernel(usage.usage_in_kernelmode);
                    p_usage.set_user(usage.usage_in_usermode);

                    // set protobuf cpu usage
                    p_cpu.set_usage(p_usage);
                }

                if let Some(throttle) = cpu.throttling_data {
                    let mut p_throttle = metrics::Throttle::new();
                    p_throttle.set_periods(throttle.periods);
                    p_throttle.set_throttled_time(throttle.throttled_time);
                    p_throttle.set_throttled_periods(throttle.throttled_periods);

                    // set protobuf cpu usage
                    p_cpu.set_throttling(p_throttle);
                }

                metric.set_cpu(p_cpu);
            }

            if let Some(m_stats) = cg_stats.memory_stats {
                let mut p_m = metrics::MemoryStat::new();
                p_m.set_cache(m_stats.cache);
                // memory usage
                if let Some(m_data) = m_stats.usage {
                    let mut p_m_entry = metrics::MemoryEntry::new();
                    p_m_entry.set_usage(m_data.usage);
                    p_m_entry.set_limit(m_data.limit);
                    p_m_entry.set_failcnt(m_data.failcnt);
                    p_m_entry.set_max(m_data.max_usage);

                    p_m.set_usage(p_m_entry);
                }
                // memory swap_usage
                if let Some(m_data) = m_stats.swap_usage {
                    let mut p_m_entry = metrics::MemoryEntry::new();
                    p_m_entry.set_usage(m_data.usage);
                    p_m_entry.set_limit(m_data.limit);
                    p_m_entry.set_failcnt(m_data.failcnt);
                    p_m_entry.set_max(m_data.max_usage);

                    p_m.set_swap(p_m_entry);
                }
                // memory kernel_usage
                if let Some(m_data) = m_stats.kernel_usage {
                    let mut p_m_entry = metrics::MemoryEntry::new();
                    p_m_entry.set_usage(m_data.usage);
                    p_m_entry.set_limit(m_data.limit);
                    p_m_entry.set_failcnt(m_data.failcnt);
                    p_m_entry.set_max(m_data.max_usage);

                    p_m.set_kernel(p_m_entry);
                }

                for (k, v) in m_stats.stats {
                    match k.as_str() {
                        "dirty" => p_m.set_dirty(v),
                        "rss" => p_m.set_rss(v),
                        "rss_huge" => p_m.set_rss_huge(v),
                        "mapped_file" => p_m.set_mapped_file(v),
                        "writeback" => p_m.set_writeback(v),
                        "pg_pg_in" => p_m.set_pg_pg_in(v),
                        "pg_pg_out" => p_m.set_pg_pg_out(v),
                        "pg_fault" => p_m.set_pg_fault(v),
                        "pg_maj_fault" => p_m.set_pg_maj_fault(v),
                        "inactive_file" => p_m.set_inactive_file(v),
                        "inactive_anon" => p_m.set_inactive_anon(v),
                        "active_file" => p_m.set_active_file(v),
                        "unevictable" => p_m.set_unevictable(v),
                        "hierarchical_memory_limit" => p_m.set_hierarchical_memory_limit(v),
                        "hierarchical_swap_limit" => p_m.set_hierarchical_swap_limit(v),
                        "total_cache" => p_m.set_total_cache(v),
                        "total_rss" => p_m.set_total_rss(v),
                        "total_mapped_file" => p_m.set_total_mapped_file(v),
                        "total_dirty" => p_m.set_total_dirty(v),

                        "total_pg_pg_in" => p_m.set_total_pg_pg_in(v),
                        "total_pg_pg_out" => p_m.set_total_pg_pg_out(v),
                        "total_pg_fault" => p_m.set_total_pg_fault(v),
                        "total_pg_maj_fault" => p_m.set_total_pg_maj_fault(v),
                        "total_inactive_file" => p_m.set_total_inactive_file(v),
                        "total_inactive_anon" => p_m.set_total_inactive_anon(v),
                        "total_active_file" => p_m.set_total_active_file(v),
                        "total_unevictable" => p_m.set_total_unevictable(v),
                        _ => (),
                    }
                }
                metric.set_memory(p_m);
            }

            if let Some(pid_stats) = cg_stats.pids_stats {
                let mut p_pid = metrics::PidsStat::new();
                p_pid.set_limit(pid_stats.limit);
                p_pid.set_current(pid_stats.current);
                metric.set_pids(p_pid);
            }

            if let Some(blk_stats) = cg_stats.blkio_stats {
                let mut p_blk_stats = metrics::BlkIOStat::new();
                p_blk_stats
                    .set_io_serviced_recursive(copy_blkio_entry(&blk_stats.io_serviced_recursive));
                p_blk_stats.set_io_service_bytes_recursive(copy_blkio_entry(
                    &blk_stats.io_service_bytes_recursive,
                ));
                p_blk_stats
                    .set_io_queued_recursive(copy_blkio_entry(&blk_stats.io_queued_recursive));
                p_blk_stats.set_io_service_time_recursive(copy_blkio_entry(
                    &blk_stats.io_service_time_recursive,
                ));
                p_blk_stats.set_io_wait_time_recursive(copy_blkio_entry(
                    &blk_stats.io_wait_time_recursive,
                ));
                p_blk_stats
                    .set_io_merged_recursive(copy_blkio_entry(&blk_stats.io_merged_recursive));
                p_blk_stats.set_io_time_recursive(copy_blkio_entry(&blk_stats.io_time_recursive));
                p_blk_stats.set_sectors_recursive(copy_blkio_entry(&blk_stats.sectors_recursive));

                metric.set_blkio(p_blk_stats);
            }

            if !cg_stats.hugetlb_stats.is_empty() {
                let mut p_huge = Vec::new();
                for (k, v) in cg_stats.hugetlb_stats {
                    let mut h = metrics::HugetlbStat::new();
                    h.set_pagesize(k);
                    h.set_max(v.max_usage);
                    h.set_usage(v.usage);
                    h.set_failcnt(v.failcnt);
                    p_huge.push(h);
                }
                metric.set_hugetlb(p_huge);
            }
        }

        let net_stats = stats.network_stats;
        if !net_stats.is_empty() {
            let mut p_net = Vec::new();
            for v in net_stats.iter() {
                let mut h = metrics::NetworkStat::new();
                h.set_name(v.name.clone());

                h.set_tx_bytes(v.tx_bytes);
                h.set_tx_packets(v.tx_packets);
                h.set_tx_errors(v.tx_errors);
                h.set_tx_dropped(v.tx_dropped);

                h.set_rx_bytes(v.rx_bytes);
                h.set_rx_packets(v.rx_packets);
                h.set_rx_errors(v.rx_errors);
                h.set_rx_dropped(v.rx_dropped);

                p_net.push(h);
            }
            metric.set_network(p_net);
        }

        StatsInfo {
            value: Some(StatsInfoValue {
                type_url: "io.containerd.cgroups.v1.Metrics".to_string(),
                value: metric.write_to_bytes().unwrap(),
            }),
        }
    }
}

fn copy_blkio_entry(entry: &[agent::BlkioStatsEntry]) -> Vec<metrics::BlkIOEntry> {
    let mut p_entry = Vec::new();

    for e in entry.iter() {
        let mut blk = metrics::BlkIOEntry::new();
        blk.set_op(e.op.clone());
        blk.set_value(e.value);
        blk.set_major(e.major);
        blk.set_minor(e.minor);

        p_entry.push(blk);
    }

    p_entry
}
