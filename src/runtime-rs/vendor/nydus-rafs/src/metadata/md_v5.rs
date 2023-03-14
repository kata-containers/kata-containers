// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use storage::device::BlobChunkFlags;

use super::cached_v5::CachedSuperBlockV5;
use super::direct_v5::DirectSuperBlockV5;
use super::layout::v5::{RafsV5PrefetchTable, RafsV5SuperBlock};
use super::*;

impl RafsSuper {
    pub(crate) fn try_load_v5(&mut self, r: &mut RafsIoReader) -> Result<bool> {
        let end = r.seek_to_end(0)?;
        r.seek_to_offset(0)?;
        let mut sb = RafsV5SuperBlock::new();
        r.read_exact(sb.as_mut())?;
        if !sb.is_rafs_v5() {
            return Ok(false);
        }
        sb.validate(end)?;

        self.meta.magic = sb.magic();
        self.meta.version = sb.version();
        self.meta.sb_size = sb.sb_size();
        self.meta.chunk_size = sb.block_size();
        self.meta.flags = RafsSuperFlags::from_bits(sb.flags())
            .ok_or_else(|| einval!(format!("invalid super flags {:x}", sb.flags())))?;
        info!("rafs superblock features: {}", self.meta.flags);

        self.meta.inodes_count = sb.inodes_count();
        self.meta.inode_table_entries = sb.inode_table_entries();
        self.meta.inode_table_offset = sb.inode_table_offset();
        self.meta.blob_table_offset = sb.blob_table_offset();
        self.meta.blob_table_size = sb.blob_table_size();
        self.meta.extended_blob_table_offset = sb.extended_blob_table_offset();
        self.meta.extended_blob_table_entries = sb.extended_blob_table_entries();
        self.meta.prefetch_table_entries = sb.prefetch_table_entries();
        self.meta.prefetch_table_offset = sb.prefetch_table_offset();

        match self.mode {
            RafsMode::Direct => {
                let mut inodes = DirectSuperBlockV5::new(&self.meta, self.validate_digest);
                inodes.load(r)?;
                self.superblock = Arc::new(inodes);
            }
            RafsMode::Cached => {
                let mut inodes = CachedSuperBlockV5::new(self.meta, self.validate_digest);
                inodes.load(r)?;
                self.superblock = Arc::new(inodes);
            }
        }

        Ok(true)
    }

    pub(crate) fn store_v5(&self, w: &mut dyn RafsIoWrite) -> Result<usize> {
        let mut sb = RafsV5SuperBlock::new();

        sb.set_magic(self.meta.magic);
        sb.set_version(self.meta.version);
        sb.set_sb_size(self.meta.sb_size);
        sb.set_block_size(self.meta.chunk_size);
        sb.set_flags(self.meta.flags.bits());

        sb.set_inodes_count(self.meta.inodes_count);
        sb.set_inode_table_entries(self.meta.inode_table_entries);
        sb.set_inode_table_offset(self.meta.inode_table_offset);
        sb.set_blob_table_offset(self.meta.blob_table_offset);
        sb.set_blob_table_size(self.meta.blob_table_size);
        sb.set_extended_blob_table_offset(self.meta.extended_blob_table_offset);
        sb.set_extended_blob_table_entries(self.meta.extended_blob_table_entries);
        sb.set_prefetch_table_offset(self.meta.prefetch_table_offset);
        sb.set_prefetch_table_entries(self.meta.prefetch_table_entries);

        w.write_all(sb.as_ref())?;
        let meta_size = w.seek_to_end()?;
        if meta_size > RAFS_MAX_METADATA_SIZE as u64 {
            return Err(einval!("metadata blob is too big"));
        }
        sb.validate(meta_size)?;
        trace!("written superblock: {}", &sb);

        Ok(meta_size as usize)
    }

    pub(crate) fn prefetch_data_v5<F>(
        &self,
        r: &mut RafsIoReader,
        root_ino: Inode,
        fetcher: F,
    ) -> RafsResult<bool>
    where
        F: Fn(&mut BlobIoVec),
    {
        let hint_entries = self.meta.prefetch_table_entries as usize;
        if hint_entries == 0 {
            return Ok(false);
        }

        // Try to prefetch according to the list of files specified by the
        // builder's `--prefetch-policy fs` option.
        let mut prefetch_table = RafsV5PrefetchTable::new();
        prefetch_table
            .load_prefetch_table_from(r, self.meta.prefetch_table_offset, hint_entries)
            .map_err(|e| {
                RafsError::Prefetch(format!(
                    "Failed in loading hint prefetch table at offset {}. {:?}",
                    self.meta.prefetch_table_offset, e
                ))
            })?;

        let mut hardlinks: HashSet<u64> = HashSet::new();
        let mut state = BlobIoMerge::default();
        let mut found_root_inode = false;
        for ino in prefetch_table.inodes {
            // Inode number 0 is invalid, it was added because prefetch table has to be aligned.
            if ino == 0 {
                break;
            }
            if ino as Inode == root_ino {
                found_root_inode = true;
            }
            debug!("hint prefetch inode {}", ino);
            self.prefetch_data(ino as u64, &mut state, &mut hardlinks, &fetcher)
                .map_err(|e| RafsError::Prefetch(e.to_string()))?;
        }
        for (_id, mut desc) in state.drain() {
            fetcher(&mut desc);
        }

        Ok(found_root_inode)
    }

