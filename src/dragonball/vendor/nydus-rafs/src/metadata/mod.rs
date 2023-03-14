// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Structs and Traits for RAFS file system meta data management.

use std::any::Any;
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::fs::OpenOptions;
use std::io::{Error, Result};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::bail;

use fuse_backend_rs::abi::fuse_abi::Attr;
use fuse_backend_rs::api::filesystem::Entry;
use nydus_utils::compress;
use nydus_utils::digest::{self, RafsDigest};
use serde::Serialize;
use storage::device::{BlobChunkInfo, BlobInfo, BlobIoMerge, BlobIoVec};

use self::layout::v5::RafsV5PrefetchTable;
use self::layout::v6::RafsV6PrefetchTable;
use self::layout::{XattrName, XattrValue, RAFS_SUPER_VERSION_V5, RAFS_SUPER_VERSION_V6};
use self::noop::NoopSuperBlock;
use crate::fs::{RafsConfig, RAFS_DEFAULT_ATTR_TIMEOUT, RAFS_DEFAULT_ENTRY_TIMEOUT};
use crate::{RafsError, RafsIoReader, RafsIoWrite, RafsResult};

pub mod cached_v5;
pub mod direct_v5;
pub mod direct_v6;
pub mod layout;
mod md_v5;
mod md_v6;
mod noop;

pub use storage::{RAFS_DEFAULT_CHUNK_SIZE, RAFS_MAX_CHUNK_SIZE};

/// Maximum size of blob id string.
pub const RAFS_BLOB_ID_MAX_LENGTH: usize = 64;
/// Block size reported to fuse by get_attr().
pub const RAFS_ATTR_BLOCK_SIZE: u32 = 4096;
/// Maximum size of file name supported by rafs.
pub const RAFS_MAX_NAME: usize = 255;
/// Maximum size of the rafs metadata blob.
pub const RAFS_MAX_METADATA_SIZE: usize = 0x8000_0000;
/// File name for Unix current directory.
pub const DOT: &str = ".";
/// File name for Unix parent directory.
pub const DOTDOT: &str = "..";

/// Type of RAFS inode number.
pub type Inode = u64;

/// Trait to get information about inodes supported by the filesystem instance.
pub trait RafsSuperInodes {
    /// Get the maximum inode number supported by the filesystem instance.
    fn get_max_ino(&self) -> Inode;

    /// Get a `RafsInode` trait object for an inode, validating the inode content if requested.
    fn get_inode(&self, ino: Inode, digest_validate: bool) -> Result<Arc<dyn RafsInode>>;

    /// Validate the content of inode itself, optionally recursively validate into children.
    fn validate_digest(
        &self,
        inode: Arc<dyn RafsInode>,
        recursive: bool,
        digester: digest::Algorithm,
    ) -> Result<bool>;
}

/// Trait to access Rafs filesystem superblock and inodes.
pub trait RafsSuperBlock: RafsSuperInodes + Send + Sync {
    /// Load the super block from a reader.
    fn load(&mut self, r: &mut RafsIoReader) -> Result<()>;

    /// Update Rafs filesystem metadata and storage backend.
    fn update(&self, r: &mut RafsIoReader) -> RafsResult<()>;

    /// Destroy a Rafs filesystem super block.
    fn destroy(&mut self);

    /// Get all blob information objects used by the filesystem.
    fn get_blob_infos(&self) -> Vec<Arc<BlobInfo>>;

    fn root_ino(&self) -> u64;

    /// Get a chunk info.
    fn get_chunk_info(&self, _idx: usize) -> Result<Arc<dyn BlobChunkInfo>> {
        unimplemented!()
    }
}

pub enum PostWalkAction {
    // Indicates the need to continue iterating
    Continue,
    // Indicates that it is necessary to stop continuing to iterate
    Break,
}

pub type ChildInodeHandler<'a> =
    &'a mut dyn FnMut(Option<Arc<dyn RafsInode>>, OsString, u64, u64) -> Result<PostWalkAction>;

/// Trait to access metadata and data for an inode.
///
/// The RAFS filesystem is a readonly filesystem, so does its inodes. The `RafsInode` trait acts
/// as field accessors for those readonly inodes, to hide implementation details.
pub trait RafsInode: Any {
    /// Validate the node for data integrity.
    ///
    /// The inode object may be transmuted from a raw buffer, read from an external file, so the
    /// caller must validate it before accessing any fields.
    fn validate(&self, max_inode: Inode, chunk_size: u64) -> Result<()>;

