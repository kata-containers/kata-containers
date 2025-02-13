// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Helper utilities for vhost-user communication channel.

use std::ops::Deref;
use std::os::unix::io::{AsRawFd, RawFd};

use dbs_utils::epoll_manager::{EventOps, EventSet, Events};
use log::*;
use vhost_rs::vhost_user::message::{VhostUserProtocolFeatures, VhostUserVringAddrFlags};
use vhost_rs::vhost_user::{
    Error as VhostUserError, Listener as VhostUserListener, Master, VhostUserMaster,
};
use vhost_rs::{Error as VhostError, VhostBackend, VhostUserMemoryRegionInfo, VringConfigData};
use virtio_bindings::bindings::virtio_net::VIRTIO_F_RING_PACKED;
use virtio_queue::QueueT;
use vm_memory::{
    Address, GuestAddress, GuestAddressSpace, GuestMemory, GuestMemoryRegion, MemoryRegionAddress,
};
use vmm_sys_util::eventfd::EventFd;

use crate::device::VirtioDeviceConfig;
use crate::{Error as VirtioError, Result as VirtioResult};

enum EndpointProtocolFlags {
    ProtocolMq = 1,
    #[allow(dead_code)]
    #[cfg(feature = "vhost-user-blk")]
    ProtocolBackend = 2,
}

pub(super) struct Listener {
    listener: VhostUserListener,
    /// Slot to register epoll event for the underlying socket.
    slot: u32,
    name: String,
    path: String,
}

impl Listener {
    pub fn new(name: String, path: String, force: bool, slot: u32) -> VirtioResult<Self> {
        info!("vhost-user: create listener at {} for {}", path, name);
        Ok(Listener {
            listener: VhostUserListener::new(&path, force)?,
            slot,
            name,
            path,
        })
    }

    // Wait for an incoming connection until success.
    pub fn accept(&self) -> VirtioResult<(Master, u64)> {
        loop {
            match self.try_accept() {
                Ok(Some((master, mut feature))) => {
                    // Disable VIRTIO_F_RING_PACKED since the layout of packed virtqueue isn't
                    // supported by `Endpoint::negotiate()`.
                    feature &= !(1 << VIRTIO_F_RING_PACKED);
                    return Ok((master, feature));
                }
                Ok(None) => continue,
                Err(e) => return Err(e),
            }
        }
    }

    pub fn try_accept(&self) -> VirtioResult<Option<(Master, u64)>> {
        let sock = match self.listener.accept() {
            Ok(Some(conn)) => conn,
            Ok(None) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let mut master = Master::from_stream(sock, 1);
        info!("{}: try to get virtio features from slave.", self.name);
        match Endpoint::initialize(&mut master) {
            Ok(Some(features)) => Ok(Some((master, features))),
            // The new connection has been closed, try again.
            Ok(None) => {
                warn!(
                    "{}: new connection get closed during initialization, waiting for another one.",
                    self.name
                );
                Ok(None)
            }
            // Unrecoverable error happened
            Err(e) => {
                error!("{}: failed to get virtio features, {}", self.name, e);
                Err(e)
            }
        }
    }

    /// Register the underlying listener to be monitored for incoming connection.
    pub fn register_epoll_event(&self, ops: &mut EventOps) -> VirtioResult<()> {
        info!("{}: monitor incoming connect at {}", self.name, self.path);
        // Switch to nonblocking mode.
        self.listener.set_nonblocking(true)?;
        let event = Events::with_data(&self.listener, self.slot, EventSet::IN);
        ops.add(event).map_err(VirtioError::EpollMgr)
    }
}

/// TODO: Mark as unused as it is used by vhost-user-block devices only.
/// Struct to pass info to vhost user backend
#[allow(dead_code)]
#[derive(Clone)]
pub struct BackendInfo {
    /// -1 means to tell backend to destroy corresponding
    /// device, while others means construct it
    fd: i32,
    /// cluster id of device, must set
    cluster_id: u32,
    /// device id of device, must set
    device_id: u64,
    /// device config file path
    filename: [u8; 128],
}

/// Struct to pass function parameters to methods of Endpoint.
pub(super) struct EndpointParam<'a, AS: GuestAddressSpace, Q: QueueT, R: GuestMemoryRegion> {
    pub virtio_config: &'a VirtioDeviceConfig<AS, Q, R>,
    pub intr_evts: Vec<&'a EventFd>,
    pub queue_sizes: &'a [u16],
    pub features: u64,
    pub protocol_flag: u16,
    pub dev_protocol_features: VhostUserProtocolFeatures,
    pub reconnect: bool,
    // TODO: Mark as unused as it is used by vhost-user-block only.
    #[allow(dead_code)]
    pub backend: Option<BackendInfo>,
    pub init_queues: u32,
    pub slave_req_fd: Option<RawFd>,
}

