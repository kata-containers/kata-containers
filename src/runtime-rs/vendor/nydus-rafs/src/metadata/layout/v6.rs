// Copyright 2020-2021 Ant Group. All rights reserved.
// Copyright (C) 2020-2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::convert::{TryFrom, TryInto};
use std::ffi::{OsStr, OsString};
use std::fmt::Debug;
use std::io::{Read, Result};
use std::mem::size_of;
use std::os::unix::ffi::OsStrExt;
use std::str::FromStr;
use std::sync::Arc;

use lazy_static::lazy_static;

use nydus_utils::{compress, digest, round_up, ByteSize};
use storage::device::{BlobFeatures, BlobInfo};
use storage::meta::{BlobMetaHeaderOndisk, BLOB_FEATURE_4K_ALIGNED};
use storage::RAFS_MAX_CHUNK_SIZE;

use crate::metadata::{layout::RafsXAttrs, RafsStore, RafsSuperFlags};
use crate::{impl_bootstrap_converter, impl_pub_getter_setter, RafsIoReader, RafsIoWrite};

/// EROFS metadata slot size.
pub const EROFS_INODE_SLOT_SIZE: usize = 1 << EROFS_INODE_SLOT_BITS;
/// EROFS logical block size.
pub const EROFS_BLOCK_SIZE: u64 = 1u64 << EROFS_BLOCK_BITS;
/// EROFS plain inode.
pub const EROFS_INODE_FLAT_PLAIN: u16 = 0;
/// EROFS inline inode.
pub const EROFS_INODE_FLAT_INLINE: u16 = 2;
/// EROFS chunked inode.
pub const EROFS_INODE_CHUNK_BASED: u16 = 4;
/// EROFS device table offset.
pub const EROFS_DEVTABLE_OFFSET: u16 =
    EROFS_SUPER_OFFSET + EROFS_SUPER_BLOCK_SIZE + EROFS_EXT_SUPER_BLOCK_SIZE;

pub const EROFS_I_VERSION_BIT: u16 = 0;
pub const EROFS_I_VERSION_BITS: u16 = 1;
pub const EROFS_I_DATALAYOUT_BITS: u16 = 3;

// Offset of EROFS super block.
pub const EROFS_SUPER_OFFSET: u16 = 1024;
// Size of EROFS super block.
pub const EROFS_SUPER_BLOCK_SIZE: u16 = 128;
// Size of extended super block, used for rafs v6 specific fields
const EROFS_EXT_SUPER_BLOCK_SIZE: u16 = 256;
// Magic number for EROFS super block.
const EROFS_SUPER_MAGIC_V1: u32 = 0xE0F5_E1E2;
// Bits of EROFS logical block size.
const EROFS_BLOCK_BITS: u8 = 12;
// Bits of EROFS metadata slot size.
const EROFS_INODE_SLOT_BITS: u8 = 5;
// 32-byte on-disk inode
#[allow(dead_code)]
const EROFS_INODE_LAYOUT_COMPACT: u16 = 0;
// 64-byte on-disk inode
const EROFS_INODE_LAYOUT_EXTENDED: u16 = 1;
// Bit flag indicating whether the inode is chunked or not.
const EROFS_CHUNK_FORMAT_INDEXES_FLAG: u16 = 0x0020;
// Encoded chunk size (log2(chunk_size) - EROFS_BLOCK_BITS).
const EROFS_CHUNK_FORMAT_SIZE_MASK: u16 = 0x001F;
/// Checksum of superblock, compatible with EROFS versions prior to Linux kernel 5.5.
#[allow(dead_code)]
const EROFS_FEATURE_COMPAT_SB_CHKSUM: u32 = 0x0000_0001;
/// Rafs v6 specific metadata, compatible with EROFS versions since Linux kernel 5.16.
const EROFS_FEATURE_COMPAT_RAFS_V6: u32 = 0x4000_0000;
/// Chunked inode, incompatible with EROFS versions prior to Linux kernel 5.15.
const EROFS_FEATURE_INCOMPAT_CHUNKED_FILE: u32 = 0x0000_0004;
/// Multi-devices, incompatible with EROFS versions prior to Linux kernel 5.16.
const EROFS_FEATURE_INCOMPAT_DEVICE_TABLE: u32 = 0x0000_0008;
/// Size of SHA256 digest string.
const BLOB_SHA256_LEN: usize = 64;
const BLOB_MAX_SIZE: u64 = 1u64 << 44;

/// RAFS v6 superblock on-disk format, 128 bytes.
///
/// The structure is designed to be compatible with EROFS superblock, so the in kernel EROFS file
/// system driver could be used to mount a RAFS v6 image.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RafsV6SuperBlock {
    /// File system magic number
    s_magic: u32,
    /// Crc32 checksum of the superblock, ignored by Rafs v6.
    s_checksum: u32,
    /// Compatible filesystem features.
    s_feature_compat: u32,
    /// Bits of block size. Only 12 is supported, thus block_size == PAGE_SIZE(4096).
    s_blkszbits: u8,
    /// Number of extended superblock slots, ignored by Rafs v6.
    /// `superblock size = 128(size of RafsV6SuperBlock) + s_extslots * 16`.
    s_extslots: u8,
    /// Nid of the root directory.
    /// `root inode offset = s_meta_blkaddr * 4096 + s_root_nid * 32`.
    pub s_root_nid: u16,
    /// Total valid ino #
    s_inos: u64,
    /// Timestamp of filesystem creation.
    s_build_time: u64,
    /// Timestamp of filesystem creation.
    s_build_time_nsec: u32,
    /// Total size of file system in blocks, used for statfs
    s_blocks: u32,
    /// Start block address of the metadata area.
    pub s_meta_blkaddr: u32,
    /// Start block address of the shared xattr area.
    s_xattr_blkaddr: u32,
    /// 128-bit uuid for volume
    s_uuid: [u8; 16],
    /// Volume name.
    s_volume_name: [u8; 16],
    /// Incompatible filesystem feature flags.
    s_feature_incompat: u32,
    /// A union of `u16` for miscellaneous usage.
    s_u: u16,
    /// # of devices besides the primary device.
    s_extra_devices: u16,
    /// Offset of the device table, `startoff = s_devt_slotoff * 128`.
    s_devt_slotoff: u16,
    /// Padding.
    s_reserved: [u8; 38],
}

impl_bootstrap_converter!(RafsV6SuperBlock);

impl RafsV6SuperBlock {
    /// Create a new instance of `RafsV6SuperBlock`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a `RafsV6SuperBlock` from a reader.
    pub fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        let mut buf1 = [0u8; EROFS_SUPER_OFFSET as usize];

        r.read_exact(&mut buf1)?;
        r.read_exact(self.as_mut())
    }

    /// Validate the Rafs v6 super block.
    pub fn validate(&self, meta_size: u64) -> Result<()> {
        if meta_size < EROFS_BLOCK_SIZE {
            return Err(einval!(format!(
                "invalid Rafs v6 metadata size: {}",
                meta_size
            )));
        }

        if meta_size & (EROFS_BLOCK_SIZE - 1) != 0 {
            return Err(einval!(format!(
                "invalid Rafs v6 metadata size: bootstrap size {} is not aligned",
                meta_size
            )));
        }

        if u32::from_le(self.s_checksum) != 0 {
            return Err(einval!(format!(
                "invalid checksum {} in Rafsv6 superblock",
                u32::from_le(self.s_checksum)
            )));
        }

        if self.s_blkszbits != EROFS_BLOCK_BITS {
            return Err(einval!(format!(
                "invalid block size bits {} in Rafsv6 superblock",
                self.s_blkszbits
            )));
        }

        if u64::from_le(self.s_inos) == 0 {
            return Err(einval!("invalid inode number in Rafsv6 superblock"));
        }

        if self.s_extslots != 0 {
            return Err(einval!("invalid extended slots in Rafsv6 superblock"));
        }

        if self.s_blocks == 0 {
            return Err(einval!("invalid blocks in Rafsv6 superblock"));
        }

        if u16::from_le(self.s_u) != 0 {
            return Err(einval!("invalid union field in Rafsv6 superblock"));
        }

        // s_build_time may be used as compact_inode's timestamp in the future.
        // if u64::from_le(self.s_build_time) != 0 || u32::from_le(self.s_build_time_nsec) != 0 {
        //     return Err(einval!("invalid build time in Rafsv6 superblock"));
        // }

        if u32::from_le(self.s_feature_incompat)
            != EROFS_FEATURE_INCOMPAT_CHUNKED_FILE | EROFS_FEATURE_INCOMPAT_DEVICE_TABLE
        {
            return Err(einval!(
                "invalid incompatible feature bits in Rafsv6 superblock"
            ));
        }

        if u32::from_le(self.s_feature_compat) & EROFS_FEATURE_COMPAT_RAFS_V6
            != EROFS_FEATURE_COMPAT_RAFS_V6
        {
            return Err(einval!(
                "invalid compatible feature bits in Rafsv6 superblock"
            ));
        }

        Ok(())
    }

    /// Check whether it's super block for Rafs v6.
    pub fn is_rafs_v6(&self) -> bool {
        self.magic() == EROFS_SUPER_MAGIC_V1
    }

    /// Set number of inodes.
    pub fn set_inos(&mut self, inos: u64) {
        self.s_inos = inos.to_le();
    }

    /// Get total inodes count of this Rafs
    pub fn inodes_count(&self) -> u64 {
        self.s_inos
    }

    /// Set number of logical blocks.
    pub fn set_blocks(&mut self, blocks: u32) {
        self.s_blocks = blocks.to_le();
    }

    /// Set EROFS root nid.
    pub fn set_root_nid(&mut self, nid: u16) {
        self.s_root_nid = nid.to_le();
    }

    /// Set EROFS meta block address.
    pub fn set_meta_addr(&mut self, meta_addr: u64) {
        debug_assert!(((meta_addr / EROFS_BLOCK_SIZE) >> 32) == 0);
        self.s_meta_blkaddr = u32::to_le((meta_addr / EROFS_BLOCK_SIZE) as u32);
    }

    /// Set number of extra devices.
    pub fn set_extra_devices(&mut self, count: u16) {
        self.s_extra_devices = count.to_le();
    }

    impl_pub_getter_setter!(magic, set_magic, s_magic, u32);
}

