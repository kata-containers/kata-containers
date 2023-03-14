// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Struct to generate and access blob metadata.
//!
//! Currently, the major responsibility of the blob metadata subsystem is to query chunks covering
//! a specific uncompressed data range. To support this functionality, some blob metadata and
//! a blob header is appended to the compressed blob. So the compressed blob is laid out as
//! `[compressed chunk data], [compressed metadata], [uncompressed header]`.
//!
//! At runtime, the compressed chunk data will be uncompressed into local cache blob file named as
//! `blobid`. The compressed metadata and header will be uncompressed into another file named as
//! `blobid.blob.meata`. Together with the chunk map file `blobid.chunkmap`, they may be used to
//! optimize the communication between blob manager and blob manager clients such as virtiofsd.

use std::any::Any;
use std::fs::OpenOptions;
use std::io::Result;
use std::mem::{size_of, ManuallyDrop};
use std::ops::{Add, BitAnd, Not};
use std::os::unix::io::AsRawFd;
use std::sync::Arc;

use nydus_utils::compress;
use nydus_utils::digest::RafsDigest;

use crate::backend::BlobReader;
use crate::device::{BlobChunkInfo, BlobInfo};
use crate::utils::alloc_buf;

const BLOB_METADATA_MAX_CHUNKS: u32 = 0xf_ffff;
const BLOB_METADATA_MAX_SIZE: u64 = 0x100_0000u64;
const BLOB_METADATA_HEADER_SIZE: u64 = 0x1000u64;
const BLOB_METADATA_RESERVED_SIZE: u64 = BLOB_METADATA_HEADER_SIZE - 44;
const BLOB_METADATA_MAGIC: u32 = 0xb10bb10bu32;
const BLOB_CHUNK_COMP_OFFSET_MASK: u64 = 0xff_ffff_ffff;
const BLOB_CHUNK_UNCOMP_OFFSET_MASK: u64 = 0xfff_ffff_f000;
const BLOB_CHUNK_SIZE_MASK: u64 = 0xff_ffff;
const BLOB_CHUNK_SIZE_LOW_MASK: u64 = 0x0f_ffff;
const BLOB_CHUNK_SIZE_HIGH_MASK: u64 = 0xf0_0000;
const BLOB_CHUNK_SIZE_LOW_SHIFT: u64 = 44;
const BLOB_CHUNK_SIZE_HIGH_COMP_SHIFT: u64 = 20;
const BLOB_CHUNK_SIZE_HIGH_UNCOMP_SHIFT: u64 = 12;
const FILE_SUFFIX: &str = "blob.meta";

pub const BLOB_FEATURE_4K_ALIGNED: u32 = 0x1;

/// Blob metadata on disk format.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BlobMetaHeaderOndisk {
    /// Blob metadata magic number
    s_magic: u32,
    /// Blob metadata feature flags.
    s_features: u32,
    /// Compression algorithm for chunk information array.
    s_ci_compressor: u32,
    /// Number of entries in chunk information array.
    s_ci_entires: u32,
    /// Offset of compressed chunk information array in the compressed blob.
    s_ci_offset: u64,
    /// Size of compressed chunk information array
    s_ci_compressed_size: u64,
    /// Size of uncompressed chunk information array
    s_ci_uncompressed_size: u64,
    s_reserved: [u8; BLOB_METADATA_RESERVED_SIZE as usize],
    /// Second blob metadata magic number
    s_magic2: u32,
}

impl Default for BlobMetaHeaderOndisk {
    fn default() -> Self {
        BlobMetaHeaderOndisk {
            s_magic: BLOB_METADATA_MAGIC,
            s_features: 0,
            s_ci_compressor: compress::Algorithm::Lz4Block as u32,
            s_ci_entires: 0,
            s_ci_offset: 0,
            s_ci_compressed_size: 0,
            s_ci_uncompressed_size: 0,
            s_reserved: [0u8; BLOB_METADATA_RESERVED_SIZE as usize],
            s_magic2: BLOB_METADATA_MAGIC,
        }
    }
}

impl BlobMetaHeaderOndisk {
    /// Get compression algorithm for chunk information array.
    pub fn ci_compressor(&self) -> compress::Algorithm {
        if self.s_ci_compressor == compress::Algorithm::Lz4Block as u32 {
            compress::Algorithm::Lz4Block
        } else if self.s_ci_compressor == compress::Algorithm::GZip as u32 {
            compress::Algorithm::GZip
        } else if self.s_ci_compressor == compress::Algorithm::Zstd as u32 {
            compress::Algorithm::Zstd
        } else {
            compress::Algorithm::None
        }
    }

    pub fn set_ci_compressor(&mut self, algo: compress::Algorithm) {
        self.s_ci_compressor = algo as u32;
    }

    /// Get number of entries in chunk information array.
    pub fn ci_entries(&self) -> u32 {
        self.s_ci_entires
    }

    /// Set number of entries in chunk information array.
    pub fn set_ci_entries(&mut self, entries: u32) {
        self.s_ci_entires = entries;
    }

    /// Get offset of compressed chunk information array.
    pub fn ci_compressed_offset(&self) -> u64 {
        self.s_ci_offset
    }

    /// Set offset of compressed chunk information array.
    pub fn set_ci_compressed_offset(&mut self, offset: u64) {
        self.s_ci_offset = offset;
    }

