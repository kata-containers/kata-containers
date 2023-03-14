// Copyright 2020-2021 Ant Group. All rights reserved.
// Copyright (C) 2020-2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! RAFS v5 on disk layout structures.
//!
//! # RAFS File System Meta Data Format Version 5
//! Previously RAFS has different formats for on disk meta data and runtime meta data. So when
//! initializing an RAFS instance, it will sequentially read and parse the on disk meta data,
//! build a copy of in memory runtime meta data. This may cause slow startup and cost too much
//! memory to build in memory meta data.
//!
//! The RAFS File System Meta Data Format Version 5 (aka V5) is defined to support directly mapping
//! RAFS meta data into process as runtime meta data, so we could parse RAFS on disk meta data on
//! demand. The V5 meta data format has following changes:
//! 1) file system version number been bumped to 0x500.
//! 2) Directory inodes will sequentially assign globally unique `child index` to it's child inodes.
//!    Two fields, "child_index" and "child_count", have been added to the OndiskInode struct.
//! 3) For inodes with hard link count as 1, the `child index` equals to its assigned inode number.
//! 4) For inodes with hard link count bigger than 1, the `child index` may be different from the
//!    assigned inode number. Among those child entries linking to the same inode, there's will be
//!    one and only one child entry having the inode number as its assigned `child index'.
//! 5) A child index mapping table is introduced, which is used to map `child index` into offset
//!    from the base of the super block. The formula to calculate the inode offset is:
//!      `inode_offset_from_sb = inode_table[child_index] << 3`
//! 6) The child index mapping table follows the super block by default.
//!
//! Giving above definition, we could get the inode object for an inode number or child index as:
//!    inode_ptr = sb_base_ptr + inode_offset_from_sb(inode_number)
//!    inode_ptr = sb_base_ptr + inode_offset_from_sb(child_index)
//!
//! On the other hand, Rafs v4 is compatible with Rafs v5, so Rafs v5 implementation supports
//! both v4 and v5 metadata.

use std::cmp;
use std::convert::TryFrom;
use std::ffi::{OsStr, OsString};
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::io::{Read, Result};
use std::mem::size_of;
use std::os::unix::ffi::OsStrExt;
use std::sync::Arc;

use nydus_utils::digest::{self, DigestHasher, RafsDigest};
use nydus_utils::{compress, ByteSize};
use storage::device::{BlobChunkInfo, BlobFeatures, BlobIoChunk, BlobIoDesc, BlobIoVec};

use crate::metadata::layout::{bytes_to_os_str, MetaRange, RafsXAttrs, RAFS_SUPER_VERSION_V5};
use crate::metadata::md_v5::V5IoChunk;
use crate::metadata::{
    Inode, RafsInode, RafsStore, RafsSuperFlags, RAFS_DEFAULT_CHUNK_SIZE, RAFS_MAX_CHUNK_SIZE,
};
use crate::{impl_bootstrap_converter, impl_pub_getter_setter, RafsIoReader, RafsIoWrite};

// With Rafs v5, the storage manager needs to access file system metadata to decompress the
// compressed blob file. To avoid circular dependency, the following Rafs v5 metadata structures
// have been moved into the storage manager.
use storage::device::v5::BlobV5ChunkInfo;
use storage::device::{BlobChunkFlags, BlobInfo};
use vm_memory::VolatileMemory;

pub(crate) const RAFSV5_ALIGNMENT: usize = 8;
pub(crate) const RAFSV5_SUPERBLOCK_SIZE: usize = 8192;
pub(crate) const RAFSV5_EXT_BLOB_ENTRY_SIZE: usize = 64;

const RAFSV5_SUPER_MAGIC: u32 = 0x5241_4653;
const RAFSV5_SUPERBLOCK_RESERVED_SIZE: usize = RAFSV5_SUPERBLOCK_SIZE - 80;
const RAFSV5_EXT_BLOB_RESERVED_SIZE: usize = RAFSV5_EXT_BLOB_ENTRY_SIZE - 24;

/// Trait to get information about a Rafs v5 inode.
pub(crate) trait RafsV5InodeOps {
    /// Get the `BlobInfo` object corresponding to the `blob_index`.
    fn get_blob_by_index(&self, blob_index: u32) -> Result<Arc<BlobInfo>>;

    /// Get chunk size for the inode.
    fn get_chunk_size(&self) -> u32;

    /// Check whether the inode has hole chunk.
    fn has_hole(&self) -> bool;

    /// Convert to the on disk data format.
    fn cast_ondisk(&self) -> Result<RafsV5Inode>;
}

pub(crate) trait RafsV5InodeChunkOps {
    /// Get chunk info object for a chunk.
    fn get_chunk_info_v5(&self, idx: u32) -> Result<Arc<dyn BlobV5ChunkInfo>>;
}

impl From<RafsSuperFlags> for digest::Algorithm {
    fn from(flags: RafsSuperFlags) -> Self {
        match flags {
            x if x.contains(RafsSuperFlags::DIGESTER_BLAKE3) => digest::Algorithm::Blake3,
            x if x.contains(RafsSuperFlags::DIGESTER_SHA256) => digest::Algorithm::Sha256,
            _ => digest::Algorithm::Blake3,
        }
    }
}

impl From<digest::Algorithm> for RafsSuperFlags {
    fn from(d: digest::Algorithm) -> RafsSuperFlags {
        match d {
            digest::Algorithm::Blake3 => RafsSuperFlags::DIGESTER_BLAKE3,
            digest::Algorithm::Sha256 => RafsSuperFlags::DIGESTER_SHA256,
        }
    }
}

impl From<RafsSuperFlags> for compress::Algorithm {
    fn from(flags: RafsSuperFlags) -> Self {
        match flags {
            x if x.contains(RafsSuperFlags::COMPRESS_NONE) => compress::Algorithm::None,
            x if x.contains(RafsSuperFlags::COMPRESS_LZ4_BLOCK) => compress::Algorithm::Lz4Block,
            x if x.contains(RafsSuperFlags::COMPRESS_GZIP) => compress::Algorithm::GZip,
            x if x.contains(RafsSuperFlags::COMPRESS_ZSTD) => compress::Algorithm::Zstd,
            _ => compress::Algorithm::Lz4Block,
        }
    }
}

impl From<compress::Algorithm> for RafsSuperFlags {
    fn from(c: compress::Algorithm) -> RafsSuperFlags {
        match c {
            compress::Algorithm::None => RafsSuperFlags::COMPRESS_NONE,
            compress::Algorithm::Lz4Block => RafsSuperFlags::COMPRESS_LZ4_BLOCK,
            compress::Algorithm::GZip => RafsSuperFlags::COMPRESS_GZIP,
            compress::Algorithm::Zstd => RafsSuperFlags::COMPRESS_ZSTD,
        }
    }
}

/// Rafs v5 superblock on disk metadata, 8192 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RafsV5SuperBlock {
    /// RAFS super magic
    s_magic: u32,
    /// RAFS version
    s_fs_version: u32,
    /// superblock on disk size
    s_sb_size: u32,
    /// block size
    s_block_size: u32,
    /// superblock flags
    s_flags: u64,
    /// V5: Number of unique inodes(hard link counts as 1).
    s_inodes_count: u64,
    /// V5: Offset of inode table
    s_inode_table_offset: u64,
    /// Those inodes which need to prefetch will have there indexes put into this table.
    /// Then Rafs has a hint to prefetch inodes and doesn't have to load all inodes to page cache
    /// under *direct* metadata mode. It helps save memory usage.
    /// [idx1:u32, idx2:u32, idx3:u32 ...]
    s_prefetch_table_offset: u64,
    /// V5: Offset of blob table
    s_blob_table_offset: u64,
    /// V5: Size of inode table
    s_inode_table_entries: u32,
    s_prefetch_table_entries: u32, // 64 bytes
    /// V5: Entries of blob table
    s_blob_table_size: u32,
    s_extended_blob_table_entries: u32, // 72 bytes
    /// Extended Blob Table
    s_extended_blob_table_offset: u64, // 80 bytes --- reduce me from `RAFS_SUPERBLOCK_RESERVED_SIZE`
    /// Unused area
    s_reserved: [u8; RAFSV5_SUPERBLOCK_RESERVED_SIZE],
}

impl RafsV5SuperBlock {
    /// Create a new instance of `RafsV5SuperBlock`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check whether it's a valid Rafs v5 super block.
    pub fn detect(&self) -> bool {
        self.is_rafs_v5()
    }

