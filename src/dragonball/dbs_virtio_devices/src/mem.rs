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

use std::any::Any;
use std::cmp;
use std::io::{self, Write};
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::Deref;
use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex};

use dbs_device::resources::{DeviceResources, ResourceConstraint};
use dbs_interrupt::{InterruptNotifier, NoopNotifier};
use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};
use kvm_ioctls::VmFd;
use log::{debug, error, info, trace, warn};
use virtio_bindings::bindings::virtio_blk::VIRTIO_F_VERSION_1;
use virtio_queue::{DescriptorChain, QueueOwnedT, QueueSync, QueueT};
use vm_memory::{
    ByteValued, Bytes, GuestAddress, GuestAddressSpace, GuestMemory, GuestMemoryError,
    GuestMemoryRegion, GuestRegionMmap, GuestUsize, MemoryRegionAddress,
};

use crate::device::{VirtioDevice, VirtioDeviceConfig, VirtioDeviceInfo};
use crate::{
    ActivateError, ActivateResult, ConfigResult, DbsGuestAddressSpace, Error, Result,
    VirtioSharedMemoryList, TYPE_MEM,
};

/// Use 4 MiB alignment because current kernel use it as the subblock_size.
pub const VIRTIO_MEM_DEFAULT_BLOCK_SIZE: u64 = 4 << 20;

/// The memory block size of guest when initial memory is less than 64GiB.
/// When initial memory is more than 64GiB, the memory block size maybe 1GiB or
/// 2GiB, and the specific algorithm is in
/// `arch/x86/mm/int_64.c:memory_block_size_bytes()`. So if we want to use
/// virtio-mem when initial memory is larger than 64GiB, we should use the
/// algorithm in kernel to get the actual memory block size.
pub const VIRTIO_MEM_DEFAULT_BLOCK_ALIGNMENT: u64 = 128 * 1024 * 1024;

const VIRTIO_MEM_MAP_REGION_SHIFT: u64 = 31;
const VIRTIO_MEM_MAP_REGION_SIZE: u64 = 1 << VIRTIO_MEM_MAP_REGION_SHIFT;
const VIRTIO_MEM_MAP_REGION_MASK: u64 = !(std::u64::MAX << VIRTIO_MEM_MAP_REGION_SHIFT);

/// Max memory block size used in guest kernel.
const MAX_MEMORY_BLOCK_SIZE: u64 = 2 << 30;
/// Amount of boot ram to judge whether to use large memory blocks.
const BOOT_MEM_SIZE_FOR_LARGE_BLOCK: u64 = 64 << 30;

const MEM_DRIVER_NAME: &str = "virtio-mem";

const QUEUE_SIZE: u16 = 128;
const NUM_QUEUES: usize = 1;
const QUEUE_SIZES: &[u16] = &[QUEUE_SIZE];

// Request processed successfully, applicable for
// - VIRTIO_MEM_REQ_PLUG
// - VIRTIO_MEM_REQ_UNPLUG
// - VIRTIO_MEM_REQ_UNPLUG_ALL
// - VIRTIO_MEM_REQ_STATE
const VIRTIO_MEM_RESP_ACK: u16 = 0;

// Request denied - e.g. trying to plug more than requested, applicable for
// - VIRTIO_MEM_REQ_PLUG
const VIRTIO_MEM_RESP_NACK: u16 = 1;

// Request cannot be processed right now, try again later, applicable for
// - VIRTIO_MEM_REQ_PLUG
// - VIRTIO_MEM_REQ_UNPLUG
// - VIRTIO_MEM_REQ_UNPLUG_ALL
// VIRTIO_MEM_RESP_BUSY: u16 = 2;

// Error in request (e.g. addresses/alignment), applicable for
// - VIRTIO_MEM_REQ_PLUG
// - VIRTIO_MEM_REQ_UNPLUG
// - VIRTIO_MEM_REQ_STATE
const VIRTIO_MEM_RESP_ERROR: u16 = 3;

// State of memory blocks is "plugged"
const VIRTIO_MEM_STATE_PLUGGED: u16 = 0;
// State of memory blocks is "unplugged"
const VIRTIO_MEM_STATE_UNPLUGGED: u16 = 1;
// State of memory blocks is "mixed"
const VIRTIO_MEM_STATE_MIXED: u16 = 2;

// request to plug memory blocks
const VIRTIO_MEM_REQ_PLUG: u16 = 0;
// request to unplug memory blocks
const VIRTIO_MEM_REQ_UNPLUG: u16 = 1;
// request to unplug all blocks and shrink the usable size
const VIRTIO_MEM_REQ_UNPLUG_ALL: u16 = 2;
// request information about the plugged state of memory blocks
const VIRTIO_MEM_REQ_STATE: u16 = 3;

// Virtio features
const VIRTIO_MEM_F_ACPI_PXM: u8 = 0;

type MapRegions = Arc<Mutex<Vec<(u32, Option<(u64, u64)>)>>>;

type MultiRegions = Option<(MapRegions, Arc<Mutex<dyn MemRegionFactory>>)>;

#[derive(Debug, thiserror::Error)]
pub enum MemError {
    /// Guest gave us bad memory addresses.
    #[error("failed to access guest memory. {0}")]
    GuestMemory(GuestMemoryError),
    /// Guest gave us a write only descriptor that protocol says to read from.
    #[error("unexpected write only descriptor.")]
    UnexpectedWriteOnlyDescriptor,
    /// Guest gave us a read only descriptor that protocol says to write to.
    #[error("unexpected read only descriptor.")]
    UnexpectedReadOnlyDescriptor,
    #[error("not enough descriptors for request.")]
    /// Guest gave us too few descriptors in a descriptor chain.
    DescriptorChainTooShort,
    /// Guest gave us a descriptor that was too short to use.
    #[error("descriptor length too small.")]
    DescriptorLengthTooSmall,
    /// Guest sent us invalid request.
    #[error("Guest sent us invalid request.")]
    InvalidRequest,
    /// virtio-mem resize usable region fail
    #[error("resize usable region fail: {0}")]
    RsizeUsabeRegionFail(String),
}

/// Specialied std::result::Result for virtio-mem related operations.
pub type MemResult<T> = std::result::Result<T, MemError>;

// Got from qemu/include/standard-headers/linux/virtio_mem.h
// rust union doesn't support std::default::Default that
// need by mem.read_obj.
// Then move virtio_mem_req_plug, virtio_mem_req_unplug and
// virtio_mem_req_state to virtio_mem_req.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct VirtioMemReq {
    req_type: u16,
    padding: [u16; 3],
    addr: u64,
    nb_blocks: u16,
}

// Safe because it only has data and has no implicit padding.
unsafe impl ByteValued for VirtioMemReq {}

