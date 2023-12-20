// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// Copyright 2019 Intel Corporation. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::Any;
use std::io;
use std::marker::PhantomData;
use std::ops::Deref;
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex, MutexGuard};

use dbs_device::resources::{DeviceResources, ResourceConstraint};
use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};
use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::VmFd;
use libc::{c_void, off64_t, pread64, pwrite64};
use log::*;
use vhost_rs::vhost_user::message::{
    VhostUserFSSlaveMsg, VhostUserFSSlaveMsgFlags, VhostUserProtocolFeatures,
    VhostUserVirtioFeatures, VHOST_USER_FS_SLAVE_ENTRIES,
};
use vhost_rs::vhost_user::{HandlerResult, Master, MasterReqHandler, VhostUserMasterReqHandler};
use vhost_rs::VhostBackend;
use virtio_queue::QueueT;
use vm_memory::{
    GuestAddress, GuestAddressSpace, GuestMemory, GuestMemoryRegion, GuestRegionMmap, GuestUsize,
    MmapRegion,
};

use crate::ConfigResult;

use super::super::super::device::{VirtioDevice, VirtioDeviceConfig, VirtioDeviceInfo};
use super::super::super::{
    ActivateError, ActivateResult, Error as VirtioError, Result as VirtioResult,
    VirtioSharedMemory, VirtioSharedMemoryList, TYPE_VIRTIO_FS,
};
use super::connection::{Endpoint, EndpointParam};

const VHOST_USER_FS_NAME: &str = "vhost-user-fs";

const CONFIG_SPACE_TAG_SIZE: usize = 36;
const CONFIG_SPACE_NUM_QUEUES_SIZE: usize = 4;
const CONFIG_SPACE_SIZE: usize = CONFIG_SPACE_TAG_SIZE + CONFIG_SPACE_NUM_QUEUES_SIZE;

// TODO: need documentation for this, why we need an extra queue?
const NUM_QUEUE_OFFSET: usize = 1;

const MASTER_SLOT: u32 = 0;
const SLAVE_REQ_SLOT: u32 = 1;

struct SlaveReqHandler<AS: GuestAddressSpace> {
    /// the address of memory region allocated for virtiofs
    cache_offset: u64,

    /// the size of memory region allocated for virtiofs
    cache_size: u64,

    /// the address of mmap region corresponding to the memory region
    mmap_cache_addr: u64,

    /// the guest memory mapping
    mem: AS,

    /// the device ID
    id: String,
}

impl<AS: GuestAddressSpace> SlaveReqHandler<AS> {
    // Make sure request is within cache range
    fn is_req_valid(&self, offset: u64, len: u64) -> bool {
        // TODO: do we need to validate alignment here?
        match offset.checked_add(len) {
            Some(n) => n <= self.cache_size,
            None => false,
        }
    }
}

impl<AS: GuestAddressSpace> VhostUserMasterReqHandler for SlaveReqHandler<AS> {
    fn handle_config_change(&self) -> HandlerResult<u64> {
        trace!(target: "vhost-fs", "{}: SlaveReqHandler::handle_config_change()", self.id);
        debug!("{}: unhandle device_config_change event", self.id);

        Ok(0)
    }