    /// Get size of compressed chunk information array.
    pub fn ci_compressed_size(&self) -> u64 {
        self.s_ci_compressed_size
    }

    /// Set size of compressed chunk information array.
    pub fn set_ci_compressed_size(&mut self, size: u64) {
        self.s_ci_compressed_size = size;
    }

    /// Get size of uncompressed chunk information array.
    pub fn ci_uncompressed_size(&self) -> u64 {
        self.s_ci_uncompressed_size
    }

    /// Set size of uncompressed chunk information array.
    pub fn set_ci_uncompressed_size(&mut self, size: u64) {
        self.s_ci_uncompressed_size = size;
    }

    /// Check whether the uncompressed data chunk is 4k aligned.
    pub fn is_4k_aligned(&self) -> bool {
        self.s_features & BLOB_FEATURE_4K_ALIGNED != 0
    }

    /// Set whether the uncompressed data chunk is 4k aligned.
    pub fn set_4k_aligned(&mut self, aligned: bool) {
        if aligned {
            self.s_features |= BLOB_FEATURE_4K_ALIGNED;
        } else {
            self.s_features &= !BLOB_FEATURE_4K_ALIGNED;
        }
    }

    pub fn meta_flags(&self) -> u32 {
        self.s_features
    }

    /// Convert the header as an `&[u8]`.
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const BlobMetaHeaderOndisk as *const u8,
                std::mem::size_of::<BlobMetaHeaderOndisk>(),
            )
        }
    }
}

/// Blob chunk compression information on disk format.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BlobChunkInfoOndisk {
    // 20bits: size (low), 32bits: offset, 4bits: size (high), 8bits reserved
    uncomp_info: u64,
    // 20bits: size (low), 4bits: size (high), offset: 40bits
    comp_info: u64,
}

impl BlobChunkInfoOndisk {
    /// Get compressed offset of the chunk.
    #[inline]
    pub fn compressed_offset(&self) -> u64 {
        self.comp_info & BLOB_CHUNK_COMP_OFFSET_MASK
    }

    /// Set compressed offset of the chunk.
    #[inline]
    pub fn set_compressed_offset(&mut self, offset: u64) {
        debug_assert!(offset & !BLOB_CHUNK_COMP_OFFSET_MASK == 0);
        self.comp_info &= !BLOB_CHUNK_COMP_OFFSET_MASK;
        self.comp_info |= offset & BLOB_CHUNK_COMP_OFFSET_MASK;
    }

    /// Get compressed size of the chunk.
    #[inline]
    pub fn compressed_size(&self) -> u32 {
        let bit20 = self.comp_info >> BLOB_CHUNK_SIZE_LOW_SHIFT;
        let bit4 = (self.comp_info & 0xf0000000000) >> BLOB_CHUNK_SIZE_HIGH_COMP_SHIFT;
        (bit4 | bit20) as u32 + 1
    }

    /// Set compressed size of the chunk.
    #[inline]
    pub fn set_compressed_size(&mut self, size: u32) {
        let size = size as u64;
        debug_assert!(size > 0 && size <= BLOB_CHUNK_SIZE_MASK + 1);

        let size_low = ((size - 1) & BLOB_CHUNK_SIZE_LOW_MASK) << BLOB_CHUNK_SIZE_LOW_SHIFT;
        let size_high = ((size - 1) & BLOB_CHUNK_SIZE_HIGH_MASK) << BLOB_CHUNK_SIZE_HIGH_COMP_SHIFT;
        let offset = self.comp_info & BLOB_CHUNK_COMP_OFFSET_MASK;

        self.comp_info = size_low | size_high | offset;
    }

    /// Get compressed end of the chunk.
    #[inline]
    pub fn compressed_end(&self) -> u64 {
        self.compressed_offset() + self.compressed_size() as u64
    }

    /// Get uncompressed offset of the chunk.
    #[inline]
    pub fn uncompressed_offset(&self) -> u64 {
        self.uncomp_info & BLOB_CHUNK_UNCOMP_OFFSET_MASK
    }

    /// Set uncompressed offset of the chunk.
    #[inline]
    pub fn set_uncompressed_offset(&mut self, offset: u64) {
        debug_assert!(offset & !BLOB_CHUNK_UNCOMP_OFFSET_MASK == 0);
        self.uncomp_info &= !BLOB_CHUNK_UNCOMP_OFFSET_MASK;
        self.uncomp_info |= offset & BLOB_CHUNK_UNCOMP_OFFSET_MASK;
    }

    /// Get uncompressed end of the chunk.
    #[inline]
    pub fn uncompressed_size(&self) -> u32 {
        let size_high = (self.uncomp_info & 0xf00) << BLOB_CHUNK_SIZE_HIGH_UNCOMP_SHIFT;
        let size_low = self.uncomp_info >> BLOB_CHUNK_SIZE_LOW_SHIFT;
        (size_high | size_low) as u32 + 1
    }