    /// Check whether it's super block for Rafs v4/v5.
    pub fn is_rafs_v5(&self) -> bool {
        self.magic() == RAFSV5_SUPER_MAGIC && self.version() == RAFS_SUPER_VERSION_V5
    }

    /// Validate the Rafs v5 super block.
    pub fn validate(&self, meta_size: u64) -> Result<()> {
        if !self.is_rafs_v5() {
            return Err(einval!("invalid super block version number"));
        } else if self.sb_size() as usize != RAFSV5_SUPERBLOCK_SIZE
            || meta_size <= RAFSV5_SUPERBLOCK_SIZE as u64
        {
            return Err(einval!("invalid super block blob size"));
        } else if !self.block_size().is_power_of_two()
            || self.block_size() < 0x1000
            || (self.block_size() as u64 > RAFS_MAX_CHUNK_SIZE && self.block_size() != 4 << 20)
        {
            // Stargz has a special chunk size of 4MB.
            return Err(einval!("invalid block size"));
        } else if RafsSuperFlags::from_bits(self.flags()).is_none() {
            return Err(einval!("invalid super block flags"));
        }

        let meta_range = MetaRange::new(
            RAFSV5_SUPERBLOCK_SIZE as u64,
            meta_size - RAFSV5_SUPERBLOCK_SIZE as u64,
            true,
        )?;

        let inodes_count = self.inodes_count();
        let inode_table_offset = self.inode_table_offset();
        let inode_table_entries = self.inode_table_entries() as u64;
        let inode_table_size = inode_table_entries * size_of::<u32>() as u64;
        let inode_table_range = MetaRange::new(inode_table_offset, inode_table_size, false)?;
        if inodes_count > inode_table_entries || !inode_table_range.is_subrange_of(&meta_range) {
            return Err(einval!("invalid inode table count, offset or entries."));
        }

        let blob_table_offset = self.blob_table_offset();
        let blob_table_size = self.blob_table_size() as u64;
        let blob_table_range = MetaRange::new(blob_table_offset, blob_table_size, false)?;
        if !blob_table_range.is_subrange_of(&meta_range)
            || blob_table_range.intersect_with(&inode_table_range)
        {
            return Err(einval!("invalid blob table offset or size."));
        }

        let ext_blob_table_offset = self.extended_blob_table_offset();
        let ext_blob_table_size =
            self.extended_blob_table_entries() as u64 * RAFSV5_EXT_BLOB_ENTRY_SIZE as u64;
        let ext_blob_table_range =
            MetaRange::new(ext_blob_table_offset, ext_blob_table_size, true)?;
        if ext_blob_table_size != 0
            && (!ext_blob_table_range.is_subrange_of(&meta_range)
                || ext_blob_table_range.intersect_with(&inode_table_range)
                || ext_blob_table_range.intersect_with(&blob_table_range))
        {
            return Err(einval!("invalid extended blob table offset or size."));
        }

        let prefetch_table_offset = self.prefetch_table_offset();
        let prefetch_table_size = self.prefetch_table_entries() as u64 * size_of::<u32>() as u64;
        let prefetch_table_range =
            MetaRange::new(prefetch_table_offset, prefetch_table_size, false)?;
        if prefetch_table_size != 0
            && (!prefetch_table_range.is_subrange_of(&meta_range)
                || prefetch_table_range.intersect_with(&inode_table_range)
                || prefetch_table_range.intersect_with(&blob_table_range)
                || (ext_blob_table_size != 0
                    && prefetch_table_range.intersect_with(&ext_blob_table_range)))
        {
            return Err(einval!("invalid prefetch table offset or size."));
        }

        Ok(())
    }

    /// Set chunk size.
    pub fn set_chunk_size(&mut self, chunk_size: u32) {
        debug_assert!(chunk_size.is_power_of_two());
        self.s_block_size = chunk_size;
    }

    /// Set compression algorithm to handle chunk of the Rafs filesystem.
    pub fn set_compressor(&mut self, compressor: compress::Algorithm) {
        let c: RafsSuperFlags = compressor.into();

        self.s_flags &= !RafsSuperFlags::COMPRESS_NONE.bits();
        self.s_flags &= !RafsSuperFlags::COMPRESS_LZ4_BLOCK.bits();
        self.s_flags &= !RafsSuperFlags::COMPRESS_GZIP.bits();
        self.s_flags &= !RafsSuperFlags::COMPRESS_ZSTD.bits();
        self.s_flags |= c.bits();
    }

    /// Set message digest algorithm to handle chunk of the Rafs filesystem.
    pub fn set_digester(&mut self, digester: digest::Algorithm) {
        let c: RafsSuperFlags = digester.into();

        self.s_flags &= !RafsSuperFlags::DIGESTER_BLAKE3.bits();
        self.s_flags &= !RafsSuperFlags::DIGESTER_SHA256.bits();
        self.s_flags |= c.bits();
    }

    /// Enable explicit Uid/Gid feature.
    pub fn set_explicit_uidgid(&mut self) {
        self.s_flags |= RafsSuperFlags::EXPLICIT_UID_GID.bits();
    }

    /// Enable support of filesystem xattr.
    pub fn set_has_xattr(&mut self) {
        self.s_flags |= RafsSuperFlags::HAS_XATTR.bits();
    }

    impl_pub_getter_setter!(magic, set_magic, s_magic, u32);
    impl_pub_getter_setter!(version, set_version, s_fs_version, u32);
    impl_pub_getter_setter!(sb_size, set_sb_size, s_sb_size, u32);
    impl_pub_getter_setter!(block_size, set_block_size, s_block_size, u32);
    impl_pub_getter_setter!(flags, set_flags, s_flags, u64);
    impl_pub_getter_setter!(inodes_count, set_inodes_count, s_inodes_count, u64);
    impl_pub_getter_setter!(
        inode_table_entries,
        set_inode_table_entries,
        s_inode_table_entries,
        u32
    );
    impl_pub_getter_setter!(
        inode_table_offset,
        set_inode_table_offset,
        s_inode_table_offset,
        u64
    );
    impl_pub_getter_setter!(blob_table_size, set_blob_table_size, s_blob_table_size, u32);
    impl_pub_getter_setter!(
        blob_table_offset,
        set_blob_table_offset,
        s_blob_table_offset,
        u64
    );
    impl_pub_getter_setter!(
        prefetch_table_offset,
        set_prefetch_table_offset,
        s_prefetch_table_offset,
        u64
    );
    impl_pub_getter_setter!(
        prefetch_table_entries,
        set_prefetch_table_entries,
        s_prefetch_table_entries,
        u32
    );
    impl_pub_getter_setter!(
        extended_blob_table_offset,
        set_extended_blob_table_offset,
        s_extended_blob_table_offset,
        u64
    );
    impl_pub_getter_setter!(
        extended_blob_table_entries,
        set_extended_blob_table_entries,
        s_extended_blob_table_entries,
        u32
    );

    /// Load a super block from a `RafsIoReader` object.
    pub fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        r.read_exact(self.as_mut())
    }

    /// Read Rafs v5 super block from a reader.
    pub fn read(r: &mut RafsIoReader) -> Result<Self> {
        let mut sb = RafsV5SuperBlock::new();

        r.read_exact(sb.as_mut())?;

        Ok(sb)
    }
}

impl RafsStore for RafsV5SuperBlock {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        w.write_all(self.as_ref())?;
        w.validate_alignment(self.as_ref().len(), RAFSV5_ALIGNMENT)
    }
}

impl_bootstrap_converter!(RafsV5SuperBlock);

impl Default for RafsV5SuperBlock {
    fn default() -> Self {
        Self {
            s_magic: u32::to_le(RAFSV5_SUPER_MAGIC as u32),
            s_fs_version: u32::to_le(RAFS_SUPER_VERSION_V5),
            s_sb_size: u32::to_le(RAFSV5_SUPERBLOCK_SIZE as u32),
            s_block_size: u32::to_le(RAFS_DEFAULT_CHUNK_SIZE as u32),
            s_flags: u64::to_le(0),
            s_inodes_count: u64::to_le(0),
            s_inode_table_entries: u32::to_le(0),
            s_inode_table_offset: u64::to_le(0),
            s_prefetch_table_offset: u64::to_le(0),
            s_prefetch_table_entries: u32::to_le(0),
            s_blob_table_size: u32::to_le(0),
            s_blob_table_offset: u64::to_le(0),
            s_extended_blob_table_offset: u64::to_le(0),
            s_extended_blob_table_entries: u32::to_le(0),
            s_reserved: [0u8; RAFSV5_SUPERBLOCK_RESERVED_SIZE],
        }
    }
}

