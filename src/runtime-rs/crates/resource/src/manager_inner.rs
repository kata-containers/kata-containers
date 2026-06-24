// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, sync::Arc, thread};

use agent::{types::Device, ARPNeighbor, Agent, OnlineCPUMemRequest, Storage};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_device_info, DeviceManager},
        util::{get_host_path, DEVICE_TYPE_BLOCK, DEVICE_TYPE_CHAR},
        DeviceConfig, DeviceType,
    },
    utils::uses_native_ccw_bus,
    BlockConfig, BlockDeviceAio, Hypervisor, VfioConfig,
};
use kata_types::mount::{kata_guest_sandbox_dir, Mount, KATA_EPHEMERAL_VOLUME_TYPE, SHM_DIR};
use kata_types::{
    config::{hypervisor::TopologyConfigInfo, TomlConfig},
    mount::{adjust_rootfs_mounts, KATA_IMAGE_FORCE_GUEST_PULL},
};
use libc::NUD_PERMANENT;
use oci::{Linux, LinuxCpu, LinuxResources};
use oci_spec::runtime::{self as oci, LinuxDeviceType};
use persist::sandbox_persist::Persist;
use std::path::PathBuf;
use tokio::{runtime, sync::RwLock};

use crate::{
    cdi_devices::{sort_options_by_pcipath, ContainerDevice, DeviceInfo},
    cgroups::{CgroupArgs, CgroupsResource},
    cpu_mem::{
        cpu::CpuResource, initial_size::InitialSizeManager, mem::MemResource, swap::SwapResource,
    },
    manager::ManagerArgs,
    network::{self, dan_config_path, Network, NetworkConfig, NetworkWithNetNsConfig},
    resource_persist::ResourceState,
    rootfs::{RootFsResource, Rootfs},
    share_fs::{self, sandbox_bind_mounts::SandboxBindMounts, NydusShareFs, ShareFs},
    volume::{utils::is_block_device_readonly, Volume, VolumeResource},
    ResourceConfig, ResourceUpdateOp,
};

pub(crate) struct ResourceManagerInner {
    sid: String,
    toml_config: Arc<TomlConfig>,
    agent: Arc<dyn Agent>,
    hypervisor: Arc<dyn Hypervisor>,
    device_manager: Arc<RwLock<DeviceManager>>,
    network: Option<Arc<dyn Network>>,
    share_fs: Option<Arc<dyn ShareFs>>,
    nydus_share_fs: Option<Arc<dyn NydusShareFs>>,

    pub rootfs_resource: RootFsResource,
    pub volume_resource: VolumeResource,
    pub cgroups_resource: CgroupsResource,
    pub cpu_resource: CpuResource,
    pub mem_resource: MemResource,
    pub swap_resource: Option<SwapResource>,
}

impl ResourceManagerInner {
    pub(crate) async fn new(
        sid: &str,
        agent: Arc<dyn Agent>,
        hypervisor: Arc<dyn Hypervisor>,
        toml_config: Arc<TomlConfig>,
        init_size_manager: InitialSizeManager,
    ) -> Result<Self> {
        let topo_config = TopologyConfigInfo::new(&toml_config);
        // create device manager
        let dev_manager = DeviceManager::new(hypervisor.clone(), topo_config.as_ref())
            .await
            .context("failed to create device manager")?;
        let device_manager = Arc::new(RwLock::new(dev_manager));

        let cgroups_resource = CgroupsResource::new(sid, &toml_config)?;
        let cpu_resource = CpuResource::new(toml_config.clone())?;
        let mem_resource = MemResource::new(init_size_manager)?;
        let swap_resource = if hypervisor
            .hypervisor_config()
            .await
            .memory_info
            .enable_guest_swap
        {
            let mut path = PathBuf::from(
                hypervisor
                    .hypervisor_config()
                    .await
                    .memory_info
                    .guest_swap_path,
            );
            path.push(sid);
            Some(
                SwapResource::new(
                    path,
                    hypervisor
                        .hypervisor_config()
                        .await
                        .memory_info
                        .guest_swap_size_percent,
                    hypervisor
                        .hypervisor_config()
                        .await
                        .memory_info
                        .guest_swap_create_threshold_secs,
                    mem_resource.clone(),
                    agent.clone(),
                    device_manager.clone(),
                )
                .await?,
            )
        } else {
            None
        };
        Ok(Self {
            sid: sid.to_string(),
            toml_config,
            agent,
            hypervisor,
            device_manager,
            network: None,
            share_fs: None,
            nydus_share_fs: None,
            rootfs_resource: RootFsResource::new(),
            volume_resource: VolumeResource::new(),
            cgroups_resource,
            cpu_resource,
            mem_resource,
            swap_resource,
        })
    }

    pub fn config(&self) -> Arc<TomlConfig> {
        self.toml_config.clone()
    }

    pub fn get_device_manager(&self) -> Arc<RwLock<DeviceManager>> {
        self.device_manager.clone()
    }

