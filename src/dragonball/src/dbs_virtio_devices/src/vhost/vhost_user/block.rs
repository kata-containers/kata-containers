// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

/// A vhost-user-blk backend driver
use std::any::Any;
use std::marker::PhantomData;
use std::mem;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::connection::{Endpoint, EndpointParam};
use crate::{
    device, ActivateError, ActivateResult, ConfigResult, DbsGuestAddressSpace,
    Error as VirtIoError, Result as VirtIoResult, VirtioDevice, VirtioDeviceConfig,
    VirtioDeviceInfo, TYPE_BLOCK,
};
use dbs_device::resources::ResourceConstraint;
use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};
use log::{debug, error, info, trace, warn};
use timerfd::{SetTimeFlags, TimerFd, TimerState};
use vhost_rs::vhost_user::message::{
    VhostUserConfigFlags, VhostUserProtocolFeatures, VhostUserVirtioFeatures,
    VHOST_USER_CONFIG_OFFSET,
};
use vhost_rs::vhost_user::{Master, VhostUserMaster};
use vhost_rs::{Error as VhostError, VhostBackend};
use virtio_bindings::bindings::virtio_blk::{VIRTIO_BLK_F_MQ, VIRTIO_BLK_F_SEG_MAX};
use virtio_queue::QueueT;
use vm_memory::{ByteValued, GuestMemoryRegion};
use vmm_sys_util::eventfd::EventFd;

// The same with guest kernel virtio_blk_config
const CONFIG_SPACE_SIZE: usize = 36;
// Remote vhost user server may disconnect, track this event
const MASTER_SLOT: u32 = 0;
// Timer events for check spool server is ready
const TIMER_SLOT: u32 = 1;
// New descriptors are pending on the virtio queue.
const QUEUE_AVAIL_SLOT: u32 = 2;
const VHOST_USER_BLOCK_DRIVER_NAME: &str = "vhost-user-blk";

#[derive(Copy, Clone, Debug, Default)]
#[repr(C, packed)]
pub struct VirtioBlockConfig {
    pub capacity: u64,
    pub size_max: u32,
    pub seg_max: u32,
    pub geometry: VirtioBlockGeometry,
    pub blk_size: u32,
    pub physical_block_exp: u8,
    pub alignment_offset: u8,
    pub min_io_size: u16,
    pub opt_io_size: u32,
    pub writeback: u8,
    pub unused: u8,
    pub num_queues: u16,
    pub max_discard_sectors: u32,
    pub max_discard_seg: u32,
    pub discard_sector_alignment: u32,
    pub max_write_zeroes_sectors: u32,
    pub max_write_zeroes_seg: u32,
    pub write_zeroes_may_unmap: u8,
    pub unused1: [u8; 3],
}

// Safe because it is only used to implement a trait
// and does not involve any potential memory safety or concurrency issues.
unsafe impl ByteValued for VirtioBlockConfig {}

#[derive(Copy, Clone, Debug, Default)]
#[repr(C, packed)]
pub struct VirtioBlockGeometry {
    pub cylinders: u16,
    pub heads: u8,
    pub sectors: u8,
}

// Safe because it is only used to implement a trait
// and does not involve any potential memory safety or concurrency issues.
unsafe impl ByteValued for VirtioBlockGeometry {}

pub struct VhostUserBlockHandler<AS, Q, R>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Send + Sync + 'static,
{
    device: Arc<Mutex<VhostUserBlockDevice>>,
    config: VirtioDeviceConfig<AS, Q, R>,
    queue_sizes: Arc<Vec<u16>>,
    intr_evts: Arc<Vec<EventFd>>,
    timer_fd: TimerFd,
    id: String,
}