impl Display for RafsV5SuperBlock {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "superblock: magic {:x}, version {:x}, sb_size {:x}, block_size {:x}, flags {:x}, inode_count {}",
               self.magic(), self.version(), self.sb_size(), self.block_size(),
               self.flags(), self.s_inodes_count)
    }
}

/// Rafs v5 on disk inode offset table.
#[derive(Clone, Default)]
pub struct RafsV5InodeTable {
    /// Inode offset array.
    pub data: Vec<u32>,
}

impl RafsV5InodeTable {
    /// Create a new instance of `RafsV5InodeTable`.
    pub fn new(entries: usize) -> Self {
        let table_size = rafsv5_align(entries * size_of::<u32>()) / size_of::<u32>();
        RafsV5InodeTable {
            data: vec![0; table_size],
        }
    }

    /// Get size in bytes of the Rafs v5 inode table.
    #[inline]
    pub fn size(&self) -> usize {
        rafsv5_align(self.data.len() * size_of::<u32>())
    }

    /// Get number of inodes in the table.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check whether the table is empty or not.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Set inode offset in the metadata blob for an inode.
    pub fn set(&mut self, ino: Inode, offset: u32) -> Result<()> {
        if ino == 0 || ino > self.data.len() as u64 {
            return Err(einval!("invalid inode number"));
        } else if offset as usize <= RAFSV5_SUPERBLOCK_SIZE || offset & 0x7 != 0 {
            return Err(einval!("invalid inode offset"));
        }

        // The offset is aligned with 8 bytes to make it easier to validate RafsV5Inode.
        let offset = offset >> 3;
        self.data[(ino - 1) as usize] = u32::to_le(offset as u32);

        Ok(())
    }

    /// Get inode offset in the metadata blob of an inode.
    pub fn get(&self, ino: Inode) -> Result<u32> {
        if ino == 0 || ino > self.data.len() as u64 {
            return Err(enoent!());
        }

        let offset = u32::from_le(self.data[(ino - 1) as usize]) as usize;
        if offset <= (RAFSV5_SUPERBLOCK_SIZE >> 3) || offset >= (1usize << 29) {
            return Err(einval!("invalid inode offset"));
        }

        Ok((offset << 3) as u32)
    }

    /// Load inode offset table for a `RafsIoReader` object.
    pub fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        let (_, data, _) = unsafe { self.data.align_to_mut::<u8>() };
        r.read_exact(data)?;
        Ok(())
    }
}

impl RafsStore for RafsV5InodeTable {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        let (_, data, _) = unsafe { self.data.align_to::<u8>() };

        w.write_all(data)?;
        w.validate_alignment(data.len(), RAFSV5_ALIGNMENT)
    }
}

/// Rafs v5 on disk inode prefetch table.
///
/// From super block disk structure, its start offset can be told.
/// In order not to load every meta/inode to page cache under rafs Direct
/// mode, which aims at saving physical memory. This prefetch table is
/// introduce. Regular files or directories which are specified during image
/// building will have their inode index persist in this disk table.
/// For a single directory, only its inode index will be put into the table.
/// But all of its descendants files(recursively) will be prefetch(by hint)
/// when rafs is mounted at the very beginning.
#[derive(Clone, Default)]
pub struct RafsV5PrefetchTable {
    /// List of inode numbers for prefetch.
    /// Note: It's not inode index of inodes table being stored here.
    pub inodes: Vec<u32>,
}

impl RafsV5PrefetchTable {
    /// Create a new instance of `RafsV5PrefetchTable`.
    pub fn new() -> RafsV5PrefetchTable {
        RafsV5PrefetchTable { inodes: vec![] }
    }

    /// Get content size of the inode prefetch table.
    pub fn size(&self) -> usize {
        rafsv5_align(self.len() * size_of::<u32>())
    }

    /// Get number of entries in the prefetch table.
    pub fn len(&self) -> usize {
        self.inodes.len()
    }

    /// Check whether the inode prefetch table is empty.
    pub fn is_empty(&self) -> bool {
        self.inodes.is_empty()
    }

    /// Add an inode into the inode prefetch table.
    pub fn add_entry(&mut self, ino: u32) {
        self.inodes.push(ino);
    }

    /// Store the inode prefetch table to a writer.
    pub fn store(&mut self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        // Sort prefetch table by inode index, hopefully, it can save time when mounting rafs
        // Because file data is dumped in the order of inode index.
        self.inodes.sort_unstable();

        let (_, data, _) = unsafe { self.inodes.align_to::<u8>() };
        w.write_all(data.as_ref())?;

        // OK. Let's see if we have to align... :-(
        let cur_len = self.inodes.len() * size_of::<u32>();
        let padding_bytes = rafsv5_align(cur_len) - cur_len;
        w.write_padding(padding_bytes)?;

        Ok(data.len() + padding_bytes)
    }

    /// Load a inode prefetch table from a reader.
    ///
    /// Note: Generally, prefetch happens after loading bootstrap, so with methods operating
    /// files with changing their offset won't bring errors. But we still use `pread` now so as
    /// to make this method more stable and robust. Even dup(2) can't give us a separated file struct.
    pub fn load_prefetch_table_from(
        &mut self,
        r: &mut RafsIoReader,
        offset: u64,
        entries: usize,
    ) -> Result<usize> {
        self.inodes = vec![0u32; entries];

        let (_, data, _) = unsafe { self.inodes.align_to_mut::<u8>() };
        r.seek_to_offset(offset)?;
        r.read_exact(data)?;

        Ok(data.len())
    }
}

/// Rafs v5 blob description table.
#[derive(Clone, Debug, Default)]
pub struct RafsV5BlobTable {
    /// Base blob information array.
    pub entries: Vec<Arc<BlobInfo>>,
    /// Extended blob information array.
    pub extended: RafsV5ExtBlobTable,
}

impl RafsV5BlobTable {
    /// Create a new instance of `RafsV5BlobTable`.
    pub fn new() -> Self {
        RafsV5BlobTable {
            entries: Vec::new(),
            extended: RafsV5ExtBlobTable::new(),
        }
    }

    /// Get blob table size, aligned with RAFS_ALIGNMENT bytes
    pub fn size(&self) -> usize {
        if self.entries.is_empty() {
            return 0;
        }
        // Blob entry split with '\0'
        rafsv5_align(
            self.entries.iter().fold(0usize, |size, entry| {
                let entry_size = size_of::<u32>() * 2 + entry.blob_id().len();
                size + entry_size + 1
            }) - 1,
        )
    }