impl RafsStore for RafsV6SuperBlock {
    // This method must be called before RafsV6SuperBlockExt::store(), otherwise data written by
    // RafsV6SuperBlockExt::store() will be overwritten.
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        debug_assert!(((EROFS_SUPER_OFFSET + EROFS_SUPER_BLOCK_SIZE) as u64) < EROFS_BLOCK_SIZE);
        w.write_all(&[0u8; EROFS_SUPER_OFFSET as usize])?;
        w.write_all(self.as_ref())?;
        w.write_all(
            &[0u8; (EROFS_BLOCK_SIZE as usize
                - (EROFS_SUPER_OFFSET + EROFS_SUPER_BLOCK_SIZE) as usize)],
        )?;

        Ok(EROFS_BLOCK_SIZE as usize)
    }
}

impl Default for RafsV6SuperBlock {
    fn default() -> Self {
        debug_assert!(size_of::<RafsV6Device>() == 128);
        Self {
            s_magic: u32::to_le(EROFS_SUPER_MAGIC_V1),
            s_checksum: u32::to_le(0),
            s_feature_compat: u32::to_le(EROFS_FEATURE_COMPAT_RAFS_V6),
            s_blkszbits: EROFS_BLOCK_BITS,
            s_extslots: 0u8,
            s_root_nid: u16::to_le(0),
            s_inos: u64::to_le(0),
            s_build_time: u64::to_le(0),
            s_build_time_nsec: u32::to_le(0),
            s_blocks: u32::to_le(1),
            s_meta_blkaddr: u32::to_le(0),
            s_xattr_blkaddr: u32::to_le(0),
            s_uuid: [0u8; 16],
            s_volume_name: [0u8; 16],
            s_feature_incompat: u32::to_le(
                EROFS_FEATURE_INCOMPAT_CHUNKED_FILE | EROFS_FEATURE_INCOMPAT_DEVICE_TABLE,
            ),
            s_u: u16::to_le(0),
            s_extra_devices: u16::to_le(0),
            s_devt_slotoff: u16::to_le(EROFS_DEVTABLE_OFFSET / size_of::<RafsV6Device>() as u16),
            s_reserved: [0u8; 38],
        }
    }
}

/// Extended superblock, 256 bytes
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RafsV6SuperBlockExt {
    /// superblock flags
    s_flags: u64,
    /// offset of blob table
    s_blob_table_offset: u64,
    /// size of blob table
    s_blob_table_size: u32,
    /// chunk size
    s_chunk_size: u32,
    /// offset of chunk table
    s_chunk_table_offset: u64,
    /// size of chunk table
    s_chunk_table_size: u64,
    s_prefetch_table_offset: u64,
    s_prefetch_table_size: u32,
    s_padding: u32,
    /// Reserved
    s_reserved: [u8; 200],
}

impl_bootstrap_converter!(RafsV6SuperBlockExt);

impl RafsV6SuperBlockExt {
    /// Create a new instance `RafsV6SuperBlockExt`.
    pub fn new() -> Self {
        debug_assert!(size_of::<Self>() == 256);
        Self::default()
    }

    /// Load an `RafsV6SuperBlockExt` from a reader.
    pub fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        r.seek_to_offset((EROFS_SUPER_OFFSET + EROFS_SUPER_BLOCK_SIZE) as u64)?;
        r.read_exact(self.as_mut())?;
        r.seek_to_offset(EROFS_BLOCK_SIZE as u64)?;

        Ok(())
    }

    /// Validate the Rafs v6 super block.
    pub fn validate(&self) -> Result<()> {
        let mut flags = self.flags();
        flags &= RafsSuperFlags::COMPRESS_NONE.bits()
            | RafsSuperFlags::COMPRESS_LZ4_BLOCK.bits()
            | RafsSuperFlags::COMPRESS_GZIP.bits()
            | RafsSuperFlags::COMPRESS_ZSTD.bits();
        if flags.count_ones() != 1 {
            return Err(einval!(format!(
                "invalid flags {:#x} related to compression algorithm in Rafs v6 extended superblock",
                flags
            )));
        }

        let mut flags = self.flags();
        flags &= RafsSuperFlags::DIGESTER_BLAKE3.bits() | RafsSuperFlags::DIGESTER_SHA256.bits();
        if flags.count_ones() != 1 {
            return Err(einval!(format!(
                "invalid flags {:#x} related to digest algorithm in Rafs v6 extended superblock",
                flags
            )));
        }

        let chunk_size = u32::from_le(self.s_chunk_size) as u64;
        if !chunk_size.is_power_of_two()
            || chunk_size < EROFS_BLOCK_SIZE
            || chunk_size > RAFS_MAX_CHUNK_SIZE
        {
            return Err(einval!("invalid chunk size in Rafs v6 extended superblock"));
        }

        if self.blob_table_offset() & (EROFS_BLOCK_SIZE - 1) != 0 {
            return Err(einval!(format!(
                "invalid blob table offset {} in Rafs v6 extended superblock",
                self.blob_table_offset()
            )));
        }
        Ok(())
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

    pub fn set_has_xattr(&mut self) {
        self.s_flags |= RafsSuperFlags::HAS_XATTR.bits();
    }

    /// Enable explicit Uid/Gid feature.
    pub fn set_explicit_uidgid(&mut self) {
        self.s_flags |= RafsSuperFlags::EXPLICIT_UID_GID.bits();
    }

    /// Set message digest algorithm to handle chunk of the Rafs filesystem.
    pub fn set_digester(&mut self, digester: digest::Algorithm) {
        let c: RafsSuperFlags = digester.into();

        self.s_flags &= !RafsSuperFlags::DIGESTER_BLAKE3.bits();
        self.s_flags &= !RafsSuperFlags::DIGESTER_SHA256.bits();
        self.s_flags |= c.bits();
    }

    pub fn set_chunk_table(&mut self, offset: u64, size: u64) {
        self.set_chunk_table_offset(offset);
        self.set_chunk_table_size(size);
    }

    impl_pub_getter_setter!(
        chunk_table_offset,
        set_chunk_table_offset,
        s_chunk_table_offset,
        u64
    );
    impl_pub_getter_setter!(
        chunk_table_size,
        set_chunk_table_size,
        s_chunk_table_size,
        u64
    );
    impl_pub_getter_setter!(chunk_size, set_chunk_size, s_chunk_size, u32);
    impl_pub_getter_setter!(flags, set_flags, s_flags, u64);
    impl_pub_getter_setter!(
        blob_table_offset,
        set_blob_table_offset,
        s_blob_table_offset,
        u64
    );
    impl_pub_getter_setter!(blob_table_size, set_blob_table_size, s_blob_table_size, u32);
    impl_pub_getter_setter!(
        prefetch_table_size,
        set_prefetch_table_size,
        s_prefetch_table_size,
        u32
    );
    impl_pub_getter_setter!(
        prefetch_table_offset,
        set_prefetch_table_offset,
        s_prefetch_table_offset,
        u64
    );
}