// Got from qemu/include/standard-headers/linux/virtio_mem.h
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct VirtioMemRespState {
    state: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct VirtioMemResp {
    resp_type: u16,
    padding: [u16; 3],
    state: VirtioMemRespState,
}

// Safe because it only has data and has no implicit padding.
unsafe impl ByteValued for VirtioMemResp {}

// Got from qemu/include/standard-headers/linux/virtio_mem.h
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct VirtioMemConfig {
    /// Block size and alignment. Cannot change.
    pub(crate) block_size: u64,
    /// Valid with VIRTIO_MEM_F_ACPI_PXM. Cannot change.
    pub(crate) node_id: u16,
    pub(crate) padding: [u8; 6],
    /// Start address of the memory region. Cannot change.
    pub(crate) addr: u64,
    /// Region size (maximum). Cannot change.
    pub(crate) region_size: u64,
    /// Currently usable region size. Can grow up to region_size. Can
    /// shrink due to VIRTIO_MEM_REQ_UNPLUG_ALL (in which case no config
    /// update will be sent).
    pub(crate) usable_region_size: u64,
    /// Currently used size. Changes due to plug/unplug requests, but no
    /// config updates will be sent.
    pub(crate) plugged_size: u64,
    /// Requested size. New plug requests cannot exceed it. Can change.
    pub(crate) requested_size: u64,
}

// Safe because it only has data and has no implicit padding.
unsafe impl ByteValued for VirtioMemConfig {}

struct Request {
    req: VirtioMemReq,
    status_addr: GuestAddress,
}

impl Request {
    fn parse<M: GuestMemory>(desc_chain: &mut DescriptorChain<&M>, mem: &M) -> MemResult<Request> {
        let avail_desc = desc_chain.next().ok_or(MemError::DescriptorChainTooShort)?;
        // The head contains the request type which MUST be readable.
        if avail_desc.is_write_only() {
            return Err(MemError::UnexpectedWriteOnlyDescriptor);
        }
        if avail_desc.len() as usize != size_of::<VirtioMemReq>() {
            return Err(MemError::InvalidRequest);
        }
        let req: VirtioMemReq = mem
            .read_obj(avail_desc.addr())
            .map_err(MemError::GuestMemory)?;

        let status_desc = desc_chain.next().ok_or(MemError::DescriptorChainTooShort)?;

        // The status MUST always be writable
        if !status_desc.is_write_only() {
            return Err(MemError::UnexpectedReadOnlyDescriptor);
        }

        if (status_desc.len() as usize) < size_of::<VirtioMemResp>() {
            return Err(MemError::DescriptorLengthTooSmall);
        }

        Ok(Request {
            req,
            status_addr: status_desc.addr(),
        })
    }
}

struct StateChangeRequest<'a> {
    id: &'a str,
    config: &'a VirtioMemConfig,
    mem_state: &'a mut Vec<bool>,
    addr: u64,
    size: u64,
    nb_blocks: u16,
    multi_region: bool,
    map_regions: MapRegions,
    host_fd: Option<RawFd>,
    plug: bool,
}

impl<'a> StateChangeRequest<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        r: &Request,
        id: &'a str,
        config: &'a VirtioMemConfig,
        mem_state: &'a mut Vec<bool>,
        multi_region: bool,
        map_regions: MapRegions,
        host_fd: Option<RawFd>,
        plug: bool,
    ) -> StateChangeRequest<'a> {
        let size: u64 = r.req.nb_blocks as u64 * config.block_size;

        StateChangeRequest {
            id,
            config,
            mem_state,
            addr: r.req.addr,
            size,
            nb_blocks: r.req.nb_blocks,
            multi_region,
            map_regions,
            host_fd,
            plug,
        }
    }
}

/// A hook for the VMM to create memory region for virtio-mem devices.
pub trait MemRegionFactory: Send {
    fn create_region(
        &mut self,
        guest_addr: GuestAddress,
        region_len: GuestUsize,
        kvm_slot: u32,
    ) -> std::result::Result<Arc<GuestRegionMmap>, Error>;

    fn restore_region_addr(&self, guest_addr: GuestAddress) -> std::result::Result<*mut u8, Error>;

    fn get_host_numa_node_id(&self) -> Option<u32>;

    fn set_host_numa_node_id(&mut self, host_numa_node_id: Option<u32>);
}

struct MemTool {}

impl MemTool {
    fn virtio_mem_valid_range(config: &VirtioMemConfig, addr: u64, size: u64) -> bool {
        // address properly aligned?
        if addr % config.block_size != 0 || size % config.block_size != 0 {
            return false;
        }

        // reasonable size
        if addr.checked_add(size).is_none() || size == 0 {
            return false;
        }

        // start address in usable range?
        if addr < config.addr || addr >= config.addr + config.usable_region_size {
            return false;
        }

        // end address in usable range?
        if addr + size > config.addr + config.usable_region_size {
            return false;
        }

        true
    }

    fn virtio_mem_check_bitmap(
        bit_index: usize,
        nb_blocks: u16,
        mem_state: &[bool],
        plug: bool,
    ) -> bool {
        for state in mem_state.iter().skip(bit_index).take(nb_blocks as usize) {
            if *state != plug {
                return false;
            }
        }
        true
    }

    fn virtio_mem_set_bitmap(bit_index: usize, nb_blocks: u16, mem_state: &mut [bool], plug: bool) {
        for state in mem_state
            .iter_mut()
            .skip(bit_index)
            .take(nb_blocks as usize)
        {
            *state = plug;
        }
    }

    fn virtio_mem_state_change_request(r: &mut StateChangeRequest) -> u16 {
        if r.plug && (r.config.plugged_size + r.size > r.config.requested_size) {
            return VIRTIO_MEM_RESP_NACK;
        }
        if !MemTool::virtio_mem_valid_range(r.config, r.addr, r.size) {
            return VIRTIO_MEM_RESP_ERROR;
        }

        let offset = r.addr - r.config.addr;
        let bit_index = (offset / r.config.block_size) as usize;
        if !MemTool::virtio_mem_check_bitmap(bit_index, r.nb_blocks, r.mem_state, !r.plug) {
            return VIRTIO_MEM_RESP_ERROR;
        }

        let host_addr = if r.multi_region {
            // Handle map_region
            let map_regions = r.map_regions.lock().unwrap();
            let map_region_index = (offset >> VIRTIO_MEM_MAP_REGION_SHIFT) as usize;
            if (offset + r.size - 1) >> VIRTIO_MEM_MAP_REGION_SHIFT != map_region_index as u64 {
                error!(
                    target: MEM_DRIVER_NAME,
                    "{}: {}: try to change more than one map_region", MEM_DRIVER_NAME, r.id,
                );
                return VIRTIO_MEM_RESP_ERROR;
            }
            if map_region_index >= map_regions.len() {
                error!(
                    target: MEM_DRIVER_NAME,
                    "{}: {}: map_region index {} is not right {:?}",
                    MEM_DRIVER_NAME,
                    r.id,
                    map_region_index,
                    map_regions,
                );
                return VIRTIO_MEM_RESP_ERROR;
            }

            let region_host_addr = if let Some(addr_tuple) = map_regions[map_region_index].1 {
                addr_tuple.0
            } else {
                error!(
                    "{}: try to access unmap region offset {} size {}",
                    MEM_DRIVER_NAME, offset, r.size
                );
                return VIRTIO_MEM_RESP_ERROR;
            };
            (offset & VIRTIO_MEM_MAP_REGION_MASK) + region_host_addr
        } else {
            let map_regions = r.map_regions.lock().unwrap();
            if let Some(addr_tuple) = map_regions[0].1 {
                addr_tuple.0 + offset
            } else {
                error!(
                    target: MEM_DRIVER_NAME,
                    "{}: {}: try to unplug unmap region", MEM_DRIVER_NAME, r.id
                );
                return VIRTIO_MEM_RESP_ERROR;
            }
        };

        if !r.plug {
            if let Some(fd) = r.host_fd {
                let res = unsafe {
                    libc::fallocate64(
                        fd,
                        libc::FALLOC_FL_PUNCH_HOLE | libc::FALLOC_FL_KEEP_SIZE,
                        offset as libc::off64_t,
                        r.size as libc::off64_t,
                    )
                };
                if res != 0 {
                    error!(
                        target: MEM_DRIVER_NAME,
                        "{}: {}: fallocate64 get error {}",
                        MEM_DRIVER_NAME,
                        r.id,
                        io::Error::last_os_error()
                    );
                    return VIRTIO_MEM_RESP_ERROR;
                }
            }
            let res = unsafe {
                libc::madvise(
                    host_addr as *mut libc::c_void,
                    r.size as libc::size_t,
                    libc::MADV_REMOVE,
                )
            };
            if res != 0 {
                error!(
                    target: MEM_DRIVER_NAME,
                    "{}: {}: madvise get error {}",
                    MEM_DRIVER_NAME,
                    r.id,
                    io::Error::last_os_error()
                );
                return VIRTIO_MEM_RESP_ERROR;
            }
            trace!(
                target: MEM_DRIVER_NAME,
                "{}: {}: unplug host_addr {} size {}",
                MEM_DRIVER_NAME,
                r.id,
                host_addr,
                r.size,
            );
        } else {
            trace!(
                target: MEM_DRIVER_NAME,
                "{}: {}: plug host_addr {} size {}",
                MEM_DRIVER_NAME,
                r.id,
                host_addr,
                r.size,
            );
        }

        MemTool::virtio_mem_set_bitmap(bit_index, r.nb_blocks, r.mem_state, r.plug);

        VIRTIO_MEM_RESP_ACK
    }

