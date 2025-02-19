// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{sync::Arc, thread};

use agent::{types::Device, Agent, OnlineCPUMemRequest, Storage};
use anyhow::{anyhow, Context, Ok, Result};
use async_trait::async_trait;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_driver, DeviceManager},
        util::{get_host_path, DEVICE_TYPE_CHAR},
        DeviceConfig, DeviceType,
    },
    BlockConfig, Hypervisor, VfioConfig,
};
use kata_types::config::{hypervisor::TopologyConfigInfo, TomlConfig};
use kata_types::mount::Mount;
use oci::{Linux, LinuxCpu, LinuxResources};
use oci_spec::runtime::{self as oci, LinuxDeviceType};
use persist::sandbox_persist::Persist;
use tokio::{runtime, sync::RwLock};

use crate::{
    cdi_devices::{sort_options_by_pcipath, ContainerDevice, DeviceInfo},
    cgroups::{CgroupArgs, CgroupsResource},
    cpu_mem::{cpu::CpuResource, initial_size::InitialSizeManager, mem::MemResource},
    manager::ManagerArgs,
    network::{self, Network, NetworkConfig},
    resource_persist::ResourceState,
    rootfs::{RootFsResource, Rootfs},
    share_fs::{self, sandbox_bind_mounts::SandboxBindMounts, ShareFs},
    volume::{Volume, VolumeResource},
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

    pub rootfs_resource: RootFsResource,
    pub volume_resource: VolumeResource,
    pub cgroups_resource: CgroupsResource,
    pub cpu_resource: CpuResource,
    pub mem_resource: MemResource,
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

        let cgroups_resource = CgroupsResource::new(sid, &toml_config)?;
        let cpu_resource = CpuResource::new(toml_config.clone())?;
        let mem_resource = MemResource::new(init_size_manager)?;
        Ok(Self {
            sid: sid.to_string(),
            toml_config,
            agent,
            hypervisor,
            device_manager: Arc::new(RwLock::new(dev_manager)),
            network: None,
            share_fs: None,
            rootfs_resource: RootFsResource::new(),
            volume_resource: VolumeResource::new(),
            cgroups_resource,
            cpu_resource,
            mem_resource,
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
                    self.share_fs = if self
                        .hypervisor
                        .capabilities()
                        .await?
                        .is_fs_sharing_supported()
                    {
                        let share_fs = share_fs::new(&self.sid, &c).context("new share fs")?;
                        share_fs
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

                        Some(share_fs)
                    } else {
                        None
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
            };
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
            // update interface
            info!(sl!(), "update interface {:?}", i);
            self.agent
                .update_interface(agent::UpdateInterfaceRequest { interface: Some(i) })
                .await
                .context("update interface")?;
        }

        Ok(())
    }

    async fn handle_neighbours(&self, network: &dyn Network) -> Result<()> {
        let neighbors = network.neighs().await.context("neighs")?;
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
        if let Some(share_fs) = self.share_fs.as_ref() {
            share_fs
                .setup_device_after_start_vm(self.hypervisor.as_ref(), &self.device_manager)
                .await
                .context("setup share fs device after start vm")?;
        }

        if let Some(network) = self.network.as_ref() {
            let network = network.as_ref();
            self.handle_interfaces(network)
                .await
                .context("handle interfaces")?;
            self.handle_neighbours(network)
                .await
                .context("handle neighbors")?;
            self.handle_routes(network).await.context("handle routes")?;
        }

        Ok(())
    }

    pub async fn get_storage_for_sandbox(&self) -> Result<Vec<Storage>> {
        let mut storages = vec![];
        if let Some(d) = self.share_fs.as_ref() {
            let mut s = d.get_storages().await.context("get storage")?;
            storages.append(&mut s);
        }
        Ok(storages)
    }

    pub async fn handler_rootfs(
        &self,
        cid: &str,
        root: &oci::Root,
        bundle_path: &str,
        rootfs_mounts: &[Mount],
    ) -> Result<Arc<dyn Rootfs>> {
        self.rootfs_resource
            .handler_rootfs(
                &self.share_fs,
                self.device_manager.as_ref(),
                self.hypervisor.as_ref(),
                &self.sid,
                cid,
                root,
                bundle_path,
                rootfs_mounts,
            )
            .await
    }

    pub async fn handler_volumes(
        &self,
        cid: &str,
        spec: &oci::Spec,
    ) -> Result<Vec<Arc<dyn Volume>>> {
        self.volume_resource
            .handler_volumes(
                &self.share_fs,
                cid,
                spec,
                self.device_manager.as_ref(),
                &self.sid,
                self.agent.clone(),
            )
            .await
    }

    pub async fn handler_devices(&self, _cid: &str, linux: &Linux) -> Result<Vec<ContainerDevice>> {
        let mut devices = vec![];

        let linux_devices = linux.devices().clone().unwrap_or_default();
        for d in linux_devices.iter() {
            match d.typ() {
                LinuxDeviceType::B => {
                    let block_driver = get_block_driver(&self.device_manager).await;
                    let dev_info = DeviceConfig::BlockCfg(BlockConfig {
                        major: d.major(),
                        minor: d.minor(),
                        driver_option: block_driver,
                        ..Default::default()
                    });

                    let device_info = do_handle_device(&self.device_manager, &dev_info)
                        .await
                        .context("do handle device")?;

                    // create block device for kata agent,
                    // if driver is virtio-blk-pci, the id will be pci address.
                    if let DeviceType::Block(device) = device_info {
                        // The following would work for drivers virtio-blk-pci and mmio.
                        // Once scsi support is added, need to handle scsi identifiers.
                        let id = if let Some(pci_path) = device.config.pci_path {
                            pci_path.to_string()
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

                    let dev_info = DeviceConfig::VfioCfg(VfioConfig {
                        host_path,
                        dev_type: "c".to_string(),
                        hostdev_prefix: "vfio_device".to_owned(),
                        ..Default::default()
                    });

                    let device_info = do_handle_device(&self.device_manager.clone(), &dev_info)
                        .await
                        .context("do handle device")?;

                    // vfio mode: vfio-pci and vfio-pci-gk for x86_64
                    // - vfio-pci, devices appear as VFIO character devices under /dev/vfio in container.
                    // - vfio-pci-gk, devices are managed by whatever driver in Guest kernel.
                    let vfio_mode = match self.toml_config.runtime.vfio_mode.as_str() {
                        "vfio" => "vfio-pci".to_string(),
                        _ => "vfio-pci-gk".to_string(),
                    };

                    // create agent device
                    if let DeviceType::Vfio(device) = device_info {
                        let device_options = sort_options_by_pcipath(device.device_options);
                        let agent_device = Device {
                            id: device.device_id, // just for kata-agent
                            container_path: d.path().display().to_string().clone(),
                            field_type: vfio_mode,
                            options: device_options,
                            ..Default::default()
                        };

                        let vendor_class = device
                            .devices
                            .first()
                            .unwrap()
                            .device_vendor_class
                            .as_ref()
                            .unwrap()
                            .get_vendor_class_id()
                            .context("get vendor class failed")?;
                        let device_info = Some(DeviceInfo {
                            vendor_id: vendor_class.0.to_owned(),
                            class_id: vendor_class.1.to_owned(),
                            host_path: d.path().clone(),
                        });
                        devices.push(ContainerDevice {
                            device_info,
                            device: agent_device,
                        });
                    }
                }
                _ => {
                    // TODO enable other devices type
                    continue;
                }
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
        // clean up cgroup
        self.cgroups_resource
            .delete()
            .await
            .context("delete cgroup")?;

        // cleanup sandbox bind mounts: setup = false
        self.handle_sandbox_bindmounts(false)
            .await
            .context("failed to cleanup sandbox bindmounts")?;

        // clean up share fs mount
        if let Some(share_fs) = &self.share_fs {
            share_fs
                .get_share_fs_mount()
                .cleanup(&self.sid)
                .await
                .context("failed to cleanup host path")?;
        }
        // TODO cleanup other resources
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
                    nb_cpus: self.cpu_resource.current_vcpu().await,
                    cpu_only: false,
                })
                .await
                .context("online vcpus")?;
        }

        // we should firstly update the vcpus and mems, and then update the host cgroups
        self.cgroups_resource
            .update_cgroups(cid, linux_resources, op, self.hypervisor.as_ref())
            .await?;

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

        Ok(Self {
            sid: resource_args.sid,
            agent: resource_args.agent,
            hypervisor: resource_args.hypervisor.clone(),
            device_manager: Arc::new(RwLock::new(
                DeviceManager::new(resource_args.hypervisor, topo_config.as_ref()).await?,
            )),
            network: None,
            share_fs: None,
            rootfs_resource: RootFsResource::new(),
            volume_resource: VolumeResource::new(),
            cgroups_resource: CgroupsResource::restore(
                args,
                resource_state.cgroup_state.unwrap_or_default(),
            )
            .await?,
            toml_config: Arc::new(TomlConfig::default()),
            cpu_resource: CpuResource::default(),
            mem_resource: MemResource::default(),
        })
    }
}
