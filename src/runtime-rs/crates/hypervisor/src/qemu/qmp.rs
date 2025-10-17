// Copyright (c) 2024 Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::pci_path::PciPath;
use crate::qemu::cmdline_generator::{DeviceVirtioNet, Netdev, QMP_SOCKET_FILE};
use crate::utils::get_jailer_root;
use crate::VcpuThreadIds;

use anyhow::{anyhow, Context, Result};
use kata_types::config::hypervisor::VIRTIO_SCSI;
use kata_types::rootless::is_rootless;
use nix::sys::socket::{sendmsg, ControlMessage, MsgFlags};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::{Debug, Error, Formatter};
use std::io::BufReader;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::str::FromStr;
use std::time::Duration;

use qapi_qmp::{
    self as qmp, BlockdevAioOptions, BlockdevOptions, BlockdevOptionsBase,
    BlockdevOptionsGenericFormat, BlockdevOptionsRaw, BlockdevRef, PciDeviceInfo,
};
use qapi_spec::Dictionary;

/// default qmp connection read timeout
const DEFAULT_QMP_READ_TIMEOUT: u64 = 250;

pub struct Qmp {
    qmp: qapi::Qmp<qapi::Stream<BufReader<UnixStream>, UnixStream>>,

    // This is basically the output of
    // `cat /sys/devices/system/memory/block_size_bytes`
    // on the guest.  Note a slightly peculiar behaviour with relation to
    // the size of hotplugged memory blocks: if an amount of memory is being
    // hotplugged whose size is not an integral multiple of page size
    // (4k usually) hotplugging fails immediately.  However, if the amount
    // is fine wrt the page size *but* isn't wrt this "guest memory block size"
    // hotplugging apparently succeeds, even though none of the hotplugged
    // blocks seem ever to be onlined in the guest by kata-agent.
    // Store as u64 to keep up the convention of bytes being represented as u64.
    guest_memory_block_size: u64,
}

// We have to implement Debug since the Hypervisor trait requires it and Qmp
// is ultimately stored in one of Hypervisor's implementations (Qemu).
// We can't do it automatically since the type of Qmp::qmp isn't Debug.
impl Debug for Qmp {
    fn fmt(&self, _f: &mut Formatter<'_>) -> Result<(), Error> {
        Ok(())
    }
}

impl Qmp {
    pub fn new(qmp_sock_path: &str) -> Result<Self> {
        let stream = UnixStream::connect(qmp_sock_path)?;

        // Set the read timeout to protect runtime-rs from blocking forever
        // trying to set up QMP connection if qemu fails to launch.  The exact
        // value is a matter of judegement.  Setting it too long would risk
        // being ineffective since container runtime would timeout first anyway
        // (containerd's task creation timeout is 2 s by default).  OTOH
        // setting it too short would risk interfering with a normal launch,
        // perhaps just seeing some delay due to a heavily loaded host.
        stream.set_read_timeout(Some(Duration::from_millis(DEFAULT_QMP_READ_TIMEOUT)))?;

        let mut qmp = Qmp {
            qmp: qapi::Qmp::new(qapi::Stream::new(
                BufReader::new(stream.try_clone()?),
                stream,
            )),
            guest_memory_block_size: 0,
        };

        let info = qmp.qmp.handshake()?;
        info!(sl!(), "QMP initialized: {:#?}", info);

        Ok(qmp)
    }