    #[allow(clippy::too_many_arguments)]
    fn virtio_mem_unplug_all(
        id: &str,
        config: &VirtioMemConfig,
        mem_state: &mut Vec<bool>,
        multi_region: bool,
        map_regions: MapRegions,
        host_fd: Option<RawFd>,
    ) -> u16 {
        for x in 0..(config.region_size / config.block_size) as usize {
            if mem_state[x] {
                let mut request = StateChangeRequest {
                    id,
                    config,
                    addr: config.addr + x as u64 * config.block_size,
                    size: config.block_size,
                    nb_blocks: 1,
                    mem_state,
                    multi_region,
                    map_regions: map_regions.clone(),
                    host_fd,
                    plug: false,
                };
                let resp_type = MemTool::virtio_mem_state_change_request(&mut request);
                if resp_type != VIRTIO_MEM_RESP_ACK {
                    return resp_type;
                }
                mem_state[x] = false;
            }
        }

        VIRTIO_MEM_RESP_ACK
    }

    fn virtio_mem_state_request(
        config: &VirtioMemConfig,
        addr: u64,
        nb_blocks: u16,
        mem_state: &mut [bool],
    ) -> (u16, u16) {
        let size: u64 = nb_blocks as u64 * config.block_size;
        let resp_type = if MemTool::virtio_mem_valid_range(config, addr, size) {
            VIRTIO_MEM_RESP_ACK
        } else {
            VIRTIO_MEM_RESP_ERROR
        };

        let offset = addr - config.addr;
        let bit_index = (offset / config.block_size) as usize;
        let resp_state = if MemTool::virtio_mem_check_bitmap(bit_index, nb_blocks, mem_state, true)
        {
            VIRTIO_MEM_STATE_PLUGGED
        } else if MemTool::virtio_mem_check_bitmap(bit_index, nb_blocks, mem_state, false) {
            VIRTIO_MEM_STATE_UNPLUGGED
        } else {
            VIRTIO_MEM_STATE_MIXED
        };

        (resp_type, resp_state)
    }

    /// The idea of virtio_mem_resize_usable_region is get from QEMU virtio_mem_resize_usable_region
    /// use alignment to calculate usable extent.
    fn virtio_mem_resize_usable_region(
        id: &str,
        config: &mut VirtioMemConfig,
        can_shrink: bool,
        alignment: u64,
        // map_regions, factory
        multi_regions: MultiRegions,
    ) -> Result<()> {
        let mut newsize = cmp::min(config.region_size, config.requested_size + 2 * alignment);

        /* The usable region size always has to be multiples of the block size. */
        newsize &= !(config.block_size - 1);

        if config.requested_size == 0 {
            newsize = 0;
        }

        if newsize > config.usable_region_size {
            if let Some((map_regions, factory)) = multi_regions {
                let mut map_regions = map_regions.lock().unwrap();
                let mut first_index =
                    (config.usable_region_size >> VIRTIO_MEM_MAP_REGION_SHIFT) as usize;
                let mut last_index = (newsize >> VIRTIO_MEM_MAP_REGION_SHIFT) as usize;
                if first_index >= map_regions.len() {
                    first_index = map_regions.len() - 1;
                }
                if last_index >= map_regions.len() {
                    last_index = map_regions.len() - 1;
                }
                // Find the first unmap index
                let mut first_unmap_index = None;
                for index in first_index..last_index + 1 {
                    if map_regions[index].1.is_none() {
                        first_unmap_index = Some(index);
                        break;
                    }
                }
                if let Some(first_index) = first_unmap_index {
                    let regions_num = (last_index - first_index + 1) as u64;
                    // Setup a new map region
                    let mut guest_addr =
                        config.addr + ((first_index as u64) << VIRTIO_MEM_MAP_REGION_SHIFT);
                    let region_len = ((regions_num - 1) << VIRTIO_MEM_MAP_REGION_SHIFT)
                        + if last_index + 1 == map_regions.len() {
                            config.region_size
                                - ((last_index as u64) << VIRTIO_MEM_MAP_REGION_SHIFT)
                        } else {
                            VIRTIO_MEM_MAP_REGION_SIZE
                        };
                    trace!(
                        target: MEM_DRIVER_NAME,
                        "{}: {}: try to get new map_region index {}-{} guest_addr 0x{:x} len 0x{:x} slot {}",
                        MEM_DRIVER_NAME,
                        id,
                        first_index,
                        last_index,
                        guest_addr,
                        region_len,
                        map_regions[first_index].0,
                    );
                    let region = factory.lock().unwrap().create_region(
                        GuestAddress(guest_addr),
                        region_len,
                        map_regions[first_index].0,
                    )?;
                    let mut host_addr = region
                        .get_host_address(MemoryRegionAddress(0))
                        .map_err(|e| MemError::RsizeUsabeRegionFail(format!("{:?}", e)))?
                        as u64;
                    info!(target: MEM_DRIVER_NAME,
                          "{}: {}: new map_region index {}-{} new region guest_addr 0x{:x}-0x{:x} host_addr 0x{:x} len 0x{:x}",
                          MEM_DRIVER_NAME, id, first_index, last_index, guest_addr, guest_addr + region_len, host_addr, region_len);
                    for index in first_index..last_index + 1 {
                        map_regions[index].1 = Some((host_addr, guest_addr));
                        host_addr += VIRTIO_MEM_MAP_REGION_SIZE;
                        guest_addr += VIRTIO_MEM_MAP_REGION_SIZE;
                    }
                }
            }
        }
        if newsize < config.usable_region_size && !can_shrink {
            return Ok(());
        }

        let oldsize = config.usable_region_size;
        info!(
            target: MEM_DRIVER_NAME,
            "{}: {}: virtio_mem_resize_usable_region {:?} {:?}",
            MEM_DRIVER_NAME,
            id,
            oldsize,
            newsize
        );
        config.usable_region_size = newsize;

        Ok(())
    }
}

pub(crate) struct MemEpollHandler<
    AS: GuestAddressSpace,
    Q: QueueT + Send = QueueSync,
    R: GuestMemoryRegion = GuestRegionMmap,
