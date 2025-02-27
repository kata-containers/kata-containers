// Copyright (c) 2024 Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::qemu::cmdline_generator::{DeviceVirtioNet, Netdev};

use anyhow::{anyhow, Result};
use nix::sys::socket::{sendmsg, ControlMessage, MsgFlags};
use std::fmt::{Debug, Error, Formatter};
use std::io::BufReader;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use qapi::qmp;
use qapi_qmp;
use qapi_spec::Dictionary;

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
        stream.set_read_timeout(Some(Duration::from_millis(250)))?;

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
}

fn vcpu_id_from_core_id(core_id: i64) -> String {
    format!("cpu-{}", core_id)
}