impl<AS, Q, R> MutEventSubscriber for VhostUserBlockHandler<AS, Q, R>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Send + Sync + 'static,
{
    fn process(&mut self, events: Events, ops: &mut EventOps) {
        let slot = events.data();
        trace!(target: "vhost-blk", "{}: VhostUserBlockHandler::process({})", self.id, slot);

        match slot {
            MASTER_SLOT => {
                info!("{}: master disconnected, try to reconnect...", self.id);
                // Do not expect poisoned lock.
                let mut device = self.device.lock().unwrap();
                self.timer_fd.set_state(
                    // A short delay to reconnect as soon as possible.
                    TimerState::Oneshot(Duration::new(0, 200)),
                    SetTimeFlags::Default,
                );
                if let Err(e) = device.handle_disconnect(ops) {
                    warn!("{}: failed to handle disconnect event, {:?}", self.id, e);
                }
                device
                    .register_timer_event(ops, &self.timer_fd)
                    .expect("vhost-user-blk: failed to register timer");
            }
            TIMER_SLOT => {
                // Do not expect poisoned lock.
                let mut device = self.device.lock().unwrap();
                match device.reconnect_to_server() {
                    Ok(master) => {
                        info!("{}: try to reconnect to the server", self.id);
                        let mut config = EndpointParam {
                            virtio_config: &self.config,
                            intr_evts: self.config.get_queue_interrupt_eventfds(),
                            queue_sizes: &self.queue_sizes,
                            features: device.get_acked_features(),
                            protocol_flag: 0,
                            dev_protocol_features: device.get_dev_protocol_features(),
                            reconnect: true,
                            backend: None,
                            init_queues: device.curr_queues,
                            slave_req_fd: None,
                        };

                        config.set_protocol_mq();

                        if let Err(e) = device.handle_reconnect(master, config, ops) {
                            info!("{}: failed to reconnect to master, {:?}", self.id, e);
                            return;
                        }
                        if let Err(e) = device.deregister_timer_event(ops, &self.timer_fd) {
                            warn!("{}: failed to deregister timer event, {:?}", self.id, e);
                        }
                    }
                    Err(_) => {
                        self.timer_fd.set_state(
                            TimerState::Oneshot(Duration::new(0, 200000000)),
                            SetTimeFlags::Default,
                        );
                    }
                }
            }
            _ => {
                let queue_idx = (slot - QUEUE_AVAIL_SLOT) as usize;
                if queue_idx < self.intr_evts.len() {
                    if let Err(e) = self.intr_evts[queue_idx].read() {
                        error!("{}: failed to read queue eventfd, {:?}", self.id, e);
                    } else if let Err(e) = self.config.queues[queue_idx].notify() {
                        error!("{}: failed to notify guest, {:?}", self.id, e);
                    }
                } else {
                    debug!("{}: unknown epoll event slot {}", self.id, slot);
                }
            }
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        trace!(target: "vhost-blk", "{}: VhostUserBlockHandler::init()", self.id);

        // Do not expect poisoned lock here.
        let device = self.device.lock().unwrap();

        if let Err(err) = device.endpoint.register_epoll_event(ops) {
            error!(
                "{}: failed to register epoll event for master, {:?}",
                self.id, err
            );
        } else {
            for idx in 0..device.queue_sizes.len() {
                let event = Events::with_data(
                    &device.intr_evts[idx],
                    QUEUE_AVAIL_SLOT + idx as u32,
                    EventSet::IN,
                );
                if let Err(err) = ops.add(event) {
                    error!(
                        "{}: failed to register epoll event for queue {}, {:?}",
                        self.id, idx, err
                    );
                }
            }
        }
    }
}

struct VhostUserBlockDevice {
    vhost_socket: String,
    queue_sizes: Arc<Vec<u16>>,
    device_info: VirtioDeviceInfo,
    endpoint: Endpoint,
    intr_evts: Arc<Vec<EventFd>>,
    timer_fd: Option<TimerFd>,
    curr_queues: u32,
    id: String,
}

impl VhostUserBlockDevice {
    pub fn new(
        config_path: String,
        queue_sizes: Arc<Vec<u16>>,
        event_mgr: EpollManager,
    ) -> VirtIoResult<Self> {
        // config_path = "spdk://xxxxxxx.sock", remove the prefix "spdk://"
        let vhost_socket = config_path
            .strip_prefix("spdk://")
            .ok_or_else(|| VirtIoError::InvalidInput)?
            .to_string();

        let init_queues = queue_sizes.len() as u32;

        info!("vhost-user-blk: try to connect to {:?}", vhost_socket);
        // Connect to the vhost-user socket.
        let mut master = Master::connect(&vhost_socket, 1).map_err(VirtIoError::VhostError)?;

        info!("vhost-user-blk: get features");
        let avail_features = master.get_features().map_err(VirtIoError::VhostError)?;
        info!(
            "vhost-user-blk: get features done, ret:{:?}, queue_size: {:?}",
            avail_features,
            queue_sizes.len()
        );

        // for the standard vhost_user_blk device, get the device config from slave.
        let config_space = {
            master.set_features(avail_features)?;
            let protocol_featuers = master.get_protocol_features()?;
            // set the config features to get the device's config from slave.
            master.set_protocol_features(protocol_featuers)?;

            let config_len = mem::size_of::<VirtioBlockConfig>();
            let config_space: Vec<u8> = vec![0u8; config_len];

            let (_, mut config_space) = master
                .get_config(
                    VHOST_USER_CONFIG_OFFSET,
                    config_len as u32,
                    VhostUserConfigFlags::WRITABLE,
                    config_space.as_slice(),
                )
                .map_err(VirtIoError::VhostError)?;

            // set the num queues
            config_space[CONFIG_SPACE_SIZE - 2] = init_queues as u8;
            config_space[CONFIG_SPACE_SIZE - 1] = (init_queues >> 8) as u8;
            config_space
        };

        let intr_evts: Vec<EventFd> = (0..init_queues).map(|_| EventFd::new(0).unwrap()).collect();

        Ok(VhostUserBlockDevice {
            vhost_socket,
            queue_sizes: queue_sizes.clone(),
            device_info: VirtioDeviceInfo::new(
                VHOST_USER_BLOCK_DRIVER_NAME.to_string(),
                avail_features,
                queue_sizes,
                config_space,
                event_mgr,
            ),
            endpoint: Endpoint::new(
                master,
                MASTER_SLOT,
                VHOST_USER_BLOCK_DRIVER_NAME.to_string(),
            ),
            timer_fd: Some(TimerFd::new().map_err(VirtIoError::IOError)?),
            intr_evts: Arc::new(intr_evts),
            curr_queues: init_queues,
            id: VHOST_USER_BLOCK_DRIVER_NAME.to_string(),
        })
    }

