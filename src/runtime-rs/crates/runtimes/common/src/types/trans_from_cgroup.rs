// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    collections::{BTreeSet, HashMap},
    convert::TryFrom,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use cgroups::stats::{
    BlkioCgroupStats, BlkioStat, CpuCgroupStats, HugeTlbStat, MemoryCgroupStats, MemoryStats,
    PidsCgroupStats,
};
use cgroups::CgroupStats;
use containerd_shim_protos::{cgroups::metrics as metrics_v1, cgroups_v2::metrics as metrics_v2};
use protobuf::Message;
use resource::cgroups::SandboxCgroupStats;

use super::{StatsInfo, StatsInfoValue};

const CGROUP_V1_METRICS_TYPE_URL: &str = "io.containerd.cgroups.v1.Metrics";
const CGROUP_V2_METRICS_TYPE_URL: &str = "io.containerd.cgroups.v2.Metrics";

impl TryFrom<SandboxCgroupStats> for StatsInfo {
    type Error = anyhow::Error;

    fn try_from(stats: SandboxCgroupStats) -> Result<Self> {
        match stats {
            SandboxCgroupStats::V1(stats) => Ok((*stats).into()),
            SandboxCgroupStats::V2(path) => v2_stats(&path),
        }
    }
}

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

fn v2_stats(path: &Path) -> Result<StatsInfo> {
    if !path.is_dir() {
        return Err(anyhow!(
            "cgroup v2 path {} is not a directory",
            path.display()
        ));
    }

    let mut metrics = metrics_v2::Metrics::new();
    metrics.set_cpu(v2_cpu_stats(path)?);
    metrics.set_memory(v2_memory_stats(path)?);
    metrics.set_pids(v2_pids_stats(path)?);
    metrics.set_io(v2_io_stats(path)?);
    metrics.set_rdma(v2_rdma_stats(path)?);
    metrics.set_hugetlb(v2_hugetlb_stats(path)?);
    metrics.set_memory_events(v2_memory_events(path)?);

    Ok(StatsInfo {
        value: Some(StatsInfoValue {
            type_url: CGROUP_V2_METRICS_TYPE_URL.to_string(),
            value: metrics
                .write_to_bytes()
                .context("serialize cgroup v2 metrics")?,
        }),
    })
}

fn v2_cpu_stats(path: &Path) -> Result<metrics_v2::CPUStat> {
    let values = read_keyed_file(&path.join("cpu.stat"))?.unwrap_or_default();
    let mut result = metrics_v2::CPUStat::new();
    result.set_usage_usec(value(&values, "usage_usec"));
    result.set_user_usec(value(&values, "user_usec"));
    result.set_system_usec(value(&values, "system_usec"));
    result.set_nr_periods(value(&values, "nr_periods"));
    result.set_nr_throttled(value(&values, "nr_throttled"));
    result.set_throttled_usec(value(&values, "throttled_usec"));
    result.set_nr_bursts(value(&values, "nr_bursts"));
    result.set_burst_usec(value(&values, "burst_usec"));
    if let Some(psi) = read_psi_file(&path.join("cpu.pressure"))? {
        result.set_psi(psi);
    }
    Ok(result)
}

