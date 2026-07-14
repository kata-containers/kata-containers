// Copyright (C) 2026 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Snapshot state definitions shared by all virtio devices.
//!
//! These are plain-old-data mirrors of the runtime structures, serializable
//! with serde. Compatibility policy: only append new fields (with
//! `#[serde(default)]`); never remove or repurpose existing ones.

use serde::{Deserialize, Serialize};
use virtio_queue::QueueT;

use crate::{Error, Result, VirtioDeviceInfo, VirtioQueueConfig};

/// The snapshot contract as implemented by virtio devices.
///
/// An alias for [`dbs_snapshot::Persist`], spelled this way at virtio device
/// impl sites so the contract reads in terms of the device rather than the
/// generic VMM-wide trait. Devices choose their own `State`; nothing here
/// constrains them to a shared shape.
///
/// Devices only: the transport ([`crate::mmio::MmioV2Device`]) is not a
/// device, so it must not be spelled with this alias. Should it ever
/// implement the contract, it should use [`dbs_snapshot::Persist`] directly
/// or its own transport-specific alias.
pub use dbs_snapshot::Persist as VirtioDevicePersist;

/// Serializable state of a virtio queue (mirror of
/// `virtio_queue::QueueState` with serde support).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct VirtioQueueState {
    /// The maximum size in elements offered by the device.
    pub max_size: u16,
    /// Tail position of the available ring.
    pub next_avail: u16,
    /// Head position of the used ring.
    pub next_used: u16,
    /// VIRTIO_F_RING_EVENT_IDX negotiated.
    pub event_idx_enabled: bool,
    /// The queue size in elements the driver selected.
    pub size: u16,
    /// Indicates if the queue finished with configuration.
    pub ready: bool,
    /// Guest physical address of the descriptor table.
    pub desc_table: u64,
    /// Guest physical address of the available ring.
    pub avail_ring: u64,
    /// Guest physical address of the used ring.
    pub used_ring: u64,
}

impl VirtioQueueState {
    /// Capture the state of a virtio queue.
    pub fn save<Q: QueueT>(queue: &Q) -> Self {
        VirtioQueueState {
            max_size: queue.max_size(),
            next_avail: queue.next_avail(),
            next_used: queue.next_used(),
            event_idx_enabled: queue.event_idx_enabled(),
            size: queue.size(),
            ready: queue.ready(),
            desc_table: queue.desc_table(),
            avail_ring: queue.avail_ring(),
            used_ring: queue.used_ring(),
        }
    }

    /// Apply this state to a freshly created virtio queue.
    ///
    /// The target queue must have been created with the same `max_size`,
    /// otherwise the state is refused.
    pub fn restore<Q: QueueT>(&self, queue: &mut Q) -> Result<()> {
        if queue.max_size() != self.max_size {
            return Err(Error::InvalidInput);
        }
        queue.set_size(self.size);
        queue.set_desc_table_address(
            Some(self.desc_table as u32),
            Some((self.desc_table >> 32) as u32),
        );
        queue.set_avail_ring_address(
            Some(self.avail_ring as u32),
            Some((self.avail_ring >> 32) as u32),
        );
        queue.set_used_ring_address(
            Some(self.used_ring as u32),
            Some((self.used_ring >> 32) as u32),
        );
        queue.set_event_idx(self.event_idx_enabled);
        queue.set_next_avail(self.next_avail);
        queue.set_next_used(self.next_used);
        // Set ready last: with all addresses in place the queue becomes valid.
        queue.set_ready(self.ready);
        Ok(())
    }
}

/// Serializable state of a `VirtioQueueConfig`.
///
/// The queue notification eventfd and the interrupt notifier are live
/// resources re-created by the transport on restore.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct VirtioQueueConfigState {
    /// Queue index into the queue array.
    pub index: u16,
    /// State of the virtio queue itself.
    pub queue: VirtioQueueState,
}

impl<Q: QueueT> VirtioQueueConfig<Q> {
    /// Capture the state of this queue configuration.
    pub fn save_state(&self) -> VirtioQueueConfigState {
        VirtioQueueConfigState {
            index: self.index(),
            queue: VirtioQueueState::save(&self.queue),
        }
    }

    /// Apply a previously saved state to this queue configuration.
    ///
    /// `self` must be a freshly created queue configuration with the same
    /// index and maximum queue size as the saved one.
    pub fn restore_state(&mut self, state: &VirtioQueueConfigState) -> Result<()> {
        if self.index() != state.index {
            return Err(Error::InvalidInput);
        }
        state.queue.restore(&mut self.queue)
    }
}