    fn fs_slave_map(&self, fs: &VhostUserFSSlaveMsg, fd: &dyn AsRawFd) -> HandlerResult<u64> {
        trace!(target: "vhost-fs", "{}: SlaveReqHandler::fs_slave_map()", self.id);

        for i in 0..VHOST_USER_FS_SLAVE_ENTRIES {
            let offset = fs.cache_offset[i];
            let len = fs.len[i];

            // Ignore if the length is 0.
            if len == 0 {
                continue;
            }

            debug!(
                "{}: fs_slave_map: offset={:x} len={:x} cache_size={:x}",
                self.id, offset, len, self.cache_size
            );

            if !self.is_req_valid(offset, len) {
                debug!(
                    "{}: fs_slave_map: Wrong offset or length, offset={:x} len={:x} cache_size={:x}",
                    self.id, offset, len, self.cache_size
                );
                return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
            }

            let addr = self.mmap_cache_addr + offset;
            let flags = fs.flags[i];
            let ret = unsafe {
                libc::mmap(
                    addr as *mut libc::c_void,
                    len as usize,
                    flags.bits() as i32,
                    libc::MAP_SHARED | libc::MAP_FIXED,
                    fd.as_raw_fd(),
                    fs.fd_offset[i] as libc::off_t,
                )
            };
            if ret == libc::MAP_FAILED {
                let e = std::io::Error::last_os_error();
                error!("{}: fs_slave_map: mmap failed, {}", self.id, e);
                return Err(e);
            }

            let ret = unsafe { libc::close(fd.as_raw_fd()) };
            if ret == -1 {
                let e = std::io::Error::last_os_error();
                error!("{}: fs_slave_map: close failed, {}", self.id, e);
                return Err(e);
            }
        }

        Ok(0)
    }

    fn fs_slave_unmap(&self, fs: &VhostUserFSSlaveMsg) -> HandlerResult<u64> {
        trace!(target: "vhost-fs", "{}: SlaveReqHandler::fs_slave_map()", self.id);

        for i in 0..VHOST_USER_FS_SLAVE_ENTRIES {
            let offset = fs.cache_offset[i];
            let mut len = fs.len[i];

            // Ignore if the length is 0.
            if len == 0 {
                continue;
            }

            debug!(
                "{}: fs_slave_unmap: offset={:x} len={:x} cache_size={:x}",
                self.id, offset, len, self.cache_size
            );

            // Need to handle a special case where the slave ask for the unmapping
            // of the entire mapping.
            if len == 0xffff_ffff_ffff_ffff {
                len = self.cache_size;
            }

            if !self.is_req_valid(offset, len) {
                error!(
                    "{}: fs_slave_map: Wrong offset or length, offset={:x} len={:x} cache_size={:x}",
                    self.id, offset, len, self.cache_size
                );
                return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
            }

            let addr = self.mmap_cache_addr + offset;
            #[allow(clippy::unnecessary_cast)]
            let ret = unsafe {
                libc::mmap(
                    addr as *mut libc::c_void,
                    len as usize,
                    libc::PROT_NONE,
                    libc::MAP_ANONYMOUS | libc::MAP_PRIVATE | libc::MAP_FIXED,
                    -1,
                    0 as libc::off_t,
                )
            };
            if ret == libc::MAP_FAILED {
                let e = std::io::Error::last_os_error();
                error!("{}: fs_slave_map: mmap failed, {}", self.id, e);
                return Err(e);
            }
        }

        Ok(0)
    }

    fn fs_slave_sync(&self, fs: &VhostUserFSSlaveMsg) -> HandlerResult<u64> {
        trace!(target: "vhost-fs", "{}: SlaveReqHandler::fs_slave_sync()", self.id);

        for i in 0..VHOST_USER_FS_SLAVE_ENTRIES {
            let offset = fs.cache_offset[i];
            let len = fs.len[i];

            // Ignore if the length is 0.
            if len == 0 {
                continue;
            }

            debug!(
                "{}: fs_slave_sync: offset={:x} len={:x} cache_size={:x}",
                self.id, offset, len, self.cache_size
            );

            if !self.is_req_valid(offset, len) {
                error!(
                    "{}: fs_slave_map: Wrong offset or length, offset={:x} len={:x} cache_size={:x}",
                    self.id, offset, len, self.cache_size
                );
                return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
            }

            let addr = self.mmap_cache_addr + offset;
            let ret =
                unsafe { libc::msync(addr as *mut libc::c_void, len as usize, libc::MS_SYNC) };
            if ret == -1 {
                let e = std::io::Error::last_os_error();
                error!("{}: fs_slave_sync: msync failed, {}", self.id, e);
                return Err(e);
            }
        }

        Ok(0)
    }

