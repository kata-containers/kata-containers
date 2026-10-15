// Copyright 2026 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::ops::Deref;
use std::sync::Arc;

use dbs_utils::epoll_manager::{EventOps, EventSet, Events, MutEventSubscriber};
use log::error;
use virtio_queue::{QueueOwnedT, QueueSync, QueueT};
use vm_memory::{Bytes, GuestAddressSpace, GuestMemoryRegion, GuestRegionMmap, MmapRegion};

use crate::{DbsGuestAddressSpace, VirtioDeviceConfig};

const VIRTIO_PMEM_REQ_TYPE_FLUSH: u32 = 0;

pub(crate) struct PmemEpollHandler<
    AS: GuestAddressSpace,
    Q: QueueT + Send = QueueSync,
    R: GuestMemoryRegion = GuestRegionMmap,
> {
    id: String,
    config: VirtioDeviceConfig<AS, Q, R>,
    region: Arc<GuestRegionMmap>,
    _mapping: Arc<MmapRegion>,
    size: u64,
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion> PmemEpollHandler<AS, Q, R> {
    fn do_flush(region: &GuestRegionMmap, size: u64, id: &str) -> i32 {
        // SAFETY: The caller keeps the file-backed mapping alive for `size` bytes.
        let ret = unsafe {
            libc::msync(
                region.as_ptr() as *mut libc::c_void,
                size as usize,
                libc::MS_SYNC,
            )
        };
        if ret < 0 {
            error!(
                target: "virtio-pmem",
                "{}: msync failed: {}",
                id,
                std::io::Error::last_os_error(),
            );
            -1
        } else {
            0
        }
    }

    fn process_queue(&mut self, queue_index: usize) -> bool {
        let guard = self.config.lock_guest_memory();
        let mem = guard.deref().memory();
        let region = Arc::clone(&self.region);
        let size = self.size;
        let id = self.id.as_str();

        let queue = &mut self.config.queues[queue_index];
        let mut queue_guard = queue.queue_mut().lock();
        let queue_size = queue_guard.size();
        let mut used_desc_heads = Vec::with_capacity(queue_size as usize);

        let mut iter = match queue_guard.iter(mem) {
            Ok(iter) => iter,
            Err(e) => {
                error!(target: "virtio-pmem", "{}: failed to process queue: {}", self.id, e);
                return false;
            }
        };

        let mut flush_result: Option<i32> = None;
        let mut first_error: Option<String> = None;

        for mut desc_chain in &mut iter {
            let head_index = desc_chain.head_index();
            if head_index >= queue_size {
                first_error.get_or_insert_with(|| {
                    format!(
                        "invalid descriptor chain head index {head_index}, queue size {queue_size}"
                    )
                });
                continue;
            }

            let mut len = 0;
            if let (Some(req), Some(resp)) = (desc_chain.next(), desc_chain.next()) {
                let malformed = req.is_write_only()
                    || req.len() != 4
                    || !resp.is_write_only()
                    || resp.len() != 4;
                let req_type = if malformed {
                    None
                } else {
                    mem.read_obj::<[u8; 4]>(req.addr())
                        .ok()
                        .map(u32::from_le_bytes)
                };

                match req_type {
                    Some(VIRTIO_PMEM_REQ_TYPE_FLUSH) => {
                        let status =
                            flush_result.get_or_insert_with(|| Self::do_flush(&region, size, id));
                        match mem.write_obj(status.to_le_bytes(), resp.addr()) {
                            Ok(_) => len = 4,
                            Err(e) => {
                                first_error
                                    .get_or_insert_with(|| format!("failed to write status: {e}"));
                            }
                        }
                    }
                    Some(t) => {
                        first_error
                            .get_or_insert_with(|| format!("unsupported request type {t:#x}"));
                    }
                    None => {
                        first_error.get_or_insert_with(|| "malformed flush request".into());
                    }
                }
            } else {
                first_error.get_or_insert_with(|| "descriptor chain too short".into());
            }
            used_desc_heads.push((head_index, len));
        }

        if let Some(e) = first_error {
            error!(
                target: "virtio-pmem",
                "{}: failed to process pmem queue: {}",
                self.id, e
            );
        }

        let mut nr_used = 0;
        for &(desc_index, len) in &used_desc_heads {
            match queue_guard.add_used(mem, desc_index, len) {
                Ok(()) => nr_used += 1,
                Err(e) => error!(
                    target: "virtio-pmem",
                    "{}: failed to add used descriptor {}: {}",
                    self.id,
                    desc_index,
                    e
                ),
            }
        }
        nr_used > 0
    }
}

impl<AS, Q, R> MutEventSubscriber for PmemEpollHandler<AS, Q, R>
where
    AS: 'static + GuestAddressSpace + Send + Sync,
    Q: QueueT + Send,
    R: GuestMemoryRegion,
{
    fn init(&mut self, ops: &mut EventOps) {
        for (idx, queue) in self.config.queues.iter().enumerate() {
            let event = Events::with_data(queue.eventfd.as_ref(), idx as u32, EventSet::IN);
            if let Err(e) = ops.add(event) {
                error!(target: "virtio-pmem", "{}: failed to register queue event: {:?}", self.id, e);
            }
        }
    }

    fn process(&mut self, events: Events, _ops: &mut EventOps) {
        let idx = events.data() as usize;
        if idx >= self.config.queues.len() {
            error!(target: "virtio-pmem", "{}: invalid queue index {}", self.id, idx);
            return;
        }
        if let Err(e) = self.config.queues[idx].consume_event() {
            error!(target: "virtio-pmem", "{}: failed to get queue event: {:?}", self.id, e);
        } else if self.process_queue(idx) {
            if let Err(e) = self.config.queues[idx].notify() {
                error!(target: "virtio-pmem", "{}: failed to signal used queue: {}", self.id, e);
            }
        }
    }
}
