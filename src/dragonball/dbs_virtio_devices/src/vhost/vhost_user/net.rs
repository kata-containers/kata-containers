// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// Copyright (C) 2019-2023 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use dbs_device::resources::ResourceConstraint;
use dbs_utils::epoll_manager::{EpollManager, EventOps, Events, MutEventSubscriber, SubscriberId};
use dbs_utils::net::MacAddr;
use log::{debug, error, info, trace, warn};
use vhost_rs::vhost_user::{
    Error as VhostUserError, Master, VhostUserProtocolFeatures, VhostUserVirtioFeatures,
};
use vhost_rs::Error as VhostError;
use virtio_bindings::bindings::virtio_net::{
    virtio_net_ctrl_hdr, VIRTIO_NET_CTRL_MQ, VIRTIO_NET_F_CTRL_MAC_ADDR, VIRTIO_NET_F_CTRL_RX,
    VIRTIO_NET_F_CTRL_VLAN, VIRTIO_NET_F_CTRL_VQ, VIRTIO_NET_F_GUEST_ANNOUNCE, VIRTIO_NET_F_MQ,
    VIRTIO_NET_F_MTU, VIRTIO_NET_OK,
};
use virtio_queue::{DescriptorChain, QueueT};
use vm_memory::GuestMemoryRegion;
use vmm_sys_util::epoll::EventSet;

use super::connection::{Endpoint, Listener};
use crate::vhost::net::{virtio_handle_ctrl_mq, virtio_handle_ctrl_status, FromNetCtrl};
use crate::vhost::vhost_user::connection::EndpointParam;
use crate::{
    setup_config_space, ActivateResult, ConfigResult, DbsGuestAddressSpace, Error as VirtioError,
    Result as VirtioResult, VirtioDevice, VirtioDeviceConfig, VirtioDeviceInfo, DEFAULT_MTU,
    TYPE_NET,
};

const NET_DRIVER_NAME: &str = "vhost-user-net";
// Epoll token for incoming connection on the Unix Domain Socket listener.
const LISTENER_SLOT: u32 = 0;
// Epoll token for monitoring the Unix Domain Socket between the master and
// the slave.
const MASTER_SLOT: u32 = 1;
// Epoll token for control queue
const CTRL_SLOT: u32 = 2;
// Control queue count
const CTRL_QUEUE_NUM: u16 = 64;

/// An implementation of vhost-user-net device
struct VhostUserNetDevice {
    /// Fixed value: "vhost-user-net".
    id: String,
    device_info: VirtioDeviceInfo,
    /// Unix domain socket connecting to the vhost-user slave.
    endpoint: Endpoint,
    /// Unix domain socket listener to accept incoming connection from the slave.
    listener: Listener,
    /// current enabled queues with vhost-user slave
    curr_queues: u32,
}

impl VhostUserNetDevice {
    fn new(
        master: Master,
        mut avail_features: u64,
        listener: Listener,
        guest_mac: Option<&MacAddr>,
        queue_sizes: Arc<Vec<u16>>,
        epoll_mgr: EpollManager,
    ) -> VirtioResult<Self> {
        info!(
            "{}: slave support features 0x{:x}",
            NET_DRIVER_NAME, avail_features
        );

        avail_features |= (1 << VIRTIO_NET_F_MTU) as u64;
        // All these features depends on availability of control channel
        // (VIRTIO_NET_F_CTRL_VQ).
        avail_features &= !(1 << VIRTIO_NET_F_CTRL_VQ
            | 1 << VIRTIO_NET_F_CTRL_RX
            | 1 << VIRTIO_NET_F_CTRL_VLAN
            | 1 << VIRTIO_NET_F_GUEST_ANNOUNCE
            | 1 << VIRTIO_NET_F_MQ
            | 1 << VIRTIO_NET_F_CTRL_MAC_ADDR) as u64;

        // Multi-queue features
        if queue_sizes.len() > 2 {
            avail_features |= (1 << VIRTIO_NET_F_MQ | 1 << VIRTIO_NET_F_CTRL_VQ) as u64;
        }

        let config_space = setup_config_space(
            NET_DRIVER_NAME,
            &guest_mac,
            &mut avail_features,
            (queue_sizes.len() / 2) as u16,
            DEFAULT_MTU,
        )?;

        Ok(VhostUserNetDevice {
            id: NET_DRIVER_NAME.to_owned(),
            device_info: VirtioDeviceInfo::new(
                NET_DRIVER_NAME.to_owned(),
                avail_features,
                queue_sizes,
                config_space,
                epoll_mgr,
            ),
            endpoint: Endpoint::new(master, MASTER_SLOT, NET_DRIVER_NAME.to_owned()),
            listener,
            curr_queues: 2,
        })
    }