fn v2_memory_stats(path: &Path) -> Result<metrics_v2::MemoryStat> {
    let values = read_keyed_file(&path.join("memory.stat"))?.unwrap_or_default();
    let mut result = metrics_v2::MemoryStat::new();

    macro_rules! set_memory_values {
        ($($setter:ident => $key:literal),+ $(,)?) => {
            $(result.$setter(value(&values, $key));)+
        };
    }
    set_memory_values! {
        set_anon => "anon",
        set_file => "file",
        set_kernel_stack => "kernel_stack",
        set_slab => "slab",
        set_sock => "sock",
        set_shmem => "shmem",
        set_file_mapped => "file_mapped",
        set_file_dirty => "file_dirty",
        set_file_writeback => "file_writeback",
        set_anon_thp => "anon_thp",
        set_inactive_anon => "inactive_anon",
        set_active_anon => "active_anon",
        set_inactive_file => "inactive_file",
        set_active_file => "active_file",
        set_unevictable => "unevictable",
        set_slab_reclaimable => "slab_reclaimable",
        set_slab_unreclaimable => "slab_unreclaimable",
        set_pgfault => "pgfault",
        set_pgmajfault => "pgmajfault",
        set_workingset_refault => "workingset_refault",
        set_workingset_activate => "workingset_activate",
        set_workingset_nodereclaim => "workingset_nodereclaim",
        set_pgrefill => "pgrefill",
        set_pgscan => "pgscan",
        set_pgsteal => "pgsteal",
        set_pgactivate => "pgactivate",
        set_pgdeactivate => "pgdeactivate",
        set_pglazyfree => "pglazyfree",
        set_pglazyfreed => "pglazyfreed",
        set_thp_fault_alloc => "thp_fault_alloc",
        set_thp_collapse_alloc => "thp_collapse_alloc",
    }

    result.set_usage(read_single_value(&path.join("memory.current"))?.unwrap_or_default());
    result.set_usage_limit(read_single_value(&path.join("memory.max"))?.unwrap_or_default());
    result.set_max_usage(read_single_value(&path.join("memory.peak"))?.unwrap_or_default());
    result
        .set_swap_usage(read_single_value(&path.join("memory.swap.current"))?.unwrap_or_default());
    result.set_swap_limit(read_single_value(&path.join("memory.swap.max"))?.unwrap_or_default());
    result
        .set_swap_max_usage(read_single_value(&path.join("memory.swap.peak"))?.unwrap_or_default());
    if let Some(psi) = read_psi_file(&path.join("memory.pressure"))? {
        result.set_psi(psi);
    }
    Ok(result)
}

fn v2_pids_stats(path: &Path) -> Result<metrics_v2::PidsStat> {
    let mut result = metrics_v2::PidsStat::new();
    result.set_current(read_single_value(&path.join("pids.current"))?.unwrap_or_default());
    result.set_limit(read_single_value(&path.join("pids.max"))?.unwrap_or_default());
    Ok(result)
}

fn v2_io_stats(path: &Path) -> Result<metrics_v2::IOStat> {
    let mut result = metrics_v2::IOStat::new();
    if let Some(contents) = read_optional_file(&path.join("io.stat"))? {
        let mut usage = Vec::new();
        for (line_number, line) in contents.lines().enumerate() {
            let mut fields = line.split_whitespace();
            let device = fields.next().ok_or_else(|| {
                anyhow!(
                    "{}:{}: missing device",
                    path.join("io.stat").display(),
                    line_number + 1
                )
            })?;
            let (major, minor) = device.split_once(':').ok_or_else(|| {
                anyhow!(
                    "{}:{}: invalid device {device}",
                    path.join("io.stat").display(),
                    line_number + 1
                )
            })?;
            let values = parse_assignments(fields, &path.join("io.stat"), line_number + 1)?;
            let mut entry = metrics_v2::IOEntry::new();
            entry.set_major(parse_u64(major, &path.join("io.stat"))?);
            entry.set_minor(parse_u64(minor, &path.join("io.stat"))?);
            entry.set_rbytes(value(&values, "rbytes"));
            entry.set_wbytes(value(&values, "wbytes"));
            entry.set_rios(value(&values, "rios"));
            entry.set_wios(value(&values, "wios"));
            usage.push(entry);
        }
        result.set_usage(usage);
    }
    if let Some(psi) = read_psi_file(&path.join("io.pressure"))? {
        result.set_psi(psi);
    }
    Ok(result)
}

fn v2_memory_events(path: &Path) -> Result<metrics_v2::MemoryEvents> {
    let values = read_keyed_file(&path.join("memory.events"))?.unwrap_or_default();
    let mut result = metrics_v2::MemoryEvents::new();
    result.set_low(value(&values, "low"));
    result.set_high(value(&values, "high"));
    result.set_max(value(&values, "max"));
    result.set_oom(value(&values, "oom"));
    result.set_oom_kill(value(&values, "oom_kill"));
    result.set_oom_group_kill(value(&values, "oom_group_kill"));
    Ok(result)
}