impl RafsStore for RafsV6SuperBlockExt {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        w.seek_offset((EROFS_SUPER_OFFSET + EROFS_SUPER_BLOCK_SIZE) as u64)?;
        w.write_all(self.as_ref())?;
        w.seek_offset(EROFS_BLOCK_SIZE as u64)?;

        Ok(EROFS_BLOCK_SIZE as usize - (EROFS_SUPER_OFFSET + EROFS_SUPER_BLOCK_SIZE) as usize)
    }
}

impl Default for RafsV6SuperBlockExt {
    fn default() -> Self {
        Self {
            s_flags: u64::to_le(0),
            s_blob_table_offset: u64::to_le(0),
            s_blob_table_size: u32::to_le(0),
            s_chunk_size: u32::to_le(0),
            s_chunk_table_offset: u64::to_le(0),
            s_chunk_table_size: u64::to_le(0),
            s_prefetch_table_offset: u64::to_le(0),
            s_prefetch_table_size: u32::to_le(0),
            s_padding: u32::to_le(0),
            s_reserved: [0u8; 200],
        }
    }
}

/// Type of EROFS inodes.
#[repr(u8)]
#[allow(non_camel_case_types, dead_code)]
enum EROFS_FILE_TYPE {
    /// Unknown file type.
    EROFS_FT_UNKNOWN,
    /// Regular file.
    EROFS_FT_REG_FILE,
    /// Directory.
    EROFS_FT_DIR,
    /// Character device.
    EROFS_FT_CHRDEV,
    /// Block device.
    EROFS_FT_BLKDEV,
    /// FIFO pipe.
    EROFS_FT_FIFO,
    /// Socket.
    EROFS_FT_SOCK,
    /// Symlink.
    EROFS_FT_SYMLINK,
    /// Maximum value of file type.
    EROFS_FT_MAX,
}

pub trait RafsV6OndiskInode: RafsStore {
    fn set_xattr_inline_count(&mut self, count: u16);
    fn set_size(&mut self, size: u64);
    fn set_ino(&mut self, ino: u32);
    fn set_nlink(&mut self, nlinks: u32);
    fn set_mode(&mut self, mode: u16);
    fn set_u(&mut self, u: u32);
    fn set_uidgid(&mut self, uid: u32, gid: u32);
    fn set_mtime(&mut self, _sec: u64, _nsec: u32);
    fn set_data_layout(&mut self, data_layout: u16);
    /// Set inode data layout format to be PLAIN.
    #[inline]
    fn set_inline_plain_layout(&mut self) {
        self.set_data_layout(EROFS_INODE_FLAT_PLAIN);
    }

    /// Set inode data layout format to be INLINE.
    #[inline]
    fn set_inline_inline_layout(&mut self) {
        self.set_data_layout(EROFS_INODE_FLAT_INLINE);
    }

    /// Set inode data layout format to be CHUNKED.
    #[inline]
    fn set_chunk_based_layout(&mut self) {
        self.set_data_layout(EROFS_INODE_CHUNK_BASED);
    }

    fn set_rdev(&mut self, rdev: u32);

    fn load(&mut self, r: &mut RafsIoReader) -> Result<()>;

    fn format(&self) -> u16;
    fn mode(&self) -> u16;
    fn size(&self) -> u64;
    fn union(&self) -> u32;
    fn ino(&self) -> u32;
    fn ugid(&self) -> (u32, u32);
    fn mtime_s_ns(&self) -> (u64, u32);
    fn nlink(&self) -> u32;
    fn rdev(&self) -> u32;
    fn xattr_inline_count(&self) -> u16;
}

impl Debug for &dyn RafsV6OndiskInode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Ondisk Inode")
            .field("format", &self.format())
            .field("ino", &self.ino())
            .field("mode", &self.mode())
            .field("size", &self.size())
            .field("union", &self.union())
            .field("nlink", &self.nlink())
            .field("xattr count", &self.xattr_inline_count())
            .finish()
    }
}

/// RAFS v6 inode on-disk format, 32 bytes.
///
/// This structure is designed to be compatible with EROFS compact inode format.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct RafsV6InodeCompact {
    /// inode format hints
    pub i_format: u16,
    pub i_xattr_icount: u16,
    pub i_mode: u16,
    pub i_nlink: u16,
    pub i_size: u32,
    pub i_reserved: u32,
    /// raw_blkaddr or rdev or rafs_v6_inode_chunk_info
    pub i_u: u32,
    pub i_ino: u32,
    pub i_uid: u16,
    pub i_gid: u16,
    pub i_reserved2: [u8; 4],
}

impl RafsV6InodeCompact {
    pub fn new() -> Self {
        Self {
            i_format: u16::to_le(EROFS_INODE_LAYOUT_COMPACT | (EROFS_INODE_FLAT_PLAIN << 1)),
            i_xattr_icount: u16::to_le(0),
            i_mode: u16::to_le(0),
            i_nlink: u16::to_le(0),
            i_size: u32::to_le(0),
            i_reserved: u32::to_le(0),
            i_u: u32::to_le(0),
            i_ino: u32::to_le(0),
            i_uid: u16::to_le(0),
            i_gid: u16::to_le(0),
            i_reserved2: [0u8; 4],
        }
    }
}

impl RafsV6OndiskInode for RafsV6InodeCompact {
    /// Set xattr inline count.
    fn set_xattr_inline_count(&mut self, count: u16) {
        self.i_xattr_icount = count.to_le();
    }

    /// Set file size for inode.
    fn set_size(&mut self, size: u64) {
        self.i_size = u32::to_le(size as u32);
    }

    /// Set ino for inode.
    fn set_ino(&mut self, ino: u32) {
        self.i_ino = ino.to_le();
    }

    /// Set number of hardlink.
    fn set_nlink(&mut self, nlinks: u32) {
        self.i_nlink = u16::to_le(nlinks as u16);
    }

    /// Set file protection mode.
    fn set_mode(&mut self, mode: u16) {
        self.i_mode = mode.to_le();
    }

    /// Set the union field.
    fn set_u(&mut self, u: u32) {
        self.i_u = u.to_le();
    }

    /// Set uid and gid for the inode.
    fn set_uidgid(&mut self, uid: u32, gid: u32) {
        self.i_uid = u16::to_le(uid as u16);
        self.i_gid = u16::to_le(gid as u16);
    }

    /// Set last modification time for the inode.
    fn set_mtime(&mut self, _sec: u64, _nsec: u32) {}

    fn set_rdev(&mut self, _rdev: u32) {}

    /// Set inode data layout format.
    fn set_data_layout(&mut self, data_layout: u16) {
        self.i_format = u16::to_le(EROFS_INODE_LAYOUT_COMPACT | (data_layout << 1));
    }

    /// Load a `RafsV6InodeCompact` from a reader.
    fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        r.read_exact(self.as_mut())
    }

    fn format(&self) -> u16 {
        self.i_format
    }

    fn mode(&self) -> u16 {
        self.i_mode
    }

    fn size(&self) -> u64 {
        self.i_size as u64
    }

    fn union(&self) -> u32 {
        self.i_u
    }

    fn ino(&self) -> u32 {
        self.i_ino
    }

    fn ugid(&self) -> (u32, u32) {
        (self.i_uid as u32, self.i_gid as u32)
    }

    fn mtime_s_ns(&self) -> (u64, u32) {
        (0, 0)
    }

    fn nlink(&self) -> u32 {
        self.i_nlink as u32
    }

    fn rdev(&self) -> u32 {
        0
    }

    fn xattr_inline_count(&self) -> u16 {
        self.i_xattr_icount
    }
}

impl_bootstrap_converter!(RafsV6InodeCompact);

impl RafsStore for RafsV6InodeCompact {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        // TODO: need to write xattr as well.
        w.write_all(self.as_ref())?;
        Ok(self.as_ref().len())
    }
}

/// RAFS v6 inode on-disk format, 64 bytes.
///
/// This structure is designed to be compatible with EROFS extended inode format.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct RafsV6InodeExtended {
    /// Layout format for of the inode.
    pub i_format: u16,
    /// TODO: doc
    /// In unit of 4Byte
    pub i_xattr_icount: u16,
    /// Protection mode.
    pub i_mode: u16,
    i_reserved: u16,
    /// Size of the file content.
    pub i_size: u64,
    /// A `u32` union: raw_blkaddr or `rdev` or `rafs_v6_inode_chunk_info`
    /// TODO: Use this field like C union style.
    pub i_u: u32,
    /// Inode number.
    pub i_ino: u32,
    /// User ID of owner.
    pub i_uid: u32,
    /// Group ID of owner
    pub i_gid: u32,
    /// Time of last modification - second part.
    pub i_mtime: u64,
    /// Time of last modification - nanoseconds part.
    pub i_mtime_nsec: u32,
    /// Number of links.
    pub i_nlink: u32,
    i_reserved2: [u8; 16],
}

