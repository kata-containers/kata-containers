// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use serde::Deserialize;

#[derive(PartialEq, Clone, Default)]
pub struct Empty {}

impl Empty {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(PartialEq, Clone, Default)]
pub struct StringUser {
    pub uid: String,
    pub gid: String,
    pub additional_gids: Vec<String>,
}

#[derive(PartialEq, Clone, Default)]
pub struct Device {
    pub id: String,
    pub field_type: String,
    pub vm_path: String,
    pub container_path: String,
    pub options: Vec<String>,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Storage {
    pub driver: String,
    pub driver_options: Vec<String>,
    pub source: String,
    pub fs_type: String,
    pub options: Vec<String>,
    pub mount_point: String,
}

#[derive(Deserialize, Clone, PartialEq, Eq, Debug, Hash)]
pub enum IPFamily {
    V4 = 0,
    V6 = 1,
}

impl ::std::default::Default for IPFamily {
    fn default() -> Self {
        IPFamily::V4
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone, Default)]
pub struct IPAddress {
    pub family: IPFamily,
    pub address: String,
    pub mask: String,
}

#[derive(Deserialize, Debug, PartialEq, Clone, Default)]
pub struct Interface {
    pub device: String,
    pub name: String,
    pub ip_addresses: Vec<IPAddress>,
    pub mtu: u64,
    pub hw_addr: String,
    #[serde(default)]
    pub pci_addr: String,
    #[serde(default)]
    pub field_type: String,
    #[serde(default)]
    pub raw_flags: u32,
}

#[derive(PartialEq, Clone, Default)]
pub struct Interfaces {
    pub interfaces: Vec<Interface>,
}

#[derive(Deserialize, Debug, PartialEq, Clone, Default)]
pub struct Route {
    pub dest: String,
    pub gateway: String,
    pub device: String,
    pub source: String,
    pub scope: u32,
    pub family: IPFamily,
}

#[derive(Deserialize, Debug, PartialEq, Clone, Default)]
pub struct Routes {
    pub routes: Vec<Route>,
}

#[derive(PartialEq, Clone, Default)]
pub struct CreateContainerRequest {
    pub process_id: ContainerProcessID,
    pub string_user: Option<StringUser>,
    pub devices: Vec<Device>,
    pub storages: Vec<Storage>,
    pub oci: Option<oci::Spec>,
    pub guest_hooks: Option<oci::Hooks>,
    pub sandbox_pidns: bool,
    pub rootfs_mounts: Vec<oci::Mount>,
}

#[derive(PartialEq, Clone, Default)]
pub struct ContainerID {
    pub container_id: String,
}

impl ContainerID {
    pub fn new(id: &str) -> Self {
        Self {
            container_id: id.to_string(),
        }
    }
}

#[derive(PartialEq, Clone, Default)]
pub struct ContainerProcessID {
    pub container_id: ContainerID,
    pub exec_id: String,
}

impl ContainerProcessID {
    pub fn new(container_id: &str, exec_id: &str) -> Self {
        Self {
            container_id: ContainerID::new(container_id),
            exec_id: exec_id.to_string(),
        }
    }

    pub fn container_id(&self) -> String {
        self.container_id.container_id.clone()
    }