    /// Get `Entry` of the inode.
    fn get_entry(&self) -> Entry;

    /// Get `Attr` of the inode.
    fn get_attr(&self) -> Attr;

    /// Get file name size of the inode.
    fn get_name_size(&self) -> u16;

    /// Get symlink target of the inode if it's a symlink.
    fn get_symlink(&self) -> Result<OsString>;

    /// Get size of symlink.
    fn get_symlink_size(&self) -> u16;

    /// Get child inode of a directory by name.
    fn get_child_by_name(&self, name: &OsStr) -> Result<Arc<dyn RafsInode>>;

    fn walk_children_inodes(&self, entry_offset: u64, handler: ChildInodeHandler) -> Result<()>;

    /// Get child inode of a directory by child index, child index starting at 0.
    fn get_child_by_index(&self, idx: u32) -> Result<Arc<dyn RafsInode>>;

    /// Get number of directory's child inode.
    fn get_child_count(&self) -> u32;

    /// Get the index into the inode table of the directory's first child.
    fn get_child_index(&self) -> Result<u32>;

    /// Get number of data chunk of a normal file.
    fn get_chunk_count(&self) -> u32;

    /// Get chunk info object for a chunk.
    fn get_chunk_info(&self, idx: u32) -> Result<Arc<dyn BlobChunkInfo>>;

    /// Check whether the inode has extended attributes.
    fn has_xattr(&self) -> bool;

    /// Get the value of xattr with key `name`.
    fn get_xattr(&self, name: &OsStr) -> Result<Option<XattrValue>>;

    /// Get all xattr keys.
    fn get_xattrs(&self) -> Result<Vec<XattrName>>;

    /// Check whether the inode is a directory.
    fn is_dir(&self) -> bool;

    /// Check whether the inode is a symlink.
    fn is_symlink(&self) -> bool;

    /// Check whether the inode is a regular file.
    fn is_reg(&self) -> bool;

    /// Check whether the inode is a hardlink.
    fn is_hardlink(&self) -> bool;

    /// Get the inode number of the inode.
    fn ino(&self) -> u64;

    /// Get file name of the inode.
    fn name(&self) -> OsString;

    /// Get inode number of the parent directory.
    fn parent(&self) -> u64;

    /// Get real device number of the inode.
    fn rdev(&self) -> u32;

    /// Get flags of the inode.
    fn flags(&self) -> u64;

    /// Get project id associated with the inode.
    fn projid(&self) -> u32;

    /// Get data size of the inode.
    fn size(&self) -> u64;

    /// Check whether the inode has no content.
    fn is_empty_size(&self) -> bool {
        self.size() == 0
    }

    /// Get digest value of the inode metadata.
    fn get_digest(&self) -> RafsDigest;

    /// Collect all descendants of the inode for image building.
    fn collect_descendants_inodes(
        &self,
        descendants: &mut Vec<Arc<dyn RafsInode>>,
    ) -> Result<usize>;

    /// Allocate blob io vectors to read file data in range [offset, offset + size).
    fn alloc_bio_vecs(&self, offset: u64, size: usize, user_io: bool) -> Result<Vec<BlobIoVec>>;

    fn as_any(&self) -> &dyn Any;

    fn walk_chunks(
        &self,
        cb: &mut dyn FnMut(&dyn BlobChunkInfo) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let chunk_count = self.get_chunk_count();
        for i in 0..chunk_count {
            cb(self.get_chunk_info(i)?.as_ref())?;
        }
        Ok(())
    }
}

/// Trait to store Rafs meta block and validate alignment.
pub trait RafsStore {
    /// Write the Rafs filesystem metadata to a writer.
    fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize>;
}

bitflags! {
    /// Rafs filesystem feature flags.
    #[derive(Serialize)]
    pub struct RafsSuperFlags: u64 {
        /// V5: Data chunks are not compressed.
        const COMPRESS_NONE = 0x0000_0001;
        /// V5: Data chunks are compressed with lz4_block.
        const COMPRESS_LZ4_BLOCK = 0x0000_0002;
        /// V5: Use blake3 hash algorithm to calculate digest.
        const DIGESTER_BLAKE3 = 0x0000_0004;
        /// V5: Use sha256 hash algorithm to calculate digest.
        const DIGESTER_SHA256 = 0x0000_0008;
        /// Inode has explicit uid gid fields.
        ///
        /// If unset, use nydusd process euid/egid for all inodes at runtime.
        const EXPLICIT_UID_GID = 0x0000_0010;
        /// Inode has extended attributes.
        const HAS_XATTR = 0x0000_0020;
        // V5: Data chunks are compressed with gzip
        const COMPRESS_GZIP = 0x0000_0040;
        // V5: Data chunks are compressed with zstd
        const COMPRESS_ZSTD = 0x0000_0080;
    }
}