    /// Set uncompressed end of the chunk.
    #[inline]
    pub fn set_uncompressed_size(&mut self, size: u32) {
        let size = size as u64;
        debug_assert!(size != 0 && size <= BLOB_CHUNK_SIZE_MASK + 1);

        let size_low = ((size - 1) & BLOB_CHUNK_SIZE_LOW_MASK) << BLOB_CHUNK_SIZE_LOW_SHIFT;
        let size_high =
            ((size - 1) & BLOB_CHUNK_SIZE_HIGH_MASK) >> BLOB_CHUNK_SIZE_HIGH_UNCOMP_SHIFT;
        let offset = self.uncomp_info & BLOB_CHUNK_UNCOMP_OFFSET_MASK;

        self.uncomp_info = size_low | offset | size_high;
    }

    /// Get uncompressed size of the chunk.
    #[inline]
    pub fn uncompressed_end(&self) -> u64 {
        self.uncompressed_offset() + self.uncompressed_size() as u64
    }

    /// Get 4k aligned uncompressed size of the chunk.
    #[inline]
    pub fn aligned_uncompressed_end(&self) -> u64 {
        round_up_4k(self.uncompressed_end())
    }

    /// Check whether the blob chunk is compressed or not.
    ///
    /// Assume the image builder guarantee that compress_size < uncompress_size if the chunk is
    /// compressed.
    #[inline]
    pub fn is_compressed(&self) -> bool {
        self.compressed_size() != self.uncompressed_size()
    }
}

/// Struct to maintain metadata information for a blob object.
///
/// Currently, the major responsibility of the `BlobMetaInfo` object is to query chunks covering
/// a specific uncompressed data range by
/// [BlobMetaInfo::get_chunks()](struct.BlobMetaInfo.html#method.get_chunks).
pub struct BlobMetaInfo {
    pub state: Arc<BlobMetaState>,
}

impl BlobMetaInfo {
    /// Create a new instance of `BlobMetaInfo`.
    ///
    /// The blob manager should create and maintain the consistence of the blob metadata file.
    /// Blob manager's clients, such as virtiofsd, may open the same blob metadata file to
    /// query chunks covering a specific uncompressed data range.
    ///
    /// When `reader` contains a valid value and the metadata is not ready yet, a new metadata file
    /// will be created.
    pub fn new(
        blob_path: &str,
        blob_info: &BlobInfo,
        reader: Option<&Arc<dyn BlobReader>>,
    ) -> Result<Self> {
        assert_eq!(
            size_of::<BlobMetaHeaderOndisk>() as u64,
            BLOB_METADATA_HEADER_SIZE
        );
        assert_eq!(size_of::<BlobChunkInfoOndisk>(), 16);
        let chunk_count = blob_info.chunk_count();
        if chunk_count == 0 || chunk_count > BLOB_METADATA_MAX_CHUNKS {
            return Err(einval!("chunk count should be greater than 0"));
        }

        let meta_path = format!("{}.{}", blob_path, FILE_SUFFIX);
        trace!(
            "meta_path {:?} info_size {} chunk_count {}",
            meta_path,
            blob_info.meta_ci_uncompressed_size(),
            chunk_count
        );
        let enable_write = reader.is_some();
        let file = OpenOptions::new()
            .read(true)
            .write(enable_write)
            .create(enable_write)
            .open(&meta_path)
            .map_err(|err| {
                einval!(format!(
                    "failed to open/create blob chunk_map file {:?}: {:?}",
                    meta_path, err
                ))
            })?;

        let info_size = blob_info.meta_ci_uncompressed_size() as usize;
        let aligned_info_size = round_up_4k(info_size);
        let expected_size = BLOB_METADATA_HEADER_SIZE as usize + aligned_info_size;
        if info_size != (chunk_count as usize) * (size_of::<BlobChunkInfoOndisk>())
            || (aligned_info_size as u64) > BLOB_METADATA_MAX_SIZE
        {
            return Err(einval!("blob metadata size is too big!"));
        }

        let file_size = file.metadata()?.len();
        if file_size == 0 && enable_write {
            file.set_len(expected_size as u64)?;
        }

        let fd = file.as_raw_fd();
        let base = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                expected_size as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if base == libc::MAP_FAILED {
            return Err(last_error!("failed to mmap blob chunk_map"));
        } else if base.is_null() {
            return Err(ebadf!("failed to mmap blob chunk_map"));
        }

        let header = unsafe { (base as *mut u8).add(aligned_info_size as usize) };
        let header = unsafe { &mut *(header as *mut BlobMetaHeaderOndisk) };
        if u32::from_le(header.s_magic) != BLOB_METADATA_MAGIC
            || u32::from_le(header.s_magic2) != BLOB_METADATA_MAGIC
            || u32::from_le(header.s_features) != blob_info.meta_flags()
            || u64::from_le(header.s_ci_offset) != blob_info.meta_ci_offset()
            || u64::from_le(header.s_ci_compressed_size) != blob_info.meta_ci_compressed_size()
            || u64::from_le(header.s_ci_uncompressed_size) != blob_info.meta_ci_uncompressed_size()
        {
            if !enable_write {
                return Err(enoent!("blob metadata file is not ready"));
            }

            let buffer = unsafe { std::slice::from_raw_parts_mut(base as *mut u8, expected_size) };
            buffer[info_size..].fill(0);
            Self::read_metadata(
                blob_info,
                reader.as_ref().unwrap(),
                &mut buffer[..info_size],
            )?;
            header.s_features = u32::to_le(blob_info.meta_flags());
            header.s_ci_offset = u64::to_le(blob_info.meta_ci_offset());
            header.s_ci_compressed_size = u64::to_le(blob_info.meta_ci_compressed_size());
            header.s_ci_uncompressed_size = u64::to_le(blob_info.meta_ci_uncompressed_size());

            file.sync_data()?;
            header.s_magic = u32::to_le(BLOB_METADATA_MAGIC);
            header.s_magic2 = u32::to_le(BLOB_METADATA_MAGIC);
        }

        let chunk_infos = unsafe {
            ManuallyDrop::new(Vec::from_raw_parts(
                base as *mut BlobChunkInfoOndisk,
                chunk_count as usize,
                chunk_count as usize,
            ))
        };

        let state = Arc::new(BlobMetaState {
            blob_index: blob_info.blob_index(),
            compressed_size: blob_info.compressed_size(),
            uncompressed_size: round_up_4k(blob_info.uncompressed_size()),
            chunk_count,
            chunks: chunk_infos,
            base: base as *const u8,
            unmap_len: expected_size,
            is_stargz: blob_info.is_stargz(),
        });

        Ok(BlobMetaInfo { state })
    }