impl<'a, AS: GuestAddressSpace, Q: QueueT, R: GuestMemoryRegion> EndpointParam<'a, AS, Q, R> {
    fn get_host_address(&self, addr: GuestAddress, mem: &AS::M) -> VirtioResult<*mut u8> {
        mem.get_host_address(addr)
            .map_err(|_| VirtioError::InvalidGuestAddress(addr))
    }

    /// set protocol multi-queue bit
    pub fn set_protocol_mq(&mut self) {
        self.protocol_flag |= EndpointProtocolFlags::ProtocolMq as u16;
    }

    /// check if multi-queue bit is set
    pub fn has_protocol_mq(&self) -> bool {
        (self.protocol_flag & (EndpointProtocolFlags::ProtocolMq as u16)) != 0
    }
}

/// Communication channel from the master to the slave.
///
/// It encapsulates a low-level vhost-user master side communication endpoint, and provides
/// connection initialization, monitoring and reconnect functionalities for vhost-user devices.
///
/// Caller needs to ensure mutual exclusive access to the object.
pub(super) struct Endpoint {
    /// Underlying vhost-user communication endpoint.
    conn: Option<Master>,
    old: Option<Master>,
    /// Token to register epoll event for the underlying socket.
    slot: u32,
    /// Identifier string for logs.
    name: String,
}

impl Endpoint {
    pub fn new(master: Master, slot: u32, name: String) -> Self {
        Endpoint {
            conn: Some(master),
            old: None,
            slot,
            name,
        }
    }

    /// First state of the connection negotiation between the master and the slave.
    ///
    /// If Ok(None) is returned, the underlying communication channel gets broken and the caller may
    /// try to recreate the communication channel and negotiate again.
    ///
    /// # Return
    /// * - Ok(Some(avial_features)): virtio features from the slave
    /// * - Ok(None): underlying communicaiton channel gets broken during negotiation
    /// * - Err(e): error conditions
    fn initialize(master: &mut Master) -> VirtioResult<Option<u64>> {
        // 1. Seems that some vhost-user slaves depend on the get_features request to driver its
        // internal state machine.
        // N.B. it's really TDD, we just found it works in this way. Any spec about this?
        let features = match master.get_features() {
            Ok(val) => val,
            Err(VhostError::VhostUserProtocol(VhostUserError::SocketBroken(_e))) => {
                return Ok(None)
            }
            Err(e) => return Err(e.into()),
        };

        Ok(Some(features))
    }

