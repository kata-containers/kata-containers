// Copyright (c) Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! OpenVMM hypervisor lifecycle management over the standalone VM service.

use anyhow::{anyhow, Context, Result};
use kata_types::config::KATA_PATH;
use protobuf::MessageField;
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::inner::OpenVmmInner;
use super::vmm_instance::OPENVMM_READY_TIMEOUT;
use super::vmservice;
use super::{
    OPENVMM_BLOCK_HOTPLUG_FIRST_DEVICE, OPENVMM_BLOCK_HOTPLUG_PORT_COUNT,
    OPENVMM_BLOCK_HOTPLUG_PORT_PREFIX, OPENVMM_NET_PCI_FIRST_DEVICE, OPENVMM_NET_PCI_MAX_COUNT,
    OPENVMM_ROOTFS_PCI_DEVICE, OPENVMM_SHAREFS_PCI_DEVICE, OPENVMM_VFIO_COLDPLUG_FIRST_DEVICE,
    OPENVMM_VFIO_COLDPLUG_FUNCTION, OPENVMM_VFIO_COLDPLUG_PORT_COUNT,
    OPENVMM_VFIO_COLDPLUG_PORT_PREFIX, OPENVMM_VSOCK_PCI_DEVICE,
};
use crate::device::driver::vfio_device::{DeviceAddress, VfioDevice, VfioDeviceModern};
use crate::device::pci_path::{PciPath, PciSlot};
use crate::kernel_param::KernelParams;
use crate::utils::{get_jailer_root, get_sandbox_path};
use crate::{DeviceType, MemoryConfig, VcpuThreadIds, VmmState, VM_ROOTFS_DRIVER_BLK};

const OPENVMM_STANDALONE_VIRTIO_FS: &str = "virtio-fs";
const OPENVMM_PCIE_LOW_MMIO_BASE: u64 = 0xc000_0000;
const OPENVMM_PCIE_LOW_MMIO_END: u64 = 0xd400_0000;
const OPENVMM_PCIE_HIGH_MMIO_BASE: u64 = 0x0020_3d30_0000;
const OPENVMM_PCIE_HIGH_MMIO_END: u64 = 0x200f_3d30_0000;
const OPENVMM_LEGACY_VFIO_DIR: &str = "/dev/vfio";

#[derive(Clone, Debug)]
struct VfioPcieAssignment {
    index: u8,
    host_bdf: String,
    port_name: String,
    slot: PciSlot,
    pci_path: PciPath,
}

type VfioHandle = Arc<Mutex<VfioDeviceModern>>;

#[derive(Clone, Debug)]
struct PendingVfioDevice {
    handle: VfioHandle,
    host_path: String,
    primary_bdf: String,
    host_bdfs: Vec<String>,
}

impl PendingVfioDevice {
    fn new(
        handle: VfioHandle,
        host_path: String,
        primary_bdf: String,
        mut host_bdfs: Vec<String>,
    ) -> Result<Self> {
        host_bdfs.retain(|bdf| !bdf.is_empty());
        host_bdfs.sort();
        host_bdfs.dedup();

        if host_bdfs.is_empty() {
            return Err(anyhow!("openvmm: VFIO group contains no PCI devices"));
        }
        if !host_bdfs.iter().any(|bdf| bdf == &primary_bdf) {
            return Err(anyhow!(
                "openvmm: VFIO primary BDF {} is absent from its device group",
                primary_bdf
            ));
        }

        Ok(Self {
            handle,
            host_path,
            primary_bdf,
            host_bdfs,
        })
    }
}

#[derive(Debug)]
struct PlannedVfioDevice {
    handle: VfioHandle,
    host_path: String,
    primary_bdf: String,
    primary_pci_path: PciPath,
    device_options: Vec<String>,
}

#[derive(Debug, Default)]
struct VfioPciePlan {
    assignments: Vec<VfioPcieAssignment>,
    devices: Vec<PlannedVfioDevice>,
}

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