    pub async fn prepare_before_start_vm(
        &mut self,
        device_configs: Vec<ResourceConfig>,
    ) -> Result<()> {
        for dc in device_configs {
            match dc {
                ResourceConfig::ShareFs(c) => {
                    if self
                        .hypervisor
                        .capabilities()
                        .await?
                        .is_fs_sharing_supported()
                    {
                        let instance = share_fs::new(&self.sid, &c).context("new share fs")?;
                        instance
                            .share_fs
                            .setup_device_before_start_vm(
                                self.hypervisor.as_ref(),
                                &self.device_manager,
                            )
                            .await
                            .context("setup share fs device before start vm")?;

                        // setup sandbox bind mounts: setup = true
                        self.handle_sandbox_bindmounts(true)
                            .await
                            .context("failed setup sandbox bindmounts")?;

                        self.share_fs = Some(instance.share_fs);
                        self.nydus_share_fs = instance.nydus_share_fs;
                    };
                }
                ResourceConfig::Network(c) => {
                    self.handle_network(c)
                        .await
                        .context("failed to handle network")?;
                }
                ResourceConfig::VmRootfs(r) => {
                    do_handle_device(&self.device_manager, &DeviceConfig::BlockCfg(r))
                        .await
                        .context("do handle device failed.")?;
                }
                ResourceConfig::HybridVsock(hv) => {
                    do_handle_device(&self.device_manager, &DeviceConfig::HybridVsockCfg(hv))
                        .await
                        .context("do handle hybrid-vsock device failed.")?;
                }
                ResourceConfig::Vsock(v) => {
                    do_handle_device(&self.device_manager, &DeviceConfig::VsockCfg(v))
                        .await
                        .context("do handle vsock device failed.")?;
                }
                ResourceConfig::Protection(p) => {
                    do_handle_device(&self.device_manager, &DeviceConfig::ProtectionDevCfg(p))
                        .await
                        .context("do handle protection device failed.")?;
                }
                ResourceConfig::PortDevice(pd) => {
                    do_handle_device(
                        &self.device_manager,
                        &DeviceConfig::PortDeviceCfg(pd.clone()),
                    )
                    .await
                    .context("do handle port device failed.")?;
                }
                ResourceConfig::InitData(id) => {
                    do_handle_device(&self.device_manager, &DeviceConfig::BlockCfg(id))
                        .await
                        .context("do handle initdata block device failed.")?;
                }
                ResourceConfig::VfioDeviceModern(vfiobase) => {
                    do_handle_device(&self.device_manager, &DeviceConfig::VfioModernCfg(vfiobase))
                        .await
                        .context("do handle vfio device failed.")?;
                }
            };
        }

        // Under cgroup v2, tell the hypervisor which sandbox cgroup the VMM
        // must join at spawn time (before the guest boots) so the guest RAM
        // is charged to the pod cgroup. No-op for hypervisors that don't
        // implement spawn-time placement, and skipped for cgroup v1.
        if let Some(cgroup_procs_path) = self.cgroups_resource.sandbox_cgroup_procs_path().await {
            self.hypervisor
                .set_vmm_cgroup_path(cgroup_procs_path)
                .await
                .context("set vmm cgroup path before start vm")?;
        }

        Ok(())
    }

    pub async fn handle_network(&mut self, network_config: NetworkConfig) -> Result<()> {
        // 1. When using Rust asynchronous programming, we use .await to
        //    allow other task to run instead of waiting for the completion of the current task.
        // 2. Also, when handling the pod network, we need to set the shim threads
        //    into the network namespace to perform those operations.
        // However, as the increase of the I/O intensive tasks, two issues could be caused by the two points above:
        // a. When the future is blocked, the current thread (which is in the pod netns)
        //    might be take over by other tasks. After the future is finished, the thread take over
        //    the current task might not be in the pod netns. But the current task still need to run in pod netns
        // b. When finish setting up the network, the current thread will be set back to the host namespace.
        //    In Rust Async, if the current thread is taken over by other task, the netns is dropped on another thread,
        //    but it is not in netns. So, the previous thread would still remain in the pod netns.
        // The solution is to block the future on the current thread, it is enabled by spawn an os thread, create a
        // tokio runtime, and block the task on it.
        let device_manager = self.device_manager.clone();
        let network = thread::spawn(move || -> Result<Arc<dyn Network>> {
            let rt = runtime::Builder::new_current_thread().enable_io().build()?;
            let d = rt
                .block_on(network::new(&network_config, device_manager))
                .context("new network")?;
            rt.block_on(d.setup()).context("setup network")?;
            Ok(d)
        })
        .join()
        .map_err(|e| anyhow!("{:?}", e))
        .context("Couldn't join on the associated thread")?
        .context("failed to set up network")?;
        self.network = Some(network);
        Ok(())
    }