    pub fn hotplug_vcpus(&mut self, vcpu_cnt: u32) -> Result<u32> {
        let hotpluggable_cpus = self.qmp.execute(&qmp::query_hotpluggable_cpus {})?;
        //info!(sl!(), "hotpluggable CPUs: {:#?}", hotpluggable_cpus);

        let mut hotplugged = 0;
        for vcpu in &hotpluggable_cpus {
            if hotplugged >= vcpu_cnt {
                break;
            }
            let core_id = match vcpu.props.core_id {
                Some(id) => id,
                None => continue,
            };
            if vcpu.qom_path.is_some() {
                info!(sl!(), "hotpluggable vcpu {} hotplugged already", core_id);
                continue;
            }
            let socket_id = match vcpu.props.socket_id {
                Some(id) => id,
                None => continue,
            };
            let thread_id = match vcpu.props.thread_id {
                Some(id) => id,
                None => continue,
            };

            let mut cpu_args = Dictionary::new();
            cpu_args.insert("socket-id".to_owned(), socket_id.into());
            cpu_args.insert("core-id".to_owned(), core_id.into());
            cpu_args.insert("thread-id".to_owned(), thread_id.into());
            self.qmp.execute(&qmp::device_add {
                bus: None,
                id: Some(vcpu_id_from_core_id(core_id)),
                driver: hotpluggable_cpus[0].type_.clone(),
                arguments: cpu_args,
            })?;

            hotplugged += 1;
        }

        info!(
            sl!(),
            "Qmp::hotplug_vcpus(): hotplugged {}/{} vcpus", hotplugged, vcpu_cnt
        );

        Ok(hotplugged)
    }

    pub fn hotunplug_vcpus(&mut self, vcpu_cnt: u32) -> Result<u32> {
        let hotpluggable_cpus = self.qmp.execute(&qmp::query_hotpluggable_cpus {})?;

        let mut hotunplugged = 0;
        for vcpu in &hotpluggable_cpus {
            if hotunplugged >= vcpu_cnt {
                break;
            }
            let core_id = match vcpu.props.core_id {
                Some(id) => id,
                None => continue,
            };
            if vcpu.qom_path.is_none() {
                info!(sl!(), "hotpluggable vcpu {} not hotplugged yet", core_id);
                continue;
            }
            self.qmp.execute(&qmp::device_del {
                id: vcpu_id_from_core_id(core_id),
            })?;
            hotunplugged += 1;
        }

        info!(
            sl!(),
            "Qmp::hotunplug_vcpus(): hotunplugged {}/{} vcpus", hotunplugged, vcpu_cnt
        );

        Ok(hotunplugged)
    }

    pub fn set_guest_memory_block_size(&mut self, size: u64) {
        self.guest_memory_block_size = size;
    }

    pub fn guest_memory_block_size(&self) -> u64 {
        self.guest_memory_block_size
    }

    pub fn hotplugged_memory_size(&mut self) -> Result<u64> {
        let memory_frontends = self.qmp.execute(&qapi_qmp::query_memory_devices {})?;

        let mut hotplugged_mem_size = 0_u64;

        info!(sl!(), "hotplugged_memory_size(): iterating over dimms");
        for mem_frontend in &memory_frontends {
            if let qapi_qmp::MemoryDeviceInfo::dimm(dimm_info) = mem_frontend {
                let id = match dimm_info.data.id {
                    Some(ref id) => id.clone(),
                    None => "".to_owned(),
                };

                info!(
                    sl!(),
                    "dimm id: {} size={}, hotplugged: {}",
                    id,
                    dimm_info.data.size,
                    dimm_info.data.hotplugged
                );

                if dimm_info.data.hotpluggable && dimm_info.data.hotplugged {
                    hotplugged_mem_size += dimm_info.data.size as u64;
                }
            }
        }

        Ok(hotplugged_mem_size)
    }

