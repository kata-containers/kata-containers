// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::io::{Read, Write};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use vm_memory::bitmap::{Bitmap, BS};
use vm_memory::guest_memory::GuestMemoryIterator;
use vm_memory::mmap::{Error, NewBitmap};
use vm_memory::{
    guest_memory, AtomicAccess, Bytes, FileOffset, GuestAddress, GuestMemory, GuestMemoryRegion,
    GuestRegionMmap, GuestUsize, MemoryRegionAddress, VolatileSlice,
};

use crate::GuestRegionRaw;

/// An adapter for different concrete implementations of `GuestMemoryRegion`.
#[derive(Debug)]
pub enum GuestRegionHybrid<B = ()> {
    /// Region of type `GuestRegionMmap`.
    Mmap(GuestRegionMmap<B>),
    /// Region of type `GuestRegionRaw`.
    Raw(GuestRegionRaw<B>),
}

impl<B: Bitmap> GuestRegionHybrid<B> {
    /// Create a `GuestRegionHybrid` object from `GuestRegionMmap` object.
    pub fn from_mmap_region(region: GuestRegionMmap<B>) -> Self {
        GuestRegionHybrid::Mmap(region)
    }

    /// Create a `GuestRegionHybrid` object from `GuestRegionRaw` object.
    pub fn from_raw_region(region: GuestRegionRaw<B>) -> Self {
        GuestRegionHybrid::Raw(region)
    }
}

impl<B: Bitmap> Bytes<MemoryRegionAddress> for GuestRegionHybrid<B> {
    type E = guest_memory::Error;

    fn write(&self, buf: &[u8], addr: MemoryRegionAddress) -> guest_memory::Result<usize> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.write(buf, addr),
            GuestRegionHybrid::Raw(region) => region.write(buf, addr),
        }
    }

    fn read(&self, buf: &mut [u8], addr: MemoryRegionAddress) -> guest_memory::Result<usize> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.read(buf, addr),
            GuestRegionHybrid::Raw(region) => region.read(buf, addr),
        }
    }

    fn write_slice(&self, buf: &[u8], addr: MemoryRegionAddress) -> guest_memory::Result<()> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.write_slice(buf, addr),
            GuestRegionHybrid::Raw(region) => region.write_slice(buf, addr),
        }
    }

    fn read_slice(&self, buf: &mut [u8], addr: MemoryRegionAddress) -> guest_memory::Result<()> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.read_slice(buf, addr),
            GuestRegionHybrid::Raw(region) => region.read_slice(buf, addr),
        }
    }

    fn read_from<F>(
        &self,
        addr: MemoryRegionAddress,
        src: &mut F,
        count: usize,
    ) -> guest_memory::Result<usize>
    where
        F: Read,
    {
        match self {
            GuestRegionHybrid::Mmap(region) => region.read_from(addr, src, count),
            GuestRegionHybrid::Raw(region) => region.read_from(addr, src, count),
        }
    }

    fn read_exact_from<F>(
        &self,
        addr: MemoryRegionAddress,
        src: &mut F,
        count: usize,
    ) -> guest_memory::Result<()>
    where
        F: Read,
    {
        match self {
            GuestRegionHybrid::Mmap(region) => region.read_exact_from(addr, src, count),
            GuestRegionHybrid::Raw(region) => region.read_exact_from(addr, src, count),
        }
    }

    fn write_to<F>(
        &self,
        addr: MemoryRegionAddress,
        dst: &mut F,
        count: usize,
    ) -> guest_memory::Result<usize>
    where
        F: Write,
    {
        match self {
            GuestRegionHybrid::Mmap(region) => region.write_to(addr, dst, count),
            GuestRegionHybrid::Raw(region) => region.write_to(addr, dst, count),
        }
    }

    fn write_all_to<F>(
        &self,
        addr: MemoryRegionAddress,
        dst: &mut F,
        count: usize,
    ) -> guest_memory::Result<()>
    where
        F: Write,
    {
        match self {
            GuestRegionHybrid::Mmap(region) => region.write_all_to(addr, dst, count),
            GuestRegionHybrid::Raw(region) => region.write_all_to(addr, dst, count),
        }
    }

    fn store<T: AtomicAccess>(
        &self,
        val: T,
        addr: MemoryRegionAddress,
        order: Ordering,
    ) -> guest_memory::Result<()> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.store(val, addr, order),
            GuestRegionHybrid::Raw(region) => region.store(val, addr, order),
        }
    }

    fn load<T: AtomicAccess>(
        &self,
        addr: MemoryRegionAddress,
        order: Ordering,
    ) -> guest_memory::Result<T> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.load(addr, order),
            GuestRegionHybrid::Raw(region) => region.load(addr, order),
        }
    }
}