    fn reconnect_to_server(&mut self) -> VirtIoResult<Master> {
        if !Path::new(self.vhost_socket.as_str()).exists() {
            return Err(VirtIoError::InternalError);
        }
        let master = Master::connect(&self.vhost_socket, 1).map_err(VirtIoError::VhostError)?;

        Ok(master)
    }

    // vhost-user protocol features this device supports
    fn get_dev_protocol_features(&self) -> VhostUserProtocolFeatures {
        let mut features = VhostUserProtocolFeatures::MQ;

        // TODO: need to support INFLIGHT_SHMFD later, https://github.com/kata-containers/kata-containers/issues/8705
        features |= VhostUserProtocolFeatures::CONFIG
            | VhostUserProtocolFeatures::CONFIGURE_MEM_SLOTS
            | VhostUserProtocolFeatures::REPLY_ACK;
        // | VhostUserProtocolFeatures::INFLIGHT_SHMFD;

        features
    }

    fn setup_slave<AS, Q, R>(&mut self, handler: &VhostUserBlockHandler<AS, Q, R>) -> ActivateResult
    where
        AS: DbsGuestAddressSpace,
        Q: QueueT + Send + 'static,
        R: GuestMemoryRegion + Sync + Send + 'static,
    {
        let mut config = EndpointParam {
            virtio_config: &handler.config,
            intr_evts: handler.config.get_queue_interrupt_eventfds(),
            queue_sizes: &self.queue_sizes,
            features: self.get_acked_features(),
            protocol_flag: 0,
            dev_protocol_features: self.get_dev_protocol_features(),
            reconnect: false,
            backend: None,
            init_queues: self.curr_queues,
            slave_req_fd: None,
        };
        config.set_protocol_mq();

        loop {
            match self.endpoint.negotiate(&config, None) {
                Ok(_) => break,
                Err(VirtIoError::VhostError(VhostError::VhostUserProtocol(e))) => {
                    if e.should_reconnect() {
                        // fall through to rebuild the connection.
                        warn!(
                            "{}: socket disconnected while initializing the connnection: {}",
                            self.id, e
                        );
                    } else {
                        error!("{}: failed to setup connection: {}", self.id, e);
                        return Err(VhostError::VhostUserProtocol(e).into());
                    }
                }
                Err(e) => {
                    error!("{}: failed to setup connection: {}", self.id, e);
                    return Err(ActivateError::InternalError);
                }
            }

            // Sleep for 100ms to limit the reconnection rate.
            let delay = std::time::Duration::from_millis(100);
            std::thread::sleep(delay);

            if !Path::new(self.vhost_socket.as_str()).exists() {
                return Err(ActivateError::InternalError);
            }
            let master = Master::connect(&String::from(self.vhost_socket.as_str()), 1)
                .map_err(VirtIoError::VhostError)?;

            self.endpoint.set_master(master);
        }

        Ok(())
    }