    /// Create a vhost-user-net server instance.
    /// The function will hang on until a connection is established with a slave.
    fn new_server(
        path: &str,
        guest_mac: Option<&MacAddr>,
        queue_sizes: Arc<Vec<u16>>,
        epoll_mgr: EpollManager,
    ) -> VirtioResult<Self> {
        info!(
            "{}: creating Unix Domain Socket listener...",
            NET_DRIVER_NAME
        );

        let listener = Listener::new(
            NET_DRIVER_NAME.to_string(),
            path.to_string(),
            true,
            LISTENER_SLOT,
        )?;

        info!(
            "{}: waiting for incoming connection from the slave...",
            NET_DRIVER_NAME
        );
        let (master, avail_features) = listener.accept()?;
        info!("{}: connection to slave is ready.", NET_DRIVER_NAME);

        Self::new(
            master,
            avail_features,
            listener,
            guest_mac,
            queue_sizes,
            epoll_mgr,
        )
    }

    fn activate_slave<AS, Q, R>(
        &mut self,
        handler: &VhostUserNetHandler<AS, Q, R>,
    ) -> ActivateResult
    where
        AS: DbsGuestAddressSpace,
        Q: QueueT + Send + 'static,
        R: GuestMemoryRegion + Sync + Send + 'static,
    {
        trace!(target: "vhost-net", "{}: VhostUserNetDevice::activate_slave()", self.id);
        let mut config = EndpointParam {
            virtio_config: &handler.config,
            intr_evts: handler.config.get_queue_interrupt_eventfds(),
            queue_sizes: &self.device_info.queue_sizes,
            features: self.get_acked_features(),
            protocol_flag: 0,
            dev_protocol_features: Self::get_dev_protocol_features(),
            reconnect: false,
            backend: None,
            init_queues: self.curr_queues,
            slave_req_fd: None,
        };
        config.set_protocol_mq();
        // Do negotiate with the vhost-user slave
        loop {
            match self.endpoint.negotiate(&config, None) {
                Ok(_) => break,
                Err(VirtioError::VhostError(VhostError::VhostUserProtocol(err))) => {
                    if err.should_reconnect() {
                        // Fall through to rebuild the connection.
                        warn!(
                            "{}: socket disconnected while initializing the connnection, {}",
                            self.id, err
                        );
                    } else {
                        error!("{}: failed to setup connection, {}", self.id, err);
                        return Err(VhostError::VhostUserProtocol(err).into());
                    }
                }
                Err(err) => {
                    error!("{}: failed to setup connection, {}", self.id, err);
                    return Err(err.into());
                }
            }
            // Do reconnect
            // Wait 100ms for the next connection
            let delay = Duration::from_millis(100);
            std::thread::sleep(delay);
            // The underlying communication channel has been disconnected,
            // recreate it again.
            let (master, avail_features) = self.listener.accept()?;
            if !avail_features & self.device_info.acked_features() != 0 {
                error!("{}: Virtio features changed when reconnecting, avail features: 0x{:X}, acked features: 0x{:X}.", 
                    self.id, avail_features, self.device_info.acked_features());
                return Err(VhostError::VhostUserProtocol(VhostUserError::FeatureMismatch).into());
            }
            self.endpoint.set_master(master);
        }
        Ok(())
    }

    fn get_acked_features(&self) -> u64 {
        // typical features: 0x17c6effcb
        // typical acked_features: 0x17000FF8B
        // typical protocol features: 0x37
        let mut features = self.device_info.acked_features();
        // Enable support of vhost-user protocol features if available
        features |=
            self.device_info.avail_features() & VhostUserVirtioFeatures::PROTOCOL_FEATURES.bits();
        features
    }

