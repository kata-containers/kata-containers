// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::any::Any;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::Result;
use std::os::unix::ffi::OsStrExt;
use std::sync::Arc;

use fuse_backend_rs::abi::fuse_abi;
use fuse_backend_rs::api::filesystem::Entry;
use nydus_utils::{digest::RafsDigest, ByteSize};

use storage::device::{BlobChunkInfo, BlobInfo, BlobIoVec};

use super::mock_chunk::MockChunkInfo;
use super::mock_super::CHUNK_SIZE;
use crate::metadata::layout::v5::{
    rafsv5_alloc_bio_vecs, RafsV5BlobTable, RafsV5Inode, RafsV5InodeChunkOps, RafsV5InodeFlags,
    RafsV5InodeOps,
};
use crate::metadata::{
    layout::{XattrName, XattrValue},
    ChildInodeHandler, Inode, RafsInode, RafsSuperMeta, RAFS_ATTR_BLOCK_SIZE,
};
use storage::device::v5::BlobV5ChunkInfo;

#[derive(Default, Clone, Debug)]
#[allow(unused)]
pub struct MockInode {
    i_ino: Inode,
    i_name: OsString,
    i_digest: RafsDigest,
    i_parent: u64,
    i_mode: u32,
    i_projid: u32,
    i_uid: u32,
    i_gid: u32,
    i_flags: RafsV5InodeFlags,
    i_size: u64,
    i_blocks: u64,
    i_nlink: u32,
    i_child_idx: u32,
    i_child_cnt: u32,
    // extra info need cache
    i_blksize: u32,
    i_rdev: u32,
    i_mtime_nsec: u32,
    i_mtime: u64,
    i_target: OsString, // for symbol link
    i_xattr: HashMap<OsString, Vec<u8>>,
    i_data: Vec<Arc<MockChunkInfo>>,
    i_child: Vec<Arc<MockInode>>,
    i_blob_table: Arc<RafsV5BlobTable>,
    i_meta: Arc<RafsSuperMeta>,
}

impl MockInode {
    pub fn mock(ino: Inode, size: u64, chunks: Vec<Arc<MockChunkInfo>>) -> Self {
        Self {
            i_ino: ino,
            i_size: size,
            i_child_cnt: chunks.len() as u32,
            i_data: chunks,
            // Ignore other bits for now.
            i_mode: libc::S_IFREG as u32,
            // It can't be changed yet.
            i_blksize: CHUNK_SIZE,
            ..Default::default()
        }
    }
}

impl RafsInode for MockInode {
    fn validate(&self, _max_inode: Inode, _chunk_size: u64) -> Result<()> {
        if self.is_symlink() && self.i_target.is_empty() {
            return Err(einval!("invalid inode"));
        }
        Ok(())
    }

    #[inline]
    fn get_entry(&self) -> Entry {
        Entry {
            attr: self.get_attr().into(),
            inode: self.i_ino,
            generation: 0,
            attr_flags: 0,
            attr_timeout: self.i_meta.attr_timeout,
            entry_timeout: self.i_meta.entry_timeout,
        }
    }

    #[inline]
    fn get_attr(&self) -> fuse_abi::Attr {
        fuse_abi::Attr {
            ino: self.i_ino,
            size: self.i_size,
            blocks: self.i_blocks,
            mode: self.i_mode,
            nlink: self.i_nlink as u32,
            blksize: RAFS_ATTR_BLOCK_SIZE,
            rdev: self.i_rdev,
            ..Default::default()
        }
    }

    fn walk_children_inodes(&self, _entry_offset: u64, _handler: ChildInodeHandler) -> Result<()> {
        todo!()
    }

    fn get_name_size(&self) -> u16 {
        self.i_name.byte_size() as u16
    }

    fn get_symlink(&self) -> Result<OsString> {
        if !self.is_symlink() {
            Err(einval!("inode is not a symlink"))
        } else {
            Ok(self.i_target.clone())
        }
    }

    fn get_symlink_size(&self) -> u16 {
        if self.is_symlink() {
            self.i_target.byte_size() as u16
        } else {
            0
        }
    }

