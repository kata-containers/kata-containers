// Copyright (C) 2020 Alibaba Cloud Computing. All rights reserved.
// Copyright (c) 2020 Ant Financial
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
#![allow(dead_code)]

use std::any::Any;
use std::cmp;
use std::convert::TryFrom;
use std::io::{self, Write};
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::Deref;
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use dbs_device::resources::ResourceConstraint;
use dbs_interrupt::{InterruptNotifier, NoopNotifier};
use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};
use dbs_utils::metric::{IncMetric, SharedIncMetric, SharedStoreMetric, StoreMetric};
use log::{debug, error, info, trace};
use serde::Serialize;
use virtio_bindings::bindings::virtio_blk::VIRTIO_F_VERSION_1;
use virtio_queue::{QueueOwnedT, QueueSync, QueueT};
use vm_memory::{
    ByteValued, Bytes, GuestAddress, GuestAddressSpace, GuestMemory, GuestMemoryRegion,
    GuestRegionMmap, MemoryRegionAddress,
};

use crate::device::{VirtioDevice, VirtioDeviceConfig, VirtioDeviceInfo, VirtioQueueConfig};
use crate::{
    ActivateResult, ConfigError, ConfigResult, DbsGuestAddressSpace, Error, Result, TYPE_BALLOON,
};

const BALLOON_DRIVER_NAME: &str = "virtio-balloon";

// Supported fields in the configuration space:
const CONFIG_SPACE_SIZE: usize = 16;

const QUEUE_SIZE: u16 = 128;
const NUM_QUEUES: usize = 2;
const QUEUE_SIZES: &[u16] = &[QUEUE_SIZE; NUM_QUEUES];
const PMD_SHIFT: u64 = 21;
const PMD_SIZE: u64 = 1 << PMD_SHIFT;

// New descriptors are pending on the virtio queue.
const INFLATE_QUEUE_AVAIL_EVENT: u32 = 0;
// New descriptors are pending on the virtio queue.
const DEFLATE_QUEUE_AVAIL_EVENT: u32 = 1;
// New descriptors are pending on the virtio queue.
const REPORTING_QUEUE_AVAIL_EVENT: u32 = 2;
// The device has been dropped.
const KILL_EVENT: u32 = 3;
// The device should be paused.
const PAUSE_EVENT: u32 = 4;
const BALLOON_EVENTS_COUNT: u32 = 5;

// Page shift in the host.
const PAGE_SHIFT: u32 = 12;
// Huge Page shift in the host.
const HUGE_PAGE_SHIFT: u32 = 21;

// Size of a PFN in the balloon interface.
const VIRTIO_BALLOON_PFN_SHIFT: u64 = 12;
// feature to deflate balloon on OOM
const VIRTIO_BALLOON_F_DEFLATE_ON_OOM: usize = 2;
// feature to enable free page reporting
const VIRTIO_BALLOON_F_REPORTING: usize = 5;

// The PAGE_REPORTING_CAPACITY of CLH is set to 32.
// This value is got from patch in https://patchwork.kernel.org/patch/11377073/.
// But dragonball reporting capacity is set to 128 in before.
// So I keep 128.
const PAGE_REPORTING_CAPACITY: u16 = 128;

#[derive(Debug, thiserror::Error)]
pub enum BalloonError {}

/// Balloon Device associated metrics.
#[derive(Default, Serialize)]
pub struct BalloonDeviceMetrics {
    /// Number of times when handling events on a balloon device.
    pub event_count: SharedIncMetric,
    /// Number of times when activate failed on a balloon device.
    pub activate_fails: SharedIncMetric,
    /// Number of balloon device inflations.
    pub inflate_count: SharedIncMetric,
    /// Number of balloon device deflations.
    pub deflate_count: SharedIncMetric,
    /// Memory size(mb) of balloon device.
    pub balloon_size_mb: SharedStoreMetric,
    /// Number of balloon device reportions
    pub reporting_count: SharedIncMetric,
    /// Number of times when handling events on a balloon device failed.
    pub event_fails: SharedIncMetric,
}