    async fn handle_interfaces(&self, network: &dyn Network) -> Result<()> {
        for i in network.interfaces().await.context("get interfaces")? {
            info!(sl!(), "update interface {:?}", i);

            // After hotplugging a network device, the guest kernel needs time
            // to probe it before the interface appears.  This is especially
            // pronounced on s390x (CCW bus) but can also happen on x86 in
            // slower CI environments.  Retry a few times.
            let mut last_error = None;
            for attempt in 0..10u32 {
                match self
                    .agent
                    .update_interface(agent::UpdateInterfaceRequest {
                        interface: Some(i.clone()),
                    })
                    .await
                {
                    core::result::Result::Ok(_) => {
                        last_error = None;
                        break;
                    }
                    core::result::Result::Err(e) => {
                        debug!(
                            sl!(),
                            "update_interface attempt {} failed, retrying: {:?}",
                            attempt + 1,
                            e
                        );
                        last_error = Some(e);
                        if attempt < 9 {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                }
            }
            if let Some(err) = last_error {
                return Err(err).context("update interface");
            }
        }

        Ok(())
    }

    async fn handle_neighbours(&self, network: &dyn Network) -> Result<()> {
        let all_neighbors = network.neighs().await.context("neighs")?;

        // We add only static ARP entries
        let neighbors: Vec<ARPNeighbor> = all_neighbors
            .iter()
            .filter(|n| n.state == NUD_PERMANENT as i32)
            .cloned()
            .collect();
        if !neighbors.is_empty() {
            info!(sl!(), "update neighbors {:?}", neighbors);
            self.agent
                .add_arp_neighbors(agent::AddArpNeighborRequest {
                    neighbors: Some(agent::ARPNeighbors { neighbors }),
                })
                .await
                .context("update neighbors")?;
        }
        Ok(())
    }

    async fn handle_routes(&self, network: &dyn Network) -> Result<()> {
        let routes = network.routes().await.context("routes")?;
        if !routes.is_empty() {
            info!(sl!(), "update routes {:?}", routes);
            self.agent
                .update_routes(agent::UpdateRoutesRequest {
                    route: Some(agent::Routes { routes }),
                })
                .await
                .context("update routes")?;
        }
        Ok(())
    }

    pub async fn setup_after_start_vm(&mut self) -> Result<()> {
        self.cgroups_resource
            .setup_after_start_vm(self.hypervisor.as_ref())
            .await
            .context("setup cgroups after start vm")?;

        if let Some(share_fs) = self.share_fs.as_ref() {
            share_fs
                .setup_device_after_start_vm(self.hypervisor.as_ref(), &self.device_manager)
                .await
                .context("setup share fs device after start vm")?;
        }

        if let Some(network) = self.network.as_ref() {
            // For cold-plugged physical-endpoint VFs, the PCIe topology
            // pre-computes a wrong path because the root port has no explicit
            // addr and QEMU auto-assigns its slot.  Resolve the actual path
            // via QMP (query-pci + device search) before sending
            // update_interface to the agent.
            resolve_physical_endpoint_pci_paths(network.as_ref(), self.hypervisor.as_ref()).await;

            self.apply_network_to_agent(network.as_ref()).await?;
        }

        if let Some(swap) = self.swap_resource.as_ref() {
            swap.update().await;
        }

        Ok(())
    }

    pub async fn apply_network_to_agent(&self, network: &dyn Network) -> Result<()> {
        self.handle_interfaces(network)
            .await
            .context("handle interfaces")?;
        self.handle_neighbours(network)
            .await
            .context("handle neighbors")?;
        self.handle_routes(network).await.context("handle routes")?;
        Ok(())
    }

    /// Check whether a rescan is needed at all (early-out conditions).
    pub fn rescan_should_skip(&self, net_cfg: &NetworkWithNetNsConfig) -> bool {
        self.toml_config.runtime.disable_new_netns
            || net_cfg.network_model == "none"
            || net_cfg.netns_path.is_empty()
            || dan_config_path(&self.toml_config, &self.sid).exists()
    }

    /// Check whether the network already has interfaces configured.
    pub async fn network_has_interfaces(&self) -> Result<bool> {
        match self.network.as_ref() {
            Some(n) => Ok(!n
                .interfaces()
                .await
                .context("check existing interfaces")?
                .is_empty()),
            None => Ok(false),
        }
    }

    /// Perform a single network scan attempt.  Returns `Some(network)` when
    /// new interfaces were found and need to be applied to the guest agent,
    /// `None` when no interfaces were found yet (caller should retry).
    /// The caller is responsible for calling `apply_network_to_agent` on
    /// the returned network **after** releasing the write lock.
    pub async fn rescan_network_once(
        &mut self,
        net_cfg: NetworkWithNetNsConfig,
    ) -> Result<Option<Arc<dyn Network>>> {
        self.handle_network(NetworkConfig::NetNs(net_cfg))
            .await
            .context("rescan handle network")?;

        let n = self
            .network
            .as_ref()
            .ok_or_else(|| anyhow!("network missing after rescan setup"))?;
        let ifs = n.interfaces().await.context("rescan get interfaces")?;
        if !ifs.is_empty() {
            return Ok(Some(Arc::clone(n)));
        }
        Ok(None)
    }

    pub async fn get_storage_for_sandbox(&self, shm_size: u64) -> Result<Vec<Storage>> {
        let mut storages = vec![];
        if let Some(d) = self.share_fs.as_ref() {
            let mut s = d.get_storages().await.context("get storage")?;
            storages.append(&mut s);
        }

        let shm_size_option = format!("size={shm_size}");
        let mount_point = format!("{}/{}", kata_guest_sandbox_dir(), SHM_DIR);

        let shm_storage = Storage {
            driver: KATA_EPHEMERAL_VOLUME_TYPE.to_string(),
            mount_point,
            source: "shm".to_string(),
            fs_type: "tmpfs".to_string(),
            options: vec![
                "noexec".to_string(),
                "nosuid".to_string(),
                "nodev".to_string(),
                "mode=1777".to_string(),
                shm_size_option,
            ],
            ..Default::default()
        };

        storages.push(shm_storage);

        Ok(storages)
    }

    pub async fn handler_rootfs(
        &self,
        cid: &str,
        root: &oci::Root,
        bundle_path: &str,
        rootfs_mounts: &[Mount],
        annotations: &HashMap<String, String>,
    ) -> Result<Arc<dyn Rootfs>> {
        let adjust_rootfs_mounts = if !self
            .config()
            .runtime
            .is_experiment_enabled(KATA_IMAGE_FORCE_GUEST_PULL)
        {
            rootfs_mounts.to_vec()
        } else {
            adjust_rootfs_mounts()?
        };

        self.rootfs_resource
            .handler_rootfs(
                &self.share_fs,
                &self.nydus_share_fs,
                self.device_manager.as_ref(),
                self.hypervisor.as_ref(),
                &self.sid,
                cid,
                root,
                bundle_path,
                &adjust_rootfs_mounts,
                annotations,
            )
            .await
    }

    pub async fn handler_volumes(
        &self,
        cid: &str,
        spec: &oci::Spec,
    ) -> Result<Vec<Arc<dyn Volume>>> {
        let ctx = crate::volume::VolumeContext {
            share_fs: &self.share_fs,
            d: self.device_manager.as_ref(),
            sid: &self.sid,
            agent: self.agent.clone(),
            emptydir_mode: &self.toml_config.runtime.emptydir_mode,
        };
        self.volume_resource.handler_volumes(&ctx, cid, spec).await
    }

    pub async fn handler_devices(&self, _cid: &str, linux: &Linux) -> Result<Vec<ContainerDevice>> {
        let mut devices = vec![];

        // Build a map of host_bdf -> Option<guest_pci_path> for cold-plugged
        // physical (VFIO) network endpoints.  When a VFIO char device in the
        // OCI spec belongs to one of these endpoints we bypass the
        // do_handle_device hot-plug path (the device is already in QEMU) and
        // build the ContainerDevice directly, mirroring Go's appendVfioDevice.
        // This also triggers the agent's container_has_vfio_device() gate,
        // which drives the guest-kernel VFIO network device setup (Ethernet,
        // RoCE and InfiniBand) — including, for IB/RoCE VFs,
        // expose_guest_infiniband_devices() injecting /dev/infiniband/* into
        // the container.
        //
        // IMPORTANT: every physical-endpoint BDF is recorded here regardless of
        // whether its guest PCI path has been resolved yet (the QMP resolution
        // in setup_after_start_vm can fail or be racy).  The decision to skip
        // do_handle_device must depend only on the device being a cold-plugged
        // endpoint — never on the guest PCI path being known — otherwise an
        // unresolved path would send the already-cold-plugged VF down the
        // hot-plug path and fail with ENOENT.
        let mut cold_plug_bdfs: std::collections::HashMap<String, Option<String>> =
            std::collections::HashMap::new();
        if let Some(network) = &self.network {
            for endpoint in network.endpoints().await {
                if let Some(bdf) = endpoint.host_bdf().await {
                    let path = endpoint.guest_pci_path().await;
                    cold_plug_bdfs.insert(bdf, path);
                }
            }
        }

        let linux_devices = linux.devices().clone().unwrap_or_default();
        for d in linux_devices.iter() {
            match d.typ() {
                LinuxDeviceType::B => {
                    let blkdev_info = get_block_device_info(&self.device_manager).await;
                    // Read-only intent comes from the cgroup device access rule.
                    // Also honor the host device's own read-only flag (BLKROGET):
                    // block-mode volumes frequently carry no read-only signal in
                    // the OCI spec, so the device flag is the only reliable
                    // source. Either signal being positive marks it read-only.
                    let is_readonly = device_cgroup_access_is_readonly(
                        linux,
                        LinuxDeviceType::B,
                        d.major(),
                        d.minor(),
                    ) || block_device_node_is_readonly(d.major(), d.minor());
                    let dev_info = DeviceConfig::BlockCfg(BlockConfig {
                        major: d.major(),
                        minor: d.minor(),
                        is_readonly,
                        driver_option: blkdev_info.block_device_driver,
                        blkdev_aio: BlockDeviceAio::new(&blkdev_info.block_device_aio),
                        num_queues: blkdev_info.num_queues,
                        queue_size: blkdev_info.queue_size,
                        logical_sector_size: blkdev_info.block_device_logical_sector_size,
                        physical_sector_size: blkdev_info.block_device_physical_sector_size,
                        ..Default::default()
                    });

                    let device_info = do_handle_device(&self.device_manager, &dev_info)
                        .await
                        .context("do handle device")?;

                    // create block device for kata agent.
                    // The device ID is derived from the available address: PCI, SCSI,
                    // CCW, or virtual path, depending on the driver and configuration.
                    if let DeviceType::Block(device) = device_info {
                        let id = if let Some(pci_path) = device.config.pci_path {
                            pci_path.to_string()
                        } else if let Some(scsi_address) = device.config.scsi_addr {
                            scsi_address
                        } else if let Some(ccw_addr) = device.config.ccw_addr {
                            ccw_addr
                        } else {
                            device.config.virt_path.clone()
                        };

                        let agent_device = Device {
                            id,
                            container_path: d.path().display().to_string().clone(),
                            field_type: device.config.driver_option,
                            vm_path: device.config.virt_path,
                            ..Default::default()
                        };
                        devices.push(ContainerDevice {
                            device_info: None,
                            device: agent_device,
                        });
                    }
                }
                LinuxDeviceType::C => {
                    let host_path = get_host_path(DEVICE_TYPE_CHAR, d.major(), d.minor())
                        .context("get host path failed")?;
                    // First of all, filter vfio devices.
                    if !host_path.starts_with("/dev/vfio") {
                        continue;
                    }

                    // `/dev/vfio/vfio` is the legacy VFIO container control
                    // node, not a passthrough device — it has no IOMMU group
                    // and no BDF.  The SR-IOV device plugin lists it alongside
                    // the VF group node; running do_handle_device on it fails
                    // with ENOENT.  Skip it unconditionally.
                    if host_path == "/dev/vfio/vfio" {
                        continue;
                    }

                    let vfio_mode = match self.toml_config.runtime.vfio_mode.as_str() {
                        "vfio" => "vfio-pci",
                        _ => "vfio-pci-gk",
                    };

                    // If this VFIO char device belongs to a cold-plugged
                    // physical endpoint, it is already present in QEMU.  We
                    // MUST NOT call do_handle_device on it — that would try to
                    // hot-plug an already-present device and fail with ENOENT.
                    //
                    // Match on the host BDF(s) the device exposes (resolved via
                    // its IOMMU group / iommufd cdev) against the physical
                    // endpoint set.  vfio_path_to_bdfs returns *every* BDF in
                    // the group so a legacy multi-device group is matched
                    // deterministically.  The skip depends only on BDF
                    // membership, never on the guest PCI path being resolved.
                    let matched_bdf = vfio_path_to_bdfs(&host_path)
                        .into_iter()
                        .find(|bdf| cold_plug_bdfs.contains_key(bdf));
                    if let Some(host_bdf) = matched_bdf {
                        // unwrap: contains_key above guarantees presence
                        let maybe_guest_path = cold_plug_bdfs.get(&host_bdf).unwrap();
                        if let Some(guest_pci_path) = maybe_guest_path {
                            let container_path = d.path().display().to_string();
                            let group_num = d
                                .path()
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or_default()
                                .to_string();
                            let agent_device = Device {
                                id: group_num,
                                container_path,
                                field_type: vfio_mode.to_string(),
                                options: vec![format!("{}={}", host_bdf, guest_pci_path)],
                                ..Default::default()
                            };
                            devices.push(ContainerDevice {
                                device_info: None,
                                device: agent_device,
                            });
                        } else {
                            warn!(
                                sl!(),
                                "handler_devices: cold-plug VFIO device has no \
                                 resolved guest PCI path, skipping agent device entry";
                                "host_bdf" => &host_bdf,
                            );
                        }
                        // Always skip do_handle_device for cold-plugged devices.
                        continue;
                    }

                    let bus_type = if uses_native_ccw_bus() {
                        "ccw".to_string()
                    } else {
                        "pci".to_string()
                    };
                    let dev_info = DeviceConfig::VfioCfg(VfioConfig {
                        host_path,
                        dev_type: "c".to_string(),
                        bus_type: bus_type.clone(),
                        hostdev_prefix: "vfio_device".to_owned(),
                        ..Default::default()
                    });

                    let device_info = do_handle_device(&self.device_manager.clone(), &dev_info)
                        .await
                        .context("do handle device")?;

                    if let DeviceType::VfioModern(vfio_dev) = device_info.clone() {
                        info!(sl!(), "device info: {:?}", vfio_dev.lock().await);
                        let vfio_device = vfio_dev.lock().await;
                        let guest_pci_path = vfio_device
                            .config
                            .guest_pci_path
                            .clone()
                            .context("VFIO device has no guest PCI path assigned")?;
                        let host_bdf = vfio_device.device.primary.addr.to_string();
                        info!(
                            sl!(),
                            "vfio device guest pci path: {:?}, host bdf: {:?}",
                            guest_pci_path,
                            &host_bdf
                        );

                        // vfio mode: vfio-pci and vfio-pci-gk for x86_64
                        // - vfio-pci, devices appear as VFIO character devices under /dev/vfio in container.
                        // - vfio-pci-gk, devices are managed by whatever driver in Guest kernel.
                        // - vfio-ap, devices appear as VFIO character devices under /dev/vfio in container for ccw devices.
                        let vfio_mode = match self.toml_config.runtime.vfio_mode.as_str() {
                            "vfio" => {
                                if bus_type == "ccw" {
                                    "vfio-ap".to_string()
                                } else {
                                    "vfio-pci".to_string()
                                }
                            }
                            _ => "vfio-pci-gk".to_string(),
                        };
                        let device_options = vec![format!("{}={}", host_bdf, guest_pci_path)];
                        // The Go runtime sets the device Id to
                        // filepath.Base(dev.ContainerPath), e.g. "vfio0".
                        // The agent policy validates this with:
                        //   i_vfio_device.id == concat("", ["vfio", suffix])
                        let group_num = d
                            .path()
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or_default()
                            .to_string();
                        let agent_device = Device {
                            id: group_num,
                            container_path: d.path().display().to_string().clone(),
                            field_type: vfio_mode,
                            options: device_options,
                            ..Default::default()
                        };

                        let device_info = Some(DeviceInfo {
                            vendor_id: vfio_device
                                .device
                                .primary
                                .vendor_id
                                .clone()
                                .unwrap_or_default(),
                            class_id: format!(
                                "{:#08x}",
                                vfio_device.device.primary.class_code.unwrap_or_default()
                            ),
                            host_path: d.path().clone(),
                        });
                        info!(
                            sl!(),
                            "vfio device info for agent: {:?}",
                            device_info.clone()
                        );
                        info!(
                            sl!(),
                            "agent device info for agent: {:?}",
                            agent_device.clone()
                        );
                        devices.push(ContainerDevice {
                            device_info,
                            device: agent_device,
                        });
                    } else {
                        // vfio mode: vfio-pci and vfio-pci-gk for x86_64
                        // - vfio-pci, devices appear as VFIO character devices under /dev/vfio in container.
                        // - vfio-pci-gk, devices are managed by whatever driver in Guest kernel.
                        // - vfio-ap, devices appear as VFIO character devices under /dev/vfio in container for ccw devices.
                        let vfio_mode = match self.toml_config.runtime.vfio_mode.as_str() {
                            "vfio" => {
                                if bus_type == "ccw" {
                                    "vfio-ap".to_string()
                                } else {
                                    "vfio-pci".to_string()
                                }
                            }
                            _ => "vfio-pci-gk".to_string(),
                        };

                        // create agent device
                        if let DeviceType::Vfio(device) = device_info {
                            let device_options = sort_options_by_pcipath(device.device_options);
                            let group_num = d
                                .path()
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or_default()
                                .to_string();
                            let agent_device = Device {
                                id: group_num,
                                container_path: d.path().display().to_string().clone(),
                                field_type: vfio_mode,
                                options: device_options,
                                ..Default::default()
                            };

                            let device_info = if let Some(device_vendor_class) =
                                &device.devices.first().unwrap().device_vendor_class
                            {
                                let vendor_class = device_vendor_class
                                    .get_vendor_class_id()
                                    .context("get vendor class failed")?;
                                Some(DeviceInfo {
                                    vendor_id: vendor_class.0.to_owned(),
                                    class_id: vendor_class.1.to_owned(),
                                    host_path: d.path().clone(),
                                })
                            } else {
                                None
                            };
                            devices.push(ContainerDevice {
                                device_info,
                                device: agent_device,
                            });
                        }
                    }
                }
                _ => {
                    // TODO enable other devices type
                    continue;
                }
            }
        }

        // The SR-IOV device plugin for physical network VFs injects the BDF
        // as a PCIDEVICE_* env var only — it does not add the VFIO char device
        // to linux.devices.  That means the LinuxDeviceType::C loop above
        // never fires for these endpoints, the cold_plug_bdfs map was built
        // but never consumed, and the agent's container_has_vfio_device() gate
        // stays closed (so no guest-kernel VFIO network device setup runs).
        //
        // For every cold-plug endpoint still unmatched (guest PCI path in the
        // map but no corresponding device pushed above), derive the VFIO group
        // char path from sysfs and synthesise a vfio-pci-gk ContainerDevice,
        // mirroring what the Go runtime does in appendPhysicalEndpointDevices.
        let seen_bdfs: std::collections::HashSet<String> = devices
            .iter()
            .filter_map(|cd| {
                cd.device.options.first().and_then(|opt| {
                    let bdf_part = opt.split('=').next()?;
                    // strip leading "0000:" to get "BB:DD.F"
                    let stripped = bdf_part.trim_start_matches("0000:");
                    Some(format!("0000:{}", stripped))
                })
            })
            .collect();

        let vfio_mode = match self.toml_config.runtime.vfio_mode.as_str() {
            "vfio" => "vfio-pci",
            _ => "vfio-pci-gk",
        };

        for (host_bdf, maybe_guest_path) in &cold_plug_bdfs {
            if seen_bdfs.contains(host_bdf) {
                continue;
            }
            let guest_pci_path = match maybe_guest_path {
                Some(p) => p,
                None => {
                    warn!(
                        sl!(),
                        "handler_devices: cold-plug physical endpoint has no resolved \
                         guest PCI path, skipping VFIO device exposure";
                        "host_bdf" => host_bdf,
                    );
                    continue;
                }
            };
            if let Some(vfio_group_path) = bdf_to_vfio_group_path(host_bdf) {
                let group_num = std::path::Path::new(&vfio_group_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string();
                info!(
                    sl!(),
                    "handler_devices: injecting vfio-pci-gk entry for cold-plug \
                     physical endpoint";
                    "host_bdf" => host_bdf,
                    "guest_pci_path" => guest_pci_path,
                    "vfio_group" => &vfio_group_path,
                );
                let agent_device = Device {
                    id: group_num,
                    container_path: vfio_group_path,
                    field_type: vfio_mode.to_string(),
                    options: vec![format!("{}={}", host_bdf, guest_pci_path)],
                    ..Default::default()
                };
                devices.push(ContainerDevice {
                    device_info: None,
                    device: agent_device,
                });
            } else {
                warn!(
                    sl!(),
                    "handler_devices: cannot resolve VFIO group for {}, skipping VFIO device exposure",
                    host_bdf
                );
            }
        }

        Ok(devices)
    }

    async fn handle_sandbox_bindmounts(&self, setup: bool) -> Result<()> {
        let bindmounts = self.toml_config.runtime.sandbox_bind_mounts.clone();
        if bindmounts.is_empty() {
            info!(sl!(), "sandbox bindmounts empty, just skip it.");
            return Ok(());
        }

        let sb_bindmnt = SandboxBindMounts::new(self.sid.clone(), bindmounts)?;

        if setup {
            sb_bindmnt.setup_sandbox_bind_mounts()
        } else {
            sb_bindmnt.cleanup_sandbox_bind_mounts()
        }
    }

    pub async fn cleanup(&self) -> Result<()> {
        // detach network endpoints (rebinds VFs from vfio-pci back to host driver)
        if let Some(network) = &self.network {
            if let Err(err) = network.remove(self.hypervisor.as_ref()).await {
                warn!(sl!(), "failed to remove network: {}", err);
            }
        }

        // clean up cgroup
        self.cgroups_resource
            .delete()
            .await
            .context("delete cgroup")?;

        // cleanup sandbox bind mounts: setup = false
        self.handle_sandbox_bindmounts(false)
            .await
            .context("failed to cleanup sandbox bindmounts")?;

        // stop share fs daemon (e.g., virtiofsd, nydusd) before cleaning up mount
        if let Some(share_fs) = &self.share_fs {
            share_fs
                .stop()
                .await
                .context("failed to stop share fs daemon")?;
        }

        // clean up share fs mount
        if let Some(share_fs) = &self.share_fs {
            share_fs
                .get_share_fs_mount()
                .cleanup(&self.sid)
                .await
                .context("failed to cleanup host path")?;
        }

        if let Some(swap) = self.swap_resource.as_ref() {
            swap.clean().await;
        }

        self.volume_resource
            .cleanup_ephemeral_disks()
            .await
            .context("failed to cleanup ephemeral disks")?;

        Ok(())
    }

    pub async fn dump(&self) {
        self.rootfs_resource.dump().await;
        self.volume_resource.dump().await;
    }

    pub async fn update_linux_resource(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
        op: ResourceUpdateOp,
    ) -> Result<Option<LinuxResources>> {
        let linux_cpus = || -> Option<&LinuxCpu> { linux_resources.as_ref()?.cpu().as_ref() }();

        // if static_sandbox_resource_mgmt, we will not have to update sandbox's cpu or mem resource
        if !self.toml_config.runtime.static_sandbox_resource_mgmt {
            // update cpu
            self.cpu_resource
                .update_cpu_resources(cid, linux_cpus, op, self.hypervisor.as_ref())
                .await?;
            // update memory
            self.mem_resource
                .update_mem_resources(cid, linux_resources, op, self.hypervisor.as_ref())
                .await?;

            self.agent
                .online_cpu_mem(OnlineCPUMemRequest {
                    wait: false,
                    nb_cpus: self.cpu_resource.current_vcpu().await.ceil() as u32,
                    cpu_only: false,
                })
                .await
                .context("online vcpus")?;
        }

        // we should firstly update the vcpus and mems, and then update the host cgroups
        self.cgroups_resource
            .update(cid, linux_resources, op, self.hypervisor.as_ref())
            .await?;

        if let Some(swap) = self.swap_resource.as_ref() {
            swap.update().await;
        }

        // update the linux resources for agent
        self.agent_linux_resources(linux_resources)
    }

    fn agent_linux_resources(
        &self,
        linux_resources: Option<&LinuxResources>,
    ) -> Result<Option<LinuxResources>> {
        let mut resources = match linux_resources {
            Some(linux_resources) => linux_resources.clone(),
            None => {
                return Ok(None);
            }
        };

        // clear the cpuset
        // for example, if there are only 5 vcpus now, and the cpuset in LinuxResources is 0-2,6, guest os will report
        // error when creating the container. so we choose to clear the cpuset here.
        if let Some(cpu) = &mut resources.cpu_mut() {
            cpu.set_cpus(None);
        }

        Ok(Some(resources))
    }
}

#[async_trait]
impl Persist for ResourceManagerInner {
    type State = ResourceState;
    type ConstructorArgs = ManagerArgs;

    /// Save a state of ResourceManagerInner
    async fn save(&self) -> Result<Self::State> {
        let mut endpoint_state = vec![];
        if let Some(network) = &self.network {
            if let Some(ens) = network.save().await {
                endpoint_state = ens;
            }
        }
        let cgroup_state = self.cgroups_resource.save().await?;
        Ok(ResourceState {
            endpoint: endpoint_state,
            cgroup_state: Some(cgroup_state),
        })
    }

    /// Restore ResourceManagerInner
    async fn restore(
        resource_args: Self::ConstructorArgs,
        resource_state: Self::State,
    ) -> Result<Self> {
        let args = CgroupArgs {
            sid: resource_args.sid.clone(),
            config: resource_args.config,
        };
        let topo_config = TopologyConfigInfo::new(&args.config);

        let mem_resource = MemResource::default();
        let device_manager = Arc::new(RwLock::new(
            DeviceManager::new(resource_args.hypervisor.clone(), topo_config.as_ref()).await?,
        ));

        let swap_resource = if resource_args
            .hypervisor
            .hypervisor_config()
            .await
            .memory_info
            .enable_guest_swap
        {
            let mut path = PathBuf::from(
                resource_args
                    .hypervisor
                    .hypervisor_config()
                    .await
                    .memory_info
                    .guest_swap_path,
            );
            path.push(resource_args.sid.clone());
            Some(SwapResource::restore(path).await)
        } else {
            None
        };

        Ok(Self {
            sid: resource_args.sid,
            agent: resource_args.agent,
            hypervisor: resource_args.hypervisor,
            device_manager,
            network: None,
            share_fs: None,
            nydus_share_fs: None,
            rootfs_resource: RootFsResource::new(),
            volume_resource: VolumeResource::new(),
            cgroups_resource: CgroupsResource::restore(
                args,
                resource_state.cgroup_state.unwrap_or_default(),
            )
            .await?,
            toml_config: Arc::new(TomlConfig::default()),
            cpu_resource: CpuResource::default(),
            mem_resource,
            swap_resource,
        })
    }
}

/// For each physical-endpoint VF in the network, resolve the actual in-guest
/// PCIe path via QMP (query-pci) and update the endpoint's `guest_pci_path`.
///
/// This must be called after the VM has started (QMP is initialised) and
/// before `apply_network_to_agent`, because the PCIe topology pre-computes
/// a wrong path (root port has no explicit addr → QEMU auto-assigns its slot;
/// only QMP can reveal the actual assignment).
/// Map a PCI BDF (e.g. `"0000:06:02.2"`) to the VFIO group char device path
/// (e.g. `"/dev/vfio/343"`).  Returns `None` if sysfs cannot be read.
fn bdf_to_vfio_group_path(bdf: &str) -> Option<String> {
    // /sys/bus/pci/devices/<bdf>/iommu_group is a symlink like
    // ../../kernel/iommu_groups/343  — the basename is the group number.
    let iommu_link = format!("/sys/bus/pci/devices/{}/iommu_group", bdf);
    let target = std::fs::read_link(&iommu_link).ok()?;
    let group = target.file_name()?.to_str()?.to_string();
    // Verify the char device exists before returning.
    let vfio_path = format!("/dev/vfio/{}", group);
    if std::path::Path::new(&vfio_path).exists() {
        Some(vfio_path)
    } else {
        None
    }
}

/// Resolve a VFIO char device path to *all* host PCI BDFs it exposes.
///
/// Handles two formats:
/// - Legacy group interface: `/dev/vfio/343`
///   Reads `/sys/kernel/iommu_groups/343/devices/` and returns every BDF in
///   the group (a group may contain more than one device).
/// - iommufd cdev interface: `/dev/vfio/devices/vfio1`
///   Reads `/sys/class/vfio_device/vfio1/device` symlink to find the BDF.
///
/// Returns an empty vector if the path cannot be resolved.  Returning all
/// BDFs (rather than just the first) makes cold-plug endpoint matching
/// deterministic regardless of `read_dir` ordering.
fn vfio_path_to_bdfs(vfio_path: &str) -> Vec<String> {
    let file_name = match std::path::Path::new(vfio_path)
        .file_name()
        .and_then(|n| n.to_str())
    {
        Some(n) => n.to_string(),
        None => return vec![],
    };

    // iommufd cdev path: /dev/vfio/devices/vfioN — file name starts with "vfio"
    // and cannot be parsed as a plain integer.
    if file_name.parse::<u32>().is_err() {
        // /sys/class/vfio_device/<name>/device -> ../../../../bus/pci/devices/<bdf>
        let dev_link = format!("/sys/class/vfio_device/{}/device", file_name);
        match std::fs::read_link(&dev_link)
            .ok()
            .and_then(|t| t.file_name().and_then(|n| n.to_str()).map(String::from))
        {
            Some(bdf) => return vec![bdf],
            None => return vec![],
        }
    }

    // Legacy group path: /dev/vfio/N — return every device BDF in the group.
    let group = match file_name.parse::<u32>() {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    let sysfs = format!("/sys/kernel/iommu_groups/{}/devices", group);
    let entries = match std::fs::read_dir(&sysfs) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

async fn resolve_physical_endpoint_pci_paths(
    network: &dyn crate::network::Network,
    hypervisor: &dyn hypervisor::Hypervisor,
) {
    for endpoint in network.endpoints().await {
        if let Some(hostdev_id) = endpoint.vfio_hostdev_id().await {
            match hypervisor.resolve_vfio_device_pci_path(&hostdev_id).await {
                Ok(pci_path) => {
                    let path_str = pci_path.to_string();
                    info!(
                        sl!(),
                        "resolved physical endpoint guest PCI path: \
                         hostdev_id={} path={}",
                        hostdev_id,
                        path_str
                    );
                    endpoint.set_guest_pci_path(path_str).await;
                }
                Err(e) => {
                    warn!(
                        sl!(),
                        "failed to resolve guest PCI path for hostdev {}: {}", hostdev_id, e
                    );
                }
            }
        }
    }
}

/// Derive a device's read-only intent from the cgroup device access rules.
///
/// Block-mode volumes (e.g. Kubernetes volumeDevices) are passed as device
/// nodes in `spec.Linux.Devices` and carry no mount "ro" option; their
/// read-only intent is expressed solely through the cgroup device access in
/// `spec.Linux.Resources.Devices` ("rm" = read+mknod, no write, for read-only;
/// "rwm" for read-write).
///
/// The allow rule that exactly matches the device (type and exact major/minor)
/// decides: the device is read-only when that rule grants access without the
/// write ("w") bit. Wildcard rules (no major/minor) describe broad device
/// classes and are ignored so they cannot override a specific device's access.
/// If no exact rule matches, the device is left read-write.
fn device_cgroup_access_is_readonly(
    linux: &Linux,
    dev_type: LinuxDeviceType,
    major: i64,
    minor: i64,
) -> bool {
    let devices = match linux
        .resources()
        .as_ref()
        .and_then(|r| r.devices().as_ref())
    {
        Some(devices) => devices,
        None => return false,
    };

    for r in devices.iter() {
        if !r.allow() {
            continue;
        }
        let (rule_major, rule_minor) = match (r.major(), r.minor()) {
            (Some(major), Some(minor)) => (major, minor),
            _ => continue,
        };
        if rule_major != major || rule_minor != minor {
            continue;
        }
        // A specific type must match; `A` (all) and an unset type are wildcards.
        if let Some(typ) = r.typ() {
            if typ != LinuxDeviceType::A && typ != dev_type {
                continue;
            }
        }

        return !r.access().as_deref().unwrap_or("").contains('w');
    }

    false
}

/// block_device_node_is_readonly reports whether the host block device
/// identified by major:minor advertises the read-only flag (BLKROGET). This is
/// the ground truth for a device's writability: block-mode volumes frequently
/// carry no read-only signal in the OCI spec, so the device flag is the only
/// reliable source. Any failure is logged and treated as not-read-only so it
/// can never flip a positive signal back.
fn block_device_node_is_readonly(major: i64, minor: i64) -> bool {
    let host_path = match get_host_path(DEVICE_TYPE_BLOCK, major, minor) {
        Ok(path) if !path.is_empty() => path,
        Ok(_) => return false,
        Err(e) => {
            warn!(
                sl!(),
                "could not resolve host path for block device {}:{}: {:?}", major, minor, e
            );
            return false;
        }
    };

    is_block_device_readonly(&host_path).unwrap_or_else(|e| {
        warn!(
            sl!(),
            "could not query block device read-only flag for {}: {:?}", host_path, e
        );
        false
    })
}

#[cfg(test)]
mod tests {
    use super::device_cgroup_access_is_readonly;
    use oci_spec::runtime::{
        Linux, LinuxBuilder, LinuxDeviceCgroup, LinuxDeviceCgroupBuilder, LinuxDeviceType,
        LinuxResourcesBuilder,
    };
    use rstest::rstest;

    const MAJOR: i64 = 8;
    const MINOR: i64 = 0;

    fn rule(
        allow: bool,
        typ: LinuxDeviceType,
        major: Option<i64>,
        minor: Option<i64>,
        access: &str,
    ) -> LinuxDeviceCgroup {
        let mut builder = LinuxDeviceCgroupBuilder::default()
            .allow(allow)
            .typ(typ)
            .access(access);
        if let Some(major) = major {
            builder = builder.major(major);
        }
        if let Some(minor) = minor {
            builder = builder.minor(minor);
        }
        builder.build().unwrap()
    }

    fn linux_with_rules(rules: Vec<LinuxDeviceCgroup>) -> Linux {
        LinuxBuilder::default()
            .resources(
                LinuxResourcesBuilder::default()
                    .devices(rules)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap()
    }

    #[rstest]
    #[case::no_rules(vec![], false)]
    #[case::exact_match_rm(vec![rule(true, LinuxDeviceType::B, Some(MAJOR), Some(MINOR), "rm")], true)]
    #[case::exact_match_r(vec![rule(true, LinuxDeviceType::B, Some(MAJOR), Some(MINOR), "r")], true)]
    #[case::exact_match_rwm(vec![rule(true, LinuxDeviceType::B, Some(MAJOR), Some(MINOR), "rwm")], false)]
    #[case::type_all_is_wildcard(vec![rule(true, LinuxDeviceType::A, Some(MAJOR), Some(MINOR), "rm")], true)]
    #[case::deny_rule_ignored(vec![rule(false, LinuxDeviceType::B, Some(MAJOR), Some(MINOR), "rm")], false)]
    #[case::wildcard_major_ignored(vec![rule(true, LinuxDeviceType::B, None, Some(MINOR), "rm")], false)]
    #[case::wildcard_minor_ignored(vec![rule(true, LinuxDeviceType::B, Some(MAJOR), None, "rm")], false)]
    #[case::type_mismatch_ignored(vec![rule(true, LinuxDeviceType::C, Some(MAJOR), Some(MINOR), "rm")], false)]
    #[case::different_device_ignored(vec![rule(true, LinuxDeviceType::B, Some(9), Some(1), "rm")], false)]
    #[case::first_exact_match_wins(
        vec![
            rule(true, LinuxDeviceType::B, Some(MAJOR), Some(MINOR), "rm"),
            rule(true, LinuxDeviceType::B, Some(MAJOR), Some(MINOR), "rwm"),
        ],
        true
    )]
    fn test_device_cgroup_access_is_readonly(
        #[case] rules: Vec<LinuxDeviceCgroup>,
        #[case] expected: bool,
    ) {
        let linux = linux_with_rules(rules);
        assert_eq!(
            device_cgroup_access_is_readonly(&linux, LinuxDeviceType::B, MAJOR, MINOR),
            expected
        );
    }

    #[test]
    fn test_no_resources() {
        let linux = LinuxBuilder::default().build().unwrap();
        assert!(!device_cgroup_access_is_readonly(
            &linux,
            LinuxDeviceType::B,
            MAJOR,
            MINOR
        ));
    }
}
