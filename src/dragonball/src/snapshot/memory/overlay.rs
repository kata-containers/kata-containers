// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use super::MemoryRange;
use super::{
    digest_hex, read_chunk, validate_memory_ranges, SparseArtifact, SparseEncodeStats,
    SPARSE_CHUNK_SIZE,
};
use anyhow::{anyhow, Result};
use ring::digest::{Context as Sha256Context, SHA256};
use std::{
    convert::TryInto,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::Instant,
};

/// Identifier for the Dragonball memory overlay format.
pub const OVERLAY_FORMAT: &str = "dragonball-memory-overlay-v1";
const OVERLAY_MAGIC: &[u8; 8] = b"DBOVLY01";
const OVERLAY_HEADER_SIZE: usize = 32;
const OVERLAY_RECORD_SIZE: usize = 16;

// Every range is stored verbatim, including zero bytes. Its encoded size
// therefore does not depend on reading the payload.
/// Compute the encoded overlay size without reading the memory payload.
pub fn memory_overlay_size(ranges: &[MemoryRange]) -> Result<u64> {
    ranges
        .iter()
        .try_fold(OVERLAY_HEADER_SIZE as u64, |size, range| {
            size.checked_add(OVERLAY_RECORD_SIZE as u64)
                .and_then(|size| size.checked_add(range.length))
                .ok_or_else(|| anyhow!("Dragonball memory overlay size overflows"))
        })
}

fn overlay_header(logical_size: u64, records: u64) -> [u8; OVERLAY_HEADER_SIZE] {
    let mut header = [0_u8; OVERLAY_HEADER_SIZE];
    header[..8].copy_from_slice(OVERLAY_MAGIC);
    header[8..16].copy_from_slice(&logical_size.to_le_bytes());
    header[16..24].copy_from_slice(&records.to_le_bytes());
    header
}

fn update_and_write(output: &mut fs::File, hasher: &mut Sha256Context, bytes: &[u8]) -> Result<()> {
    output.write_all(bytes)?;
    hasher.update(bytes);
    Ok(())
}

/// Encode exact delta ranges, including zero overwrites, and synchronize the file.
pub fn encode_memory_overlay(
    delta: &fs::File,
    ranges: &[MemoryRange],
    destination: &Path,
    logger: &slog::Logger,
) -> Result<SparseArtifact> {
    let started = Instant::now();

    let logical_size = delta.metadata()?.len();
    validate_memory_ranges(ranges, logical_size, "Dragonball overlay")?;

    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .custom_flags(libc::O_NOFOLLOW)
        .open(destination)?;
    let mut hasher = Sha256Context::new(&SHA256);
    update_and_write(
        &mut output,
        &mut hasher,
        &overlay_header(logical_size, ranges.len() as u64),
    )?;

    let mut buffer = vec![0_u8; SPARSE_CHUNK_SIZE];
    let mut payload_bytes = 0_u64;
    for range in ranges {
        let mut record = [0_u8; OVERLAY_RECORD_SIZE];
        record[..8].copy_from_slice(&range.file_offset.to_le_bytes());
        record[8..].copy_from_slice(&range.length.to_le_bytes());
        update_and_write(&mut output, &mut hasher, &record)?;
        let mut offset = range.file_offset;
        let end = range.file_offset + range.length;
        while offset < end {
            let length = (end - offset).min(buffer.len() as u64) as usize;
            read_chunk(delta, offset, &mut buffer[..length])?;
            update_and_write(&mut output, &mut hasher, &buffer[..length])?;
            offset += length as u64;
            payload_bytes += length as u64;
        }
    }

    let sync_started = Instant::now();
    output.sync_all()?;
    let sync_us = sync_started.elapsed().as_micros();
    let encoded_size = output.metadata()?.len();

    slog::debug!(logger, "encoded Dragonball memory overlay";
        "logical_bytes" => logical_size,
        "encoded_bytes" => encoded_size,
        "payload_bytes" => payload_bytes,
        "ranges" => ranges.len(),
        "elapsed_us" => started.elapsed().as_micros(),
        "sync_us" => sync_us,
    );

    Ok(SparseArtifact {
        logical_size,
        encoded_size,
        sha256: digest_hex(hasher),
        non_zero_chunks: ranges.len() as u64,
        stats: SparseEncodeStats {
            candidate_chunks: ranges.len() as u64,
            candidate_bytes: payload_bytes,
            written_bytes: encoded_size,
        },
    })
}

/// A streaming decoder of overlay record locations; payload stays in the file.
pub struct OverlayReader<'a> {
    file: &'a fs::File,
    logical_size: u64,
    encoded_size: u64,
    remaining: u64,
    cursor: u64,
    previous_end: u64,
    page_size: u64,
}

impl<'a> OverlayReader<'a> {
    /// Validate the header against the expected memory layout.
    pub fn new(file: &'a fs::File, logical_size: u64) -> Result<Self> {
        use std::os::unix::fs::FileExt;
        let encoded_size = file.metadata()?.len();
        let mut header = [0_u8; OVERLAY_HEADER_SIZE];
        file.read_exact_at(&mut header, 0)?;
        let records = u64::from_le_bytes(header[16..24].try_into().unwrap());
        if &header[..8] != OVERLAY_MAGIC
            || u64::from_le_bytes(header[8..16].try_into().unwrap()) != logical_size
            || header[24..].iter().any(|byte| *byte != 0)
            || records
                > encoded_size.saturating_sub(OVERLAY_HEADER_SIZE as u64)
                    / OVERLAY_RECORD_SIZE as u64
        {
            return Err(super::CodecError::Invalid(
                "invalid Dragonball memory overlay header".into(),
            )
            .into());
        }
        Ok(Self {
            file,
            logical_size,
            encoded_size,
            remaining: records,
            cursor: OVERLAY_HEADER_SIZE as u64,
            previous_end: 0,
            page_size: super::host_page_size()? as u64,
        })
    }