    // TODO: Remove this after enabling vhost-user-fs on the runtime-rs. Issue:
    // https://github.com/kata-containers/kata-containers/issues/8691
    #[allow(dead_code)]
    pub fn update_memory<AS: GuestAddressSpace>(&mut self, vm_as: &AS) -> VirtioResult<()> {
        let master = match self.conn.as_mut() {
            Some(conn) => conn,
            None => {
                error!("vhost user master is None!");
                return Err(VirtioError::InternalError);
            }
        };
        let guard = vm_as.memory();
        let mem = guard.deref();
        let mut regions = Vec::new();
        for region in mem.iter() {
            let guest_phys_addr = region.start_addr();
            let file_offset = region.file_offset().ok_or_else(|| {
                error!("region file_offset get error!");
                VirtioError::InvalidGuestAddress(guest_phys_addr)
            })?;
            let userspace_addr = region
                .get_host_address(MemoryRegionAddress(0))
                .map_err(|e| {
                    error!("get_host_address error! {:?}", e);
                    VirtioError::InvalidGuestAddress(guest_phys_addr)
                })?;

            regions.push(VhostUserMemoryRegionInfo {
                guest_phys_addr: guest_phys_addr.raw_value(),
                memory_size: region.len(),
                userspace_addr: userspace_addr as *const u8 as u64,
                mmap_offset: file_offset.start(),
                mmap_handle: file_offset.file().as_raw_fd(),
            });
        }
        master.set_mem_table(&regions)?;
        Ok(())
    }