    pub(crate) fn skip_v5_superblock(&self, r: &mut RafsIoReader) -> Result<()> {
        let _ = RafsV5SuperBlock::read(r)?;

        Ok(())
    }

    fn merge_chunks_io(orig: &mut BlobIoVec, more: &[BlobIoVec]) {
        if orig.bi_vec.is_empty() {
            return;
        }

        // safe to unwrap since it is already checked before
        let mut cki = &orig.bi_vec.last().unwrap().chunkinfo;
        let mut last_chunk = cki.as_base();

        // caller should ensure that `window_base` won't overlap last chunk of user IO.
        for d in more {
            let head_ck = &d.bi_vec[0].chunkinfo.as_base();

            if last_chunk.compressed_offset() + last_chunk.compressed_size() as u64
                != head_ck.compressed_offset()
            {
                break;
            }

            // Safe to unwrap since bi_vec shouldn't be empty
            cki = &d.bi_vec.last().unwrap().chunkinfo;
            last_chunk = cki.as_base();
            orig.bi_vec.extend_from_slice(d.bi_vec.as_slice());
        }
    }

    // TODO: Add a UT for me.
    // `window_base` is calculated by caller, which MUST be the chunk that does
    // not overlap user IO's chunk.
    // V5 rafs tries to amplify user IO by expanding more chunks to user IO and
    // expect that those chunks are likely to be continuous with user IO's chunks.
    pub(crate) fn amplify_io(
        &self,
        max_size: u32,
        descs: &mut [BlobIoVec],
        inode: &Arc<dyn RafsInode>,
        window_base: u64,
        mut window_size: u64,
    ) -> Result<()> {
        let inode_size = inode.size();

        let last_desc = if let Some(d) = descs.last_mut() {
            d
        } else {
            return Ok(());
        };

        // Read left content of current file.
        if window_base < inode_size {
            let size = inode_size - window_base;
            let sz = std::cmp::min(size, window_size);
            let amplified_io_vec = inode.alloc_bio_vecs(window_base, sz as usize, false)?;
            debug_assert!(!amplified_io_vec.is_empty() && !amplified_io_vec[0].bi_vec.is_empty());
            // caller should ensure that `window_base` won't overlap last chunk of user IO.
            Self::merge_chunks_io(last_desc, &amplified_io_vec);
            window_size -= sz;
            if window_size == 0 {
                return Ok(());
            }
        }

        // Read more small files.
        let mut next_ino = inode.ino();
        while window_size > 0 {
            next_ino += 1;
            if let Ok(ni) = self.get_inode(next_ino, false) {
                if ni.is_reg() {
                    let next_size = ni.size();
                    if next_size > max_size as u64 {
                        break;
                    }

                    if next_size == 0 {
                        continue;
                    }

                    let sz = std::cmp::min(window_size, next_size);
                    let amplified_io_vec = ni.alloc_bio_vecs(0, sz as usize, false)?;
                    debug_assert!(
                        !amplified_io_vec.is_empty() && !amplified_io_vec[0].bi_vec.is_empty()
                    );
                    if last_desc.has_same_blob(&amplified_io_vec[0]) {
                        // caller should ensure that `window_base` won't overlap last chunk
                        Self::merge_chunks_io(last_desc, &amplified_io_vec);
                    } else {
                        break;
                    }
                    window_size -= sz;
                }
            } else {
                break;
            }
        }

        Ok(())
    }
}

/// Represents backend storage chunked IO address for V5 since V5 format has to
/// load below chunk address from rafs layer and pass it to storage layer.
pub struct V5IoChunk {
    // block hash
    pub block_id: Arc<RafsDigest>,
    // blob containing the block
    pub blob_index: u32,
    // chunk index in blob
    pub index: u32,
    // position of the block within the file
    // offset of the block within the blob
    pub compressed_offset: u64,
    pub uncompressed_offset: u64,
    // size of the block, compressed
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub flags: BlobChunkFlags,
}

impl BlobChunkInfo for V5IoChunk {
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
    impl_getter!(compressed_offset, compressed_offset, u64);
    impl_getter!(compressed_size, compressed_size, u32);
    impl_getter!(uncompressed_offset, uncompressed_offset, u64);
    impl_getter!(uncompressed_size, uncompressed_size, u32);
}

#[cfg(test)]
mod tests {
    // TODO: add unit test cases for RafsSuper::{try_load_v5, amplify_io}
}
