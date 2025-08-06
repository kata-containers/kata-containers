// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::io::{Read, Write};
use std::sync::atomic::Ordering;

use vm_memory::bitmap::{Bitmap, BS};
use vm_memory::mmap::NewBitmap;
use vm_memory::volatile_memory::compute_offset;
use vm_memory::{
    guest_memory, volatile_memory, Address, AtomicAccess, Bytes, FileOffset, GuestAddress,
    GuestMemoryRegion, GuestUsize, MemoryRegionAddress, VolatileSlice,
};

/// Guest memory region for virtio-fs DAX window.
#[derive(Debug)]
pub struct GuestRegionRaw<B = ()> {
    guest_base: GuestAddress,
    addr: *mut u8,
    size: usize,
    bitmap: B,
}

impl<B: NewBitmap> GuestRegionRaw<B> {
    /// Create a `GuestRegionRaw` object from raw pointer.
    ///
    /// # Safety
    /// Caller needs to ensure `addr` and `size` are valid with static lifetime.
    pub unsafe fn new(guest_base: GuestAddress, addr: *mut u8, size: usize) -> Self {
        let bitmap = B::with_len(size);

        GuestRegionRaw {
            guest_base,
            addr,
            size,
            bitmap,
        }
    }
}

impl<B: Bitmap> Bytes<MemoryRegionAddress> for GuestRegionRaw<B> {
    type E = guest_memory::Error;

    fn write(&self, buf: &[u8], addr: MemoryRegionAddress) -> guest_memory::Result<usize> {
        let maddr = addr.raw_value() as usize;
        self.as_volatile_slice()
            .unwrap()
            .write(buf, maddr)
            .map_err(Into::into)
    }

    fn read(&self, buf: &mut [u8], addr: MemoryRegionAddress) -> guest_memory::Result<usize> {
        let maddr = addr.raw_value() as usize;
        self.as_volatile_slice()
            .unwrap()
            .read(buf, maddr)
            .map_err(Into::into)
    }

    fn write_slice(&self, buf: &[u8], addr: MemoryRegionAddress) -> guest_memory::Result<()> {
        let maddr = addr.raw_value() as usize;
        self.as_volatile_slice()
            .unwrap()
            .write_slice(buf, maddr)
            .map_err(Into::into)
    }

    fn read_slice(&self, buf: &mut [u8], addr: MemoryRegionAddress) -> guest_memory::Result<()> {
        let maddr = addr.raw_value() as usize;
        self.as_volatile_slice()
            .unwrap()
            .read_slice(buf, maddr)
            .map_err(Into::into)
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
        let maddr = addr.raw_value() as usize;
        self.as_volatile_slice()
            .unwrap()
            .read_from::<F>(maddr, src, count)
            .map_err(Into::into)
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
        let maddr = addr.raw_value() as usize;
        self.as_volatile_slice()
            .unwrap()
            .read_exact_from::<F>(maddr, src, count)
            .map_err(Into::into)
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
        let maddr = addr.raw_value() as usize;
        self.as_volatile_slice()
            .unwrap()
            .write_to::<F>(maddr, dst, count)
            .map_err(Into::into)
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
        let maddr = addr.raw_value() as usize;
        self.as_volatile_slice()
            .unwrap()
            .write_all_to::<F>(maddr, dst, count)
            .map_err(Into::into)
    }

    fn store<T: AtomicAccess>(
        &self,
        val: T,
        addr: MemoryRegionAddress,
        order: Ordering,
    ) -> guest_memory::Result<()> {
        self.as_volatile_slice().and_then(|s| {
            s.store(val, addr.raw_value() as usize, order)
                .map_err(Into::into)
        })
    }

    fn load<T: AtomicAccess>(
        &self,
        addr: MemoryRegionAddress,
        order: Ordering,
    ) -> guest_memory::Result<T> {
        self.as_volatile_slice()
            .and_then(|s| s.load(addr.raw_value() as usize, order).map_err(Into::into))
    }
}

impl<B: Bitmap> GuestMemoryRegion for GuestRegionRaw<B> {
    type B = B;

    fn len(&self) -> GuestUsize {
        self.size as GuestUsize
    }

    fn start_addr(&self) -> GuestAddress {
        self.guest_base
    }

    fn bitmap(&self) -> &Self::B {
        &self.bitmap
    }

    fn get_host_address(&self, addr: MemoryRegionAddress) -> guest_memory::Result<*mut u8> {
        // Not sure why wrapping_offset is not unsafe.  Anyway this
        // is safe because we've just range-checked addr using check_address.
        self.check_address(addr)
            .ok_or(guest_memory::Error::InvalidBackendAddress)
            .map(|addr| self.addr.wrapping_offset(addr.raw_value() as isize))
    }