    /// Get blob chunks covering uncompressed data range [start, start + size).
    ///
    /// `size` also includes chunks alignment. It is a range on blob with chunks and alignments between them.
    /// The method returns error if any of following condition is true:
    /// - range [start, start + size) is invalid.
    /// - `start` is bigger than blob size.
    /// - some portion of the range [start, start + size) is not covered by chunks.
    /// - the blob metadata is invalid.
    pub fn get_chunks_uncompressed(
        &self,
        start: u64,
        size: u64,
        batch_size: u64,
    ) -> Result<Vec<Arc<dyn BlobChunkInfo>>> {
        let end = start.checked_add(size).ok_or_else(|| einval!())?;
        if end > self.state.uncompressed_size {
            return Err(einval!(format!(
                "get_chunks_uncompressed: end {} uncompressed_size {}",
                end, self.state.uncompressed_size
            )));
        }
        let batch_end = if batch_size <= size {
            end
        } else {
            std::cmp::min(
                start.checked_add(batch_size).unwrap_or(end),
                self.state.uncompressed_size,
            )
        };

        let infos = &*self.state.chunks;
        let mut index = self.state.get_chunk_index_nocheck(start, false)?;
        debug_assert!(index < infos.len());
        let entry = &infos[index];
        self.validate_chunk(entry)?;
        debug_assert!(entry.uncompressed_offset() <= start);
        debug_assert!(entry.uncompressed_end() > start);
        trace!(
            "get_chunks_uncompressed: entry {} {}",
            entry.uncompressed_offset(),
            entry.uncompressed_end()
        );

        let mut vec = Vec::with_capacity(512);
        vec.push(BlobMetaChunk::new(index, &self.state));

        let mut last_end = entry.aligned_uncompressed_end();
        if last_end >= batch_end {
            Ok(vec)
        } else {
            while index + 1 < infos.len() {
                index += 1;
                let entry = &infos[index];
                self.validate_chunk(entry)?;

                // For stargz chunks, disable this check.
                if !self.state.is_stargz && entry.uncompressed_offset() != last_end {
                    return Err(einval!(format!(
                        "mismatch uncompressed {} size {} last_end {}",
                        entry.uncompressed_offset(),
                        entry.uncompressed_size(),
                        last_end
                    )));
                }

                // Avoid read amplify if next chunk is too big.
                if last_end >= end && entry.aligned_uncompressed_end() > batch_end {
                    return Ok(vec);
                }

                vec.push(BlobMetaChunk::new(index, &self.state));
                last_end = entry.aligned_uncompressed_end();
                if last_end >= batch_end {
                    return Ok(vec);
                }
            }

            Err(einval!(format!(
                "entry not found index {} infos.len {}",
                index,
                infos.len(),
            )))
        }
    }

    /// Get blob chunks covering compressed data range [start, start + size).
    ///
    /// The method returns error if any of following condition is true:
    /// - range [start, start + size) is invalid.
    /// - `start` is bigger than blob size.
    /// - some portion of the range [start, start + size) is not covered by chunks.
    /// - the blob metadata is invalid.
    pub fn get_chunks_compressed(
        &self,
        start: u64,
        size: u64,
        batch_size: u64,
    ) -> Result<Vec<Arc<dyn BlobChunkInfo>>> {
        let end = start.checked_add(size).ok_or_else(|| einval!())?;
        if end > self.state.compressed_size {
            return Err(einval!(format!(
                "get_chunks_compressed: end {} compressed_size {}",
                end, self.state.compressed_size
            )));
        }
        let batch_end = if batch_size <= size {
            end
        } else {
            std::cmp::min(
                start.checked_add(batch_size).unwrap_or(end),
                self.state.compressed_size,
            )
        };

        let infos = &*self.state.chunks;
        let mut index = self.state.get_chunk_index_nocheck(start, true)?;
        debug_assert!(index < infos.len());
        let entry = &infos[index];
        self.validate_chunk(entry)?;

        let mut vec = Vec::with_capacity(512);
        vec.push(BlobMetaChunk::new(index, &self.state));

        let mut last_end = entry.compressed_end();
        if last_end >= batch_end {
            Ok(vec)
        } else {
            while index + 1 < infos.len() {
                index += 1;
                let entry = &infos[index];
                self.validate_chunk(entry)?;
                if entry.compressed_offset() != last_end {
                    return Err(einval!());
                }

                // Avoid read amplify if next chunk is too big.
                if last_end >= end && entry.compressed_end() > batch_end {
                    return Ok(vec);
                }

                vec.push(BlobMetaChunk::new(index, &self.state));
                last_end = entry.compressed_end();
                if last_end >= batch_end {
                    return Ok(vec);
                }
            }

            Err(einval!())
        }
    }