impl Default for RafsSuperFlags {
    fn default() -> Self {
        RafsSuperFlags::empty()
    }
}

impl Display for RafsSuperFlags {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{:?}", self)?;
        Ok(())
    }
}

/// Rafs filesystem meta-data cached from on disk RAFS super block.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct RafsSuperMeta {
    /// Filesystem magic number.
    pub magic: u32,
    /// Filesystem version number.
    pub version: u32,
    /// Size of on disk super block.
    pub sb_size: u32,
    /// Inode number of root inode.
    pub root_inode: Inode,
    /// Chunk size.
    pub chunk_size: u32,
    /// Number of inodes in the filesystem.
    pub inodes_count: u64,
    /// V5: superblock flags for Rafs v5.
    pub flags: RafsSuperFlags,
    /// Number of inode entries in inode offset table.
    pub inode_table_entries: u32,
    /// Offset of the inode offset table into the metadata blob.
    pub inode_table_offset: u64,
    /// Size of blob information table.
    pub blob_table_size: u32,
    /// Offset of the blob information table into the metadata blob.
    pub blob_table_offset: u64,
    /// Size of extended blob information table.
    pub extended_blob_table_offset: u64,
    /// Offset of the extended blob information table into the metadata blob.
    pub extended_blob_table_entries: u32,
    /// Start of data prefetch range.
    pub blob_readahead_offset: u32,
    /// Size of data prefetch range.
    pub blob_readahead_size: u32,
    /// Offset of the inode prefetch table into the metadata blob.
    pub prefetch_table_offset: u64,
    /// Size of the inode prefetch table.
    pub prefetch_table_entries: u32,
    /// Default attribute timeout value.
    pub attr_timeout: Duration,
    /// Default inode timeout value.
    pub entry_timeout: Duration,
    pub meta_blkaddr: u32,
    pub root_nid: u16,
    pub is_chunk_dict: bool,
    /// Offset of the chunk table
    pub chunk_table_offset: u64,
    /// Size  of the chunk table
    pub chunk_table_size: u64,
}

impl RafsSuperMeta {
    /// Check whether the superblock is for Rafs v5 filesystems.
    pub fn is_v5(&self) -> bool {
        self.version == RAFS_SUPER_VERSION_V5
    }

    /// Check whether the superblock is for Rafs v6 filesystems.
    pub fn is_v6(&self) -> bool {
        self.version == RAFS_SUPER_VERSION_V6
    }

    pub fn is_chunk_dict(&self) -> bool {
        self.is_chunk_dict
    }

    /// Check whether the explicit UID/GID feature has been enable or not.
    pub fn explicit_uidgid(&self) -> bool {
        self.flags.contains(RafsSuperFlags::EXPLICIT_UID_GID)
    }

    /// Check whether the filesystem supports extended attribute or not.
    pub fn has_xattr(&self) -> bool {
        self.flags.contains(RafsSuperFlags::HAS_XATTR)
    }

    /// Get compression algorithm to handle chunk data for the filesystem.
    pub fn get_compressor(&self) -> compress::Algorithm {
        if self.is_v5() || self.is_v6() {
            self.flags.into()
        } else {
            compress::Algorithm::None
        }
    }

    /// V5: get message digest algorithm to validate chunk data for the filesystem.
    pub fn get_digester(&self) -> digest::Algorithm {
        if self.is_v5() || self.is_v6() {
            self.flags.into()
        } else {
            digest::Algorithm::Blake3
        }
    }
}

impl Default for RafsSuperMeta {
    fn default() -> Self {
        RafsSuperMeta {
            magic: 0,
            version: 0,
            sb_size: 0,
            inodes_count: 0,
            root_inode: 0,
            chunk_size: 0,
            flags: RafsSuperFlags::empty(),
            inode_table_entries: 0,
            inode_table_offset: 0,
            blob_table_size: 0,
            blob_table_offset: 0,
            extended_blob_table_offset: 0,
            extended_blob_table_entries: 0,
            blob_readahead_offset: 0,
            blob_readahead_size: 0,
            prefetch_table_offset: 0,
            prefetch_table_entries: 0,
            attr_timeout: Duration::from_secs(RAFS_DEFAULT_ATTR_TIMEOUT),
            entry_timeout: Duration::from_secs(RAFS_DEFAULT_ENTRY_TIMEOUT),
            meta_blkaddr: 0,
            root_nid: 0,
            is_chunk_dict: false,
            chunk_table_offset: 0,
            chunk_table_size: 0,
        }
    }
}