    pub fn hotplug_memory(&mut self, size: u64) -> Result<()> {
        let memdev_idx = self
            .qmp
            .execute(&qapi_qmp::query_memory_devices {})?
            .into_iter()
            .filter(|memdev| {
                if let qapi_qmp::MemoryDeviceInfo::dimm(dimm_info) = memdev {
                    return dimm_info.data.hotpluggable && dimm_info.data.hotplugged;
                }
                false
            })
            .count();

        let memory_backend_id = format!("hotplugged-{}", memdev_idx);

        let memory_backend = qmp::object_add(qapi_qmp::ObjectOptions::memory_backend_file {
            id: memory_backend_id.clone(),
            memory_backend_file: qapi_qmp::MemoryBackendFileProperties {
                base: qapi_qmp::MemoryBackendProperties {
                    dump: None,
                    host_nodes: None,
                    merge: None,
                    policy: None,
                    prealloc: None,
                    prealloc_context: None,
                    prealloc_threads: None,
                    reserve: None,
                    share: Some(true),
                    x_use_canonical_path_for_ramblock_id: None,
                    size,
                },
                align: None,
                discard_data: None,
                offset: None,
                pmem: None,
                readonly: None,
                mem_path: "/dev/shm".to_owned(),
            },
        });
        self.qmp.execute(&memory_backend)?;

        let memory_frontend_id = format!("frontend-to-{}", memory_backend_id);

        let mut mem_frontend_args = Dictionary::new();
        mem_frontend_args.insert("memdev".to_owned(), memory_backend_id.into());
        self.qmp.execute(&qmp::device_add {
            bus: None,
            id: Some(memory_frontend_id),
            driver: "pc-dimm".to_owned(),
            arguments: mem_frontend_args,
        })?;

        Ok(())
    }

    pub fn hotunplug_memory(&mut self, size: i64) -> Result<()> {
        let frontend = self
            .qmp
            .execute(&qapi_qmp::query_memory_devices {})?
            .into_iter()
            .find(|memdev| {
                if let qapi_qmp::MemoryDeviceInfo::dimm(dimm_info) = memdev {
                    let dimm_id = match dimm_info.data.id {
                        Some(ref id) => id,
                        None => return false,
                    };
                    if dimm_info.data.hotpluggable
                        && dimm_info.data.hotplugged
                        && dimm_info.data.size == size
                        && dimm_id.starts_with("frontend-to-hotplugged-")
                    {
                        return true;
                    }
                }
                false
            });

        if let Some(frontend) = frontend {
            if let qapi_qmp::MemoryDeviceInfo::dimm(frontend) = frontend {
                info!(sl!(), "found frontend to hotunplug: {:#?}", frontend);

                let frontend_id = match frontend.data.id {
                    Some(id) => id,
                    // This shouldn't happen as it was checked by find() above already.
                    None => return Err(anyhow!("memory frontend to hotunplug has empty id")),
                };

                let backend_id = match frontend_id.strip_prefix("frontend-to-") {
                    Some(id) => id.to_owned(),
                    // This shouldn't happen as it was checked by find() above already.
                    None => {
                        return Err(anyhow!(
                        "memory backend to hotunplug has id that doesn't have the expected prefix"
                    ))
                    }
                };

                self.qmp.execute(&qmp::device_del { id: frontend_id })?;
                self.qmp.execute(&qmp::object_del { id: backend_id })?;
            } else {
                // This shouldn't happen as it was checked by find() above already.
                return Err(anyhow!("memory device to hotunplug is not a dimm"));
            }
        } else {
            return Err(anyhow!(
                "couldn't find a suitable memory device to hotunplug"
            ));
        }
        Ok(())
    }

    fn find_free_slot(&mut self) -> Result<(String, i64)> {
        let pci = self.qmp.execute(&qapi_qmp::query_pci {})?;
        for pci_info in &pci {
            for pci_dev in &pci_info.devices {
                let pci_bridge = match &pci_dev.pci_bridge {
                    Some(bridge) => bridge,
                    None => continue,
                };

                info!(sl!(), "found PCI bridge: {}", pci_dev.qdev_id);

                if let Some(bridge_devices) = &pci_bridge.devices {
                    let occupied_slots = bridge_devices
                        .iter()
                        .map(|pci_dev| pci_dev.slot)
                        .collect::<Vec<_>>();

                    info!(
                        sl!(),
                        "already occupied slots on bridge {}: {:#?}",
                        pci_dev.qdev_id,
                        occupied_slots
                    );

                    // from virtcontainers' bridges.go
                    let pci_bridge_max_capacity = 30;
                    for slot in 0..pci_bridge_max_capacity {
                        if !occupied_slots.iter().any(|elem| *elem == slot) {
                            info!(
                                sl!(),
                                "found free slot on bridge {}: {}", pci_dev.qdev_id, slot
                            );
                            return Ok((pci_dev.qdev_id.clone(), slot));
                        }
                    }
                }
            }
        }
        Err(anyhow!("no free slots on PCI bridges"))
    }