    /// Try to amplify the request by appending more continuous chunks.
    pub fn add_more_chunks(
        &self,
        chunks: &[Arc<dyn BlobChunkInfo>],
        max_size: u64,
    ) -> Option<Vec<Arc<dyn BlobChunkInfo>>> {
        let infos = &*self.state.chunks;
        let mut index = chunks[chunks.len() - 1].id() as usize;
        debug_assert!(index < infos.len());
        let entry = &infos[index];
        if self.validate_chunk(entry).is_err() {
            return None;
        }
        let end = entry.compressed_end();
        if end > self.state.compressed_size {
            return None;
        }
        let batch_end = std::cmp::min(
            end.checked_add(max_size).unwrap_or(end),
            self.state.compressed_size,
        );
        if batch_end <= end {
            return None;
        }

        let mut last_end = end;
        let mut vec = chunks.to_vec();
        while index + 1 < infos.len() {
            index += 1;
            let entry = &infos[index];
            if self.validate_chunk(entry).is_err() || entry.compressed_offset() != last_end {
                break;
            }

            // Avoid read amplification if next chunk is too big.
            if entry.compressed_end() > batch_end {
                break;
            }

            vec.push(BlobMetaChunk::new(index, &self.state));
            last_end = entry.compressed_end();
            if last_end >= batch_end {
                break;
            }
        }

        trace!("try to extend request with {} more bytes", last_end - end);

        Some(vec)
    }

    #[inline]
    fn validate_chunk(&self, entry: &BlobChunkInfoOndisk) -> Result<()> {
        // For stargz blob, self.state.compressed_size == 0, so don't validate it.
        if (!self.state.is_stargz && entry.compressed_end() > self.state.compressed_size)
            || entry.uncompressed_end() > self.state.uncompressed_size
        {
            Err(einval!(format!(
                "invalid chunk, blob_index {} compressed_end {} compressed_size {} uncompressed_end {} uncompressed_size {}",
                self.state.blob_index,
                entry.compressed_end(),
                self.state.compressed_size,
                entry.uncompressed_end(),
                self.state.uncompressed_size,
            )))
        } else {
            Ok(())
        }
    }

    fn read_metadata(
        blob_info: &BlobInfo,
        reader: &Arc<dyn BlobReader>,
        buffer: &mut [u8],
    ) -> Result<()> {
        trace!(
            "blob_info compressor {} ci_compressor {} ci_compressed_size {} ci_uncompressed_size {}",
            blob_info.compressor(),
            blob_info.meta_ci_compressor(),
            blob_info.meta_ci_compressed_size(),
            blob_info.meta_ci_uncompressed_size(),
        );

        if blob_info.meta_ci_compressor() == compress::Algorithm::None {
            let size = reader
                .read(buffer, blob_info.meta_ci_offset())
                .map_err(|e| {
                    eio!(format!(
                        "failed to read metadata from backend(compressor is none), {:?}",
                        e
                    ))
                })?;
            if size as u64 != blob_info.meta_ci_uncompressed_size() {
                return Err(eio!(
                    "failed to read blob metadata from backend(compressor is None)"
                ));
            }
        } else {
            let compressed_size = blob_info.meta_ci_compressed_size();
            let mut buf = alloc_buf(compressed_size as usize);
            let size = reader
                .read(&mut buf, blob_info.meta_ci_offset())
                .map_err(|e| eio!(format!("failed to read metadata from backend, {:?}", e)))?;
            if size as u64 != compressed_size {
                return Err(eio!("failed to read blob metadata from backend"));
            }

            // Lz4 does not support concurrent decompression of the same data into
            // the same piece of memory. There will be multiple containers mmap the
            // same file, causing the buffer to be shared between different
            // processes. This will cause data errors due to race issues when
            // decompressing with lz4. We solve this problem by creating a temporary
            // memory to hold the decompressed data.
            //
            // Because this process will only be executed when the blob.meta file is
            // created for the first time, which means that a machine will only
            // execute the process once when the blob.meta is created for the first
            // time, the memory consumption and performance impact are relatively
            // small.
            let mut uncom_buf = vec![0u8; buffer.len()];
            compress::decompress(&buf, None, &mut uncom_buf, blob_info.meta_ci_compressor())
                .map_err(|e| {
                    error!("failed to decompress metadata: {}", e);
                    e
                })?;
            buffer.copy_from_slice(&uncom_buf);
        }

        // TODO: validate metadata

        Ok(())
    }
}

pub struct BlobMetaState {
    blob_index: u32,
    // The file size of blob file when it contains compressed chunks.
    compressed_size: u64,
    // The file size of blob file when it contains raw(uncompressed)
    // chunks, it usually refers to a blob file in cache(e.g. filecache).
    uncompressed_size: u64,
    chunk_count: u32,
    chunks: ManuallyDrop<Vec<BlobChunkInfoOndisk>>,
    base: *const u8,
    unmap_len: usize,
    /// The blob meta is for an stargz image.
    is_stargz: bool,
}