impl RafsV6InodeExtended {
    /// Create a new instance of `RafsV6InodeExtended`.
    pub fn new() -> Self {
        Self {
            i_format: u16::to_le(EROFS_INODE_LAYOUT_EXTENDED | (EROFS_INODE_FLAT_PLAIN << 1)),
            i_xattr_icount: u16::to_le(0),
            i_mode: u16::to_le(0),
            i_reserved: u16::to_le(0),
            i_size: u64::to_le(0),
            i_u: u32::to_le(0),
            i_ino: u32::to_le(0),
            i_uid: u32::to_le(0),
            i_gid: u32::to_le(0),
            i_mtime: u64::to_le(0),
            i_mtime_nsec: u32::to_le(0),
            i_nlink: u32::to_le(0),
            i_reserved2: [0u8; 16],
        }
    }
}

impl RafsV6OndiskInode for RafsV6InodeExtended {
    /// Set xattr inline count.
    fn set_xattr_inline_count(&mut self, count: u16) {
        self.i_xattr_icount = count.to_le();
    }

    /// Set file size for inode.
    fn set_size(&mut self, size: u64) {
        self.i_size = size.to_le();
    }

    /// Set ino for inode.
    fn set_ino(&mut self, ino: u32) {
        self.i_ino = ino.to_le();
    }

    /// Set number of hardlink.
    fn set_nlink(&mut self, nlinks: u32) {
        self.i_nlink = nlinks.to_le();
    }

    /// Set file protection mode.
    fn set_mode(&mut self, mode: u16) {
        self.i_mode = mode.to_le();
    }

    /// Set the union field.
    fn set_u(&mut self, u: u32) {
        self.i_u = u.to_le();
    }

    /// Set uid and gid for the inode.
    fn set_uidgid(&mut self, uid: u32, gid: u32) {
        self.i_uid = u32::to_le(uid);
        self.i_gid = u32::to_le(gid);
    }

    /// Set last modification time for the inode.
    fn set_mtime(&mut self, sec: u64, nsec: u32) {
        self.i_mtime = u64::to_le(sec);
        self.i_mtime_nsec = u32::to_le(nsec);
    }

    fn set_rdev(&mut self, rdev: u32) {
        self.i_u = rdev
    }

    /// Set inode data layout format.
    fn set_data_layout(&mut self, data_layout: u16) {
        self.i_format = u16::to_le(EROFS_INODE_LAYOUT_EXTENDED | (data_layout << 1));
    }

    /// Load a `RafsV6InodeExtended` from a reader.
    fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        r.read_exact(self.as_mut())
    }

    fn format(&self) -> u16 {
        self.i_format
    }

    fn mode(&self) -> u16 {
        self.i_mode
    }

    fn size(&self) -> u64 {
        self.i_size
    }

    fn union(&self) -> u32 {
        self.i_u
    }

    fn ino(&self) -> u32 {
        self.i_ino
    }

    fn ugid(&self) -> (u32, u32) {
        (self.i_uid, self.i_gid)
    }

    fn mtime_s_ns(&self) -> (u64, u32) {
        (self.i_mtime, self.i_mtime_nsec)
    }

    fn nlink(&self) -> u32 {
        self.i_nlink
    }

    fn rdev(&self) -> u32 {
        self.i_u
    }

    fn xattr_inline_count(&self) -> u16 {
        self.i_xattr_icount
    }
}

impl_bootstrap_converter!(RafsV6InodeExtended);

impl RafsStore for RafsV6InodeExtended {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        // TODO: need to write xattr as well.
        w.write_all(self.as_ref())?;
        Ok(self.as_ref().len())
    }
}

/// Dirent sorted in alphabet order to improve performance by binary search.
#[repr(C, packed(2))]
#[derive(Default, Clone, Copy, Debug)]
pub struct RafsV6Dirent {
    /// Node number, inode offset = s_meta_blkaddr * 4096 + nid * 32
    pub e_nid: u64,
    /// start offset of file name in the block
    pub e_nameoff: u16,
    /// file type
    pub e_file_type: u8,
    /// reserved
    e_reserved: u8,
}

impl_bootstrap_converter!(RafsV6Dirent);

impl RafsV6Dirent {
    /// Create a new instance of `RafsV6Dirent`.
    pub fn new(nid: u64, nameoff: u16, file_type: u8) -> Self {
        Self {
            e_nid: u64::to_le(nid),
            e_nameoff: u16::to_le(nameoff),
            e_file_type: u8::to_le(file_type),
            e_reserved: u8::to_le(0),
        }
    }

    /// Get file type from file mode.
    pub fn file_type(mode: u32) -> u8 {
        let val = match mode {
            mode if mode & libc::S_IFMT as u32 == libc::S_IFREG as u32 => {
                EROFS_FILE_TYPE::EROFS_FT_REG_FILE
            }
            mode if mode & libc::S_IFMT as u32 == libc::S_IFDIR as u32 => {
                EROFS_FILE_TYPE::EROFS_FT_DIR
            }
            mode if mode & libc::S_IFMT as u32 == libc::S_IFCHR as u32 => {
                EROFS_FILE_TYPE::EROFS_FT_CHRDEV
            }
            mode if mode & libc::S_IFMT as u32 == libc::S_IFBLK as u32 => {
                EROFS_FILE_TYPE::EROFS_FT_BLKDEV
            }
            mode if mode & libc::S_IFMT as u32 == libc::S_IFIFO as u32 => {
                EROFS_FILE_TYPE::EROFS_FT_FIFO
            }
            mode if mode & libc::S_IFMT as u32 == libc::S_IFSOCK as u32 => {
                EROFS_FILE_TYPE::EROFS_FT_SOCK
            }
            mode if mode & libc::S_IFMT as u32 == libc::S_IFLNK as u32 => {
                EROFS_FILE_TYPE::EROFS_FT_SYMLINK
            }
            _ => EROFS_FILE_TYPE::EROFS_FT_UNKNOWN,
        };

        val as u8
    }

    /// Set name offset of the dirent.
    pub fn set_name_offset(&mut self, offset: u16) {
        debug_assert!(offset < EROFS_BLOCK_SIZE as u16);
        self.e_nameoff = u16::to_le(offset);
    }

    /// Load a `RafsV6Dirent` from a reader.
    pub fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        r.read_exact(self.as_mut())
    }
}

impl RafsStore for RafsV6Dirent {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        w.write_all(self.as_ref())?;
        Ok(self.as_ref().len())
    }
}

/// Rafs v6 ChunkHeader on-disk format.
#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct RafsV6InodeChunkHeader {
    /// Chunk layout format.
    format: u16,
    reserved: u16,
}

impl RafsV6InodeChunkHeader {
    /// Create a new instance of `RafsV6InodeChunkHeader`.
    pub fn new(chunk_size: u32) -> Self {
        debug_assert!(chunk_size.is_power_of_two());
        let chunk_bits = chunk_size.trailing_zeros() as u16;
        debug_assert!(chunk_bits >= EROFS_BLOCK_BITS as u16);
        let chunk_bits = chunk_bits - EROFS_BLOCK_BITS as u16;
        debug_assert!(chunk_bits <= EROFS_CHUNK_FORMAT_SIZE_MASK);
        let format = EROFS_CHUNK_FORMAT_INDEXES_FLAG | chunk_bits;

        Self {
            format: u16::to_le(format),
            reserved: u16::to_le(0),
        }
    }

    /// Convert to a u32 value.
    pub fn to_u32(&self) -> u32 {
        (u16::from_le(self.format) as u32) | ((u16::from_le(self.reserved) as u32) << 16)
    }

    /// Convert a u32 value to `RafsV6InodeChunkHeader`.
    pub fn from_u32(val: u32) -> Self {
        Self {
            format: (val as u16).to_le(),
            reserved: ((val >> 16) as u16).to_le(),
        }
    }
}

impl_bootstrap_converter!(RafsV6InodeChunkHeader);