    fn get_child_by_name(&self, name: &OsStr) -> Result<Arc<dyn RafsInode>> {
        let idx = self
            .i_child
            .binary_search_by(|c| c.i_name.as_os_str().cmp(name))
            .map_err(|_| enoent!())?;
        Ok(self.i_child[idx].clone())
    }

    #[inline]
    fn get_child_by_index(&self, index: u32) -> Result<Arc<dyn RafsInode>> {
        Ok(self.i_child[index as usize].clone())
    }

    #[inline]
    fn get_child_count(&self) -> u32 {
        self.i_child_cnt
    }

    fn get_child_index(&self) -> Result<u32> {
        Ok(self.i_child_idx)
    }

    fn get_chunk_count(&self) -> u32 {
        self.get_child_count()
    }

    #[inline]
    fn get_chunk_info(&self, idx: u32) -> Result<Arc<dyn BlobChunkInfo>> {
        Ok(self.i_data[idx as usize].clone())
    }

    fn has_xattr(&self) -> bool {
        self.i_flags.contains(RafsV5InodeFlags::XATTR)
    }

    #[inline]
    fn get_xattr(&self, name: &OsStr) -> Result<Option<XattrValue>> {
        Ok(self.i_xattr.get(name).cloned())
    }

    fn get_xattrs(&self) -> Result<Vec<XattrName>> {
        Ok(self
            .i_xattr
            .keys()
            .map(|k| k.as_bytes().to_vec())
            .collect::<Vec<XattrName>>())
    }

    fn is_dir(&self) -> bool {
        self.i_mode & libc::S_IFMT as u32 == libc::S_IFDIR as u32
    }

    fn is_symlink(&self) -> bool {
        self.i_mode & libc::S_IFMT as u32 == libc::S_IFLNK as u32
    }

    fn is_reg(&self) -> bool {
        self.i_mode & libc::S_IFMT as u32 == libc::S_IFREG as u32
    }

    fn is_hardlink(&self) -> bool {
        !self.is_dir() && self.i_nlink > 1
    }

    fn name(&self) -> OsString {
        self.i_name.clone()
    }

    fn flags(&self) -> u64 {
        self.i_flags.bits()
    }

    fn get_digest(&self) -> RafsDigest {
        self.i_digest
    }

    fn collect_descendants_inodes(
        &self,
        descendants: &mut Vec<Arc<dyn RafsInode>>,
    ) -> Result<usize> {
        if !self.is_dir() {
            return Err(enotdir!());
        }

        let mut child_dirs: Vec<Arc<dyn RafsInode>> = Vec::new();

        for child_inode in &self.i_child {
            if child_inode.is_dir() {
                trace!("Got dir {:?}", child_inode.name());
                child_dirs.push(child_inode.clone());
            } else {
                if child_inode.is_empty_size() {
                    continue;
                }
                descendants.push(child_inode.clone());
            }
        }

        for d in child_dirs {
            d.collect_descendants_inodes(descendants)?;
        }

        Ok(0)
    }

    fn alloc_bio_vecs(&self, offset: u64, size: usize, user_io: bool) -> Result<Vec<BlobIoVec>> {
        rafsv5_alloc_bio_vecs(self, offset, size, user_io)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    impl_getter!(ino, i_ino, u64);
    impl_getter!(parent, i_parent, u64);
    impl_getter!(size, i_size, u64);
    impl_getter!(rdev, i_rdev, u32);
    impl_getter!(projid, i_projid, u32);
}

impl RafsV5InodeChunkOps for MockInode {
    fn get_chunk_info_v5(&self, idx: u32) -> Result<Arc<dyn BlobV5ChunkInfo>> {
        Ok(self.i_data[idx as usize].clone())
    }
}

impl RafsV5InodeOps for MockInode {
    fn get_blob_by_index(&self, _idx: u32) -> Result<Arc<BlobInfo>> {
        Ok(Arc::new(BlobInfo::default()))
    }

    fn get_chunk_size(&self) -> u32 {
        CHUNK_SIZE
    }

    fn has_hole(&self) -> bool {
        false
    }

    fn cast_ondisk(&self) -> Result<RafsV5Inode> {
        unimplemented!()
    }
}
