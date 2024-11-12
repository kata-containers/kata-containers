// Copyright 2019-2020 Alibnc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::collections::HashMap;
use std::ops::Deref;
use std::os::unix::io::AsRawFd;
use std::sync::mpsc::{Receiver, Sender};

use dbs_utils::{
    epoll_manager::{EventOps, Events, MutEventSubscriber},
    rate_limiter::{BucketUpdate, RateLimiter, TokenType},
};
use log::{debug, error, info, warn};
use virtio_bindings::bindings::virtio_blk::*;
use virtio_queue::{Queue, QueueOwnedT, QueueT};
use vm_memory::{Bytes, GuestAddress, GuestMemory, GuestMemoryRegion, GuestRegionMmap};
use vmm_sys_util::eventfd::EventFd;

use crate::{
    epoll_helper::{EpollHelper, EpollHelperError, EpollHelperHandler},
    DbsGuestAddressSpace, Error, Result, VirtioDeviceConfig, VirtioQueueConfig,
};

use super::{ExecuteError, IoDataDesc, KillEvent, Request, RequestType, Ufile, SECTOR_SHIFT};

// New descriptors are pending on the virtio queue.
pub const QUEUE_AVAIL_EVENT: u32 = 0;
// Rate limiter budget is now available.
pub const RATE_LIMITER_EVENT: u32 = 1;
// Some AIO requests have been completed. Used to support Linux AIO/TDC AIO.
pub const END_IO_EVENT: u32 = 2;
// trigger the thread to deal with some specific event
pub const KILL_EVENT: u32 = 4;

pub(crate) struct InnerBlockEpollHandler<AS: DbsGuestAddressSpace, Q: QueueT> {
    pub(crate) disk_image: Box<dyn Ufile>,
    pub(crate) disk_image_id: Vec<u8>,
    pub(crate) rate_limiter: RateLimiter,
    pub(crate) pending_req_map: HashMap<u16, Request>,
    pub(crate) data_desc_vec: Vec<Vec<IoDataDesc>>,
    pub(crate) iovecs_vec: Vec<Vec<IoDataDesc>>,
    pub(crate) kill_evt: EventFd,
    pub(crate) evt_receiver: Receiver<KillEvent>,

    pub(crate) vm_as: AS,
    pub(crate) queue: VirtioQueueConfig<Q>,
}