    /// Add information for new blob into the blob information table.
    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        blob_id: String,
        readahead_offset: u32,
        readahead_size: u32,
        chunk_size: u32,
        chunk_count: u32,
        uncompressed_size: u64,
        compressed_size: u64,
        blob_features: BlobFeatures,
        flags: RafsSuperFlags,
    ) -> u32 {
        let blob_index = self.entries.len() as u32;
        let mut blob_info = BlobInfo::new(
            blob_index,
            blob_id,
            uncompressed_size,
            compressed_size,
            chunk_size,
            chunk_count,
            blob_features,
        );

        blob_info.set_compressor(flags.into());
        blob_info.set_digester(flags.into());
        blob_info.set_readahead(readahead_offset as u64, readahead_size as u64);

        self.entries.push(Arc::new(blob_info));
        self.extended
            .add(chunk_count, uncompressed_size, compressed_size);

        blob_index
    }

    /// Get base information for a blob.
    #[inline]
    pub fn get(&self, blob_index: u32) -> Result<Arc<BlobInfo>> {
        if blob_index >= self.entries.len() as u32 {
            return Err(enoent!("blob not found"));
        }
        Ok(self.entries[blob_index as usize].clone())
    }

    /// Load blob information table from a reader.
    pub fn load(
        &mut self,
        r: &mut RafsIoReader,
        blob_table_size: u32,
        chunk_size: u32,
        flags: RafsSuperFlags,
    ) -> Result<()> {
        if blob_table_size == 0 {
            return Ok(());
        }

        debug!("blob table size {}", blob_table_size);
        let mut data = vec![0u8; blob_table_size as usize];
        r.read_exact(&mut data)?;

        // Each entry frame looks like:
        // u32 | u32 | string | trailing '\0' , except that the last entry has no trailing '\0'
        let mut buf = data.as_mut_slice();
        while buf.len() > 2 * size_of::<u32>() {
            let readahead_offset =
                unsafe { std::ptr::read_unaligned::<u32>(buf[0..4].as_ptr() as *const u32) };
            let readahead_size =
                unsafe { std::ptr::read_unaligned::<u32>(buf[4..8].as_ptr() as *const u32) };

            let mut pos = 8;
            while pos < buf.len() && buf[pos] != 0 {
                pos += 1;
            }
            let blob_id = std::str::from_utf8(&buf[8..pos])
                .map(|v| v.to_owned())
                .map_err(|e| einval!(e))?;
            if pos == buf.len() {
                buf = &mut buf[pos..];
            } else {
                buf = &mut buf[pos + 1..];
            }
            debug!("blob {:?} lies on", blob_id);

            let index = self.entries.len();
            let (chunk_count, uncompressed_size, compressed_size, blob_features) =
                // For compatibility, blob table might not be associated with extended blob table.
                if !self.extended.entries.is_empty() {
                    let ext_len = self.extended.entries.len();
                    if index >= ext_len {
                        error!( "Extended blob table({}) is shorter than blob table", ext_len);
                        return Err(einval!());
                    }
                    let entry = &self.extended.entries[index];
                    (entry.chunk_count, entry.uncompressed_size, entry.compressed_size, BlobFeatures::empty())
                } else {
                    (0, 0, 0, BlobFeatures::V5_NO_EXT_BLOB_TABLE)
                };

            let mut blob_info = BlobInfo::new(
                index as u32,
                blob_id,
                uncompressed_size,
                compressed_size,
                chunk_size,
                chunk_count,
                blob_features,
            );

            blob_info.set_compressor(flags.into());
            blob_info.set_digester(flags.into());
            blob_info.set_readahead(readahead_offset as u64, readahead_size as u64);

            self.entries.push(Arc::new(blob_info));
        }

        Ok(())
    }

    /// Get the base blob information array.
    pub fn get_all(&self) -> Vec<Arc<BlobInfo>> {
        self.entries.clone()
    }

    /// Store the extended blob information array.
    pub fn store_extended(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        self.extended.store(w)
    }
}

impl RafsStore for RafsV5BlobTable {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        let mut size = 0;
        self.entries
            .iter()
            .enumerate()
            .try_for_each::<_, Result<()>>(|(idx, entry)| {
                w.write_all(&u32::to_le_bytes(entry.readahead_offset() as u32))?;
                w.write_all(&u32::to_le_bytes(entry.readahead_size() as u32))?;
                w.write_all(entry.blob_id().as_bytes())?;
                if idx != self.entries.len() - 1 {
                    size += size_of::<u32>() * 2 + entry.blob_id().len() + 1;
                    w.write_all(&[b'\0'])?;
                } else {
                    size += size_of::<u32>() * 2 + entry.blob_id().len();
                }
                Ok(())
            })?;

        let padding = rafsv5_align(size) - size;
        w.write_padding(padding)?;
        size += padding;

        w.validate_alignment(size, RAFSV5_ALIGNMENT)
    }
}

/// Rafs v5 extended blob information on disk metadata.
///
/// RafsV5ExtDBlobEntry is appended to the tail of bootstrap,
/// can be used as an extended table for the original blob table.
// This disk structure is well defined and rafs aligned.
#[repr(C)]
#[derive(Clone)]
pub struct RafsV5ExtBlobEntry {
    /// Number of chunks in a blob file.
    pub chunk_count: u32,
    pub reserved1: [u8; 4],     //   --  8 Bytes
    pub uncompressed_size: u64, // -- 16 Bytes
    pub compressed_size: u64,   // -- 24 Bytes
    pub reserved2: [u8; RAFSV5_EXT_BLOB_RESERVED_SIZE],
}

// Implement Debug trait ourselves, as rust prior to 1.47 doesn't impl Debug for array with size
// larger than 32
impl Debug for RafsV5ExtBlobEntry {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        f.debug_struct("ExtendedBlobTableEntry")
            .field("chunk_count", &self.chunk_count)
            .field("blob_cache_size", &self.uncompressed_size)
            .field("compressed_blob_size", &self.compressed_size)
            .finish()
    }
}

impl Default for RafsV5ExtBlobEntry {
    fn default() -> Self {
        RafsV5ExtBlobEntry {
            chunk_count: 0,
            reserved1: [0; 4],
            uncompressed_size: 0,
            compressed_size: 0,
            reserved2: [0; RAFSV5_EXT_BLOB_RESERVED_SIZE],
        }
    }
}

impl RafsV5ExtBlobEntry {
    pub fn new(chunk_count: u32, blob_cache_size: u64, compressed_blob_size: u64) -> Self {
        Self {
            chunk_count,
            uncompressed_size: blob_cache_size,
            compressed_size: compressed_blob_size,
            ..Default::default()
        }
    }
}

/// Rafs v5 on disk extended blob information table.
#[derive(Clone, Debug, Default)]
pub struct RafsV5ExtBlobTable {
    /// The vector index means blob index, every entry represents
    /// extended information of a blob.
    pub entries: Vec<Arc<RafsV5ExtBlobEntry>>,
}

impl RafsV5ExtBlobTable {
    /// Create a new instance of `RafsV5ExtBlobTable`.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Get content size of the extended blob information table.
    pub fn size(&self) -> usize {
        // `ExtendedBlobTableEntry` is already a well defined disk structure and rafs-aligned
        // So directly use its `size_of()` is reliable.
        rafsv5_align(size_of::<RafsV5ExtBlobEntry>() * self.entries.len())
    }

    /// Get number of entries in the extended blob information table.
    pub fn entries(&self) -> usize {
        self.entries.len()
    }

    /// Add a new entry into the extended blob information table.
    pub fn add(&mut self, chunk_count: u32, blob_cache_size: u64, compressed_blob_size: u64) {
        self.entries.push(Arc::new(RafsV5ExtBlobEntry::new(
            chunk_count,
            blob_cache_size,
            compressed_blob_size,
        )));
    }

    /// Get extended information about a blob.
    pub fn get(&self, blob_index: u32) -> Option<Arc<RafsV5ExtBlobEntry>> {
        let len = self.entries.len();

        if len == 0 || blob_index as usize >= len {
            None
        } else {
            Some(self.entries[blob_index as usize].clone())
        }
    }

    /// Load extended blob information table from a reader.
    pub fn load(&mut self, r: &mut RafsIoReader, count: usize) -> Result<()> {
        let mut entries = Vec::<RafsV5ExtBlobEntry>::with_capacity(count);
        // Safe because it is already reserved enough space
        let (_, data, _) = unsafe {
            entries.set_len(count);
            (&mut entries).align_to_mut::<u8>()
        };

        r.read_exact(data)?;
        self.entries = entries.iter().cloned().map(Arc::new).collect();

        Ok(())
    }
}

impl RafsStore for RafsV5ExtBlobTable {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        let mut size = 0;

        // Store the list of entries
        self.entries
            .iter()
            .enumerate()
            .try_for_each::<_, Result<()>>(|(_idx, entry)| {
                w.write_all(&u32::to_le_bytes(entry.chunk_count))?;
                w.write_all(&entry.reserved1)?;
                w.write_all(&u64::to_le_bytes(entry.uncompressed_size))?;
                w.write_all(&u64::to_le_bytes(entry.compressed_size))?;
                w.write_all(&entry.reserved2)?;
                size += RAFSV5_EXT_BLOB_ENTRY_SIZE;
                Ok(())
            })?;

        // Append padding for RAFS alignment
        let padding = rafsv5_align(size) - size;
        w.write_padding(padding)?;
        size += padding;

        w.validate_alignment(size, RAFSV5_ALIGNMENT)
    }
}

bitflags! {
    /// Rafs v5 inode flags.
    pub struct RafsV5InodeFlags: u64 {
        /// Inode is a symlink.
        const SYMLINK = 0x0000_0001;
        /// Inode has hardlinks.
        const HARDLINK = 0x0000_0002;
        /// Inode has extended attributes.
        const XATTR = 0x0000_0004;
        /// Inode chunks has holes.
        const HAS_HOLE = 0x0000_0008;
   }
}

impl Default for RafsV5InodeFlags {
    fn default() -> Self {
        RafsV5InodeFlags::empty()
    }
}

