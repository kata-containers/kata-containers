// Copyright 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

//! Wrappers over `InterruptNotifier` to support virtio device interrupt management.

use std::sync::Arc;

use dbs_interrupt::{
    InterruptIndex, InterruptNotifier, InterruptSourceGroup, InterruptSourceType,
    InterruptStatusRegister32, LegacyNotifier, MsiNotifier,
};

use crate::{VIRTIO_INTR_CONFIG, VIRTIO_INTR_VRING};

/// Create an interrupt notifier for virtio device change events.
pub fn create_device_notifier(
    group: Arc<Box<dyn InterruptSourceGroup>>,
    intr_status: Arc<InterruptStatusRegister32>,
    intr_index: InterruptIndex,
) -> Arc<dyn InterruptNotifier> {
    match group.interrupt_type() {
        InterruptSourceType::LegacyIrq => {
            Arc::new(LegacyNotifier::new(group, intr_status, VIRTIO_INTR_CONFIG))
        }
        InterruptSourceType::MsiIrq => Arc::new(MsiNotifier::new(group, intr_index)),
    }
}

/// Create an interrupt notifier for virtio queue notification events.
pub fn create_queue_notifier(
    group: Arc<Box<dyn InterruptSourceGroup>>,
    intr_status: Arc<InterruptStatusRegister32>,
    intr_index: InterruptIndex,
) -> Arc<dyn InterruptNotifier> {
    match group.interrupt_type() {
        InterruptSourceType::LegacyIrq => {
            Arc::new(LegacyNotifier::new(group, intr_status, VIRTIO_INTR_VRING))
        }
        InterruptSourceType::MsiIrq => Arc::new(MsiNotifier::new(group, intr_index)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbs_interrupt::InterruptManager;

    #[test]
    fn test_create_virtio_legacy_notifier() {
        let (_vmfd, irq_manager) = crate::tests::create_vm_and_irq_manager();
        let group = irq_manager
            .create_group(InterruptSourceType::LegacyIrq, 0, 1)
            .unwrap();
        let status = Arc::new(InterruptStatusRegister32::new());
        assert_eq!(status.read(), 0);

        let notifer = create_queue_notifier(group.clone(), status.clone(), 0);
        notifer.notify().unwrap();
        assert!(notifer.notifier().is_some());

        assert_eq!(status.read(), VIRTIO_INTR_VRING);
        status.clear_bits(VIRTIO_INTR_VRING);
        assert_eq!(status.read(), 0);
        let eventfd = notifer.notifier().unwrap();
        eventfd.write(2).unwrap();
        assert_eq!(eventfd.read().unwrap(), 3);
    }

    #[test]
    fn test_create_virtio_msi_notifier() {
        let (_vmfd, irq_manager) = crate::tests::create_vm_and_irq_manager();
        let group = irq_manager
            .create_group(InterruptSourceType::MsiIrq, 0, 3)
            .unwrap();
        let status = Arc::new(InterruptStatusRegister32::new());

        let notifier1 = create_device_notifier(group.clone(), status.clone(), 1);
        let notifier2 = create_queue_notifier(group.clone(), status.clone(), 2);
        let notifier3 = create_queue_notifier(group.clone(), status, 3);
        assert!(notifier1.notifier().is_some());
        assert!(notifier2.notifier().is_some());
        assert!(notifier3.notifier().is_none());
        notifier1.notify().unwrap();
        notifier1.notify().unwrap();
        notifier2.notify().unwrap();
        assert_eq!(notifier1.notifier().unwrap().read().unwrap(), 2);
        assert_eq!(notifier2.notifier().unwrap().read().unwrap(), 1);
    }
}