impl<B: Bitmap> GuestMemoryRegion for GuestRegionHybrid<B> {
    type B = B;

    fn len(&self) -> GuestUsize {
        match self {
            GuestRegionHybrid::Mmap(region) => region.len(),
            GuestRegionHybrid::Raw(region) => region.len(),
        }
    }

    fn start_addr(&self) -> GuestAddress {
        match self {
            GuestRegionHybrid::Mmap(region) => region.start_addr(),
            GuestRegionHybrid::Raw(region) => region.start_addr(),
        }
    }

    fn bitmap(&self) -> &Self::B {
        match self {
            GuestRegionHybrid::Mmap(region) => region.bitmap(),
            GuestRegionHybrid::Raw(region) => region.bitmap(),
        }
    }

    fn get_host_address(&self, addr: MemoryRegionAddress) -> guest_memory::Result<*mut u8> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.get_host_address(addr),
            GuestRegionHybrid::Raw(region) => region.get_host_address(addr),
        }
    }

    fn file_offset(&self) -> Option<&FileOffset> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.file_offset(),
            GuestRegionHybrid::Raw(region) => region.file_offset(),
        }
    }

    unsafe fn as_slice(&self) -> Option<&[u8]> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.as_slice(),
            GuestRegionHybrid::Raw(region) => region.as_slice(),
        }
    }

    unsafe fn as_mut_slice(&self) -> Option<&mut [u8]> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.as_mut_slice(),
            GuestRegionHybrid::Raw(region) => region.as_mut_slice(),
        }
    }

    fn get_slice(
        &self,
        offset: MemoryRegionAddress,
        count: usize,
    ) -> guest_memory::Result<VolatileSlice<BS<B>>> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.get_slice(offset, count),
            GuestRegionHybrid::Raw(region) => region.get_slice(offset, count),
        }
    }

    #[cfg(target_os = "linux")]
    fn is_hugetlbfs(&self) -> Option<bool> {
        match self {
            GuestRegionHybrid::Mmap(region) => region.is_hugetlbfs(),
            GuestRegionHybrid::Raw(region) => region.is_hugetlbfs(),
        }
    }
}

/// [`GuestMemory`](trait.GuestMemory.html) implementation that manage hybrid types of guest memory
/// regions.
///
/// Represents the entire physical memory of the guest by tracking all its memory regions.
/// Each region is an instance of `GuestRegionHybrid`.
#[derive(Clone, Debug, Default)]
pub struct GuestMemoryHybrid<B = ()> {
    pub(crate) regions: Vec<Arc<GuestRegionHybrid<B>>>,
}

impl<B: NewBitmap> GuestMemoryHybrid<B> {
    /// Creates an empty `GuestMemoryHybrid` instance.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<B: Bitmap> GuestMemoryHybrid<B> {
    /// Creates a new `GuestMemoryHybrid` from a vector of regions.
    ///
    /// # Arguments
    ///
    /// * `regions` - The vector of regions.
    ///               The regions shouldn't overlap and they should be sorted
    ///               by the starting address.
    pub fn from_regions(mut regions: Vec<GuestRegionHybrid<B>>) -> Result<Self, Error> {
        Self::from_arc_regions(regions.drain(..).map(Arc::new).collect())
    }

    /// Creates a new `GuestMemoryHybrid` from a vector of Arc regions.
    ///
    /// Similar to the constructor `from_regions()` as it returns a
    /// `GuestMemoryHybrid`. The need for this constructor is to provide a way for
    /// consumer of this API to create a new `GuestMemoryHybrid` based on existing
    /// regions coming from an existing `GuestMemoryHybrid` instance.
    ///
    /// # Arguments
    ///
    /// * `regions` - The vector of `Arc` regions.
    ///               The regions shouldn't overlap and they should be sorted
    ///               by the starting address.
    pub fn from_arc_regions(regions: Vec<Arc<GuestRegionHybrid<B>>>) -> Result<Self, Error> {
        if regions.is_empty() {
            return Err(Error::NoMemoryRegion);
        }

        for window in regions.windows(2) {
            let prev = &window[0];
            let next = &window[1];

            if prev.start_addr() > next.start_addr() {
                return Err(Error::UnsortedMemoryRegions);
            }

            if prev.last_addr() >= next.start_addr() {
                return Err(Error::MemoryRegionOverlap);
            }
        }

        Ok(Self { regions })
    }