    /// Drive the negotiation and initialization process with the vhost-user slave.
    pub fn negotiate<AS: GuestAddressSpace, Q: QueueT, R: GuestMemoryRegion>(
        &mut self,
        config: &EndpointParam<AS, Q, R>,
        mut old: Option<&mut Master>,
    ) -> VirtioResult<()> {
        let guard = config.virtio_config.lock_guest_memory();
        let mem = guard.deref();
        let queue_num = config.virtio_config.queues.len();
        assert_eq!(queue_num, config.queue_sizes.len());
        assert_eq!(queue_num, config.intr_evts.len());

        let master = match self.conn.as_mut() {
            Some(conn) => conn,
            None => return Err(VirtioError::InternalError),
        };

        info!("{}: negotiate()", self.name);
        master.set_owner()?;
        info!("{}: set_owner()", self.name);

        // 3. query features again after set owner.
        let features = master.get_features()?;
        info!("{}: get_features({:X})", self.name, features);

        // 4. set virtio features.
        master.set_features(config.features)?;
        info!("{}: set_features({:X})", self.name, config.features);

        // 5. set vhost-user protocol features
        // typical protocol features: 0x37
        let mut protocol_features = master.get_protocol_features()?;
        info!(
            "{}: get_protocol_features({:X})",
            self.name, protocol_features
        );
        // There are two virtque for rx/tx.
        if config.has_protocol_mq() && !protocol_features.contains(VhostUserProtocolFeatures::MQ) {
            return Err(VhostError::VhostUserProtocol(VhostUserError::FeatureMismatch).into());
        }
        protocol_features &= config.dev_protocol_features;
        master.set_protocol_features(protocol_features)?;
        info!(
            "{}: set_protocol_features({:X}), dev_protocol_features({:X})",
            self.name, protocol_features, config.dev_protocol_features
        );

        // Setup slave channel if SLAVE_REQ protocol feature is set
        if protocol_features.contains(VhostUserProtocolFeatures::SLAVE_REQ) {
            match config.slave_req_fd {
                Some(fd) => master.set_slave_request_fd(&fd)?,
                None => {
                    error!(
                        "{}: Protocol feature SLAVE_REQ is set but not slave channel fd",
                        self.name
                    );
                    return Err(VhostError::VhostUserProtocol(VhostUserError::InvalidParam).into());
                }
            }
        } else {
            info!("{}: has no SLAVE_REQ protocol feature set", self.name);
        }

        // 6. check number of queues supported
        if config.has_protocol_mq() {
            let queue_num = master.get_queue_num()?;
            info!("{}: get_queue_num({:X})", self.name, queue_num);
            if queue_num < config.queue_sizes.len() as u64 {
                return Err(VhostError::VhostUserProtocol(VhostUserError::FeatureMismatch).into());
            }
        }

        // 7. trigger the backend state machine.
        for queue_index in 0..queue_num {
            master.set_vring_call(queue_index, config.intr_evts[queue_index])?;
        }
        info!("{}: set_vring_call()", self.name);

        // 8. set mem_table
        let mut regions = Vec::new();
        for region in mem.iter() {
            let guest_phys_addr = region.start_addr();
            let file_offset = region
                .file_offset()
                .ok_or(VirtioError::InvalidGuestAddress(guest_phys_addr))?;
            let userspace_addr = region
                .get_host_address(MemoryRegionAddress(0))
                .map_err(|_| VirtioError::InvalidGuestAddress(guest_phys_addr))?;

            regions.push(VhostUserMemoryRegionInfo {
                guest_phys_addr: guest_phys_addr.raw_value(),
                memory_size: region.len(),
                userspace_addr: userspace_addr as *const u8 as u64,
                mmap_offset: file_offset.start(),
                mmap_handle: file_offset.file().as_raw_fd(),
            });
        }
        master.set_mem_table(&regions)?;
        info!("{}: set_mem_table()", self.name);

        // 9. setup vrings
        for queue_cfg in config.virtio_config.queues.iter() {
            master.set_vring_num(queue_cfg.index() as usize, queue_cfg.actual_size())?;
            info!(
                "{}: set_vring_num(idx: {}, size: {})",
                self.name,
                queue_cfg.index(),
                queue_cfg.actual_size(),
            );
        }
        // On reconnection, the slave may have processed some packets in virtque and queue
        // base is not zero any more. So don't set queue base on reconnection.
        // N.B. it's really TDD, we just found it works in this way. Any spec about this?
        for queue_index in 0..queue_num {
            let base = if old.is_some() {
                let conn = old.as_mut().unwrap();
                match conn.get_vring_base(queue_index) {
                    Ok(val) => Some(val),
                    Err(_) => None,
                }
            } else if !config.reconnect {
                Some(0)
            } else {
                None
            };
            if let Some(val) = base {
                master.set_vring_base(queue_index, val as u16)?;
                info!(
                    "{}: set_vring_base(idx: {}, base: {})",
                    self.name, queue_index, val
                );
            }
        }
        for queue_cfg in config.virtio_config.queues.iter() {
            let queue = &queue_cfg.queue;
            let queue_index = queue_cfg.index() as usize;
            let desc_addr =
                config.get_host_address(vm_memory::GuestAddress(queue.desc_table()), mem)?;
            let used_addr =
                config.get_host_address(vm_memory::GuestAddress(queue.used_ring()), mem)?;
            let avail_addr =
                config.get_host_address(vm_memory::GuestAddress(queue.avail_ring()), mem)?;
            master.set_vring_addr(
                queue_index,
                &VringConfigData {
                    queue_max_size: queue.max_size(),
                    queue_size: queue_cfg.actual_size(),
                    flags: VhostUserVringAddrFlags::empty().bits(),
                    desc_table_addr: desc_addr as u64,
                    used_ring_addr: used_addr as u64,
                    avail_ring_addr: avail_addr as u64,
                    log_addr: None,
                },
            )?;
            info!(
                "{}: set_vring_addr(idx: {}, addr: {:p})",
                self.name, queue_index, desc_addr
            );
        }
        for queue_index in 0..queue_num {
            master.set_vring_kick(
                queue_index,
                &config.virtio_config.queues[queue_index].eventfd,
            )?;
            info!(
                "{}: set_vring_kick(idx: {}, fd: {})",
                self.name,
                queue_index,
                config.virtio_config.queues[queue_index].eventfd.as_raw_fd()
            );
        }
        for queue_index in 0..queue_num {
            let intr_index = if config.intr_evts.len() == 1 {
                0
            } else {
                queue_index
            };
            master.set_vring_call(queue_index, config.intr_evts[intr_index])?;
            info!(
                "{}: set_vring_call(idx: {}, fd: {})",
                self.name,
                queue_index,
                config.intr_evts[intr_index].as_raw_fd()
            );
        }
        for queue_index in 0..queue_num {
            master.set_vring_enable(queue_index, true)?;
            info!(
                "{}: set_vring_enable(idx: {}, enable: {})",
                self.name, queue_index, true
            );
            if (queue_index + 1) == config.init_queues as usize {
                break;
            }
        }
        info!("{}: protocol negotiate completed successfully.", self.name);

        Ok(())
    }