fn v2_rdma_stats(path: &Path) -> Result<metrics_v2::RdmaStat> {
    let mut result = metrics_v2::RdmaStat::new();
    result.set_current(read_rdma_file(&path.join("rdma.current"))?);
    result.set_limit(read_rdma_file(&path.join("rdma.max"))?);
    Ok(result)
}

fn read_rdma_file(path: &Path) -> Result<Vec<metrics_v2::RdmaEntry>> {
    let Some(contents) = read_optional_file(path)? else {
        return Ok(Vec::new());
    };

    contents
        .lines()
        .enumerate()
        .map(|(line_number, line)| {
            let mut fields = line.split_whitespace();
            let device = fields.next().ok_or_else(|| {
                anyhow!(
                    "{}:{}: missing RDMA device",
                    path.display(),
                    line_number + 1
                )
            })?;
            let values = parse_assignments(fields, path, line_number + 1)?;
            let mut entry = metrics_v2::RdmaEntry::new();
            entry.set_device(device.to_string());
            entry.set_hca_handles(value_to_u32(&values, "hca_handle", path)?);
            entry.set_hca_objects(value_to_u32(&values, "hca_object", path)?);
            Ok(entry)
        })
        .collect()
}

fn v2_hugetlb_stats(path: &Path) -> Result<Vec<metrics_v2::HugeTlbStat>> {
    let mut page_sizes = BTreeSet::new();
    for entry in
        fs::read_dir(path).with_context(|| format!("read cgroup directory {}", path.display()))?
    {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        if let Some(page_size) = name
            .strip_prefix("hugetlb.")
            .and_then(|name| name.strip_suffix(".current"))
        {
            page_sizes.insert(page_size.to_string());
        }
    }

    page_sizes
        .into_iter()
        .map(|page_size| {
            let prefix = path.join(format!("hugetlb.{page_size}"));
            let events = read_keyed_file(&path.join(format!("hugetlb.{page_size}.events")))?
                .unwrap_or_default();
            let mut result = metrics_v2::HugeTlbStat::new();
            result.set_pagesize(page_size);
            result.set_current(
                read_single_value(&with_suffix(&prefix, ".current"))?.unwrap_or_default(),
            );
            result.set_max(read_single_value(&with_suffix(&prefix, ".max"))?.unwrap_or_default());
            result.set_failcnt(value(&events, "max"));
            Ok(result)
        })
        .collect()
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{}", path.display(), suffix))
}

fn read_optional_file(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read cgroup file {}", path.display())),
    }
}

fn read_single_value(path: &Path) -> Result<Option<u64>> {
    read_optional_file(path)?
        .map(|contents| parse_u64(contents.trim(), path))
        .transpose()
}

fn read_keyed_file(path: &Path) -> Result<Option<HashMap<String, u64>>> {
    read_optional_file(path)?
        .map(|contents| {
            contents
                .lines()
                .enumerate()
                .map(|(line_number, line)| {
                    let mut fields = line.split_whitespace();
                    let key = fields.next().ok_or_else(|| {
                        anyhow!("{}:{}: missing key", path.display(), line_number + 1)
                    })?;
                    let raw_value = fields.next().ok_or_else(|| {
                        anyhow!("{}:{}: missing value", path.display(), line_number + 1)
                    })?;
                    if fields.next().is_some() {
                        return Err(anyhow!(
                            "{}:{}: unexpected extra field",
                            path.display(),
                            line_number + 1
                        ));
                    }
                    Ok((key.to_string(), parse_u64(raw_value, path)?))
                })
                .collect()
        })
        .transpose()
}

