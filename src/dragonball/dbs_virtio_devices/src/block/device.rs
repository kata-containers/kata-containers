// Copyright 2019-2020 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::any::Any;
use std::collections::HashMap;
use std::io::{Seek, SeekFrom};
use std::marker::PhantomData;
use std::sync::{mpsc, Arc};
use std::thread;

use dbs_device::resources::ResourceConstraint;
use dbs_utils::{
    epoll_manager::{EpollManager, SubscriberId},
    rate_limiter::{BucketUpdate, RateLimiter},
};
use log::{debug, error, info, warn};
use virtio_bindings::bindings::virtio_blk::*;
use virtio_queue::QueueT;
use vm_memory::GuestMemoryRegion;
use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};

use crate::{
    ActivateError, ActivateResult, ConfigResult, DbsGuestAddressSpace, Error, Result, VirtioDevice,
    VirtioDeviceConfig, VirtioDeviceInfo, TYPE_BLOCK,
};

use super::{
    BlockEpollHandler, InnerBlockEpollHandler, KillEvent, Ufile, BLK_DRIVER_NAME, SECTOR_SHIFT,
    SECTOR_SIZE,
};

/// Supported fields in the configuration space:
/// - 64-bit disk size
/// - 32-bit size max
/// - 32-bit seg max
/// - 16-bit num_queues at offset 34
const CONFIG_SPACE_SIZE: usize = 64;

/// Max segments in a data request.
const CONFIG_MAX_SEG: u32 = 16;

fn build_device_id(disk_image: &dyn Ufile) -> Vec<u8> {
    let mut default_disk_image_id = vec![0; VIRTIO_BLK_ID_BYTES as usize];
    match disk_image.get_device_id() {
        Err(_) => warn!("Could not generate device id. We'll use a default."),
        Ok(m) => {
            // The kernel only knows to read a maximum of VIRTIO_BLK_ID_BYTES.
            // This will also zero out any leftover bytes.
            let disk_id = m.as_bytes();
            let bytes_to_copy = std::cmp::min(disk_id.len(), VIRTIO_BLK_ID_BYTES as usize);
            default_disk_image_id[..bytes_to_copy].clone_from_slice(&disk_id[..bytes_to_copy])
        }
    }
    default_disk_image_id
}

/// Virtio device for exposing block level read/write operations on a host file.
pub struct Block<AS: DbsGuestAddressSpace> {
    pub(crate) device_info: VirtioDeviceInfo,
    disk_images: Vec<Box<dyn Ufile>>,
    rate_limiters: Vec<RateLimiter>,
    queue_sizes: Arc<Vec<u16>>,
    subscriber_id: Option<SubscriberId>,
    kill_evts: Vec<EventFd>,
    evt_senders: Vec<mpsc::Sender<KillEvent>>,
    epoll_threads: Vec<thread::JoinHandle<()>>,
    phantom: PhantomData<AS>,
}

impl<AS: DbsGuestAddressSpace> Block<AS> {
    /// Create a new virtio block device that operates on the given file.
    ///
    /// The given file must be seekable and sizable.
    pub fn new(
        mut disk_images: Vec<Box<dyn Ufile>>,
        is_disk_read_only: bool,
        queue_sizes: Arc<Vec<u16>>,
        epoll_mgr: EpollManager,
        rate_limiters: Vec<RateLimiter>,
    ) -> Result<Self> {
        let num_queues = disk_images.len();

        if num_queues == 0 {
            return Err(Error::InvalidInput);
        }

        let disk_image = &mut disk_images[0];

        let disk_size = disk_image.seek(SeekFrom::End(0)).map_err(Error::IOError)?;
        if disk_size % SECTOR_SIZE != 0 {
            warn!(
                "Disk size {} is not a multiple of sector size {}; \
                 the remainder will not be visible to the guest.",
                disk_size, SECTOR_SIZE
            );
        }
        let mut avail_features = 1u64 << VIRTIO_F_VERSION_1;
        avail_features |= 1u64 << VIRTIO_BLK_F_SIZE_MAX;
        avail_features |= 1u64 << VIRTIO_BLK_F_SEG_MAX;

        if is_disk_read_only {
            avail_features |= 1u64 << VIRTIO_BLK_F_RO;
        };

        if num_queues > 1 {
            avail_features |= 1u64 << VIRTIO_BLK_F_MQ;
        }

        let config_space =
            Self::build_config_space(disk_size, disk_image.get_max_size(), num_queues as u16);

        Ok(Block {
            device_info: VirtioDeviceInfo::new(
                BLK_DRIVER_NAME.to_string(),
                avail_features,
                queue_sizes.clone(),
                config_space,
                epoll_mgr,
            ),
            disk_images,
            rate_limiters,
            queue_sizes,
            subscriber_id: None,
            phantom: PhantomData,
            evt_senders: Vec::with_capacity(num_queues),
            kill_evts: Vec::with_capacity(num_queues),
            epoll_threads: Vec::with_capacity(num_queues),
        })
    }

