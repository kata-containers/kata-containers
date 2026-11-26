// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use super::CodecError;
use super::MemoryRange;
use super::{
    data_chunks, digest_hex, header, read_chunk, validate_memory_ranges, write_all_at,
    SparseArtifact, SparseEncodeStats, SPARSE_CHUNK_SIZE, SPARSE_RECORD_SIZE,
};
use anyhow::{anyhow, Context, Result};
use ring::digest::{Context as Sha256Context, SHA256};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    os::fd::AsRawFd,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

// Queue completed checkpoint writes in batches so the final sync can overlap
// device writeback with hashing the remaining guest-memory chunks.
const FLAT_SPARSE_WRITEBACK_BATCH: u64 = 16 * 1024 * 1024;

/// Copy a sparse flat image without changing its logical layout and compute
/// the canonical sparse digest used by the checkpoint manifest.
fn process_flat_sparse_file(
    source: &mut fs::File,
    destination: Option<&Path>,
) -> Result<SparseArtifact> {
    let metadata = source.metadata()?;
    let logical_size = metadata.len();
    if !metadata.is_file() || logical_size == 0 || logical_size > libc::off_t::MAX as u64 {
        return Err(anyhow!(
            "flat sparse source is not a supported regular file"
        ));
    }

    let output = destination
        .map(|path| {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o400)
                .custom_flags(libc::O_NOFOLLOW)
                .open(path)
        })
        .transpose()?;
    if let Some(output) = &output {
        output.set_len(logical_size)?;
    }

    let mut hasher = Sha256Context::new(&SHA256);
    hasher.update(&header(logical_size));

    let chunk_count = logical_size.div_ceil(SPARSE_CHUNK_SIZE as u64);
    let chunks =
        data_chunks(source, logical_size)?.unwrap_or_else(|| (0..chunk_count).collect::<Vec<_>>());
    let candidate_chunks = chunks.len() as u64;
    let mut buffer = vec![0_u8; SPARSE_CHUNK_SIZE];
    let mut non_zero_chunks = 0_u64;
    let mut candidate_bytes = 0_u64;
    let mut written_bytes = 0_u64;
    let mut next_writeback = FLAT_SPARSE_WRITEBACK_BATCH;
    for chunk in chunks {
        let offset = chunk
            .checked_mul(SPARSE_CHUNK_SIZE as u64)
            .ok_or_else(|| anyhow!("flat sparse chunk offset overflow"))?;
        let length = (logical_size - offset).min(SPARSE_CHUNK_SIZE as u64) as usize;

        read_chunk(source, offset, &mut buffer[..length])?;

        candidate_bytes = candidate_bytes
            .checked_add(length as u64)
            .ok_or_else(|| anyhow!("flat sparse candidate size overflow"))?;

        let all_zero = buffer[..length].iter().all(|byte| *byte == 0);
        if all_zero {
            continue;
        }

        let mut record = [0_u8; SPARSE_RECORD_SIZE];
        record[..8].copy_from_slice(&offset.to_le_bytes());
        record[8..12].copy_from_slice(&(length as u32).to_le_bytes());
        hasher.update(&record);
        hasher.update(&buffer[..length]);

        if let Some(output) = &output {
            write_all_at(output, &buffer[..length], offset)?;
            written_bytes = written_bytes
                .checked_add(length as u64)
                .ok_or_else(|| anyhow!("flat sparse written size overflow"))?;

            if written_bytes >= next_writeback {
                start_writeback(output)?;
                next_writeback = written_bytes
                    .checked_add(FLAT_SPARSE_WRITEBACK_BATCH)
                    .ok_or_else(|| anyhow!("flat sparse writeback threshold overflow"))?;
            }
        }

        non_zero_chunks += 1;
    }

    if let Some(output) = &output {
        output.sync_all()?;
    }

    Ok(SparseArtifact {
        logical_size,
        encoded_size: logical_size,
        sha256: digest_hex(hasher),
        non_zero_chunks,
        stats: SparseEncodeStats {
            candidate_chunks,
            candidate_bytes,
            written_bytes,
        },
    })
}

/// Copy and synchronize a flat sparse image, computing its canonical digest.
pub fn copy_flat_sparse_file(source: &mut fs::File, destination: &Path) -> Result<SparseArtifact> {
    process_flat_sparse_file(source, Some(destination))
}