pub type BalloonResult<T> = std::result::Result<T, BalloonError>;

// Got from include/uapi/linux/virtio_balloon.h
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct VirtioBalloonConfig {
    // Number of pages host wants Guest to give up.
    pub(crate) num_pages: u32,
    // Number of pages we've actually got in balloon.
    pub(crate) actual: u32,
}

// Safe because it only has data and has no implicit padding.
unsafe impl ByteValued for VirtioBalloonConfig {}

pub struct BalloonEpollHandler<
    AS: GuestAddressSpace,
    Q: QueueT + Send = QueueSync,
    R: GuestMemoryRegion = GuestRegionMmap,
> {
    pub(crate) config: VirtioDeviceConfig<AS, Q, R>,
    pub(crate) inflate: VirtioQueueConfig<Q>,
    pub(crate) deflate: VirtioQueueConfig<Q>,
    pub(crate) reporting: Option<VirtioQueueConfig<Q>>,
    balloon_config: Arc<Mutex<VirtioBalloonConfig>>,
    metrics: Arc<BalloonDeviceMetrics>,
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion>
    BalloonEpollHandler<AS, Q, R>
{
    fn process_reporting_queue(&mut self) -> bool {
        self.metrics.reporting_count.inc();
        if let Some(queue) = &mut self.reporting {
            if let Err(e) = queue.consume_event() {
                error!("Failed to get reporting queue event: {:?}", e);
                return false;
            }
            let mut used_desc_heads = [(0, 0); QUEUE_SIZE as usize];
            let mut used_count = 0;
            let conf = &mut self.config;
            let guard = conf.lock_guest_memory();
            let mem = guard.deref().memory();

            let mut queue_guard = queue.queue_mut().lock();

            let mut iter = match queue_guard.iter(mem) {
                Err(e) => {
                    error!("virtio-balloon: failed to process reporting queue. {}", e);
                    return false;
                }
                Ok(iter) => iter,
            };

            for mut desc_chain in &mut iter {
                let mut next_desc = desc_chain.next();
                let mut len = 0;
                while let Some(avail_desc) = next_desc {
                    if avail_desc.len() as usize % size_of::<u32>() != 0 {
                        error!("the request size {} is not right", avail_desc.len());
                        break;
                    }
                    let size = avail_desc.len();
                    let addr = avail_desc.addr();
                    len += size;

                    if let Some(region) = mem.find_region(addr) {
                        let host_addr = match mem.get_host_address(addr) {
                            Ok(v) => v,
                            Err(e) => {
                                error!("virtio-balloon get host address failed! addr:{:x} size: {:x} error:{:?}", addr.0, size, e);
                                break;
                            }
                        };
                        if region.file_offset().is_some() {
                            // when guest memory has file backend we use fallocate free memory
                            let file_offset = region.file_offset().unwrap();
                            let file_fd = file_offset.file().as_raw_fd();
                            let file_start = file_offset.start();
                            let mode = libc::FALLOC_FL_PUNCH_HOLE | libc::FALLOC_FL_KEEP_SIZE;
                            let start_addr =
                                region.get_host_address(MemoryRegionAddress(0)).unwrap();
                            let offset = file_start as i64 + host_addr as i64 - start_addr as i64;
                            if let Err(e) = Self::do_fallocate(file_fd, offset, size as i64, mode) {
                                info!(
                                    "virtio-balloon reporting failed fallocate guest address: {:x}  offset: {:x} size {:x} fd {:?}",
                                    addr.0,
                                    offset,
                                    size,
                                    file_fd
                                );
                                error!("fallocate get error {}", e);
                            }
                        } else {
                            // when guest memory have no file backend or comes from we use madvise free memory
                            let advise = libc::MADV_DONTNEED;
                            if let Err(e) = Self::do_madvise(
                                host_addr as *mut libc::c_void,
                                size as usize,
                                advise,
                            ) {
                                info!(
                                    "guest address: {:?}  host address: {:?} size {:?} advise {:?}",
                                    addr,
                                    host_addr,
                                    1 << PAGE_SHIFT,
                                    advise
                                );
                                error!("madvise get error {}", e);
                            }
                        }
                    }
                    next_desc = desc_chain.next();
                }
                used_desc_heads[used_count] = (desc_chain.head_index(), len);
                used_count += 1;
            }

            drop(queue_guard);

            for &(desc_index, len) in &used_desc_heads[..used_count] {
                queue.add_used(mem, desc_index, len);
            }
            if used_count > 0 {
                match queue.notify() {
                    Ok(_v) => true,
                    Err(e) => {
                        error!(
                            "{}: Failed to signal device change event: {}",
                            BALLOON_DRIVER_NAME, e
                        );
                        false
                    }
                }
            } else {
                true
            }
        } else {
            error!(
                "{}: Invalid event: Free pages reporting was not configured",
                BALLOON_DRIVER_NAME
            );
            false
        }
    }

    fn process_queue(&mut self, idx: u32) -> bool {
        let conf = &mut self.config;
        match idx {
            INFLATE_QUEUE_AVAIL_EVENT => self.metrics.inflate_count.inc(),
            DEFLATE_QUEUE_AVAIL_EVENT => self.metrics.deflate_count.inc(),
            _ => {}
        }
        let queue = match idx {
            INFLATE_QUEUE_AVAIL_EVENT => &mut self.inflate,
            DEFLATE_QUEUE_AVAIL_EVENT => &mut self.deflate,
            _ => {
                error!("{}: unsupport idx {}", BALLOON_DRIVER_NAME, idx);
                return false;
            }
        };

        if let Err(e) = queue.consume_event() {
            error!(
                "{}: Failed to get idx {} queue event: {:?}",
                BALLOON_DRIVER_NAME, idx, e
            );
            return false;
        }

        let mut advice = match idx {
            INFLATE_QUEUE_AVAIL_EVENT => libc::MADV_DONTNEED,
            DEFLATE_QUEUE_AVAIL_EVENT => libc::MADV_WILLNEED,
            _ => {
                error!(
                    "{}: balloon idx: {:?} is not right",
                    BALLOON_DRIVER_NAME, idx
                );
                return false;
            }
        };

        let mut used_desc_heads = [0; QUEUE_SIZE as usize];
        let mut used_count = 0;
        let guard = conf.lock_guest_memory();
        let mem = guard.deref().memory();

        let mut queue_guard = queue.queue_mut().lock();

        let mut iter = match queue_guard.iter(mem) {
            Err(e) => {
                error!("virtio-balloon: failed to process queue. {}", e);
                return false;
            }
            Ok(iter) => iter,
        };

        for mut desc_chain in &mut iter {
            let avail_desc = match desc_chain.next() {
                Some(avail_desc) => avail_desc,
                None => {
                    error!(
                        "{}: Failed to parse balloon available descriptor chain",
                        BALLOON_DRIVER_NAME
                    );
                    return false;
                }
            };

            if avail_desc.is_write_only() {
                error!(
                    "{}: The head contains the request type is not right",
                    BALLOON_DRIVER_NAME
                );
                continue;
            }
            let avail_desc_len = avail_desc.len();
            if avail_desc_len as usize % size_of::<u32>() != 0 {
                error!(
                    "{}: the request size {} is not right",
                    BALLOON_DRIVER_NAME, avail_desc_len
                );
                continue;
            }

            let mut offset = 0u64;
            while offset < avail_desc_len as u64 {
                // Get pfn
                let pfn: u32 = match mem.read_obj(GuestAddress(avail_desc.addr().0 + offset)) {
                    Ok(ret) => ret,
                    Err(e) => {
                        error!(
                            "{}: Fail to read addr {}: {:?}",
                            BALLOON_DRIVER_NAME,
                            avail_desc.addr().0 + offset,
                            e
                        );
                        break;
                    }
                };
                offset += size_of::<u32>() as u64;

                // Get pfn_len
                let pfn_len = match idx {
                    INFLATE_QUEUE_AVAIL_EVENT | DEFLATE_QUEUE_AVAIL_EVENT => 1 << PAGE_SHIFT,
                    _ => {
                        error!(
                            "{}: balloon idx: {:?} is not right",
                            BALLOON_DRIVER_NAME, idx
                        );
                        return false;
                    }
                };

                trace!(
                    "{}: process_queue pfn {} len {}",
                    BALLOON_DRIVER_NAME,
                    pfn,
                    pfn_len
                );

                let guest_addr = (pfn as u64) << VIRTIO_BALLOON_PFN_SHIFT;

                if let Some(region) = mem.find_region(GuestAddress(guest_addr)) {
                    let host_addr = mem.get_host_address(GuestAddress(guest_addr)).unwrap();
                    if advice == libc::MADV_DONTNEED && region.file_offset().is_some() {
                        advice = libc::MADV_REMOVE;
                    }
                    if let Err(e) = Self::do_madvise(
                        host_addr as *mut libc::c_void,
                        pfn_len as libc::size_t,
                        advice,
                    ) {
                        info!(
                            "{}: guest address: {:?}  host address: {:?} size {:?} advise {:?}",
                            BALLOON_DRIVER_NAME, guest_addr, host_addr, pfn_len, advice
                        );
                        error!("{}: madvise get error {}", BALLOON_DRIVER_NAME, e);
                    }
                } else {
                    error!(
                        "{}: guest address 0x{:x} size {:?} advise {:?} is not available",
                        BALLOON_DRIVER_NAME, guest_addr, pfn_len, advice
                    );
                }
            }

            used_desc_heads[used_count] = desc_chain.head_index();
            used_count += 1;
        }

        drop(queue_guard);

        for &desc_index in &used_desc_heads[..used_count] {
            queue.add_used(mem, desc_index, 0);
        }
        if used_count > 0 {
            match queue.notify() {
                Ok(_v) => true,
                Err(e) => {
                    error!(
                        "{}: Failed to signal device queue event: {}",
                        BALLOON_DRIVER_NAME, e
                    );
                    false
                }
            }
        } else {
            true
        }
    }

    fn do_madvise(
        addr: *mut libc::c_void,
        size: libc::size_t,
        advise: libc::c_int,
    ) -> std::result::Result<(), io::Error> {
        let res = unsafe { libc::madvise(addr, size, advise) };
        if res != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn do_fallocate(
        file_fd: RawFd,
        offset: libc::off_t,
        len: libc::off_t,
        mode: libc::c_int,
    ) -> std::result::Result<(), io::Error> {
        let res = unsafe { libc::fallocate(file_fd, mode, offset, len) };
        if res != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion> MutEventSubscriber
    for BalloonEpollHandler<AS, Q, R>
where
    AS: 'static + GuestAddressSpace + Send + Sync,
{
    fn init(&mut self, ops: &mut EventOps) {
        trace!(
            target: BALLOON_DRIVER_NAME,
            "{}: BalloonEpollHandler::init()",
            BALLOON_DRIVER_NAME,
        );
        let events = Events::with_data(
            self.inflate.eventfd.as_ref(),
            INFLATE_QUEUE_AVAIL_EVENT,
            EventSet::IN,
        );
        if let Err(e) = ops.add(events) {
            error!(
                "{}: failed to register INFLATE QUEUE event, {:?}",
                BALLOON_DRIVER_NAME, e
            );
        }

        let events = Events::with_data(
            self.deflate.eventfd.as_ref(),
            DEFLATE_QUEUE_AVAIL_EVENT,
            EventSet::IN,
        );
        if let Err(e) = ops.add(events) {
            error!(
                "{}: failed to register deflate queue event, {:?}",
                BALLOON_DRIVER_NAME, e
            );
        }

        if let Some(reporting) = &self.reporting {
            let events = Events::with_data(
                reporting.eventfd.as_ref(),
                REPORTING_QUEUE_AVAIL_EVENT,
                EventSet::IN,
            );
            if let Err(e) = ops.add(events) {
                error!(
                    "{}: failed to register reporting queue event, {:?}",
                    BALLOON_DRIVER_NAME, e
                );
            }
        }
    }

    fn process(&mut self, events: Events, _ops: &mut EventOps) {
        let guard = self.config.lock_guest_memory();
        let _mem = guard.deref();
        let idx = events.data();

        trace!(
            target: BALLOON_DRIVER_NAME,
            "{}: BalloonEpollHandler::process() idx {}",
            BALLOON_DRIVER_NAME,
            idx
        );
        self.metrics.event_count.inc();
        match idx {
            INFLATE_QUEUE_AVAIL_EVENT | DEFLATE_QUEUE_AVAIL_EVENT => {
                if !self.process_queue(idx) {
                    self.metrics.event_fails.inc();
                    error!("{}: Failed to handle {} queue", BALLOON_DRIVER_NAME, idx);
                }
            }
            REPORTING_QUEUE_AVAIL_EVENT => {
                if !self.process_reporting_queue() {
                    self.metrics.event_fails.inc();
                    error!("Failed to handle reporting queue");
                }
            }
            KILL_EVENT => {
                debug!("kill_evt received");
            }
            _ => {
                error!("{}: unknown idx {}", BALLOON_DRIVER_NAME, idx);
            }
        }
    }
}

fn page_number_to_mib(number: u64) -> u64 {
    number << PAGE_SHIFT >> 10 >> 10
}

fn mib_to_page_number(mib: u64) -> u64 {
    mib << 10 << 10 >> PAGE_SHIFT
}

/// Virtio device for exposing entropy to the guest OS through virtio.
pub struct Balloon<AS: GuestAddressSpace> {
    pub(crate) device_info: VirtioDeviceInfo,
    pub(crate) config: Arc<Mutex<VirtioBalloonConfig>>,
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) device_change_notifier: Arc<dyn InterruptNotifier>,
    pub(crate) subscriber_id: Option<SubscriberId>,
    pub(crate) phantom: PhantomData<AS>,
    metrics: Arc<BalloonDeviceMetrics>,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct BalloonConfig {
    pub f_deflate_on_oom: bool,
    pub f_reporting: bool,
}

impl<AS: GuestAddressSpace> Balloon<AS> {
    // Create a new virtio-balloon.
    pub fn new(epoll_mgr: EpollManager, cfg: BalloonConfig) -> Result<Self> {
        let mut avail_features = 1u64 << VIRTIO_F_VERSION_1;

        let mut queue_sizes = QUEUE_SIZES.to_vec();

        if cfg.f_deflate_on_oom {
            avail_features |= 1u64 << VIRTIO_BALLOON_F_DEFLATE_ON_OOM;
        }
        if cfg.f_reporting {
            avail_features |= 1u64 << VIRTIO_BALLOON_F_REPORTING;
            queue_sizes.push(PAGE_REPORTING_CAPACITY);
        }

        let config = VirtioBalloonConfig::default();

        Ok(Balloon {
            device_info: VirtioDeviceInfo::new(
                BALLOON_DRIVER_NAME.to_string(),
                avail_features,
                Arc::new(queue_sizes),
                config.as_slice().to_vec(),
                epoll_mgr,
            ),
            config: Arc::new(Mutex::new(config)),
            paused: Arc::new(AtomicBool::new(false)),
            device_change_notifier: Arc::new(NoopNotifier::new()),
            subscriber_id: None,
            phantom: PhantomData,
            metrics: Arc::new(BalloonDeviceMetrics::default()),
        })
    }

    pub fn set_size(&self, size_mb: u64) -> Result<()> {
        self.metrics.balloon_size_mb.store(size_mb as usize);
        let num_pages = mib_to_page_number(size_mb);

        let balloon_config = &mut self.config.lock().unwrap();
        balloon_config.num_pages = num_pages as u32;
        if let Err(e) = self.device_change_notifier.notify() {
            error!(
                "{}: failed to signal device change event: {}",
                BALLOON_DRIVER_NAME, e
            );
            return Err(Error::IOError(e));
        }

        Ok(())
    }

    pub fn metrics(&self) -> Arc<BalloonDeviceMetrics> {
        self.metrics.clone()
    }
}

impl<AS, Q, R> VirtioDevice<AS, Q, R> for Balloon<AS>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    fn device_type(&self) -> u32 {
        TYPE_BALLOON
    }

    fn queue_max_sizes(&self) -> &[u16] {
        &self.device_info.queue_sizes
    }

    fn get_avail_features(&self, page: u32) -> u32 {
        self.device_info.get_avail_features(page)
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        trace!(
            target: BALLOON_DRIVER_NAME,
            "{}: VirtioDevice::set_acked_features({}, 0x{:x})",
            BALLOON_DRIVER_NAME,
            page,
            value
        );
        self.device_info.set_acked_features(page, value)
    }

    fn read_config(&mut self, offset: u64, mut data: &mut [u8]) -> ConfigResult {
        trace!(
            target: BALLOON_DRIVER_NAME,
            "{}: VirtioDevice::read_config(0x{:x}, {:?})",
            BALLOON_DRIVER_NAME,
            offset,
            data
        );
        let config = &self.config.lock().unwrap();
        let config_space = config.as_slice().to_vec();
        let config_len = config_space.len() as u64;
        if offset >= config_len {
            error!(
                "{}: config space read request out of range, offset {}",
                BALLOON_DRIVER_NAME, offset
            );
            return Err(ConfigError::InvalidOffset(offset));
        }
        if let Some(end) = offset.checked_add(data.len() as u64) {
            // This write can't fail, offset and end are checked against config_len.
            data.write_all(&config_space[offset as usize..cmp::min(end, config_len) as usize])
                .unwrap();
        }
        Ok(())
    }

    fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
        let config = &mut self.config.lock().unwrap();
        let config_slice = config.as_mut_slice();
        let Ok(start) = usize::try_from(offset) else {
            error!("Failed to write config space");
            return Err(ConfigError::InvalidOffset(offset));
        };
        let Some(dst) = start
            .checked_add(data.len())
            .and_then(|end| config_slice.get_mut(start..end))
        else {
            error!("Failed to write config space");
            return Err(ConfigError::InvalidOffsetPlusDataLen(
                offset + data.len() as u64,
            ));
        };
        dst.copy_from_slice(data);
        Ok(())
    }

    fn activate(&mut self, mut config: VirtioDeviceConfig<AS, Q, R>) -> ActivateResult {
        self.device_info
            .check_queue_sizes(&config.queues)
            .map_err(|e| {
                self.metrics.activate_fails.inc();
                e
            })?;
        self.device_change_notifier = config.device_change_notifier.clone();

        trace!(
            "{}: activate acked_features 0x{:x}",
            BALLOON_DRIVER_NAME,
            self.device_info.acked_features
        );

        let inflate = config.queues.remove(0);
        let deflate = config.queues.remove(0);
        let mut reporting = None;
        if (self.device_info.acked_features & (1u64 << VIRTIO_BALLOON_F_REPORTING)) != 0 {
            reporting = Some(config.queues.remove(0));
        }

        let handler = Box::new(BalloonEpollHandler {
            config,
            inflate,
            deflate,
            reporting,
            balloon_config: self.config.clone(),
            metrics: self.metrics.clone(),
        });

        self.subscriber_id = Some(self.device_info.register_event_handler(handler));

        Ok(())
    }

    fn get_resource_requirements(
        &self,
        requests: &mut Vec<ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        requests.push(ResourceConstraint::LegacyIrq { irq: None });
        if use_generic_irq {
            requests.push(ResourceConstraint::GenericIrq {
                size: (self.device_info.queue_sizes.len() + 1) as u32,
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
pub(crate) mod tests {
    use dbs_device::resources::DeviceResources;
    use dbs_utils::epoll_manager::SubscriberOps;
    use kvm_ioctls::Kvm;
    use vm_memory::GuestMemoryMmap;
    use vmm_sys_util::eventfd::EventFd;

    use super::*;
    use crate::tests::{create_address_space, VirtQueue};

    fn create_balloon_epoll_handler() -> BalloonEpollHandler<Arc<GuestMemoryMmap>> {
        let mem = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0x0), 0x10000)]).unwrap());
        let queues = vec![VirtioQueueConfig::create(128, 0).unwrap()];
        let resources = DeviceResources::new();
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let address_space = create_address_space();

        let config = VirtioDeviceConfig::new(
            mem,
            address_space,
            vm_fd,
            resources,
            queues,
            None,
            Arc::new(NoopNotifier::new()),
        );

        let inflate = VirtioQueueConfig::create(128, 0).unwrap();
        let deflate = VirtioQueueConfig::create(128, 0).unwrap();
        let reporting = Some(VirtioQueueConfig::create(128, 0).unwrap());
        let balloon_config = Arc::new(Mutex::new(VirtioBalloonConfig::default()));
        let metrics = Arc::new(BalloonDeviceMetrics::default());
        BalloonEpollHandler {
            config,
            inflate,
            deflate,
            reporting,
            balloon_config,
            metrics,
        }
    }

    #[test]
    fn test_balloon_page_number_to_mib() {
        assert_eq!(page_number_to_mib(1024), 4);
        assert_eq!(page_number_to_mib(1023), 3);
        assert_eq!(page_number_to_mib(0), 0);
    }

    #[test]
    fn test_balloon_mib_to_page_number() {
        assert_eq!(mib_to_page_number(4), 1024);
        assert_eq!(mib_to_page_number(2), 512);
        assert_eq!(mib_to_page_number(0), 0);
    }

    #[test]
    fn test_balloon_virtio_device_normal() {
        let epoll_mgr = EpollManager::default();
        let config = BalloonConfig {
            f_deflate_on_oom: true,
            f_reporting: true,
        };

        let mut dev = Balloon::<Arc<GuestMemoryMmap>>::new(epoll_mgr, config).unwrap();

        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::device_type(&dev),
            TYPE_BALLOON
        );

        let queue_size = vec![128, 128, 128];
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
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::set_acked_features(
            &mut dev, 2, 0,
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 2),
            0,
        );
        let config: [u8; 8] = [0; 8];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::write_config(
            &mut dev, 0, &config,
        )
        .unwrap();
        let mut data: [u8; 8] = [1; 8];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
            &mut dev, 0, &mut data,
        )
        .unwrap();
        assert_eq!(config, data);
    }

    #[test]
    fn test_balloon_virtio_device_active() {
        let epoll_mgr = EpollManager::default();

        // check queue sizes error
        {
            let config = BalloonConfig {
                f_deflate_on_oom: true,
                f_reporting: true,
            };

            let mut dev = Balloon::<Arc<GuestMemoryMmap>>::new(epoll_mgr.clone(), config).unwrap();
            let queues = vec![
                VirtioQueueConfig::<QueueSync>::create(16, 0).unwrap(),
                VirtioQueueConfig::<QueueSync>::create(16, 0).unwrap(),
            ];

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
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
            assert!(dev.activate(config).is_err());
        }
        // Success
        {
            let config = BalloonConfig {
                f_deflate_on_oom: true,
                f_reporting: true,
            };

            let mut dev = Balloon::<Arc<GuestMemoryMmap>>::new(epoll_mgr, config).unwrap();

            let queues = vec![
                VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap(),
                VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap(),
                VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap(),
            ];

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
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
            assert!(dev.activate(config).is_ok());
        }
    }

    #[test]
    fn test_balloon_set_size() {
        let epoll_mgr = EpollManager::default();
        let config = BalloonConfig {
            f_deflate_on_oom: true,
            f_reporting: true,
        };

        let dev = Balloon::<Arc<GuestMemoryMmap>>::new(epoll_mgr, config).unwrap();
        let size = 1024;
        assert!(dev.set_size(size).is_ok());
    }

    #[test]
    fn test_balloon_epoll_handler_handle_event() {
        let handler = create_balloon_epoll_handler();
        let event_fd = EventFd::new(0).unwrap();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_op = inner_mgr.event_ops(id).unwrap();
        let event_set = EventSet::EDGE_TRIGGERED;
        let mut handler = create_balloon_epoll_handler();

        // test for INFLATE_QUEUE_AVAIL_EVENT
        let events = Events::with_data(&event_fd, INFLATE_QUEUE_AVAIL_EVENT, event_set);
        handler.process(events, &mut event_op);

        // test for DEFLATE_QUEUE_AVAIL_EVENT
        let events = Events::with_data(&event_fd, DEFLATE_QUEUE_AVAIL_EVENT, event_set);
        handler.process(events, &mut event_op);

        // test for REPORTING_QUEUE_AVAIL_EVENT
        let events = Events::with_data(&event_fd, REPORTING_QUEUE_AVAIL_EVENT, event_set);
        handler.process(events, &mut event_op);

        // test for KILL_EVENT
        let events = Events::with_data(&event_fd, KILL_EVENT, event_set);
        handler.process(events, &mut event_op);

        // test for unknown event
        let events = Events::with_data(&event_fd, BALLOON_EVENTS_COUNT + 10, event_set);
        handler.process(events, &mut event_op);
    }

    #[test]
    fn test_balloon_epoll_handler_process_report_queue() {
        let mut handler = create_balloon_epoll_handler();
        let m = &handler.config.vm_as.clone();

        // Failed to get reporting queue event
        assert!(!handler.process_reporting_queue());

        // No reporting queue
        handler.reporting = None;
        assert!(!handler.process_reporting_queue());

        let vq = VirtQueue::new(GuestAddress(0), m, 16);
        let q = vq.create_queue();
        vq.avail.idx().store(1);
        vq.avail.ring(0).store(0);
        vq.dtable(0).set(0x2000, 0x1000, 0, 0);
        let queue_config = VirtioQueueConfig::new(
            q,
            Arc::new(EventFd::new(0).unwrap()),
            Arc::new(NoopNotifier::new()),
            0,
        );
        assert!(queue_config.generate_event().is_ok());
        handler.reporting = Some(queue_config);
        //Success
        assert!(handler.process_reporting_queue());
    }

    #[test]
    fn test_balloon_epoll_handler_process_queue() {
        let mut handler = create_balloon_epoll_handler();
        let m = &handler.config.vm_as.clone();
        // invalid idx
        {
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            let q = vq.create_queue();
            vq.avail.idx().store(1);
            vq.avail.ring(0).store(0);
            vq.dtable(0).set(0x2000, 0x1000, 0, 0);
            let queue_config = VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            );
            assert!(queue_config.generate_event().is_ok());
            handler.inflate = queue_config;
            assert!(!handler.process_queue(10));
        }
        // INFLATE_QUEUE_AVAIL_EVENT
        {
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            let q = vq.create_queue();
            vq.avail.idx().store(1);
            vq.avail.ring(0).store(0);
            vq.dtable(0).set(0x2000, 0x1000, 0, 0);
            vq.dtable(0).set(0x2000, 0x1000, 0, 0);
            let queue_config = VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            );
            assert!(queue_config.generate_event().is_ok());
            handler.inflate = queue_config;
            assert!(handler.process_queue(INFLATE_QUEUE_AVAIL_EVENT));
        }
        // DEFLATE_QUEUE_AVAIL_EVENT
        {
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            let q = vq.create_queue();
            vq.avail.idx().store(1);
            vq.avail.ring(0).store(0);
            vq.dtable(0).set(0x2000, 0x1000, 0, 0);
            let queue_config = VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            );
            assert!(queue_config.generate_event().is_ok());
            handler.deflate = queue_config;
            assert!(handler.process_queue(DEFLATE_QUEUE_AVAIL_EVENT));
        }
    }
}