    // monitor connection to the slave for disconnection/errors.
    fn register_timer_event(&self, ops: &mut EventOps, tfd: &TimerFd) -> VirtIoResult<()> {
        let event = Events::with_data(tfd, TIMER_SLOT, EventSet::IN);

        ops.add(event).map_err(VirtIoError::EpollMgr)
    }

    fn deregister_timer_event(&self, ops: &mut EventOps, tfd: &TimerFd) -> VirtIoResult<()> {
        let event = Events::with_data(tfd, TIMER_SLOT, EventSet::IN);

        ops.remove(event).map_err(VirtIoError::EpollMgr)
    }

    fn handle_reconnect<
        AS: DbsGuestAddressSpace,
        Q: QueueT,
        R: GuestMemoryRegion + Send + Sync + 'static,
    >(
        &mut self,
        master: Master,
        config: EndpointParam<AS, Q, R>,
        ops: &mut EventOps,
    ) -> std::result::Result<(), VirtIoError> {
        self.endpoint.reconnect(master, &config, ops)
    }

    fn handle_disconnect(&mut self, ops: &mut EventOps) -> std::result::Result<(), VirtIoError> {
        self.endpoint.disconnect(ops)
    }

    fn get_acked_features(&self) -> u64 {
        let mut features = self.device_info.acked_features();

        // Enable support of vhost-user protocol features if available
        features |=
            self.device_info.avail_features() & VhostUserVirtioFeatures::PROTOCOL_FEATURES.bits();

        features
    }
}

#[derive(Clone)]
pub struct VhostUserBlock<AS>
where
    AS: DbsGuestAddressSpace,
{
    device: Arc<Mutex<VhostUserBlockDevice>>,
    queue_sizes: Arc<Vec<u16>>,
    subscriber_id: Option<SubscriberId>,
    id: String,
    phantom: PhantomData<AS>,
}

impl<AS> VhostUserBlock<AS>
where
    AS: DbsGuestAddressSpace,
{
    /// Create a new vhost user block device.
    pub fn new(
        config_path: String,
        queue_sizes: Arc<Vec<u16>>,
        event_mgr: EpollManager,
    ) -> VirtIoResult<Self> {
        let device = VhostUserBlockDevice::new(config_path, queue_sizes.clone(), event_mgr)?;
        let id = device.device_info.driver_name.clone();

        Ok(VhostUserBlock {
            device: Arc::new(Mutex::new(device)),
            queue_sizes,
            subscriber_id: None,
            id,
            phantom: PhantomData,
        })
    }

    fn device(&self) -> MutexGuard<VhostUserBlockDevice> {
        self.device.lock().unwrap()
    }
}

impl<AS, Q, R> VirtioDevice<AS, Q, R> for VhostUserBlock<AS>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    fn device_type(&self) -> u32 {
        TYPE_BLOCK
    }

    fn queue_max_sizes(&self) -> &[u16] {
        &self.queue_sizes
    }

    fn get_avail_features(&self, page: u32) -> u32 {
        let device = self.device();

        let mut avail_features = device.device_info.get_avail_features(page);
        if self.queue_sizes.len() > 1 {
            avail_features |= (1 << VIRTIO_BLK_F_MQ) | (1 << VIRTIO_BLK_F_SEG_MAX);
        }
        avail_features
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        trace!(target: "vhost-blk", "{}: VirtioDevice::set_acked_features({}, 0x{:x})",
               self.id, page, value
        );

        self.device().device_info.set_acked_features(page, value)
    }

    fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
        trace!(target: "vhost-blk", "{}: VirtioDevice::read_config(0x{:x}, {:?})",
               self.id, offset, data);