fn agent_vsock_pcie_port(socket_path: &str) -> vmservice::PciePort {
    make_pcie_port(
        "vsock",
        PciSlot::new(OPENVMM_VSOCK_PCI_DEVICE),
        false,
        Some(vsock_device_kind(socket_path.to_string())),
    )
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

fn vfio_device_kind(host_pci_address: String) -> vmservice::PcieDeviceKind {
    vmservice::PcieDeviceKind {
        kind: Some(vmservice::pcie_device_kind::Kind::Vfio(
            vmservice::VfioDevice {
                host_pci_address,
                ..Default::default()
            },
        )),
        ..Default::default()
    }
}

/// Build a PCIe root port at `slot`, optionally carrying a
/// cold-plug endpoint. Empty `hotplug` ports are populated later via
/// AddPcieDevice.
fn make_pcie_port(
    name: &str,
    slot: PciSlot,
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
        devfn: Some(u32::from(slot.devfn())),
        ..Default::default()
    }
}

fn validate_legacy_vfio_backend(device: &VfioDevice, vfio_dir: &Path) -> Result<()> {
    let group_id = device
        .iommu_group_id
        .context("openvmm: VFIO device has no IOMMU group ID")?;
    let group = device
        .iommu_group
        .as_ref()
        .context("openvmm: VFIO device has no legacy group metadata")?;
    let expected_devnode = vfio_dir.join(group_id.to_string());

    if group.devnode != expected_devnode {
        return Err(anyhow!(
            "openvmm TTRPC VFIO requires legacy group node {}; device was discovered via {}",
            expected_devnode.display(),
            group.devnode.display()
        ));
    }

    let metadata = fs::metadata(&expected_devnode).with_context(|| {
        format!(
            "openvmm TTRPC VFIO requires legacy group node {}",
            expected_devnode.display()
        )
    })?;
    if !metadata.file_type().is_char_device() {
        return Err(anyhow!(
            "openvmm TTRPC VFIO legacy group node {} is not a character device",
            expected_devnode.display()
        ));
    }

    Ok(())
}