// // Safe to Send/Sync because the underlying data structures are readonly
unsafe impl Send for BlobMetaState {}
unsafe impl Sync for BlobMetaState {}

impl Drop for BlobMetaState {
    fn drop(&mut self) {
        if !self.base.is_null() {
            let size = self.unmap_len;
            unsafe { libc::munmap(self.base as *mut u8 as *mut libc::c_void, size) };
            self.base = std::ptr::null();
        }
    }
}

impl BlobMetaState {
    fn get_chunk_index_nocheck(&self, addr: u64, compressed: bool) -> Result<usize> {
        let chunks = &self.chunks;
        let mut size = self.chunk_count as usize;
        let mut left = 0;
        let mut right = size;
        let mut start = 0;
        let mut end = 0;

        while left < right {
            let mid = left + size / 2;
            // SAFETY: the call is made safe by the following invariants:
            // - `mid >= 0`
            // - `mid < size`: `mid` is limited by `[left; right)` bound.
            let entry = unsafe { chunks.get_unchecked(mid) };
            if compressed {
                start = entry.compressed_offset();
                end = entry.compressed_end();
            } else {
                start = entry.uncompressed_offset();
                end = entry.uncompressed_end();
            };

            if start > addr {
                right = mid;
            } else if end <= addr {
                left = mid + 1;
            } else {
                return Ok(mid);
            }

            size = right - left;
        }

        // if addr == self.chunks[last].compressed_offset, return einval
        // with error msg.
        Err(einval!(format!(
            "start: {}, end: {}, addr: {}",
            start, end, addr
        )))
    }
}

/// A fake `BlobChunkInfo` object created from blob metadata.
///
/// It represents a chunk within memory mapped chunk maps, which
/// means it is only used with blobs with chunk meta accommodated.
/// So for rafs v5, we should avoid using it on IO path.
pub struct BlobMetaChunk {
    chunk_index: usize,
    meta: Arc<BlobMetaState>,
}

impl BlobMetaChunk {
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new(chunk_index: usize, meta: &Arc<BlobMetaState>) -> Arc<dyn BlobChunkInfo> {
        debug_assert!(chunk_index <= u32::MAX as usize);
        Arc::new(BlobMetaChunk {
            chunk_index,
            meta: meta.clone(),
        }) as Arc<dyn BlobChunkInfo>
    }
}

impl BlobChunkInfo for BlobMetaChunk {
    fn chunk_id(&self) -> &RafsDigest {
        panic!("BlobMetaChunk doesn't support `chunk_id()`");
    }

    fn id(&self) -> u32 {
        self.chunk_index as u32
    }

    fn blob_index(&self) -> u32 {
        self.meta.blob_index
    }

    fn compressed_offset(&self) -> u64 {
        self.meta.chunks[self.chunk_index].compressed_offset()
    }

    fn compressed_size(&self) -> u32 {
        self.meta.chunks[self.chunk_index].compressed_size()
    }

    fn uncompressed_offset(&self) -> u64 {
        self.meta.chunks[self.chunk_index].uncompressed_offset()
    }

    fn uncompressed_size(&self) -> u32 {
        self.meta.chunks[self.chunk_index].uncompressed_size()
    }

    fn is_compressed(&self) -> bool {
        self.meta.chunks[self.chunk_index].is_compressed()
    }