    fn build_config_space(disk_size: u64, max_size: u32, num_queues: u16) -> Vec<u8> {
        // The disk size field of the configuration space, which uses the first two words.
        // If the image is not a multiple of the sector size, the tail bits are not exposed.
        // The config space is little endian.
        let mut config = Vec::with_capacity(CONFIG_SPACE_SIZE);
        let num_sectors = disk_size >> SECTOR_SHIFT;
        for i in 0..8 {
            config.push((num_sectors >> (8 * i)) as u8);
        }

        // The max_size field of the configuration space.
        for i in 0..4 {
            config.push((max_size >> (8 * i)) as u8);
        }

        // The max_seg field of the configuration space.
        let max_segs = CONFIG_MAX_SEG;
        for i in 0..4 {
            config.push((max_segs >> (8 * i)) as u8);
        }

        for _i in 0..18 {
            config.push(0_u8);
        }

        for i in 0..2 {
            config.push((num_queues >> (8 * i)) as u8);
        }

        config
    }

    pub fn set_patch_rate_limiters(&self, bytes: BucketUpdate, ops: BucketUpdate) -> Result<()> {
        if self.evt_senders.is_empty()
            || self.kill_evts.is_empty()
            || self.evt_senders.len() != self.kill_evts.len()
        {
            error!("virtio-blk: failed to establish channel to send rate-limiter patch data");
            return Err(Error::InternalError);
        }

        for sender in self.evt_senders.iter() {
            if sender
                .send(KillEvent::BucketUpdate(bytes.clone(), ops.clone()))
                .is_err()
            {
                error!("virtio-blk: failed to send rate-limiter patch data");
                return Err(Error::InternalError);
            }
        }

        for kill_evt in self.kill_evts.iter() {
            if let Err(e) = kill_evt.write(1) {
                error!(
                    "virtio-blk: failed to write rate-limiter patch event {:?}",
                    e
                );
                return Err(Error::InternalError);
            }
        }

        Ok(())
    }
}