    /// Vhost-user protocol features this device supports
    fn get_dev_protocol_features() -> VhostUserProtocolFeatures {
        VhostUserProtocolFeatures::MQ
    }

    fn handle_connect<AS, Q, R>(
        &mut self,
        ops: &mut EventOps,
        _events: Events,
        handler: &VhostUserNetHandler<AS, Q, R>,
    ) -> VirtioResult<()>
    where
        AS: DbsGuestAddressSpace,
        Q: QueueT + Send + 'static,
        R: GuestMemoryRegion + Sync + Send + 'static,
    {
        info!("{}: try to accept new socket for reconnect...", self.id);
        match self.listener.try_accept() {
            Ok(Some((master, _avail_features))) => {
                let mut config = EndpointParam {
                    virtio_config: &handler.config,
                    intr_evts: handler.config.get_queue_interrupt_eventfds(),
                    queue_sizes: &self.device_info.queue_sizes,
                    features: self.get_acked_features(),
                    protocol_flag: 0,
                    dev_protocol_features: VhostUserNetDevice::get_dev_protocol_features(),
                    reconnect: true,
                    backend: None,
                    init_queues: self.curr_queues,
                    slave_req_fd: None,
                };
                config.set_protocol_mq();
                self.endpoint.reconnect(master, &config, ops)?;
                info!("{}: communication channel has been recovered.", self.id);
                Ok(())
            }
            Ok(None) => {
                warn!(
                    "{}: no incoming connection available when handle incoming connection",
                    self.id
                );
                Ok(())
            }
            Err(_) => {
                warn!("{}: no incoming connection available", self.id);
                Err(VirtioError::InternalError)
            }
        }
    }

    fn handle_disconnect(&mut self, ops: &mut EventOps) -> VirtioResult<()> {
        trace!(target: "vhost-net", "{}: VhostUserNetDevice::handle_disconnect()", self.id);
        self.endpoint.disconnect(ops)
    }