/// Verify a flat sparse image without creating another memory file.
pub fn verify_flat_sparse_file(
    source: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<SparseArtifact> {
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(source)?;

    let artifact = process_flat_sparse_file(&mut source, None)?;
    if artifact.logical_size != expected_size || artifact.sha256 != expected_sha256 {
        return Err(CodecError::Invalid(
            "flat sparse artifact failed size or SHA-256 validation".to_string(),
        )
        .into());
    }

    Ok(artifact)
}

/// Merge the listed delta ranges over a base into a synchronized flat image.
pub fn merge_flat_sparse_file(
    base: &fs::File,
    delta: &mut fs::File,
    ranges: &[MemoryRange],
    destination: &Path,
) -> Result<SparseArtifact> {
    let base_size = base.metadata()?.len();
    let delta_size = delta.metadata()?.len();
    if base_size == 0 || base_size != delta_size {
        return Err(anyhow!("flat sparse base and delta size mismatch"));
    }

    validate_memory_ranges(ranges, base_size, "Dragonball delta")?;

    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .custom_flags(libc::O_NOFOLLOW)
        .open(destination)?;
    output.set_len(base_size)?;

    let chunk_count = base_size.div_ceil(SPARSE_CHUNK_SIZE as u64);
    let mut chunks = data_chunks(base, base_size)?
        .map(|chunks| chunks.into_iter().collect::<BTreeSet<_>>())
        .unwrap_or_else(|| (0..chunk_count).collect());

    for range in ranges {
        let first = range.file_offset / SPARSE_CHUNK_SIZE as u64;
        let last = (range.file_offset + range.length - 1) / SPARSE_CHUNK_SIZE as u64;
        chunks.extend(first..=last);
    }

    let candidate_chunks = chunks.len() as u64;
    let mut buffer = vec![0u8; SPARSE_CHUNK_SIZE];
    let mut hasher = Sha256Context::new(&SHA256);
    hasher.update(&header(base_size));

    let mut non_zero_chunks = 0u64;
    let mut candidate_bytes = 0u64;
    let mut written_bytes = 0u64;
    let mut next_writeback = FLAT_SPARSE_WRITEBACK_BATCH;
    let mut range_index = 0;
    for chunk in chunks {
        let offset = chunk * SPARSE_CHUNK_SIZE as u64;
        let length = (base_size - offset).min(SPARSE_CHUNK_SIZE as u64) as usize;
        let chunk_end = offset + length as u64;
        read_chunk(base, offset, &mut buffer[..length])?;

        while range_index < ranges.len()
            && ranges[range_index].file_offset + ranges[range_index].length <= offset
        {
            range_index += 1;
        }

        for range in &ranges[range_index..] {
            if range.file_offset >= chunk_end {
                break;
            }

            let range_end = range.file_offset + range.length;
            let overlay_start = offset.max(range.file_offset);
            let overlay_end = chunk_end.min(range_end);
            if overlay_start < overlay_end {
                let buffer_start = (overlay_start - offset) as usize;
                let buffer_end = (overlay_end - offset) as usize;
                read_chunk(delta, overlay_start, &mut buffer[buffer_start..buffer_end])?;
            }
        }

        candidate_bytes = candidate_bytes
            .checked_add(length as u64)
            .ok_or_else(|| anyhow!("flat sparse candidate size overflow"))?;
        let all_zero = buffer[..length].iter().all(|byte| *byte == 0);
        if all_zero {
            continue;
        }

        let mut record = [0u8; SPARSE_RECORD_SIZE];
        record[..8].copy_from_slice(&offset.to_le_bytes());
        record[8..12].copy_from_slice(&(length as u32).to_le_bytes());
        hasher.update(&record);
        hasher.update(&buffer[..length]);

        write_all_at(&output, &buffer[..length], offset)?;
        written_bytes = written_bytes
            .checked_add(length as u64)
            .ok_or_else(|| anyhow!("flat sparse written size overflow"))?;

        if written_bytes >= next_writeback {
            start_writeback(&output)?;
            next_writeback = written_bytes
                .checked_add(FLAT_SPARSE_WRITEBACK_BATCH)
                .ok_or_else(|| anyhow!("flat sparse writeback threshold overflow"))?;
        }

        non_zero_chunks += 1;
    }

    output.sync_all()?;

    Ok(SparseArtifact {
        logical_size: base_size,
        encoded_size: base_size,
        sha256: digest_hex(hasher),
        non_zero_chunks,
        stats: SparseEncodeStats {
            candidate_chunks,
            candidate_bytes,
            written_bytes,
        },
    })
}

fn start_writeback(file: &fs::File) -> Result<()> {
    let rc = unsafe {
        libc::sync_file_range(
            file.as_raw_fd(),
            0,
            0,
            libc::SYNC_FILE_RANGE_WRITE as libc::c_uint,
        )
    };

    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error()).context("start flat sparse artifact writeback")
    }
}