impl<AS: DbsGuestAddressSpace, Q: QueueT> InnerBlockEpollHandler<AS, Q> {
    pub(crate) fn process_queue(&mut self) -> bool {
        let as_mem = self.vm_as.memory();
        let mem = as_mem.deref();
        let mut queue = self.queue.queue_mut().lock();

        let mut iter = match queue.iter(mem) {
            Err(e) => {
                error!("virtio-blk: failed to iterate queue. {}", e);
                return false;
            }
            Ok(iter) => iter,
        };

        // Used to collect used descriptors. (index, size)
        let mut used_desc_vec: Vec<(u16, u32)> = Vec::new();
        let mut rate_limited = false;

        'next_desc: for mut desc_chain in &mut iter {
            // Safe to index data_desc_vec with index, as index has been checked in iterator
            let index = desc_chain.head_index();
            let data_descs = &mut self.data_desc_vec[index as usize];
            let iovecs = &mut self.iovecs_vec[index as usize];
            data_descs.clear();
            iovecs.clear();
            match Request::parse(&mut desc_chain, data_descs, self.disk_image.get_max_size()) {
                Err(e) => {
                    // It's caused by invalid request from guest, simple...
                    debug!("Failed to parse available descriptor chain: {:?}", e);
                    used_desc_vec.push((index, 0));
                }
                Ok(req) => {
                    if Self::trigger_rate_limit(&mut self.rate_limiter, &req, data_descs) {
                        // stop processing the queue
                        rate_limited = true;
                        break 'next_desc;
                    }
                    // We try processing READ/WRITE requests using AIO first, and fallback to
                    // synchronous processing if it fails.
                    match Self::process_aio_request(
                        &req,
                        data_descs,
                        iovecs,
                        &mut self.disk_image,
                        mem,
                    ) {
                        Ok(submited) => {
                            if submited {
                                self.pending_req_map.insert(req.request_index, req.clone());
                                continue 'next_desc;
                            }
                            // Else not Submited, fallback to synchronous processing
                        }
                        Err(_e) => {
                            req.update_status(mem, VIRTIO_BLK_S_IOERR);
                            used_desc_vec.push((index, 0));
                            continue 'next_desc;
                        }
                    }
                    // Synchronously execute the request
                    // Take a new immutable data_descs reference, as previous mutable one may have
                    // been consumed.
                    let data_descs = &self.data_desc_vec[req.request_index as usize];
                    match Self::process_request(
                        &req,
                        &data_descs[..],
                        &mut self.disk_image,
                        &self.disk_image_id,
                        mem,
                    ) {
                        Ok(num_bytes_to_mem) => {
                            used_desc_vec.push((index, num_bytes_to_mem));
                        }
                        Err(_e) => {
                            //METRICS.block.execute_fails.inc();
                            used_desc_vec.push((index, 0));
                        }
                    }
                }
            }
        }
        if rate_limited {
            // If rate limiting kicked in, queue had advanced one element that we aborted
            // processing; go back one element so it can be processed next time.
            // TODO: log rate limit message or METRIC
            iter.go_to_previous_position();
        }
        drop(queue);
        if !used_desc_vec.is_empty() {
            for entry in &used_desc_vec {
                self.queue.add_used(mem, entry.0, entry.1);
            }
            true
        } else {
            false
        }
    }

    fn trigger_rate_limit(
        rate_limiter: &mut RateLimiter,
        req: &Request,
        data_descs: &[IoDataDesc],
    ) -> bool {
        // If limiter.consume() fails it means there is no more TokenType::Ops budget
        // and rate limiting is in effect.
        if !rate_limiter.consume(1, TokenType::Ops) {
            // stop processing the queue
            return true;
        }
        // Exercise the rate limiter only if this request is of data transfer type.
        if req.request_type == RequestType::In || req.request_type == RequestType::Out {
            // If limiter.consume() fails it means there is no more TokenType::Bytes
            // budget and rate limiting is in effect.

            if !rate_limiter.consume(u64::from(req.data_len(data_descs)), TokenType::Bytes) {
                // Revert the OPS consume().
                rate_limiter.manual_replenish(1, TokenType::Ops);
                return true;
            }
        }
        false
    }

    fn process_request<M: GuestMemory>(
        req: &Request,
        data_descs: &[IoDataDesc],
        disk_image: &mut Box<dyn Ufile>,
        disk_image_id: &[u8],
        mem: &M,
    ) -> std::result::Result<u32, ExecuteError> {
        match req.execute(disk_image, mem, data_descs, disk_image_id) {
            Ok(l) => {
                req.update_status(mem, VIRTIO_BLK_S_OK);
                Ok(l)
            }
            Err(e) => {
                let err_code = match &e {
                    ExecuteError::BadRequest(e) => {
                        // It's caused by invalid request from guest, simple...
                        debug!("Failed to execute GetDeviceID request: {:?}", e);
                        VIRTIO_BLK_S_IOERR
                    }
                    ExecuteError::Flush(e) => {
                        // only temporary errors are possible here
                        // TODO recovery
                        debug!("Failed to execute Flush request: {:?}", e);
                        VIRTIO_BLK_S_IOERR
                    }
                    ExecuteError::Read(e) | ExecuteError::Write(e) => {
                        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                        // The error recovery policy here is a little messy.
                        // We can't tell the error type from the returned error code
                        // and no easy way to recover.
                        // Hopefully AIO are used and read/write requests never ever
                        // reaches here when TDC live upgrading is enabled.
                        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                        warn!("virtio-blk: Failed to execute Read/Write request: {:?}", e);
                        VIRTIO_BLK_S_IOERR
                    }
                    ExecuteError::Seek(e) => {
                        // It's caused by invalid request from guest, simple...
                        warn!(
                            "virtio-blk: Failed to execute out-of-boundary request: {:?}",
                            e
                        );
                        VIRTIO_BLK_S_IOERR
                    }
                    ExecuteError::GetDeviceID(e) => {
                        // It's caused by invalid request from guest, simple...
                        warn!("virtio-blk: Failed to execute GetDeviceID request: {:?}", e);
                        VIRTIO_BLK_S_IOERR
                    }
                    ExecuteError::Unsupported(e) => {
                        // It's caused by invalid request from guest, simple...
                        warn!("virtio-blk: Failed to execute request: {:?}", e);
                        VIRTIO_BLK_S_UNSUPP
                    }
                };

                req.update_status(mem, err_code);
                Err(e)
            }
        }
    }

    // TODO: We should hide the logic of this function inside the Ufile implementation,
    // instead of appearing here.
    fn process_aio_request<M: GuestMemory>(
        req: &Request,
        data_descs: &[IoDataDesc],
        iovecs: &mut Vec<IoDataDesc>,
        disk_image: &mut Box<dyn Ufile>,
        mem: &M,
    ) -> std::result::Result<bool, ExecuteError> {
        if req.request_type != RequestType::In && req.request_type != RequestType::Out {
            return Ok(false);
        }

        req.check_capacity(disk_image, data_descs).map_err(|e| {
            // It's caused by invalid request from guest, simple...
            debug!("Failed to get buffer address for request");
            e
        })?;

        for io in data_descs {
            let host_addr = mem
                .get_host_address(GuestAddress(io.data_addr))
                .map_err(|e| {
                    // It's caused by invalid request from guest, simple...
                    warn!(
                        "virtio-blk: Failed to get buffer guest address {:?} for request {:?}",
                        io.data_addr, req
                    );
                    ExecuteError::BadRequest(Error::GuestMemory(e))
                })?;
            iovecs.push(IoDataDesc {
                data_addr: host_addr as u64,
                data_len: io.data_len,
            });
        }

        let submiter: fn(
            &mut (dyn Ufile + 'static),
            i64,
            &mut Vec<IoDataDesc>,
            u16,
        ) -> std::io::Result<usize> = match req.request_type {
            RequestType::In => Ufile::io_read_submit,
            RequestType::Out => Ufile::io_write_submit,
            _ => panic!(
                "virtio-blk: unexpected request type {:?} in async I/O",
                req.request_type
            ),
        };

        match submiter(
            disk_image.as_mut(),
            (req.sector << SECTOR_SHIFT) as i64,
            iovecs,
            req.request_index,
        ) {
            Ok(_) => {
                // The request has been queued waiting for process
                Ok(true)
            }
            Err(e) => {
                warn!("virtio-blk: submit request {:?} error. {}", req, e);
                // Failure may be caused by:
                // no enough resource to queue the AIO request
                // TODO recover

                // Now fallback to synchronous processing
                Ok(false)
            }
        }
    }

    pub(crate) fn io_complete(&mut self) -> Result<()> {
        let as_mem = self.vm_as.memory();
        let mem: &AS::M = as_mem.deref();
        let iovs = self.disk_image.io_complete()?;

        // No data to handle
        if iovs.is_empty() {
            return Ok(());
        }

        for (index, res2) in &iovs {
            match self.pending_req_map.remove(index) {
                Some(req) => {
                    // Just ignore the result of write_obj(). Though we have validated
                    // request.status_addr, but we have released and reacquired the
                    // guest memory object and the guest may have hot-removed the
                    // memory maliciously.
                    let _ = mem.write_obj(*res2 as u8, req.status_addr);
                    let data_descs = &self.data_desc_vec[req.request_index as usize];
                    let len = match req.request_type {
                        RequestType::In => req.data_len(data_descs),
                        RequestType::Out => 0,
                        _ => panic!(
                            "virtio-blk: unexpected request type {:?} in async I/O completion",
                            req.request_type
                        ),
                    };
                    self.queue.add_used(mem, req.request_index, len);
                }
                None => {
                    error!("virtio-blk: Cant't find request for AIO completion event.");
                    // We have run into inconsistent state, let the device manager to do recovery.
                    return Err(Error::InternalError);
                }
            }
        }
        self.queue.notify()
    }

    pub(crate) fn get_patch_rate_limiters(&mut self, bytes: BucketUpdate, ops: BucketUpdate) {
        self.rate_limiter.update_buckets(bytes, ops);
        info!(
            "virtio-blk: Update rate limiter for block device {:?}",
            String::from_utf8(self.disk_image_id.clone())
        );
    }

    pub(crate) fn run(&mut self) -> std::result::Result<(), EpollHelperError> {
        let mut helper = EpollHelper::new()?;
        helper.add_event(self.queue.eventfd.as_raw_fd(), QUEUE_AVAIL_EVENT)?;
        helper.add_event_custom(
            self.disk_image.get_data_evt_fd(),
            END_IO_EVENT,
            epoll::Events::EPOLLIN | epoll::Events::EPOLLET,
        )?;

        helper.add_event(self.rate_limiter.as_raw_fd(), RATE_LIMITER_EVENT)?;

        helper.add_event(self.kill_evt.as_raw_fd(), KILL_EVENT)?;

        helper.run(self)?;

        Ok(())
    }
}