    pub fn exec_id(&self) -> String {
        self.exec_id.clone()
    }
}

#[derive(PartialEq, Clone, Debug, Default)]
pub struct RemoveContainerRequest {
    pub container_id: String,
    pub timeout: u32,
}

impl RemoveContainerRequest {
    pub fn new(id: &str, timeout: u32) -> Self {
        Self {
            container_id: id.to_string(),
            timeout,
        }
    }
}

#[derive(PartialEq, Clone, Default)]
pub struct SignalProcessRequest {
    pub process_id: ContainerProcessID,
    pub signal: u32,
}

#[derive(PartialEq, Clone, Default)]
pub struct WaitProcessRequest {
    pub process_id: ContainerProcessID,
}

#[derive(PartialEq, Clone, Default)]
pub struct ListProcessesRequest {
    pub container_id: String,
    pub format: String,
    pub args: Vec<String>,
}

#[derive(PartialEq, Clone, Default)]
pub struct UpdateContainerRequest {
    pub container_id: String,
    pub resources: oci::LinuxResources,
    pub mounts: Vec<oci::Mount>,
}

#[derive(PartialEq, Clone, Default)]
pub struct WriteStreamRequest {
    pub process_id: ContainerProcessID,
    pub data: Vec<u8>,
}

#[derive(PartialEq, Clone, Default)]
pub struct WriteStreamResponse {
    pub length: u32,
}

#[derive(PartialEq, Clone, Default)]
pub struct ExecProcessRequest {
    pub process_id: ContainerProcessID,
    pub string_user: Option<StringUser>,
    pub process: Option<oci::Process>,
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
pub struct LoadData {
    pub one: String,
    pub five: String,
    pub fifteen: String,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct CpuStats {
    pub cpu_usage: Option<CpuUsage>,
    pub throttling_data: Option<ThrottlingData>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct MemoryData {
    pub usage: u64,
    pub max_usage: u64,
    pub failcnt: u64,
    pub limit: u64,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct MemoryStats {
    pub cache: u64,
    pub usage: Option<MemoryData>,
    pub swap_usage: Option<MemoryData>,
    pub kernel_usage: Option<MemoryData>,
    pub use_hierarchy: bool,
    pub stats: ::std::collections::HashMap<String, u64>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct PidsStats {
    pub current: u64,
    pub limit: u64,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct BlkioStatsEntry {
    pub major: u64,
    pub minor: u64,
    pub op: String,
    pub value: u64,
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
pub struct HugetlbStats {
    pub usage: u64,
    pub max_usage: u64,
    pub failcnt: u64,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct CgroupStats {
    pub cpu_stats: Option<CpuStats>,
    pub memory_stats: Option<MemoryStats>,
    pub pids_stats: Option<PidsStats>,
    pub blkio_stats: Option<BlkioStats>,
    pub hugetlb_stats: ::std::collections::HashMap<String, HugetlbStats>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct NetworkStats {
    pub name: String,
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errors: u64,
    pub rx_dropped: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errors: u64,
    pub tx_dropped: u64,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct StatsContainerResponse {
    pub cgroup_stats: Option<CgroupStats>,
    pub network_stats: Vec<NetworkStats>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct WaitProcessResponse {
    pub status: i32,
}

#[derive(PartialEq, Clone, Default)]
pub struct ReadStreamRequest {
    pub process_id: ContainerProcessID,
    pub len: u32,
}

#[derive(PartialEq, Clone, Default)]
pub struct ReadStreamResponse {
    pub data: Vec<u8>,
}

#[derive(PartialEq, Clone, Default)]
pub struct CloseStdinRequest {
    pub process_id: ContainerProcessID,
}

#[derive(PartialEq, Clone, Default)]
pub struct TtyWinResizeRequest {
    pub process_id: ContainerProcessID,
    pub row: u32,
    pub column: u32,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct UpdateInterfaceRequest {
    pub interface: Option<Interface>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct UpdateRoutesRequest {
    pub route: Option<Routes>,
}

#[derive(Deserialize, PartialEq, Clone, Default, Debug)]
pub struct ARPNeighbor {
    pub to_ip_address: Option<IPAddress>,
    pub device: String,
    pub ll_addr: String,
    pub state: i32,
    pub flags: i32,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct ARPNeighbors {
    pub neighbors: Vec<ARPNeighbor>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct AddArpNeighborRequest {
    pub neighbors: Option<ARPNeighbors>,
}

#[derive(PartialEq, Clone, Default)]
pub struct KernelModule {
    pub name: String,
    pub parameters: Vec<String>,
}

#[derive(PartialEq, Clone, Default)]
pub struct CreateSandboxRequest {
    pub hostname: String,
    pub dns: Vec<String>,
    pub storages: Vec<Storage>,
    pub sandbox_pidns: bool,
    pub sandbox_id: String,
    pub guest_hook_path: String,
    pub kernel_modules: Vec<KernelModule>,
}

#[derive(PartialEq, Clone, Default)]
pub struct OnlineCPUMemRequest {
    pub wait: bool,
    pub nb_cpus: u32,
    pub cpu_only: bool,
}

#[derive(PartialEq, Clone, Default)]
pub struct ReseedRandomDevRequest {
    pub data: ::std::vec::Vec<u8>,
}

#[derive(PartialEq, Clone, Default)]
pub struct GetGuestDetailsRequest {
    pub mem_block_size: bool,
    pub mem_hotplug_probe: bool,
}

#[derive(PartialEq, Clone, Default)]
pub struct MemHotplugByProbeRequest {
    pub mem_hotplug_probe_addr: ::std::vec::Vec<u64>,
}

#[derive(PartialEq, Clone, Default)]
pub struct SetGuestDateTimeRequest {
    pub sec: i64,
    pub usec: i64,
}

#[derive(PartialEq, Clone, Default)]
pub struct AgentDetails {
    pub version: String,
    pub init_daemon: bool,
    pub device_handlers: Vec<String>,
    pub storage_handlers: Vec<std::string::String>,
    pub supports_seccomp: bool,
}

#[derive(PartialEq, Clone, Default)]
pub struct GuestDetailsResponse {
    pub mem_block_size_bytes: u64,
    pub agent_details: Option<AgentDetails>,
    pub support_mem_hotplug_probe: bool,
}

#[derive(PartialEq, Clone, Default)]
pub struct CopyFileRequest {
    pub path: String,
    pub file_size: i64,
    pub file_mode: u32,
    pub dir_mode: u32,
    pub uid: i32,
    pub gid: i32,
    pub offset: i64,
    pub data: ::std::vec::Vec<u8>,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct CheckRequest {
    pub service: String,
}

impl CheckRequest {
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_string(),
        }
    }
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct HealthCheckResponse {
    pub status: u32,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct VersionCheckResponse {
    pub grpc_version: String,
    pub agent_version: String,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct OomEventResponse {
    pub container_id: String,
}