/// Serializable state of a `VirtioDeviceInfo`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct VirtioDeviceInfoState {
    /// Features offered by the device when the snapshot was taken.
    pub avail_features: u64,
    /// Features acknowledged by the guest driver.
    pub acked_features: u64,
    /// Device specific configuration space.
    pub config_space: Vec<u8>,
}

impl VirtioDeviceInfo {
    /// Capture the guest-negotiated state of this device.
    pub fn save_state(&self) -> VirtioDeviceInfoState {
        VirtioDeviceInfoState {
            avail_features: self.avail_features,
            acked_features: self.acked_features,
            config_space: self.config_space.clone(),
        }
    }

    /// Apply a previously saved state to this device.
    ///
    /// The device must have been re-created with the same configuration: if
    /// the offered feature set differs from the one the guest negotiated
    /// against, the state is refused (regenerate the snapshot instead).
    pub fn restore_state(&mut self, state: &VirtioDeviceInfoState) -> Result<()> {
        if self.avail_features != state.avail_features {
            return Err(Error::InvalidInput);
        }
        self.acked_features = state.acked_features;
        self.config_space = state.config_space.clone();
        Ok(())
    }
}

/// Serializable state of one MSI vector programmed by the guest.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct MsiVectorState {
    /// Low 32 bits of the MSI message address.
    pub address_low: u32,
    /// High 32 bits of the MSI message address.
    pub address_high: u32,
    /// MSI message data.
    pub data: u32,
}

/// Serializable state of a `MmioV2Device` transport.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct MmioV2TransportState {
    /// Virtio device status register.
    pub driver_status: u32,
    /// Configuration generation counter.
    pub config_generation: u32,
    /// Pending interrupt status bits.
    pub interrupt_status: u32,
    /// Device features word currently selected by the driver.
    pub features_select: u32,
    /// Driver features word currently selected by the driver.
    pub acked_features_select: u32,
    /// Queue currently selected by the driver.
    pub queue_select: u32,
    /// Per-queue state, in queue index order (including the control queue,
    /// if any).
    pub queues: Vec<VirtioQueueConfigState>,
    /// Whether the device had been activated by the guest.
    pub device_activated: bool,
    /// Whether the guest had switched the device to MSI interrupt mode.
    /// Without replaying this, device-to-guest interrupts never fire after
    /// restore and the guest waits forever on virtio completions.
    #[serde(default)]
    pub msi_enabled: bool,
    /// Per-vector MSI configuration programmed by the guest, in vector
    /// order.
    #[serde(default)]
    pub msi_vectors: Vec<MsiVectorState>,
}

#[cfg(test)]
mod tests {
    use virtio_queue::{Queue, QueueSync};

    use super::*;

    #[test]
    fn test_virtio_queue_state_roundtrip() {
        let mut src: Queue = Queue::new(256).unwrap();
        src.set_size(128);
        src.set_desc_table_address(Some(0x1000), Some(0x1));
        src.set_avail_ring_address(Some(0x2000), Some(0));
        src.set_used_ring_address(Some(0x3000), Some(0));
        src.set_event_idx(true);
        src.set_next_avail(10);
        src.set_next_used(5);
        src.set_ready(true);

        let state = VirtioQueueState::save(&src);
        assert_eq!(state.max_size, 256);
        assert_eq!(state.size, 128);
        assert_eq!(state.desc_table, 0x1_0000_1000);
        assert_eq!(state.next_avail, 10);
        assert_eq!(state.next_used, 5);
        assert!(state.ready);
        assert!(state.event_idx_enabled);

        // JSON round-trip.
        let json = serde_json::to_string(&state).unwrap();
        let state: VirtioQueueState = serde_json::from_str(&json).unwrap();

        let mut dst: Queue = Queue::new(256).unwrap();
        state.restore(&mut dst).unwrap();
        assert_eq!(VirtioQueueState::save(&dst), state);

        // Mismatched max size must be refused.
        let mut small: Queue = Queue::new(128).unwrap();
        assert!(state.restore(&mut small).is_err());
    }

    #[test]
    fn test_virtio_queue_config_state_roundtrip() {
        let mut src: VirtioQueueConfig<QueueSync> = VirtioQueueConfig::create(256, 3).unwrap();
        src.queue.set_size(64);
        src.queue.set_next_avail(7);
        let state = src.save_state();
        assert_eq!(state.index, 3);
        assert_eq!(state.queue.size, 64);

        let mut dst: VirtioQueueConfig<QueueSync> = VirtioQueueConfig::create(256, 3).unwrap();
        dst.restore_state(&state).unwrap();
        assert_eq!(dst.save_state(), state);

        // Wrong index must be refused.
        let mut other: VirtioQueueConfig<QueueSync> = VirtioQueueConfig::create(256, 4).unwrap();
        assert!(other.restore_state(&state).is_err());
    }
}