impl<AS, Q, R> VirtioDevice<AS, Q, R> for Block<AS>
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
        self.device_info.get_avail_features(page)
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        self.device_info.set_acked_features(page, value)
    }

    fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
        self.device_info.read_config(offset, data)
    }

    fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
        self.device_info.write_config(offset, data)
    }

    fn activate(&mut self, mut config: VirtioDeviceConfig<AS, Q, R>) -> ActivateResult {
        self.device_info.check_queue_sizes(&config.queues[..])?;

        if self.disk_images.len() != config.queues.len() {
            error!(
                "The disk images number: {} is not equal to queues number: {}",
                self.disk_images.len(),
                config.queues.len()
            );
            return Err(ActivateError::InternalError);
        }
        let mut kill_evts = Vec::with_capacity(self.queue_sizes.len());

        let mut i = 0;
        // first to reverse the queue's order, thus to make sure the following
        // pop queue got the right queue order.
        config.queues.reverse();
        while let Some(queue) = config.queues.pop() {
            let disk_image = self.disk_images.pop().unwrap();
            let disk_image_id = build_device_id(disk_image.as_ref());

            let data_desc_vec =
                vec![Vec::with_capacity(CONFIG_MAX_SEG as usize); self.queue_sizes[0] as usize];
            let iovecs_vec =
                vec![Vec::with_capacity(CONFIG_MAX_SEG as usize); self.queue_sizes[0] as usize];

            let rate_limiter = self.rate_limiters.pop().unwrap_or_default();

            let (evt_sender, evt_receiver) = mpsc::channel();
            self.evt_senders.push(evt_sender);

            let kill_evt = EventFd::new(EFD_NONBLOCK)?;

            let mut handler = Box::new(InnerBlockEpollHandler {
                rate_limiter,
                disk_image,
                disk_image_id,
                pending_req_map: HashMap::new(),
                data_desc_vec,
                iovecs_vec,
                evt_receiver,
                vm_as: config.vm_as.clone(),
                queue,
                kill_evt: kill_evt.try_clone().unwrap(),
            });

            kill_evts.push(kill_evt.try_clone().unwrap());
            self.kill_evts.push(kill_evt);

            thread::Builder::new()
                .name(format!("{}_q{}", "blk_iothread", i))
                .spawn(move || {
                    if let Err(e) = handler.run() {
                        error!("Error running worker: {:?}", e);
                    }
                })
                .map(|thread| self.epoll_threads.push(thread))
                .map_err(|e| {
                    error!("failed to clone the virtio-block epoll thread: {}", e);
                    ActivateError::InternalError
                })?;

            i += 1;
        }
        let block_handler = Box::new(BlockEpollHandler {
            kill_evts,
            evt_senders: self.evt_senders.clone(),
            config,
        });

        // subscribe this handler for io drain.
        self.subscriber_id = Some(self.device_info.register_event_handler(block_handler));

        Ok(())
    }

    fn reset(&mut self) -> ActivateResult {
        Ok(())
    }

    fn remove(&mut self) {
        // if the subsriber_id is invalid, it has not been activated yet.
        if let Some(subscriber_id) = self.subscriber_id {
            // Remove BlockEpollHandler from event manager, so it could be dropped and the resources
            // could be freed, e.g. close disk_image, so vmm won't hold the backend file.
            match self.device_info.remove_event_handler(subscriber_id) {
                Ok(_) => debug!("virtio-blk: removed subscriber_id {:?}", subscriber_id),
                Err(e) => {
                    warn!("virtio-blk: failed to remove event handler: {:?}", e);
                }
            }
        }

        for sender in self.evt_senders.iter() {
            if sender.send(KillEvent::Kill).is_err() {
                error!("virtio-blk: failed to send kill event to epoller thread");
            }
        }

        // notify the io threads handlers to terminate.
        for kill_evt in self.kill_evts.iter() {
            if let Err(e) = kill_evt.write(1) {
                error!("virtio-blk: failed to write kill event {:?}", e);
            }
        }

        while let Some(thread) = self.epoll_threads.pop() {
            if let Err(e) = thread.join() {
                error!("virtio-blk: failed to reap the io threads: {:?}", e);
            } else {
                info!("io thread got reaped.");
            }
        }

        self.subscriber_id = None;
    }

    fn get_resource_requirements(
        &self,
        requests: &mut Vec<ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        requests.push(ResourceConstraint::LegacyIrq { irq: None });
        if use_generic_irq {
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
    use std::io::{self, Read, Seek, SeekFrom, Write};
    use std::os::unix::io::RawFd;

    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::NoopNotifier;
    use dbs_utils::rate_limiter::{TokenBucket, TokenType};
    use kvm_ioctls::Kvm;
    use virtio_queue::QueueSync;
    use vm_memory::{Bytes, GuestAddress, GuestMemoryMmap, GuestRegionMmap};
    use vmm_sys_util::eventfd::EventFd;

    use crate::epoll_helper::*;
    use crate::tests::{create_address_space, VirtQueue, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};
    use crate::{Error as VirtioError, VirtioQueueConfig};

    use super::*;
    use crate::block::*;

    pub(super) struct DummyFile {
        pub(super) device_id: Option<String>,
        pub(super) capacity: u64,
        pub(super) have_complete_io: bool,
        pub(super) max_size: u32,
        pub(super) flush_error: bool,
    }

    impl DummyFile {
        pub(super) fn new() -> Self {
            DummyFile {
                device_id: None,
                capacity: 0,
                have_complete_io: false,
                max_size: 0x100000,
                flush_error: false,
            }
        }
    }

    impl Read for DummyFile {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            Ok(buf.len())
        }
    }

    impl Write for DummyFile {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.flush_error {
                Err(io::Error::new(io::ErrorKind::Other, "test flush error"))
            } else {
                Ok(())
            }
        }
    }
    impl Seek for DummyFile {
        fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
            Ok(0)
        }
    }

    impl Ufile for DummyFile {
        fn get_capacity(&self) -> u64 {
            self.capacity
        }

        fn get_max_size(&self) -> u32 {
            self.max_size
        }

        fn get_device_id(&self) -> io::Result<String> {
            match &self.device_id {
                Some(id) => Ok(id.to_string()),
                None => Err(io::Error::new(io::ErrorKind::Other, "dummy_error")),
            }
        }

        // std err
        fn get_data_evt_fd(&self) -> RawFd {
            2
        }

        fn io_read_submit(
            &mut self,
            _offset: i64,
            _iovecs: &mut Vec<IoDataDesc>,
            _aio_data: u16,
        ) -> io::Result<usize> {
            Ok(0)
        }

        fn io_write_submit(
            &mut self,
            _offset: i64,
            _iovecs: &mut Vec<IoDataDesc>,
            _aio_data: u16,
        ) -> io::Result<usize> {
            Ok(0)
        }

        fn io_complete(&mut self) -> io::Result<Vec<(u16, u32)>> {
            let mut v = Vec::new();
            if self.have_complete_io {
                v.push((0, 1));
            }
            Ok(v)
        }
    }

    #[test]
    fn test_block_build_device_id() {
        let device_id = "dummy_device_id";
        let mut file = DummyFile::new();
        file.device_id = Some(device_id.to_string());
        let disk_image: Box<dyn Ufile> = Box::new(file);
        let disk_id = build_device_id(disk_image.as_ref());
        assert_eq!(disk_id.len() as u32, VIRTIO_BLK_ID_BYTES);
        let disk_image: Box<dyn Ufile> = Box::new(DummyFile::new());
        let disk_id2 = build_device_id(disk_image.as_ref());
        assert_eq!(disk_id2.len() as u32, VIRTIO_BLK_ID_BYTES);
        assert_ne!(disk_id, disk_id2);
    }

    #[test]
    fn test_block_request_parse() {
        let m = &GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = VirtQueue::new(GuestAddress(0), m, 16);
        let mut data_descs = Vec::with_capacity(CONFIG_MAX_SEG as usize);

        assert!(vq.end().0 < 0x1000);

        vq.avail.ring(0).store(0);
        vq.avail.idx().store(1);

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // write only request type descriptor
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_OUT, GuestAddress(0x1000))
                .unwrap();
            m.write_obj::<u64>(114, GuestAddress(0x1000 + 8)).unwrap();
            assert!(matches!(
                Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32),
                Err(Error::UnexpectedWriteOnlyDescriptor)
            ));
        }

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // chain too short; no status_desc
            vq.dtable(0).flags().store(0);
            assert!(matches!(
                Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32),
                Err(Error::DescriptorChainTooShort)
            ));
        }

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // chain too short; no data desc
            vq.dtable(0).flags().store(VIRTQ_DESC_F_NEXT);
            vq.dtable(1).set(0x2000, 0x1000, 0, 2);
            assert!(matches!(
                Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32),
                Err(Error::DescriptorChainTooShort)
            ));
        }

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // write only data for OUT
            vq.dtable(1)
                .flags()
                .store(VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE);
            vq.dtable(2).set(0x3000, 0, 0, 0);
            assert!(matches!(
                Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32),
                Err(Error::UnexpectedWriteOnlyDescriptor)
            ));
        }

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // read only data for OUT
            m.write_obj::<u32>(VIRTIO_BLK_T_OUT, GuestAddress(0x1000))
                .unwrap();
            vq.dtable(1)
                .flags()
                .store(VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE);
            assert!(matches!(
                Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32),
                Err(Error::UnexpectedWriteOnlyDescriptor)
            ));
        }

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // length too big data for OUT
            m.write_obj::<u32>(VIRTIO_BLK_T_OUT, GuestAddress(0x1000))
                .unwrap();
            vq.dtable(1).flags().store(VIRTQ_DESC_F_NEXT);
            vq.dtable(1).len().store(64);
            assert!(matches!(
                Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32),
                Err(Error::DescriptorLengthTooBig)
            ));
        }

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // read only data for IN
            m.write_obj::<u32>(VIRTIO_BLK_T_IN, GuestAddress(0x1000))
                .unwrap();
            vq.dtable(1).flags().store(VIRTQ_DESC_F_NEXT);
            assert!(matches!(
                Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32),
                Err(Error::UnexpectedReadOnlyDescriptor)
            ));
        }

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // length too big data for IN
            m.write_obj::<u32>(VIRTIO_BLK_T_IN, GuestAddress(0x1000))
                .unwrap();
            vq.dtable(1)
                .flags()
                .store(VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE);
            vq.dtable(1).len().store(64);
            assert!(matches!(
                Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32),
                Err(Error::DescriptorLengthTooBig)
            ));
        }

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // data desc write only and request type is getDeviceId
            m.write_obj::<u32>(VIRTIO_BLK_T_GET_ID, GuestAddress(0x1000))
                .unwrap();
            vq.dtable(1)
                .flags()
                .store(VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE);
            assert!(matches!(
                Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32),
                Err(Error::UnexpectedReadOnlyDescriptor)
            ));
        }

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // status desc read only
            vq.dtable(2).flags().store(0);
            assert!(matches!(
                Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32),
                Err(Error::UnexpectedReadOnlyDescriptor)
            ));
        }

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // status desc too small
            vq.dtable(2).flags().store(VIRTQ_DESC_F_WRITE);
            vq.dtable(2).len().store(0);
            assert!(matches!(
                Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32),
                Err(Error::DescriptorLengthTooSmall)
            ));
        }

        {
            let mut q = vq.create_queue();
            data_descs.clear();
            // should be OK now
            vq.dtable(2).len().store(0x1000);
            let r = Request::parse(&mut q.pop_descriptor_chain(m).unwrap(), &mut data_descs, 32)
                .unwrap();

            assert_eq!(r.request_type, RequestType::GetDeviceID);
            assert_eq!(r.sector, 114);
            assert_eq!(data_descs[0].data_addr, 0x2000);
            assert_eq!(data_descs[0].data_len, 0x40);
            assert_eq!(r.status_addr, GuestAddress(0x3000));
        }
    }

    #[test]
    fn test_block_request_execute() {
        let m = &GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = VirtQueue::new(GuestAddress(0), m, 16);
        let mut data_descs = Vec::with_capacity(CONFIG_MAX_SEG as usize);
        assert!(vq.end().0 < 0x1000);
        vq.avail.ring(0).store(0);
        vq.avail.idx().store(1);

        let mut file = DummyFile::new();
        file.capacity = 4096;
        let mut disk: Box<dyn Ufile> = Box::new(file);
        let disk_id = build_device_id(disk.as_ref());

        {
            // RequestType::In
            let mut q = vq.create_queue();
            data_descs.clear();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_IN, GuestAddress(0x1000))
                .unwrap();
            let req = Request::parse(
                &mut q.pop_descriptor_chain(m).unwrap(),
                &mut data_descs,
                0x100000,
            )
            .unwrap();
            assert!(req.execute(&mut disk, m, &data_descs, &disk_id).is_ok());
        }

        {
            // RequestType::Out
            let mut q = vq.create_queue();
            data_descs.clear();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1).set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_OUT, GuestAddress(0x1000))
                .unwrap();
            let req = Request::parse(
                &mut q.pop_descriptor_chain(m).unwrap(),
                &mut data_descs,
                0x100000,
            )
            .unwrap();
            assert!(req.execute(&mut disk, m, &data_descs, &disk_id).is_ok());
        }

        {
            // RequestType::Flush
            let mut q = vq.create_queue();
            data_descs.clear();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1).set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_FLUSH, GuestAddress(0x1000))
                .unwrap();
            let req = Request::parse(
                &mut q.pop_descriptor_chain(m).unwrap(),
                &mut data_descs,
                0x100000,
            )
            .unwrap();
            assert!(req.execute(&mut disk, m, &data_descs, &disk_id).is_ok());
        }

        {
            // RequestType::GetDeviceID
            let mut q = vq.create_queue();
            data_descs.clear();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_GET_ID, GuestAddress(0x1000))
                .unwrap();
            let req = Request::parse(
                &mut q.pop_descriptor_chain(m).unwrap(),
                &mut data_descs,
                0x100000,
            )
            .unwrap();
            assert!(req.execute(&mut disk, m, &data_descs, &disk_id).is_ok());
        }

        {
            // RequestType::unsupport
            let mut q = vq.create_queue();
            data_descs.clear();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_GET_ID + 10, GuestAddress(0x1000))
                .unwrap();
            let req = Request::parse(
                &mut q.pop_descriptor_chain(m).unwrap(),
                &mut data_descs,
                0x100000,
            )
            .unwrap();
            match req.execute(&mut disk, m, &data_descs, &disk_id) {
                Err(ExecuteError::Unsupported(n)) => assert_eq!(n, VIRTIO_BLK_T_GET_ID + 10),
                _ => panic!(),
            }
        }
    }

    #[test]
    fn test_block_request_update_status() {
        let m = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap());
        let vq = VirtQueue::new(GuestAddress(0), &m, 16);
        let mut data_descs = Vec::with_capacity(CONFIG_MAX_SEG as usize);
        assert!(vq.end().0 < 0x1000);
        vq.avail.ring(0).store(0);
        vq.avail.idx().store(1);
        let mut q = vq.create_queue();
        vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
        vq.dtable(1)
            .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
        vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
        m.write_obj::<u32>(VIRTIO_BLK_T_IN, GuestAddress(0x1000))
            .unwrap();
        let req = Request::parse(
            &mut q.pop_descriptor_chain(m.as_ref()).unwrap(),
            &mut data_descs,
            0x100000,
        )
        .unwrap();
        req.update_status(m.as_ref(), 0);
    }

    #[test]
    fn test_block_request_check_capacity() {
        let m = &GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = VirtQueue::new(GuestAddress(0), m, 16);
        let mut data_descs = Vec::with_capacity(CONFIG_MAX_SEG as usize);
        assert!(vq.end().0 < 0x1000);
        vq.avail.ring(0).store(0);
        vq.avail.idx().store(1);

        let mut disk: Box<dyn Ufile> = Box::new(DummyFile::new());
        let disk_id = build_device_id(disk.as_ref());
        let mut q = vq.create_queue();
        vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
        vq.dtable(1)
            .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
        vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
        m.write_obj::<u32>(VIRTIO_BLK_T_IN, GuestAddress(0x1000))
            .unwrap();
        let req = Request::parse(
            &mut q.pop_descriptor_chain(m).unwrap(),
            &mut data_descs,
            0x100000,
        )
        .unwrap();
        assert!(matches!(
            req.execute(&mut disk, m, &data_descs, &disk_id),
            Err(ExecuteError::BadRequest(VirtioError::InvalidOffset))
        ));

        let mut file = DummyFile::new();
        file.capacity = 4096;
        let mut disk: Box<dyn Ufile> = Box::new(file);
        let mut q = vq.create_queue();
        data_descs.clear();
        vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
        vq.dtable(1)
            .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
        vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
        m.write_obj::<u32>(VIRTIO_BLK_T_IN, GuestAddress(0x1000))
            .unwrap();
        let req = Request::parse(
            &mut q.pop_descriptor_chain(m).unwrap(),
            &mut data_descs,
            0x100000,
        )
        .unwrap();
        assert!(req.check_capacity(&mut disk, &data_descs).is_ok());
    }

    #[test]
    fn test_block_virtio_device_normal() {
        let device_id = "dummy_device_id";
        let epoll_mgr = EpollManager::default();

        let mut file = DummyFile::new();
        println!("max size {}", file.max_size);
        file.device_id = Some(device_id.to_string());
        let disk_image: Box<dyn Ufile> = Box::new(file);
        let mut dev = Block::<Arc<GuestMemoryMmap>>::new(
            vec![disk_image],
            true,
            Arc::new(vec![128]),
            epoll_mgr,
            vec![],
        )
        .unwrap();

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
            dev.device_info.get_avail_features(0)
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 1),
            dev.device_info.get_avail_features(1)
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 2),
            dev.device_info.get_avail_features(2)
        );
        let mut config: [u8; 1] = [0];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
            &mut dev,
            0,
            &mut config,
        )
        .unwrap();
        let config: [u8; 16] = [0; 16];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::write_config(
            &mut dev, 0, &config,
        )
        .unwrap();
    }

    #[test]
    fn test_block_virtio_device_active() {
        let device_id = "dummy_device_id";
        let epoll_mgr = EpollManager::default();

        {
            // check_queue_sizes error
            let mut file = DummyFile::new();
            file.device_id = Some(device_id.to_string());
            let disk_image: Box<dyn Ufile> = Box::new(file);
            let mut dev = Block::<Arc<GuestMemoryMmap<()>>>::new(
                vec![disk_image],
                true,
                Arc::new(vec![128]),
                epoll_mgr.clone(),
                vec![],
            )
            .unwrap();

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues = Vec::new();

            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>>::new(
                Arc::new(mem),
                address_space,
                vm_fd,
                resources,
                queues,
                None,
                Arc::new(NoopNotifier::new()),
            );

            assert!(matches!(
                dev.activate(config),
                Err(ActivateError::InvalidParam)
            ));
        }

        {
            // test no disk_image
            let mut file = DummyFile::new();
            file.device_id = Some(device_id.to_string());
            let disk_image: Box<dyn Ufile> = Box::new(file);
            let mut dev = Block::new(
                vec![disk_image],
                true,
                Arc::new(vec![128]),
                epoll_mgr.clone(),
                vec![],
            )
            .unwrap();
            dev.disk_images = vec![];

            let mem = GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues = vec![VirtioQueueConfig::<QueueSync>::create(256, 0).unwrap()];

            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>>::new(
                Arc::new(mem),
                address_space,
                vm_fd,
                resources,
                queues,
                None,
                Arc::new(NoopNotifier::new()),
            );

            assert!(matches!(
                dev.activate(config),
                Err(ActivateError::InternalError)
            ));
        }

        {
            // Ok
            let mut file = DummyFile::new();
            file.device_id = Some(device_id.to_string());
            let disk_image: Box<dyn Ufile> = Box::new(file);
            let mut dev = Block::new(
                vec![disk_image],
                true,
                Arc::new(vec![128]),
                epoll_mgr,
                vec![],
            )
            .unwrap();

            let mem = GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues = vec![VirtioQueueConfig::<QueueSync>::create(256, 0).unwrap()];

            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>>::new(
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
    }

    #[test]
    fn test_block_set_patch_rate_limiters() {
        let device_id = "dummy_device_id";
        let epoll_mgr = EpollManager::default();
        let mut file = DummyFile::new();
        file.device_id = Some(device_id.to_string());
        let disk_image: Box<dyn Ufile> = Box::new(file);
        let mut dev = Block::<Arc<GuestMemoryMmap>>::new(
            vec![disk_image],
            true,
            Arc::new(vec![128]),
            epoll_mgr,
            vec![],
        )
        .unwrap();

        let (sender, _receiver) = mpsc::channel();
        dev.evt_senders = vec![sender];
        let event = EventFd::new(0).unwrap();
        dev.kill_evts = vec![event];

        assert!(dev
            .set_patch_rate_limiters(BucketUpdate::None, BucketUpdate::None)
            .is_ok());
    }

    fn get_block_epoll_handler_with_file(
        file: DummyFile,
    ) -> InnerBlockEpollHandler<Arc<GuestMemoryMmap>, QueueSync> {
        let mem = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0x0), 0x10000)]).unwrap());
        let queue = VirtioQueueConfig::create(256, 0).unwrap();
        let rate_limiter = RateLimiter::default();
        let disk_image: Box<dyn Ufile> = Box::new(file);
        let disk_image_id = build_device_id(disk_image.as_ref());

        let data_desc_vec = vec![Vec::with_capacity(CONFIG_MAX_SEG as usize); 256];
        let iovecs_vec = vec![Vec::with_capacity(CONFIG_MAX_SEG as usize); 256];

        let (_, evt_receiver) = mpsc::channel();

        InnerBlockEpollHandler {
            disk_image,
            disk_image_id,
            rate_limiter,
            pending_req_map: HashMap::new(),
            data_desc_vec,
            iovecs_vec,

            kill_evt: EventFd::new(0).unwrap(),
            evt_receiver,

            vm_as: mem,
            queue,
        }
    }

    fn get_block_epoll_handler() -> InnerBlockEpollHandler<Arc<GuestMemoryMmap>, QueueSync> {
        let mut file = DummyFile::new();
        file.capacity = 0x100000;
        get_block_epoll_handler_with_file(file)
    }

    #[test]
    fn test_block_get_patch_rate_limiters() {
        let mut handler = get_block_epoll_handler();
        let tokenbucket = TokenBucket::new(1, 1, 4);

        handler.get_patch_rate_limiters(
            BucketUpdate::None,
            BucketUpdate::Update(tokenbucket.clone()),
        );
        assert_eq!(handler.rate_limiter.ops().unwrap(), &tokenbucket);
    }

    #[test]
    fn test_block_epoll_handler_handle_event() {
        let mut handler = get_block_epoll_handler();
        let mut helper = EpollHelper::new().unwrap();

        // test for QUEUE_AVAIL_EVENT
        let events = epoll::Event::new(epoll::Events::EPOLLIN, QUEUE_AVAIL_EVENT as u64);
        handler.handle_event(&mut helper, &events);
        handler.queue.generate_event().unwrap();
        handler.handle_event(&mut helper, &events);

        // test for RATE_LIMITER_EVENT
        let events = epoll::Event::new(epoll::Events::EPOLLIN, RATE_LIMITER_EVENT as u64);
        handler.handle_event(&mut helper, &events);

        // test for END_IO_EVENT
        let events = epoll::Event::new(epoll::Events::EPOLLIN, END_IO_EVENT as u64);
        handler.handle_event(&mut helper, &events);
    }

    #[test]
    #[should_panic]
    fn test_block_epoll_handler_handle_unknown_event() {
        let mut handler = get_block_epoll_handler();
        let mut helper = EpollHelper::new().unwrap();

        // test for unknown event
        let events = epoll::Event::new(epoll::Events::EPOLLIN, KILL_EVENT as u64 + 10);
        handler.handle_event(&mut helper, &events);
    }

    #[test]
    fn test_block_epoll_handler_process_queue() {
        {
            let mut file = DummyFile::new();
            file.capacity = 0x100000;
            // set disk max_size to 0 will cause Request parse error
            file.max_size = 0;
            let mut handler = get_block_epoll_handler_with_file(file);

            let m = &handler.vm_as.clone();
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            let q = vq.create_queue();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_IN, GuestAddress(0x1000))
                .unwrap();

            handler.queue = VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            );
            assert!(handler.process_queue());
        }

        {
            // will cause check_capacity error
            let file = DummyFile::new();
            let mut handler = get_block_epoll_handler_with_file(file);
            let m = &handler.vm_as.clone();
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            let q = vq.create_queue();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_IN, GuestAddress(0x1000))
                .unwrap();

            handler.queue = VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            );
            assert!(handler.process_queue());
            let err_info: u32 = handler.vm_as.read_obj(GuestAddress(0x3000)).unwrap();
            assert_eq!(err_info, VIRTIO_BLK_S_IOERR);
        }

        {
            // test io submit
            let mut file = DummyFile::new();
            file.capacity = 0x100000;
            let mut handler = get_block_epoll_handler_with_file(file);
            let m = &handler.vm_as.clone();
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            let q = vq.create_queue();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_IN, GuestAddress(0x1000))
                .unwrap();

            handler.queue = VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            );
            assert!(!handler.process_queue());
            assert_eq!(handler.pending_req_map.len(), 1);
        }

        {
            // test for other execute type (not IN/OUT)
            let mut file = DummyFile::new();
            file.capacity = 0x100000;
            let mut handler = get_block_epoll_handler_with_file(file);
            let m = &handler.vm_as.clone();
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            let q = vq.create_queue();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_FLUSH, GuestAddress(0x1000))
                .unwrap();

            handler.queue = VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            );
            assert!(handler.process_queue());
            let err_info: u32 = handler.vm_as.read_obj(GuestAddress(0x3000)).unwrap();
            assert_eq!(err_info, VIRTIO_BLK_S_OK);
        }

        {
            // test for other execute type (not IN/OUT) : error
            let mut file = DummyFile::new();
            file.capacity = 0x100000;
            file.flush_error = true;
            let mut handler = get_block_epoll_handler_with_file(file);
            let m = &handler.vm_as.clone();
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            let q = vq.create_queue();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_FLUSH, GuestAddress(0x1000))
                .unwrap();

            handler.queue = VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            );
            assert!(handler.process_queue());
            let err_info: u32 = handler.vm_as.read_obj(GuestAddress(0x3000)).unwrap();
            assert_eq!(err_info, VIRTIO_BLK_S_IOERR);
        }

        {
            // test for other execute type (not IN/OUT) : non_supported
            let mut file = DummyFile::new();
            file.capacity = 0x100000;
            let mut handler = get_block_epoll_handler_with_file(file);
            let m = &handler.vm_as.clone();
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            let q = vq.create_queue();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_FLUSH + 10, GuestAddress(0x1000))
                .unwrap();

            handler.queue = VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            );
            assert!(handler.process_queue());
            let err_info: u32 = handler.vm_as.read_obj(GuestAddress(0x3000)).unwrap();
            assert_eq!(err_info, VIRTIO_BLK_S_UNSUPP);
        }

        {
            // test for rate limiter
            let mut file = DummyFile::new();
            file.capacity = 0x100000;
            let mut handler = get_block_epoll_handler_with_file(file);
            handler.rate_limiter = RateLimiter::new(0, 0, 0, 1, 0, 100).unwrap();
            handler.rate_limiter.consume(1, TokenType::Ops);
            let m = &handler.vm_as.clone();
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            let q = vq.create_queue();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u32>(VIRTIO_BLK_T_FLUSH, GuestAddress(0x1000))
                .unwrap();

            handler.queue = VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            );
            assert!(!handler.process_queue());
            // test if rate limited
            assert!(handler.rate_limiter.is_blocked());
        }
    }

    #[test]
    fn test_block_epoll_handler_io_complete() {
        let m = &GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        // no data
        let mut handler = get_block_epoll_handler();
        let mut data_descs = Vec::with_capacity(CONFIG_MAX_SEG as usize);
        assert!(handler.io_complete().is_ok());

        // have data
        let mut file = DummyFile::new();
        file.have_complete_io = true;
        let disk_image = Box::new(file);
        handler.disk_image = disk_image;

        // no data in pending_req_map
        assert!(matches!(handler.io_complete(), Err(Error::InternalError)));

        // data in pending_req_map
        let vq = VirtQueue::new(GuestAddress(0), m, 16);
        assert!(vq.end().0 < 0x1000);
        vq.avail.ring(0).store(0);
        vq.avail.idx().store(1);
        let mut q = vq.create_queue();
        vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
        vq.dtable(1)
            .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
        vq.dtable(2).set(0x0, 1, VIRTQ_DESC_F_WRITE, 1);
        m.write_obj::<u32>(VIRTIO_BLK_T_IN, GuestAddress(0x1000))
            .unwrap();
        let req = Request::parse(
            &mut q.pop_descriptor_chain(m).unwrap(),
            &mut data_descs,
            0x100000,
        )
        .unwrap();
        handler.pending_req_map.insert(0, req);
        handler.io_complete().unwrap();
    }
}
