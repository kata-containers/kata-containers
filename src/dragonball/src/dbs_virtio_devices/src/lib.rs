// Copyright 2019-2020 Alibaba Cloud. All rights reserved.
// Portions Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Interfaces and implementations of virtio devices.
//!
//! Please refer to [Virtio Specification]
//! (http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-1090002)
//! for more information.

mod device;
pub use self::device::*;

mod notifier;
pub use self::notifier::*;

pub mod epoll_helper;

#[cfg(feature = "virtio-mmio")]
pub mod mmio;

#[cfg(feature = "virtio-vsock")]
pub mod vsock;

#[cfg(feature = "virtio-net")]
pub mod net;

#[cfg(feature = "virtio-blk")]
pub mod block;

#[cfg(feature = "virtio-fs")]
pub mod fs;

#[cfg(feature = "virtio-mem")]
pub mod mem;

#[cfg(feature = "virtio-balloon")]
pub mod balloon;

use std::io::Error as IOError;

use virtio_queue::Error as VqError;
use vm_memory::{GuestAddress, GuestAddressSpace, GuestMemoryError};

pub trait DbsGuestAddressSpace: GuestAddressSpace + 'static + Clone + Send + Sync {}

impl<T> DbsGuestAddressSpace for T where T: GuestAddressSpace + 'static + Clone + Send + Sync {}

/// Version of virtio specifications supported by PCI virtio devices.
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VirtioVersion {
    /// Unknown/non-virtio VFIO device.
    VIRTIO_VERSION_UNKNOWN,
    /// Virtio specification 0.95(Legacy).
    VIRTIO_VERSION_0_95,
    /// Virtio specification 1.0/1.1.
    VIRTIO_VERSION_1_X,
}

/// Page size for legacy PCI virtio devices. Assume it's 4K.
pub const VIRTIO_LEGACY_PAGE_SIZE: u32 = 0x1000;

/// Initial state after device initialization/reset.
pub const DEVICE_INIT: u32 = 0x0;
/// Indicates that the guest OS has found the device and recognized it as a valid virtio device.
pub const DEVICE_ACKNOWLEDGE: u32 = 0x01;
/// Indicates that the guest OS knows how to drive the device.
pub const DEVICE_DRIVER: u32 = 0x02;
/// Indicates that the driver is set up and ready to drive the device.
pub const DEVICE_DRIVER_OK: u32 = 0x04;
/// Indicates that the driver has acknowledged all the features it understands, and feature
/// negotiation is complete.
pub const DEVICE_FEATURES_OK: u32 = 0x08;
/// Indicates that the device has experienced an error from which it can’t recover.
pub const DEVICE_NEEDS_RESET: u32 = 0x40;
/// Indicates that something went wrong in the guest, and it has given up on the device.
/// This could be an internal error, or the driver didn’t like the device for some reason, or even
/// a fatal error during device operation.
pub const DEVICE_FAILED: u32 = 0x80;

/// Virtio network card device.
pub const TYPE_NET: u32 = 1;
/// Virtio block device.
pub const TYPE_BLOCK: u32 = 2;
/// Virtio-rng device.
pub const TYPE_RNG: u32 = 4;
/// Virtio balloon device.
pub const TYPE_BALLOON: u32 = 5;
/// Virtio vsock device.
pub const TYPE_VSOCK: u32 = 19;
/// Virtio mem device.
pub const TYPE_MEM: u32 = 24;
/// Virtio-fs virtual device.
pub const TYPE_VIRTIO_FS: u32 = 26;
/// Virtio-pmem device.
pub const TYPE_PMEM: u32 = 27;

// Interrupt status flags for legacy interrupts. It happens to be the same for both PCI and MMIO
// virtio devices.
/// Data available in used queue.
pub const VIRTIO_INTR_VRING: u32 = 0x01;
/// Device configuration changed.
pub const VIRTIO_INTR_CONFIG: u32 = 0x02;