/// Rafs metadata working mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RafsMode {
    /// Directly mapping and accessing metadata into process by mmap().
    Direct,
    /// Read metadata into memory before using.
    Cached,
}

impl FromStr for RafsMode {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "direct" => Ok(Self::Direct),
            "cached" => Ok(Self::Cached),
            _ => Err(einval!("rafs mode should be direct or cached")),
        }
    }
}

impl Display for RafsMode {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Self::Direct => write!(f, "direct"),
            Self::Cached => write!(f, "cached"),
        }
    }
}

/// Cached Rafs super block and inode information.
pub struct RafsSuper {
    /// Rafs metadata working mode.
    pub mode: RafsMode,
    /// Whether validate data read from storage backend.
    pub validate_digest: bool,
    /// Cached metadata from on disk super block.
    pub meta: RafsSuperMeta,
    /// Rafs filesystem super block.
    pub superblock: Arc<dyn RafsSuperBlock>,
}

impl Default for RafsSuper {
    fn default() -> Self {
        Self {
            mode: RafsMode::Direct,
            validate_digest: false,
            meta: RafsSuperMeta::default(),
            superblock: Arc::new(NoopSuperBlock::new()),
        }
    }
}

impl RafsSuper {
    /// Create a new `RafsSuper` instance from a `RafsConfig` object.
    pub fn new(conf: &RafsConfig) -> Result<Self> {
        Ok(Self {
            mode: RafsMode::from_str(conf.mode.as_str())?,
            validate_digest: conf.digest_validate,
            ..Default::default()
        })
    }

    /// Destroy the filesystem super block.
    pub fn destroy(&mut self) {
        Arc::get_mut(&mut self.superblock)
            .expect("Inodes are no longer used.")
            .destroy();
    }

    /// Load Rafs super block from a metadata file.
    pub fn load_from_metadata<P: AsRef<Path>>(
        path: P,
        mode: RafsMode,
        validate_digest: bool,
    ) -> Result<Self> {
        // open bootstrap file
        let file = OpenOptions::new()
            .read(true)
            .write(false)
            .open(path.as_ref())?;
        let mut rs = RafsSuper {
            mode,
            validate_digest,
            ..Default::default()
        };
        let mut reader = Box::new(file) as RafsIoReader;

        rs.load(&mut reader)?;

        Ok(rs)
    }

    pub fn load_chunk_dict_from_metadata(path: &Path) -> Result<Self> {
        // open bootstrap file
        let file = OpenOptions::new().read(true).write(false).open(path)?;
        let mut rs = RafsSuper {
            mode: RafsMode::Direct,
            validate_digest: true,
            ..Default::default()
        };
        let mut reader = Box::new(file) as RafsIoReader;

        rs.meta.is_chunk_dict = true;
        rs.load(&mut reader)?;

        Ok(rs)
    }

    /// Load RAFS metadata and optionally cache inodes.
    pub fn load(&mut self, r: &mut RafsIoReader) -> Result<()> {
        // Try to load the filesystem as Rafs v5
        if self.try_load_v5(r)? {
            return Ok(());
        }

        if self.try_load_v6(r)? {
            return Ok(());
        }

        Err(einval!("invalid superblock version number"))
    }

    /// Update the filesystem metadata and storage backend.
    pub fn update(&self, r: &mut RafsIoReader) -> RafsResult<()> {
        if self.meta.is_v5() {
            self.skip_v5_superblock(r)
                .map_err(RafsError::FillSuperblock)?;
        }

        self.superblock.update(r)
    }

    /// Store RAFS metadata to backend storage.
    pub fn store(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        if self.meta.is_v5() {
            return self.store_v5(w);
        }

        Err(einval!("invalid superblock version number"))
    }

    /// Get an inode from an inode number, optionally validating the inode metadata.
    pub fn get_inode(&self, ino: Inode, digest_validate: bool) -> Result<Arc<dyn RafsInode>> {
        self.superblock.get_inode(ino, digest_validate)
    }

    /// Get the maximum inode number supported by the filesystem instance.
    pub fn get_max_ino(&self) -> Inode {
        self.superblock.get_max_ino()
    }