/// Rafs v6 chunk address on-disk format, 8 bytes.
#[repr(C)]
#[derive(Default, Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct RafsV6InodeChunkAddr {
    /// Lower part of encoded blob address.
    c_blob_addr_lo: u16,
    /// Higher part of encoded blob address.
    c_blob_addr_hi: u16,
    /// start block address of this inode chunk
    /// decompressed offset must be aligned, in unit of block
    c_blk_addr: u32,
}

impl RafsV6InodeChunkAddr {
    /// Create a new instance of `RafsV6InodeChunkIndex`.
    pub fn new() -> Self {
        Self {
            c_blob_addr_lo: u16::to_le(0),
            c_blob_addr_hi: u16::to_le(0),
            c_blk_addr: u32::to_le(0),
        }
    }

    /// Note: for erofs, bump id by 1 since device id 0 is bootstrap.
    /// The index in BlobInfo grows from 0, so when using this method to index the corresponding blob,
    /// the index always needs to be minus 1
    /// Get the blob index of the chunk.
    pub fn blob_index(&self) -> u8 {
        (u16::from_le(self.c_blob_addr_hi) & 0x00ff) as u8
    }

    /// Set the blob index of the chunk.
    pub fn set_blob_index(&mut self, blob_idx: u8) {
        let mut val = u16::from_le(self.c_blob_addr_hi);
        val &= 0xff00;
        val |= blob_idx as u16;
        self.c_blob_addr_hi = val.to_le();
    }

    /// Get the index into the blob compression information array.
    /// Within blob, chunk's index 24bits
    pub fn blob_comp_index(&self) -> u32 {
        let val = (u16::from_le(self.c_blob_addr_hi) as u32) >> 8;
        (val << 16) | (u16::from_le(self.c_blob_addr_lo) as u32)
    }

    /// Set the index into the blob compression information array.
    pub fn set_blob_comp_index(&mut self, comp_index: u32) {
        debug_assert!(comp_index <= 0x00ff_ffff);
        let val = (comp_index >> 8) as u16 & 0xff00 | (u16::from_le(self.c_blob_addr_hi) & 0x00ff);
        self.c_blob_addr_hi = val.to_le();
        self.c_blob_addr_lo = u16::to_le(comp_index as u16);
    }

    /// Get block address.
    pub fn block_addr(&self) -> u32 {
        u32::from_le(self.c_blk_addr)
    }

    /// Set block address.
    pub fn set_block_addr(&mut self, addr: u32) {
        self.c_blk_addr = addr.to_le();
    }

    /// Load a `RafsV6InodeChunkAddr` from a reader.
    pub fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        r.read_exact(self.as_mut())
    }
}

impl_bootstrap_converter!(RafsV6InodeChunkAddr);

impl RafsStore for RafsV6InodeChunkAddr {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        w.write_all(self.as_ref())?;

        Ok(self.as_ref().len())
    }
}

/// Rafs v6 device information on-disk format, 128 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RafsV6Device {
    /// Blob id of sha256.
    blob_id: [u8; BLOB_SHA256_LEN],
    /// Number of blocks on the device.
    blocks: u32,
    /// Mapping start address.
    mapped_blkaddr: u32,
    reserved2: [u8; 56],
}

impl Default for RafsV6Device {
    fn default() -> Self {
        Self {
            blob_id: [0u8; 64],
            blocks: u32::to_le(0),
            mapped_blkaddr: u32::to_le(0),
            reserved2: [0u8; 56],
        }
    }
}

impl RafsV6Device {
    /// Create a new instance of `RafsV6DeviceSlot`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get blob id.
    pub fn blob_id(&self) -> &[u8] {
        &self.blob_id
    }

    /// Set blob id.
    pub fn set_blob_id(&mut self, id: &[u8; 64]) {
        self.blob_id.copy_from_slice(id);
    }

    /// Get number of blocks.
    pub fn blocks(&self) -> u32 {
        u32::from_le(self.blocks)
    }

    /// Set number of blocks.
    pub fn set_blocks(&mut self, size: u64) {
        debug_assert!(size % EROFS_BLOCK_SIZE == 0);
        let blocks: u32 = (size >> EROFS_BLOCK_BITS) as u32;
        self.blocks = blocks.to_le();
    }

    /// Set mapped block address.
    pub fn set_mapped_blkaddr(&mut self, addr: u32) {
        self.mapped_blkaddr = addr.to_le();
    }

    /// Load a `RafsV6Device` from a reader.
    pub fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        r.read_exact(self.as_mut())
    }

    /// Validate the Rafs v6 Device slot.
    pub fn validate(&self) -> Result<()> {
        match String::from_utf8(self.blob_id.to_vec()) {
            Ok(v) => {
                if v.len() != BLOB_SHA256_LEN {
                    return Err(einval!(format!("v.len {} is invalid", v.len())));
                }
            }

            Err(_) => {
                return Err(einval!("blob_id from_utf8 is invalid"));
            }
        }

        if self.blocks() == 0 {
            return Err(einval!(format!(
                "invalid blocks {} in Rafs v6 Device",
                self.blocks()
            )));
        }

        if u32::from_le(self.mapped_blkaddr) != 0 {
            return Err(einval!("invalid mapped_addr in Rafs v6 Device"));
        }

        Ok(())
    }
}

impl_bootstrap_converter!(RafsV6Device);

impl RafsStore for RafsV6Device {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        w.write_all(self.as_ref())?;

        Ok(self.as_ref().len())
    }
}

#[inline]
pub fn align_offset(offset: u64, aligned_size: u64) -> u64 {
    round_up(offset, aligned_size)
}