/// Error code for VirtioDevice::activate().
#[derive(Debug, thiserror::Error)]
pub enum ActivateError {
    #[error("Invalid param.")]
    InvalidParam,
    #[error("Internal error.")]
    InternalError,
    #[error("Invalid queue config.")]
    InvalidQueueConfig,
    #[error("IO: {0}.")]
    IOError(#[from] IOError),
}

/// Error code for VirtioDevice::read_config()/write_config().
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum ConfigError {
    #[error("Invalid offset: {0}.")]
    InvalidOffset(u64),
    #[error("Offset({0}) plus data length ({0}) overflow.")]
    PlusOverflow(u64, u64),
    #[error("Invalid offset plus data length: {0}.")]
    InvalidOffsetPlusDataLen(u64),
}

/// Specialized std::result::Result for VirtioDevice::activate().
pub type ActivateResult = std::result::Result<(), ActivateError>;
/// Specialized std::result::Result for VirtioDevice::read_config()/write_config().
pub type ConfigResult = std::result::Result<(), ConfigError>;

/// Error for virtio devices to handle requests from guests.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Guest gave us too few descriptors in a descriptor chain.
    #[error("not enough descriptors for request.")]
    DescriptorChainTooShort,
    /// Guest gave us a descriptor that was too short to use.
    #[error("descriptor length too small.")]
    DescriptorLengthTooSmall,
    /// Guest gave us a descriptor that was too big to use.
    #[error("descriptor length too big.")]
    DescriptorLengthTooBig,
    /// Guest gave us a write only descriptor that protocol says to read from.
    #[error("unexpected write only descriptor.")]
    UnexpectedWriteOnlyDescriptor,
    /// Guest gave us a read only descriptor that protocol says to write to.
    #[error("unexpected read only descriptor.")]
    UnexpectedReadOnlyDescriptor,
    /// Invalid input parameter or status.
    #[error("invalid input parameter or status.")]
    InvalidInput,
    /// The requested operation would cause a seek beyond disk end.
    #[error("invalid offset.")]
    InvalidOffset,
    /// Internal unspecific error
    #[error("internal unspecific error.")]
    InternalError,
    /// Device resource doesn't match what requested
    #[error("invalid resource.")]
    InvalidResource,
    /// Generic IO error
    #[error("IO: {0}.")]
    IOError(#[from] IOError),
    /// Error from virtio_queue
    #[error("virtio queue error: {0}")]
    VirtioQueueError(#[from] VqError),
    /// Error from Device activate.
    #[error("Device activate error: {0}")]
    ActivateError(#[from] ActivateError),
    /// Error from Interrupt.
    #[error("Interrupt error: {0}")]
    InterruptError(IOError),
    /// Guest gave us bad memory addresses.
    #[error("failed to access guest memory. {0}")]
    GuestMemory(GuestMemoryError),
    /// Guest gave us an invalid guest memory address.
    #[error("invalid guest memory address. {0:?}")]
    InvalidGuestAddress(GuestAddress),
    /// Failed creating a new MmapRegion instance.
    #[error("new mmap region failed: {0}")]
    NewMmapRegion(vm_memory::mmap::MmapRegionError),
    /// Failed setting kvm user memory region.
    #[error("set user memory region failed: {0}")]
    SetUserMemoryRegion(kvm_ioctls::Error),
    /// Inserting mmap region failed.
    #[error("inserting mmap region failed: {0}")]
    InsertMmap(vm_memory::mmap::Error),
    /// Failed to set madvise on guest memory region.
    #[error("failed to set madvice() on guest memory region")]
    Madvise(#[source] nix::Error),

    #[cfg(feature = "virtio-vsock")]
    #[error("virtio-vsock error: {0}")]
    VirtioVsockError(#[from] self::vsock::VsockError),

    #[cfg(feature = "virtio-net")]
    #[error("Virtio-net error: {0}")]
    VirtioNetError(#[from] crate::net::NetError),

    #[cfg(feature = "virtio-fs")]
    /// Error from Virtio fs.
    #[error("virtio-fs error: {0}")]
    VirtioFs(fs::Error),

    #[cfg(feature = "virtio-mem")]
    #[error("Virtio-mem error: {0}")]
    VirtioMemError(#[from] mem::MemError),

    #[cfg(feature = "virtio-balloon")]
    #[error("Virtio-balloon error: {0}")]
    VirtioBalloonError(#[from] balloon::BalloonError),
}

/// Specialized std::result::Result for Virtio device operations.
pub type Result<T> = std::result::Result<T, Error>;

#[allow(unused_macros)]
macro_rules! warn_or_panic {
    ($($arg:tt)*) => {
        if cfg!(test) {
            panic!($($arg)*)
        } else {
            log::warn!($($arg)*)
        }
    }
}
#[allow(unused_imports)]
pub(crate) use warn_or_panic;

#[cfg(test)]
pub mod tests {
    use std::marker::PhantomData;
    use std::mem;
    use std::sync::Arc;

    use dbs_interrupt::KvmIrqManager;
    use kvm_ioctls::{Kvm, VmFd};
    use virtio_queue::{QueueSync, QueueT};
    use vm_memory::{
        Address, GuestAddress, GuestMemory, GuestMemoryMmap, GuestUsize, VolatileMemory,
        VolatileRef, VolatileSlice,
    };

    pub const VIRTQ_DESC_F_NEXT: u16 = 0x1;
    pub const VIRTQ_DESC_F_WRITE: u16 = 0x2;

    pub fn create_vm_and_irq_manager() -> (Arc<VmFd>, Arc<KvmIrqManager>) {
        let kvm = Kvm::new().unwrap();
        let vmfd = Arc::new(kvm.create_vm().unwrap());
        assert!(vmfd.create_irq_chip().is_ok());
        let irq_manager = Arc::new(KvmIrqManager::new(vmfd.clone()));
        assert!(irq_manager.initialize().is_ok());

        (vmfd, irq_manager)
    }

    // Represents a virtio descriptor in guest memory.
    pub struct VirtqDesc<'a> {
        pub desc: VolatileSlice<'a>,
    }

    #[repr(C)]
    // Used to calculate field offset
    pub struct DescriptorTmp {
        addr: vm_memory::Le64,
        len: vm_memory::Le32,
        flags: vm_memory::Le16,
        next: vm_memory::Le16,
    }

    macro_rules! offset_of {
        ($ty:ty, $field:ident) => {
            unsafe {
                let base = std::mem::MaybeUninit::<$ty>::uninit();
                let base_ptr = base.as_ptr();
                let c = std::ptr::addr_of!((*base_ptr).$field);
                (c as usize) - (base_ptr as usize)
            }
        };
    }

    impl<'a> VirtqDesc<'a> {
        fn new(dtable: &'a VolatileSlice<'a>, i: u16) -> Self {
            let desc = dtable
                .get_slice((i as usize) * Self::dtable_len(1), Self::dtable_len(1))
                .unwrap();
            VirtqDesc { desc }
        }

        pub fn addr(&self) -> VolatileRef<u64> {
            self.desc.get_ref(offset_of!(DescriptorTmp, addr)).unwrap()
        }

        pub fn len(&self) -> VolatileRef<u32> {
            self.desc.get_ref(offset_of!(DescriptorTmp, len)).unwrap()
        }

        pub fn flags(&self) -> VolatileRef<u16> {
            self.desc.get_ref(offset_of!(DescriptorTmp, flags)).unwrap()
        }

        pub fn next(&self) -> VolatileRef<u16> {
            self.desc.get_ref(offset_of!(DescriptorTmp, next)).unwrap()
        }

        pub fn set(&self, addr: u64, len: u32, flags: u16, next: u16) {
            self.addr().store(addr);
            self.len().store(len);
            self.flags().store(flags);
            self.next().store(next);
        }

        fn dtable_len(nelem: u16) -> usize {
            16 * nelem as usize
        }
    }

    // Represents a virtio queue ring. The only difference between the used and available rings,
    // is the ring element type.
    pub struct VirtqRing<'a, T> {
        pub ring: VolatileSlice<'a>,
        pub start: GuestAddress,
        pub qsize: u16,
        _marker: PhantomData<*const T>,
    }

    impl<'a, T> VirtqRing<'a, T>
    where
        T: vm_memory::ByteValued,
    {
        fn new(
            start: GuestAddress,
            mem: &'a GuestMemoryMmap,
            qsize: u16,
            alignment: GuestUsize,
        ) -> Self {
            assert_eq!(start.0 & (alignment - 1), 0);

            let (region, addr) = mem.to_region_addr(start).unwrap();
            let size = Self::ring_len(qsize);
            let ring = region.get_slice(addr.0 as usize, size).unwrap();

            let result = VirtqRing {
                ring,
                start,
                qsize,
                _marker: PhantomData,
            };

            result.flags().store(0);
            result.idx().store(0);
            result.event().store(0);
            result
        }

        pub fn start(&self) -> GuestAddress {
            self.start
        }

        pub fn end(&self) -> GuestAddress {
            self.start.unchecked_add(self.ring.len() as GuestUsize)
        }

        pub fn flags(&self) -> VolatileRef<u16> {
            self.ring.get_ref(0).unwrap()
        }

        pub fn idx(&self) -> VolatileRef<u16> {
            self.ring.get_ref(2).unwrap()
        }

        fn ring_offset(i: u16) -> usize {
            4 + mem::size_of::<T>() * (i as usize)
        }

        pub fn ring(&self, i: u16) -> VolatileRef<T> {
            assert!(i < self.qsize);
            self.ring.get_ref(Self::ring_offset(i)).unwrap()
        }

        pub fn event(&self) -> VolatileRef<u16> {
            self.ring.get_ref(Self::ring_offset(self.qsize)).unwrap()
        }

        fn ring_len(qsize: u16) -> usize {
            Self::ring_offset(qsize) + 2
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct VirtqUsedElem {
        pub id: u32,
        pub len: u32,
    }

    unsafe impl vm_memory::ByteValued for VirtqUsedElem {}

    pub type VirtqAvail<'a> = VirtqRing<'a, u16>;
    pub type VirtqUsed<'a> = VirtqRing<'a, VirtqUsedElem>;

    trait GuestAddressExt {
        fn align_up(&self, x: GuestUsize) -> GuestAddress;
    }
    impl GuestAddressExt for GuestAddress {
        fn align_up(&self, x: GuestUsize) -> GuestAddress {
            Self((self.0 + (x - 1)) & !(x - 1))
        }
    }

    pub struct VirtQueue<'a> {
        pub start: GuestAddress,
        pub dtable: VolatileSlice<'a>,
        pub avail: VirtqAvail<'a>,
        pub used: VirtqUsed<'a>,
    }

    impl<'a> VirtQueue<'a> {
        // We try to make sure things are aligned properly :-s
        pub fn new(start: GuestAddress, mem: &'a GuestMemoryMmap, qsize: u16) -> Self {
            // power of 2?
            assert!(qsize > 0 && qsize & (qsize - 1) == 0);

            let (region, addr) = mem.to_region_addr(start).unwrap();
            let dtable = region
                .get_slice(addr.0 as usize, VirtqDesc::dtable_len(qsize))
                .unwrap();

            const AVAIL_ALIGN: GuestUsize = 2;

            let avail_addr = start
                .unchecked_add(VirtqDesc::dtable_len(qsize) as GuestUsize)
                .align_up(AVAIL_ALIGN);
            let avail = VirtqAvail::new(avail_addr, mem, qsize, AVAIL_ALIGN);

            const USED_ALIGN: GuestUsize = 4;

            let used_addr = avail.end().align_up(USED_ALIGN);
            let used = VirtqUsed::new(used_addr, mem, qsize, USED_ALIGN);

            VirtQueue {
                start,
                dtable,
                avail,
                used,
            }
        }

        fn size(&self) -> u16 {
            (self.dtable.len() / VirtqDesc::dtable_len(1)) as u16
        }

        pub fn dtable(&self, i: u16) -> VirtqDesc {
            VirtqDesc::new(&self.dtable, i)
        }

        fn dtable_start(&self) -> GuestAddress {
            self.start
        }

        fn avail_start(&self) -> GuestAddress {
            self.avail.start()
        }

        fn used_start(&self) -> GuestAddress {
            self.used.start()
        }

        // Creates a new QueueSync, using the underlying memory regions represented by the VirtQueue.
        pub fn create_queue(&self) -> QueueSync {
            let mut q = QueueSync::new(self.size()).unwrap();

            q.set_size(self.size());
            q.set_ready(true);
            let _ = q.lock().try_set_desc_table_address(self.dtable_start());
            let _ = q.lock().try_set_avail_ring_address(self.avail_start());
            let _ = q.lock().try_set_used_ring_address(self.used_start());

            q
        }

        pub fn start(&self) -> GuestAddress {
            self.dtable_start()
        }

        pub fn end(&self) -> GuestAddress {
            self.used.end()
        }
    }
}