/// Rafs v5 inode on disk metadata.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct RafsV5Inode {
    /// sha256(sha256(chunk) + ...), [char; RAFS_SHA256_LENGTH]
    pub i_digest: RafsDigest, // 32
    /// parent inode number
    pub i_parent: u64,
    /// Artifact inode number set by the nydus image builder. Start from RAFS_ROOT_INODE = 1.
    pub i_ino: u64,
    pub i_uid: u32,
    pub i_gid: u32,
    pub i_projid: u32,
    pub i_mode: u32, // 64
    pub i_size: u64,
    pub i_blocks: u64,
    pub i_flags: RafsV5InodeFlags,
    pub i_nlink: u32,
    /// for dir, child start index
    pub i_child_index: u32, // 96
    /// for dir, means child count.
    /// for regular file, means chunk info count.
    pub i_child_count: u32,
    /// file name size, [char; i_name_size]
    pub i_name_size: u16,
    /// symlink path size, [char; i_symlink_size]
    pub i_symlink_size: u16, // 104
    // inode device block number, ignored for non-special files
    pub i_rdev: u32,
    // for alignment reason, we put nsec first
    pub i_mtime_nsec: u32,
    pub i_mtime: u64,        // 120
    pub i_reserved: [u8; 8], // 128
}

impl RafsV5Inode {
    /// Create a new instance of `RafsV5Inode`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set size of the file name.
    #[inline]
    pub fn set_name_size(&mut self, name_len: usize) {
        self.i_name_size = name_len as u16;
    }

    /// Mark the inode as a symlink.
    #[inline]
    pub fn set_symlink_size(&mut self, symlink_len: usize) {
        self.i_symlink_size = symlink_len as u16;
    }

    /// Get on disk size of the inode content.
    #[inline]
    pub fn size(&self) -> usize {
        size_of::<Self>()
            + (rafsv5_align(self.i_name_size as usize) + rafsv5_align(self.i_symlink_size as usize))
                as usize
    }

    /// Get the uid and the gid of the inode.
    #[inline]
    pub fn uidgid(&self) -> (u32, u32) {
        (self.i_uid, self.i_gid)
    }

    /// Get the uid and the gid of the inode.
    #[inline]
    pub fn mtime(&self) -> (u64, u32) {
        (self.i_mtime, self.i_mtime_nsec)
    }

    /// Get the mode of the inode.
    #[inline]
    pub fn mode(&self) -> u32 {
        self.i_mode
    }

    /// Check whether the inode is a directory.
    #[inline]
    pub fn is_dir(&self) -> bool {
        self.i_mode & libc::S_IFMT as u32 == libc::S_IFDIR as u32
    }

    /// Check whether the inode is a symlink.
    #[inline]
    pub fn is_symlink(&self) -> bool {
        self.i_mode & libc::S_IFMT as u32 == libc::S_IFLNK as u32
    }

    /// Check whether the inode is a regular file.
    #[inline]
    pub fn is_reg(&self) -> bool {
        self.i_mode & libc::S_IFMT as u32 == libc::S_IFREG as u32
    }

    /// Check whether the inode is a hardlink.
    #[inline]
    pub fn is_hardlink(&self) -> bool {
        self.i_nlink > 1
    }

    /// Mark the inode as having extended attributes.
    #[inline]
    pub fn has_xattr(&self) -> bool {
        self.i_flags.contains(RafsV5InodeFlags::XATTR)
    }

    /// Mark the inode as having hole chunks.
    #[inline]
    pub fn has_hole(&self) -> bool {
        self.i_flags.contains(RafsV5InodeFlags::HAS_HOLE)
    }

    /// Load an inode from a reader.
    pub fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        r.read_exact(self.as_mut())
    }

    /// Set filename for the inode.
    pub fn load_file_name(&self, r: &mut RafsIoReader) -> Result<OsString> {
        let mut name_buf = vec![0u8; self.i_name_size as usize];
        r.read_exact(name_buf.as_mut_slice())?;
        r.seek_to_next_aligned(name_buf.len(), RAFSV5_ALIGNMENT)?;
        Ok(bytes_to_os_str(&name_buf).to_os_string())
    }
}

impl_bootstrap_converter!(RafsV5Inode);

/// A in-memory wrapper of a Rafs v5 inode.
pub struct RafsV5InodeWrapper<'a> {
    pub name: &'a OsStr,
    pub symlink: Option<&'a OsStr>,
    pub inode: &'a RafsV5Inode,
}

impl<'a> RafsStore for RafsV5InodeWrapper<'a> {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        let mut size: usize = 0;

        let inode_data = self.inode.as_ref();
        w.write_all(inode_data)?;
        size += inode_data.len();

        let name = self.name.as_bytes();
        w.write_all(name)?;
        size += name.len();
        let padding = rafsv5_align(self.inode.i_name_size as usize) - name.len();
        w.write_padding(padding)?;
        size += padding;

        if let Some(symlink) = self.symlink {
            let symlink_path = symlink.as_bytes();
            w.write_all(symlink_path)?;
            size += symlink_path.len();
            let padding = rafsv5_align(self.inode.i_symlink_size as usize) - symlink_path.len();
            w.write_padding(padding)?;
            size += padding;
        }

        w.validate_alignment(size, RAFSV5_ALIGNMENT)
    }
}

/// Rafs v5 chunk on disk metadata.
#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct RafsV5ChunkInfo {
    /// sha256(chunk), [char; RAFS_SHA256_LENGTH]
    pub block_id: RafsDigest, // 32
    /// blob index.
    pub blob_index: u32,
    /// chunk flags
    pub flags: BlobChunkFlags, // 40
    /// compressed size in blob
    pub compressed_size: u32,
    /// uncompressed size in blob
    pub uncompressed_size: u32, // 48
    /// compressed offset in blob
    pub compressed_offset: u64, // 56
    /// uncompressed offset in blob
    pub uncompressed_offset: u64, // 64
    /// offset in file
    pub file_offset: u64, // 72
    /// chunk index, it's allocated sequentially and starting from 0 for one blob.
    pub index: u32,
    /// reserved
    pub reserved: u32, //80
}

impl RafsV5ChunkInfo {
    /// Create a new instance of `RafsV5ChunkInfo`.
    pub fn new() -> Self {
        RafsV5ChunkInfo::default()
    }

    /// Load a Rafs v5 indoe from a reader.
    pub fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        r.read_exact(self.as_mut())
    }
}

impl RafsStore for RafsV5ChunkInfo {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        w.write_all(self.as_ref())?;
        w.validate_alignment(self.as_ref().len(), RAFSV5_ALIGNMENT)
    }
}

impl_bootstrap_converter!(RafsV5ChunkInfo);

impl Display for RafsV5ChunkInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "file_offset {}, compress_offset {}, compress_size {}, uncompress_offset {}, uncompress_size {}, blob_index {}, block_id {}, index {}, is_compressed {}",
            self.file_offset,
            self.compressed_offset,
            self.compressed_size,
            self.uncompressed_offset,
            self.uncompressed_size,
            self.blob_index,
            self.block_id,
            self.index,
            self.flags.contains(BlobChunkFlags::COMPRESSED),
        )
    }
}

/// Rafs v5 on disk extended attribute table.
///
/// A on disk Rafs v5 extended attribute table contains an u64 content size, followed by extended
/// attribute pairs.
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct RafsV5XAttrsTable {
    pub size: u64,
}

impl RafsV5XAttrsTable {
    /// Create a new instance of `RafsV5XAttrsTable`.
    pub fn new() -> Self {
        RafsV5XAttrsTable {
            ..Default::default()
        }
    }

    /// Get content size of the extended attribute table.
    #[inline]
    pub fn size(self) -> usize {
        self.size as usize
    }

    /// Get aligned content size of the extended attribute table.
    #[inline]
    pub fn aligned_size(self) -> usize {
        rafsv5_align(self.size())
    }
}

impl_bootstrap_converter!(RafsV5XAttrsTable);

impl RafsXAttrs {
    /// Get aligned content size of the extended attribute table.
    #[inline]
    pub fn aligned_size_v5(&self) -> usize {
        rafsv5_align(self.size())
    }

    pub fn store_v5(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        let mut size = 0;

        if !self.pairs.is_empty() {
            let size_data = (self.size() as u64).to_le_bytes();
            w.write_all(&size_data)?;
            size += size_data.len();

            for (key, value) in self.pairs.iter() {
                let pair_size = key.byte_size() + 1 + value.len();
                let pair_size_data = (pair_size as u32).to_le_bytes();
                w.write_all(&pair_size_data)?;
                size += pair_size_data.len();

                let key_data = key.as_bytes();
                w.write_all(key_data)?;
                w.write_all(&[0u8])?;
                size += key_data.len() + 1;

                w.write_all(value)?;
                size += value.len();
            }
        }

        let padding = rafsv5_align(size) - size;
        w.write_padding(padding)?;
        size += padding;

        w.validate_alignment(size, RAFSV5_ALIGNMENT)
    }
}