> {
    pub(crate) config: VirtioDeviceConfig<AS, Q, R>,
    mem_config: Arc<Mutex<VirtioMemConfig>>,
    pub(crate) multi_region: bool,
    // kvm_slot, Option(host_addr, guest_addr)
    pub(crate) map_regions: MapRegions,
    host_fd: Option<RawFd>,
    pub(crate) mem_state: Vec<bool>,
    id: String,
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion> MemEpollHandler<AS, Q, R> {
    fn process_queue(&mut self, queue_index: usize) -> bool {
        // Do not expect poisoned lock.
        let config = &mut self.mem_config.lock().unwrap();
        let conf = &mut self.config;
        let guard = conf.lock_guest_memory();
        let mem = guard.deref();
        let queue = &mut conf.queues[queue_index];
        let mut guard = queue.queue_mut().lock();
        let mut used_desc_heads = Vec::with_capacity(QUEUE_SIZE as usize);

        let mut iter = match guard.iter(mem) {
            Err(e) => {
                error!(
                    "{}: {}: failed to process queue. {}",
                    MEM_DRIVER_NAME, self.id, e
                );
                return false;
            }
            Ok(iter) => iter,
        };

        for mut avail_desc in &mut iter {
            let len = match Request::parse(&mut avail_desc, mem) {
                Err(e) => {
                    debug!(
                        target: MEM_DRIVER_NAME,
                        "{}: {}: failed parse VirtioMemReq, {:?}", MEM_DRIVER_NAME, self.id, e
                    );
                    0
                }
                Ok(r) => match r.req.req_type {
                    VIRTIO_MEM_REQ_PLUG => {
                        let mut request = StateChangeRequest::new(
                            &r,
                            &self.id,
                            config,
                            &mut self.mem_state,
                            self.multi_region,
                            self.map_regions.clone(),
                            self.host_fd,
                            true,
                        );
                        let resp_type = MemTool::virtio_mem_state_change_request(&mut request);
                        let size = request.size;
                        drop(request);
                        if resp_type == VIRTIO_MEM_RESP_ACK {
                            config.plugged_size += size;
                            let new_plugged_size = config.plugged_size;
                            trace!(
                                target: MEM_DRIVER_NAME,
                                "{}: {}: process_queue VIRTIO_MEM_REQ_PLUG {:?} plugged_size {:?}",
                                MEM_DRIVER_NAME,
                                self.id,
                                size,
                                new_plugged_size
                            );
                        }
                        Self::send_response(&self.id, mem, r.status_addr, resp_type, 0)
                    }
                    VIRTIO_MEM_REQ_UNPLUG => {
                        let mut request = StateChangeRequest::new(
                            &r,
                            &self.id,
                            config,
                            &mut self.mem_state,
                            self.multi_region,
                            self.map_regions.clone(),
                            self.host_fd,
                            false,
                        );
                        let resp_type = MemTool::virtio_mem_state_change_request(&mut request);
                        let size = request.size;
                        drop(request);
                        if resp_type == VIRTIO_MEM_RESP_ACK {
                            config.plugged_size -= size;
                            let new_plugged_size = config.plugged_size;
                            trace!(
                                target: MEM_DRIVER_NAME,
                                "{}: {}: process_queue VIRTIO_MEM_REQ_UNPLUG {:?} plugged_size {:?}",
                                MEM_DRIVER_NAME, self.id, size, new_plugged_size
                            );
                        }
                        Self::send_response(&self.id, mem, r.status_addr, resp_type, 0)
                    }
                    VIRTIO_MEM_REQ_UNPLUG_ALL => {
                        let resp_type = MemTool::virtio_mem_unplug_all(
                            &self.id,
                            config,
                            &mut self.mem_state,
                            self.multi_region,
                            self.map_regions.clone(),
                            self.host_fd,
                        );
                        if resp_type == VIRTIO_MEM_RESP_ACK {
                            config.plugged_size = 0;
                            /* Does not call MemTool::virtio_mem_resize_usable_region because current doesn't support unmap region. */
                            trace!(
                                target: MEM_DRIVER_NAME,
                                "{}: {}: process_queue VIRTIO_MEM_REQ_UNPLUG_ALL",
                                MEM_DRIVER_NAME,
                                self.id,
                            );
                        }
                        Self::send_response(&self.id, mem, r.status_addr, resp_type, 0)
                    }
                    VIRTIO_MEM_REQ_STATE => {
                        let (resp_type, resp_state) = MemTool::virtio_mem_state_request(
                            config,
                            r.req.addr,
                            r.req.nb_blocks,
                            &mut self.mem_state,
                        );
                        Self::send_response(&self.id, mem, r.status_addr, resp_type, resp_state)
                    }
                    _ => {
                        debug!(
                            target: MEM_DRIVER_NAME,
                            "{}: {}: VirtioMemReq unknown request type {:?}",
                            MEM_DRIVER_NAME,
                            self.id,
                            r.req.req_type
                        );
                        0
                    }
                },
            };

            used_desc_heads.push((avail_desc.head_index(), len));
        }

        drop(guard);

        for &(desc_index, len) in &used_desc_heads {
            queue.add_used(mem, desc_index, len);
        }

        !used_desc_heads.is_empty()
    }

    fn send_response(
        id: &str,
        mem: &AS::M,
        status_addr: GuestAddress,
        resp_type: u16,
        state: u16,
    ) -> u32 {
        let mut resp = VirtioMemResp {
            resp_type,
            ..VirtioMemResp::default()
        };
        resp.state.state = state;
        match mem.write_obj(resp, status_addr) {
            Ok(_) => size_of::<VirtioMemResp>() as u32,
            Err(e) => {
                debug!(
                    target: MEM_DRIVER_NAME,
                    "{}: {}: bad guest memory address, {}", MEM_DRIVER_NAME, id, e
                );
                0
            }
        }
    }
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion> MutEventSubscriber
    for MemEpollHandler<AS, Q, R>
{
    fn process(&mut self, events: Events, _ops: &mut EventOps) {
        trace!(
            target: MEM_DRIVER_NAME,
            "{}: {}: MemEpollHandler::process()",
            MEM_DRIVER_NAME,
            self.id
        );

        let idx = events.data() as usize;
        if idx >= self.config.queues.len() {
            error!(
                target: MEM_DRIVER_NAME,
                "{}: {}: invalid queue index {}", MEM_DRIVER_NAME, self.id, idx
            );
            return;
        }

        if let Err(e) = self.config.queues[idx].consume_event() {
            error!(
                target: MEM_DRIVER_NAME,
                "{}: {}: failed to get queue event, {:?}", MEM_DRIVER_NAME, self.id, e
            );
        } else if self.process_queue(idx) {
            if let Err(e) = self.config.queues[idx].notify() {
                error!(
                    target: MEM_DRIVER_NAME,
                    "{}: {}: failed to signal used queue, {}", MEM_DRIVER_NAME, self.id, e
                );
            }
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        trace!(
            target: MEM_DRIVER_NAME,
            "{}: {}: MemEpollHandler::init()",
            MEM_DRIVER_NAME,
            self.id
        );

        for (idx, queue) in self.config.queues.iter().enumerate() {
            ops.add(Events::with_data(
                queue.eventfd.as_ref(),
                idx as u32,
                EventSet::IN,
            ))
            .unwrap_or_else(|_| {
                panic!(
                    "{}: {}: failed to register queue event handler",
                    MEM_DRIVER_NAME, self.id
                )
            });
        }
    }
}

fn get_map_regions_num(region_size: u64) -> usize {
    ((region_size >> VIRTIO_MEM_MAP_REGION_SHIFT)
        + u64::from(region_size & VIRTIO_MEM_MAP_REGION_MASK > 0)) as usize
}

/// Virtio device for exposing memory hotplug to the guest OS through virtio.
pub struct Mem<AS: GuestAddressSpace> {
    pub(crate) device_info: VirtioDeviceInfo,
    config: Arc<Mutex<VirtioMemConfig>>,
    capacity: u64,
    factory: Arc<Mutex<dyn MemRegionFactory>>,
    host_fd: Option<RawFd>,
    device_change_notifier: Arc<dyn InterruptNotifier>,
    subscriber_id: Option<SubscriberId>,
    id: String,
    phantom: PhantomData<AS>,
    alignment: u64,
    // used for liveupgrade to record the memory state map in epoll handler
    mem_state_map: Option<Vec<bool>>,
    multi_region: bool,
    // kvm_slot, Option(host_addr, guest_addr)
    map_regions: MapRegions,
}

impl<AS: GuestAddressSpace> Mem<AS> {
    /// Create a new virtio-mem device.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        mut capacity: u64,
        requested_size_mib: u64,
        mut multi_region: bool,
        numa_node_id: Option<u16>,
        epoll_mgr: EpollManager,
        factory: Arc<Mutex<dyn MemRegionFactory>>,
        boot_mem_byte: u64,
    ) -> Result<Self> {
        trace!(
            target: MEM_DRIVER_NAME,
            "{}: {}: Mem::new()",
            MEM_DRIVER_NAME,
            id
        );

        let mut avail_features = 1u64 << VIRTIO_F_VERSION_1 as u64;

        // calculate alignment depending on boot memory size
        // algorithm is from kernel (arch/x86/mm/init_64.c: probe_memory_block_size())
        let alignment = {
            if boot_mem_byte < BOOT_MEM_SIZE_FOR_LARGE_BLOCK {
                VIRTIO_MEM_DEFAULT_BLOCK_ALIGNMENT
            } else {
                let mut bz = MAX_MEMORY_BLOCK_SIZE;
                while bz > VIRTIO_MEM_DEFAULT_BLOCK_ALIGNMENT {
                    if boot_mem_byte & (bz - 1) == 0 {
                        break;
                    }
                    bz >>= 1
                }
                bz
            }
        };

        // Align to 2 * alignment (256MB when boot mem size < 64G).
        capacity = capacity * 1024 * 1024;
        let usable_extent = 2 * alignment;
        capacity = (capacity + usable_extent - 1) & !(usable_extent - 1);
        let requested_size = requested_size_mib * 1024 * 1024;
        if capacity == 0
            || requested_size > capacity
            || requested_size % VIRTIO_MEM_DEFAULT_BLOCK_SIZE != 0
        {
            return Err(Error::InvalidInput);
        }

        let mut config = VirtioMemConfig::default();
        if let Some(node_id) = numa_node_id {
            avail_features |= 1u64 << VIRTIO_MEM_F_ACPI_PXM;
            config.node_id = node_id;
        }
        config.block_size = VIRTIO_MEM_DEFAULT_BLOCK_SIZE;
        config.region_size = capacity;
        config.requested_size = requested_size;
        //config.usable_region_size will be setup in set_resource through virtio_mem_resize_usable_region

        if config.region_size <= VIRTIO_MEM_MAP_REGION_SIZE {
            multi_region = false;
        }

        // For warning unaligned_references
        // adding curly braces means that a copy of the field is made, stored
        // in a (properly aligned) temporary, and a reference to that temporary
        // is being formatted.
        info!(target: MEM_DRIVER_NAME, "{}: {}: new block_size: 0x{:x} region_size: 0x{:x} requested_size: 0x{:x} usable_region_size: 0x{:x} multi_region: {} numa_node_id: {:?}", 
            MEM_DRIVER_NAME, id, {config.block_size}, {config.region_size}, {config.requested_size}, {config.usable_region_size}, multi_region, numa_node_id);

        let device_info = VirtioDeviceInfo::new(
            MEM_DRIVER_NAME.to_string(),
            avail_features,
            Arc::new(vec![QUEUE_SIZE; NUM_QUEUES]),
            config.as_slice().to_vec(),
            epoll_mgr,
        );

        Ok(Mem {
            device_info,
            config: Arc::new(Mutex::new(config)),
            capacity,
            factory,
            device_change_notifier: Arc::new(NoopNotifier::new()),
            host_fd: None,
            subscriber_id: None,
            id,
            phantom: PhantomData,
            alignment,
            mem_state_map: None,
            multi_region,
            map_regions: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Set requested size of the memory device.
    pub fn set_requested_size(&self, requested_size_mb: u64) -> Result<()> {
        // Align to 4MB.
        let requested_size = requested_size_mb * 1024 * 1024;
        if requested_size > self.capacity || requested_size % VIRTIO_MEM_DEFAULT_BLOCK_SIZE != 0 {
            return Err(Error::InvalidInput);
        }

        let mem_config = &mut self.config.lock().unwrap();
        /*
         * QEMU set config.requested_size after call
         * virtio_mem_resize_usable_region.
         * But virtio_mem_resize_usable_region of QEMU use new size as
         * the requested_size.
         * So this part should set requested_size before call
         * MemTool::virtio_mem_resize_usable_region.
         * Then MemTool::virtio_mem_resize_usable_region will get the new size
         * from mem_config.requested_size.
         */
        info!(
            target: MEM_DRIVER_NAME,
            "{}: {}: set_requested_size {} Mib", MEM_DRIVER_NAME, self.id, requested_size_mb
        );
        mem_config.requested_size = requested_size;
        MemTool::virtio_mem_resize_usable_region(
            &self.id,
            mem_config,
            false,
            self.alignment,
            if self.multi_region {
                Some((self.map_regions.clone(), self.factory.clone()))
            } else {
                None
            },
        )?;
        if let Err(e) = self.device_change_notifier.notify() {
            error!(
                target: MEM_DRIVER_NAME,
                "{}: {}: failed to signal device change event: {}", MEM_DRIVER_NAME, self.id, e
            );
            return Err(Error::IOError(e));
        }

        Ok(())
    }
}

impl<AS, Q, R> VirtioDevice<AS, Q, R> for Mem<AS>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    fn device_type(&self) -> u32 {
        TYPE_MEM
    }

    fn queue_max_sizes(&self) -> &[u16] {
        QUEUE_SIZES
    }

    fn get_avail_features(&self, page: u32) -> u32 {
        self.device_info.get_avail_features(page)
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        trace!(
            target: MEM_DRIVER_NAME,
            "{}: {}: VirtioDevice::set_acked_features({}, 0x{:x})",
            MEM_DRIVER_NAME,
            self.id,
            page,
            value
        );

        self.device_info.set_acked_features(page, value)
    }

    fn read_config(&mut self, offset: u64, mut data: &mut [u8]) -> ConfigResult {
        trace!(
            target: MEM_DRIVER_NAME,
            "{}: {}: VirtioDevice::read_config(0x{:x}, {:?})",
            MEM_DRIVER_NAME,
            self.id,
            offset,
            data
        );

        // Do not expect poisoned lock.
        let mem_config = self.config.lock().unwrap();
        let config_space = mem_config.as_slice().to_vec();
        let config_len = config_space.len() as u64;

        if offset >= config_len {
            debug!(
                target: MEM_DRIVER_NAME,
                "{}: {}: config space read request out of range, offset {}",
                MEM_DRIVER_NAME,
                self.id,
                offset
            );
        } else if let Some(end) = offset.checked_add(data.len() as u64) {
            let end = cmp::min(end, config_len) as usize;
            // This write can't fail, offset and end are checked against config_len.
            let _ = data.write(&config_space[offset as usize..end]).unwrap();
        }
        Ok(())
    }

    fn write_config(&mut self, _offset: u64, _data: &[u8]) -> ConfigResult {
        debug!(
            target: MEM_DRIVER_NAME,
            "{}: {}: device configuration is read-only", MEM_DRIVER_NAME, self.id
        );
        Ok(())
    }

    fn activate(&mut self, config: VirtioDeviceConfig<AS, Q, R>) -> ActivateResult {
        trace!(
            target: MEM_DRIVER_NAME,
            "{}: {}: VirtioDevice::activate()",
            MEM_DRIVER_NAME,
            self.id
        );

        // Do not support control queue and multi queue.
        if config.queues.len() != 1 {
            error!(
                target: MEM_DRIVER_NAME,
                "{}: {}: failed to activate, invalid queue_num {}.",
                MEM_DRIVER_NAME,
                self.id,
                config.queues.len()
            );
            return Err(ActivateError::InvalidParam);
        }
        self.device_info.check_queue_sizes(&config.queues)?;

        self.device_change_notifier = config.device_change_notifier.clone();

        // Do not expect poisoned lock
        let mem_config = self.config.lock().unwrap();

        let slot_num = if self.multi_region {
            get_map_regions_num(mem_config.region_size)
        } else {
            1
        };

        let map_regions_len = self.map_regions.lock().unwrap().len();
        if map_regions_len != slot_num {
            error!(
                target: MEM_DRIVER_NAME,
                "{}: {}: map_region.len {}, slot_num {}",
                MEM_DRIVER_NAME,
                self.id,
                map_regions_len,
                slot_num
            );
            return Err(ActivateError::InternalError);
        }

        let mem_state = self.mem_state_map.take().unwrap_or_else(|| {
            vec![false; mem_config.region_size as usize / mem_config.block_size as usize]
        });

        let handler = Box::new(MemEpollHandler {
            config,
            mem_config: self.config.clone(),
            multi_region: self.multi_region,
            map_regions: self.map_regions.clone(),
            host_fd: self.host_fd,
            mem_state,
            id: self.id.clone(),
        });

        self.subscriber_id = Some(self.device_info.register_event_handler(handler));

        Ok(())
    }

    fn remove(&mut self) {
        if let Some(subscriber_id) = self.subscriber_id {
            // Remove MemEpollHandler from event manager, so it could be dropped and the resources
            // could be freed.
            match self.device_info.remove_event_handler(subscriber_id) {
                Ok(_) => debug!("virtio-mem: removed subscriber_id {:?}", subscriber_id),
                Err(e) => {
                    warn!("virtio-mem: failed to remove event handler: {:?}", e);
                }
            }
        }
        self.subscriber_id = None;
    }

    fn get_resource_requirements(
        &self,
        requests: &mut Vec<ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        trace!(
            target: MEM_DRIVER_NAME,
            "{}: {}: VirtioDevice::get_resource_requirements()",
            MEM_DRIVER_NAME,
            self.id
        );

        requests.push(ResourceConstraint::LegacyIrq { irq: None });
        if use_generic_irq {
            // Allocate one irq for device configuration change events, and one irq for each queue.
            requests.push(ResourceConstraint::GenericIrq {
                size: (self.device_info.queue_sizes.len() + 1) as u32,
            });
        }

        // Do not expect poisoned lock.
        let config = self.config.lock().unwrap();

        // The memory needs to be 2MiB aligned in order to support huge pages.
        // And we also need to align the memory's start address to guest's
        // memory block size (usually 128MB), or the virtio-mem driver in guest
        // kernel would cause some memory unusable which outside the alignment.
        // Then, the memory needs to be above 4G to avoid conflicts with
        // lapic/ioapic devices.
        requests.push(ResourceConstraint::MemAddress {
            range: None,
            align: self.alignment,
            size: config.region_size,
        });

        // Request for new kvm memory slot.
        let slot_num = if self.multi_region {
            get_map_regions_num(config.region_size)
        } else {
            1
        };
        for _ in 0..slot_num {
            requests.push(ResourceConstraint::KvmMemSlot {
                slot: None,
                size: 1,
            });
        }
    }

    fn set_resource(
        &mut self,
        _vm_fd: Arc<VmFd>,
        resource: DeviceResources,
    ) -> Result<Option<VirtioSharedMemoryList<R>>> {
        trace!(
            target: MEM_DRIVER_NAME,
            "{}: {}: VirtioDevice::set_resource()",
            MEM_DRIVER_NAME,
            self.id
        );

        let mem_res = resource.get_mem_address_ranges();
        let slot_res = resource.get_kvm_mem_slots();

        // Check if we get memory resource.
        if mem_res.is_empty() {
            return Err(Error::InvalidResource);
        }

        let mut mem_config = self.config.lock().unwrap();

        let slot_num = if self.multi_region {
            get_map_regions_num(mem_config.region_size)
        } else {
            1
        };

        // Make sure we have the correct resource as requested.
        if slot_res.len() != slot_num
            || mem_res.len() != 1
            || mem_res[0].1 != mem_config.region_size
        {
            error!(
                target: MEM_DRIVER_NAME,
                "{}: {}: wrong mem or kvm slot resource ({:?}, {:?})",
                MEM_DRIVER_NAME,
                self.id,
                mem_res.len(),
                slot_res.len()
            );
            return Err(Error::InvalidResource);
        }

        // update mem config's addr
        mem_config.addr = mem_res[0].0;

        // Setup map_regions
        let mut map_regions = self.map_regions.lock().unwrap();
        if map_regions.is_empty() {
            if self.multi_region {
                for slot in slot_res {
                    map_regions.push((slot, None));
                }
            } else {
                let region = self.factory.lock().unwrap().create_region(
                    GuestAddress(mem_config.addr),
                    mem_config.region_size,
                    slot_res[0],
                )?;
                let addr = region.get_host_address(MemoryRegionAddress(0)).unwrap() as u64;
                map_regions.push((slot_res[0], Some((addr, mem_config.addr))));
                let guest_addr = mem_config.addr;
                let size = mem_config.region_size;
                info!(
                    "{}: {}: set_resource new region guest addr 0x{:x}-0x{:x} host addr 0x{:x} size {}",
                    MEM_DRIVER_NAME,
                    self.id,
                    guest_addr,
                    guest_addr + size,
                    addr,
                    size,
                );
            }
        }
        drop(map_regions);

        MemTool::virtio_mem_resize_usable_region(
            &self.id,
            &mut mem_config,
            false,
            self.alignment,
            if self.multi_region {
                Some((self.map_regions.clone(), self.factory.clone()))
            } else {
                None
            },
        )?;

        Ok(None)
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
    use std::ffi::CString;
    use std::fs::File;
    use std::os::unix::io::FromRawFd;

    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::NoopNotifier;
    use dbs_utils::epoll_manager::SubscriberOps;
    use kvm_ioctls::Kvm;
    use nix::sys::memfd;
    use virtio_queue::QueueSync;
    use vm_memory::{
        FileOffset, GuestAddress, GuestMemoryMmap, GuestRegionMmap, GuestUsize, MmapRegion,
    };
    use vmm_sys_util::eventfd::EventFd;

    use super::*;
    use crate::tests::{create_address_space, VirtQueue, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};
    use crate::VirtioQueueConfig;

    struct DummyMemRegionFactory {}

    impl MemRegionFactory for DummyMemRegionFactory {
        fn create_region(
            &mut self,
            guest_addr: GuestAddress,
            region_len: GuestUsize,
            _kvm_slot: u32,
        ) -> std::result::Result<Arc<GuestRegionMmap>, Error> {
            let file_offset = {
                let fd = memfd::memfd_create(
                    // safe to unwrap, no nul byte in file name
                    &CString::new("virtio_fs_mem").unwrap(),
                    memfd::MemFdCreateFlag::empty(),
                )
                .map_err(|_| Error::InvalidInput)?;
                let file: File = unsafe { File::from_raw_fd(fd) };
                file.set_len(region_len).map_err(|_| Error::InvalidInput)?;
                Some(FileOffset::new(file, 0))
            };

            // unmap will be handled on MmapRegion'd Drop.
            let mmap_region = MmapRegion::build(
                file_offset,
                region_len as usize,
                libc::PROT_NONE,
                libc::MAP_ANONYMOUS | libc::MAP_NORESERVE | libc::MAP_PRIVATE,
            )
            .map_err(Error::NewMmapRegion)?;

            let region =
                Arc::new(GuestRegionMmap::new(mmap_region, guest_addr).map_err(Error::InsertMmap)?);

            Ok(region)
        }

        fn restore_region_addr(
            &self,
            _guest_addr: GuestAddress,
        ) -> std::result::Result<*mut u8, Error> {
            Err(Error::InvalidInput)
        }

        fn get_host_numa_node_id(&self) -> Option<u32> {
            None
        }

        fn set_host_numa_node_id(&mut self, _host_numa_node_id: Option<u32>) {}
    }

    fn create_mem_epoll_handler(id: String) -> MemEpollHandler<Arc<GuestMemoryMmap>> {
        let mem = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0x0), 0x10000)]).unwrap());
        let queues = vec![VirtioQueueConfig::create(256, 0).unwrap()];
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let resources = DeviceResources::new();
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
        let mem_config = Arc::new(Mutex::new(VirtioMemConfig::default()));
        let map_regions = vec![(0, Some((0, 0)))];
        MemEpollHandler {
            config,
            mem_config,
            multi_region: false,
            map_regions: Arc::new(Mutex::new(map_regions)),
            host_fd: None,
            mem_state: Vec::new(),
            id,
        }
    }

    #[test]
    fn test_mem_request_parse() {
        let m = &GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let vq = VirtQueue::new(GuestAddress(0), m, 16);

        assert!(vq.end().0 < 0x1000);

        vq.avail.ring(0).store(0);
        vq.avail.idx().store(1);
        // write only request type descriptor
        {
            let mut queue = vq.create_queue();
            let mut q = queue.lock();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_WRITE, 1);
            m.write_obj::<u64>(114, GuestAddress(0x1000 + 8)).unwrap();
            assert!(matches!(
                Request::parse(&mut q.iter(m).unwrap().next().unwrap(), m),
                Err(MemError::UnexpectedWriteOnlyDescriptor)
            ));
        }
        // desc len error
        {
            let mut queue = vq.create_queue();
            let mut q = queue.lock();
            vq.dtable(0).flags().store(0);
            m.write_obj::<u64>(114, GuestAddress(0x1000 + 8)).unwrap();
            assert!(matches!(
                Request::parse(&mut q.iter(m).unwrap().next().unwrap(), m),
                Err(MemError::InvalidRequest)
            ));
        }
        // desc chain too short
        {
            let mut queue = vq.create_queue();
            let mut q = queue.lock();
            vq.dtable(0).flags().store(0);
            vq.dtable(0).set(0x1000, 0x18, 0, 1);
            assert!(matches!(
                Request::parse(&mut q.iter(m).unwrap().next().unwrap(), m),
                Err(MemError::DescriptorChainTooShort)
            ));
        }
        // unexpected read only descriptor
        {
            let mut queue = vq.create_queue();
            let mut q = queue.lock();
            vq.dtable(0).set(0x1000, 0x18, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1).set(0x2000, 0x18, VIRTQ_DESC_F_NEXT, 2);
            assert!(matches!(
                Request::parse(&mut q.iter(m).unwrap().next().unwrap(), m),
                Err(MemError::UnexpectedReadOnlyDescriptor)
            ));
        }
        // desc len too short
        {
            let mut queue = vq.create_queue();
            let mut q = queue.lock();
            vq.dtable(0).set(0x1000, 0x18, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1).set(0x2000, 0x9, VIRTQ_DESC_F_WRITE, 2);
            assert!(matches!(
                Request::parse(&mut q.iter(m).unwrap().next().unwrap(), m),
                Err(MemError::DescriptorLengthTooSmall)
            ));
        }
        // success
        {
            let mut queue = vq.create_queue();
            let mut q = queue.lock();
            vq.dtable(0).set(0x1000, 0x18, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1).set(0x2000, 0x18, VIRTQ_DESC_F_WRITE, 2);
            assert!(Request::parse(&mut q.iter(m).unwrap().next().unwrap(), m).is_ok());
        }
    }

    #[test]
    fn test_mem_tool_valid_range() {
        let config = VirtioMemConfig {
            block_size: 0x100,
            addr: 0x1000,
            usable_region_size: 0x1000,
            ..Default::default()
        };

        // address not properly aligned.
        assert!(!MemTool::virtio_mem_valid_range(&config, 0x14, 0x100));
        assert!(!MemTool::virtio_mem_valid_range(&config, 0x100, 5));

        // unreasonable size.
        assert!(!MemTool::virtio_mem_valid_range(
            &config,
            0x1000,
            i32::MAX as u64
        ));
        assert!(!MemTool::virtio_mem_valid_range(&config, 0x1000, 0));

        // start address not in usable range.
        assert!(!MemTool::virtio_mem_valid_range(&config, 0x200, 0x200));
        assert!(!MemTool::virtio_mem_valid_range(&config, 0x3000, 0x200),);

        // end address not in usable range.
        assert!(!MemTool::virtio_mem_valid_range(&config, 0x1000, 0x2000),);

        // success
        assert!(MemTool::virtio_mem_valid_range(&config, 0x1000, 0x500),);
    }

    #[test]
    fn test_mem_tool_check_bitmap() {
        let bit_index = 2;
        let nb_blocks = 2;
        let mut mem_state = [false, false, false, false];
        let plug = false;

        // true
        assert!(MemTool::virtio_mem_check_bitmap(
            bit_index, nb_blocks, &mem_state, plug
        ),);

        mem_state[2] = true;
        // false
        assert!(!MemTool::virtio_mem_check_bitmap(
            bit_index, nb_blocks, &mem_state, plug
        ),);
    }

    #[test]
    fn test_mem_tool_set_bitmap() {
        let bit_index = 2;
        let nb_blocks = 2;
        let mut mem_state = vec![false, false, false, false];
        let plug = true;

        MemTool::virtio_mem_set_bitmap(bit_index, nb_blocks, &mut mem_state, plug);
        assert!(mem_state[2]);
        assert!(mem_state[3]);
    }

    #[test]
    fn test_mem_tool_state_request() {
        let config = VirtioMemConfig {
            block_size: 0x100,
            addr: 0x1000,
            usable_region_size: 0x1000,
            ..Default::default()
        };
        let mut mem_state = vec![false, false, false, false];

        // invalid range.
        let (resp_type, resp_state) =
            MemTool::virtio_mem_state_request(&config, 0x2000, 0, &mut mem_state);
        assert_eq!(resp_type, VIRTIO_MEM_RESP_ERROR);
        assert_eq!(resp_state, VIRTIO_MEM_STATE_PLUGGED);

        // valid range & unplugged.
        let (resp_type, resp_state) =
            MemTool::virtio_mem_state_request(&config, 0x1200, 2, &mut mem_state);
        assert_eq!(resp_type, VIRTIO_MEM_RESP_ACK);
        assert_eq!(resp_state, VIRTIO_MEM_STATE_UNPLUGGED);

        // mixed mem state.
        mem_state = vec![false, false, true, false];
        let (resp_type, resp_state) =
            MemTool::virtio_mem_state_request(&config, 0x1200, 2, &mut mem_state);
        assert_eq!(resp_type, VIRTIO_MEM_RESP_ACK);
        assert_eq!(resp_state, VIRTIO_MEM_STATE_MIXED);

        // plugged.
        mem_state = vec![true, true, true, true];
        let (resp_type, resp_state) =
            MemTool::virtio_mem_state_request(&config, 0x1200, 2, &mut mem_state);
        assert_eq!(resp_type, VIRTIO_MEM_RESP_ACK);
        assert_eq!(resp_state, VIRTIO_MEM_STATE_PLUGGED);
    }

    #[test]
    fn test_mem_tool_resize_usable_region() {
        use std::ptr::{addr_of, read_unaligned};

        let mut config = VirtioMemConfig {
            region_size: 0x200,
            block_size: 0x100,
            usable_region_size: 0x1000,
            requested_size: 0,
            ..Default::default()
        };

        let id = "mem0".to_string();

        // unshrink.
        MemTool::virtio_mem_resize_usable_region(
            &id,
            &mut config,
            false,
            VIRTIO_MEM_DEFAULT_BLOCK_ALIGNMENT,
            None,
        )
        .unwrap();
        assert_eq!(
            unsafe { read_unaligned(addr_of!(config.usable_region_size)) },
            0x1000
        );

        // request size is 0.
        MemTool::virtio_mem_resize_usable_region(
            &id,
            &mut config,
            true,
            VIRTIO_MEM_DEFAULT_BLOCK_ALIGNMENT,
            None,
        )
        .unwrap();
        assert_eq!(
            unsafe { read_unaligned(addr_of!(config.usable_region_size)) },
            0
        );

        // shrink.
        config.requested_size = 0x5;
        MemTool::virtio_mem_resize_usable_region(
            &id,
            &mut config,
            true,
            VIRTIO_MEM_DEFAULT_BLOCK_ALIGNMENT,
            None,
        )
        .unwrap();
        assert_eq!(
            unsafe { read_unaligned(addr_of!(config.usable_region_size)) },
            0x200
        );

        // test alignment
        config.region_size = 2 << 30;
        config.requested_size = 1 << 30;
        // alignment unchanged.
        MemTool::virtio_mem_resize_usable_region(
            &id,
            &mut config,
            true,
            VIRTIO_MEM_DEFAULT_BLOCK_ALIGNMENT,
            None,
        )
        .unwrap();
        assert_eq!(
            unsafe { read_unaligned(addr_of!(config.usable_region_size)) },
            (1 << 30) + 2 * VIRTIO_MEM_DEFAULT_BLOCK_ALIGNMENT
        );
        // alignemnt changed.
        MemTool::virtio_mem_resize_usable_region(
            &id,
            &mut config,
            true,
            MAX_MEMORY_BLOCK_SIZE,
            None,
        )
        .unwrap();
        assert_eq!(
            unsafe { read_unaligned(addr_of!(config.usable_region_size)) },
            2 << 30
        );
    }

    #[test]
    fn test_mem_virtio_device_normal() {
        let epoll_mgr = EpollManager::default();
        let id = "mem0".to_string();
        let factory = Arc::new(Mutex::new(DummyMemRegionFactory {}));
        let mut dev =
            Mem::<Arc<GuestMemoryMmap>>::new(id, 200, 200, false, None, epoll_mgr, factory, 200)
                .unwrap();

        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::device_type(&dev),
            TYPE_MEM
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
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::set_acked_features(
            &mut dev, 2, 0,
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 2),
            0,
        );

        let mut data: [u8; 8] = [1; 8];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
            &mut dev, 0, &mut data,
        )
        .unwrap();
        let config: [u8; 8] = [0; 8];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::write_config(
            &mut dev, 0, &config,
        )
        .unwrap();
        let mut data2: [u8; 8] = [1; 8];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
            &mut dev, 0, &mut data2,
        )
        .unwrap();
        assert_eq!(data, data2);
    }

    #[test]
    fn test_mem_virtio_device_get_resource_requirements() {
        let epoll_mgr = EpollManager::default();
        let id = "mem0".to_string();
        let factory = Arc::new(Mutex::new(DummyMemRegionFactory {}));
        let dev = Mem::<Arc<GuestMemoryMmap>>::new(
            id, 0x100, 0x100, false, None, epoll_mgr, factory, 0xc0000000,
        )
        .unwrap();
        let mut requirements = vec![
            ResourceConstraint::new_mmio(0x1000),
            ResourceConstraint::new_mmio(0x1000),
        ];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_resource_requirements(
            &dev, &mut requirements, true,
        );
        assert_eq!(requirements[2], ResourceConstraint::LegacyIrq { irq: None });
        assert_eq!(requirements[3], ResourceConstraint::GenericIrq { size: 2 });
        assert_eq!(
            requirements[4],
            ResourceConstraint::MemAddress {
                range: None,
                align: VIRTIO_MEM_DEFAULT_BLOCK_ALIGNMENT,
                size: 0x100 << 20,
            }
        );
        assert_eq!(
            requirements[5],
            ResourceConstraint::KvmMemSlot {
                slot: None,
                size: 1
            }
        );
    }

    #[test]
    fn test_mem_virtio_device_set_resource() {
        let epoll_mgr = EpollManager::default();
        let id = "mem0".to_string();
        let factory = Arc::new(Mutex::new(DummyMemRegionFactory {}));

        // enable multi-region in virtio-mem
        {
            let mut dev = Mem::<Arc<GuestMemoryMmap>>::new(
                id.clone(),
                0xc00,
                0xc00,
                true,
                None,
                epoll_mgr.clone(),
                factory.clone(),
                0xc0000000,
            )
            .unwrap();

            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let mut resources = DeviceResources::new();
            let entry = dbs_device::resources::Resource::MemAddressRange {
                base: 0x100000000,
                size: 0xc00 << 20,
            };
            resources.append(entry);
            let entry = dbs_device::resources::Resource::KvmMemSlot(0);
            resources.append(entry);
            let entry = dbs_device::resources::Resource::KvmMemSlot(1);
            resources.append(entry);
            let content =
                VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::set_resource(
                    &mut dev, vm_fd, resources,
                )
                .unwrap();
            assert!(content.is_none());
        }

        // disable multi-region in virtio-mem
        {
            let mut dev = Mem::<Arc<GuestMemoryMmap>>::new(
                id, 0xc00, 0xc00, false, None, epoll_mgr, factory, 0xc0000000,
            )
            .unwrap();

            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let mut resources = DeviceResources::new();
            let entry = dbs_device::resources::Resource::MemAddressRange {
                base: 0x100000000,
                size: 0xc00 << 20,
            };
            resources.append(entry);
            let entry = dbs_device::resources::Resource::KvmMemSlot(0);
            resources.append(entry);
            let content =
                VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::set_resource(
                    &mut dev, vm_fd, resources,
                )
                .unwrap();
            assert!(content.is_none());
        }
    }

    #[test]
    fn test_mem_virtio_device_spec() {
        let epoll_mgr = EpollManager::default();
        let id = "mem0".to_string();
        let factory = Arc::new(Mutex::new(DummyMemRegionFactory {}));
        let dev =
            Mem::<Arc<GuestMemoryMmap>>::new(id, 200, 200, false, None, epoll_mgr, factory, 200)
                .unwrap();
        assert!(dev.set_requested_size(200).is_ok());
    }

    #[test]
    fn test_mem_virtio_device_activate() {
        let epoll_mgr = EpollManager::default();
        let id = "mem0".to_string();
        let factory = Arc::new(Mutex::new(DummyMemRegionFactory {}));
        // queue length error
        {
            let mut dev = Mem::<Arc<GuestMemoryMmap>>::new(
                id.clone(),
                200,
                200,
                false,
                None,
                epoll_mgr.clone(),
                factory.clone(),
                200,
            )
            .unwrap();

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues = vec![
                VirtioQueueConfig::<QueueSync>::create(16, 0).unwrap(),
                VirtioQueueConfig::<QueueSync>::create(16, 0).unwrap(),
            ];

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
            let result = dev.activate(config);
            assert!(matches!(result, Err(ActivateError::InvalidParam)));
        }
        // fail because map_regions should not be empty
        {
            let mut dev = Mem::<Arc<GuestMemoryMmap>>::new(
                id.clone(),
                200,
                200,
                false,
                None,
                epoll_mgr.clone(),
                factory.clone(),
                200,
            )
            .unwrap();

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues = vec![VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap()];

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
            let result = dev.activate(config);
            assert!(matches!(result, Err(ActivateError::InternalError)));
        }
        // test activate mem device is correct
        {
            let mut dev = Mem::<Arc<GuestMemoryMmap>>::new(
                id, 200, 200, false, None, epoll_mgr, factory, 200,
            )
            .unwrap();

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues = vec![VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap()];

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
            dev.map_regions.lock().unwrap().push((0, None));
            assert!(dev.activate(config).is_ok());
        }
    }

    #[test]
    fn test_mem_virtio_device_remove() {
        let epoll_mgr = EpollManager::default();
        let id = "mem0".to_string();
        let factory = Arc::new(Mutex::new(DummyMemRegionFactory {}));
        let mut dev =
            Mem::<Arc<GuestMemoryMmap>>::new(id, 200, 200, false, None, epoll_mgr, factory, 200)
                .unwrap();

        let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let queues = vec![VirtioQueueConfig::<QueueSync>::create(128, 0).unwrap()];

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
        dev.map_regions.lock().unwrap().push((0, None));

        // test activate mem device is correct
        assert!(dev.activate(config).is_ok());
        assert!(dev.subscriber_id.is_some());
        // test remove mem device is correct
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::remove(&mut dev);
        assert!(dev.subscriber_id.is_none());
    }

    #[test]
    fn test_mem_epoll_handler_handle_event() {
        let handler = create_mem_epoll_handler("test_1".to_string());
        let event_fd = EventFd::new(0).unwrap();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_op = inner_mgr.event_ops(id).unwrap();
        let event_set = EventSet::EDGE_TRIGGERED;
        let mut handler = create_mem_epoll_handler("test_2".to_string());

        //invalid queue index
        let events = Events::with_data(&event_fd, 1024, event_set);
        handler.config.queues[0].generate_event().unwrap();
        handler.process(events, &mut event_op);
        //valid
        let events = Events::with_data(&event_fd, 0, event_set);
        handler.config.queues[0].generate_event().unwrap();
        handler.process(events, &mut event_op);
    }

    #[test]
    fn test_mem_epoll_handler_process_queue() {
        let mut handler = create_mem_epoll_handler("test_1".to_string());
        let m = &handler.config.vm_as.clone();
        // fail to parse available descriptor chain
        {
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            let q = vq.create_queue();
            vq.dtable(0).set(0x1000, 0x400, VIRTQ_DESC_F_NEXT, 1);
            handler.config.queues = vec![VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            )];
            handler.config.queues[0].generate_event().unwrap();
            assert!(handler.process_queue(0));
        }
        // success
        {
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            let q = vq.create_queue();
            vq.dtable(0).set(0x1000, 0x4, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1).set(0x2000, 0x4, VIRTQ_DESC_F_WRITE, 2);
            handler.config.queues = vec![VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            )];
            handler.config.queues[0].generate_event().unwrap();
            assert!(handler.process_queue(0));
        }
    }
}