/// Generate EROFS `nid` from `offset`.
pub fn calculate_nid(offset: u64, meta_size: u64) -> u64 {
    (offset - meta_size) >> EROFS_INODE_SLOT_BITS
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RafsV6Blob {
    // SHA256 blob id in string form.
    blob_id: [u8; BLOB_SHA256_LEN],
    // Index in the blob table.
    blob_index: u32,
    // Chunk size of the blob.
    chunk_size: u32,
    // Number of chunks in the blob.
    chunk_count: u32,
    // Compression algorithm for chunks in the blob.
    compression_algo: u32,
    // Digest algorithm for chunks in the blob.
    digest_algo: u32,
    // Flags for the compression information array.
    meta_features: u32,
    // Size of the compressed blob.
    compressed_size: u64,
    // Size of the uncompressed blob.
    uncompressed_size: u64,

    reserved1: u32,
    // Compression algorithm for the compression information array.
    ci_compressor: u32,
    // Offset into the compressed blob for the compression information array.
    ci_offset: u64,
    // Size of the compressed compression information array.
    ci_compressed_size: u64,
    // Size of the uncompressed compression information array.
    ci_uncompressed_size: u64,
    // SHA256 digest of the compression information array in binary form.
    ci_digest: [u8; 32],

    reserved2: [u8; 88],
}

impl Default for RafsV6Blob {
    fn default() -> Self {
        RafsV6Blob {
            blob_id: [0u8; BLOB_SHA256_LEN],
            blob_index: 0u32.to_le(),
            chunk_size: 0u32.to_le(),
            chunk_count: 0u32.to_le(),
            compression_algo: (compress::Algorithm::None as u32).to_le(),
            digest_algo: (digest::Algorithm::Blake3 as u32).to_le(),
            meta_features: 0u32.to_le(),
            compressed_size: 0u64.to_le(),
            uncompressed_size: 0u64.to_le(),
            reserved1: 0u32.to_le(),
            ci_compressor: (compress::Algorithm::None as u32).to_le(),
            ci_offset: 0u64.to_le(),
            ci_compressed_size: 0u64.to_le(),
            ci_uncompressed_size: 0u64.to_le(),
            ci_digest: [0u8; 32],
            reserved2: [0u8; 88],
        }
    }
}

impl_bootstrap_converter!(RafsV6Blob);

impl RafsV6Blob {
    #[allow(clippy::wrong_self_convention)]
    fn to_blob_info(&self) -> Result<BlobInfo> {
        // debug_assert!(RAFS_DIGEST_LENGTH == 32);
        debug_assert!(size_of::<RafsV6Blob>() == 256);

        let blob_id = String::from_utf8(self.blob_id.to_vec())
            .map_err(|e| einval!(format!("invalid blob id, {}", e)))?;
        let mut blob_info = BlobInfo::new(
            u32::from_le(self.blob_index),
            blob_id,
            u64::from_le(self.uncompressed_size),
            u64::from_le(self.compressed_size),
            u32::from_le(self.chunk_size),
            u32::from_le(self.chunk_count),
            BlobFeatures::empty(),
        );

        let comp = compress::Algorithm::try_from(u32::from_le(self.compression_algo))
            .map_err(|_| einval!("invalid compression algorithm in Rafs v6 blob entry"))?;
        blob_info.set_compressor(comp);
        let digest = digest::Algorithm::try_from(u32::from_le(self.digest_algo))
            .map_err(|_| einval!("invalid digest algorithm in Rafs v6 blob entry"))?;
        blob_info.set_digester(digest);
        // blob_info.set_readahead(readahead_offset as u64, readahead_size as u64);
        blob_info.set_blob_meta_info(
            u32::from_le(self.meta_features),
            u64::from_le(self.ci_offset),
            u64::from_le(self.ci_compressed_size),
            u64::from_le(self.ci_uncompressed_size),
            u32::from_le(self.ci_compressor),
        );

        Ok(blob_info)
    }

    fn from_blob_info(blob_info: &BlobInfo) -> Result<Self> {
        if blob_info.blob_id().len() != BLOB_SHA256_LEN {
            return Err(einval!(format!(
                "invalid blob id in blob info, {}",
                blob_info.blob_id()
            )));
        }

        let mut blob_id = [0u8; BLOB_SHA256_LEN];
        blob_id.copy_from_slice(blob_info.blob_id().as_bytes());

        Ok(RafsV6Blob {
            blob_id,
            blob_index: blob_info.blob_index().to_le(),
            chunk_size: blob_info.chunk_size().to_le(),
            chunk_count: blob_info.chunk_count().to_le(),
            compression_algo: (blob_info.compressor() as u32).to_le(),
            digest_algo: (blob_info.digester() as u32).to_le(),
            reserved1: 0u32.to_le(),
            compressed_size: blob_info.compressed_size().to_le(),
            uncompressed_size: blob_info.uncompressed_size().to_le(),
            meta_features: blob_info.meta_flags().to_le(),
            ci_compressor: (blob_info.meta_ci_compressor() as u32).to_le(),
            ci_offset: blob_info.meta_ci_offset().to_le(),
            ci_compressed_size: blob_info.meta_ci_compressed_size().to_le(),
            ci_uncompressed_size: blob_info.meta_ci_uncompressed_size().to_le(),
            ci_digest: [0u8; 32],
            reserved2: [0u8; 88],
        })
    }

    fn validate(&self, blob_index: u32, chunk_size: u32, _flags: RafsSuperFlags) -> bool {
        /* TODO: check fields: compressed_size, meta_features, ci_offset, ci_compressed_size, ci_uncompressed_size */
        match String::from_utf8(self.blob_id.to_vec()) {
            Ok(v) => {
                if v.len() != BLOB_SHA256_LEN {
                    error!(
                        "RafsV6Blob: idx {} v.len {} is invalid",
                        blob_index,
                        v.len()
                    );
                    return false;
                }
            }

            Err(_) => {
                error!(
                    "RafsV6Blob: idx {} blob_id from_utf8 is invalid",
                    blob_index
                );
                return false;
            }
        }

        if self.blob_index != blob_index.to_le() {
            error!(
                "RafsV6Blob: blob_index mismatch {} {}",
                self.blob_index, blob_index
            );
            return false;
        }

        let c_size = u32::from_le(self.chunk_size) as u64;
        if c_size.count_ones() != 1
            || c_size < EROFS_BLOCK_SIZE
            || c_size > RAFS_MAX_CHUNK_SIZE
            || c_size != chunk_size as u64
        {
            error!(
                "RafsV6Blob: idx {} invalid c_size {}, count_ones() {}",
                blob_index,
                c_size,
                c_size.count_ones()
            );
            return false;
        }

        if u32::from_le(self.chunk_count) >= (1u32 << 24) {
            error!(
                "RafsV6Blob: idx {} invalid chunk_count {}",
                blob_index,
                u32::from_le(self.chunk_count)
            );
            return false;
        }

        let uncompressed_blob_size = u64::from_le(self.uncompressed_size);
        // TODO: Make blobs of 4k size aligned.

        if uncompressed_blob_size > BLOB_MAX_SIZE {
            error!(
                "RafsV6Blob: idx {} invalid uncompressed_size {}",
                blob_index, uncompressed_blob_size
            );
            return false;
        }

        if compress::Algorithm::try_from(u32::from_le(self.compression_algo)).is_err()
            || compress::Algorithm::try_from(u32::from_le(self.ci_compressor)).is_err()
            || digest::Algorithm::try_from(u32::from_le(self.digest_algo)).is_err()
        {
            error!(
                "RafsV6Blob: idx {} invalid compression_algo {} ci_compressor {} digest_algo {}",
                blob_index, self.compression_algo, self.ci_compressor, self.digest_algo
            );
            return false;
        }

        if self.ci_digest != [0u8; 32] {
            error!("RafsV6Blob: idx {} invalid ci_digest", blob_index);
            return false;
        }

        // for now the uncompressed data chunk of v6 image is 4k aligned.
        if u32::from_le(self.meta_features) & BLOB_FEATURE_4K_ALIGNED == 0 {
            error!("RafsV6Blob: idx {} invalid meta_features", blob_index);
            return false;
        }

        let ci_compr_size = u64::from_le(self.ci_compressed_size);
        let ci_uncompr_size = u64::from_le(self.ci_uncompressed_size);
        if ci_compr_size > ci_uncompr_size {
            error!("RafsV6Blob: idx {} invalid fields, ci_compressed_size {} is greater than ci_uncompressed_size {}", blob_index, ci_compr_size, ci_uncompr_size);
            return false;
        }

        true
    }
}

/// Rafs v6 blob description table.
#[derive(Clone, Debug, Default)]
pub struct RafsV6BlobTable {
    /// Base blob information array.
    pub entries: Vec<Arc<BlobInfo>>,
}

impl RafsV6BlobTable {
    /// Create a new instance of `RafsV6BlobTable`.
    pub fn new() -> Self {
        RafsV6BlobTable {
            entries: Vec::new(),
        }
    }

    /// Get blob table size.
    pub fn size(&self) -> usize {
        self.entries.len() * size_of::<RafsV6Blob>()
    }

    /// Get base information for a blob.
    #[inline]
    pub fn get(&self, blob_index: u32) -> Result<Arc<BlobInfo>> {
        if blob_index >= self.entries.len() as u32 {
            Err(enoent!("blob not found"))
        } else {
            Ok(self.entries[blob_index as usize].clone())
        }
    }

    /// Get the base blob information array.
    pub fn get_all(&self) -> Vec<Arc<BlobInfo>> {
        self.entries.clone()
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
        header: BlobMetaHeaderOndisk,
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
        blob_info.set_blob_meta_info(
            header.meta_flags(),
            header.ci_compressed_offset(),
            header.ci_compressed_size(),
            header.ci_uncompressed_size(),
            header.ci_compressor() as u32,
        );

        self.entries.push(Arc::new(blob_info));

        blob_index
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
        if blob_table_size as usize % size_of::<RafsV6Blob>() != 0 {
            return Err(einval!(format!(
                "invalid Rafs v6 blob table size {}",
                blob_table_size
            )));
        }

        for idx in 0..(blob_table_size as usize / size_of::<RafsV6Blob>()) {
            let mut blob = RafsV6Blob::default();
            r.read_exact(blob.as_mut())?;
            if !blob.validate(idx as u32, chunk_size, flags) {
                return Err(einval!("invalid Rafs v6 blob entry"));
            }
            let blob_info = blob.to_blob_info()?;
            self.entries.push(Arc::new(blob_info));
        }

        Ok(())
    }
}

impl RafsStore for RafsV6BlobTable {
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        for blob_info in self.entries.iter() {
            let blob: RafsV6Blob = RafsV6Blob::from_blob_info(blob_info)?;
            trace!(
                "blob_info index {}, chunk_count {} blob_id {:?}",
                blob_info.blob_index(),
                blob_info.chunk_count(),
                blob_info.blob_id(),
            );
            w.write_all(blob.as_ref())?;
        }

        Ok(self.entries.len() * size_of::<RafsV6Blob>())
    }
}

// RafsV6 xattr
const EROFS_XATTR_INDEX_USER: u8 = 1;
const EROFS_XATTR_INDEX_POSIX_ACL_ACCESS: u8 = 2;
const EROFS_XATTR_INDEX_POSIX_ACL_DEFAULT: u8 = 3;
const EROFS_XATTR_INDEX_TRUSTED: u8 = 4;
// const EROFS_XATTR_INDEX_LUSTRE: u8 = 5;
const EROFS_XATTR_INDEX_SECURITY: u8 = 6;

