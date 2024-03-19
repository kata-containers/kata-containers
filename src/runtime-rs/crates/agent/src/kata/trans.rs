// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::Into;

use protocols::{
    agent::{self, Metrics, OOMEvent},
    csi, empty, health, types,
};

use crate::{
    types::{
        ARPNeighbor, ARPNeighbors, AddArpNeighborRequest, AgentDetails, BlkioStats,
        BlkioStatsEntry, CgroupStats, CheckRequest, CloseStdinRequest, ContainerID,
        CopyFileRequest, CpuStats, CpuUsage, CreateContainerRequest, CreateSandboxRequest, Device,
        Empty, ExecProcessRequest, FSGroup, FSGroupChangePolicy, GetIPTablesRequest,
        GetIPTablesResponse, GuestDetailsResponse, HealthCheckResponse, HugetlbStats, IPAddress,
        IPFamily, Interface, Interfaces, KernelModule, MemHotplugByProbeRequest, MemoryData,
        MemoryStats, MetricsResponse, NetworkStats, OnlineCPUMemRequest, PidsStats,
        ReadStreamRequest, ReadStreamResponse, RemoveContainerRequest, ReseedRandomDevRequest,
        ResizeVolumeRequest, Route, Routes, SetGuestDateTimeRequest, SetIPTablesRequest,
        SetIPTablesResponse, SharedMount, SignalProcessRequest, StatsContainerResponse, Storage,
        StringUser, ThrottlingData, TtyWinResizeRequest, UpdateContainerRequest,
        UpdateInterfaceRequest, UpdateRoutesRequest, VersionCheckResponse, VolumeStatsRequest,
        VolumeStatsResponse, WaitProcessRequest, WriteStreamRequest,
    },
    GetGuestDetailsRequest, OomEventResponse, WaitProcessResponse, WriteStreamResponse,
};

fn trans_vec<F: Sized + Clone, T: From<F>>(from: Vec<F>) -> Vec<T> {
    from.into_iter().map(|f| f.into()).collect()
}

fn from_option<F: Sized, T: From<F>>(from: Option<F>) -> protobuf::MessageField<T> {
    match from {
        Some(f) => protobuf::MessageField::from_option(Some(T::from(f))),
        None => protobuf::MessageField::none(),
    }
}

fn into_option<F: Into<T>, T: Sized>(from: protobuf::MessageField<F>) -> Option<T> {
    from.into_option().map(|f| f.into())
}

fn into_hash_map<F: Into<T>, T>(
    from: std::collections::HashMap<String, F>,
) -> std::collections::HashMap<String, T> {
    let mut to: std::collections::HashMap<String, T> = Default::default();

    for (key, value) in from {
        to.insert(key, value.into());
    }

    to
}

impl From<empty::Empty> for Empty {
    fn from(_: empty::Empty) -> Self {
        Self {}
    }
}

impl From<FSGroup> for agent::FSGroup {
    fn from(from: FSGroup) -> Self {
        let policy = match from.group_change_policy {
            FSGroupChangePolicy::Always => types::FSGroupChangePolicy::Always,
            FSGroupChangePolicy::OnRootMismatch => types::FSGroupChangePolicy::OnRootMismatch,
        };

        Self {
            group_id: from.group_id,
            group_change_policy: policy.into(),
            ..Default::default()
        }
    }
}

impl From<StringUser> for agent::StringUser {
    fn from(from: StringUser) -> Self {
        Self {
            uid: from.uid,
            gid: from.gid,
            additionalGids: from.additional_gids,
            ..Default::default()
        }
    }
}

impl From<Device> for agent::Device {
    fn from(from: Device) -> Self {
        Self {
            id: from.id,
            type_: from.field_type,
            vm_path: from.vm_path,
            container_path: from.container_path,
            options: trans_vec(from.options),
            ..Default::default()
        }
    }
}

impl From<Storage> for agent::Storage {
    fn from(from: Storage) -> Self {
        Self {
            driver: from.driver,
            driver_options: trans_vec(from.driver_options),
            source: from.source,
            fstype: from.fs_type,
            fs_group: from_option(from.fs_group),
            options: trans_vec(from.options),
            mount_point: from.mount_point,
            ..Default::default()
        }
    }
}