    fn fs_slave_io(&self, fs: &VhostUserFSSlaveMsg, fd: &dyn AsRawFd) -> HandlerResult<u64> {
        trace!(target: "vhost-fs", "{}: SlaveReqHandler::fs_slave_io()", self.id);

        let guard = self.mem.memory();
        let mem = guard.deref();
        let mut done: u64 = 0;
        for i in 0..VHOST_USER_FS_SLAVE_ENTRIES {
            // Ignore if the length is 0.
            if fs.len[i] == 0 {
                continue;
            }

            let mut foffset = fs.fd_offset[i];
            let mut len = fs.len[i] as usize;
            let gpa = fs.cache_offset[i];
            let cache_end = self.cache_offset + self.cache_size;
            let efault = libc::EFAULT;

            debug!(
                "{}: fs_slave_io: gpa={:x} len={:x} foffset={:x} cache_offset={:x} cache_size={:x}",
                self.id, gpa, len, foffset, self.cache_offset, self.cache_size
            );

            let mut ptr = if gpa >= self.cache_offset && gpa < cache_end {
                let offset = gpa
                    .checked_sub(self.cache_offset)
                    .ok_or_else(|| io::Error::from_raw_os_error(efault))?;
                let end = gpa
                    .checked_add(fs.len[i])
                    .ok_or_else(|| io::Error::from_raw_os_error(efault))?;

                if end >= cache_end {
                    error!( "{}: fs_slave_io: Wrong gpa or len (gpa={:x} len={:x} cache_offset={:x}, cache_size={:x})", self.id, gpa, len, self.cache_offset, self.cache_size );
                    return Err(io::Error::from_raw_os_error(efault));
                }
                self.mmap_cache_addr + offset
            } else {
                // gpa is a RAM addr.
                mem.get_host_address(GuestAddress(gpa))
                    .map_err(|e| {
                        error!(
                            "{}: fs_slave_io: Failed to find RAM region associated with gpa 0x{:x}: {:?}",
                            self.id, gpa, e
                        );
                        io::Error::from_raw_os_error(efault)
                    })? as u64
            };

            while len > 0 {
                let ret = if (fs.flags[i] & VhostUserFSSlaveMsgFlags::MAP_W)
                    == VhostUserFSSlaveMsgFlags::MAP_W
                {
                    debug!("{}: write: foffset={:x}, len={:x}", self.id, foffset, len);
                    unsafe {
                        pwrite64(
                            fd.as_raw_fd(),
                            ptr as *const c_void,
                            len,
                            foffset as off64_t,
                        )
                    }
                } else {
                    debug!("{}: read: foffset={:x}, len={:x}", self.id, foffset, len);
                    unsafe { pread64(fd.as_raw_fd(), ptr as *mut c_void, len, foffset as off64_t) }
                };

                if ret < 0 {
                    let e = std::io::Error::last_os_error();
                    if (fs.flags[i] & VhostUserFSSlaveMsgFlags::MAP_W)
                        == VhostUserFSSlaveMsgFlags::MAP_W
                    {
                        error!("{}: fs_slave_io: pwrite failed, {}", self.id, e);
                    } else {
                        error!("{}: fs_slave_io: pread failed, {}", self.id, e);
                    }

                    return Err(e);
                }

                if ret == 0 {
                    // EOF
                    let e = io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "failed to access whole buffer",
                    );
                    error!("{}: fs_slave_io: IO error, {}", self.id, e);
                    return Err(e);
                }
                len -= ret as usize;
                foffset += ret as u64;
                ptr += ret as u64;
                done += ret as u64;
            }

            let ret = unsafe { libc::close(fd.as_raw_fd()) };
            if ret == -1 {
                let e = std::io::Error::last_os_error();
                error!("{}: fs_slave_io: close failed, {}", self.id, e);
                return Err(e);
            }
        }

        Ok(done)
    }
}

pub struct VhostUserFsHandler<
    AS: GuestAddressSpace,
    Q: QueueT,
    R: GuestMemoryRegion,
    S: VhostUserMasterReqHandler,