const XATTR_USER_PREFIX: &str = "user.";
const XATTR_SECURITY_PREFIX: &str = "security.";
const XATTR_TRUSTED_PREFIX: &str = "trusted.";
const XATTR_NAME_POSIX_ACL_ACCESS: &str = "system.posix_acl_access";
const XATTR_NAME_POSIX_ACL_DEFAULT: &str = "system.posix_acl_default";

struct RafsV6XattrPrefix {
    index: u8,
    prefix: &'static str,
    prefix_len: usize,
}

impl RafsV6XattrPrefix {
    fn new(prefix: &'static str, index: u8, prefix_len: usize) -> Self {
        RafsV6XattrPrefix {
            index,
            prefix,
            prefix_len,
        }
    }
}

lazy_static! {
    static ref RAFSV6_XATTR_TYPES: Vec<RafsV6XattrPrefix> = vec![
        RafsV6XattrPrefix::new(
            XATTR_USER_PREFIX,
            EROFS_XATTR_INDEX_USER,
            XATTR_USER_PREFIX.as_bytes().len()
        ),
        RafsV6XattrPrefix::new(
            XATTR_NAME_POSIX_ACL_ACCESS,
            EROFS_XATTR_INDEX_POSIX_ACL_ACCESS,
            XATTR_NAME_POSIX_ACL_ACCESS.as_bytes().len()
        ),
        RafsV6XattrPrefix::new(
            XATTR_NAME_POSIX_ACL_DEFAULT,
            EROFS_XATTR_INDEX_POSIX_ACL_DEFAULT,
            XATTR_NAME_POSIX_ACL_DEFAULT.as_bytes().len()
        ),
        RafsV6XattrPrefix::new(
            XATTR_TRUSTED_PREFIX,
            EROFS_XATTR_INDEX_TRUSTED,
            XATTR_TRUSTED_PREFIX.as_bytes().len()
        ),
        RafsV6XattrPrefix::new(
            XATTR_SECURITY_PREFIX,
            EROFS_XATTR_INDEX_SECURITY,
            XATTR_SECURITY_PREFIX.as_bytes().len()
        ),
    ];
}

// inline xattrs (n == i_xattr_icount):
// erofs_xattr_ibody_header(1) + (n - 1) * 4 bytes
//          12 bytes           /                   \
//                            /                     \
//                           /-----------------------\
//                           |  erofs_xattr_entries+ |
//                           +-----------------------+
// inline xattrs must starts in erofs_xattr_ibody_header,
// for read-only fs, no need to introduce h_refcount
#[repr(C)]
#[derive(Default)]
pub struct RafsV6XattrIbodyHeader {
    h_reserved: u32,
    h_shared_count: u8,
    h_reserved2: [u8; 7],
    // may be followed by shared xattr id array
}

impl_bootstrap_converter!(RafsV6XattrIbodyHeader);

impl RafsV6XattrIbodyHeader {
    pub fn new() -> Self {
        RafsV6XattrIbodyHeader::default()
    }

    /// Load a `RafsV6XattrIbodyHeader` from a reader.
    pub fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        r.read_exact(self.as_mut())
    }
}

// RafsV6 xattr entry (for both inline & shared xattrs)
#[repr(C)]
#[derive(Default, PartialEq)]
pub struct RafsV6XattrEntry {
    // length of name
    e_name_len: u8,
    // attribute name index
    e_name_index: u8,
    // size of attribute value
    e_value_size: u16,
    // followed by e_name and e_value
}

impl_bootstrap_converter!(RafsV6XattrEntry);

impl RafsV6XattrEntry {
    fn new() -> Self {
        RafsV6XattrEntry::default()
    }

    pub fn name_len(&self) -> u32 {
        self.e_name_len as u32
    }

    pub fn name_index(&self) -> u8 {
        self.e_name_index
    }

    pub fn value_size(&self) -> u32 {
        u32::from_le(self.e_value_size as u32)
    }

    fn set_name_len(&mut self, v: u8) {
        self.e_name_len = v;
    }

    fn set_name_index(&mut self, v: u8) {
        self.e_name_index = v;
    }

    fn set_value_size(&mut self, v: u16) {
        self.e_value_size = v.to_le();
    }
}

pub fn recover_namespace(index: u8) -> Result<OsString> {
    let pos = RAFSV6_XATTR_TYPES
        .iter()
        .position(|x| x.index == index)
        .ok_or_else(|| einval!(format!("invalid xattr name index {}", index)))?;
    // Safe to unwrap since it is encoded in Nydus
    Ok(OsString::from_str(RAFSV6_XATTR_TYPES[pos].prefix).unwrap())
}

impl RafsXAttrs {
    /// Get the number of xattr pairs.
    pub fn count_v6(&self) -> usize {
        if self.is_empty() {
            0
        } else {
            let size = self.aligned_size_v6();
            (size - size_of::<RafsV6XattrIbodyHeader>()) / size_of::<RafsV6XattrEntry>() + 1
        }
    }

    /// Get aligned size of all xattr pairs.
    pub fn aligned_size_v6(&self) -> usize {
        if self.is_empty() {
            0
        } else {
            let mut size: usize = size_of::<RafsV6XattrIbodyHeader>();
            for (key, value) in self.pairs.iter() {
                let (_, prefix_len) = Self::match_prefix(key).expect("xattr is not valid");

                size += size_of::<RafsV6XattrEntry>();
                size += key.byte_size() - prefix_len + value.len();
                size = round_up(size as u64, size_of::<RafsV6XattrEntry>() as u64) as usize;
            }

            size
        }
    }

    /// Write Xattr to rafsv6 ondisk inode.
    pub fn store_v6(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        // TODO: check count of shared.
        let header = RafsV6XattrIbodyHeader::new();
        w.write_all(header.as_ref())?;

        if !self.pairs.is_empty() {
            for (key, value) in self.pairs.iter() {
                // TODO: fix error handling on unknown xattr.
                let (index, prefix_len) = Self::match_prefix(key).expect("xattr is not valid");

                let mut entry = RafsV6XattrEntry::new();
                entry.set_name_len((key.byte_size() - prefix_len) as u8);
                entry.set_name_index(index);
                entry.set_value_size(value.len().try_into().unwrap());

                w.write_all(entry.as_ref())?;
                w.write_all(&key.as_bytes()[prefix_len..])?;
                w.write_all(value.as_ref())?;

                let size =
                    size_of::<RafsV6XattrEntry>() + key.byte_size() - prefix_len + value.len();

                let padding =
                    round_up(size as u64, size_of::<RafsV6XattrEntry>() as u64) as usize - size;
                w.write_padding(padding)?;
            }
        }

        Ok(0)
    }

    fn match_prefix(key: &OsStr) -> Result<(u8, usize)> {
        let pos = RAFSV6_XATTR_TYPES
            .iter()
            .position(|x| key.to_string_lossy().starts_with(x.prefix))
            .ok_or_else(|| einval!(format!("xattr prefix {:?} is not valid", key)))?;
        Ok((
            RAFSV6_XATTR_TYPES[pos].index,
            RAFSV6_XATTR_TYPES[pos].prefix_len,
        ))
    }
}

#[derive(Clone, Default, Debug)]
pub struct RafsV6PrefetchTable {
    /// List of inode numbers for prefetch.
    /// Note: It's not inode index of inodes table being stored here.
    pub inodes: Vec<u32>,
}

impl RafsV6PrefetchTable {
    /// Create a new instance of `RafsV6PrefetchTable`.
    pub fn new() -> RafsV6PrefetchTable {
        RafsV6PrefetchTable { inodes: vec![] }
    }

    /// Get content size of the inode prefetch table.
    pub fn size(&self) -> usize {
        self.len() * size_of::<u32>()
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
        // let cur_len = self.inodes.len() * size_of::<u32>();

        Ok(data.len())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BufWriter, RafsIoRead};
    use std::ffi::OsString;
    use std::fs::OpenOptions;
    use std::io::Write;
    use vmm_sys_util::tempfile::TempFile;

    #[test]
    #[ignore]
    fn test_super_block_load_store() {
        let mut sb = RafsV6SuperBlock::new();
        let temp = TempFile::new().unwrap();
        let w = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.as_path())
            .unwrap();
        let r = OpenOptions::new()
            .read(true)
            .write(false)
            .open(temp.as_path())
            .unwrap();
        let mut writer = BufWriter::new(w);
        let mut reader: Box<dyn RafsIoRead> = Box::new(r);

        sb.s_blocks = 0x1000;
        sb.s_extra_devices = 5;
        sb.s_inos = 0x200;
        sb.store(&mut writer).unwrap();
        writer.flush().unwrap();