fn read_psi_file(path: &Path) -> Result<Option<metrics_v2::PSIStats>> {
    read_optional_file(path)?
        .map(|contents| {
            let mut result = metrics_v2::PSIStats::new();
            for (line_number, line) in contents.lines().enumerate() {
                let mut fields = line.split_whitespace();
                let kind = fields.next().ok_or_else(|| {
                    anyhow!("{}:{}: missing PSI kind", path.display(), line_number + 1)
                })?;
                let values = fields
                    .map(|field| {
                        field.split_once('=').ok_or_else(|| {
                            anyhow!(
                                "{}:{}: invalid PSI field {field}",
                                path.display(),
                                line_number + 1
                            )
                        })
                    })
                    .collect::<Result<HashMap<_, _>>>()?;
                let mut data = metrics_v2::PSIData::new();
                data.set_avg10(parse_f64_field(&values, "avg10", path)?);
                data.set_avg60(parse_f64_field(&values, "avg60", path)?);
                data.set_avg300(parse_f64_field(&values, "avg300", path)?);
                data.set_total(parse_u64_field(&values, "total", path)?);
                match kind {
                    "some" => result.set_some(data),
                    "full" => result.set_full(data),
                    _ => {
                        return Err(anyhow!(
                            "{}:{}: unknown PSI kind {kind}",
                            path.display(),
                            line_number + 1
                        ))
                    }
                }
            }
            Ok(result)
        })
        .transpose()
}

fn parse_assignments<'a>(
    fields: impl Iterator<Item = &'a str>,
    path: &Path,
    line_number: usize,
) -> Result<HashMap<String, u64>> {
    fields
        .map(|field| {
            let (key, raw_value) = field.split_once('=').ok_or_else(|| {
                anyhow!("{}:{line_number}: invalid field {field}", path.display())
            })?;
            Ok((key.to_string(), parse_u64(raw_value, path)?))
        })
        .collect()
}

fn parse_u64(raw_value: &str, path: &Path) -> Result<u64> {
    if raw_value == "max" {
        return Ok(u64::MAX);
    }
    raw_value
        .parse()
        .with_context(|| format!("parse cgroup value {raw_value:?} from {}", path.display()))
}

fn parse_u64_field(values: &HashMap<&str, &str>, key: &str, path: &Path) -> Result<u64> {
    values
        .get(key)
        .ok_or_else(|| anyhow!("{}: missing {key}", path.display()))
        .and_then(|value| parse_u64(value, path))
}

fn parse_f64_field(values: &HashMap<&str, &str>, key: &str, path: &Path) -> Result<f64> {
    values
        .get(key)
        .ok_or_else(|| anyhow!("{}: missing {key}", path.display()))?
        .parse()
        .with_context(|| format!("parse cgroup field {key} from {}", path.display()))
}

fn value(values: &HashMap<String, u64>, key: &str) -> u64 {
    values.get(key).copied().unwrap_or_default()
}

fn value_to_u32(values: &HashMap<String, u64>, key: &str, path: &Path) -> Result<u32> {
    let value = value(values, key);
    if value == u64::MAX {
        return Ok(u32::MAX);
    }
    u32::try_from(value)
        .with_context(|| format!("cgroup field {key} from {} exceeds u32", path.display()))
}

#[cfg(test)]
mod tests {
    use std::{convert::TryFrom, fs};

    use cgroups::stats::{CpuAcctStats, CpuCgroupStats, MemoryCgroupStats, MemoryStats};
    use cgroups::CgroupStats;
    use containerd_shim_protos::{
        cgroups::metrics as metrics_v1, cgroups_v2::metrics as metrics_v2,
    };
    use protobuf::Message;
    use resource::cgroups::SandboxCgroupStats;
    use tempfile::tempdir;

    use super::{StatsInfo, CGROUP_V1_METRICS_TYPE_URL, CGROUP_V2_METRICS_TYPE_URL};

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