    fn pass_fd(&mut self, fd: RawFd, fdname: &str) -> Result<()> {
        info!(sl!(), "passing fd {:?} as {}", fd, fdname);

        // Put the QMP 'getfd' command itself into the message payload.
        let getfd_cmd = format!(
            "{{ \"execute\": \"getfd\", \"arguments\": {{ \"fdname\": \"{}\" }} }}",
            fdname
        );
        let buf = getfd_cmd.as_bytes();
        let bufs = &mut [std::io::IoSlice::new(buf)][..];

        debug!(sl!(), "bufs: {:?}", bufs);

        let fds = [fd];
        let cmsg = [ControlMessage::ScmRights(&fds)];

        let result = sendmsg::<()>(
            self.qmp.inner_mut().get_mut_write().as_raw_fd(),
            bufs,
            &cmsg,
            MsgFlags::empty(),
            None,
        );
        info!(sl!(), "sendmsg() result: {:#?}", result);

        let result = self.qmp.read_response::<&qmp::getfd>();

        match result {
            Ok(_) => {
                info!(sl!(), "successfully passed {} ({})", fdname, fd);
                Ok(())
            }
            Err(err) => Err(anyhow!("failed to pass {} ({}): {}", fdname, fd, err)),
        }
    }

    pub fn hotplug_network_device(
        &mut self,
        netdev: &Netdev,
        virtio_net_device: &DeviceVirtioNet,
    ) -> Result<()> {
        debug!(
            sl!(),
            "hotplug_network_device(): PCI before {}: {:#?}",
            virtio_net_device.get_netdev_id(),
            self.qmp.execute(&qapi_qmp::query_pci {})?
        );

        let (bus, slot) = self.find_free_slot()?;

        let mut fd_names = vec![];
        for (idx, fd) in netdev.get_fds().iter().enumerate() {
            let fdname = format!("fd{}", idx);
            self.pass_fd(fd.as_raw_fd(), fdname.as_ref())?;
            fd_names.push(fdname);
        }

        let mut vhostfd_names = vec![];
        for (idx, fd) in netdev.get_vhostfds().iter().enumerate() {
            let vhostfdname = format!("vhostfd{}", idx);
            self.pass_fd(fd.as_raw_fd(), vhostfdname.as_ref())?;
            vhostfd_names.push(vhostfdname);
        }

        self.qmp
            .execute(&qapi_qmp::netdev_add(qapi_qmp::Netdev::tap {
                id: netdev.get_id().clone(),
                tap: qapi_qmp::NetdevTapOptions {
                    br: None,
                    downscript: None,
                    fd: None,
                    // Logic in cmdline_generator::Netdev::new() seems to
                    // guarantee that there will always be at least one fd.
                    fds: Some(fd_names.join(",")),
                    helper: None,
                    ifname: None,
                    poll_us: None,
                    queues: None,
                    script: None,
                    sndbuf: None,
                    vhost: if vhostfd_names.is_empty() {
                        None
                    } else {
                        Some(true)
                    },
                    vhostfd: None,
                    vhostfds: if vhostfd_names.is_empty() {
                        None
                    } else {
                        Some(vhostfd_names.join(","))
                    },
                    vhostforce: None,
                    vnet_hdr: None,
                },
            }))?;

        let mut netdev_frontend_args = Dictionary::new();
        netdev_frontend_args.insert(
            "netdev".to_owned(),
            virtio_net_device.get_netdev_id().clone().into(),
        );
        netdev_frontend_args.insert("addr".to_owned(), format!("{:02}", slot).into());
        netdev_frontend_args.insert("mac".to_owned(), virtio_net_device.get_mac_addr().into());
        netdev_frontend_args.insert("mq".to_owned(), "on".into());
        // As the golang runtime documents the vectors computation, it's
        // 2N+2 vectors, N for tx queues, N for rx queues, 1 for config, and one for possible control vq
        netdev_frontend_args.insert(
            "vectors".to_owned(),
            (2 * virtio_net_device.get_num_queues() + 2).into(),
        );
        if virtio_net_device.get_disable_modern() {
            netdev_frontend_args.insert("disable-modern".to_owned(), true.into());
        }

        self.qmp.execute(&qmp::device_add {
            bus: Some(bus),
            id: Some(format!("frontend-{}", virtio_net_device.get_netdev_id())),
            driver: virtio_net_device.get_device_driver().clone(),
            arguments: netdev_frontend_args,
        })?;

        debug!(
            sl!(),
            "hotplug_network_device(): PCI after {}: {:#?}",
            virtio_net_device.get_netdev_id(),
            self.qmp.execute(&qapi_qmp::query_pci {})?
        );

        Ok(())
    }