    /// Convert an inode number to a file path.
    pub fn path_from_ino(&self, ino: Inode) -> Result<PathBuf> {
        if ino == self.superblock.root_ino() {
            return Ok(self.get_inode(ino, false)?.name().into());
        }

        let mut path = PathBuf::new();
        let mut cur_ino = ino;
        let mut inode;

        loop {
            inode = self.get_inode(cur_ino, false)?;
            let e: PathBuf = inode.name().into();
            path = e.join(path);

            if inode.ino() == self.superblock.root_ino() {
                break;
            } else {
                cur_ino = inode.parent();
            }
        }

        Ok(path)
    }

    /// Convert a file path to an inode number.
    pub fn ino_from_path(&self, f: &Path) -> Result<u64> {
        let root_ino = self.superblock.root_ino();
        if f == Path::new("/") {
            return Ok(root_ino);
        }

        if !f.starts_with("/") {
            return Err(einval!());
        }

        let mut parent = self.get_inode(root_ino, self.validate_digest)?;

        let entries = f
            .components()
            .filter(|comp| *comp != Component::RootDir)
            .map(|comp| match comp {
                Component::Normal(name) => Some(name),
                Component::ParentDir => Some(OsStr::from_bytes(DOTDOT.as_bytes())),
                Component::CurDir => Some(OsStr::from_bytes(DOT.as_bytes())),
                _ => None,
            })
            .collect::<Vec<_>>();

        if entries.is_empty() {
            warn!("Path can't be parsed {:?}", f);
            return Err(enoent!());
        }

        for p in entries {
            if p.is_none() {
                error!("Illegal specified path {:?}", f);
                return Err(einval!());
            }

            // Safe because it already checks if p is None above.
            match parent.get_child_by_name(p.unwrap()) {
                Ok(p) => parent = p,
                Err(_) => {
                    warn!("File {:?} not in rafs", p.unwrap());
                    return Err(enoent!());
                }
            }
        }

        Ok(parent.ino())
    }

    /// Prefetch filesystem and file data to improve performance.
    ///
    /// To improve application filesystem access performance, the filesystem may prefetch file or
    /// metadata in advance. There are ways to configure the file list to be prefetched.
    /// 1. Static file prefetch list configured during image building, recorded in prefetch list
    ///    in Rafs v5 file system metadata.
    ///     Base on prefetch table which is persisted to bootstrap when building image.
    /// 2. Dynamic file prefetch list configured by command line. The dynamic file prefetch list
    ///    has higher priority and the static file prefetch list will be ignored if there's dynamic
    ///    prefetch list. When a directory is specified for dynamic prefetch list, all sub directory
    ///    and files under the directory will be prefetched.
    ///
    /// Each inode passed into should correspond to directory. And it already does the file type
    /// check inside.
    pub fn prefetch_files(
        &self,
        r: &mut RafsIoReader,
        root_ino: Inode,
        files: Option<Vec<Inode>>,
        fetcher: &dyn Fn(&mut BlobIoVec),
    ) -> RafsResult<bool> {
        // Try to prefetch files according to the list specified by the `--prefetch-files` option.
        if let Some(files) = files {
            // Avoid prefetching multiple times for hardlinks to the same file.
            let mut hardlinks: HashSet<u64> = HashSet::new();
            let mut state = BlobIoMerge::default();
            for f_ino in files {
                self.prefetch_data(f_ino, &mut state, &mut hardlinks, fetcher)
                    .map_err(|e| RafsError::Prefetch(e.to_string()))?;
            }
            for (_id, mut desc) in state.drain() {
                fetcher(&mut desc);
            }
            // Flush the pending prefetch requests.
            Ok(false)
        } else if self.meta.is_v5() {
            self.prefetch_data_v5(r, root_ino, fetcher)
        } else if self.meta.is_v6() {
            self.prefetch_data_v6(r, root_ino, fetcher)
        } else {
            Err(RafsError::Prefetch(
                "Unknown filesystem version, prefetch disabled".to_string(),
            ))
        }
    }

    #[inline]
    fn prefetch_inode<F>(
        inode: &Arc<dyn RafsInode>,
        state: &mut BlobIoMerge,
        hardlinks: &mut HashSet<u64>,
        prefetcher: F,
    ) -> Result<()>
    where
        F: Fn(&mut BlobIoMerge),
    {
        // Check for duplicated hardlinks.
        if inode.is_hardlink() {
            if hardlinks.contains(&inode.ino()) {
                return Ok(());
            } else {
                hardlinks.insert(inode.ino());
            }
        }

        let descs = inode.alloc_bio_vecs(0, inode.size() as usize, false)?;
        for desc in descs {
            state.append(desc);
            prefetcher(state);
        }

        Ok(())
    }

