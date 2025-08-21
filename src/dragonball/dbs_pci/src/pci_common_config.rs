// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//
// Copyright (C) 2024 Alibaba Cloud. All rights reserved.
//
// Copyright (C) 2025 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause

use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

use byteorder::{ByteOrder, LittleEndian};
use log::{error, trace, warn};
use serde::{Deserialize, Serialize};
use virtio_queue::QueueT;
use vm_memory::{GuestAddressSpace, GuestMemoryRegion};

use crate::ArcMutexBoxDynVirtioDevice;
use dbs_virtio_devices::VirtioQueueConfig;

#[derive(Clone, Serialize, Deserialize)]
pub struct VirtioPciCommonConfigState {
    pub driver_status: u8,
    pub config_generation: u8,
    pub device_feature_select: u32,
    pub driver_feature_select: u32,
    pub queue_select: u16,
    pub msix_config: u16,
    pub msix_queues: Vec<u16>,
}

/* The standard layout for the ring is a continuous chunk of memory which looks
 * like this.  We assume num is a power of 2.
 *
 * struct vring
 * {
 *	// The actual descriptors (16 bytes each)
 *	struct vring_desc desc[num];
 *
 *	// A ring of available descriptor heads with free-running index.
 *	__virtio16 avail_flags;
 *	__virtio16 avail_idx;
 *	__virtio16 available[num];
 *	__virtio16 used_event_idx;
 *
 *	// Padding to the next align boundary.
 *	char pad[];
 *
 *	// A ring of used descriptor heads with free-running index.
 *	__virtio16 used_flags;
 *	__virtio16 used_idx;
 *	struct vring_used_elem used[num];
 *	__virtio16 avail_event_idx;
 * };
 * struct vring_desc {
 *	__virtio64 addr;
 *	__virtio32 len;
 *	__virtio16 flags;
 *	__virtio16 next;
 * };
 *
 * struct vring_avail {
 *	__virtio16 flags;
 *	__virtio16 idx;
 *	__virtio16 ring[];
 * };
 *
 * // u32 is used here for ids for padding reasons.
 * struct vring_used_elem {
 *	// Index of start of used descriptor chain.
 *	__virtio32 id;
 *	// Total length of the descriptor chain which was used (written to)
 *	__virtio32 len;
 * };
*
 * Kernel header used for this reference: include/uapi/linux/virtio_ring.h
 * Virtio Spec: https://docs.oasis-open.org/virtio/virtio/v1.2/csd01/virtio-v1.2-csd01.html
 *
 */

/// Contains the data for reading and writing the common configuration structure of a virtio PCI
/// device.
///
/// * Registers:
///
/// ** About the whole device.
///    le32 device_feature_select;     // 0x00 // read-write
///    le32 device_feature;            // 0x04 // read-only for driver
///    le32 driver_feature_select;     // 0x08 // read-write
///    le32 driver_feature;            // 0x0C // read-write
///    le16 msix_config;               // 0x10 // read-write
///    le16 num_queues;                // 0x12 // read-only for driver
///    u8 device_status;               // 0x14 // read-write (driver_status)
///    u8 config_generation;           // 0x15 // read-only for driver
///
/// ** About a specific virtqueue.
///    le16 queue_select;              // 0x16 // read-write
///    le16 queue_size;                // 0x18 // read-write, power of 2, or 0.
///    le16 queue_msix_vector;         // 0x1A // read-write
///    le16 queue_enable;              // 0x1C // read-write (Ready)
///    le16 queue_notify_off;          // 0x1E // read-only for driver
///    le64 queue_desc;                // 0x20 // read-write
///    le64 queue_avail;               // 0x28 // read-write
///    le64 queue_used;                // 0x30 // read-write
pub struct VirtioPciCommonConfig {
    pub driver_status: u8,
    pub config_generation: u8,
    pub device_feature_select: u32,
    pub driver_feature_select: u32,
    pub queue_select: u16,
    pub msix_config: Arc<AtomicU16>,
    pub msix_queues: Arc<Mutex<Vec<u16>>>,
}

impl VirtioPciCommonConfig {
    pub fn new(state: VirtioPciCommonConfigState) -> Self {
        VirtioPciCommonConfig {
            driver_status: state.driver_status,
            config_generation: state.config_generation,
            device_feature_select: state.device_feature_select,
            driver_feature_select: state.driver_feature_select,
            queue_select: state.queue_select,
            msix_config: Arc::new(AtomicU16::new(state.msix_config)),
            msix_queues: Arc::new(Mutex::new(state.msix_queues)),
        }
    }