impl From<SharedMount> for agent::SharedMount {
    fn from(from: SharedMount) -> Self {
        Self {
            name: from.name,
            src_ctr: from.src_ctr,
            src_path: from.src_path,
            dst_ctr: from.dst_ctr,
            dst_path: from.dst_path,
            ..Default::default()
        }
    }
}

impl From<KernelModule> for agent::KernelModule {
    fn from(from: KernelModule) -> Self {
        Self {
            name: from.name,
            parameters: trans_vec(from.parameters),
            ..Default::default()
        }
    }
}

impl From<IPFamily> for types::IPFamily {
    fn from(from: IPFamily) -> Self {
        if from == IPFamily::V4 {
            types::IPFamily::v4
        } else {
            types::IPFamily::v6
        }
    }
}

impl From<types::IPFamily> for IPFamily {
    fn from(src: types::IPFamily) -> Self {
        match src {
            types::IPFamily::v4 => IPFamily::V4,
            types::IPFamily::v6 => IPFamily::V6,
        }
    }
}

impl From<IPAddress> for types::IPAddress {
    fn from(from: IPAddress) -> Self {
        Self {
            family: protobuf::EnumOrUnknown::new(from.family.into()),
            address: from.address,
            mask: from.mask,
            ..Default::default()
        }
    }
}

impl From<types::IPAddress> for IPAddress {
    fn from(src: types::IPAddress) -> Self {
        Self {
            family: src.family.unwrap().into(),
            address: "".to_string(),
            mask: "".to_string(),
        }
    }
}

impl From<Interface> for types::Interface {
    fn from(from: Interface) -> Self {
        Self {
            device: from.device,
            name: from.name,
            IPAddresses: trans_vec(from.ip_addresses),
            mtu: from.mtu,
            hwAddr: from.hw_addr,
            pciPath: from.pci_addr,
            type_: from.field_type,
            raw_flags: from.raw_flags,
            ..Default::default()
        }
    }
}

impl From<types::Interface> for Interface {
    fn from(src: types::Interface) -> Self {
        Self {
            device: src.device,
            name: src.name,
            ip_addresses: trans_vec(src.IPAddresses),
            mtu: src.mtu,
            hw_addr: src.hwAddr,
            pci_addr: src.pciPath,
            field_type: src.type_,
            raw_flags: src.raw_flags,
        }
    }
}

impl From<agent::Interfaces> for Interfaces {
    fn from(src: agent::Interfaces) -> Self {
        Self {
            interfaces: trans_vec(src.Interfaces),
        }
    }
}

impl From<Route> for types::Route {
    fn from(from: Route) -> Self {
        Self {
            dest: from.dest,
            gateway: from.gateway,
            device: from.device,
            source: from.source,
            scope: from.scope,
            family: protobuf::EnumOrUnknown::new(from.family.into()),
            ..Default::default()
        }
    }
}

impl From<types::Route> for Route {
    fn from(src: types::Route) -> Self {
        Self {
            dest: src.dest,
            gateway: src.gateway,
            device: src.device,
            source: src.source,
            scope: src.scope,
            family: src.family.unwrap().into(),
        }
    }
}

impl From<Routes> for agent::Routes {
    fn from(from: Routes) -> Self {
        Self {
            Routes: trans_vec(from.routes),
            ..Default::default()
        }
    }
}

impl From<agent::Routes> for Routes {
    fn from(src: agent::Routes) -> Self {
        Self {
            routes: trans_vec(src.Routes),
        }
    }
}