    fn file_offset(&self) -> Option<&FileOffset> {
        None
    }

    unsafe fn as_slice(&self) -> Option<&[u8]> {
        // This is safe because we mapped the area at addr ourselves, so this slice will not
        // overflow. However, it is possible to alias.
        Some(std::slice::from_raw_parts(self.addr, self.size))
    }

    unsafe fn as_mut_slice(&self) -> Option<&mut [u8]> {
        // This is safe because we mapped the area at addr ourselves, so this slice will not
        // overflow. However, it is possible to alias.
        Some(std::slice::from_raw_parts_mut(self.addr, self.size))
    }

    fn get_slice(
        &self,
        offset: MemoryRegionAddress,
        count: usize,
    ) -> guest_memory::Result<VolatileSlice<BS<B>>> {
        let offset = offset.raw_value() as usize;
        let end = compute_offset(offset, count)?;
        if end > self.size {
            return Err(volatile_memory::Error::OutOfBounds { addr: end }.into());
        }

        // Safe because we checked that offset + count was within our range and we only ever hand
        // out volatile accessors.
        Ok(unsafe {
            VolatileSlice::with_bitmap(
                (self.addr as usize + offset) as *mut _,
                count,
                self.bitmap.slice_at(offset),
            )
        })
    }

    #[cfg(target_os = "linux")]
    fn is_hugetlbfs(&self) -> Option<bool> {
        None
    }
}

#[cfg(test)]
mod tests {
    extern crate vmm_sys_util;

    use super::*;
    use crate::{GuestMemoryHybrid, GuestRegionHybrid};
    use std::sync::Arc;
    use vm_memory::{GuestAddressSpace, GuestMemory, VolatileMemory};

    /*
    use crate::bitmap::tests::test_guest_memory_and_region;
    use crate::bitmap::AtomicBitmap;
    use crate::GuestAddressSpace;

    use std::fs::File;
    use std::mem;
    use std::path::Path;
    use vmm_sys_util::tempfile::TempFile;

    type GuestMemoryMmap = super::GuestMemoryMmap<()>;
    type GuestRegionMmap = super::GuestRegionMmap<()>;
    type MmapRegion = super::MmapRegion<()>;
     */

    #[test]
    fn test_region_raw_new() {
        let mut buf = [0u8; 1024];
        let m =
            unsafe { GuestRegionRaw::<()>::new(GuestAddress(0x10_0000), &mut buf as *mut _, 1024) };

        assert_eq!(m.start_addr(), GuestAddress(0x10_0000));
        assert_eq!(m.len(), 1024);
    }

    /*
    fn check_guest_memory_mmap(
        maybe_guest_mem: Result<GuestMemoryMmap, Error>,
        expected_regions_summary: &[(GuestAddress, usize)],
    ) {
        assert!(maybe_guest_mem.is_ok());

        let guest_mem = maybe_guest_mem.unwrap();
        assert_eq!(guest_mem.num_regions(), expected_regions_summary.len());
        let maybe_last_mem_reg = expected_regions_summary.last();
        if let Some((region_addr, region_size)) = maybe_last_mem_reg {
            let mut last_addr = region_addr.unchecked_add(*region_size as u64);
            if last_addr.raw_value() != 0 {
                last_addr = last_addr.unchecked_sub(1);
            }
            assert_eq!(guest_mem.last_addr(), last_addr);
        }
        for ((region_addr, region_size), mmap) in expected_regions_summary
            .iter()
            .zip(guest_mem.regions.iter())
        {
            assert_eq!(region_addr, &mmap.guest_base);
            assert_eq!(region_size, &mmap.mapping.size());

            assert!(guest_mem.find_region(*region_addr).is_some());
        }
    }

    fn new_guest_memory_mmap(
        regions_summary: &[(GuestAddress, usize)],
    ) -> Result<GuestMemoryMmap, Error> {
        GuestMemoryMmap::from_ranges(regions_summary)
    }

    fn new_guest_memory_mmap_from_regions(
        regions_summary: &[(GuestAddress, usize)],
    ) -> Result<GuestMemoryMmap, Error> {
        GuestMemoryMmap::from_regions(
            regions_summary
                .iter()
                .map(|(region_addr, region_size)| {
                    GuestRegionMmap::new(MmapRegion::new(*region_size).unwrap(), *region_addr)
                        .unwrap()
                })
                .collect(),
        )
    }

    fn new_guest_memory_mmap_from_arc_regions(
        regions_summary: &[(GuestAddress, usize)],
    ) -> Result<GuestMemoryMmap, Error> {
        GuestMemoryMmap::from_arc_regions(
            regions_summary
                .iter()
                .map(|(region_addr, region_size)| {
                    Arc::new(
                        GuestRegionMmap::new(MmapRegion::new(*region_size).unwrap(), *region_addr)
                            .unwrap(),
                    )
                })
                .collect(),
        )
    }

    fn new_guest_memory_mmap_with_files(
        regions_summary: &[(GuestAddress, usize)],
    ) -> Result<GuestMemoryMmap, Error> {
        let regions: Vec<(GuestAddress, usize, Option<FileOffset>)> = regions_summary
            .iter()
            .map(|(region_addr, region_size)| {
                let f = TempFile::new().unwrap().into_file();
                f.set_len(*region_size as u64).unwrap();

                (*region_addr, *region_size, Some(FileOffset::new(f, 0)))
            })
            .collect();

        GuestMemoryMmap::from_ranges_with_files(&regions)
    }
    */