    // TODO(fupan): use for live upgrade later
    #[allow(dead_code)]
    fn state(&self) -> VirtioPciCommonConfigState {
        VirtioPciCommonConfigState {
            driver_status: self.driver_status,
            config_generation: self.config_generation,
            device_feature_select: self.device_feature_select,
            driver_feature_select: self.driver_feature_select,
            queue_select: self.queue_select,
            msix_config: self.msix_config.load(Ordering::Acquire),
            msix_queues: self.msix_queues.lock().unwrap().clone(),
        }
    }

    fn read_common_config_byte(&self, offset: u64) -> u8 {
        trace!("read_common_config_byte: offset 0x{:x}", offset);
        // The driver is only allowed to do aligned, properly sized access.
        match offset {
            0x14 => self.driver_status,
            0x15 => self.config_generation,
            _ => {
                warn!("invalid virtio config byte read: 0x{:x}", offset);
                0
            }
        }
    }

    fn write_common_config_byte(&mut self, offset: u64, value: u8) {
        trace!(
            "write_common_config_byte: offset 0x{:x} value 0x{:x}",
            offset,
            value
        );
        match offset {
            0x14 => self.driver_status = value,
            _ => {
                warn!("invalid virtio config byte write: 0x{:x}", offset);
            }
        }
    }

