// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! OpenVMM hypervisor lifecycle management over the standalone VM service.

use anyhow::{anyhow, Context, Result};
use kata_types::config::KATA_PATH;
use protobuf::MessageField;
use std::fs;

use super::inner::OpenVmmInner;
use super::vmservice;
use super::{
    OPENVMM_BLOCK_HOTPLUG_FIRST_DEVICE, OPENVMM_BLOCK_HOTPLUG_PORT_COUNT,
    OPENVMM_BLOCK_HOTPLUG_PORT_PREFIX, OPENVMM_NET_PCI_FIRST_DEVICE, OPENVMM_NET_PCI_MAX_COUNT,
    OPENVMM_ROOTFS_PCI_DEVICE, OPENVMM_SHAREFS_PCI_DEVICE, OPENVMM_VSOCK_PCI_DEVICE,
};
use crate::kernel_param::KernelParams;
use crate::utils::{get_jailer_root, get_sandbox_path};
use crate::{DeviceType, MemoryConfig, VcpuThreadIds, VmmState, VM_ROOTFS_DRIVER_BLK};

const OPENVMM_STANDALONE_VIRTIO_FS: &str = "virtio-fs";

fn build_kernel_cmdline(
    debug: bool,
    kernel_params: &str,
    kernel_verity_params: &str,
    rootfs_type: &str,
) -> Result<String> {
    let mut params = KernelParams::new(debug);

    let mut rootfs_params = KernelParams::new_rootfs_kernel_params(
        kernel_verity_params,
        VM_ROOTFS_DRIVER_BLK,
        rootfs_type,
        false,
    )?;
    params.append(&mut rootfs_params);
    params.append(&mut KernelParams::from_string(kernel_params));

    params.to_string()
}

fn adapt_cmdline_for_rpc(cmdline: String) -> String {
    cmdline.replace("console=hvc0", "console=ttyS0")
}

/// Wrap a virtio device function as a `PcieDeviceKind` (the endpoint behind a
/// PCIe root port).
fn virtio_pcie_device(kind: vmservice::virtio_device::Kind) -> vmservice::PcieDeviceKind {
    vmservice::PcieDeviceKind {
        kind: Some(vmservice::pcie_device_kind::Kind::Virtio(
            vmservice::VirtioDevice {
                kind: Some(kind),
                ..Default::default()
            },
        )),
        ..Default::default()
    }
}

/// Build a virtio-blk-pci endpoint backed by a host file or block device node.
pub(super) fn blk_device_kind(path: String, read_only: bool) -> vmservice::PcieDeviceKind {
    virtio_pcie_device(vmservice::virtio_device::Kind::Blk(vmservice::VirtioBlk {
        backend: MessageField::some(vmservice::DiskBackend {
            kind: Some(vmservice::disk_backend::Kind::File(vmservice::FileDisk {
                path,
                direct: false,
                ..Default::default()
            })),
            ..Default::default()
        }),
        read_only,
        ..Default::default()
    }))
}

/// Build a virtio-net-pci endpoint backed by a host TAP, opened by name inside
/// the OpenVMM process (which runs in the sandbox network namespace).
fn net_device_kind(mac_address: String, tap_name: String) -> vmservice::PcieDeviceKind {
    virtio_pcie_device(vmservice::virtio_device::Kind::Net(vmservice::VirtioNet {
        backend: MessageField::some(vmservice::NicBackend {
            kind: Some(vmservice::nic_backend::Kind::Tap(vmservice::TapBackend {
                name: tap_name,
                ..Default::default()
            })),
            ..Default::default()
        }),
        mac_address,
        ..Default::default()
    }))
}

/// Build a virtio-vsock-pci endpoint relayed over a host Unix socket.
fn vsock_device_kind(socket_path: String) -> vmservice::PcieDeviceKind {
    virtio_pcie_device(vmservice::virtio_device::Kind::Vsock(
        vmservice::VirtioVsock {
            socket_path,
            ..Default::default()
        },
    ))
}

