// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

//! Native memory-file codecs, independent of a running VM and Pod storage policy.

use anyhow::{anyhow, Result};
use kata_sys_util::fs::{read_exact_at as read_chunk, write_all_at};
use ring::digest::Context as Sha256Context;
use std::fs;

pub use dbs_snapshot::MemoryRange;

mod flat;
mod overlay;

pub use flat::{copy_flat_sparse_file, merge_flat_sparse_file, verify_flat_sparse_file};
pub use overlay::{encode_memory_overlay, memory_overlay_size, OverlayReader, OVERLAY_FORMAT};

/// Errors that callers must distinguish from native file I/O failures.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    /// Artifact bytes or metadata failed validation.
    #[error("{0}")]
    Invalid(String),
}

/// Identifier for flat sparse memory and its canonical digest.
pub const FLAT_SPARSE_FORMAT: &str = "dragonball-flat-sparse-v1";

const SPARSE_CHUNK_SIZE: usize = 1024 * 1024;
const SPARSE_MAGIC: &[u8; 8] = b"KTSPRAW1";
const SPARSE_HEADER_SIZE: usize = 32;
const SPARSE_RECORD_SIZE: usize = 16;

/// Native memory artifact metadata computed during encoding or verification.
pub struct SparseArtifact {
    /// Size of the complete Guest memory represented by this artifact.
    pub logical_size: u64,
    /// Length of the encoded file.
    pub encoded_size: u64,
    /// Format-specific SHA-256 digest.
    pub sha256: String,
    /// Number of nonzero flat chunks or encoded overlay ranges.
    pub non_zero_chunks: u64,
    /// I/O work performed by the codec.
    pub stats: SparseEncodeStats,
}

/// Counters collected without rescanning the memory artifact.
pub struct SparseEncodeStats {
    /// Number of chunks or ranges considered.
    pub candidate_chunks: u64,
    /// Number of payload bytes inspected.
    pub candidate_bytes: u64,
    /// Number of bytes written to the output.
    pub written_bytes: u64,
}

fn validate_memory_ranges(
    ranges: &[MemoryRange],
    logical_size: u64,
    operation: &str,
) -> Result<()> {
    let page_size = host_page_size()? as u64;
    let mut previous_end = 0;
    for range in ranges {
        let end = range
            .file_offset
            .checked_add(range.length)
            .ok_or_else(|| anyhow!("{operation} range overflows"))?;
        if range.length == 0
            || range.file_offset % page_size != 0
            || range.length % page_size != 0
            || range.file_offset < previous_end
            || end > logical_size
        {
            return Err(anyhow!("invalid {operation} range"));
        }
        previous_end = end;
    }
    Ok(())
}

fn digest_hex(hasher: Sha256Context) -> String {
    hasher
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn header(logical_size: u64) -> [u8; SPARSE_HEADER_SIZE] {
    let mut value = [0_u8; SPARSE_HEADER_SIZE];
    value[..8].copy_from_slice(SPARSE_MAGIC);
    value[8..12].copy_from_slice(&(SPARSE_HEADER_SIZE as u32).to_le_bytes());
    value[12..16].copy_from_slice(&(SPARSE_CHUNK_SIZE as u32).to_le_bytes());
    value[16..24].copy_from_slice(&logical_size.to_le_bytes());

    value
}

fn host_page_size() -> Result<usize> {
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page_size <= 0 {
        return Err(anyhow!("cannot determine host page size"));
    }

    Ok(page_size as usize)
}

fn data_chunks(source: &fs::File, logical_size: u64) -> Result<Option<Vec<u64>>> {
    kata_sys_util::fs::sparse_file_chunks(source, logical_size, SPARSE_CHUNK_SIZE)
}