    pub fn set_queues_attach(&mut self, curr_queues: u32) -> VirtioResult<()> {
        let master = match self.conn.as_mut() {
            Some(conn) => conn,
            None => return Err(VirtioError::InternalError),
        };

        for index in 0..curr_queues {
            master.set_vring_enable(index as usize, true)?;
            info!(
                "{}: set_vring_enable(idx: {}, enable: {})",
                self.name, index, true
            );
        }

        Ok(())
    }

    /// Restore communication with the vhost-user slave on reconnect.
    pub fn reconnect<AS: GuestAddressSpace, Q: QueueT, R: GuestMemoryRegion>(
        &mut self,
        master: Master,
        config: &EndpointParam<AS, Q, R>,
        ops: &mut EventOps,
    ) -> VirtioResult<()> {
        let mut old = self.conn.replace(master);
        if let Err(e) = self.negotiate(config, old.as_mut()) {
            error!("{}: failed to initialize connection: {}", self.name, e);
            self.conn = old;
            return Err(e);
        }
        if let Err(e) = self.register_epoll_event(ops) {
            error!("{}: failed to add fd to epoll: {}", self.name, e);
            self.conn = old;
            return Err(e);
        }
        self.old = old;
        Ok(())
    }

    /// Teardown the communication channel to the vhost-user slave.
    pub fn disconnect(&mut self, ops: &mut EventOps) -> VirtioResult<()> {
        info!("vhost-user-net: disconnect communication channel.");
        match self.old.take() {
            Some(master) => {
                info!("close old connection");
                self.deregister_epoll_event(&master, ops)
            }
            None => match self.conn.take() {
                Some(master) => {
                    info!("disconnect connection.");
                    self.deregister_epoll_event(&master, ops)
                }
                None => {
                    info!("get disconnect notification when it's already disconnected.");
                    Ok(())
                }
            },
        }
    }

    /// Register the underlying socket to be monitored for socket disconnect events.
    pub fn register_epoll_event(&self, ops: &mut EventOps) -> VirtioResult<()> {
        match self.conn.as_ref() {
            Some(master) => {
                info!(
                    "{}: monitor disconnect event for fd {}.",
                    self.name,
                    master.as_raw_fd()
                );
                ops.add(Events::with_data(
                    master,
                    self.slot,
                    EventSet::HANG_UP | EventSet::EDGE_TRIGGERED,
                ))
                .map_err(VirtioError::EpollMgr)
            }
            None => Err(VirtioError::InternalError),
        }
    }

    /// Deregister the underlying socket from the epoll controller.
    pub fn deregister_epoll_event(&self, master: &Master, ops: &mut EventOps) -> VirtioResult<()> {
        info!(
            "{}: unregister epoll event for fd {}.",
            self.name,
            master.as_raw_fd()
        );
        ops.remove(Events::with_data(
            master,
            self.slot,
            EventSet::HANG_UP | EventSet::EDGE_TRIGGERED,
        ))
        .map_err(VirtioError::EpollMgr)
    }

    pub fn set_master(&mut self, master: Master) {
        self.conn = Some(master);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_flags() {
        assert_eq!(EndpointProtocolFlags::ProtocolMq as u16, 0x1);
    }

    #[should_panic]
    #[test]
    fn test_connect_try_accept() {
        let listener = Listener::new(
            "test_listener".to_string(),
            "/tmp/test_vhost_listener".to_string(),
            true,
            1,
        )
        .unwrap();

        listener.listener.set_nonblocking(true).unwrap();

        assert!(listener.try_accept().is_err());
    }
}