    #[test]
    fn converts_native_cgroup_v2_stats() {
        let directory = tempdir().expect("create cgroup fixture");
        let path = directory.path();
        fs::write(
            path.join("cpu.stat"),
            "usage_usec 42\nuser_usec 30\nsystem_usec 12\nnr_periods 8\nnr_throttled 2\nthrottled_usec 7\nnr_bursts 1\nburst_usec 3\n",
        )
        .unwrap();
        fs::write(
            path.join("cpu.pressure"),
            "some avg10=1.25 avg60=0.50 avg300=0.10 total=123\nfull avg10=0.25 avg60=0.20 avg300=0.05 total=23\n",
        )
        .unwrap();
        fs::write(
            path.join("memory.stat"),
            "anon 1024\nfile 2048\nfile_mapped 64\npgfault 11\npgmajfault 2\n",
        )
        .unwrap();
        fs::write(path.join("memory.current"), "4096\n").unwrap();
        fs::write(path.join("memory.max"), "max\n").unwrap();
        fs::write(path.join("memory.peak"), "8192\n").unwrap();
        fs::write(path.join("memory.swap.current"), "128\n").unwrap();
        fs::write(path.join("memory.swap.max"), "256\n").unwrap();
        fs::write(path.join("memory.swap.peak"), "192\n").unwrap();
        fs::write(
            path.join("memory.events"),
            "low 1\nhigh 2\nmax 3\noom 4\noom_kill 5\noom_group_kill 6\n",
        )
        .unwrap();
        fs::write(path.join("pids.current"), "9\n").unwrap();
        fs::write(path.join("pids.max"), "max\n").unwrap();
        fs::write(
            path.join("io.stat"),
            "8:0 rbytes=10 wbytes=20 rios=2 wios=3 dbytes=4 dios=1\n",
        )
        .unwrap();
        fs::write(
            path.join("io.pressure"),
            "some avg10=0.10 avg60=0.20 avg300=0.30 total=40\nfull avg10=0.01 avg60=0.02 avg300=0.03 total=4\n",
        )
        .unwrap();
        fs::write(path.join("hugetlb.2MB.current"), "2097152\n").unwrap();
        fs::write(path.join("hugetlb.2MB.max"), "max\n").unwrap();
        fs::write(path.join("hugetlb.2MB.events"), "max 7\n").unwrap();
        fs::write(
            path.join("rdma.current"),
            "mlx5_0 hca_handle=2 hca_object=20\n",
        )
        .unwrap();
        fs::write(
            path.join("rdma.max"),
            "mlx5_0 hca_handle=max hca_object=40\n",
        )
        .unwrap();

        let value = StatsInfo::try_from(SandboxCgroupStats::V2(path.to_path_buf()))
            .expect("convert v2 metrics")
            .value
            .expect("metrics payload");
        assert_eq!(value.type_url, CGROUP_V2_METRICS_TYPE_URL);

        let decoded = metrics_v2::Metrics::parse_from_bytes(&value.value).expect("decode metrics");
        assert_eq!(decoded.cpu().usage_usec(), 42);
        assert_eq!(decoded.cpu().nr_throttled(), 2);
        assert_eq!(decoded.cpu().psi().some().total(), 123);
        assert_eq!(decoded.memory().anon(), 1024);
        assert_eq!(decoded.memory().file(), 2048);
        assert_eq!(decoded.memory().usage(), 4096);
        assert_eq!(decoded.memory().usage_limit(), u64::MAX);
        assert_eq!(decoded.memory().swap_max_usage(), 192);
        assert_eq!(decoded.memory_events().oom_kill(), 5);
        assert_eq!(decoded.pids().current(), 9);
        assert_eq!(decoded.pids().limit(), u64::MAX);
        assert_eq!(decoded.io().usage()[0].major(), 8);
        assert_eq!(decoded.io().usage()[0].wbytes(), 20);
        assert_eq!(decoded.io().psi().full().total(), 4);
        assert_eq!(decoded.hugetlb()[0].pagesize(), "2MB");
        assert_eq!(decoded.hugetlb()[0].max(), u64::MAX);
        assert_eq!(decoded.hugetlb()[0].failcnt(), 7);
        assert_eq!(decoded.rdma().current()[0].device(), "mlx5_0");
        assert_eq!(decoded.rdma().current()[0].hca_objects(), 20);
        assert_eq!(decoded.rdma().limit()[0].hca_handles(), u32::MAX);
    }
}