    pub fn get_device_by_qdev_id(&mut self, qdev_id: &str) -> Result<PciPath> {
        let format_str = |vec: &Vec<i64>| -> String {
            vec.iter()
                .map(|num| format!("{:02x}", num))
                .collect::<Vec<String>>()
                .join("/")
        };

        let mut path = vec![];
        let pci = self.qmp.execute(&qapi_qmp::query_pci {})?;
        for pci_info in pci.iter() {
            if let Some(_device) = get_pci_path_by_qdev_id(&pci_info.devices, qdev_id, &mut path) {
                let pci_path = format_str(&path);
                return PciPath::try_from(pci_path.as_str());
            }
        }

        Err(anyhow!("no target device found"))
    }

    /// Hotplug block device:
    /// {
    ///     "execute": "blockdev-add",
    ///     "arguments": {
    ///         "node-name": "drive-0",
    ///         "file": {"driver": "file", "filename": "/path/to/block"},
    ///         "cache": {"direct": true},
    ///         "read-only": false
    ///     }
    /// }
    ///
    /// {
    ///     "execute": "device_add",
    ///     "arguments": {
    ///         "id": "drive-0",
    ///         "driver": "virtio-blk-pci",
    ///         "drive": "drive-0",
    ///         "addr":"0x0",
    ///         "bus": "pcie.1"
    ///     }
    /// }
    /// Hotplug SCSI block device
    /// # virtio-scsi0
    /// {"execute":"device_add","arguments":{"driver":"virtio-scsi-pci","id":"virtio-scsi0","bus":"bus1"}}
    /// {"return": {}}
    ///
    /// {"execute":"blockdev_add", "arguments": {"file":"/path/to/block.image","format":"qcow2","id":"virtio-scsi0"}}
    /// {"return": {}}
    /// {"execute":"device_add","arguments":{"driver":"scsi-hd","drive":"virtio-scsi0","id":"scsi_device_0","bus":"virtio-scsi1.0"}}
    /// {"return": {}}
    ///
    #[allow(clippy::too_many_arguments)]
    pub fn hotplug_block_device(
        &mut self,
        block_driver: &str,
        index: u64,
        path_on_host: &str,
        blkdev_aio: &str,
        is_direct: Option<bool>,
        is_readonly: bool,
        no_drop: bool,
    ) -> Result<(Option<PciPath>, Option<String>)> {
        // Helper closure to decode a flattened u16 SCSI index into an (ID, LUN) pair.
        let get_scsi_id_lun = |index_u16: u16| -> Result<(u8, u8)> {
            // Uses bitwise operations for efficient and clear conversion.
            let scsi_id = (index_u16 >> 8) as u8; // Equivalent to index_u16 / 256
            let lun = (index_u16 & 0xFF) as u8; // Equivalent to index_u16 % 256

            Ok((scsi_id, lun))
        };

        // `blockdev-add`
        let node_name = format!("drive-{index}");

        // Pre-check block drive and block device with qapi
        {
            let node_exists = self
                .qmp
                .execute(&qapi_qmp::query_named_block_nodes { flat: Some(true) })?
                .into_iter()
                .any(|d| d.node_name == Some(node_name.clone()));
            let device_exists = self
                .qmp
                .execute(&qapi_qmp::query_block {})?
                .into_iter()
                .any(|d| match d.inserted {
                    Some(node) => node.node_name == Some(node_name.clone()),
                    None => false,
                });

            if node_exists && device_exists {
                if block_driver == VIRTIO_SCSI {
                    // Safely convert the u64 index to u16, ensuring it does not exceed `u16::MAX` (65535).
                    let (scsi_id, lun) = get_scsi_id_lun(u16::try_from(index)?)?;
                    let scsi_addr = format!("{}:{}", scsi_id, lun);

                    return Ok((None, Some(scsi_addr)));
                } else {
                    let pci_path = self
                        .get_device_by_qdev_id(&node_name)
                        .context("get device by qdev_id failed")?;
                    info!(
                        sl!(),
                        "hotplug block device return pci path: {:?}", &pci_path
                    );

                    return Ok((Some(pci_path), None));
                }
            }

            if node_exists && !device_exists {
                warn!(
                    sl!(),
                    "Found orphaned backend node {:?}, do cleanup before retry.", &node_name
                );
                self.qmp
                    .execute(&qapi_qmp::blockdev_del {
                        node_name: node_name.clone(),
                    })
                    .ok();
            }
        }

        let create_base_options = || qapi_qmp::BlockdevOptionsBase {
            auto_read_only: None,
            cache: if is_direct.is_none() {
                None
            } else {
                Some(qapi_qmp::BlockdevCacheOptions {
                    direct: is_direct,
                    no_flush: None,
                })
            },
            detect_zeroes: None,
            discard: None,
            force_share: None,
            node_name: None,
            read_only: Some(is_readonly),
        };

        let create_backend_options = || qapi_qmp::BlockdevOptionsFile {
            aio: Some(
                BlockdevAioOptions::from_str(blkdev_aio).unwrap_or(BlockdevAioOptions::io_uring),
            ),
            aio_max_batch: None,
            drop_cache: if !no_drop { None } else { Some(no_drop) },
            locking: None,
            pr_manager: None,
            x_check_cache_dropped: None,
            filename: path_on_host.to_owned(),
        };

        // Add block device backend and check if the file is a regular file or device
        let blockdev_file = if std::fs::metadata(path_on_host)?.is_file() {
            // Regular file
            qmp::BlockdevOptions::file {
                base: create_base_options(),
                file: create_backend_options(),
            }
        } else {
            // Host device (e.g., /dev/sdx, /dev/loopX)
            qmp::BlockdevOptions::host_device {
                base: create_base_options(),
                host_device: create_backend_options(),
            }
        };

        let blockdev_options_raw = BlockdevOptions::raw {
            base: BlockdevOptionsBase {
                detect_zeroes: None,
                cache: None,
                discard: None,
                force_share: None,
                auto_read_only: None,
                node_name: Some(node_name.clone()),
                read_only: None,
            },
            raw: BlockdevOptionsRaw {
                base: BlockdevOptionsGenericFormat {
                    file: BlockdevRef::definition(Box::new(blockdev_file)),
                },
                offset: None,
                size: None,
            },
        };

        self.qmp
            .execute(&qapi_qmp::blockdev_add(blockdev_options_raw))
            .map_err(|e| anyhow!("blockdev-add backend {:?}", e))
            .map(|_| ())?;

        // block device
        // `device_add`
        let mut blkdev_add_args = Dictionary::new();
        blkdev_add_args.insert("drive".to_owned(), node_name.clone().into());

        if block_driver == VIRTIO_SCSI {
            // Safely convert the u64 index to u16, ensuring it does not exceed `u16::MAX` (65535).
            let (scsi_id, lun) = get_scsi_id_lun(u16::try_from(index)?)?;
            let scsi_addr = format!("{}:{}", scsi_id, lun);

            // add SCSI frontend device
            blkdev_add_args.insert("scsi-id".to_string(), scsi_id.into());
            blkdev_add_args.insert("lun".to_string(), lun.into());
            blkdev_add_args.insert("share-rw".to_string(), true.into());

            self.qmp
                .execute(&qmp::device_add {
                    bus: Some("scsi0.0".to_string()),
                    id: Some(node_name.clone()),
                    driver: "scsi-hd".to_string(),
                    arguments: blkdev_add_args,
                })
                .map_err(|e| anyhow!("device_add {:?}", e))
                .map(|_| ())?;

            info!(
                sl!(),
                "hotplug scsi block device return scsi address: {:?}", &scsi_addr
            );

            Ok((None, Some(scsi_addr)))
        } else {
            let (bus, slot) = self.find_free_slot()?;
            blkdev_add_args.insert("addr".to_owned(), format!("{:02}", slot).into());
            blkdev_add_args.insert("share-rw".to_string(), true.into());

            self.qmp
                .execute(&qmp::device_add {
                    bus: Some(bus),
                    id: Some(node_name.clone()),
                    driver: block_driver.to_string(),
                    arguments: blkdev_add_args,
                })
                .map_err(|e| anyhow!("device_add {:?}", e))
                .map(|_| ())?;

            let pci_path = self
                .get_device_by_qdev_id(&node_name)
                .context("get device by qdev_id failed")?;
            info!(
                sl!(),
                "hotplug block device return pci path: {:?}", &pci_path
            );

            Ok((Some(pci_path), None))
        }
    }