    fn handle_set_queues(&mut self, queue_pairs: u32) -> VirtioResult<()> {
        trace!(target: "vhost-net", "{}: VhostUserNetDevice::handle_set_queues({})", self.id, queue_pairs);
        self.curr_queues = queue_pairs * 2;
        debug!("{}: set multi-queue to {}", self.id, self.curr_queues);
        loop {
            match self.endpoint.set_queues_attach(self.curr_queues) {
                Ok(_) => break,
                Err(VirtioError::VhostError(VhostError::VhostUserProtocol(err))) => {
                    if err.should_reconnect() {
                        warn!(
                            "{}: socket disconnected while initializing the connnection: {}",
                            self.id, err
                        );
                    } else {
                        error!("{}: failed to setup connection: {}", self.id, err);
                        return Err(VhostError::VhostUserProtocol(err).into());
                    }
                }
                Err(err) => {
                    error!("{}: failed to setup connection: {}", self.id, err);
                    return Err(err);
                }
            }
            // Do reconnect
            // Wait 100ms for the next connection
            let delay = Duration::from_millis(100);
            std::thread::sleep(delay);
            // The underlying communication channel has been disconnected,
            // recreate it again.
            let (master, avail_features) = self.listener.accept()?;
            if !avail_features & self.device_info.acked_features() != 0 {
                error!("{}: Virtio features changed when reconnecting, avail features: 0x{:X}, acked features: 0x{:X}.", 
                    self.id, avail_features, self.device_info.acked_features());
                return Err(VhostError::VhostUserProtocol(VhostUserError::FeatureMismatch).into());
            }
            self.endpoint.set_master(master);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct VhostUserNet<AS>
where
    AS: DbsGuestAddressSpace,
{
    id: String,
    device: Arc<Mutex<VhostUserNetDevice>>,
    queue_sizes: Arc<Vec<u16>>,
    ctrl_queue_sizes: u16,
    subscriber_id: Option<SubscriberId>,
    phantom: PhantomData<AS>,
}

impl<AS> VhostUserNet<AS>
where
    AS: DbsGuestAddressSpace,
{
    /// Create a new vhost-user net device.
    ///
    /// Create a Unix Domain Socket listener and wait until the the first incoming connection is
    /// ready. The listener will be used to accept new incoming connections when the current
    /// connection gets broken.
    pub fn new_server(
        path: &str,
        guest_mac: Option<&MacAddr>,
        queue_sizes: Arc<Vec<u16>>,
        epoll_mgr: EpollManager,
    ) -> VirtioResult<Self> {
        let device =
            VhostUserNetDevice::new_server(path, guest_mac, queue_sizes.clone(), epoll_mgr)?;
        let ctrl_queue_sizes = if queue_sizes.len() > 2 {
            CTRL_QUEUE_NUM
        } else {
            0
        };
        let id = device.device_info.driver_name.clone();
        Ok(VhostUserNet {
            id,
            device: Arc::new(Mutex::new(device)),
            queue_sizes,
            ctrl_queue_sizes,
            subscriber_id: None,
            phantom: PhantomData,
        })
    }

    fn device(&self) -> MutexGuard<VhostUserNetDevice> {
        // Do not expect poisoned lock.
        self.device.lock().unwrap()
    }
}

impl<AS, Q, R> VirtioDevice<AS, Q, R> for VhostUserNet<AS>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    fn device_type(&self) -> u32 {
        TYPE_NET
    }

    fn queue_max_sizes(&self) -> &[u16] {
        &self.queue_sizes
    }

    fn ctrl_queue_max_sizes(&self) -> u16 {
        self.ctrl_queue_sizes
    }

    fn get_avail_features(&self, page: u32) -> u32 {
        self.device().device_info.get_avail_features(page)
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        trace!(target: "vhost-net", "{}: VirtioDevice::set_acked_features({}, 0x{:x})",
               self.id, page, value);
        self.device().device_info.set_acked_features(page, value)
    }

    fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
        trace!(target: "vhost-net", "{}: VirtioDevice::read_config(0x{:x}, {:?})",
               self.id, offset, data);
        self.device().device_info.read_config(offset, data)?;
        Ok(())
    }

    fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
        trace!(target: "vhost-net", "{}: VirtioDevice::write_config(0x{:x}, {:?})",
               self.id, offset, data);
        self.device().device_info.write_config(offset, data)?;
        Ok(())
    }

    fn activate(&mut self, config: VirtioDeviceConfig<AS, Q, R>) -> ActivateResult {
        trace!(target: "vhost-net", "{}: VirtioDevice::activate()", self.id);
        let mut device = self.device();
        device.device_info.check_queue_sizes(&config.queues)?;
        let handler = VhostUserNetHandler {
            device: self.device.clone(),
            config,
            id: self.id.clone(),
        };
        device.activate_slave(&handler)?;
        let epoll_mgr = device.device_info.epoll_manager.clone();
        drop(device);
        self.subscriber_id = Some(epoll_mgr.add_subscriber(Box::new(handler)));
        Ok(())
    }

