// Copyright 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

//! Event notifier to inject device interrupts to virtual machines.

use std::any::Any;
use std::io::Error;
use std::sync::Arc;

use vmm_sys_util::eventfd::EventFd;

use crate::{InterruptIndex, InterruptSourceGroup, InterruptStatusRegister32};

#[cfg(feature = "legacy-irq")]
pub use self::legacy::*;
#[cfg(feature = "msi-irq")]
pub use self::msi::*;

/// Trait to inject device interrupts to virtual machines.
pub trait InterruptNotifier: Send + Sync {
    /// Inject a device interrupt to the virtual machine.
    fn notify(&self) -> Result<(), Error>;

    /// Get the optional `EventFd` object to inject interrupt to the virtual machine.
    fn notifier(&self) -> Option<&EventFd>;

    /// Clone a boxed dyn trait object.
    fn clone_boxed(&self) -> Box<dyn InterruptNotifier>;

    /// Convert `self` to `std::any::Any`.
    fn as_any(&self) -> &dyn Any;
}

#[cfg(feature = "legacy-irq")]
mod legacy {
    use super::*;

    /// Struct to inject legacy interrupt to guest.
    #[derive(Clone)]
    pub struct LegacyNotifier {
        pub(crate) intr_group: Arc<Box<dyn InterruptSourceGroup>>,
        pub(crate) intr_status: Arc<InterruptStatusRegister32>,
        pub(crate) status_bits: u32,
    }

    impl LegacyNotifier {
        /// Create a legacy notifier.
        pub fn new(
            intr_group: Arc<Box<dyn InterruptSourceGroup>>,
            intr_status: Arc<InterruptStatusRegister32>,
            status_bits: u32,
        ) -> Self {
            Self {
                intr_group,
                intr_status,
                status_bits,
            }
        }
    }

    impl InterruptNotifier for LegacyNotifier {
        fn notify(&self) -> Result<(), Error> {
            self.intr_status.set_bits(self.status_bits);
            self.intr_group.trigger(0)
        }

        fn notifier(&self) -> Option<&EventFd> {
            self.intr_group.notifier(0)
        }

        fn clone_boxed(&self) -> Box<dyn InterruptNotifier> {
            Box::new(self.clone())
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }
}

#[cfg(feature = "msi-irq")]
mod msi {
    use super::*;

    /// Struct to inject message signalled interrupt to guest.
    #[derive(Clone)]
    pub struct MsiNotifier {
        pub(crate) intr_group: Arc<Box<dyn InterruptSourceGroup>>,
        pub(crate) intr_index: InterruptIndex,
    }

    impl MsiNotifier {
        /// Create a notifier to inject message signalled interrupt to guest.
        pub fn new(
            intr_group: Arc<Box<dyn InterruptSourceGroup>>,
            intr_index: InterruptIndex,
        ) -> Self {
            MsiNotifier {
                intr_group,
                intr_index,
            }
        }
    }

    impl InterruptNotifier for MsiNotifier {
        fn notify(&self) -> Result<(), Error> {
            self.intr_group.trigger(self.intr_index)
        }

        fn notifier(&self) -> Option<&EventFd> {
            self.intr_group.notifier(self.intr_index)
        }

        fn clone_boxed(&self) -> Box<dyn InterruptNotifier> {
            Box::new(self.clone())
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }
}

/// Struct to discard interrupts.
#[derive(Copy, Clone, Debug, Default)]
pub struct NoopNotifier {}

impl NoopNotifier {
    /// Create a noop notifier to discard interrupts.
    pub fn new() -> Self {
        NoopNotifier {}
    }
}

impl InterruptNotifier for NoopNotifier {
    fn notify(&self) -> Result<(), Error> {
        Ok(())
    }

    fn notifier(&self) -> Option<&EventFd> {
        None
    }

    fn clone_boxed(&self) -> Box<dyn InterruptNotifier> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Clone a boxed interrupt notifier object.
pub fn clone_notifier(notifier: &dyn InterruptNotifier) -> Box<dyn InterruptNotifier> {
    notifier.clone_boxed()
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    #![allow(dead_code)]
    use super::*;

    use crate::{InterruptManager, InterruptSourceType};

    const VIRTIO_INTR_VRING: u32 = 0x01;
    const VIRTIO_INTR_CONFIG: u32 = 0x02;

    #[test]
    fn create_virtio_null_notifier() {
        let notifier = NoopNotifier::new();

        notifier.notify().unwrap();
        assert!(notifier.notifier().is_none());
    }

    #[cfg(feature = "kvm-legacy-irq")]
    #[test]
    fn test_create_legacy_notifier() {
        let (_vmfd, irq_manager) = crate::kvm::tests::create_kvm_irq_manager();
        let group = irq_manager
            .create_group(InterruptSourceType::LegacyIrq, 0, 1)
            .unwrap();
        let status = Arc::new(InterruptStatusRegister32::new());
        assert_eq!(status.read(), 0);

        let notifer = LegacyNotifier::new(group.clone(), status.clone(), VIRTIO_INTR_CONFIG);
        notifer.notify().unwrap();
        assert!(notifer.notifier().is_some());
        assert_eq!(notifer.status_bits, VIRTIO_INTR_CONFIG);
        assert_eq!(status.read_and_clear(), VIRTIO_INTR_CONFIG);
        assert_eq!(status.read(), 0);

        let notifier = LegacyNotifier::new(group.clone(), status.clone(), VIRTIO_INTR_VRING);
        notifier.notify().unwrap();
        assert!(notifier.notifier().is_some());
        assert_eq!(status.read(), VIRTIO_INTR_VRING);
        status.clear_bits(VIRTIO_INTR_VRING);
        assert_eq!(status.read(), 0);
        let eventfd = notifier.notifier().unwrap();
        assert_eq!(eventfd.read().unwrap(), 2);

        let clone = clone_notifier(&notifier);
        assert_eq!(clone.as_any().type_id(), notifier.as_any().type_id());
    }

    #[cfg(feature = "kvm-msi-irq")]
    #[test]
    fn test_virtio_msi_notifier() {
        let (_vmfd, irq_manager) = crate::kvm::tests::create_kvm_irq_manager();
        let group = irq_manager
            .create_group(InterruptSourceType::MsiIrq, 0, 3)
            .unwrap();
        let notifier1 = MsiNotifier::new(group.clone(), 1);
        let notifier2 = MsiNotifier::new(group.clone(), 2);
        let notifier3 = MsiNotifier::new(group.clone(), 3);
        assert!(notifier1.notifier().is_some());
        assert!(notifier2.notifier().is_some());
        assert!(notifier3.notifier().is_none());

        notifier1.notify().unwrap();
        notifier1.notify().unwrap();
        notifier2.notify().unwrap();
        assert_eq!(notifier1.notifier().unwrap().read().unwrap(), 2);
        assert_eq!(notifier2.notifier().unwrap().read().unwrap(), 1);

        let clone = clone_notifier(&notifier1);
        assert_eq!(clone.as_any().type_id(), notifier1.as_any().type_id());
    }
}