    pub fn hotplug_vfio_device(
        &mut self,
        hostdev_id: &str,
        bus_slot_func: &str,
        driver: &str,
        bus: &str,
    ) -> Result<Option<PciPath>> {
        let mut vfio_args = Dictionary::new();
        let bdf = if !bus_slot_func.starts_with("0000") {
            format!("0000:{}", bus_slot_func)
        } else {
            bus_slot_func.to_owned()
        };
        vfio_args.insert("addr".to_owned(), "0x0".into());
        vfio_args.insert("host".to_owned(), bdf.into());
        vfio_args.insert("multifunction".to_owned(), "off".into());

        let vfio_device_add = qmp::device_add {
            driver: driver.to_string(),
            bus: Some(bus.to_string()),
            id: Some(hostdev_id.to_string()),
            arguments: vfio_args,
        };
        info!(sl!(), "vfio_device_add: {:?}", vfio_device_add.clone());

        // We've chosen to set a 5-second read timeout on Unix sockets for QMP operations. We consider set_read_timeout()
        // a lightweight operation that shouldn't significantly impact performance, even with multiple VFIO devices.
        // However, we also need to ensure its debuggability.
        // As it could obscure the root cause of connection failures as set an excessively long QMP timeout.
        // For example, if QEMU fails to launch, a 5-second QMP timeout will immediately provide a "QMP connection failed" log message,
        // clearly pinpointing the issue. Conversely, a prolonged timeout might only result in vague error messages, making debugging
        // difficult as it won't explicitly indicate where the problem lies.

        // Given our current inability to comprehensively test across a wide range of hardware and configurations, we've made a pragmatic
        // decision: we'll maintain the 5-second timeout for now. A configurable timeout option will be introduced if future use cases
        // clearly demonstrate a justified need.
        {
            // set read timeout with 5000
            self.qmp
                .inner_mut()
                .get_mut_write()
                .set_read_timeout(Some(Duration::from_millis(5000)))?;
            // send the VFIO hotplug request
            self.qmp
                .execute(&vfio_device_add)
                .map_err(|e| anyhow!("device_add vfio device failed {:?}", e))?;
            // reset read timeout with 250
            self.qmp
                .inner_mut()
                .get_mut_write()
                .set_read_timeout(Some(Duration::from_millis(DEFAULT_QMP_READ_TIMEOUT)))?;
        }

        let pci_path = self
            .get_device_by_qdev_id(hostdev_id)
            .context("get device by qdev_id failed")?;

        Ok(Some(pci_path))
    }