    fn prefetch_data<F>(
        &self,
        ino: u64,
        state: &mut BlobIoMerge,
        hardlinks: &mut HashSet<u64>,
        fetcher: F,
    ) -> Result<()>
    where
        F: Fn(&mut BlobIoVec),
    {
        let try_prefetch = |state: &mut BlobIoMerge| {
            if let Some(desc) = state.get_current_element() {
                // Issue a prefetch request since target is large enough.
                // As files belonging to the same directory are arranged in adjacent,
                // it should fetch a range of blob in batch.
                if (desc.bi_size as u64) >= RAFS_DEFAULT_CHUNK_SIZE {
                    trace!("fetching head bio size {}", desc.bi_size);
                    fetcher(desc);
                    desc.reset();
                }
            }
        };

        let inode = self
            .superblock
            .get_inode(ino, self.validate_digest)
            .map_err(|_e| enoent!("Can't find inode"))?;

        if inode.is_dir() {
            let mut descendants = Vec::new();
            let _ = inode.collect_descendants_inodes(&mut descendants)?;
            for i in descendants.iter() {
                Self::prefetch_inode(i, state, hardlinks, try_prefetch)?;
            }
        } else if !inode.is_empty_size() && inode.is_reg() {
            // An empty regular file will also be packed into nydus image,
            // then it has a size of zero.
            // Moreover, for rafs v5, symlink has size of zero but non-zero size
            // for symlink size. For rafs v6, symlink size is also represented by i_size.
            // So we have to restrain the condition here.
            Self::prefetch_inode(&inode, state, hardlinks, try_prefetch)?;
        }

        Ok(())
    }

    /// Get prefetched inos
    pub fn get_prefetched_inos(&self, bootstrap: &mut RafsIoReader) -> Result<Vec<u32>> {
        if self.meta.is_v5() {
            let mut pt = RafsV5PrefetchTable::new();
            pt.load_prefetch_table_from(
                bootstrap,
                self.meta.prefetch_table_offset,
                self.meta.prefetch_table_entries as usize,
            )?;
            Ok(pt.inodes)
        } else {
            let mut pt = RafsV6PrefetchTable::new();
            pt.load_prefetch_table_from(
                bootstrap,
                self.meta.prefetch_table_offset,
                self.meta.prefetch_table_entries as usize,
            )?;
            Ok(pt.inodes)
        }
    }

    /// Walkthrough the file tree rooted at ino, calling cb for each file or directory
    /// in the tree by DFS order, including ino, please ensure ino is a directory.
    pub fn walk_dir(
        &self,
        ino: Inode,
        parent: Option<&PathBuf>,
        cb: &mut dyn FnMut(&dyn RafsInode, &Path) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let inode = self.get_inode(ino, false)?;
        if !inode.is_dir() {
            bail!("inode {} is not a directory", ino);
        }
        self.walk_dir_inner(inode.as_ref(), parent, cb)
    }

    fn walk_dir_inner(
        &self,
        inode: &dyn RafsInode,
        parent: Option<&PathBuf>,
        cb: &mut dyn FnMut(&dyn RafsInode, &Path) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let path = if let Some(parent) = parent {
            parent.join(inode.name())
        } else {
            PathBuf::from("/")
        };
        cb(inode, &path)?;
        if !inode.is_dir() {
            return Ok(());
        }
        let child_count = inode.get_child_count();
        for idx in 0..child_count {
            let child = inode.get_child_by_index(idx)?;
            self.walk_dir_inner(child.as_ref(), Some(&path), cb)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rafs_mode() {
        assert!(RafsMode::from_str("").is_err());
        assert!(RafsMode::from_str("directed").is_err());
        assert!(RafsMode::from_str("Direct").is_err());
        assert!(RafsMode::from_str("Cached").is_err());
        assert_eq!(RafsMode::from_str("direct").unwrap(), RafsMode::Direct);
        assert_eq!(RafsMode::from_str("cached").unwrap(), RafsMode::Cached);
        assert_eq!(&format!("{}", RafsMode::Direct), "direct");
        assert_eq!(&format!("{}", RafsMode::Cached), "cached");
    }
}