        let mut sb2 = RafsV6SuperBlock::new();
        sb2.load(&mut reader).unwrap();
        assert_eq!(sb2.s_magic, EROFS_SUPER_MAGIC_V1.to_le());
        assert_eq!(sb2.s_blocks, 0x1000u32.to_le());
        assert_eq!(sb2.s_extra_devices, 5u16.to_le());
        assert_eq!(sb2.s_inos, 0x200u64.to_le());
        assert_eq!(sb2.s_feature_compat, EROFS_FEATURE_COMPAT_RAFS_V6.to_le());
        assert_eq!(
            sb2.s_feature_incompat,
            (EROFS_FEATURE_INCOMPAT_CHUNKED_FILE | EROFS_FEATURE_INCOMPAT_DEVICE_TABLE).to_le()
        );
    }

    #[test]
    #[ignore]
    fn test_rafs_v6_inode_extended() {
        let temp = TempFile::new().unwrap();
        let w = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.as_path())
            .unwrap();
        let r = OpenOptions::new()
            .read(true)
            .write(false)
            .open(temp.as_path())
            .unwrap();
        let mut writer = BufWriter::new(w);
        let mut reader: Box<dyn RafsIoRead> = Box::new(r);

        let mut inode = RafsV6InodeExtended::new();
        assert_eq!(
            inode.i_format,
            u16::to_le(EROFS_INODE_LAYOUT_EXTENDED | (EROFS_INODE_FLAT_PLAIN << 1))
        );
        inode.set_data_layout(EROFS_INODE_FLAT_INLINE);
        assert_eq!(
            inode.i_format,
            u16::to_le(EROFS_INODE_LAYOUT_EXTENDED | (EROFS_INODE_FLAT_INLINE << 1))
        );
        inode.set_inline_plain_layout();
        assert_eq!(
            inode.i_format,
            u16::to_le(EROFS_INODE_LAYOUT_EXTENDED | (EROFS_INODE_FLAT_PLAIN << 1))
        );
        inode.set_inline_inline_layout();
        assert_eq!(
            inode.i_format,
            u16::to_le(EROFS_INODE_LAYOUT_EXTENDED | (EROFS_INODE_FLAT_INLINE << 1))
        );
        inode.set_chunk_based_layout();
        assert_eq!(
            inode.i_format,
            u16::to_le(EROFS_INODE_LAYOUT_EXTENDED | (EROFS_INODE_CHUNK_BASED << 1))
        );
        inode.set_uidgid(1, 2);
        inode.set_mtime(3, 4);
        inode.store(&mut writer).unwrap();
        writer.flush().unwrap();

        let mut inode2 = RafsV6InodeExtended::new();
        inode2.load(&mut reader).unwrap();
        assert_eq!(inode2.i_uid, 1u32.to_le());
        assert_eq!(inode2.i_gid, 2u32.to_le());
        assert_eq!(inode2.i_mtime, 3u64.to_le());
        assert_eq!(inode2.i_mtime_nsec, 4u32.to_le());
        assert_eq!(
            inode2.i_format,
            u16::to_le(EROFS_INODE_LAYOUT_EXTENDED | (EROFS_INODE_CHUNK_BASED << 1))
        );
    }

    #[test]
    fn test_rafs_v6_chunk_header() {
        let chunk_size: u32 = 1024 * 1024;
        let header = RafsV6InodeChunkHeader::new(chunk_size);
        let target = EROFS_CHUNK_FORMAT_INDEXES_FLAG | (20 - 12) as u16;
        assert_eq!(u16::from_le(header.format), target);
    }

    #[test]
    #[ignore]
    fn test_rafs_v6_chunk_addr() {
        let temp = TempFile::new().unwrap();
        let w = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.as_path())
            .unwrap();
        let r = OpenOptions::new()
            .read(true)
            .write(false)
            .open(temp.as_path())
            .unwrap();
        let mut writer = BufWriter::new(w);
        let mut reader: Box<dyn RafsIoRead> = Box::new(r);

        let mut chunk = RafsV6InodeChunkAddr::new();
        chunk.set_blob_index(3);
        chunk.set_blob_comp_index(0x123456);
        chunk.set_block_addr(0xa5a53412);
        chunk.store(&mut writer).unwrap();
        writer.flush().unwrap();
        let mut chunk2 = RafsV6InodeChunkAddr::new();
        chunk2.load(&mut reader).unwrap();
        assert_eq!(chunk2.blob_index(), 3);
        assert_eq!(chunk2.blob_comp_index(), 0x123456);
        assert_eq!(chunk2.block_addr(), 0xa5a53412);
    }

    #[test]
    #[ignore]
    fn test_rafs_v6_device() {
        let temp = TempFile::new().unwrap();
        let w = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.as_path())
            .unwrap();
        let r = OpenOptions::new()
            .read(true)
            .write(false)
            .open(temp.as_path())
            .unwrap();
        let mut writer = BufWriter::new(w);
        let mut reader: Box<dyn RafsIoRead> = Box::new(r);

        let id = [0xa5u8; 64];
        let mut device = RafsV6Device::new();
        device.set_blocks(0x1234);
        device.set_blob_id(&id);
        device.store(&mut writer).unwrap();
        writer.flush().unwrap();
        let mut device2 = RafsV6Device::new();
        device2.load(&mut reader).unwrap();
        assert_eq!(device2.blocks(), 0x1);
        assert_eq!(device.blob_id(), &id);
    }

    #[test]
    fn test_rafs_xattr_count_v6() {
        let mut xattrs = RafsXAttrs::new();
        xattrs.add(OsString::from("user.a"), vec![1u8]);
        xattrs.add(OsString::from("trusted.b"), vec![2u8]);

        assert_eq!(xattrs.count_v6(), 5);

        let xattrs2 = RafsXAttrs::new();
        assert_eq!(xattrs2.count_v6(), 0);
    }

    #[test]
    fn test_rafs_xattr_size_v6() {
        let mut xattrs = RafsXAttrs::new();
        xattrs.add(OsString::from("user.a"), vec![1u8]);
        xattrs.add(OsString::from("trusted.b"), vec![2u8]);

        let size = 12 + 8 + 8;
        assert_eq!(xattrs.aligned_size_v6(), size);

        let xattrs2 = RafsXAttrs::new();
        assert_eq!(xattrs2.aligned_size_v6(), 0);

        // let mut xattrs2 = RafsXAttrs::new();
        // xattrs2.add(OsString::from("user.a"), vec![1u8]);
        // xattrs2.add(OsString::from("unknown.b"), vec![2u8]);

        // assert_eq!(xattrs2.aligned_size_v6().is_error(), true);
    }

    #[test]
    #[ignore]
    fn test_rafs_xattr_store_v6() {
        let temp = TempFile::new().unwrap();
        let w = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.as_path())
            .unwrap();
        let r = OpenOptions::new()
            .read(true)
            .write(false)
            .open(temp.as_path())
            .unwrap();
        let mut writer = BufWriter::new(w);
        let mut reader: Box<dyn RafsIoRead> = Box::new(r);

        let mut xattrs = RafsXAttrs::new();
        xattrs.add(OsString::from("user.nydus"), vec![1u8]);
        xattrs.add(OsString::from("security.rafs"), vec![2u8, 3u8]);
        xattrs.store_v6(&mut writer).unwrap();
        writer.flush().unwrap();

        let mut header = RafsV6XattrIbodyHeader::new();
        header.load(&mut reader).unwrap();
        let mut size = size_of::<RafsV6XattrIbodyHeader>();

        assert_eq!(header.h_shared_count, 0u8);

        let target1 = RafsV6XattrEntry {
            e_name_len: 4u8,
            e_name_index: 6u8,
            e_value_size: u16::to_le(2u16),
        };

        let target2 = RafsV6XattrEntry {
            e_name_len: 5u8,
            e_name_index: 1u8,
            e_value_size: u16::to_le(1u16),
        };

        let mut entry1 = RafsV6XattrEntry::new();
        reader.read_exact(entry1.as_mut()).unwrap();
        assert!((entry1 == target1 || entry1 == target2));

        size += size_of::<RafsV6XattrEntry>()
            + entry1.name_len() as usize
            + entry1.value_size() as usize;

        reader
            .seek_to_offset(round_up(size as u64, size_of::<RafsV6XattrEntry>() as u64))
            .unwrap();

        let mut entry2 = RafsV6XattrEntry::new();
        reader.read_exact(entry2.as_mut()).unwrap();
        if entry1 == target1 {
            assert!(entry2 == target2);
        } else {
            assert!(entry2 == target1);
        }
    }
}