    /// Get vCPU thread IDs through QMP query_cpus_fast.
    pub fn get_vcpu_thread_ids(&mut self) -> Result<VcpuThreadIds> {
        let vcpu_info = self
            .qmp
            .execute(&qmp::query_cpus_fast {})
            .map_err(|e| anyhow!("query_cpus_fast failed: {:?}", e))?;

        let vcpus: HashMap<u32, u32> = vcpu_info
            .iter()
            .map(|info| match info {
                qmp::CpuInfoFast::aarch64(cpu_info)
                | qmp::CpuInfoFast::alpha(cpu_info)
                | qmp::CpuInfoFast::arm(cpu_info)
                | qmp::CpuInfoFast::avr(cpu_info)
                | qmp::CpuInfoFast::cris(cpu_info)
                | qmp::CpuInfoFast::hppa(cpu_info)
                | qmp::CpuInfoFast::i386(cpu_info)
                | qmp::CpuInfoFast::loongarch64(cpu_info)
                | qmp::CpuInfoFast::m68k(cpu_info)
                | qmp::CpuInfoFast::microblaze(cpu_info)
                | qmp::CpuInfoFast::microblazeel(cpu_info)
                | qmp::CpuInfoFast::mips(cpu_info)
                | qmp::CpuInfoFast::mips64(cpu_info)
                | qmp::CpuInfoFast::mips64el(cpu_info)
                | qmp::CpuInfoFast::mipsel(cpu_info)
                | qmp::CpuInfoFast::nios2(cpu_info)
                | qmp::CpuInfoFast::or1k(cpu_info)
                | qmp::CpuInfoFast::ppc(cpu_info)
                | qmp::CpuInfoFast::ppc64(cpu_info)
                | qmp::CpuInfoFast::riscv32(cpu_info)
                | qmp::CpuInfoFast::riscv64(cpu_info)
                | qmp::CpuInfoFast::rx(cpu_info)
                | qmp::CpuInfoFast::sh4(cpu_info)
                | qmp::CpuInfoFast::sh4eb(cpu_info)
                | qmp::CpuInfoFast::sparc(cpu_info)
                | qmp::CpuInfoFast::sparc64(cpu_info)
                | qmp::CpuInfoFast::tricore(cpu_info)
                | qmp::CpuInfoFast::x86_64(cpu_info)
                | qmp::CpuInfoFast::xtensa(cpu_info)
                | qmp::CpuInfoFast::xtensaeb(cpu_info) => {
                    let vcpu_id = cpu_info.cpu_index as u32;
                    let thread_id = cpu_info.thread_id as u32;
                    (vcpu_id, thread_id)
                }
                qmp::CpuInfoFast::s390x { base, .. } => {
                    let vcpu_id = base.cpu_index as u32;
                    let thread_id = base.thread_id as u32;
                    (vcpu_id, thread_id)
                }
            })
            .collect();

        Ok(VcpuThreadIds { vcpus })
    }
}

fn vcpu_id_from_core_id(core_id: i64) -> String {
    format!("cpu-{}", core_id)
}

// The get_pci_path_by_qdev_id function searches a device list for a device matching a given qdev_id,
// tracking the device's path. It recursively explores bridge devices and returns the found device along
// with its updated path.
pub fn get_pci_path_by_qdev_id(
    devices: &[PciDeviceInfo],
    qdev_id: &str,
    path: &mut Vec<i64>,
) -> Option<PciDeviceInfo> {
    for device in devices {
        path.push(device.slot);
        if device.qdev_id == qdev_id {
            return Some(device.clone());
        }

        if let Some(ref bridge) = device.pci_bridge {
            if let Some(ref bridge_devices) = bridge.devices {
                if let Some(found_device) = get_pci_path_by_qdev_id(bridge_devices, qdev_id, path) {
                    return Some(found_device);
                }
            }
        }

        // If the device not found, pop the current slot before moving to next device
        path.pop();
    }
    None
}

pub fn get_qmp_socket_path(sid: &str) -> String {
    if is_rootless() {
        [get_jailer_root(sid).as_str(), QMP_SOCKET_FILE].join("/")
    } else {
        QMP_SOCKET_FILE.to_string()
    }
}
