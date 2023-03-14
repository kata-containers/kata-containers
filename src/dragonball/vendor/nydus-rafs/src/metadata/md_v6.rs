// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;
use std::io::Result;
use std::mem::size_of;
use std::sync::Arc;

use super::direct_v6::DirectSuperBlockV6;
use super::layout::v6::{RafsV6PrefetchTable, RafsV6SuperBlock, RafsV6SuperBlockExt};
use super::layout::RAFS_SUPER_VERSION_V6;
use super::*;
use super::{RafsMode, RafsSuper, RafsSuperBlock, RafsSuperFlags};

use crate::RafsIoReader;
use crate::{RafsError, RafsResult};

impl RafsSuper {
    pub(crate) fn try_load_v6(&mut self, r: &mut RafsIoReader) -> Result<bool> {
        let end = r.seek_to_end(0)?;
        r.seek_to_offset(0)?;

        let mut sb = RafsV6SuperBlock::new();
        if sb.load(r).is_err() {
            return Ok(false);
        }
        if !sb.is_rafs_v6() {
            return Ok(false);
        }
        sb.validate(end)?;
        self.meta.magic = sb.magic();
        self.meta.version = RAFS_SUPER_VERSION_V6;

        let mut ext_sb = RafsV6SuperBlockExt::new();
        ext_sb.load(r)?;
        ext_sb.validate()?;
        self.meta.chunk_size = ext_sb.chunk_size();
        self.meta.blob_table_offset = ext_sb.blob_table_offset();
        self.meta.blob_table_size = ext_sb.blob_table_size();
        self.meta.chunk_table_offset = ext_sb.chunk_table_offset();
        self.meta.chunk_table_size = ext_sb.chunk_table_size();
        self.meta.inodes_count = sb.inodes_count();

        self.meta.flags = RafsSuperFlags::from_bits(ext_sb.flags())
            .ok_or_else(|| einval!(format!("invalid super flags {:x}", ext_sb.flags())))?;
        info!("rafs superblock features: {}", self.meta.flags);
        self.meta.meta_blkaddr = sb.s_meta_blkaddr;
        self.meta.root_nid = sb.s_root_nid;

        self.meta.prefetch_table_entries = ext_sb.prefetch_table_size() / size_of::<u32>() as u32;
        self.meta.prefetch_table_offset = ext_sb.prefetch_table_offset();

        trace!(
            "prefetch table offset {} entries {} ",
            self.meta.prefetch_table_offset,
            self.meta.prefetch_table_entries
        );

        match self.mode {
            RafsMode::Direct => {
                let mut sb_v6 = DirectSuperBlockV6::new(&self.meta, self.validate_digest);
                sb_v6.load(r)?;
                self.superblock = Arc::new(sb_v6);
                Ok(true)
            }
            RafsMode::Cached => Err(enosys!("Rafs v6 does not support cached mode")),
        }
    }

    pub(crate) fn prefetch_data_v6<F>(
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
        let mut prefetch_table = RafsV6PrefetchTable::new();
        prefetch_table
            .load_prefetch_table_from(r, self.meta.prefetch_table_offset, hint_entries)
            .map_err(|e| {
                RafsError::Prefetch(format!(
                    "Failed in loading hint prefetch table at offset {}. {:?}",
                    self.meta.prefetch_table_offset, e
                ))
            })?;
        trace!("prefetch table contents {:?}", prefetch_table);

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
        // The left chunks whose size is smaller than 4MB will be fetched here.
        for (_id, mut desc) in state.drain() {
            fetcher(&mut desc);
        }

        Ok(found_root_inode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::RafsStore;
    use crate::BufWriter;
    use std::fs::OpenOptions;
    use std::io::Write;
    use vmm_sys_util::tempfile::TempFile;

    #[test]
    fn test_v6_load_too_small_superblock() {
        let t_file = TempFile::new().unwrap();

        let file = OpenOptions::new()
            .read(true)
            .write(false)
            .open(t_file.as_path())
            .unwrap();
        let mut reader = Box::new(file) as RafsIoReader;
        let mut rs = RafsSuper {
            mode: RafsMode::Direct,
            validate_digest: true,
            ..Default::default()
        };

        assert!(!rs.try_load_v6(&mut reader).unwrap());
    }

    #[test]
    fn test_v6_load_invalid_magic() {
        let t_file = TempFile::new().unwrap();

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(t_file.as_path())
            .unwrap();
        file.write_all(&[0u8; 4096]).unwrap();
        let mut reader = Box::new(file) as RafsIoReader;
        let mut rs = RafsSuper {
            mode: RafsMode::Direct,
            validate_digest: true,
            ..Default::default()
        };

        assert!(!rs.try_load_v6(&mut reader).unwrap());
    }

    #[test]
    #[ignore]
    fn test_v6_load_invalid_superblock() {
        let t_file = TempFile::new().unwrap();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(t_file.as_path())
            .unwrap();
        let sb = RafsV6SuperBlock::new();
        let mut writer = BufWriter::new(file);
        sb.store(&mut writer).unwrap();
        writer.flush().unwrap();

        let file = OpenOptions::new()
            .read(true)
            .write(false)
            .open(t_file.as_path())
            .unwrap();
        let mut reader = Box::new(file) as RafsIoReader;
        let mut rs = RafsSuper {
            mode: RafsMode::Direct,
            validate_digest: true,
            ..Default::default()
        };

        assert!(rs.try_load_v6(&mut reader).is_err());
    }

    /*
    #[test]
    fn test_try_load_v6() {
        let root_dir = &std::env::var("CARGO_MANIFEST_DIR").expect("$CARGO_MANIFEST_DIR");
        let mut source_path = PathBuf::from(root_dir);
        source_path.push("../tests/texture/bootstrap/rafs_v6.boot");

        let file = OpenOptions::new().read(true).write(false).open(path).unwrap();
        let mut reader = Box::new(file) as RafsIoReader;
        let mut rs = RafsSuper {
            mode: RafsMode::Direct,
            validate_digest: true,
            ..Default::default()
        };

        rs.try_load_v6(&mut reader).unwrap();
    }
     */
}