    fn read_common_config_word<Q: QueueT + 'static>(
        &self,
        offset: u64,
        queues: &[VirtioQueueConfig<Q>],
    ) -> u16 {
        trace!("read_common_config_word: offset 0x{:x}", offset);
        match offset {
            0x10 => self.msix_config.load(Ordering::Acquire),
            0x12 => queues.len() as u16, // num_queues
            0x16 => self.queue_select,
            0x18 => self.with_queue(queues, |q| q.max_size()).unwrap_or(0),
            0x1a => self.msix_queues.lock().unwrap()[self.queue_select as usize],
            0x1c => u16::from(self.with_queue(queues, |q| q.ready()).unwrap_or(false)),
            0x1e => self.queue_select, // notify_off
            _ => {
                warn!("invalid virtio register word read: 0x{:x}", offset);
                0
            }
        }
    }

    fn write_common_config_word<Q: QueueT + 'static>(
        &mut self,
        offset: u64,
        value: u16,
        queues: &mut [VirtioQueueConfig<Q>],
    ) {
        trace!(
            "write_common_config_word: offset 0x{:x} value 0x{:x}",
            offset,
            value
        );
        match offset {
            0x10 => self.msix_config.store(value, Ordering::Release),
            0x16 => self.queue_select = value,
            0x18 => self.with_queue_mut(queues, |q| q.set_size(value)),
            0x1a => self.msix_queues.lock().unwrap()[self.queue_select as usize] = value,
            0x1c => self.with_queue_mut(queues, |q| {
                let ready = value == 1;
                q.set_ready(ready);
            }),
            _ => {
                warn!("invalid virtio register word write: 0x{:x}", offset);
            }
        }
    }

    fn read_common_config_dword<
        AS: GuestAddressSpace + 'static,
        Q: QueueT + 'static,
        R: 'static + GuestMemoryRegion,
    >(
        &self,
        offset: u64,
        device: ArcMutexBoxDynVirtioDevice<AS, Q, R>,
    ) -> u32 {
        trace!("read_common_config_dword: offset 0x{:x}", offset);
        match offset {
            0x00 => self.device_feature_select,
            0x04 => {
                // Only 64 bits of features (2 pages) are defined for now, so limit
                // device_feature_select to avoid shifting by 64 or more bits.
                let locked_device = device.lock().unwrap();
                if self.device_feature_select < 2 {
                    locked_device.get_avail_features(self.device_feature_select)
                } else {
                    0
                }
            }
            0x08 => self.driver_feature_select,
            _ => {
                warn!("invalid virtio register dword read: 0x{:x}", offset);
                0
            }
        }
    }

    fn write_common_config_dword<
        AS: GuestAddressSpace + 'static,
        Q: QueueT + 'static,
        R: 'static + GuestMemoryRegion,
    >(
        &mut self,
        offset: u64,
        value: u32,
        queues: &mut [VirtioQueueConfig<Q>],
        device: ArcMutexBoxDynVirtioDevice<AS, Q, R>,
    ) {
        trace!(
            "write_common_config_dword: offset 0x{:x} value 0x{:x}",
            offset,
            value
        );

        match offset {
            0x00 => self.device_feature_select = value,
            0x08 => self.driver_feature_select = value,
            0x0c => {
                if self.driver_feature_select < 2 {
                    let mut locked_device = device.lock().unwrap();
                    locked_device.set_acked_features(self.driver_feature_select, value);
                } else {
                    warn!(
                        "invalid ack_features (page {}, value 0x{:x})",
                        self.driver_feature_select, value
                    );
                }
            }
            0x20 => self.with_queue_mut(queues, |q| q.set_desc_table_address(Some(value), None)),
            0x24 => self.with_queue_mut(queues, |q| q.set_desc_table_address(None, Some(value))),
            0x28 => self.with_queue_mut(queues, |q| q.set_avail_ring_address(Some(value), None)),
            0x2c => self.with_queue_mut(queues, |q| q.set_avail_ring_address(None, Some(value))),
            0x30 => self.with_queue_mut(queues, |q| q.set_used_ring_address(Some(value), None)),
            0x34 => self.with_queue_mut(queues, |q| q.set_used_ring_address(None, Some(value))),
            _ => {
                warn!("invalid virtio register dword write: 0x{:x}", offset);
            }
        }
    }

    fn read_common_config_qword(&self, _offset: u64) -> u64 {
        trace!("read_common_config_qword: offset 0x{:x}", _offset);
        0 // Assume the guest has no reason to read write-only registers.
    }

    fn write_common_config_qword<Q: QueueT + 'static>(
        &mut self,
        offset: u64,
        value: u64,
        queues: &mut [VirtioQueueConfig<Q>],
    ) {
        trace!(
            "write_common_config_qword: offset 0x{:x}, value 0x{:x}",
            offset,
            value
        );

        let low = Some((value & 0xffff_ffff) as u32);
        let high = Some((value >> 32) as u32);

        match offset {
            0x20 => self.with_queue_mut(queues, |q| q.set_desc_table_address(low, high)),
            0x28 => self.with_queue_mut(queues, |q| q.set_avail_ring_address(low, high)),
            0x30 => self.with_queue_mut(queues, |q| q.set_used_ring_address(low, high)),
            _ => {
                warn!("invalid virtio register qword write: 0x{:x}", offset);
            }
        }
    }

    fn with_queue<U, F, Q>(&self, queues: &[VirtioQueueConfig<Q>], f: F) -> Option<U>
    where
        F: FnOnce(&Q) -> U,
        Q: QueueT + 'static,
    {
        queues.get(self.queue_select as usize).map(|q| f(&q.queue))
    }

    fn with_queue_mut<F: FnOnce(&mut Q), Q: QueueT + 'static>(
        &self,
        queues: &mut [VirtioQueueConfig<Q>],
        f: F,
    ) {
        if let Some(queue) = queues.get_mut(self.queue_select as usize) {
            f(&mut queue.queue);
        }
    }

    pub fn read<
        AS: GuestAddressSpace + 'static,
        Q: QueueT + 'static,
        R: 'static + GuestMemoryRegion,
    >(
        &self,
        offset: u64,
        data: &mut [u8],
        queues: &[VirtioQueueConfig<Q>],
        device: ArcMutexBoxDynVirtioDevice<AS, Q, R>,
    ) {
        assert!(data.len() <= 8);

        match data.len() {
            1 => {
                let v = self.read_common_config_byte(offset);
                data[0] = v;
            }
            2 => {
                let v = self.read_common_config_word(offset, queues);
                LittleEndian::write_u16(data, v);
            }
            4 => {
                let v = self.read_common_config_dword(offset, device);
                LittleEndian::write_u32(data, v);
            }
            8 => {
                let v = self.read_common_config_qword(offset);
                LittleEndian::write_u64(data, v);
            }
            _ => error!("invalid data length for virtio read: len {}", data.len()),
        }
    }

    pub fn write<
        AS: GuestAddressSpace + 'static,
        Q: QueueT + 'static,
        R: 'static + GuestMemoryRegion,
    >(
        &mut self,
        offset: u64,
        data: &[u8],
        queues: &mut [VirtioQueueConfig<Q>],
        device: ArcMutexBoxDynVirtioDevice<AS, Q, R>,
    ) {
        assert!(data.len() <= 8);

        match data.len() {
            1 => self.write_common_config_byte(offset, data[0]),
            2 => self.write_common_config_word(offset, LittleEndian::read_u16(data), queues),
            4 => {
                self.write_common_config_dword(offset, LittleEndian::read_u32(data), queues, device)
            }
            8 => self.write_common_config_qword(offset, LittleEndian::read_u64(data), queues),
            _ => error!("invalid data length for virtio write: len {}", data.len()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::virtio_pci::tests::{DummyDevice, DUMMY_FEATURES};
    use super::*;
    use dbs_virtio_devices::VirtioDevice;
    use virtio_queue::QueueSync;
    use vm_memory::{GuestMemoryMmap, GuestRegionMmap};

    #[test]
    fn write_base_regs() {
        let regs_state = VirtioPciCommonConfigState {
            driver_status: 0xaa,
            config_generation: 0x55,
            device_feature_select: 0x0,
            driver_feature_select: 0x0,
            queue_select: 0xff,
            msix_config: 0,
            msix_queues: vec![0; 3],
        };
        let mut regs = VirtioPciCommonConfig::new(regs_state);

        let dev: Arc<
            Mutex<Box<dyn VirtioDevice<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>>>,
        > = Arc::new(Mutex::new(Box::new(DummyDevice::new())));
        let mut queues = Vec::new();
        queues.push(VirtioQueueConfig::create(2, 0).unwrap());
        queues.push(VirtioQueueConfig::create(2, 1).unwrap());

        // Can set all bits of driver_status.
        regs.write(0x14, &[0x55], &mut queues, Arc::clone(&dev));
        let mut read_back = vec![0x00];
        regs.read(0x14, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(read_back[0], 0x55);

        // The config generation register is read only.
        regs.write(0x15, &[0xaa], &mut queues, Arc::clone(&dev));
        let mut read_back = vec![0x00];
        regs.read(0x15, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(read_back[0], 0x55);

        // Device features is read-only and passed through from the device.
        regs.write(0x04, &[0, 0, 0, 0], &mut queues, Arc::clone(&dev));
        let mut read_back = vec![0, 0, 0, 0];
        regs.read(0x04, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(LittleEndian::read_u32(&read_back), DUMMY_FEATURES as u32);

        // Read device features with device_feature_select as 0
        regs.write(0x00, &[0, 0, 0, 0], &mut queues, Arc::clone(&dev));
        let mut read_back = vec![0, 0, 0, 0];
        regs.read(0x04, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(LittleEndian::read_u32(&read_back), DUMMY_FEATURES as u32);

        // Read device features with device_feature_select as 1
        regs.write(0x00, &[1, 0, 0, 0], &mut queues, Arc::clone(&dev));
        let mut read_back = vec![0, 0, 0, 0];
        regs.read(0x04, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(
            LittleEndian::read_u32(&read_back),
            (DUMMY_FEATURES >> 32) as u32
        );

        // Feature select registers are read/write.
        regs.write(0x00, &[1, 2, 3, 4], &mut queues, Arc::clone(&dev));
        let mut read_back = vec![0, 0, 0, 0];
        regs.read(0x00, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(LittleEndian::read_u32(&read_back), 0x0403_0201);
        regs.write(0x08, &[1, 2, 3, 4], &mut queues, Arc::clone(&dev));
        let mut read_back = vec![0, 0, 0, 0];
        regs.read(0x08, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(LittleEndian::read_u32(&read_back), 0x0403_0201);

        // 'queue_select' can be read and written.
        regs.write(0x16, &[0xaa, 0x55], &mut queues, Arc::clone(&dev));
        let mut read_back = vec![0x00, 0x00];
        regs.read(0x16, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(read_back[0], 0xaa);
        assert_eq!(read_back[1], 0x55);

        // write msix_queues by queue_select 2
        regs.write(0x16, &[0x02, 0x00], &mut queues, Arc::clone(&dev));
        regs.write(0x1a, &[0xbb, 0xcc], &mut queues, Arc::clone(&dev));
        let mut read_back = vec![0x00, 0x00];
        regs.read(0x1a, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(read_back[0], 0xbb);
        assert_eq!(read_back[1], 0xcc);

        // 'msix_config' can be read and written.
        regs.write(0x10, &[0xdd, 0xee], &mut queues, Arc::clone(&dev));
        let mut read_back = vec![0x00, 0x00];
        regs.read(0x10, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(read_back[0], 0xdd);
        assert_eq!(read_back[1], 0xee);

        // 'queue_size' can be read and set.
        let mut read_back = vec![0x00, 0x00];
        // queue_select is 2 and queues[2] is None, so queue_size is 0
        regs.read(0x18, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(read_back[0], 0x00);
        assert_eq!(read_back[1], 0x00);
        // queue_select is 1, so queue_size is 2
        regs.write(0x16, &[0x01, 0x00], &mut queues, Arc::clone(&dev));
        regs.read(0x18, &mut read_back, &queues, Arc::clone(&dev));
        assert_eq!(read_back[0], 0x02);
        assert_eq!(read_back[1], 0x00);
    }
}