    /// Insert a region into the `GuestMemoryHybrid` object and return a new `GuestMemoryHybrid`.
    ///
    /// # Arguments
    /// * `region`: the memory region to insert into the guest memory object.
    pub fn insert_region(
        &self,
        region: Arc<GuestRegionHybrid<B>>,
    ) -> Result<GuestMemoryHybrid<B>, Error> {
        let mut regions = self.regions.clone();
        regions.push(region);
        regions.sort_by_key(|x| x.start_addr());

        Self::from_arc_regions(regions)
    }

    /// Remove a region into the `GuestMemoryHybrid` object and return a new `GuestMemoryHybrid`
    /// on success, together with the removed region.
    ///
    /// # Arguments
    /// * `base`: base address of the region to be removed
    /// * `size`: size of the region to be removed
    pub fn remove_region(
        &self,
        base: GuestAddress,
        size: GuestUsize,
    ) -> Result<(GuestMemoryHybrid<B>, Arc<GuestRegionHybrid<B>>), Error> {
        if let Ok(region_index) = self.regions.binary_search_by_key(&base, |x| x.start_addr()) {
            if self.regions.get(region_index).unwrap().len() as GuestUsize == size {
                let mut regions = self.regions.clone();
                let region = regions.remove(region_index);
                return Ok((Self { regions }, region));
            }
        }

        Err(Error::InvalidGuestRegion)
    }
}

/// An iterator over the elements of `GuestMemoryHybrid`.
///
/// This struct is created by `GuestMemory::iter()`. See its documentation for more.
pub struct Iter<'a, B>(std::slice::Iter<'a, Arc<GuestRegionHybrid<B>>>);

impl<'a, B> Iterator for Iter<'a, B> {
    type Item = &'a GuestRegionHybrid<B>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(AsRef::as_ref)
    }
}

impl<'a, B: 'a> GuestMemoryIterator<'a, GuestRegionHybrid<B>> for GuestMemoryHybrid<B> {
    type Iter = Iter<'a, B>;
}

