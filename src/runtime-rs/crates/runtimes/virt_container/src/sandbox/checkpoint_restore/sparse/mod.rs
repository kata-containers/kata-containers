// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::fs;

use anyhow::Result;

mod canonical;

pub(super) use canonical::{decode_sparse_artifact, encode_sparse_artifact};

pub(super) const SPARSE_FORMAT: &str = "kata-sparse-raw-v1";
pub(super) const SPARSE_CHUNK_SIZE: usize = 1024 * 1024;
const SPARSE_MAGIC: &[u8; 8] = b"KTSPRAW1";
const SPARSE_HEADER_SIZE: usize = 32;
const SPARSE_RECORD_SIZE: usize = 16;

pub(super) struct SparseArtifact {
    pub(super) logical_size: u64,
    pub(super) encoded_size: u64,
    pub(super) non_zero_chunks: u64,
    pub(super) stats: SparseEncodeStats,
}

pub(super) struct SparseEncodeStats {
    pub(super) candidate_chunks: u64,
    pub(super) candidate_bytes: u64,
    pub(super) written_bytes: u64,
}

pub(super) struct SparseDecodeStats {
    pub(super) records: u64,
    pub(super) written_bytes: u64,
}

pub(super) struct SparseMeta {
    pub(super) encoded_size: u64,
    pub(super) logical_size: u64,
}

fn header(logical_size: u64) -> [u8; SPARSE_HEADER_SIZE] {
    let mut value = [0_u8; SPARSE_HEADER_SIZE];
    value[..8].copy_from_slice(SPARSE_MAGIC);
    value[8..12].copy_from_slice(&(SPARSE_HEADER_SIZE as u32).to_le_bytes());
    value[12..16].copy_from_slice(&(SPARSE_CHUNK_SIZE as u32).to_le_bytes());
    value[16..24].copy_from_slice(&logical_size.to_le_bytes());

    value
}

use kata_sys_util::fs::{read_exact_at as read_chunk, write_all_at};

fn data_chunks(source: &fs::File, logical_size: u64) -> Result<Option<Vec<u64>>> {
    kata_sys_util::fs::sparse_file_chunks(source, logical_size, SPARSE_CHUNK_SIZE)
}