    fn is_hole(&self) -> bool {
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn round_up_4k<T: Add<Output = T> + BitAnd<Output = T> + Not<Output = T> + From<u16>>(val: T) -> T {
    (val + T::from(0xfff)) & !T::from(0xfff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{BackendResult, BlobReader};
    use crate::device::BlobFeatures;
    use crate::RAFS_MAX_CHUNK_SIZE;
    use nix::sys::uio;
    use nydus_utils::metrics::BackendMetrics;
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use vmm_sys_util::tempfile::TempFile;

    #[test]
    fn test_get_chunk_index_with_hole() {
        let state = BlobMetaState {
            blob_index: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            chunk_count: 2,
            chunks: ManuallyDrop::new(vec![
                BlobChunkInfoOndisk {
                    uncomp_info: 0x01ff_f000_0000_0000,
                    comp_info: 0x00ff_f000_0000_0000,
                },
                BlobChunkInfoOndisk {
                    uncomp_info: 0x01ff_f000_0010_0000,
                    comp_info: 0x00ff_f000_0010_0000,
                },
            ]),
            base: std::ptr::null(),
            unmap_len: 0,
            is_stargz: false,
        };

        assert_eq!(state.get_chunk_index_nocheck(0, false).unwrap(), 0);
        assert_eq!(state.get_chunk_index_nocheck(0x1fff, false).unwrap(), 0);
        assert_eq!(state.get_chunk_index_nocheck(0x100000, false).unwrap(), 1);
        assert_eq!(state.get_chunk_index_nocheck(0x101fff, false).unwrap(), 1);
        state.get_chunk_index_nocheck(0x2000, false).unwrap_err();
        state.get_chunk_index_nocheck(0xfffff, false).unwrap_err();
        state.get_chunk_index_nocheck(0x102000, false).unwrap_err();
    }

    #[test]
    fn test_new_chunk_on_disk() {
        let mut chunk = BlobChunkInfoOndisk::default();

        assert_eq!(chunk.compressed_offset(), 0);
        assert_eq!(chunk.compressed_size(), 1);
        assert_eq!(chunk.compressed_end(), 1);
        assert_eq!(chunk.uncompressed_offset(), 0);
        assert_eq!(chunk.uncompressed_size(), 1);
        assert_eq!(chunk.aligned_uncompressed_end(), 0x1000);

        chunk.set_compressed_offset(0x1000);
        chunk.set_compressed_size(0x100);
        assert_eq!(chunk.compressed_offset(), 0x1000);
        assert_eq!(chunk.compressed_size(), 0x100);

        chunk.set_uncompressed_offset(0x1000);
        chunk.set_uncompressed_size(0x100);
        assert_eq!(chunk.uncompressed_offset(), 0x1000);
        assert_eq!(chunk.uncompressed_size(), 0x100);

        chunk.set_compressed_offset(0xffffffffff);
        chunk.set_compressed_size(0x1000000);
        assert_eq!(chunk.compressed_offset(), 0xffffffffff);
        assert_eq!(chunk.compressed_size(), 0x1000000);

        chunk.set_uncompressed_offset(0xffffffff000);
        chunk.set_uncompressed_size(0x1000000);
        assert_eq!(chunk.uncompressed_offset(), 0xffffffff000);
        assert_eq!(chunk.uncompressed_size(), 0x1000000);

        // For testing old format compatibility.
        let chunk = BlobChunkInfoOndisk {
            uncomp_info: 0xffff_ffff_f100_0000,
            comp_info: 0xffff_f0ff_ffff_ffff,
        };
        assert_eq!(chunk.uncompressed_size(), 0x000f_ffff + 1);
        assert_eq!(chunk.uncompressed_offset(), 0xffff_1000 * 0x1000);
        assert_eq!(chunk.compressed_size(), 0x000f_ffff + 1);
        assert_eq!(chunk.compressed_offset(), 0x00ff_ffff_ffff);
    }

    #[test]
    fn test_get_chunks() {
        let state = BlobMetaState {
            blob_index: 1,
            compressed_size: 0x6001,
            uncompressed_size: 0x102001,
            chunk_count: 5,
            chunks: ManuallyDrop::new(vec![
                BlobChunkInfoOndisk {
                    uncomp_info: 0x0100_0000_0000_0000,
                    comp_info: 0x00ff_f000_0000_0000,
                },
                BlobChunkInfoOndisk {
                    uncomp_info: 0x01ff_f000_0000_2000,
                    comp_info: 0x01ff_f000_0000_1000,
                },
                BlobChunkInfoOndisk {
                    uncomp_info: 0x01ff_f000_0000_4000,
                    comp_info: 0x00ff_f000_0000_3000,
                },
                BlobChunkInfoOndisk {
                    uncomp_info: 0x01ff_f000_0010_0000,
                    comp_info: 0x00ff_f000_0000_4000,
                },
                BlobChunkInfoOndisk {
                    uncomp_info: 0x01ff_f000_0010_2000,
                    comp_info: 0x00ff_f000_0000_5000,
                },
            ]),
            base: std::ptr::null(),
            unmap_len: 0,
            is_stargz: false,
        };
        let info = BlobMetaInfo {
            state: Arc::new(state),
        };

        let vec = info.get_chunks_uncompressed(0x0, 0x1001, 0).unwrap();
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0].blob_index(), 1);
        assert_eq!(vec[0].id(), 0);
        assert_eq!(vec[0].compressed_offset(), 0);
        assert_eq!(vec[0].compressed_size(), 0x1000);
        assert_eq!(vec[0].uncompressed_offset(), 0);
        assert_eq!(vec[0].uncompressed_size(), 0x1001);
        assert!(vec[0].is_compressed());
        assert!(!vec[0].is_hole());

        let vec = info.get_chunks_uncompressed(0x0, 0x4000, 0).unwrap();
        assert_eq!(vec.len(), 2);
        assert_eq!(vec[1].blob_index(), 1);
        assert_eq!(vec[1].id(), 1);
        assert_eq!(vec[1].compressed_offset(), 0x1000);
        assert_eq!(vec[1].compressed_size(), 0x2000);
        assert_eq!(vec[1].uncompressed_offset(), 0x2000);
        assert_eq!(vec[1].uncompressed_size(), 0x2000);
        assert!(!vec[1].is_compressed());
        assert!(!vec[1].is_hole());

        let vec = info.get_chunks_uncompressed(0x0, 0x4001, 0).unwrap();
        assert_eq!(vec.len(), 3);

        let vec = info.get_chunks_uncompressed(0x100000, 0x2000, 0).unwrap();
        assert_eq!(vec.len(), 1);

        assert!(info.get_chunks_uncompressed(0x0, 0x6001, 0).is_err());
        assert!(info.get_chunks_uncompressed(0x0, 0xfffff, 0).is_err());
        assert!(info.get_chunks_uncompressed(0x0, 0x100000, 0).is_err());
        assert!(info.get_chunks_uncompressed(0x0, 0x104000, 0).is_err());
        assert!(info.get_chunks_uncompressed(0x0, 0x104001, 0).is_err());
        assert!(info.get_chunks_uncompressed(0x100000, 0x2001, 0).is_err());
        assert!(info.get_chunks_uncompressed(0x100000, 0x4000, 0).is_err());
        assert!(info.get_chunks_uncompressed(0x100000, 0x4001, 0).is_err());
        assert!(info
            .get_chunks_uncompressed(0x102000, 0xffff_ffff_ffff_ffff, 0)
            .is_err());
        assert!(info.get_chunks_uncompressed(0x104000, 0x1, 0).is_err());
    }

    #[test]
    fn test_round_up_4k() {
        assert_eq!(round_up_4k(0), 0x0u32);
        assert_eq!(round_up_4k(1), 0x1000u32);
        assert_eq!(round_up_4k(0xfff), 0x1000u32);
        assert_eq!(round_up_4k(0x1000), 0x1000u32);
        assert_eq!(round_up_4k(0x1001), 0x2000u32);
        assert_eq!(round_up_4k(0x1fff), 0x2000u64);
    }

    struct DummyBlobReader {
        pub metrics: Arc<BackendMetrics>,
        file: File,
    }

    impl BlobReader for DummyBlobReader {
        fn blob_size(&self) -> BackendResult<u64> {
            Ok(0)
        }

        fn try_read(&self, buf: &mut [u8], offset: u64) -> BackendResult<usize> {
            let ret = uio::pread(self.file.as_raw_fd(), buf, offset as i64).unwrap();
            Ok(ret)
        }

        fn metrics(&self) -> &BackendMetrics {
            &self.metrics
        }
    }

    #[test]
    fn test_read_metadata_compressor_none() {
        let temp = TempFile::new().unwrap();
        let mut w = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.as_path())
            .unwrap();
        let r = OpenOptions::new()
            .read(true)
            .write(false)
            .open(temp.as_path())
            .unwrap();

        let chunks = vec![
            BlobChunkInfoOndisk {
                uncomp_info: 0x01ff_f000_0000_0000,
                comp_info: 0x00ff_f000_0000_0000,
            },
            BlobChunkInfoOndisk {
                uncomp_info: 0x01ff_f000_0010_0000,
                comp_info: 0x00ff_f000_0010_0000,
            },
        ];

        let data = unsafe {
            std::slice::from_raw_parts(
                chunks.as_ptr() as *const u8,
                chunks.len() * std::mem::size_of::<BlobChunkInfoOndisk>(),
            )
        };

        let pos = 0;
        w.write_all(data).unwrap();

        let mut blob_info = BlobInfo::new(
            0,
            "dummy".to_string(),
            0,
            0,
            RAFS_MAX_CHUNK_SIZE as u32,
            0,
            BlobFeatures::default(),
        );
        blob_info.set_blob_meta_info(
            0,
            pos,
            data.len() as u64,
            data.len() as u64,
            compress::Algorithm::None as u32,
        );

        let mut buffer = alloc_buf(data.len());
        let reader: Arc<dyn BlobReader> = Arc::new(DummyBlobReader {
            metrics: BackendMetrics::new("dummy", "localfs"),
            file: r,
        });
        BlobMetaInfo::read_metadata(&blob_info, &reader, &mut buffer).unwrap();

        assert_eq!(buffer, data);
    }