/// Allocate a group of `BlobIoVec` to handle blob io to range `offset..(offset+size)`.
///
/// The range `offset..(offset+size)` may be backed by multiple blobs, so a group of `BlobIoVec` will
/// be returned on success, each one covers a continuous range on a single blob.
pub(crate) fn rafsv5_alloc_bio_vecs<I: RafsInode + RafsV5InodeChunkOps + RafsV5InodeOps>(
    inode: &I,
    offset: u64,
    size: usize,
    user_io: bool,
) -> Result<Vec<BlobIoVec>> {
    let end = offset
        .checked_add(size as u64)
        .ok_or_else(|| einval!("invalid read size"))?;
    let (index_start, index_end) = calculate_bio_chunk_index(
        offset,
        end,
        inode.get_chunk_size() as u64,
        inode.get_child_count(),
        inode.has_hole(),
    );
    trace!(
        "alloc bio desc offset {} size {} i_size {} index_start {} index_end {} i_child_count {}",
        offset,
        size,
        inode.size(),
        index_start,
        index_end,
        inode.get_child_count()
    );
    if size == 0 || index_start >= inode.get_chunk_count() {
        return Ok(vec![]);
    }

    let mut descs = Vec::with_capacity(4);
    let mut desc = BlobIoVec::new();
    let chunk = inode.get_chunk_info_v5(index_start)?;
    let blob = inode.get_blob_by_index(chunk.blob_index())?;
    if !add_chunk_to_bio_desc(&mut desc, offset, end, chunk, blob, user_io) {
        return Err(einval!("failed to create blob io vector"));
    }
    for idx in index_start + 1..index_end {
        let chunk = inode.get_chunk_info_v5(idx)?;
        let blob = inode.get_blob_by_index(chunk.blob_index())?;
        if blob.blob_index() != desc.bi_vec[0].blob.blob_index() {
            descs.push(desc);
            desc = BlobIoVec::new();
        }
        if !add_chunk_to_bio_desc(&mut desc, offset, end, chunk, blob, user_io) {
            return Err(einval!("failed to create blob io vector"));
        }
    }
    descs.push(desc);

    Ok(descs)
}

/// Add a new bio covering the IO range into the provided bio desc.
///
/// Returns true if caller should continue checking more chunks.
///
/// # Parameters
/// - desc: the targeting bio desc.
/// - offset: IO offset to the file start, inclusive.
/// - end: IO end to the file start, exclusive.
/// - chunk: a data chunk overlapping with the IO range.
/// - chunk_size: chunk size.
/// - blob: the blob which the chunk data belongs to.
fn add_chunk_to_bio_desc(
    desc: &mut BlobIoVec,
    offset: u64,
    end: u64,
    chunk: Arc<dyn BlobV5ChunkInfo>,
    blob: Arc<BlobInfo>,
    user_io: bool,
) -> bool {
    // The chunk is ahead of the start of the range.
    if offset >= (chunk.file_offset() + chunk.uncompressed_size() as u64) {
        return true;
    }
    // The chunk is passing the end of the range.
    if end <= chunk.file_offset() {
        return false;
    }

    let chunk_start = if offset > chunk.file_offset() {
        offset - chunk.file_offset()
    } else {
        0
    };
    let chunk_end = if end < (chunk.file_offset() + chunk.uncompressed_size() as u64) {
        end - chunk.file_offset()
    } else {
        chunk.uncompressed_size() as u64
    };

    let io_chunk = Arc::new(V5IoChunk {
        // TODO: try to make `chunk_id` return Arc<RafsDigest> to get rid of potential memory copy
        block_id: Arc::new(*chunk.chunk_id()),
        blob_index: chunk.blob_index(),
        index: chunk.index(),
        compressed_offset: chunk.compressed_offset(),
        uncompressed_offset: chunk.uncompressed_offset(),
        compressed_size: chunk.compressed_size(),
        uncompressed_size: chunk.uncompressed_size(),
        flags: chunk.flags(),
    }) as Arc<dyn BlobChunkInfo>;

    let bio = BlobIoDesc::new(
        blob,
        BlobIoChunk::Base(io_chunk),
        chunk_start as u32,
        (chunk_end - chunk_start) as u32,
        user_io,
    );
    desc.bi_size += bio.size;
    desc.bi_vec.push(bio);

    true
}

/// Calculate bio chunk indices that overlaps with the provided IO range.
///
/// # Parameters
/// - offset: IO offset to the file start, inclusive.
/// - end: IO end to the file start, exclusive.
/// - chunk_size: chunk size.
/// - chunk_cnt: maximum number of chunks
/// - has_hole: whether a file has holes in it.
fn calculate_bio_chunk_index(
    offset: u64,
    end: u64,
    chunk_size: u64,
    chunk_cnt: u32,
    has_hole: bool,
) -> (u32, u32) {
    debug_assert!(offset < end);

    let index_start = if !has_hole {
        (offset / chunk_size) as u32
    } else {
        0
    };
    let index_end = if !has_hole {
        cmp::min(((end - 1) / chunk_size) as u32 + 1, chunk_cnt)
    } else {
        chunk_cnt
    };

    (index_start, index_end)
}

pub(crate) fn rafsv5_align(size: usize) -> usize {
    if size & (RAFSV5_ALIGNMENT - 1) == 0 {
        size
    } else {
        size + (RAFSV5_ALIGNMENT - (size & (RAFSV5_ALIGNMENT - 1)))
    }
}

/// Validate inode metadata, include children, chunks and symblink etc.
///
/// The default implementation is for rafs v5. The chunk data is not validated here, which will
/// be validate on fs read.
pub(crate) fn rafsv5_validate_digest(
    inode: Arc<dyn RafsInode>,
    recursive: bool,
    digester: digest::Algorithm,
) -> Result<bool> {
    let child_count = inode.get_child_count();
    let expected_digest = inode.get_digest();
    let mut hasher = RafsDigest::hasher(digester);

    if inode.is_symlink() {
        hasher.digest_update(inode.get_symlink()?.as_bytes());
    } else if inode.is_reg() {
        for idx in 0..child_count {
            let chunk = inode.get_chunk_info(idx)?;
            let chunk_digest = chunk.chunk_id();

            hasher.digest_update(chunk_digest.as_ref());
        }
    } else if inode.is_dir() {
        for idx in 0..child_count {
            let child = inode.get_child_by_index(idx)?;
            if (child.is_reg() || child.is_symlink() || (recursive && child.is_dir()))
                && !rafsv5_validate_digest(child.clone(), recursive, digester)?
            {
                return Ok(false);
            }
            let child_digest = child.get_digest();
            let child_digest = child_digest.as_ref();

            hasher.digest_update(child_digest);
        }
    }

    let digest = hasher.digest_finalize();
    let result = expected_digest == digest;
    if !result {
        error!(
            "invalid inode digest {}, expected {}, ino: {} name: {:?}",
            digest,
            expected_digest,
            inode.ino(),
            inode.name()
        );
    }

    Ok(result)
}

#[cfg(test)]
pub mod tests {
    use std::fs::OpenOptions;
    use std::io::BufWriter;
    use std::io::{SeekFrom, Write};

    use storage::device::BlobChunkInfo;
    use vmm_sys_util::tempfile::TempFile;

    use super::*;
    use crate::metadata::RafsStore;
    use crate::{RafsIoRead, RafsIoReader};
    use std::any::Any;
    use std::str::FromStr;

    struct Entry {
        foo: u32,
        bar: u32,
    }

    unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
        ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>())
    }

    #[test]
    fn test_load_blob_table() {
        let mut buffer = Vec::new();
        let first = Entry { foo: 1, bar: 2 };
        let second = Entry { foo: 3, bar: 4 };
        let third = Entry { foo: 5, bar: 6 };

        let first_id = "355d403e35d7120cbd6a145874a2705e6842ce9974985013ebdc1fa5199a0184";
        let second_id = "19ebb6e9bdcbbce3f24d694fe20e0e552ae705ce079e26023ad0ecd61d4b130019ebb6e9bdcbbce3f24d694fe20e0e552ae705ce079e26023ad0ecd61d4";
        let third_id = "19ebb6e9bdcbbce3f24d694fe20e0e552ae705ce079e";

        let first_slice = unsafe { any_as_u8_slice(&first) };
        let second_slice = unsafe { any_as_u8_slice(&second) };
        let third_slice = unsafe { any_as_u8_slice(&third) };

        buffer.extend_from_slice(first_slice);
        buffer.extend_from_slice(first_id.as_bytes());
        buffer.push(b'\0');
        buffer.extend_from_slice(second_slice);
        buffer.extend_from_slice(second_id.as_bytes());
        buffer.push(b'\0');
        buffer.extend_from_slice(third_slice);
        buffer.extend_from_slice(third_id.as_bytes());
        // buffer.push(b'\0');

        let tmp_file = TempFile::new().unwrap();

        // Store extended blob table
        let mut tmp_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(tmp_file.as_path())
            .unwrap();
        tmp_file.write_all(&buffer).unwrap();
        tmp_file.flush().unwrap();

        let mut file: RafsIoReader = Box::new(tmp_file);
        let mut blob_table = RafsV5BlobTable::new();

        file.seek(SeekFrom::Start(0)).unwrap();
        blob_table
            .load(
                &mut file,
                buffer.len() as u32,
                RAFS_DEFAULT_CHUNK_SIZE as u32,
                RafsSuperFlags::empty(),
            )
            .unwrap();
        for b in &blob_table.entries {
            let _c = b.clone();
            trace!("{:?}", _c);
        }

        assert_eq!(first.bar, first.foo + 1);
        assert_eq!(blob_table.size(), rafsv5_align(buffer.len()));
        assert_eq!(blob_table.get(0).unwrap().blob_id(), first_id);
        assert_eq!(blob_table.get(1).unwrap().blob_id(), second_id);
        assert_eq!(blob_table.get(2).unwrap().blob_id(), third_id);
        assert!(blob_table.get(3).is_err());
        assert_eq!(blob_table.get_all().len(), 3);

        blob_table.entries.truncate(0);
        file.seek(SeekFrom::Start(0)).unwrap();
        blob_table
            .load(
                &mut file,
                0,
                RAFS_DEFAULT_CHUNK_SIZE as u32,
                RafsSuperFlags::empty(),
            )
            .unwrap();
        assert_eq!(blob_table.size(), 0);
        assert_eq!(blob_table.entries.len(), 0);
        assert!(blob_table.get(0).is_err());

        blob_table.entries.truncate(0);
        file.seek(SeekFrom::Start(0)).unwrap();
        blob_table
            .load(
                &mut file,
                (buffer.len() - 100) as u32,
                RAFS_DEFAULT_CHUNK_SIZE as u32,
                RafsSuperFlags::empty(),
            )
            .unwrap();
        assert_eq!(blob_table.entries[0].blob_id(), first_id);
        assert_eq!(blob_table.get_all().len(), 2);
    }

    #[test]
    fn test_extended_blob_table() {
        let tmp_file = TempFile::new().unwrap();

        // Create extended blob table
        let mut table = RafsV5ExtBlobTable::new();
        for i in 0..5 {
            table.add(i * 3, 100, 100);
        }

        // Store extended blob table
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(tmp_file.as_path())
            .unwrap();
        let mut writer = BufWriter::new(file);
        table.store(&mut writer).unwrap();

        // Load extended blob table
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(tmp_file.as_path())
            .unwrap();
        let mut reader = Box::new(file) as Box<dyn RafsIoRead>;
        let mut table = RafsV5ExtBlobTable::new();
        table.load(&mut reader, 5).unwrap();

        assert_eq!(table.size(), 5 * RAFSV5_EXT_BLOB_ENTRY_SIZE);
        assert_eq!(table.entries(), 5);
        assert!(table.get(0).is_some());
        assert!(table.get(4).is_some());
        assert!(table.get(5).is_none());

        // Check expected blob table
        for i in 0..5 {
            assert_eq!(table.get(i).unwrap().chunk_count, i * 3);
            assert_eq!(table.get(i).unwrap().reserved1, [0u8; 4]);
            assert_eq!(table.get(i).unwrap().uncompressed_size, 100);
            assert_eq!(
                table.get(i).unwrap().reserved2,
                [0u8; RAFSV5_EXT_BLOB_RESERVED_SIZE]
            );
        }
    }

    #[derive(Default, Copy, Clone)]
    struct MockChunkInfo {
        pub block_id: RafsDigest,
        pub blob_index: u32,
        pub flags: BlobChunkFlags,
        pub compress_size: u32,
        pub uncompress_size: u32,
        pub compress_offset: u64,
        pub uncompress_offset: u64,
        pub file_offset: u64,
        pub index: u32,
        #[allow(unused)]
        pub reserved: u32,
    }

    impl MockChunkInfo {
        fn new() -> Self {
            MockChunkInfo::default()
        }
    }

    impl BlobChunkInfo for MockChunkInfo {
        fn chunk_id(&self) -> &RafsDigest {
            &self.block_id
        }

        fn id(&self) -> u32 {
            self.index
        }

        fn is_compressed(&self) -> bool {
            self.flags.contains(BlobChunkFlags::COMPRESSED)
        }

        fn is_hole(&self) -> bool {
            self.flags.contains(BlobChunkFlags::HOLECHUNK)
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        impl_getter!(blob_index, blob_index, u32);
        impl_getter!(compressed_offset, compress_offset, u64);
        impl_getter!(compressed_size, compress_size, u32);
        impl_getter!(uncompressed_offset, uncompress_offset, u64);
        impl_getter!(uncompressed_size, uncompress_size, u32);
    }

    impl BlobV5ChunkInfo for MockChunkInfo {
        fn as_base(&self) -> &dyn BlobChunkInfo {
            self
        }

        impl_getter!(index, index, u32);
        impl_getter!(file_offset, file_offset, u64);
        impl_getter!(flags, flags, BlobChunkFlags);
    }

    #[test]
    fn test_add_chunk_to_bio_desc() {
        let mut chunk = MockChunkInfo::new();
        let offset = 4096;
        let size: u64 = 1024;
        // [offset, offset + size)
        chunk.file_offset = offset;
        chunk.uncompress_size = size as u32;

        // (offset, end, expected_chunk_start, expected_size)
        let data = vec![
            // Non-overlapping IO
            (0, 0, 0, 0, false),
            (0, offset, 0, 0, false),
            (offset + size, 0, 0, 0, true),
            (offset + size + 1, 0, 0, 0, true),
            // Overlapping IO
            (0, offset + 1, 0, 1, true),
            (0, offset + size, 0, size, true),
            (0, offset + size + 1, 0, size, true),
            (0, offset + size - 1, 0, size - 1, true),
            (offset, offset + 1, 0, 1, true),
            (offset, offset + size, 0, size, true),
            (offset, offset + size - 1, 0, size - 1, true),
            (offset, offset + size + 1, 0, size, true),
            (offset + 1, offset + 2, 1, 1, true),
            (offset + 1, offset + size, 1, size - 1, true),
            (offset + 1, offset + size - 1, 1, size - 2, true),
            (offset + 1, offset + size + 1, 1, size - 1, true),
        ];

        for (offset, end, expected_chunk_start, expected_size, result) in data.iter() {
            let mut desc = BlobIoVec::new();
            let blob = Arc::new(BlobInfo::new(
                0,
                String::from("blobid"),
                0,
                0,
                0,
                0,
                BlobFeatures::V5_NO_EXT_BLOB_TABLE,
            ));
            let res = add_chunk_to_bio_desc(&mut desc, *offset, *end, Arc::new(chunk), blob, true);
            assert_eq!(*result, res);
            if !desc.bi_vec.is_empty() {
                assert_eq!(desc.bi_vec.len(), 1);
                let bio = &desc.bi_vec[0];
                assert_eq!(*expected_chunk_start, bio.offset);
                assert_eq!(*expected_size as u32, bio.size);
            }
        }
    }

    #[test]
    fn test_calculate_bio_chunk_index() {
        let (blksize, chunk_cnt) = (1024, 4);

        let io_range: Vec<(u64, u64, u32, u64)> = vec![
            (0, 1, 0, 1),
            (0, blksize - 1, 0, 1),
            (0, blksize, 0, 1),
            (0, blksize + 1, 0, 2),
            (0, blksize * chunk_cnt, 0, chunk_cnt),
            (0, blksize * chunk_cnt + 1, 0, chunk_cnt),
            (0, blksize * chunk_cnt - 1, 0, chunk_cnt),
            (blksize - 1, 1, 0, 1),
            (blksize - 1, 2, 0, 2),
            (blksize - 1, 3, 0, 2),
            (blksize - 1, blksize - 1, 0, 2),
            (blksize - 1, blksize, 0, 2),
            (blksize - 1, blksize + 1, 0, 2),
            (blksize - 1, blksize * chunk_cnt, 0, chunk_cnt),
            (blksize, 1, 1, 2),
            (blksize, 2, 1, 2),
            (blksize, blksize - 1, 1, 2),
            (blksize, blksize + 1, 1, 3),
            (blksize, blksize + 2, 1, 3),
            (blksize, blksize * chunk_cnt, 1, chunk_cnt),
            (blksize + 1, 1, 1, 2),
            (blksize + 1, blksize - 2, 1, 2),
            (blksize + 1, blksize - 1, 1, 2),
            (blksize + 1, blksize, 1, 3),
            (blksize + 1, blksize * chunk_cnt, 1, chunk_cnt),
        ];

        for (io_start, io_size, expected_start, expected_end) in io_range.iter() {
            let (start, end) = calculate_bio_chunk_index(
                *io_start,
                *io_start + *io_size,
                blksize,
                chunk_cnt as u32,
                false,
            );

            assert_eq!(start, *expected_start);
            assert_eq!(end, *expected_end as u32);
        }
    }

    #[test]
    fn test_rafsv5_align() {
        assert_eq!(rafsv5_align(0), 0);
        assert_eq!(rafsv5_align(1), 8);
        assert_eq!(rafsv5_align(7), 8);
        assert_eq!(rafsv5_align(8), 8);
        assert_eq!(rafsv5_align(9), 16);
    }

    #[test]
    fn test_rafsv5_superflags() {
        assert_eq!(
            RafsSuperFlags::from(digest::Algorithm::Blake3),
            RafsSuperFlags::DIGESTER_BLAKE3
        );
        assert_eq!(
            RafsSuperFlags::from(digest::Algorithm::Sha256),
            RafsSuperFlags::DIGESTER_SHA256
        );
        assert_eq!(
            digest::Algorithm::from(RafsSuperFlags::DIGESTER_BLAKE3),
            digest::Algorithm::Blake3
        );
        assert_eq!(
            digest::Algorithm::from(RafsSuperFlags::DIGESTER_SHA256),
            digest::Algorithm::Sha256
        );

        assert_eq!(
            RafsSuperFlags::from(compress::Algorithm::Zstd),
            RafsSuperFlags::COMPRESS_ZSTD
        );
        assert_eq!(
            RafsSuperFlags::from(compress::Algorithm::GZip),
            RafsSuperFlags::COMPRESS_GZIP
        );
        assert_eq!(
            RafsSuperFlags::from(compress::Algorithm::Lz4Block),
            RafsSuperFlags::COMPRESS_LZ4_BLOCK
        );
        assert_eq!(
            RafsSuperFlags::from(compress::Algorithm::None),
            RafsSuperFlags::COMPRESS_NONE
        );
        assert_eq!(
            compress::Algorithm::from(RafsSuperFlags::COMPRESS_ZSTD),
            compress::Algorithm::Zstd
        );
        assert_eq!(
            compress::Algorithm::from(RafsSuperFlags::COMPRESS_GZIP),
            compress::Algorithm::GZip
        );
        assert_eq!(
            compress::Algorithm::from(RafsSuperFlags::COMPRESS_LZ4_BLOCK),
            compress::Algorithm::Lz4Block
        );
        assert_eq!(
            compress::Algorithm::from(RafsSuperFlags::COMPRESS_NONE),
            compress::Algorithm::None
        );
    }

    #[test]
    fn test_rafsv5_inode_table() {
        let mut table = RafsV5InodeTable::new(1);
        assert_eq!(table.size(), 8);
        assert_eq!(table.len(), 2);

        assert!(table.set(0, 0x2000).is_err());
        assert!(table.set(2, 0x2000).is_err());
        assert!(table.set(1, 0x1000).is_err());
        assert!(table.set(1, 0x2001).is_err());

        assert!(table.get(0).is_err());
        assert!(table.get(2).is_err());
        assert!(table.get(1).is_err());
        table.data[1] = 0x1000;
        assert!(table.get(1).is_err());
        table.data[1] = 0x1 << 30;
        assert!(table.get(1).is_err());
        assert!(table.set(1, 0x2008).is_ok());
        assert_eq!(table.get(1).unwrap(), 0x2008);
    }

    #[test]
    fn test_rafsv5_prefetch_table() {
        let mut table = RafsV5PrefetchTable::new();

        assert_eq!(table.size(), 0);
        assert_eq!(table.len(), 0);
        assert!(table.is_empty());
        table.add_entry(0x1);
        assert_eq!(table.size(), 8);
        assert_eq!(table.len(), 1);
        assert!(!table.is_empty());

        let tmp_file = TempFile::new().unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(tmp_file.as_path())
            .unwrap();
        let mut writer = BufWriter::new(file);
        writer.write_all(&[0u8; 8]).unwrap();
        assert_eq!(table.store(&mut writer).unwrap(), 8);
        writer.flush().unwrap();

        // Load extended blob table
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(tmp_file.as_path())
            .unwrap();
        let mut reader = Box::new(file) as Box<dyn RafsIoRead>;
        let mut table = RafsV5PrefetchTable::new();
        table.load_prefetch_table_from(&mut reader, 8, 2).unwrap();
        assert_eq!(table.size(), 8);
        assert_eq!(table.len(), 2);
        assert!(!table.is_empty());
        assert_eq!(table.inodes[0], 0x1);
        assert_eq!(table.inodes[1], 0x0);
    }

    #[test]
    fn test_new_inode() {
        let mut inode = RafsV5Inode::new();
        inode.set_name_size(3);
        assert_eq!(inode.size(), 136);
        assert!(!inode.is_symlink());
        assert!(!inode.is_hardlink());
        assert!(!inode.is_dir());
        assert!(!inode.is_reg());
        assert!(!inode.has_hole());
        assert!(!inode.has_xattr());

        let mut inode = RafsV5Inode::new();
        inode.set_symlink_size(3);
        assert_eq!(inode.size(), 136);
    }

    #[test]
    fn test_inode_load_store() {
        let mut inode = RafsV5Inode::new();
        inode.i_size = 0x1000;
        inode.i_blocks = 1;
        inode.i_child_count = 10;
        inode.i_child_index = 20;
        inode.set_name_size(4);
        inode.set_symlink_size(6);
        inode.i_flags = RafsV5InodeFlags::SYMLINK;

        let name = OsString::from_str("test").unwrap();
        let symlink = OsString::from_str("/test12").unwrap();
        let inode_wrapper = RafsV5InodeWrapper {
            name: &name,
            symlink: Some(&symlink),
            inode: &inode,
        };

        let tmp_file = TempFile::new().unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(tmp_file.as_path())
            .unwrap();
        let mut writer = BufWriter::new(file);
        assert_eq!(inode_wrapper.store(&mut writer).unwrap(), 144);
        writer.flush().unwrap();

        // Load inode
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(tmp_file.as_path())
            .unwrap();
        let mut reader = Box::new(file) as Box<dyn RafsIoRead>;
        let mut inode2 = RafsV5Inode::new();
        inode2.load(&mut reader).unwrap();
        assert_eq!(inode2.i_name_size, 4);
        assert_eq!(inode2.i_symlink_size, 6);
        assert_eq!(inode2.i_size, 0x1000);
        assert_eq!(inode2.i_blocks, 1);
        assert_eq!(inode2.i_child_count, 10);
        assert_eq!(inode2.i_child_index, 20);

        let filename = inode2.load_file_name(&mut reader).unwrap();
        assert_eq!(filename, OsString::from_str("test").unwrap());
    }

    #[test]
    fn test_rafsv5_new_xattrs() {
        let mut xattrs = RafsXAttrs::new();
        assert_eq!(xattrs.size(), 0);

        xattrs.add(OsString::from("key1"), vec![0x1u8, 0x2, 0x3, 0x4]);
        assert_eq!(xattrs.size(), 13);
        xattrs.add(OsString::from("key21"), vec![0x1u8, 0x2, 0x3, 0x4]);
        assert_eq!(xattrs.size(), 27);
        xattrs.remove(&OsString::from("key1"));
        assert_eq!(xattrs.size(), 14);
    }
}