    /// Return the next memory range and its payload offset, without copying it.
    pub fn next_range(&mut self) -> Result<Option<(MemoryRange, u64)>> {
        use std::os::unix::fs::FileExt;
        let invalid =
            || super::CodecError::Invalid("invalid Dragonball memory overlay record".into());
        if self.remaining == 0 {
            if self.cursor != self.encoded_size {
                return Err(invalid().into());
            }
            return Ok(None);
        }
        let record_end = self
            .cursor
            .checked_add(OVERLAY_RECORD_SIZE as u64)
            .ok_or_else(invalid)?;
        if record_end > self.encoded_size {
            return Err(invalid().into());
        }
        let mut record = [0_u8; OVERLAY_RECORD_SIZE];
        self.file.read_exact_at(&mut record, self.cursor)?;
        let file_offset = u64::from_le_bytes(record[..8].try_into().unwrap());
        let length = u64::from_le_bytes(record[8..].try_into().unwrap());
        let end = file_offset.checked_add(length).ok_or_else(invalid)?;
        let payload_end = record_end.checked_add(length).ok_or_else(invalid)?;
        if length == 0
            || file_offset % self.page_size != 0
            || length % self.page_size != 0
            || file_offset < self.previous_end
            || end > self.logical_size
            || payload_end > self.encoded_size
        {
            return Err(invalid().into());
        }
        self.cursor = payload_end;
        self.previous_end = end;
        self.remaining -= 1;
        Ok(Some((
            MemoryRange {
                file_offset,
                length,
            },
            record_end,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicted_size_matches_the_encoded_file() {
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        let delta = tempfile::tempfile().unwrap();
        delta.set_len(page * 4).unwrap();
        let directory = tempfile::tempdir().unwrap();
        for (index, ranges) in [
            vec![],
            vec![MemoryRange {
                file_offset: 0,
                length: page,
            }],
            vec![
                MemoryRange {
                    file_offset: 0,
                    length: page,
                },
                MemoryRange {
                    file_offset: page * 2,
                    length: page * 2,
                },
            ],
        ]
        .iter()
        .enumerate()
        {
            let encoded = encode_memory_overlay(
                &delta,
                ranges,
                &directory.path().join(index.to_string()),
                &slog::Logger::root(slog::Discard, slog::o!()),
            )
            .unwrap();
            assert_eq!(memory_overlay_size(ranges).unwrap(), encoded.encoded_size);
            let output = fs::File::open(directory.path().join(index.to_string())).unwrap();
            let mut reader = OverlayReader::new(&output, page * 4).unwrap();
            for expected in ranges {
                let (actual, _) = reader.next_range().unwrap().unwrap();
                assert_eq!(actual.file_offset, expected.file_offset);
                assert_eq!(actual.length, expected.length);
            }
            assert!(reader.next_range().unwrap().is_none());
        }
        assert!(memory_overlay_size(&[MemoryRange {
            file_offset: 0,
            length: u64::MAX
        }])
        .is_err());
    }
    #[test]
    fn decoder_rejects_invalid_headers_ranges_and_payload_lengths() {
        let page = super::super::host_page_size().unwrap() as u64;
        let source = tempfile::tempfile().unwrap();
        source.set_len(page * 2).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("overlay");
        encode_memory_overlay(
            &source,
            &[
                MemoryRange {
                    file_offset: 0,
                    length: page,
                },
                MemoryRange {
                    file_offset: page,
                    length: page,
                },
            ],
            &path,
            &slog::Logger::root(slog::Discard, slog::o!()),
        )
        .unwrap();
        let original = fs::read(&path).unwrap();
        let mut cases = Vec::new();
        let mut bytes = original.clone();
        bytes[0] ^= 1;
        cases.push(bytes);
        let mut bytes = original.clone();
        bytes[8..16].copy_from_slice(&(page * 3).to_le_bytes());
        cases.push(bytes);
        let mut bytes = original.clone();
        bytes[24] = 1;
        cases.push(bytes);
        let mut bytes = original.clone();
        bytes[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
        cases.push(bytes);
        let mut bytes = original.clone();
        bytes[32..40].copy_from_slice(&1_u64.to_le_bytes());
        cases.push(bytes);
        let mut bytes = original.clone();
        bytes[40..48].copy_from_slice(&0_u64.to_le_bytes());
        cases.push(bytes);
        let mut bytes = original.clone();
        let second = 48 + page as usize;
        bytes[second..second + 8].copy_from_slice(&0_u64.to_le_bytes());
        cases.push(bytes);
        let mut bytes = original.clone();
        bytes.pop();
        cases.push(bytes);
        let mut bytes = original;
        bytes.push(0);
        cases.push(bytes);
        cases.push(vec![0; 8]);
        for (index, bytes) in cases.iter().enumerate() {
            let path = directory.path().join(index.to_string());
            fs::write(&path, bytes).unwrap();
            let file = fs::File::open(path).unwrap();
            let result = (|| -> Result<()> {
                let mut reader = OverlayReader::new(&file, page * 2)?;
                while reader.next_range()?.is_some() {}
                Ok(())
            })();
            assert!(
                result.is_err(),
                "invalid overlay case {} was accepted",
                index
            );
        }
    }
}