/// Build a vhost-user-fs endpoint (virtiofsd backend reached over a Unix socket).
fn vhost_user_fs_device_kind(socket_path: String, tag: String) -> vmservice::PcieDeviceKind {
    virtio_pcie_device(vmservice::virtio_device::Kind::VhostUser(
        vmservice::VhostUser {
            socket_path,
            device: MessageField::some(vmservice::VhostUserDevice {
                kind: Some(vmservice::vhost_user_device::Kind::Fs(
                    vmservice::VhostUserFs {
                        tag,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            }),
            ..Default::default()
        },
    ))
}

/// Build a PCIe root port at `device` (function 0), optionally carrying a
/// cold-plug endpoint. Empty `hotplug` ports are populated later via
/// AddPcieDevice.
fn make_pcie_port(
    name: &str,
    device: u8,
    hotplug: bool,
    device_kind: Option<vmservice::PcieDeviceKind>,
) -> vmservice::PciePort {
    let attached = match device_kind {
        Some(dev) => MessageField::some(vmservice::PcieAttachment {
            kind: Some(vmservice::pcie_attachment::Kind::Device(dev)),
            ..Default::default()
        }),
        None => MessageField::none(),
    };
    vmservice::PciePort {
        name: name.to_string(),
        hotplug,
        attached,
        // Pin the port at function 0 of its device so the guest-visible path is
        // deterministic ("DD/00"); see OpenVmmHotplugPort.
        devfn: Some((device as u32) << 3),
        ..Default::default()
    }
}

fn mac_address(device: &crate::NetworkDevice, index: usize) -> String {
    device
        .config
        .guest_mac
        .as_ref()
        .map(|mac| {
            format!(
                "{:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}",
                mac.0[0], mac.0[1], mac.0[2], mac.0[3], mac.0[4], mac.0[5]
            )
        })
        .unwrap_or_else(|| format!("02-00-00-00-00-{:02X}", index + 1))
}

impl OpenVmmInner {
    pub(crate) async fn prepare_vm(&mut self, id: &str, netns: Option<String>) -> Result<()> {
        info!(sl!(), "openvmm: prepare_vm id={}", id);
        self.id = id.to_string();
        self.state = VmmState::NotReady;
        self.pending_devices.clear();
        self.reset_block_hotplug_ports();
        self.vm_path = get_sandbox_path(id);
        self.jailer_root = get_jailer_root(id);
        self.netns = netns;

        self.run_dir = format!("{}/{}", KATA_PATH, id);
        fs::create_dir_all(&self.jailer_root)
            .with_context(|| format!("failed to create jailer root: {}", self.jailer_root))?;
        fs::create_dir_all(&self.run_dir)
            .with_context(|| format!("failed to create run dir: {}", self.run_dir))?;

        Ok(())
    }

    pub(crate) async fn start_vm(&mut self, _timeout: i32) -> Result<()> {
        info!(sl!(), "openvmm: start_vm via external ttrpc process");
        self.reset_block_hotplug_ports();

        let cmdline = build_kernel_cmdline(
            self.config.debug_info.enable_debug,
            &self.config.boot_info.kernel_params,
            &self.config.boot_info.kernel_verity_params,
            &self.config.boot_info.rootfs_type,
        )?;
        let cmdline = adapt_cmdline_for_rpc(cmdline);

        info!(sl!(), "openvmm: kernel={}", self.config.boot_info.kernel);
        info!(sl!(), "openvmm: image={}", self.config.boot_info.image);
        info!(sl!(), "openvmm: cmdline={}", cmdline);

        // Build the PCIe topology: every Kata device is a virtio (or
        // vhost-user) function at function 0 of its own root port on a single
        // root complex. Cold-plug devices (rootfs, sharefs, network, the agent
        // vsock) are attached here; block volumes are hot-added after resume
        // into the pre-declared empty hotplug ports.
        let mut root_ports: Vec<vmservice::PciePort> = Vec::new();

        // rootfs as virtio-blk-pci. The guest mounts it via the kernel cmdline
        // (root=/dev/vda), so no guest pci_path needs to be reported.
        let rootfs_disk_path = if !self.config.boot_info.image.is_empty() {
            let disk_path = self.config.boot_info.image.clone();
            info!(
                sl!(),
                "openvmm: rootfs as virtio-blk-pci at device {}: {}",
                OPENVMM_ROOTFS_PCI_DEVICE,
                disk_path
            );
            root_ports.push(make_pcie_port(
                "rootfs",
                OPENVMM_ROOTFS_PCI_DEVICE,
                false,
                Some(blk_device_kind(disk_path.clone(), true)),
            ));
            Some(disk_path)
        } else {
            None
        };

        let pending = self.pending_devices.clone();
        let mut deferred_block_devices = Vec::new();
        let mut network_index = 0u8;

        for dev in &pending {
            match dev {
                DeviceType::HybridVsock(hvsock_dev) => {
                    info!(
                        sl!(),
                        "openvmm: HybridVsock requested, guest_cid={}, uds_path={}",
                        hvsock_dev.config.guest_cid,
                        hvsock_dev.config.uds_path
                    );
                }
                DeviceType::Vsock(vsock_dev) => {
                    info!(
                        sl!(),
                        "openvmm: Vsock requested, guest_cid={}", vsock_dev.config.guest_cid
                    );
                }
                DeviceType::Network(net_dev) => {
                    if network_index >= OPENVMM_NET_PCI_MAX_COUNT {
                        return Err(anyhow!(
                            "openvmm supports at most {} virtio-net-pci devices",
                            OPENVMM_NET_PCI_MAX_COUNT
                        ));
                    }
                    let device = OPENVMM_NET_PCI_FIRST_DEVICE + network_index;
                    info!(
                        sl!(),
                        "openvmm: virtio-net-pci at device {} over host TAP {}",
                        device,
                        net_dev.config.host_dev_name
                    );
                    root_ports.push(make_pcie_port(
                        &format!("net{}", network_index),
                        device,
                        false,
                        Some(net_device_kind(
                            mac_address(net_dev, network_index as usize),
                            net_dev.config.host_dev_name.clone(),
                        )),
                    ));
                    network_index += 1;
                }
                DeviceType::ShareFs(fs_dev) => {
                    // Only vhost-user virtio-fs over PCIe is supported (no
                    // vmbus / inline transport). The virtiofsd backend is
                    // started by the shared-fs resource layer, which populates
                    // sock_path.
                    if fs_dev.config.fs_type != OPENVMM_STANDALONE_VIRTIO_FS {
                        return Err(anyhow!(
                            "openvmm only supports vhost-user virtio-fs (fs_type '{}'), got '{}'",
                            OPENVMM_STANDALONE_VIRTIO_FS,
                            fs_dev.config.fs_type
                        ));
                    }
                    if fs_dev.config.sock_path.is_empty() {
                        return Err(anyhow!(
                            "openvmm vhost-user-fs for tag '{}' has no virtiofsd socket path",
                            fs_dev.config.mount_tag
                        ));
                    }
                    info!(
                        sl!(),
                        "openvmm: vhost-user-fs at device {} tag={} sock={}",
                        OPENVMM_SHAREFS_PCI_DEVICE,
                        fs_dev.config.mount_tag,
                        fs_dev.config.sock_path
                    );
                    root_ports.push(make_pcie_port(
                        "sharefs",
                        OPENVMM_SHAREFS_PCI_DEVICE,
                        false,
                        Some(vhost_user_fs_device_kind(
                            fs_dev.config.sock_path.clone(),
                            fs_dev.config.mount_tag.clone(),
                        )),
                    ));
                }
                DeviceType::BlockModern(block_device) => {
                    let path_on_host = block_device.lock().await.config.path_on_host.clone();
                    if Some(path_on_host.as_str()) == rootfs_disk_path.as_deref() {
                        info!(
                            sl!(),
                            "openvmm: skipping duplicate BlockModern device already used as rootfs: {}",
                            path_on_host
                        );
                    } else {
                        deferred_block_devices.push(dev.clone());
                    }
                }
                DeviceType::Vfio(_) => {
                    return Err(anyhow!(
                        "openvmm: VFIO device pass-through is not yet wired in Kata. \
                         OpenVMM's ttrpc API now supports it (PcieDeviceKind::Vfio with a \
                         host BDF), so the remaining work is Kata-side: convert the VFIO \
                         device to a proto VfioDevice, place it behind a PCIe root port, \
                         and report its guest pci_path."
                    ));
                }
                other => {
                    warn!(sl!(), "openvmm: unsupported pending device type: {}", other);
                }
            }
        }

        let vsock_socket_path = format!("{}/vsock.sock", self.run_dir);
        let ttrpc_socket_path = format!("{}/openvmm.sock", self.run_dir);
        let serial_socket_path = format!("{}/serial.sock", self.run_dir);
        let _ = std::fs::remove_file(&vsock_socket_path);
        let _ = std::fs::remove_file(&ttrpc_socket_path);
        let _ = std::fs::remove_file(&serial_socket_path);

        // virtio-vsock-pci carries the Kata agent channel (replacing the
        // Hyper-V socket). OpenVMM binds a listener at this UDS and relays it to
        // the guest's virtio-vsock; the runtime connects over the same UDS using
        // the hybrid-vsock "hvsock://" scheme (see get_agent_socket).
        root_ports.push(make_pcie_port(
            "vsock",
            OPENVMM_VSOCK_PCI_DEVICE,
            false,
            Some(vsock_device_kind(vsock_socket_path.clone())),
        ));

        // Pre-declare empty, hotplug-capable ports (hp0..) for block volumes
        // that are hot-added after resume. Their device numbers match the
        // OpenVmmHotplugPort pool so the guest pci_path can be computed without
        // an OpenVMM round-trip.
        for index in 0..OPENVMM_BLOCK_HOTPLUG_PORT_COUNT {
            let device = OPENVMM_BLOCK_HOTPLUG_FIRST_DEVICE + index;
            root_ports.push(make_pcie_port(
                &format!("{}{}", OPENVMM_BLOCK_HOTPLUG_PORT_PREFIX, index),
                device,
                true,
                None,
            ));
        }

        let pcie = vmservice::PcieTopologyConfig {
            root_complexes: vec![vmservice::PcieRootComplex {
                name: "rc0".to_string(),
                segment: 0,
                start_bus: 0,
                end_bus: 127,
                // MMIO apertures sized for ~32 virtio root-port bridges; bases
                // are auto-assigned by OpenVMM to stay consistent with the ECAM
                // and ACPI layout it generates.
                low_mmio: 0x1000_0000,    // 256 MiB (32-bit)
                high_mmio: 0x4_0000_0000, // 16 GiB (64-bit)
                preserve_bars: false,
                root_ports,
                ..Default::default()
            }],
            ..Default::default()
        };

        let request = vmservice::CreateVMRequest {
            config: MessageField::some(vmservice::VMConfig {
                memory_config: MessageField::some(vmservice::MemoryConfig {
                    memory_mb: self.config.memory_info.default_memory as u64,
                    ..Default::default()
                }),
                processor_config: MessageField::some(vmservice::ProcessorConfig {
                    processor_count: self.config.cpu_info.default_vcpus.ceil() as u32,
                    ..Default::default()
                }),
                pcie: MessageField::some(pcie),
                serial_config: MessageField::some(vmservice::SerialConfig {
                    ports: vec![vmservice::serial_config::Config {
                        port: 0,
                        socket_path: serial_socket_path,
                        connect: false,
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                BootConfig: Some(vmservice::vmconfig::BootConfig::DirectBoot(
                    vmservice::DirectBoot {
                        kernel_path: self.config.boot_info.kernel.clone(),
                        initrd_path: self.config.boot_info.initrd.clone(),
                        kernel_cmdline: cmdline,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            }),
            log_id: self.id.clone(),
            ..Default::default()
        };

        let startup_result: Result<()> = async {
            info!(sl!(), "openvmm: launching standalone OpenVMM process");
            self.vmm_instance
                .launch(
                    &self.config.path,
                    ttrpc_socket_path,
                    request,
                    self.netns.clone(),
                    Some(self.run_dir.clone()),
                )
                .await
                .context("failed to launch standalone OpenVMM")?;

            info!(sl!(), "openvmm: resuming VM");
            self.vmm_instance
                .resume()
                .await
                .context("failed to resume VM")?;

            self.state = VmmState::VmRunning;
            for device in deferred_block_devices {
                self.add_device(device)
                    .await
                    .context("failed to hotplug deferred block device")?;
            }

            self.vmm_instance
                .start_wait_task()
                .context("failed to start OpenVMM process monitor")?;
            Ok(())
        }
        .await;

        if let Err(err) = startup_result {
            self.state = VmmState::NotReady;
            if let Err(cleanup_err) = self.vmm_instance.stop().await {
                warn!(
                    sl!(),
                    "openvmm: failed cleaning up unsuccessful startup: {:?}", cleanup_err
                );
            }
            self.reset_block_hotplug_ports();
            return Err(err);
        }

        self.pending_devices.clear();
        info!(sl!(), "openvmm: VM is running");

        Ok(())
    }

    pub(crate) async fn stop_vm(&mut self) -> Result<()> {
        info!(sl!(), "openvmm: stop_vm");
        self.vmm_instance.stop().await?;
        self.state = VmmState::NotReady;
        Ok(())
    }

    pub(crate) async fn pause_vm(&self) -> Result<()> {
        self.vmm_instance.pause().await
    }

    pub(crate) async fn resume_vm(&self) -> Result<()> {
        self.vmm_instance.resume().await
    }

    pub(crate) async fn save_vm(&self) -> Result<()> {
        Err(anyhow!("openvmm save_vm not yet implemented"))
    }

    pub(crate) async fn resize_vcpu(&self, old_vcpus: u32, _new_vcpus: u32) -> Result<(u32, u32)> {
        Ok((old_vcpus, old_vcpus))
    }

    pub(crate) async fn resize_memory(&mut self, new_mem_mb: u32) -> Result<(u32, MemoryConfig)> {
        Ok((new_mem_mb, MemoryConfig::default()))
    }

    pub(crate) async fn get_agent_socket(&self) -> Result<String> {
        Ok(format!("hvsock://{}/vsock.sock", self.run_dir))
    }

    pub(crate) async fn disconnect(&mut self) {
        info!(sl!(), "openvmm: disconnect");
    }

    pub(crate) async fn get_thread_ids(&self) -> Result<VcpuThreadIds> {
        let pid = self.get_vmm_master_tid().await?;
        let proc_path = format!("/proc/{pid}");
        let vcpus = crate::utils::get_vcpu_tids(&proc_path, "vp-")?;
        Ok(VcpuThreadIds { vcpus })
    }

    pub(crate) async fn cleanup(&self) -> Result<()> {
        Ok(())
    }

    pub(crate) async fn get_pids(&self) -> Result<Vec<u32>> {
        Ok(self.vmm_instance.pid().into_iter().collect())
    }

    pub(crate) async fn get_vmm_master_tid(&self) -> Result<u32> {
        self.vmm_instance
            .pid()
            .ok_or_else(|| anyhow!("could not get openvmm process id"))
    }

    pub(crate) async fn get_ns_path(&self) -> Result<String> {
        let pid = self.get_vmm_master_tid().await?;
        Ok(format!("/proc/{pid}/ns"))
    }

    pub(crate) async fn check(&self) -> Result<()> {
        Ok(())
    }

    pub(crate) async fn get_jailer_root(&self) -> Result<String> {
        fs::create_dir_all(&self.jailer_root).with_context(|| {
            format!("failed to create openvmm jailer root {}", self.jailer_root)
        })?;
        Ok(self.jailer_root.clone())
    }

    pub(crate) async fn get_hypervisor_metrics(&self) -> Result<String> {
        Err(anyhow!("openvmm hypervisor metrics not implemented"))
    }

    pub(crate) async fn get_passfd_listener_addr(&self) -> Result<(String, u32)> {
        Err(anyhow!("openvmm passfd IO is not supported"))
    }
}