impl From<CreateContainerRequest> for agent::CreateContainerRequest {
    fn from(from: CreateContainerRequest) -> Self {
        Self {
            container_id: from.process_id.container_id(),
            exec_id: from.process_id.exec_id(),
            string_user: from_option(from.string_user),
            devices: trans_vec(from.devices),
            storages: trans_vec(from.storages),
            OCI: from_option(from.oci),
            sandbox_pidns: from.sandbox_pidns,
            shared_mounts: trans_vec(from.shared_mounts),
            stdin_port: from.stdin_port.unwrap_or_default(),
            stdout_port: from.stdout_port.unwrap_or_default(),
            stderr_port: from.stderr_port.unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl From<RemoveContainerRequest> for agent::RemoveContainerRequest {
    fn from(from: RemoveContainerRequest) -> Self {
        Self {
            container_id: from.container_id,
            timeout: from.timeout,
            ..Default::default()
        }
    }
}

impl From<ContainerID> for agent::StartContainerRequest {
    fn from(from: ContainerID) -> Self {
        Self {
            container_id: from.container_id,
            ..Default::default()
        }
    }
}

impl From<ContainerID> for agent::StatsContainerRequest {
    fn from(from: ContainerID) -> Self {
        Self {
            container_id: from.container_id,
            ..Default::default()
        }
    }
}

impl From<ContainerID> for agent::PauseContainerRequest {
    fn from(from: ContainerID) -> Self {
        Self {
            container_id: from.container_id,
            ..Default::default()
        }
    }
}

impl From<ContainerID> for agent::ResumeContainerRequest {
    fn from(from: ContainerID) -> Self {
        Self {
            container_id: from.container_id,
            ..Default::default()
        }
    }
}

impl From<SignalProcessRequest> for agent::SignalProcessRequest {
    fn from(from: SignalProcessRequest) -> Self {
        Self {
            container_id: from.process_id.container_id(),
            exec_id: from.process_id.exec_id(),
            signal: from.signal,
            ..Default::default()
        }
    }
}

impl From<WaitProcessRequest> for agent::WaitProcessRequest {
    fn from(from: WaitProcessRequest) -> Self {
        Self {
            container_id: from.process_id.container_id(),
            exec_id: from.process_id.exec_id(),
            ..Default::default()
        }
    }
}

impl From<UpdateContainerRequest> for agent::UpdateContainerRequest {
    fn from(from: UpdateContainerRequest) -> Self {
        Self {
            container_id: from.container_id,
            resources: from_option(from.resources),
            ..Default::default()
        }
    }
}

impl From<WriteStreamRequest> for agent::WriteStreamRequest {
    fn from(from: WriteStreamRequest) -> Self {
        Self {
            container_id: from.process_id.container_id(),
            exec_id: from.process_id.exec_id(),
            data: from.data,
            ..Default::default()
        }
    }
}

impl From<agent::WriteStreamResponse> for WriteStreamResponse {
    fn from(from: agent::WriteStreamResponse) -> Self {
        Self { length: from.len }
    }
}

impl From<GetIPTablesRequest> for agent::GetIPTablesRequest {
    fn from(from: GetIPTablesRequest) -> Self {
        Self {
            is_ipv6: from.is_ipv6,
            ..Default::default()
        }
    }
}

impl From<agent::GetIPTablesResponse> for GetIPTablesResponse {
    fn from(from: agent::GetIPTablesResponse) -> Self {
        Self {
            data: from.data().to_vec(),
        }
    }
}

impl From<SetIPTablesRequest> for agent::SetIPTablesRequest {
    fn from(from: SetIPTablesRequest) -> Self {
        Self {
            is_ipv6: from.is_ipv6,
            data: from.data,
            ..Default::default()
        }
    }
}

impl From<agent::SetIPTablesResponse> for SetIPTablesResponse {
    fn from(from: agent::SetIPTablesResponse) -> Self {
        Self {
            data: from.data().to_vec(),
        }
    }
}

impl From<ExecProcessRequest> for agent::ExecProcessRequest {
    fn from(from: ExecProcessRequest) -> Self {
        Self {
            container_id: from.process_id.container_id(),
            exec_id: from.process_id.exec_id(),
            string_user: from_option(from.string_user),
            process: from_option(from.process),
            stdin_port: from.stdin_port.unwrap_or_default(),
            stdout_port: from.stdout_port.unwrap_or_default(),
            stderr_port: from.stderr_port.unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl From<agent::CpuUsage> for CpuUsage {
    fn from(src: agent::CpuUsage) -> Self {
        Self {
            total_usage: src.total_usage,
            percpu_usage: src.percpu_usage,
            usage_in_kernelmode: src.usage_in_kernelmode,
            usage_in_usermode: src.usage_in_usermode,
        }
    }
}

impl From<agent::ThrottlingData> for ThrottlingData {
    fn from(src: agent::ThrottlingData) -> Self {
        Self {
            periods: src.periods,
            throttled_periods: src.throttled_periods,
            throttled_time: src.throttled_time,
        }
    }
}

impl From<agent::CpuStats> for CpuStats {
    fn from(src: agent::CpuStats) -> Self {
        Self {
            cpu_usage: into_option(src.cpu_usage),
            throttling_data: into_option(src.throttling_data),
        }
    }
}

impl From<agent::MemoryData> for MemoryData {
    fn from(src: agent::MemoryData) -> Self {
        Self {
            usage: src.usage,
            max_usage: src.max_usage,
            failcnt: src.failcnt,
            limit: src.limit,
        }
    }
}

impl From<agent::MemoryStats> for MemoryStats {
    fn from(src: agent::MemoryStats) -> Self {
        Self {
            cache: src.cache,
            usage: into_option(src.usage),
            swap_usage: into_option(src.swap_usage),
            kernel_usage: into_option(src.kernel_usage),
            use_hierarchy: src.use_hierarchy,
            stats: into_hash_map(src.stats),
        }
    }
}

impl From<agent::PidsStats> for PidsStats {
    fn from(src: agent::PidsStats) -> Self {
        Self {
            current: src.current,
            limit: src.limit,
        }
    }
}

impl From<agent::BlkioStatsEntry> for BlkioStatsEntry {
    fn from(src: agent::BlkioStatsEntry) -> Self {
        Self {
            major: src.major,
            minor: src.minor,
            op: src.op,
            value: src.value,
        }
    }
}

impl From<agent::BlkioStats> for BlkioStats {
    fn from(src: agent::BlkioStats) -> Self {
        Self {
            io_service_bytes_recursive: trans_vec(src.io_service_bytes_recursive),
            io_serviced_recursive: trans_vec(src.io_serviced_recursive),
            io_queued_recursive: trans_vec(src.io_queued_recursive),
            io_service_time_recursive: trans_vec(src.io_service_time_recursive),
            io_wait_time_recursive: trans_vec(src.io_wait_time_recursive),
            io_merged_recursive: trans_vec(src.io_merged_recursive),
            io_time_recursive: trans_vec(src.io_time_recursive),
            sectors_recursive: trans_vec(src.sectors_recursive),
        }
    }
}

impl From<agent::HugetlbStats> for HugetlbStats {
    fn from(src: agent::HugetlbStats) -> Self {
        Self {
            usage: src.usage,
            max_usage: src.max_usage,
            failcnt: src.failcnt,
        }
    }
}

impl From<agent::CgroupStats> for CgroupStats {
    fn from(src: agent::CgroupStats) -> Self {
        Self {
            cpu_stats: into_option(src.cpu_stats),
            memory_stats: into_option(src.memory_stats),
            pids_stats: into_option(src.pids_stats),
            blkio_stats: into_option(src.blkio_stats),
            hugetlb_stats: into_hash_map(src.hugetlb_stats),
        }
    }
}

impl From<agent::NetworkStats> for NetworkStats {
    fn from(src: agent::NetworkStats) -> Self {
        Self {
            name: src.name,
            rx_bytes: src.rx_bytes,
            rx_packets: src.rx_packets,
            rx_errors: src.rx_errors,
            rx_dropped: src.rx_dropped,
            tx_bytes: src.tx_bytes,
            tx_packets: src.tx_packets,
            tx_errors: src.tx_errors,
            tx_dropped: src.tx_dropped,
        }
    }
}

// translate ttrpc::agent response to interface::agent response
impl From<agent::StatsContainerResponse> for StatsContainerResponse {
    fn from(src: agent::StatsContainerResponse) -> Self {
        Self {
            cgroup_stats: into_option(src.cgroup_stats),
            network_stats: trans_vec(src.network_stats),
        }
    }
}

impl From<ReadStreamRequest> for agent::ReadStreamRequest {
    fn from(from: ReadStreamRequest) -> Self {
        Self {
            container_id: from.process_id.container_id(),
            exec_id: from.process_id.exec_id(),
            len: from.len,
            ..Default::default()
        }
    }
}

impl From<agent::ReadStreamResponse> for ReadStreamResponse {
    fn from(from: agent::ReadStreamResponse) -> Self {
        Self { data: from.data }
    }
}

impl From<CloseStdinRequest> for agent::CloseStdinRequest {
    fn from(from: CloseStdinRequest) -> Self {
        Self {
            container_id: from.process_id.container_id(),
            exec_id: from.process_id.exec_id(),
            ..Default::default()
        }
    }
}

impl From<TtyWinResizeRequest> for agent::TtyWinResizeRequest {
    fn from(from: TtyWinResizeRequest) -> Self {
        Self {
            container_id: from.process_id.container_id(),
            exec_id: from.process_id.exec_id(),
            row: from.row,
            column: from.column,
            ..Default::default()
        }
    }
}

impl From<UpdateInterfaceRequest> for agent::UpdateInterfaceRequest {
    fn from(from: UpdateInterfaceRequest) -> Self {
        Self {
            interface: from_option(from.interface),
            ..Default::default()
        }
    }
}

impl From<Empty> for agent::ListInterfacesRequest {
    fn from(_: Empty) -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl From<UpdateRoutesRequest> for agent::UpdateRoutesRequest {
    fn from(from: UpdateRoutesRequest) -> Self {
        Self {
            routes: from_option(from.route),
            ..Default::default()
        }
    }
}

impl From<Empty> for agent::ListRoutesRequest {
    fn from(_: Empty) -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl From<ARPNeighbor> for types::ARPNeighbor {
    fn from(from: ARPNeighbor) -> Self {
        Self {
            toIPAddress: from_option(from.to_ip_address),
            device: from.device,
            lladdr: from.ll_addr,
            state: from.state,
            flags: from.flags,
            ..Default::default()
        }
    }
}

impl From<ARPNeighbors> for agent::ARPNeighbors {
    fn from(from: ARPNeighbors) -> Self {
        Self {
            ARPNeighbors: trans_vec(from.neighbors),
            ..Default::default()
        }
    }
}

impl From<AddArpNeighborRequest> for agent::AddARPNeighborsRequest {
    fn from(from: AddArpNeighborRequest) -> Self {
        Self {
            neighbors: from_option(from.neighbors),
            ..Default::default()
        }
    }
}

impl From<CreateSandboxRequest> for agent::CreateSandboxRequest {
    fn from(from: CreateSandboxRequest) -> Self {
        Self {
            hostname: from.hostname,
            dns: trans_vec(from.dns),
            storages: trans_vec(from.storages),
            sandbox_pidns: from.sandbox_pidns,
            sandbox_id: from.sandbox_id,
            guest_hook_path: from.guest_hook_path,
            kernel_modules: trans_vec(from.kernel_modules),
            ..Default::default()
        }
    }
}

impl From<Empty> for agent::DestroySandboxRequest {
    fn from(_: Empty) -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl From<OnlineCPUMemRequest> for agent::OnlineCPUMemRequest {
    fn from(from: OnlineCPUMemRequest) -> Self {
        Self {
            wait: from.wait,
            nb_cpus: from.nb_cpus,
            cpu_only: from.cpu_only,
            ..Default::default()
        }
    }
}

impl From<ReseedRandomDevRequest> for agent::ReseedRandomDevRequest {
    fn from(from: ReseedRandomDevRequest) -> Self {
        Self {
            data: from.data,
            ..Default::default()
        }
    }
}

impl From<MemHotplugByProbeRequest> for agent::MemHotplugByProbeRequest {
    fn from(from: MemHotplugByProbeRequest) -> Self {
        Self {
            memHotplugProbeAddr: from.mem_hotplug_probe_addr,
            ..Default::default()
        }
    }
}

impl From<SetGuestDateTimeRequest> for agent::SetGuestDateTimeRequest {
    fn from(from: SetGuestDateTimeRequest) -> Self {
        Self {
            Sec: from.sec,
            Usec: from.usec,
            ..Default::default()
        }
    }
}

impl From<GetGuestDetailsRequest> for agent::GuestDetailsRequest {
    fn from(from: GetGuestDetailsRequest) -> Self {
        Self {
            mem_block_size: from.mem_block_size,
            mem_hotplug_probe: from.mem_hotplug_probe,
            ..Default::default()
        }
    }
}

impl From<agent::AgentDetails> for AgentDetails {
    fn from(src: agent::AgentDetails) -> Self {
        Self {
            version: src.version,
            init_daemon: src.init_daemon,
            device_handlers: trans_vec(src.device_handlers),
            storage_handlers: trans_vec(src.storage_handlers),
            supports_seccomp: src.supports_seccomp,
            extra_features: trans_vec(src.extra_features),
        }
    }
}

impl From<agent::GuestDetailsResponse> for GuestDetailsResponse {
    fn from(src: agent::GuestDetailsResponse) -> Self {
        Self {
            mem_block_size_bytes: src.mem_block_size_bytes,
            agent_details: into_option(src.agent_details),
            support_mem_hotplug_probe: src.support_mem_hotplug_probe,
        }
    }
}

impl From<CopyFileRequest> for agent::CopyFileRequest {
    fn from(from: CopyFileRequest) -> Self {
        Self {
            path: from.path,
            file_size: from.file_size,
            file_mode: from.file_mode,
            dir_mode: from.dir_mode,
            uid: from.uid,
            gid: from.gid,
            offset: from.offset,
            data: from.data,
            ..Default::default()
        }
    }
}

impl From<agent::WaitProcessResponse> for WaitProcessResponse {
    fn from(from: agent::WaitProcessResponse) -> Self {
        Self {
            status: from.status,
        }
    }
}

impl From<Empty> for agent::GetMetricsRequest {
    fn from(_: Empty) -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl From<Empty> for agent::GetOOMEventRequest {
    fn from(_: Empty) -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl From<CheckRequest> for health::CheckRequest {
    fn from(from: CheckRequest) -> Self {
        Self {
            service: from.service,
            ..Default::default()
        }
    }
}

impl From<health::HealthCheckResponse> for HealthCheckResponse {
    fn from(from: health::HealthCheckResponse) -> Self {
        Self {
            status: from.status.value() as u32,
        }
    }
}

impl From<health::VersionCheckResponse> for VersionCheckResponse {
    fn from(from: health::VersionCheckResponse) -> Self {
        Self {
            grpc_version: from.grpc_version,
            agent_version: from.agent_version,
        }
    }
}

impl From<agent::Metrics> for MetricsResponse {
    fn from(from: Metrics) -> Self {
        Self {
            metrics: from.metrics,
        }
    }
}

impl From<agent::OOMEvent> for OomEventResponse {
    fn from(from: OOMEvent) -> Self {
        Self {
            container_id: from.container_id,
        }
    }
}

impl From<VolumeStatsRequest> for agent::VolumeStatsRequest {
    fn from(from: VolumeStatsRequest) -> Self {
        Self {
            volume_guest_path: from.volume_guest_path,
            ..Default::default()
        }
    }
}

impl From<csi::VolumeStatsResponse> for VolumeStatsResponse {
    fn from(from: csi::VolumeStatsResponse) -> Self {
        let result: String = format!(
            "Usage: {:?} Volume Condition: {:?}",
            from.usage(),
            from.volume_condition()
        );
        Self { data: result }
    }
}

impl From<ResizeVolumeRequest> for agent::ResizeVolumeRequest {
    fn from(from: ResizeVolumeRequest) -> Self {
        Self {
            volume_guest_path: from.volume_guest_path,
            size: from.size,
            ..Default::default()
        }
    }
}