impl<B: Bitmap + 'static> GuestMemory for GuestMemoryHybrid<B> {
    type R = GuestRegionHybrid<B>;

    type I = Self;

    fn num_regions(&self) -> usize {
        self.regions.len()
    }

    fn find_region(&self, addr: GuestAddress) -> Option<&GuestRegionHybrid<B>> {
        let index = match self.regions.binary_search_by_key(&addr, |x| x.start_addr()) {
            Ok(x) => Some(x),
            // Within the closest region with starting address < addr
            Err(x) if (x > 0 && addr <= self.regions[x - 1].last_addr()) => Some(x - 1),
            _ => None,
        };
        index.map(|x| self.regions[x].as_ref())
    }

    fn iter(&self) -> Iter<B> {
        Iter(self.regions.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Seek;
    use vm_memory::{GuestMemoryError, MmapRegion};
    use vmm_sys_util::tempfile::TempFile;

    #[test]
    fn test_region_new() {
        let start_addr = GuestAddress(0x0);

        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x400).unwrap(), start_addr).unwrap();
        let guest_region = GuestRegionHybrid::from_mmap_region(mmap_reg);

        assert_eq!(guest_region.start_addr(), start_addr);
        assert_eq!(guest_region.len(), 0x400);

        let mut buf = [0u8; 1024];
        let raw_region =
            unsafe { GuestRegionRaw::<()>::new(start_addr, &mut buf as *mut _, 0x800) };
        let guest_region = GuestRegionHybrid::from_raw_region(raw_region);

        assert_eq!(guest_region.start_addr(), start_addr);
        assert_eq!(guest_region.len(), 0x800);
    }

    #[test]
    fn test_write_and_read_on_mmap_region() {
        let start_addr = GuestAddress(0x0);
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x800).unwrap(), start_addr).unwrap();
        let guest_region = GuestRegionHybrid::from_mmap_region(mmap_reg);
        let buf_to_write = [0xF0u8; 0x400];
        let write_addr = MemoryRegionAddress(0x400);

        // Normal case.
        let number_of_bytes_write = guest_region.write(&buf_to_write, write_addr).unwrap();
        assert_eq!(number_of_bytes_write, 0x400);
        let mut buf_read = [0u8; 0x400];
        let number_of_bytes_read = guest_region.read(&mut buf_read, write_addr).unwrap();
        assert_eq!(number_of_bytes_read, 0x400);
        assert_eq!(buf_read, [0xF0u8; 0x400]);

        // Error invalid backend address case in write().
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_region
                .write(&buf_to_write, invalid_addr)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error invalid backend address case in read().
        assert!(matches!(
            guest_region
                .read(&mut buf_read, invalid_addr)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));
    }

    #[test]
    fn test_write_and_read_on_raw_region() {
        let start_addr = GuestAddress(0x0);
        let mut buf_of_raw_region = [0u8; 0x800];
        let raw_region = unsafe {
            GuestRegionRaw::<()>::new(start_addr, &mut buf_of_raw_region as *mut _, 0x800)
        };
        let guest_region = GuestRegionHybrid::from_raw_region(raw_region);
        let buf_to_write = [0xF0u8; 0x400];
        let write_addr = MemoryRegionAddress(0x400);

        // Normal case.
        let number_of_bytes_write = guest_region.write(&buf_to_write, write_addr).unwrap();
        assert_eq!(number_of_bytes_write, 0x400);
        let mut buf_read = [0u8; 0x400];
        let number_of_bytes_read = guest_region.read(&mut buf_read, write_addr).unwrap();
        assert_eq!(number_of_bytes_read, 0x400);
        assert_eq!(buf_read, [0xF0u8; 0x400]);

        // Error invalid backend address case in write().
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_region
                .write(&buf_to_write, invalid_addr)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error invalid backend address case in read().
        assert!(matches!(
            guest_region
                .read(&mut buf_read, invalid_addr)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));
    }

    #[test]
    fn test_write_slice_and_read_slice_on_mmap_region() {
        let start_addr = GuestAddress(0x0);
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x800).unwrap(), start_addr).unwrap();
        let guest_region = GuestRegionHybrid::from_mmap_region(mmap_reg);
        let buf_to_write = [0xF0u8; 0x400];
        let write_addr = MemoryRegionAddress(0x400);

        // Normal case.
        guest_region.write_slice(&buf_to_write, write_addr).unwrap();
        let mut buf_read = [0x0u8; 0x400];
        guest_region.read_slice(&mut buf_read, write_addr).unwrap();
        assert_eq!(buf_read, [0xF0u8; 0x400]);

        // Error invalid backend address case in write_slice().
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_region
                .write_slice(&buf_to_write, invalid_addr)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error partial buffer case in write_slice().
        let insufficient_addr = MemoryRegionAddress(0x600);
        assert_eq!(
            format!(
                "{:?}",
                guest_region
                    .write_slice(&buf_to_write, insufficient_addr)
                    .err()
                    .unwrap()
            ),
            format!(
                "PartialBuffer {{ expected: {:?}, completed: {:?} }}",
                buf_to_write.len(),
                guest_region.len() as usize - 0x600_usize
            )
        );

        // Error invalid backend address case in write_slice().
        let invalid_addr = MemoryRegionAddress(0x900);
        let mut buf_read = [0x0u8; 0x400];
        assert!(matches!(
            guest_region
                .read_slice(&mut buf_read, invalid_addr)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error partial buffer case in write_slice().
        let insufficient_addr = MemoryRegionAddress(0x600);
        let mut buf_read = [0x0u8; 0x400];
        assert_eq!(
            format!(
                "{:?}",
                guest_region
                    .read_slice(&mut buf_read, insufficient_addr)
                    .err()
                    .unwrap()
            ),
            format!(
                "PartialBuffer {{ expected: {:?}, completed: {:?} }}",
                buf_to_write.len(),
                guest_region.len() as usize - 0x600_usize
            )
        );
        assert_eq!(
            {
                let mut buf = [0x0u8; 0x400];
                for cell in buf.iter_mut().take(0x200) {
                    *cell = 0xF0;
                }
                buf
            },
            buf_read
        );
    }

    #[test]
    fn test_write_and_read_slice_on_raw_region() {
        let start_addr = GuestAddress(0x0);
        let mut buf_of_raw_region = [0u8; 0x800];
        let raw_region = unsafe {
            GuestRegionRaw::<()>::new(start_addr, &mut buf_of_raw_region as *mut _, 0x800)
        };
        let guest_region = GuestRegionHybrid::from_raw_region(raw_region);
        let buf_to_write = [0xF0u8; 0x400];
        let write_addr = MemoryRegionAddress(0x400);

        // Normal case.
        guest_region.write_slice(&buf_to_write, write_addr).unwrap();
        let mut buf_read = [0x0u8; 0x400];
        guest_region.read_slice(&mut buf_read, write_addr).unwrap();
        assert_eq!(buf_read, [0xF0u8; 0x400]);

        // Error invalid backend address case in write_slice().
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_region
                .write_slice(&buf_to_write, invalid_addr)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error partial buffer case in write_slice().
        let insufficient_addr = MemoryRegionAddress(0x600);
        assert_eq!(
            format!(
                "{:?}",
                guest_region
                    .write_slice(&buf_to_write, insufficient_addr)
                    .err()
                    .unwrap()
            ),
            format!(
                "PartialBuffer {{ expected: {:?}, completed: {:?} }}",
                buf_to_write.len(),
                guest_region.len() as usize - 0x600_usize
            )
        );

        // Error invalid backend address case in write_slice().
        let invalid_addr = MemoryRegionAddress(0x900);
        let mut buf_read = [0x0u8; 0x400];
        assert!(matches!(
            guest_region
                .read_slice(&mut buf_read, invalid_addr)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error partial buffer case in write_slice().
        let insufficient_addr = MemoryRegionAddress(0x600);
        let mut buf_read = [0x0u8; 0x400];
        assert_eq!(
            format!(
                "{:?}",
                guest_region
                    .read_slice(&mut buf_read, insufficient_addr)
                    .err()
                    .unwrap()
            ),
            format!(
                "PartialBuffer {{ expected: {:?}, completed: {:?} }}",
                buf_to_write.len(),
                guest_region.len() as usize - 0x600_usize
            )
        );
        assert_eq!(
            {
                let mut buf = [0x0u8; 0x400];
                for cell in buf.iter_mut().take(0x200) {
                    *cell = 0xF0;
                }
                buf
            },
            buf_read
        );
    }

    #[test]
    fn test_read_from_and_write_to_on_mmap_region() {
        let start_addr = GuestAddress(0x0);
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x800).unwrap(), start_addr).unwrap();
        let guest_region = GuestRegionHybrid::from_mmap_region(mmap_reg);
        let write_addr = MemoryRegionAddress(0x400);
        let original_content = b"hello world";
        let size_of_file = original_content.len();

        // Normal case.
        let mut file_to_write_mmap_region = TempFile::new().unwrap().into_file();
        file_to_write_mmap_region
            .set_len(size_of_file as u64)
            .unwrap();
        file_to_write_mmap_region
            .write_all(original_content)
            .unwrap();
        // Rewind file pointer after write operation.
        file_to_write_mmap_region.rewind().unwrap();
        guest_region
            .read_from(write_addr, &mut file_to_write_mmap_region, size_of_file)
            .unwrap();
        let mut file_read_from_mmap_region = TempFile::new().unwrap().into_file();
        file_read_from_mmap_region
            .set_len(size_of_file as u64)
            .unwrap();
        guest_region
            .write_all_to(write_addr, &mut file_read_from_mmap_region, size_of_file)
            .unwrap();
        // Rewind file pointer after write operation.
        file_read_from_mmap_region.rewind().unwrap();
        let mut content = String::new();
        file_read_from_mmap_region
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content.as_bytes(), original_content);
        assert_eq!(
            file_read_from_mmap_region.metadata().unwrap().len(),
            size_of_file as u64
        );

        // Error invalid backend address case in read_from() on mmap region.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_region
                .read_from(invalid_addr, &mut file_to_write_mmap_region, size_of_file)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error invalid backend address case in write_to() on mmap region.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_region
                .write_to(invalid_addr, &mut file_read_from_mmap_region, size_of_file)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));
    }

    #[test]
    fn test_read_from_and_write_to_on_raw_region() {
        let start_addr = GuestAddress(0x0);
        let mut buf_of_raw_region = [0u8; 0x800];
        let raw_region = unsafe {
            GuestRegionRaw::<()>::new(start_addr, &mut buf_of_raw_region as *mut _, 0x800)
        };
        let guest_region = GuestRegionHybrid::from_raw_region(raw_region);
        let write_addr = MemoryRegionAddress(0x400);
        let original_content = b"hello world";
        let size_of_file = original_content.len();

        // Normal case.
        let mut file_to_write_mmap_region = TempFile::new().unwrap().into_file();
        file_to_write_mmap_region
            .set_len(size_of_file as u64)
            .unwrap();
        file_to_write_mmap_region
            .write_all(original_content)
            .unwrap();
        // Rewind file pointer after write operation.
        file_to_write_mmap_region.rewind().unwrap();
        guest_region
            .read_from(write_addr, &mut file_to_write_mmap_region, size_of_file)
            .unwrap();
        let mut file_read_from_mmap_region = TempFile::new().unwrap().into_file();
        file_read_from_mmap_region
            .set_len(size_of_file as u64)
            .unwrap();
        guest_region
            .write_all_to(write_addr, &mut file_read_from_mmap_region, size_of_file)
            .unwrap();
        // Rewind file pointer after write operation.
        file_read_from_mmap_region.rewind().unwrap();
        let mut content = String::new();
        file_read_from_mmap_region
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content.as_bytes(), original_content);
        assert_eq!(
            file_read_from_mmap_region.metadata().unwrap().len(),
            size_of_file as u64
        );

        // Error invalid backend address case in read_from() on raw region.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_region
                .read_from(invalid_addr, &mut file_to_write_mmap_region, size_of_file)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error invalid backend address case in write_to() on raw region.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_region
                .write_to(invalid_addr, &mut file_read_from_mmap_region, size_of_file)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));
    }

    #[test]
    fn test_write_all_to_and_read_exact_from() {
        let start_addr = GuestAddress(0x0);
        let write_addr = MemoryRegionAddress(0x400);
        let original_content = b"hello world";
        let size_of_file = original_content.len();
        // Preset a GuestRegionHybrid from a mmap region
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x800).unwrap(), start_addr).unwrap();
        let guest_mmap_region = GuestRegionHybrid::from_mmap_region(mmap_reg);
        // Preset a GuestRegionHybrid from a raw region
        let mut buf_of_raw_region = [0u8; 0x800];
        let raw_region = unsafe {
            GuestRegionRaw::<()>::new(start_addr, &mut buf_of_raw_region as *mut _, 0x800)
        };
        let guest_raw_region = GuestRegionHybrid::from_raw_region(raw_region);

        // Normal case on mmap region.
        let mut file_to_write_mmap_region = TempFile::new().unwrap().into_file();
        file_to_write_mmap_region
            .set_len(size_of_file as u64)
            .unwrap();
        file_to_write_mmap_region
            .write_all(original_content)
            .unwrap();
        file_to_write_mmap_region.rewind().unwrap();
        guest_mmap_region
            .read_exact_from(write_addr, &mut file_to_write_mmap_region, size_of_file)
            .unwrap();
        let mut file_read_from_mmap_region = TempFile::new().unwrap().into_file();
        file_read_from_mmap_region
            .set_len(size_of_file as u64)
            .unwrap();
        guest_mmap_region
            .write_all_to(write_addr, &mut file_read_from_mmap_region, size_of_file)
            .unwrap();
        file_read_from_mmap_region.rewind().unwrap();
        let mut content = String::new();
        file_read_from_mmap_region
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content.as_bytes(), original_content);
        assert_eq!(
            file_read_from_mmap_region.metadata().unwrap().len(),
            size_of_file as u64
        );

        // Normal case on raw region.
        let mut file_to_write_raw_region = TempFile::new().unwrap().into_file();
        file_to_write_raw_region
            .set_len(size_of_file as u64)
            .unwrap();
        file_to_write_raw_region
            .write_all(original_content)
            .unwrap();
        file_to_write_raw_region.rewind().unwrap();
        guest_raw_region
            .read_exact_from(write_addr, &mut file_to_write_raw_region, size_of_file)
            .unwrap();
        let mut file_read_from_raw_region = TempFile::new().unwrap().into_file();
        file_read_from_raw_region
            .set_len(size_of_file as u64)
            .unwrap();
        guest_raw_region
            .write_all_to(write_addr, &mut file_read_from_raw_region, size_of_file)
            .unwrap();
        file_read_from_raw_region.rewind().unwrap();
        let mut content = String::new();
        file_read_from_raw_region
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content.as_bytes(), original_content);
        assert_eq!(
            file_read_from_raw_region.metadata().unwrap().len(),
            size_of_file as u64
        );

        // Error invalid backend address case in read_exact_from() on mmap region.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_mmap_region
                .read_exact_from(invalid_addr, &mut file_to_write_mmap_region, size_of_file)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error invalid backend address case in write_all_to() on mmap region.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_mmap_region
                .write_all_to(invalid_addr, &mut file_read_from_mmap_region, size_of_file)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error invalid backend address case in read_exact_from() on raw region.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_raw_region
                .read_exact_from(invalid_addr, &mut file_to_write_raw_region, size_of_file)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error invalid backend address case in write_all_to() on raw region.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_raw_region
                .write_all_to(invalid_addr, &mut file_read_from_raw_region, size_of_file)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));
    }

    #[test]
    fn test_store_and_load() {
        let test_val = 0xFF;
        let start_addr = GuestAddress(0x0);
        let write_addr = MemoryRegionAddress(0x400);
        // Preset a GuestRegionHybrid from a mmap region
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x800).unwrap(), start_addr).unwrap();
        let guest_mmap_region = GuestRegionHybrid::from_mmap_region(mmap_reg);
        // Preset a GuestRegionHybrid from a raw region
        let mut buf_of_raw_region = [0u8; 0x800];
        let raw_region = unsafe {
            GuestRegionRaw::<()>::new(start_addr, &mut buf_of_raw_region as *mut _, 0x800)
        };
        let guest_raw_region = GuestRegionHybrid::from_raw_region(raw_region);

        // Normal case.
        guest_mmap_region
            .store(test_val, write_addr, Ordering::Relaxed)
            .unwrap();
        let val_read_from_mmap_region: u64 = guest_mmap_region
            .load(write_addr, Ordering::Relaxed)
            .unwrap();
        assert_eq!(val_read_from_mmap_region, test_val);
        guest_raw_region
            .store(test_val, write_addr, Ordering::Relaxed)
            .unwrap();
        let val_read_from_raw_region: u64 = guest_raw_region
            .load(write_addr, Ordering::Relaxed)
            .unwrap();
        assert_eq!(val_read_from_raw_region, test_val);

        // Error invalid backend address case in store() on mmap region.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_mmap_region
                .store(test_val, invalid_addr, Ordering::Relaxed)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error invalid backend address case in store() on raw region.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_raw_region
                .store(test_val, invalid_addr, Ordering::Relaxed)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error invalid backend address case in laod() on mmap region.
        assert!(matches!(
            guest_mmap_region
                .load::<u64>(invalid_addr, Ordering::Relaxed)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));

        // Error invalid backend address case in laod() on raw region.
        assert!(matches!(
            guest_raw_region
                .load::<u64>(invalid_addr, Ordering::Relaxed)
                .err()
                .unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));
    }

    #[test]
    fn test_bitmap() {
        // TODO: #185 Need futher and detailed test on bitmap object.
        let start_addr = GuestAddress(0x0);
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x800).unwrap(), start_addr).unwrap();
        let guest_mmap_region = GuestRegionHybrid::from_mmap_region(mmap_reg);
        let mut buf_of_raw_region = [0u8; 0x800];
        let raw_region = unsafe {
            GuestRegionRaw::<()>::new(start_addr, &mut buf_of_raw_region as *mut _, 0x800)
        };
        let guest_raw_region = GuestRegionHybrid::from_raw_region(raw_region);

        assert_eq!(guest_mmap_region.bitmap(), guest_raw_region.bitmap());
    }

    #[test]
    fn test_get_host_address_on_mmap_region() {
        let start_addr = GuestAddress(0x0);
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x800).unwrap(), start_addr).unwrap();
        let guest_region = GuestRegionHybrid::from_mmap_region(mmap_reg);

        // Normal case.
        let addr_1 = guest_region
            .get_host_address(MemoryRegionAddress(0x0))
            .unwrap();
        let addr_2 = guest_region
            .get_host_address(MemoryRegionAddress(0x400))
            .unwrap();
        assert_eq!(addr_1 as u64 + 0x400, addr_2 as u64);

        // Error invalid backend address case.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_region.get_host_address(invalid_addr).err().unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));
    }

    #[test]
    fn test_get_host_address_on_raw_region() {
        let start_addr = GuestAddress(0x0);
        let mut buf_of_raw_region = [0u8; 0x800];
        let raw_region = unsafe {
            GuestRegionRaw::<()>::new(start_addr, &mut buf_of_raw_region as *mut _, 0x800)
        };
        let guest_region = GuestRegionHybrid::from_raw_region(raw_region);

        // Normal case.
        let addr_1 = guest_region
            .get_host_address(MemoryRegionAddress(0x0))
            .unwrap();
        let addr_2 = guest_region
            .get_host_address(MemoryRegionAddress(0x400))
            .unwrap();
        assert_eq!(addr_1 as u64 + 0x400, addr_2 as u64);

        // Error invalid backend address case.
        let invalid_addr = MemoryRegionAddress(0x900);
        assert!(matches!(
            guest_region.get_host_address(invalid_addr).err().unwrap(),
            GuestMemoryError::InvalidBackendAddress
        ));
    }

    // TODO: #186 The following function are not yet implemented:
    // - 'fn file_offset()'
    // - 'unsafe fn as_slice()'
    // - 'unsafe fn as_mut_slice()'
    // Tests of these functions will be needed when they are implemented.

    #[test]
    fn test_guest_memory_mmap_get_slice() {
        //Preset a GuestRegionHybrid from a mmap region
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x400).unwrap(), GuestAddress(0)).unwrap();
        let guest_mmap_region = GuestRegionHybrid::from_mmap_region(mmap_reg);

        // Normal case.
        let slice_addr = MemoryRegionAddress(0x100);
        let slice_size = 0x200;
        let slice = guest_mmap_region.get_slice(slice_addr, slice_size).unwrap();
        assert_eq!(slice.len(), slice_size);

        // Empty slice.
        let slice_addr = MemoryRegionAddress(0x200);
        let slice_size = 0x0;
        let slice = guest_mmap_region.get_slice(slice_addr, slice_size).unwrap();
        assert!(slice.is_empty());

        // Error case when slice_size is beyond the boundary.
        let slice_addr = MemoryRegionAddress(0x300);
        let slice_size = 0x200;
        assert!(guest_mmap_region.get_slice(slice_addr, slice_size).is_err());
    }

    #[test]
    fn test_from_regions_on_guest_memory_hybrid() {
        // Normal case.
        let mut regions = Vec::<GuestRegionHybrid<()>>::new();
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x100).unwrap(), GuestAddress(0x100))
                .unwrap();
        regions.push(GuestRegionHybrid::Mmap(mmap_reg));
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x100).unwrap(), GuestAddress(0x200))
                .unwrap();
        regions.push(GuestRegionHybrid::Mmap(mmap_reg));
        let guest_region = GuestMemoryHybrid::<()>::from_regions(regions).unwrap();
        assert_eq!(guest_region.regions[0].start_addr(), GuestAddress(0x100));
        assert_eq!(guest_region.regions[1].start_addr(), GuestAddress(0x200));

        // Error unsorted region case.
        let mut regions = Vec::<GuestRegionHybrid<()>>::new();
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x400).unwrap(), GuestAddress(0x200))
                .unwrap();
        regions.push(GuestRegionHybrid::Mmap(mmap_reg));
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x400).unwrap(), GuestAddress(0x100))
                .unwrap();
        regions.push(GuestRegionHybrid::Mmap(mmap_reg));
        let guest_region = GuestMemoryHybrid::<()>::from_regions(regions);
        assert!(matches!(
            guest_region.err().unwrap(),
            Error::UnsortedMemoryRegions
        ));

        // Error no memory region case.
        let regions = Vec::<GuestRegionHybrid<()>>::new();
        let guest_region = GuestMemoryHybrid::<()>::from_regions(regions);
        assert!(matches!(guest_region.err().unwrap(), Error::NoMemoryRegion));
    }

    #[test]
    fn test_iterator_on_guest_region_hybrid() {
        let mut regions = Vec::<GuestRegionHybrid<()>>::new();
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x100).unwrap(), GuestAddress(0x100))
                .unwrap();
        regions.push(GuestRegionHybrid::Mmap(mmap_reg));
        let mmap_reg =
            GuestRegionMmap::new(MmapRegion::<()>::new(0x100).unwrap(), GuestAddress(0x200))
                .unwrap();
        regions.push(GuestRegionHybrid::Mmap(mmap_reg));
        let guest_region = GuestMemoryHybrid::<()>::from_regions(regions).unwrap();
        let mut region = guest_region.iter();

        assert_eq!(region.next().unwrap().start_addr(), GuestAddress(0x100));
        assert_eq!(region.next().unwrap().start_addr(), GuestAddress(0x200));
    }
}