    #[test]
    fn slice_addr() {
        let mut buf = [0u8; 1024];
        let m =
            unsafe { GuestRegionRaw::<()>::new(GuestAddress(0x10_0000), &mut buf as *mut _, 1024) };

        let s = m.get_slice(MemoryRegionAddress(2), 3).unwrap();
        assert_eq!(s.as_ptr(), &mut buf[2] as *mut _);
    }

    /*
    #[test]
    fn test_address_in_range() {
        let f1 = TempFile::new().unwrap().into_file();
        f1.set_len(0x400).unwrap();
        let f2 = TempFile::new().unwrap().into_file();
        f2.set_len(0x400).unwrap();

        let start_addr1 = GuestAddress(0x0);
        let start_addr2 = GuestAddress(0x800);
        let guest_mem =
            GuestMemoryMmap::from_ranges(&[(start_addr1, 0x400), (start_addr2, 0x400)]).unwrap();
        let guest_mem_backed_by_file = GuestMemoryMmap::from_ranges_with_files(&[
            (start_addr1, 0x400, Some(FileOffset::new(f1, 0))),
            (start_addr2, 0x400, Some(FileOffset::new(f2, 0))),
        ])
            .unwrap();

        let guest_mem_list = vec![guest_mem, guest_mem_backed_by_file];
        for guest_mem in guest_mem_list.iter() {
            assert!(guest_mem.address_in_range(GuestAddress(0x200)));
            assert!(!guest_mem.address_in_range(GuestAddress(0x600)));
            assert!(guest_mem.address_in_range(GuestAddress(0xa00)));
            assert!(!guest_mem.address_in_range(GuestAddress(0xc00)));
        }
    }

    #[test]
    fn test_check_address() {
        let f1 = TempFile::new().unwrap().into_file();
        f1.set_len(0x400).unwrap();
        let f2 = TempFile::new().unwrap().into_file();
        f2.set_len(0x400).unwrap();

        let start_addr1 = GuestAddress(0x0);
        let start_addr2 = GuestAddress(0x800);
        let guest_mem =
            GuestMemoryMmap::from_ranges(&[(start_addr1, 0x400), (start_addr2, 0x400)]).unwrap();
        let guest_mem_backed_by_file = GuestMemoryMmap::from_ranges_with_files(&[
            (start_addr1, 0x400, Some(FileOffset::new(f1, 0))),
            (start_addr2, 0x400, Some(FileOffset::new(f2, 0))),
        ])
            .unwrap();

        let guest_mem_list = vec![guest_mem, guest_mem_backed_by_file];
        for guest_mem in guest_mem_list.iter() {
            assert_eq!(
                guest_mem.check_address(GuestAddress(0x200)),
                Some(GuestAddress(0x200))
            );
            assert_eq!(guest_mem.check_address(GuestAddress(0x600)), None);
            assert_eq!(
                guest_mem.check_address(GuestAddress(0xa00)),
                Some(GuestAddress(0xa00))
            );
            assert_eq!(guest_mem.check_address(GuestAddress(0xc00)), None);
        }
    }

    #[test]
    fn test_to_region_addr() {
        let f1 = TempFile::new().unwrap().into_file();
        f1.set_len(0x400).unwrap();
        let f2 = TempFile::new().unwrap().into_file();
        f2.set_len(0x400).unwrap();

        let start_addr1 = GuestAddress(0x0);
        let start_addr2 = GuestAddress(0x800);
        let guest_mem =
            GuestMemoryMmap::from_ranges(&[(start_addr1, 0x400), (start_addr2, 0x400)]).unwrap();
        let guest_mem_backed_by_file = GuestMemoryMmap::from_ranges_with_files(&[
            (start_addr1, 0x400, Some(FileOffset::new(f1, 0))),
            (start_addr2, 0x400, Some(FileOffset::new(f2, 0))),
        ])
            .unwrap();

        let guest_mem_list = vec![guest_mem, guest_mem_backed_by_file];
        for guest_mem in guest_mem_list.iter() {
            assert!(guest_mem.to_region_addr(GuestAddress(0x600)).is_none());
            let (r0, addr0) = guest_mem.to_region_addr(GuestAddress(0x800)).unwrap();
            let (r1, addr1) = guest_mem.to_region_addr(GuestAddress(0xa00)).unwrap();
            assert!(r0.as_ptr() == r1.as_ptr());
            assert_eq!(addr0, MemoryRegionAddress(0));
            assert_eq!(addr1, MemoryRegionAddress(0x200));
        }
    }

    #[test]
    fn test_get_host_address() {
        let f1 = TempFile::new().unwrap().into_file();
        f1.set_len(0x400).unwrap();
        let f2 = TempFile::new().unwrap().into_file();
        f2.set_len(0x400).unwrap();

        let start_addr1 = GuestAddress(0x0);
        let start_addr2 = GuestAddress(0x800);
        let guest_mem =
            GuestMemoryMmap::from_ranges(&[(start_addr1, 0x400), (start_addr2, 0x400)]).unwrap();
        let guest_mem_backed_by_file = GuestMemoryMmap::from_ranges_with_files(&[
            (start_addr1, 0x400, Some(FileOffset::new(f1, 0))),
            (start_addr2, 0x400, Some(FileOffset::new(f2, 0))),
        ])
            .unwrap();

        let guest_mem_list = vec![guest_mem, guest_mem_backed_by_file];
        for guest_mem in guest_mem_list.iter() {
            assert!(guest_mem.get_host_address(GuestAddress(0x600)).is_err());
            let ptr0 = guest_mem.get_host_address(GuestAddress(0x800)).unwrap();
            let ptr1 = guest_mem.get_host_address(GuestAddress(0xa00)).unwrap();
            assert_eq!(
                ptr0,
                guest_mem.find_region(GuestAddress(0x800)).unwrap().as_ptr()
            );
            assert_eq!(unsafe { ptr0.offset(0x200) }, ptr1);
        }
    }

    #[test]
    fn test_deref() {
        let f = TempFile::new().unwrap().into_file();
        f.set_len(0x400).unwrap();

        let start_addr = GuestAddress(0x0);
        let guest_mem = GuestMemoryMmap::from_ranges(&[(start_addr, 0x400)]).unwrap();
        let guest_mem_backed_by_file = GuestMemoryMmap::from_ranges_with_files(&[(
            start_addr,
            0x400,
            Some(FileOffset::new(f, 0)),
        )])
            .unwrap();

        let guest_mem_list = vec![guest_mem, guest_mem_backed_by_file];
        for guest_mem in guest_mem_list.iter() {
            let sample_buf = &[1, 2, 3, 4, 5];

            assert_eq!(guest_mem.write(sample_buf, start_addr).unwrap(), 5);
            let slice = guest_mem
                .find_region(GuestAddress(0))
                .unwrap()
                .as_volatile_slice()
                .unwrap();

            let buf = &mut [0, 0, 0, 0, 0];
            assert_eq!(slice.read(buf, 0).unwrap(), 5);
            assert_eq!(buf, sample_buf);
        }
    }

    #[test]
    fn test_read_u64() {
        let f1 = TempFile::new().unwrap().into_file();
        f1.set_len(0x1000).unwrap();
        let f2 = TempFile::new().unwrap().into_file();
        f2.set_len(0x1000).unwrap();

        let start_addr1 = GuestAddress(0x0);
        let start_addr2 = GuestAddress(0x1000);
        let bad_addr = GuestAddress(0x2001);
        let bad_addr2 = GuestAddress(0x1ffc);
        let max_addr = GuestAddress(0x2000);

        let gm =
            GuestMemoryMmap::from_ranges(&[(start_addr1, 0x1000), (start_addr2, 0x1000)]).unwrap();
        let gm_backed_by_file = GuestMemoryMmap::from_ranges_with_files(&[
            (start_addr1, 0x1000, Some(FileOffset::new(f1, 0))),
            (start_addr2, 0x1000, Some(FileOffset::new(f2, 0))),
        ])
            .unwrap();

        let gm_list = vec![gm, gm_backed_by_file];
        for gm in gm_list.iter() {
            let val1: u64 = 0xaa55_aa55_aa55_aa55;
            let val2: u64 = 0x55aa_55aa_55aa_55aa;
            assert_eq!(
                format!("{:?}", gm.write_obj(val1, bad_addr).err().unwrap()),
                format!("InvalidGuestAddress({:?})", bad_addr,)
            );
            assert_eq!(
                format!("{:?}", gm.write_obj(val1, bad_addr2).err().unwrap()),
                format!(
                    "PartialBuffer {{ expected: {:?}, completed: {:?} }}",
                    mem::size_of::<u64>(),
                    max_addr.checked_offset_from(bad_addr2).unwrap()
                )
            );

            gm.write_obj(val1, GuestAddress(0x500)).unwrap();
            gm.write_obj(val2, GuestAddress(0x1000 + 32)).unwrap();
            let num1: u64 = gm.read_obj(GuestAddress(0x500)).unwrap();
            let num2: u64 = gm.read_obj(GuestAddress(0x1000 + 32)).unwrap();
            assert_eq!(val1, num1);
            assert_eq!(val2, num2);
        }
    }

    #[test]
    fn write_and_read() {
        let f = TempFile::new().unwrap().into_file();
        f.set_len(0x400).unwrap();

        let mut start_addr = GuestAddress(0x1000);
        let gm = GuestMemoryMmap::from_ranges(&[(start_addr, 0x400)]).unwrap();
        let gm_backed_by_file = GuestMemoryMmap::from_ranges_with_files(&[(
            start_addr,
            0x400,
            Some(FileOffset::new(f, 0)),
        )])
            .unwrap();

        let gm_list = vec![gm, gm_backed_by_file];
        for gm in gm_list.iter() {
            let sample_buf = &[1, 2, 3, 4, 5];

            assert_eq!(gm.write(sample_buf, start_addr).unwrap(), 5);

            let buf = &mut [0u8; 5];
            assert_eq!(gm.read(buf, start_addr).unwrap(), 5);
            assert_eq!(buf, sample_buf);

            start_addr = GuestAddress(0x13ff);
            assert_eq!(gm.write(sample_buf, start_addr).unwrap(), 1);
            assert_eq!(gm.read(buf, start_addr).unwrap(), 1);
            assert_eq!(buf[0], sample_buf[0]);
            start_addr = GuestAddress(0x1000);
        }
    }

    #[test]
    fn read_to_and_write_from_mem() {
        let f = TempFile::new().unwrap().into_file();
        f.set_len(0x400).unwrap();

        let gm = GuestMemoryMmap::from_ranges(&[(GuestAddress(0x1000), 0x400)]).unwrap();
        let gm_backed_by_file = GuestMemoryMmap::from_ranges_with_files(&[(
            GuestAddress(0x1000),
            0x400,
            Some(FileOffset::new(f, 0)),
        )])
            .unwrap();

        let gm_list = vec![gm, gm_backed_by_file];
        for gm in gm_list.iter() {
            let addr = GuestAddress(0x1010);
            let mut file = if cfg!(unix) {
                File::open(Path::new("/dev/zero")).unwrap()
            } else {
                File::open(Path::new("c:\\Windows\\system32\\ntoskrnl.exe")).unwrap()
            };
            gm.write_obj(!0u32, addr).unwrap();
            gm.read_exact_from(addr, &mut file, mem::size_of::<u32>())
                .unwrap();
            let value: u32 = gm.read_obj(addr).unwrap();
            if cfg!(unix) {
                assert_eq!(value, 0);
            } else {
                assert_eq!(value, 0x0090_5a4d);
            }

            let mut sink = Vec::new();
            gm.write_all_to(addr, &mut sink, mem::size_of::<u32>())
                .unwrap();
            if cfg!(unix) {
                assert_eq!(sink, vec![0; mem::size_of::<u32>()]);
            } else {
                assert_eq!(sink, vec![0x4d, 0x5a, 0x90, 0x00]);
            };
        }
    }

    #[test]
    fn create_vec_with_regions() {
        let region_size = 0x400;
        let regions = vec![
            (GuestAddress(0x0), region_size),
            (GuestAddress(0x1000), region_size),
        ];
        let mut iterated_regions = Vec::new();
        let gm = GuestMemoryMmap::from_ranges(&regions).unwrap();

        for region in gm.iter() {
            assert_eq!(region.len(), region_size as GuestUsize);
        }

        for region in gm.iter() {
            iterated_regions.push((region.start_addr(), region.len() as usize));
        }
        assert_eq!(regions, iterated_regions);

        assert!(regions
            .iter()
            .map(|x| (x.0, x.1))
            .eq(iterated_regions.iter().copied()));

        assert_eq!(gm.regions[0].guest_base, regions[0].0);
        assert_eq!(gm.regions[1].guest_base, regions[1].0);
    }

    #[test]
    fn test_memory() {
        let region_size = 0x400;
        let regions = vec![
            (GuestAddress(0x0), region_size),
            (GuestAddress(0x1000), region_size),
        ];
        let mut iterated_regions = Vec::new();
        let gm = Arc::new(GuestMemoryMmap::from_ranges(&regions).unwrap());
        let mem = gm.memory();

        for region in mem.iter() {
            assert_eq!(region.len(), region_size as GuestUsize);
        }

        for region in mem.iter() {
            iterated_regions.push((region.start_addr(), region.len() as usize));
        }
        assert_eq!(regions, iterated_regions);

        assert!(regions
            .iter()
            .map(|x| (x.0, x.1))
            .eq(iterated_regions.iter().copied()));

        assert_eq!(gm.regions[0].guest_base, regions[0].0);
        assert_eq!(gm.regions[1].guest_base, regions[1].0);
    }

    #[test]
    fn test_access_cross_boundary() {
        let f1 = TempFile::new().unwrap().into_file();
        f1.set_len(0x1000).unwrap();
        let f2 = TempFile::new().unwrap().into_file();
        f2.set_len(0x1000).unwrap();

        let start_addr1 = GuestAddress(0x0);
        let start_addr2 = GuestAddress(0x1000);
        let gm =
            GuestMemoryMmap::from_ranges(&[(start_addr1, 0x1000), (start_addr2, 0x1000)]).unwrap();
        let gm_backed_by_file = GuestMemoryMmap::from_ranges_with_files(&[
            (start_addr1, 0x1000, Some(FileOffset::new(f1, 0))),
            (start_addr2, 0x1000, Some(FileOffset::new(f2, 0))),
        ])
            .unwrap();

        let gm_list = vec![gm, gm_backed_by_file];
        for gm in gm_list.iter() {
            let sample_buf = &[1, 2, 3, 4, 5];
            assert_eq!(gm.write(sample_buf, GuestAddress(0xffc)).unwrap(), 5);
            let buf = &mut [0u8; 5];
            assert_eq!(gm.read(buf, GuestAddress(0xffc)).unwrap(), 5);
            assert_eq!(buf, sample_buf);
        }
    }

    #[test]
    fn test_retrieve_fd_backing_memory_region() {
        let f = TempFile::new().unwrap().into_file();
        f.set_len(0x400).unwrap();

        let start_addr = GuestAddress(0x0);
        let gm = GuestMemoryMmap::from_ranges(&[(start_addr, 0x400)]).unwrap();
        assert!(gm.find_region(start_addr).is_some());
        let region = gm.find_region(start_addr).unwrap();
        assert!(region.file_offset().is_none());

        let gm = GuestMemoryMmap::from_ranges_with_files(&[(
            start_addr,
            0x400,
            Some(FileOffset::new(f, 0)),
        )])
            .unwrap();
        assert!(gm.find_region(start_addr).is_some());
        let region = gm.find_region(start_addr).unwrap();
        assert!(region.file_offset().is_some());
    }

    // Windows needs a dedicated test where it will retrieve the allocation
    // granularity to determine a proper offset (other than 0) that can be
    // used for the backing file. Refer to Microsoft docs here:
    // https://docs.microsoft.com/en-us/windows/desktop/api/memoryapi/nf-memoryapi-mapviewoffile
    #[test]
    #[cfg(unix)]
    fn test_retrieve_offset_from_fd_backing_memory_region() {
        let f = TempFile::new().unwrap().into_file();
        f.set_len(0x1400).unwrap();
        // Needs to be aligned on 4k, otherwise mmap will fail.
        let offset = 0x1000;

        let start_addr = GuestAddress(0x0);
        let gm = GuestMemoryMmap::from_ranges(&[(start_addr, 0x400)]).unwrap();
        assert!(gm.find_region(start_addr).is_some());
        let region = gm.find_region(start_addr).unwrap();
        assert!(region.file_offset().is_none());

        let gm = GuestMemoryMmap::from_ranges_with_files(&[(
            start_addr,
            0x400,
            Some(FileOffset::new(f, offset)),
        )])
            .unwrap();
        assert!(gm.find_region(start_addr).is_some());
        let region = gm.find_region(start_addr).unwrap();
        assert!(region.file_offset().is_some());
        assert_eq!(region.file_offset().unwrap().start(), offset);
    }
     */