> {
    config: VirtioDeviceConfig<AS, Q, R>,
    device: Arc<Mutex<VhostUserFsDevice>>,
    slave_req_handler: Option<MasterReqHandler<S>>,
    id: String,
}

impl<AS, Q, R, S> MutEventSubscriber for VhostUserFsHandler<AS, Q, R, S>
where
    AS: 'static + GuestAddressSpace + Send + Sync,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Send + Sync + 'static,
    S: 'static + Send + VhostUserMasterReqHandler,
{
    fn process(&mut self, events: Events, _ops: &mut EventOps) {
        trace!(target: "vhost-fs", "{}: VhostUserFsHandler::process({})", self.id, events.data());

        match events.data() {
            MASTER_SLOT => {
                // If virtiofsd crashes, vmm will exit too.
                error!("{}: Master-slave disconnected, exiting...", self.id);
                // TODO: how to make dragonball crash here?
            }
            SLAVE_REQ_SLOT => match self.slave_req_handler.as_mut() {
                Some(handler) => {
                    if let Err(e) = handler.handle_request() {
                        error!(
                            "{}: failed to handle slave request failed, {:?}",
                            self.id, e
                        );
                    }
                }
                None => error!(
                    "{}: no slave_req_handler setup but got event from slave",
                    self.id
                ),
            },
            _ => error!("{}: unknown epoll event slot {}", self.id, events.data()),
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        trace!(target: "vhost-fs", "{}: VhostUserFsHandler::init()", self.id);

        let device = self.device.lock().unwrap();

        if let Err(e) = device.endpoint.register_epoll_event(ops) {
            error!(
                "{}: failed to register epoll event for endpoint, {}",
                self.id, e
            );
        }
        if let Some(slave_req_handler) = self.slave_req_handler.as_ref() {
            let events = Events::with_data(slave_req_handler, SLAVE_REQ_SLOT, EventSet::IN);
            if let Err(e) = ops.add(events) {
                error!(
                    "{}: failed to register epoll event for slave request handler, {:?}",
                    self.id, e
                );
            }
        }
    }
}

pub struct VhostUserFsDevice {
    device_info: VirtioDeviceInfo,
    endpoint: Endpoint,
    curr_queues: u32,
    cache_size: u64,
}

impl VhostUserFsDevice {
    pub fn new(
        path: &str,
        tag: &str,
        req_num_queues: usize,
        queue_size: u16,
        cache_size: u64,
        epoll_mgr: EpollManager,
    ) -> VirtioResult<Self> {
        // Connect to the vhost-user socket.
        info!("{}: try to connect to {:?}", VHOST_USER_FS_NAME, path);
        let num_queues = NUM_QUEUE_OFFSET + req_num_queues;
        let master = Master::connect(path, num_queues as u64).map_err(VirtioError::VhostError)?;

        info!("{}: get features", VHOST_USER_FS_NAME);
        let avail_features = master.get_features().map_err(VirtioError::VhostError)?;

        // Create virtio device config space.
        // First by adding the tag.
        let mut config_space = tag.to_string().into_bytes();
        config_space.resize(CONFIG_SPACE_SIZE, 0);

        // And then by copying the number of queues.
        let mut num_queues_slice: [u8; 4] = (req_num_queues as u32).to_be_bytes();
        num_queues_slice.reverse();
        config_space[CONFIG_SPACE_TAG_SIZE..CONFIG_SPACE_SIZE].copy_from_slice(&num_queues_slice);

        Ok(VhostUserFsDevice {
            device_info: VirtioDeviceInfo::new(
                VHOST_USER_FS_NAME.to_string(),
                avail_features,
                Arc::new(vec![queue_size; num_queues]),
                config_space,
                epoll_mgr,
            ),
            endpoint: Endpoint::new(master, MASTER_SLOT, VHOST_USER_FS_NAME.to_string()),
            curr_queues: num_queues as u32,
            cache_size,
        })
    }

    pub fn update_memory<AS: GuestAddressSpace>(&mut self, vm_as: &AS) -> VirtioResult<()> {
        self.endpoint.update_memory(vm_as)
    }

    fn is_dax_on(&self) -> bool {
        self.cache_size > 0
    }

    fn get_acked_features(&self) -> u64 {
        let mut features = self.device_info.acked_features();
        // Enable support of vhost-user protocol features if available
        features |=
            self.device_info.avail_features() & VhostUserVirtioFeatures::PROTOCOL_FEATURES.bits();
        features
    }

    // vhost-user protocol features this device supports
    fn get_dev_protocol_features(&self) -> VhostUserProtocolFeatures {
        let mut features = VhostUserProtocolFeatures::MQ | VhostUserProtocolFeatures::REPLY_ACK;
        if self.is_dax_on() {
            features |=
                VhostUserProtocolFeatures::SLAVE_REQ | VhostUserProtocolFeatures::SLAVE_SEND_FD;
        }
        features
    }

    fn setup_slave<
        AS: GuestAddressSpace,
        Q: QueueT,
        R: GuestMemoryRegion,
        S: VhostUserMasterReqHandler,
    >(
        &mut self,
        handler: &VhostUserFsHandler<AS, Q, R, S>,
    ) -> ActivateResult {
        let slave_req_fd = handler
            .slave_req_handler
            .as_ref()
            .map(|h| h.get_tx_raw_fd());
        let config = EndpointParam {
            virtio_config: &handler.config,
            intr_evts: handler.config.get_queue_interrupt_eventfds(),
            queue_sizes: &self.device_info.queue_sizes,
            features: self.get_acked_features(),
            protocol_flag: 0,
            dev_protocol_features: self.get_dev_protocol_features(),
            reconnect: false,
            backend: None,
            init_queues: self.curr_queues,
            slave_req_fd,
        };

        self.endpoint.negotiate(&config, None).map_err(|e| {
            error!(
                "{}: failed to setup connection: {}",
                self.device_info.driver_name, e
            );
            ActivateError::InternalError
        })
    }
}

#[derive(Clone)]
pub struct VhostUserFs<AS: GuestAddressSpace> {
    device: Arc<Mutex<VhostUserFsDevice>>,
    queue_sizes: Arc<Vec<u16>>,
    subscriber_id: Option<SubscriberId>,
    id: String,
    phantom: PhantomData<AS>,
}

impl<AS: GuestAddressSpace> VhostUserFs<AS> {
    /// Create a new vhost user fs device.
    pub fn new(
        path: String,
        tag: String,
        req_num_queues: usize,
        queue_size: u16,
        cache_size: u64,
        epoll_mgr: EpollManager,
    ) -> VirtioResult<Self> {
        // Calculate the actual number of queues needed.
        let num_queues = NUM_QUEUE_OFFSET + req_num_queues;
        let device = VhostUserFsDevice::new(
            &path,
            &tag,
            req_num_queues,
            queue_size,
            cache_size,
            epoll_mgr,
        )?;
        let id = device.device_info.driver_name.clone();

        Ok(VhostUserFs {
            device: Arc::new(Mutex::new(device)),
            queue_sizes: Arc::new(vec![queue_size; num_queues]),
            subscriber_id: None,
            id,
            phantom: PhantomData,
        })
    }

    pub fn get_vhost_user_fs_device(&self) -> Arc<Mutex<VhostUserFsDevice>> {
        self.device.clone()
    }

    fn device(&self) -> MutexGuard<VhostUserFsDevice> {
        // Do not expect poisoned lock.
        self.device.lock().unwrap()
    }

    fn id(&self) -> &str {
        &self.id
    }
}

impl<AS, Q> VirtioDevice<AS, Q, GuestRegionMmap> for VhostUserFs<AS>
where
    AS: 'static + GuestAddressSpace + Clone + Send + Sync,
    AS::T: Send,
    AS::M: Sync + Send,
    Q: QueueT + Send + 'static,
{
    fn device_type(&self) -> u32 {
        TYPE_VIRTIO_FS
    }

    fn queue_max_sizes(&self) -> &[u16] {
        &self.queue_sizes
    }

    fn get_avail_features(&self, page: u32) -> u32 {
        self.device().device_info.get_avail_features(page)
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        trace!(target: "vhost-fs", "{}: VirtioDevice::set_acked_features({}, 0x{:x})",
               self.id(), page, value);
        self.device().device_info.set_acked_features(page, value)
    }

    fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
        trace!(target: "vhost-fs", "{}: VirtioDevice::read_config(0x{:x}, {:?})",
               self.id(), offset, data);
        self.device().device_info.read_config(offset, data)
    }

    fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
        trace!(target: "vhost-fs", "{}: VirtioDevice::write_config(0x{:x}, {:?})",
               self.id(), offset, data);
        self.device().device_info.write_config(offset, data)
    }

    fn activate(&mut self, config: VirtioDeviceConfig<AS, Q>) -> ActivateResult {
        trace!(target: "vhost-fs", "{}: VirtioDevice::activate()", self.id());

        let mut device = self.device.lock().unwrap();
        device.device_info.check_queue_sizes(&config.queues)?;

        let slave_req_handler = if let Some((addr, guest_addr)) = config.get_shm_region_addr() {
            let vu_master_req_handler = Arc::new(SlaveReqHandler {
                cache_offset: guest_addr,
                cache_size: device.cache_size,
                mmap_cache_addr: addr,
                mem: config.vm_as.clone(),
                id: device.device_info.driver_name.clone(),
            });
            let req_handler = MasterReqHandler::new(vu_master_req_handler)
                .map_err(|e| ActivateError::VhostActivate(vhost_rs::Error::VhostUserProtocol(e)))?;

            Some(req_handler)
        } else {
            None
        };

        let handler = VhostUserFsHandler {
            config,
            device: self.device.clone(),
            slave_req_handler,
            id: device.device_info.driver_name.clone(),
        };
        device.setup_slave(&handler)?;
        let epoll_mgr = device.device_info.epoll_manager.clone();
        drop(device);
        self.subscriber_id = Some(epoll_mgr.add_subscriber(Box::new(handler)));

        Ok(())
    }

    // Please keep in synchronization with virtio-fs/fs.rs
    fn get_resource_requirements(
        &self,
        requests: &mut Vec<ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        trace!(target: "vhost-fs", "{}: VirtioDevice::get_resource_requirements()", self.id());

        requests.push(ResourceConstraint::LegacyIrq { irq: None });
        if use_generic_irq {
            // Allocate one irq for device configuration change events, and one irq for each queue.
            requests.push(ResourceConstraint::GenericIrq {
                size: (self.queue_sizes.len() + 1) as u32,
            });
        }

        // Check if we have dax enabled or not, just return if no dax window requested.
        let device = self.device();
        if !device.is_dax_on() {
            info!("{}: DAX window is disabled.", self.id());
            return;
        }

        // Request for DAX window. The memory needs to be 2MiB aligned in order to support
        // huge pages, and needs to be above 4G to avoid conflicts with lapic/ioapic devices.
        requests.push(ResourceConstraint::MmioAddress {
            range: Some((0x1_0000_0000, std::u64::MAX)),
            align: 0x0020_0000,
            size: device.cache_size,
        });

        // Request for new kvm memory slot for DAX window.
        requests.push(ResourceConstraint::KvmMemSlot {
            slot: None,
            size: 1,
        });
    }

    // Please keep in synchronization with virtio-fs/fs.rs
    fn set_resource(
        &mut self,
        vm_fd: Arc<VmFd>,
        resource: DeviceResources,
    ) -> VirtioResult<Option<VirtioSharedMemoryList<GuestRegionMmap>>> {
        trace!(target: "vhost-fs", "{}: VirtioDevice::set_resource()", self.id());

        let mmio_res = resource.get_mmio_address_ranges();
        let slot_res = resource.get_kvm_mem_slots();

        // Do nothing if there's no dax window requested.
        if mmio_res.is_empty() {
            return Ok(None);
        }

        // Make sure we have the correct resource as requested, and currently we only support one
        // shm region for DAX window (version table and journal are not supported yet).
        if mmio_res.len() != slot_res.len() || mmio_res.len() != 1 {
            error!(
                "{}: wrong number of mmio or kvm slot resource ({}, {})",
                self.id(),
                mmio_res.len(),
                slot_res.len()
            );
            return Err(VirtioError::InvalidResource);
        }

        let guest_addr = mmio_res[0].0;
        let cache_len = mmio_res[0].1;

        // unmap will be handled on MmapRegion'd Drop.
        let mmap_region = MmapRegion::build(
            None,
            cache_len as usize,
            libc::PROT_NONE,
            libc::MAP_ANONYMOUS | libc::MAP_NORESERVE | libc::MAP_PRIVATE,
        )
        .map_err(VirtioError::NewMmapRegion)?;
        let host_addr: u64 = mmap_region.as_ptr() as u64;

        debug!(
            "{}: mmio shared memory kvm slot {}, host_addr {:X}, guest_addr {:X}",
            self.id(),
            slot_res[0],
            host_addr,
            guest_addr
        );

        // add to guest memory mapping
        let kvm_mem_region = kvm_userspace_memory_region {
            slot: slot_res[0],
            flags: 0,
            guest_phys_addr: guest_addr,
            memory_size: cache_len,
            userspace_addr: host_addr,
        };
        // Safe because the user mem region is just created, and kvm slot is allocated
        // by resource allocator.
        unsafe {
            vm_fd
                .set_user_memory_region(kvm_mem_region)
                .map_err(VirtioError::SetUserMemoryRegion)?
        };

        let guest_mmap_region = Arc::new(
            GuestRegionMmap::new(mmap_region, GuestAddress(guest_addr))
                .map_err(VirtioError::InsertMmap)?,
        );

        Ok(Some(VirtioSharedMemoryList {
            host_addr,
            guest_addr: GuestAddress(guest_addr),
            len: cache_len as GuestUsize,
            kvm_userspace_memory_region_flags: 0,
            kvm_userspace_memory_region_slot: slot_res[0],
            region_list: vec![VirtioSharedMemory {
                offset: 0,
                len: cache_len,
            }],
            mmap_region: guest_mmap_region,
        }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::{InterruptManager, InterruptSourceType, MsiNotifier, NoopNotifier};
    use dbs_utils::epoll_manager::EpollManager;
    use kvm_ioctls::Kvm;
    use vhost_rs::vhost_user::message::{
        VhostUserProtocolFeatures, VhostUserU64, VhostUserVirtioFeatures,
    };
    use vhost_rs::vhost_user::Listener;
    use virtio_queue::QueueSync;
    use vm_memory::{FileOffset, GuestMemoryMmap, GuestRegionMmap};
    use vmm_sys_util::tempfile::TempFile;

    use crate::device::VirtioDevice;
    use crate::tests::create_address_space;
    use crate::vhost::vhost_user::fs::VhostUserFs;
    use crate::vhost::vhost_user::test_utils::*;
    use crate::{GuestAddress, VirtioDeviceConfig, VirtioQueueConfig, TYPE_VIRTIO_FS};

    fn create_vhost_user_fs_slave(slave: &mut Endpoint<MasterReq>) {
        let (hdr, rfds) = slave.recv_header().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::GET_FEATURES);
        assert!(rfds.is_none());
        let vfeatures = 0x15 | VhostUserVirtioFeatures::PROTOCOL_FEATURES.bits();
        let hdr = VhostUserMsgHeader::new(MasterReq::GET_FEATURES, 0x4, 8);
        let msg = VhostUserU64::new(vfeatures);
        slave.send_message(&hdr, &msg, None).unwrap();
    }

    #[test]
    fn test_vhost_user_fs_virtio_device_normal() {
        let device_socket = "/tmp/vhost.1";
        let tag = "test_fs";

        let handler = thread::spawn(move || {
            let listener = Listener::new(device_socket, true).unwrap();
            let mut slave = Endpoint::<MasterReq>::from_stream(listener.accept().unwrap().unwrap());
            create_vhost_user_fs_slave(&mut slave);
        });

        thread::sleep(Duration::from_millis(20));

        let epoll_mgr = EpollManager::default();

        let mut dev: VhostUserFs<Arc<GuestMemoryMmap>> = VhostUserFs::new(
            String::from(device_socket),
            String::from(tag),
            2,
            2,
            2,
            epoll_mgr,
        )
        .unwrap();

        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::device_type(&dev),
            TYPE_VIRTIO_FS
        );

        let queue_size = vec![2, 2, 2];
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::queue_max_sizes(
                &dev
            ),
            &queue_size[..]
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 0),
            dev.device().device_info.get_avail_features(0)
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 1),
            dev.device().device_info.get_avail_features(1)
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 2),
            dev.device().device_info.get_avail_features(2)
        );
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::set_acked_features(
            &mut dev, 2, 0,
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 2),
            0
        );
        let config: [u8; 8] = [0; 8];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::write_config(
            &mut dev, 0, &config,
        );
        let mut data: [u8; 8] = [1; 8];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
            &mut dev, 0, &mut data,
        );
        assert_eq!(config, data);

        handler.join().unwrap();
    }

    #[test]
    fn test_vhost_user_fs_virtio_device_activate() {
        let device_socket = "/tmp/vhost.1";
        let tag = "test_fs";

        let handler = thread::spawn(move || {
            let listener = Listener::new(device_socket, true).unwrap();
            let mut slave = Endpoint::<MasterReq>::from_stream(listener.accept().unwrap().unwrap());
            create_vhost_user_fs_slave(&mut slave);

            let pfeatures = VhostUserProtocolFeatures::CONFIG;
            negotiate_slave(&mut slave, pfeatures, false, 3);
        });

        thread::sleep(Duration::from_millis(20));

        let epoll_mgr = EpollManager::default();
        let mut dev: VhostUserFs<Arc<GuestMemoryMmap>> = VhostUserFs::new(
            String::from(device_socket),
            String::from(tag),
            2,
            2,
            2,
            epoll_mgr,
        )
        .unwrap();

        // invalid queue size
        {
            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let resources = DeviceResources::new();
            let queues = vec![VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap()];
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::new(
                Arc::new(mem),
                address_space,
                vm_fd,
                resources,
                queues,
                None,
                Arc::new(NoopNotifier::new()),
            );
            assert!(dev.activate(config).is_err());
        }

        // success
        {
            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());

            let (_vmfd, irq_manager) = crate::tests::create_vm_and_irq_manager();
            let group = irq_manager
                .create_group(InterruptSourceType::MsiIrq, 0, 3)
                .unwrap();

            let notifier = MsiNotifier::new(group.clone(), 1);
            let notifier2 = MsiNotifier::new(group.clone(), 1);
            let notifier3 = MsiNotifier::new(group.clone(), 1);
            let mut queues = vec![
                VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap(),
                VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap(),
                VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap(),
            ];
            queues[0].set_interrupt_notifier(Arc::new(notifier));
            queues[1].set_interrupt_notifier(Arc::new(notifier2));
            queues[2].set_interrupt_notifier(Arc::new(notifier3));

            let f = TempFile::new().unwrap().into_file();
            f.set_len(0x400).unwrap();
            let mem = GuestMemoryMmap::from_ranges_with_files(&[(
                GuestAddress(0),
                0x400,
                Some(FileOffset::new(f, 0)),
            )])
            .unwrap();
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::new(
                Arc::new(mem),
                address_space,
                vm_fd,
                resources,
                queues,
                None,
                Arc::new(NoopNotifier::new()),
            );

            dev.activate(config).unwrap();
        }

        handler.join().unwrap();
    }
}