#[cfg(test)]
mod tests {
    use super::super::host_page_size;
    use std::os::unix::fs::FileExt;

    use super::*;

    #[test]
    fn flat_delta_merge_uses_exact_ranges_and_preserves_zero_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let base_path = directory.path().join("base");
        let delta_path = directory.path().join("delta");
        let output_path = directory.path().join("merged");
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
        let size = page * 4;
        let base = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&base_path)
            .unwrap();
        base.set_len(size as u64).unwrap();
        base.write_all_at(&vec![0xa5; page * 2], 0).unwrap();
        let mut delta = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&delta_path)
            .unwrap();
        delta.set_len(size as u64).unwrap();
        // The first range deliberately contains zeros and must clear base page 0.
        delta.write_all_at(&vec![0; page], 0).unwrap();
        delta
            .write_all_at(&vec![0x5a; page], (page * 2) as u64)
            .unwrap();
        let ranges = vec![
            MemoryRange {
                file_offset: 0,
                length: page as u64,
            },
            MemoryRange {
                file_offset: (page * 2) as u64,
                length: page as u64,
            },
        ];
        let merged = merge_flat_sparse_file(&base, &mut delta, &ranges, &output_path).unwrap();
        let bytes = fs::read(output_path).unwrap();
        assert!(bytes[..page].iter().all(|byte| *byte == 0));
        assert!(bytes[page..page * 2].iter().all(|byte| *byte == 0xa5));
        assert!(bytes[page * 2..page * 3].iter().all(|byte| *byte == 0x5a));
        assert!(bytes[page * 3..].iter().all(|byte| *byte == 0));
        assert_eq!(merged.stats.written_bytes, size as u64);
    }

    #[test]
    fn flat_delta_merge_keeps_backing_and_handles_ranges_across_chunks() {
        let directory = tempfile::tempdir().unwrap();
        let base_path = directory.path().join("base");
        let delta_path = directory.path().join("delta");
        let page = host_page_size().unwrap();
        let size = 4 * SPARSE_CHUNK_SIZE + page;
        let mut expected = vec![0xa5; size];
        fs::write(&base_path, &expected).unwrap();
        let base = fs::File::open(&base_path).unwrap();
        fs::remove_file(&base_path).unwrap();
        fs::write(&base_path, vec![0xff; size]).unwrap();
        let mut delta = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&delta_path)
            .unwrap();
        delta.set_len(size as u64).unwrap();
        let ranges = vec![
            MemoryRange {
                file_offset: 0,
                length: page as u64,
            },
            MemoryRange {
                file_offset: (SPARSE_CHUNK_SIZE - page) as u64,
                length: (2 * SPARSE_CHUNK_SIZE + 2 * page) as u64,
            },
            MemoryRange {
                file_offset: (size - page) as u64,
                length: page as u64,
            },
        ];
        for range in &ranges {
            let start = range.file_offset as usize;
            let end = start + range.length as usize;
            expected[start..end].fill(0);
        }
        delta
            .write_all_at(&[0x5a], (2 * SPARSE_CHUNK_SIZE) as u64)
            .unwrap();
        expected[2 * SPARSE_CHUNK_SIZE] = 0x5a;
        for (name, ranges, contents) in [
            ("merged", ranges.as_slice(), expected.as_slice()),
            ("unchanged", &[][..], &vec![0xa5; size][..]),
        ] {
            let output = directory.path().join(name);
            let artifact = merge_flat_sparse_file(&base, &mut delta, ranges, &output).unwrap();
            assert_eq!(fs::read(&output).unwrap(), contents);
            verify_flat_sparse_file(&output, size as u64, &artifact.sha256).unwrap();
        }
    }
}