    #[test]
    fn test_mmap_insert_region() {
        let start_addr1 = GuestAddress(0);
        let start_addr2 = GuestAddress(0x10_0000);

        let guest_mem = GuestMemoryHybrid::<()>::new();
        let mut raw_buf = [0u8; 0x1000];
        let raw_ptr = &mut raw_buf as *mut u8;
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr1, raw_ptr, 0x1000) };
        let guest_mem = guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr2, raw_ptr, 0x1000) };
        let gm = &guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();
        let mem_orig = gm.memory();
        assert_eq!(mem_orig.num_regions(), 2);

        let reg = unsafe { GuestRegionRaw::new(GuestAddress(0x8000), raw_ptr, 0x1000) };
        let mmap = Arc::new(GuestRegionHybrid::from_raw_region(reg));
        let gm = gm.insert_region(mmap).unwrap();
        let reg = unsafe { GuestRegionRaw::new(GuestAddress(0x4000), raw_ptr, 0x1000) };
        let mmap = Arc::new(GuestRegionHybrid::from_raw_region(reg));
        let gm = gm.insert_region(mmap).unwrap();
        let reg = unsafe { GuestRegionRaw::new(GuestAddress(0xc000), raw_ptr, 0x1000) };
        let mmap = Arc::new(GuestRegionHybrid::from_raw_region(reg));
        let gm = gm.insert_region(mmap).unwrap();
        let reg = unsafe { GuestRegionRaw::new(GuestAddress(0xc000), raw_ptr, 0x1000) };
        let mmap = Arc::new(GuestRegionHybrid::from_raw_region(reg));
        gm.insert_region(mmap).unwrap_err();

        assert_eq!(mem_orig.num_regions(), 2);
        assert_eq!(gm.num_regions(), 5);

        assert_eq!(gm.regions[0].start_addr(), GuestAddress(0x0000));
        assert_eq!(gm.regions[1].start_addr(), GuestAddress(0x4000));
        assert_eq!(gm.regions[2].start_addr(), GuestAddress(0x8000));
        assert_eq!(gm.regions[3].start_addr(), GuestAddress(0xc000));
        assert_eq!(gm.regions[4].start_addr(), GuestAddress(0x10_0000));
    }

    #[test]
    fn test_mmap_remove_region() {
        let start_addr1 = GuestAddress(0);
        let start_addr2 = GuestAddress(0x10_0000);

        let guest_mem = GuestMemoryHybrid::<()>::new();
        let mut raw_buf = [0u8; 0x1000];
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr1, &mut raw_buf as *mut _, 0x1000) };
        let guest_mem = guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr2, &mut raw_buf as *mut _, 0x1000) };
        let gm = &guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();
        let mem_orig = gm.memory();
        assert_eq!(mem_orig.num_regions(), 2);

        gm.remove_region(GuestAddress(0), 128).unwrap_err();
        gm.remove_region(GuestAddress(0x4000), 128).unwrap_err();
        let (gm, region) = gm.remove_region(GuestAddress(0x10_0000), 0x1000).unwrap();

        assert_eq!(mem_orig.num_regions(), 2);
        assert_eq!(gm.num_regions(), 1);

        assert_eq!(gm.regions[0].start_addr(), GuestAddress(0x0000));
        assert_eq!(region.start_addr(), GuestAddress(0x10_0000));
    }

    #[test]
    fn test_guest_memory_mmap_get_slice() {
        let start_addr1 = GuestAddress(0);
        let mut raw_buf = [0u8; 0x400];
        let region =
            unsafe { GuestRegionRaw::<()>::new(start_addr1, &mut raw_buf as *mut _, 0x400) };

        // Normal case.
        let slice_addr = MemoryRegionAddress(0x100);
        let slice_size = 0x200;
        let slice = region.get_slice(slice_addr, slice_size).unwrap();
        assert_eq!(slice.len(), slice_size);

        // Empty slice.
        let slice_addr = MemoryRegionAddress(0x200);
        let slice_size = 0x0;
        let slice = region.get_slice(slice_addr, slice_size).unwrap();
        assert!(slice.is_empty());

        // Error case when slice_size is beyond the boundary.
        let slice_addr = MemoryRegionAddress(0x300);
        let slice_size = 0x200;
        assert!(region.get_slice(slice_addr, slice_size).is_err());
    }

    #[test]
    fn test_guest_memory_mmap_as_volatile_slice() {
        let start_addr1 = GuestAddress(0);
        let mut raw_buf = [0u8; 0x400];
        let region =
            unsafe { GuestRegionRaw::<()>::new(start_addr1, &mut raw_buf as *mut _, 0x400) };
        let region_size = 0x400;

        // Test slice length.
        let slice = region.as_volatile_slice().unwrap();
        assert_eq!(slice.len(), region_size);

        // Test slice data.
        let v = 0x1234_5678u32;
        let r = slice.get_ref::<u32>(0x200).unwrap();
        r.store(v);
        assert_eq!(r.load(), v);
    }

    #[test]
    fn test_guest_memory_get_slice() {
        let start_addr1 = GuestAddress(0);
        let start_addr2 = GuestAddress(0x800);

        let guest_mem = GuestMemoryHybrid::<()>::new();
        let mut raw_buf = [0u8; 0x400];
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr1, &mut raw_buf as *mut _, 0x400) };
        let guest_mem = guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr2, &mut raw_buf as *mut _, 0x400) };
        let guest_mem = guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();

        // Normal cases.
        let slice_size = 0x200;
        let slice = guest_mem
            .get_slice(GuestAddress(0x100), slice_size)
            .unwrap();
        assert_eq!(slice.len(), slice_size);

        let slice_size = 0x400;
        let slice = guest_mem
            .get_slice(GuestAddress(0x800), slice_size)
            .unwrap();
        assert_eq!(slice.len(), slice_size);

        // Empty slice.
        assert!(guest_mem
            .get_slice(GuestAddress(0x900), 0)
            .unwrap()
            .is_empty());

        // Error cases, wrong size or base address.
        assert!(guest_mem.get_slice(GuestAddress(0), 0x500).is_err());
        assert!(guest_mem.get_slice(GuestAddress(0x600), 0x100).is_err());
        assert!(guest_mem.get_slice(GuestAddress(0xc00), 0x100).is_err());
    }

    #[test]
    fn test_checked_offset() {
        let start_addr1 = GuestAddress(0);
        let start_addr2 = GuestAddress(0x800);
        let start_addr3 = GuestAddress(0xc00);

        let guest_mem = GuestMemoryHybrid::<()>::new();
        let mut raw_buf = [0u8; 0x400];
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr1, &mut raw_buf as *mut _, 0x400) };
        let guest_mem = guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr2, &mut raw_buf as *mut _, 0x400) };
        let guest_mem = guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr3, &mut raw_buf as *mut _, 0x400) };
        let guest_mem = guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();

        assert_eq!(
            guest_mem.checked_offset(start_addr1, 0x200),
            Some(GuestAddress(0x200))
        );
        assert_eq!(
            guest_mem.checked_offset(start_addr1, 0xa00),
            Some(GuestAddress(0xa00))
        );
        assert_eq!(
            guest_mem.checked_offset(start_addr2, 0x7ff),
            Some(GuestAddress(0xfff))
        );
        assert_eq!(guest_mem.checked_offset(start_addr2, 0xc00), None);
        assert_eq!(guest_mem.checked_offset(start_addr1, std::usize::MAX), None);

        assert_eq!(guest_mem.checked_offset(start_addr1, 0x400), None);
        assert_eq!(
            guest_mem.checked_offset(start_addr1, 0x400 - 1),
            Some(GuestAddress(0x400 - 1))
        );
    }

    #[test]
    fn test_check_range() {
        let start_addr1 = GuestAddress(0);
        let start_addr2 = GuestAddress(0x800);
        let start_addr3 = GuestAddress(0xc00);

        let guest_mem = GuestMemoryHybrid::<()>::new();
        let mut raw_buf = [0u8; 0x400];
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr1, &mut raw_buf as *mut _, 0x400) };
        let guest_mem = guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr2, &mut raw_buf as *mut _, 0x400) };
        let guest_mem = guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();
        let reg = unsafe { GuestRegionRaw::<()>::new(start_addr3, &mut raw_buf as *mut _, 0x400) };
        let guest_mem = guest_mem
            .insert_region(Arc::new(GuestRegionHybrid::from_raw_region(reg)))
            .unwrap();

        assert!(guest_mem.check_range(start_addr1, 0x0));
        assert!(guest_mem.check_range(start_addr1, 0x200));
        assert!(guest_mem.check_range(start_addr1, 0x400));
        assert!(!guest_mem.check_range(start_addr1, 0xa00));
        assert!(guest_mem.check_range(start_addr2, 0x7ff));
        assert!(guest_mem.check_range(start_addr2, 0x800));
        assert!(!guest_mem.check_range(start_addr2, 0x801));
        assert!(!guest_mem.check_range(start_addr2, 0xc00));
        assert!(!guest_mem.check_range(start_addr1, usize::MAX));
    }
}