        self.device().device_info.read_config(offset, data)
    }

    fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
        trace!(target: "vhost-blk", "{}: VirtioDevice::write_config(0x{:x}, {:?})",
               self.id, offset, data);

        self.device().device_info.write_config(offset, data)
    }

    fn activate(&mut self, config: device::VirtioDeviceConfig<AS, Q, R>) -> ActivateResult {
        trace!(target: "vhost-blk", "{}: VirtioDevice::activate()", self.id);

        let mut device = self.device();
        if config.queues.len() != device.queue_sizes.len() {
            error!(
                "{}: cannot perform activate, expected {} queue(s), got {}",
                self.id,
                device.queue_sizes.len(),
                config.queues.len()
            );
            return Err(ActivateError::InvalidParam);
        }

        let timer_fd = device.timer_fd.take().unwrap();
        let handler = VhostUserBlockHandler {
            device: self.device.clone(),
            queue_sizes: self.queue_sizes.clone(),
            intr_evts: device.intr_evts.clone(),
            timer_fd,
            config,
            id: self.id.clone(),
        };

        device.setup_slave(&handler)?;
        let epoll_mgr = device.device_info.epoll_manager.clone();
        drop(device);
        self.subscriber_id = Some(epoll_mgr.add_subscriber(Box::new(handler)));

        Ok(())
    }

    fn get_resource_requirements(
        &self,
        requests: &mut Vec<ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        trace!(target: "vhost-blk", "{}: VirtioDevice::get_resource_requirements()", self.id);

        requests.push(ResourceConstraint::LegacyIrq { irq: None });
        if use_generic_irq {
            /* Allocate one irq for device configuration change events, and one irq for each queue. */
            requests.push(ResourceConstraint::GenericIrq {
                size: (self.queue_sizes.len() + 1) as u32,
            });
        }
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
    use std::mem;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::{InterruptManager, InterruptSourceType, MsiNotifier, NoopNotifier};
    use dbs_utils::epoll_manager::EpollManager;
    use kvm_ioctls::Kvm;
    use vhost_rs::vhost_user::message::{
        VhostUserConfig, VhostUserProtocolFeatures, VhostUserU64, VhostUserVirtioFeatures,
    };
    use vhost_rs::vhost_user::Listener;
    use virtio_queue::QueueSync;
    use vm_memory::{FileOffset, GuestMemoryMmap, GuestRegionMmap};
    use vmm_sys_util::tempfile::TempFile;

    use crate::tests::create_address_space;
    use crate::vhost::vhost_user::block::{
        VhostUserBlock, VhostUserConfigFlags, VirtioBlockConfig, VHOST_USER_CONFIG_OFFSET,
    };
    use crate::vhost::vhost_user::test_utils::*;
    use crate::{GuestAddress, VirtioDevice, VirtioDeviceConfig, VirtioQueueConfig, TYPE_BLOCK};

    fn create_vhost_user_block_slave(slave: &mut Endpoint<MasterReq>) {
        // get features
        let (hdr, rfds) = slave.recv_header().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::GET_FEATURES);
        assert!(rfds.is_none());
        let vfeatures = 0x15 | VhostUserVirtioFeatures::PROTOCOL_FEATURES.bits();
        let hdr = VhostUserMsgHeader::new(MasterReq::GET_FEATURES, 0x4, 8);
        let msg = VhostUserU64::new(vfeatures);
        slave.send_message(&hdr, &msg, None).unwrap();

        // set features
        let (hdr, _msg, rfds) = slave.recv_body::<VhostUserU64>().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::SET_FEATURES);
        assert!(rfds.is_none());

        // get protocol features
        let mut pfeatures = VhostUserProtocolFeatures::all();
        pfeatures -= VhostUserProtocolFeatures::INFLIGHT_SHMFD; // TODO: need to support INFLIGHT_SHMFD later, https://github.com/kata-containers/kata-containers/issues/8705
        let hdr = VhostUserMsgHeader::new(MasterReq::GET_PROTOCOL_FEATURES, 0x4, 8);
        let msg = VhostUserU64::new(pfeatures.bits());
        slave.send_message(&hdr, &msg, None).unwrap();
        let (hdr, rfds) = slave.recv_header().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::GET_PROTOCOL_FEATURES);
        assert!(rfds.is_none());

        // set protocol features
        let (hdr, _msg, rfds) = slave.recv_body::<VhostUserU64>().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::SET_PROTOCOL_FEATURES);
        assert!(rfds.is_none());
        let val = msg.value;
        assert_eq!(val, pfeatures.bits());

        // get config
        let config_len = mem::size_of::<VirtioBlockConfig>();
        let mut config_space: Vec<u8> = vec![0u8; config_len as usize];
        let (hdr, _msg, _payload, rfds) = slave
            .recv_payload_into_buf::<VhostUserConfig>(&mut config_space)
            .unwrap();
        assert_eq!(hdr.get_code(), MasterReq::GET_CONFIG);
        assert!(rfds.is_none());
        let hdr = VhostUserMsgHeader::new(MasterReq::GET_CONFIG, 0x4, 72);
        let msg = VhostUserConfig::new(
            VHOST_USER_CONFIG_OFFSET,
            config_len as u32,
            VhostUserConfigFlags::WRITABLE,
        );
        slave
            .send_message_with_payload(&hdr, &msg, config_space.as_slice(), None)
            .unwrap();
    }

    #[test]
    fn test_vhost_user_block_virtio_device_spdk() {
        let socket_path = "/tmp/vhost.1";

        let handler = thread::spawn(move || {
            let listener = Listener::new(socket_path, true).unwrap();
            let mut slave = Endpoint::<MasterReq>::from_stream(listener.accept().unwrap().unwrap());
            create_vhost_user_block_slave(&mut slave);
        });

        thread::sleep(Duration::from_millis(20));

        let spdk_path = format!("spdk://{}", socket_path);
        let queue_sizes = Arc::new(vec![128]);
        let epoll_mgr = EpollManager::default();
        let mut dev: VhostUserBlock<Arc<GuestMemoryMmap>> =
            VhostUserBlock::new(spdk_path, queue_sizes, epoll_mgr).unwrap();

        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::device_type(&dev),
            TYPE_BLOCK
        );

        let queue_size = vec![128];
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
        assert_eq!(VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 2), 0);
        let config: [u8; 8] = [0; 8];
        let _result =
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::write_config(
                &mut dev, 0, &config,
            );
        let mut data: [u8; 8] = [1; 8];
        let _result =
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
                &mut dev, 0, &mut data,
            );
        assert_eq!(config, data);

        handler.join().unwrap();
    }

    #[test]
    fn test_vhost_user_block_virtio_device_activate_spdk() {
        let socket_path = "/tmp/vhost.2";

        let handler = thread::spawn(move || {
            // create vhost user block device
            let listener = Listener::new(socket_path, true).unwrap();
            let mut slave = Endpoint::<MasterReq>::from_stream(listener.accept().unwrap().unwrap());
            create_vhost_user_block_slave(&mut slave);

            let mut pfeatures = VhostUserProtocolFeatures::all();
            pfeatures -= VhostUserProtocolFeatures::INFLIGHT_SHMFD; // TODO: need to support INFLIGHT_SHMFD later, https://github.com/kata-containers/kata-containers/issues/8705
            negotiate_slave(&mut slave, pfeatures, true, 2);
        });

        thread::sleep(Duration::from_millis(20));

        let spdk_path = format!("spdk://{}", socket_path);
        let queue_sizes = Arc::new(vec![128, 128]);
        let epoll_mgr = EpollManager::default();
        let mut dev: VhostUserBlock<Arc<GuestMemoryMmap>> =
            VhostUserBlock::new(spdk_path, queue_sizes, epoll_mgr).unwrap();

        // invalid queue size
        {
            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let resources = DeviceResources::new();
            let queues = vec![VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap()];
            let address_space = create_address_space();
            let config: VirtioDeviceConfig<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap> =
                VirtioDeviceConfig::new(
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
            let mut queues = vec![
                VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap(),
                VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap(),
            ];
            queues[0].set_interrupt_notifier(Arc::new(notifier));
            queues[1].set_interrupt_notifier(Arc::new(notifier2));

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
            let config: VirtioDeviceConfig<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap> =
                VirtioDeviceConfig::new(
                    Arc::new(mem),
                    address_space,
                    vm_fd,
                    resources,
                    queues,
                    None,
                    Arc::new(NoopNotifier::new()),
                );

            dev.activate(config).unwrap();

            handler.join().unwrap();
        }
    }
}