    fn get_resource_requirements(
        &self,
        requests: &mut Vec<dbs_device::resources::ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        trace!(target: "vhost-net", "{}: VirtioDevice::get_resource_requirements()", self.id);
        requests.push(ResourceConstraint::LegacyIrq { irq: None });
        if use_generic_irq {
            // Allocate one irq for device configuration change events, and
            // one irq for each queue.
            requests.push(ResourceConstraint::GenericIrq {
                size: (self.queue_sizes.len() + 1) as u32,
            });
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

pub struct VhostUserNetHandler<AS, Q, R>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    device: Arc<Mutex<VhostUserNetDevice>>,
    config: VirtioDeviceConfig<AS, Q, R>,
    id: String,
}

impl<AS, Q, R> VhostUserNetHandler<AS, Q, R>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    fn device(&self) -> MutexGuard<VhostUserNetDevice> {
        // Do not expect poisoned lock here
        self.device.lock().unwrap()
    }

    fn process_ctrl_request(&mut self) -> VirtioResult<()> {
        let guard = self.config.lock_guest_memory();
        let mem = guard.deref();
        // It is safty to unwrap here as the value of ctrl_queue is
        // confirmed in `CTRL_SLOT`.
        let cvq = self.config.ctrl_queue.as_mut().unwrap();
        let device = self.device.clone();
        while let Some(mut desc_chain) = cvq.get_next_descriptor(mem)? {
            let len = match Self::process_ctrl_desc(&mut desc_chain, &device, mem) {
                Ok(len) => {
                    debug!("{}: process ctrl desc succeed!", self.id);
                    len
                }
                Err(e) => {
                    debug!(
                        "{}: failed to process control queue request, {}",
                        self.id, e
                    );
                    0
                }
            };
            cvq.add_used(mem, desc_chain.head_index(), len);
        }
        Ok(())
    }

    fn process_ctrl_desc(
        desc_chain: &mut DescriptorChain<&AS::M>,
        device: &Arc<Mutex<VhostUserNetDevice>>,
        mem: &AS::M,
    ) -> VirtioResult<u32> {
        if let Some(header) = desc_chain.next() {
            let ctrl_hdr = virtio_net_ctrl_hdr::from_net_ctrl_st(mem, &header)?;
            match ctrl_hdr.class as u32 {
                VIRTIO_NET_CTRL_MQ => {
                    virtio_handle_ctrl_mq::<AS, _>(desc_chain, ctrl_hdr.cmd, mem, |curr_queues| {
                        device.lock().unwrap().handle_set_queues(curr_queues as u32)
                    })?;
                    return virtio_handle_ctrl_status::<AS>(
                        NET_DRIVER_NAME,
                        desc_chain,
                        VIRTIO_NET_OK as u8,
                        mem,
                    );
                }
                _ => error!(
                    "{}: unknown net control request class: 0x{:x}",
                    NET_DRIVER_NAME, ctrl_hdr.class
                ),
            }
        }
        Ok(0)
    }
}

impl<AS, Q, R> MutEventSubscriber for VhostUserNetHandler<AS, Q, R>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    fn process(&mut self, events: Events, ops: &mut EventOps) {
        match events.data() {
            LISTENER_SLOT => {
                if let Err(err) = self.device().handle_connect(ops, events, self) {
                    warn!(
                        "{}: failed to accept incoming connection, {:?}",
                        self.id, err
                    );
                }
            }
            MASTER_SLOT => {
                if let Err(e) = self.device().handle_disconnect(ops) {
                    warn!("{}: failed to handle disconnect event, {:?}", self.id, e);
                }
            }
            CTRL_SLOT => {
                if let Some(config) = self.config.ctrl_queue.as_ref() {
                    if let Err(err) = config.consume_event() {
                        error!("{}: failed to read eventfd, {:?}", self.id, err);
                    } else if let Err(err) = self.process_ctrl_request() {
                        error!(
                            "{}: failed to handle control queue request, {:?}",
                            self.id, err
                        );
                    }
                }
            }
            _ => warn!("{}: unknown epoll event slot {}", self.id, events.data()),
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        trace!(target: "vhost-net", "{}: VhostUserFsHandler::init()", self.id);
        // Do not detect socket disconnection between the time window to
        // register epoll events. Though it has dependency on the presence
        // of self.connection, but it doesn't really send/receive data
        // through the socket, so delay the detection of disconnect to the
        // registered connection monitor handler.
        let device = self.device();
        if let Err(err) = device.endpoint.register_epoll_event(ops) {
            error!(
                "{}: failed to register epoll event for endpoint, {:?}",
                self.id, err
            );
        }
        if let Err(e) = device.listener.register_epoll_event(ops) {
            error!(
                "{}: failed to register epoll event for listener, {:?}",
                self.id, e
            );
        }
        if let Some(config) = self.config.ctrl_queue.as_ref() {
            let event = Events::with_data(&config.eventfd, CTRL_SLOT, EventSet::IN);
            if let Err(err) = ops.add(event) {
                error!(
                    "{}: failed to register epoll event for control queue, {:?}",
                    self.id, err
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::{InterruptManager, InterruptSourceType, MsiNotifier, NoopNotifier};
    use dbs_utils::epoll_manager::EpollManager;
    use kvm_ioctls::Kvm;
    use vhost_rs::vhost_user::message::VhostUserU64;
    use vhost_rs::vhost_user::{VhostUserProtocolFeatures, VhostUserVirtioFeatures};
    use virtio_queue::QueueSync;
    use vm_memory::{FileOffset, GuestAddress, GuestMemoryMmap, GuestRegionMmap};
    use vmm_sys_util::tempfile::TempFile;

    use crate::tests::create_address_space;
    use crate::vhost::vhost_user::net::VhostUserNet;
    use crate::vhost::vhost_user::test_utils::{
        negotiate_slave, Endpoint, MasterReq, VhostUserMsgHeader,
    };
    use crate::{VirtioDevice, VirtioDeviceConfig, VirtioQueueConfig, TYPE_NET};

    fn connect_slave(path: &str) -> Option<Endpoint<MasterReq>> {
        let mut retry_count = 5;
        loop {
            match Endpoint::<MasterReq>::connect(path) {
                Ok(endpoint) => return Some(endpoint),
                Err(_) => {
                    if retry_count > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        retry_count -= 1;
                        continue;
                    } else {
                        return None;
                    }
                }
            }
        }
    }

    fn create_vhost_user_net_slave(slave: &mut Endpoint<MasterReq>) {
        let (hdr, rfds) = slave.recv_header().unwrap();
        assert_eq!(hdr.get_code(), MasterReq::GET_FEATURES);
        assert!(rfds.is_none());
        let vfeatures = 0x15 | VhostUserVirtioFeatures::PROTOCOL_FEATURES.bits();
        let hdr = VhostUserMsgHeader::new(MasterReq::GET_FEATURES, 0x4, 8);
        let msg = VhostUserU64::new(vfeatures);
        slave.send_message(&hdr, &msg, None).unwrap();
    }

    #[test]
    fn test_vhost_user_net_virtio_device_normal() {
        let device_socket = "/tmp/vhost.1";
        let queue_sizes = Arc::new(vec![128]);
        let epoll_mgr = EpollManager::default();
        let handler = thread::spawn(move || {
            let mut slave = connect_slave(device_socket).unwrap();
            create_vhost_user_net_slave(&mut slave);
        });
        let mut dev: VhostUserNet<Arc<GuestMemoryMmap>> =
            VhostUserNet::new_server(device_socket, None, queue_sizes, epoll_mgr).unwrap();
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::device_type(&dev),
            TYPE_NET
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
    fn test_vhost_user_net_virtio_device_activate() {
        let device_socket = "/tmp/vhost.1";
        let queue_sizes = Arc::new(vec![128]);
        let epoll_mgr = EpollManager::default();
        let handler = thread::spawn(move || {
            let mut slave = connect_slave(device_socket).unwrap();
            create_vhost_user_net_slave(&mut slave);
            let mut pfeatures = VhostUserProtocolFeatures::all();
            // A workaround for no support for `INFLIGHT_SHMFD`. File an issue to track
            // this: https://github.com/kata-containers/kata-containers/issues/8705.
            pfeatures -= VhostUserProtocolFeatures::INFLIGHT_SHMFD;
            negotiate_slave(&mut slave, pfeatures, true, 1);
        });
        let mut dev: VhostUserNet<Arc<GuestMemoryMmap>> =
            VhostUserNet::new_server(device_socket, None, queue_sizes, epoll_mgr).unwrap();
        // invalid queue size
        {
            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let resources = DeviceResources::new();
            let queues = vec![
                VirtioQueueConfig::create(128, 0).unwrap(),
                VirtioQueueConfig::create(128, 0).unwrap(),
            ];
            let address_space = create_address_space();
            let config =
                VirtioDeviceConfig::<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>::new(
                    Arc::new(mem),
                    address_space,
                    vm_fd,
                    resources,
                    queues,
                    None,
                    Arc::new(NoopNotifier::default()),
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
            let mut queues = vec![VirtioQueueConfig::create(128, 0).unwrap()];
            queues[0].set_interrupt_notifier(Arc::new(notifier));
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
            let config =
                VirtioDeviceConfig::<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>::new(
                    Arc::new(mem),
                    address_space,
                    vm_fd,
                    resources,
                    queues,
                    None,
                    Arc::new(NoopNotifier::default()),
                );
            dev.activate(config).unwrap();
        }
        handler.join().unwrap();
    }
}