impl<AS: DbsGuestAddressSpace, Q: QueueT> EpollHelperHandler for InnerBlockEpollHandler<AS, Q> {
    fn handle_event(&mut self, _helper: &mut EpollHelper, event: &epoll::Event) -> bool {
        let slot = event.data as u32;
        match slot {
            QUEUE_AVAIL_EVENT => {
                if let Err(e) = self.queue.consume_event() {
                    error!("virtio-blk: failed to get queue event: {:?}", e);
                    return true;
                } else if self.rate_limiter.is_blocked() {
                    // While limiter is blocked, don't process any more requests.
                } else if self.process_queue() {
                    self.queue
                        .notify()
                        .expect("virtio-blk: failed to notify guest");
                }
            }
            END_IO_EVENT => {
                // NOTE: Here we should drain io event fd, but different Ufile implementations
                // may use different Events, and complete may depend on the count of reads from
                // within io event. so leave it to IoEngine::complete to drain event fd.
                // io_complete() only returns permanent errors.
                self.io_complete()
                    .expect("virtio-blk: failed to complete IO requests");
            }
            RATE_LIMITER_EVENT => {
                // Upon rate limiter event, call the rate limiter handler
                // and restart processing the queue.
                if self.rate_limiter.event_handler().is_ok() && self.process_queue() {
                    self.queue
                        .notify()
                        .expect("virtio-blk: failed to notify guest");
                }
            }
            KILL_EVENT => {
                let _ = self.kill_evt.read();
                while let Ok(evt) = self.evt_receiver.try_recv() {
                    match evt {
                        KillEvent::Kill => {
                            info!("virtio-blk: KILL_EVENT received, stopping inner epoll handler loop");

                            return true;
                        }
                        KillEvent::BucketUpdate(bytes, ops) => {
                            info!(
                                "virtio-blk: patch the io limiter bucket: {:?}, {:?}",
                                &bytes, &ops
                            );
                            self.get_patch_rate_limiters(bytes, ops);
                        }
                    }
                }
            }
            _ => panic!("virtio_blk: unknown event slot {}", slot),
        }
        false
    }
}

#[allow(dead_code)]
pub(crate) struct BlockEpollHandler<
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send = Queue,
    R: GuestMemoryRegion = GuestRegionMmap,
> {
    pub(crate) evt_senders: Vec<Sender<KillEvent>>,
    pub(crate) kill_evts: Vec<EventFd>,
    pub(crate) config: VirtioDeviceConfig<AS, Q, R>,
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion> MutEventSubscriber
    for BlockEpollHandler<AS, Q, R>
{
    // a dumb impl for BlockEpollHandler to registe event manager for io drain.
    fn process(&mut self, _events: Events, _ops: &mut EventOps) {}
    fn init(&mut self, _ops: &mut EventOps) {}
}