fn plan_vfio_devices(pending_devices: Vec<PendingVfioDevice>) -> Result<VfioPciePlan> {
    let mut all_bdfs = Vec::new();
    for device in &pending_devices {
        all_bdfs.extend(device.host_bdfs.iter().cloned());
    }

    all_bdfs.sort();
    all_bdfs.dedup();
    if all_bdfs.len() > usize::from(OPENVMM_VFIO_COLDPLUG_PORT_COUNT) {
        return Err(anyhow!(
            "openvmm: too many VFIO devices (limit {})",
            OPENVMM_VFIO_COLDPLUG_PORT_COUNT
        ));
    }

    let mut assignments = Vec::with_capacity(all_bdfs.len());
    for (index, host_bdf) in all_bdfs.into_iter().enumerate() {
        let index = index as u8;
        let slot = PciSlot::new_with_function(
            OPENVMM_VFIO_COLDPLUG_FIRST_DEVICE + index,
            OPENVMM_VFIO_COLDPLUG_FUNCTION,
        )?;
        let pci_path = PciPath::new(vec![slot, PciSlot::new(0)])
            .context("openvmm: failed to build VFIO guest PCI path")?;
        assignments.push(VfioPcieAssignment {
            index,
            port_name: format!("{}{}", OPENVMM_VFIO_COLDPLUG_PORT_PREFIX, index),
            host_bdf,
            slot,
            pci_path,
        });
    }

    let devices = pending_devices
        .into_iter()
        .map(|device| {
            let primary_pci_path = assignments
                .iter()
                .find(|assignment| assignment.host_bdf == device.primary_bdf)
                .map(|assignment| assignment.pci_path.clone())
                .context("openvmm: failed to map the primary VFIO device")?;
            let device_options = device
                .host_bdfs
                .iter()
                .map(|host_bdf| {
                    let assignment = assignments
                        .iter()
                        .find(|assignment| assignment.host_bdf == *host_bdf)
                        .context("openvmm: failed to map a VFIO device")?;
                    Ok(format!("{}={}", host_bdf, assignment.pci_path))
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(PlannedVfioDevice {
                handle: device.handle,
                host_path: device.host_path,
                primary_bdf: device.primary_bdf,
                primary_pci_path,
                device_options,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(VfioPciePlan {
        assignments,
        devices,
    })
}

fn make_pcie_root_complex(root_ports: Vec<vmservice::PciePort>) -> vmservice::PcieRootComplex {
    vmservice::PcieRootComplex {
        name: "rc0".to_string(),
        segment: 0,
        start_bus: 0,
        end_bus: 127,
        low_mmio: OPENVMM_PCIE_LOW_MMIO_END - OPENVMM_PCIE_LOW_MMIO_BASE,
        high_mmio: OPENVMM_PCIE_HIGH_MMIO_END - OPENVMM_PCIE_HIGH_MMIO_BASE,
        low_mmio_base: Some(OPENVMM_PCIE_LOW_MMIO_BASE),
        high_mmio_base: Some(OPENVMM_PCIE_HIGH_MMIO_BASE),
        preserve_bars: false,
        root_ports,
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
        fs::create_dir_all(&self.jailer_root).context(format!(
            "failed to create jailer root: {}",
            self.jailer_root
        ))?;
        fs::create_dir_all(&self.run_dir)
            .context(format!("failed to create run dir: {}", self.run_dir))?;

        Ok(())
    }

    pub(crate) async fn start_vm(&mut self, _timeout: i32) -> Result<()> {
        info!(sl!(), "openvmm: start_vm via external ttrpc process");

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
                PciSlot::new(OPENVMM_ROOTFS_PCI_DEVICE),
                false,
                Some(blk_device_kind(disk_path.clone(), true)),
            ));
            Some(disk_path)
        } else {
            None
        };

        let vsock_socket_path = format!("{}/vsock.sock", self.run_dir);
        let pending = self.pending_devices.clone();
        let mut deferred_block_devices = Vec::new();
        let mut network_index = 0u8;
        let mut agent_vsock_port = None;
        let mut pending_vfio_devices = Vec::new();

        for dev in &pending {
            match dev {
                DeviceType::HybridVsock(hvsock_dev) => {
                    info!(
                        sl!(),
                        "openvmm: HybridVsock requested, guest_cid={}, uds_path={}",
                        hvsock_dev.config.guest_cid,
                        hvsock_dev.config.uds_path
                    );
                    // OpenVMM always backs virtio-vsock with its fixed UDS
                    // relay, so normalize Kata's hybrid-vsock request to it.
                    agent_vsock_port = Some(agent_vsock_pcie_port(&vsock_socket_path));
                }
                DeviceType::Vsock(vsock_dev) => {
                    info!(
                        sl!(),
                        "openvmm: Vsock requested, guest_cid={}", vsock_dev.config.guest_cid
                    );
                    // OpenVMM supports the guest virtio-vsock device through
                    // the same fixed UDS relay; its VM service fixes CID to 3.
                    agent_vsock_port = Some(agent_vsock_pcie_port(&vsock_socket_path));
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
                        PciSlot::new(device),
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
                        PciSlot::new(OPENVMM_SHAREFS_PCI_DEVICE),
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
                DeviceType::VfioModern(vfio_handle) => {
                    let vfio_device = vfio_handle.lock().await;
                    validate_legacy_vfio_backend(
                        &vfio_device.device,
                        Path::new(OPENVMM_LEGACY_VFIO_DIR),
                    )?;
                    let primary_bdf = match &vfio_device.device.primary.addr {
                        DeviceAddress::Pci(bdf) => bdf.to_string(),
                        other => {
                            return Err(anyhow!(
                                "openvmm only supports PCI VFIO devices, got primary {}",
                                other
                            ));
                        }
                    };
                    let group_devices = if vfio_device.device.devices.is_empty() {
                        vec![vfio_device.device.primary.clone()]
                    } else {
                        vfio_device.device.devices.clone()
                    };
                    let host_bdfs = group_devices
                        .iter()
                        .map(|device| match &device.addr {
                            DeviceAddress::Pci(bdf) => Ok(bdf.to_string()),
                            other => Err(anyhow!(
                                "openvmm only supports PCI VFIO devices, got {}",
                                other
                            )),
                        })
                        .collect::<Result<Vec<_>>>()?;
                    pending_vfio_devices.push(PendingVfioDevice::new(
                        vfio_handle.clone(),
                        vfio_device.config.host_path.clone(),
                        primary_bdf,
                        host_bdfs,
                    )?);
                }
                DeviceType::Vfio(_) => {
                    return Err(anyhow!(
                        "openvmm does not support the legacy VFIO device model; use VfioModern"
                    ));
                }
                other => {
                    warn!(sl!(), "openvmm: unsupported pending device type: {}", other);
                }
            }
        }

        let vfio_plan = plan_vfio_devices(pending_vfio_devices)?;
        for device in &vfio_plan.devices {
            info!(
                sl!(),
                "openvmm: cold-plug VFIO group {} with {} PCI function(s), primary {} at {}",
                device.host_path,
                device.device_options.len(),
                device.primary_bdf,
                device.primary_pci_path
            );
        }

        let ttrpc_socket_path = format!("{}/openvmm.sock", self.run_dir);
        let serial_socket_path = format!("{}/serial.sock", self.run_dir);
        let _ = std::fs::remove_file(&vsock_socket_path);
        let _ = std::fs::remove_file(&ttrpc_socket_path);
        let _ = std::fs::remove_file(&serial_socket_path);

        // virtio-vsock-pci carries the Kata agent channel (replacing the
        // Hyper-V socket). OpenVMM binds a listener at this UDS and relays it to
        // the guest's virtio-vsock; the runtime connects over the same UDS using
        // the hybrid-vsock "hvsock://" scheme (see get_agent_socket).
        root_ports
            .push(agent_vsock_port.unwrap_or_else(|| agent_vsock_pcie_port(&vsock_socket_path)));

        // Pre-declare empty, hotplug-capable ports (hp0..) for block volumes
        // that are hot-added after resume. Their device numbers match the
        // OpenVmmHotplugPort pool so the guest pci_path can be computed without
        // an OpenVMM round-trip.
        for index in 0..OPENVMM_BLOCK_HOTPLUG_PORT_COUNT {
            let device = OPENVMM_BLOCK_HOTPLUG_FIRST_DEVICE + index;
            root_ports.push(make_pcie_port(
                &format!("{}{}", OPENVMM_BLOCK_HOTPLUG_PORT_PREFIX, index),
                PciSlot::new(device),
                true,
                None,
            ));
        }

        for index in 0..OPENVMM_VFIO_COLDPLUG_PORT_COUNT {
            let slot = PciSlot::new_with_function(
                OPENVMM_VFIO_COLDPLUG_FIRST_DEVICE + index,
                OPENVMM_VFIO_COLDPLUG_FUNCTION,
            )?;
            let device_kind = vfio_plan
                .assignments
                .iter()
                .find(|assignment| assignment.index == index)
                .map(|assignment| {
                    debug!(
                        sl!(),
                        "openvmm: assigning VFIO BDF {} to port {} (guest pci_path={})",
                        assignment.host_bdf,
                        assignment.port_name,
                        assignment.pci_path
                    );
                    debug_assert_eq!(assignment.slot, slot);
                    vfio_device_kind(assignment.host_bdf.clone())
                });
            root_ports.push(make_pcie_port(
                &format!("{}{}", OPENVMM_VFIO_COLDPLUG_PORT_PREFIX, index),
                slot,
                false,
                device_kind,
            ));
        }

        let pcie = vmservice::PcieTopologyConfig {
            root_complexes: vec![make_pcie_root_complex(root_ports)],
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
                .launch_with_timeout(
                    &self.config.path,
                    ttrpc_socket_path,
                    request,
                    self.netns.clone(),
                    Some(self.run_dir.clone()),
                    OPENVMM_READY_TIMEOUT,
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

        for planned_device in vfio_plan.devices {
            let mut vfio_device = planned_device.handle.lock().await;
            vfio_device.config.guest_pci_path = Some(planned_device.primary_pci_path);
            vfio_device.device_options = planned_device.device_options;
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
        fs::create_dir_all(&self.jailer_root).context(format!(
            "failed to create openvmm jailer root {}",
            self.jailer_root
        ))?;
        Ok(self.jailer_root.clone())
    }

    pub(crate) async fn get_hypervisor_metrics(&self) -> Result<String> {
        Err(anyhow!("openvmm hypervisor metrics not implemented"))
    }

    pub(crate) async fn get_passfd_listener_addr(&self) -> Result<(String, u32)> {
        Err(anyhow!("openvmm passfd IO is not supported"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn pending_vfio_device(primary_bdf: &str, host_bdfs: &[&str]) -> Result<PendingVfioDevice> {
        PendingVfioDevice::new(
            Arc::new(Mutex::new(VfioDeviceModern::default())),
            "/dev/vfio/test".to_string(),
            primary_bdf.to_string(),
            host_bdfs.iter().map(|bdf| bdf.to_string()).collect(),
        )
    }

    fn vfio_device_with_group(group_id: u32, devnode: &Path) -> VfioDevice {
        let mut device = VfioDevice {
            iommu_group_id: Some(group_id),
            ..Default::default()
        };
        let mut group = device.iommu_group.take().unwrap_or_default();
        group.group_id = group_id;
        group.devnode = devnode.to_path_buf();
        device.iommu_group = Some(group);
        device
    }

    #[test]
    fn vfio_assignments_are_deterministic_and_track_the_actual_primary() {
        let plan = plan_vfio_devices(vec![pending_vfio_device(
            "0000:02:00.0",
            &["0000:02:00.0", "0000:01:00.0"],
        )
        .unwrap()])
        .unwrap();

        assert_eq!(plan.assignments[0].host_bdf, "0000:01:00.0");
        assert_eq!(plan.assignments[0].port_name, "vfio0");
        assert_eq!(plan.assignments[0].pci_path.to_string(), "08.1/00");
        assert_eq!(plan.assignments[1].host_bdf, "0000:02:00.0");
        assert_eq!(plan.assignments[1].port_name, "vfio1");
        assert_eq!(plan.devices[0].primary_pci_path.to_string(), "09.1/00");
    }

    #[test]
    fn vfio_assignments_enforce_capacity_and_primary_membership() {
        let sixteen = (0..OPENVMM_VFIO_COLDPLUG_PORT_COUNT)
            .map(|index| format!("0000:{index:02x}:00.0"))
            .collect::<Vec<_>>();
        let sixteen_device = PendingVfioDevice::new(
            Arc::new(Mutex::new(VfioDeviceModern::default())),
            "/dev/vfio/16".to_string(),
            sixteen[0].clone(),
            sixteen.clone(),
        )
        .unwrap();
        assert!(plan_vfio_devices(vec![sixteen_device]).is_ok());

        let mut seventeen = sixteen;
        seventeen.push("0000:10:00.0".to_string());
        let seventeen_device = PendingVfioDevice::new(
            Arc::new(Mutex::new(VfioDeviceModern::default())),
            "/dev/vfio/17".to_string(),
            seventeen[0].clone(),
            seventeen,
        )
        .unwrap();
        assert!(plan_vfio_devices(vec![seventeen_device]).is_err());
        assert!(pending_vfio_device("0000:02:00.0", &["0000:01:00.0"]).is_err());
    }

    #[test]
    fn vfio_assignments_deduplicate_across_handles() {
        let plan = plan_vfio_devices(vec![
            pending_vfio_device(
                "0000:03:00.0",
                &["", "0000:03:00.0", "0000:02:00.0", "0000:03:00.0"],
            )
            .unwrap(),
            pending_vfio_device("0000:03:00.0", &["0000:02:00.0", "0000:03:00.0"]).unwrap(),
        ])
        .unwrap();

        assert_eq!(plan.assignments.len(), 2);
        assert_eq!(plan.assignments[0].port_name, "vfio0");
        assert_eq!(plan.assignments[0].pci_path.to_string(), "08.1/00");
        assert_eq!(plan.assignments[1].port_name, "vfio1");
        assert_eq!(plan.assignments[1].pci_path.to_string(), "09.1/00");
        assert_eq!(
            plan.devices[0].primary_pci_path,
            plan.devices[1].primary_pci_path
        );
        assert_eq!(plan.devices[0].primary_pci_path.to_string(), "09.1/00");
        assert!(pending_vfio_device("", &[]).is_err());
    }

    #[test]
    fn duplicate_handles_do_not_exhaust_vfio_capacity() {
        let sixteen = (0..OPENVMM_VFIO_COLDPLUG_PORT_COUNT)
            .map(|index| format!("0000:{index:02x}:00.0"))
            .collect::<Vec<_>>();
        let device = PendingVfioDevice::new(
            Arc::new(Mutex::new(VfioDeviceModern::default())),
            "/dev/vfio/16".to_string(),
            sixteen[0].clone(),
            sixteen,
        )
        .unwrap();
        let plan = plan_vfio_devices(vec![device.clone(), device]).unwrap();

        assert_eq!(plan.assignments.len(), 16);
        assert_eq!(plan.devices.len(), 2);
        assert_eq!(
            plan.devices[0].primary_pci_path,
            plan.devices[1].primary_pci_path
        );
    }

    #[test]
    fn vfio_group_options_include_every_assigned_bdf() {
        let plan = plan_vfio_devices(vec![pending_vfio_device(
            "0000:03:00.0",
            &["0000:03:00.0", "0000:02:00.0"],
        )
        .unwrap()])
        .unwrap();

        assert_eq!(
            plan.devices[0].device_options,
            [
                "0000:02:00.0=08.1/00".to_string(),
                "0000:03:00.0=09.1/00".to_string(),
            ]
        );
    }

    #[test]
    fn empty_vfio_plan_is_valid() {
        let plan = plan_vfio_devices(Vec::new()).unwrap();
        assert!(plan.assignments.is_empty());
        assert!(plan.devices.is_empty());
    }

    #[test]
    fn openvmm_vfio_requires_a_legacy_group_node() {
        let vfio_dir = TempDir::new().unwrap();
        let legacy_path = vfio_dir.path().join("42");
        symlink("/dev/null", &legacy_path).unwrap();

        let legacy = vfio_device_with_group(42, &legacy_path);
        validate_legacy_vfio_backend(&legacy, vfio_dir.path()).unwrap();

        let cdev = vfio_device_with_group(42, Path::new("/dev/vfio/devices/vfio7"));
        let error = validate_legacy_vfio_backend(&cdev, vfio_dir.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("requires legacy group node"));
        assert!(error.contains("/dev/vfio/devices/vfio7"));
    }

    #[test]
    fn pcie_root_complex_uses_gpu_sized_fixed_mmio_windows() {
        let root = make_pcie_root_complex(Vec::new());
        assert_eq!(root.low_mmio_base, Some(OPENVMM_PCIE_LOW_MMIO_BASE));
        assert_eq!(root.low_mmio, 320 * 1024 * 1024);
        assert_eq!(root.high_mmio_base, Some(OPENVMM_PCIE_HIGH_MMIO_BASE));
        assert_eq!(
            root.high_mmio,
            OPENVMM_PCIE_HIGH_MMIO_END - OPENVMM_PCIE_HIGH_MMIO_BASE
        );
        assert!(!root.preserve_bars);
    }
}