    #[test]
    fn test_read_metadata_compressor_lz4() {
        let temp = TempFile::new().unwrap();
        let mut w = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.as_path())
            .unwrap();
        let r = OpenOptions::new()
            .read(true)
            .write(false)
            .open(temp.as_path())
            .unwrap();

        let chunks = vec![
            BlobChunkInfoOndisk {
                uncomp_info: 0x01ff_f000_0000_0000,
                comp_info: 0x00ff_f000_0000_0000,
            },
            BlobChunkInfoOndisk {
                uncomp_info: 0x01ff_f000_0010_0000,
                comp_info: 0x00ff_f000_0010_0000,
            },
        ];

        let data = unsafe {
            std::slice::from_raw_parts(
                chunks.as_ptr() as *const u8,
                chunks.len() * std::mem::size_of::<BlobChunkInfoOndisk>(),
            )
        };

        let (buf, compressed) = compress::compress(data, compress::Algorithm::Lz4Block).unwrap();
        assert!(compressed);

        let pos = 0;
        w.write_all(&buf).unwrap();

        let compressed_size = buf.len();
        let uncompressed_size = data.len();
        let mut blob_info = BlobInfo::new(
            0,
            "dummy".to_string(),
            0,
            0,
            RAFS_MAX_CHUNK_SIZE as u32,
            0,
            BlobFeatures::default(),
        );
        blob_info.set_blob_meta_info(
            0,
            pos,
            compressed_size as u64,
            uncompressed_size as u64,
            compress::Algorithm::Lz4Block as u32,
        );

        let mut buffer = alloc_buf(uncompressed_size);
        let reader: Arc<dyn BlobReader> = Arc::new(DummyBlobReader {
            metrics: BackendMetrics::new("dummy", "localfs"),
            file: r,
        });
        BlobMetaInfo::read_metadata(&blob_info, &reader, &mut buffer).unwrap();

        assert_eq!(buffer, data);
    }
}
