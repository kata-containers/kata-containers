// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[derive(thiserror::Error, Debug)]
pub enum Error {}

pub type Result<T> = std::result::Result<T, Error>;

// TODO: just a placeholder here. Details will be released later.
pub trait Agent: Send + Sync {
    fn get_oom_event(&self) -> Result<String>;
}

#[derive(Clone, Default, Debug)]
pub struct StatsContainerResponse {
    pub cgroup_stats: Option<CgroupStats>,
    pub network_stats: Vec<NetworkStats>,
}

#[derive(Clone, Default, Debug)]
pub struct CgroupStats {
    pub cpu_stats: Option<CpuStats>,
    pub memory_stats: Option<MemoryStats>,
    pub pids_stats: Option<PidsStats>,
    pub blkio_stats: Option<BlkioStats>,
    pub hugetlb_stats: ::std::collections::HashMap<::std::string::String, HugetlbStats>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct CpuStats {
    pub cpu_usage: Option<CpuUsage>,
    pub throttling_data: Option<ThrottlingData>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct CpuUsage {
    pub total_usage: u64,
    pub percpu_usage: ::std::vec::Vec<u64>,
    pub usage_in_kernelmode: u64,
    pub usage_in_usermode: u64,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct ThrottlingData {
    pub periods: u64,
    pub throttled_periods: u64,
    pub throttled_time: u64,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct MemoryStats {
    pub cache: u64,
    pub usage: Option<MemoryData>,
    pub swap_usage: Option<MemoryData>,
    pub kernel_usage: Option<MemoryData>,
    pub use_hierarchy: bool,
    pub stats: ::std::collections::HashMap<::std::string::String, u64>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct MemoryData {
    pub usage: u64,
    pub max_usage: u64,
    pub failcnt: u64,
    pub limit: u64,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct PidsStats {
    pub current: u64,
    pub limit: u64,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct BlkioStats {
    pub io_service_bytes_recursive: Vec<BlkioStatsEntry>,
    pub io_serviced_recursive: Vec<BlkioStatsEntry>,
    pub io_queued_recursive: Vec<BlkioStatsEntry>,
    pub io_service_time_recursive: Vec<BlkioStatsEntry>,
    pub io_wait_time_recursive: Vec<BlkioStatsEntry>,
    pub io_merged_recursive: Vec<BlkioStatsEntry>,
    pub io_time_recursive: Vec<BlkioStatsEntry>,
    pub sectors_recursive: Vec<BlkioStatsEntry>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct BlkioStatsEntry {
    pub major: u64,
    pub minor: u64,
    pub op: ::std::string::String,
    pub value: u64,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct HugetlbStats {
    pub usage: u64,
    pub max_usage: u64,
    pub failcnt: u64,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct NetworkStats {
    pub name: ::std::string::String,
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errors: u64,
    pub rx_dropped: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errors: u64,
    pub tx_dropped: u64,
}
