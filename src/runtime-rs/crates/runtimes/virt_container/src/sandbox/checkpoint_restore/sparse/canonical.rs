// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use super::{
    data_chunks, digest_hex, header, read_chunk, write_all_at, SparseArtifact, SparseDecodeStats,
    SparseEncodeStats, SparseMeta, SPARSE_CHUNK_SIZE, SPARSE_HEADER_SIZE, SPARSE_MAGIC,
    SPARSE_RECORD_SIZE,
};
use anyhow::{anyhow, Result};
use common::error::Error as CommonError;
use ring::digest::{Context as Sha256Context, SHA256};
use std::{
    convert::TryInto,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::mpsc::sync_channel,
    thread,
};

struct SparseWriteChunk {
    offset: u64,
    length: usize,
    payload: Vec<u8>,
}

struct SparseReaderResult {
    records: u64,
    written_bytes: u64,
}

fn parse_header(value: &[u8; SPARSE_HEADER_SIZE]) -> Result<u64> {
    if &value[..8] != SPARSE_MAGIC
        || u32::from_le_bytes(value[8..12].try_into().unwrap()) != SPARSE_HEADER_SIZE as u32
        || u32::from_le_bytes(value[12..16].try_into().unwrap()) != SPARSE_CHUNK_SIZE as u32
        || value[24..].iter().any(|byte| *byte != 0)
    {
        return Err(CommonError::InvalidSandboxOperation(
            "invalid canonical sparse artifact header".to_string(),
        )
        .into());
    }

    let logical_size = u64::from_le_bytes(value[16..24].try_into().unwrap());
    if logical_size == 0 {
        return Err(CommonError::InvalidSandboxOperation(
            "canonical sparse artifact has zero logical size".to_string(),
        )
        .into());
    }

    Ok(logical_size)
}

pub(in crate::sandbox::checkpoint_restore) fn encode_sparse_artifact(
    source: &Path,
    destination: &Path,
) -> Result<SparseArtifact> {
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(source)?;

    encode_sparse_file(&mut source, destination)
}

fn encode_sparse_file(source: &mut fs::File, destination: &Path) -> Result<SparseArtifact> {
    let source_metadata = source.metadata()?;
    let logical_size = source_metadata.len();
    if !source_metadata.is_file() || logical_size == 0 || logical_size > libc::off_t::MAX as u64 {
        return Err(anyhow!(
            "sparse artifact source is not a supported regular file"
        ));
    }

    let mut hasher = Sha256Context::new(&SHA256);
    let sparse_header = header(logical_size);

    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .custom_flags(libc::O_NOFOLLOW)
        .open(destination)?;

    output.write_all(&sparse_header)?;
    hasher.update(&sparse_header);

    let chunk_count = logical_size.div_ceil(SPARSE_CHUNK_SIZE as u64);
    let chunks =
        data_chunks(source, logical_size)?.unwrap_or_else(|| (0..chunk_count).collect::<Vec<_>>());
    let candidate_chunks = chunks.len() as u64;

    let mut buffer = vec![0_u8; SPARSE_CHUNK_SIZE];
    let mut non_zero_chunks = 0_u64;
    let mut candidate_bytes = 0_u64;
    let mut written_bytes = 0_u64;

    for chunk in chunks {
        let offset = chunk
            .checked_mul(SPARSE_CHUNK_SIZE as u64)
            .ok_or_else(|| anyhow!("sparse artifact chunk offset overflow"))?;
        let length = (logical_size - offset).min(SPARSE_CHUNK_SIZE as u64) as usize;

        read_chunk(source, offset, &mut buffer[..length])?;
        candidate_bytes = candidate_bytes
            .checked_add(length as u64)
            .ok_or_else(|| anyhow!("canonical sparse artifact candidate size overflow"))?;

        let all_zero = buffer[..length].iter().all(|byte| *byte == 0);
        if all_zero {
            continue;
        }

        let mut record = [0_u8; SPARSE_RECORD_SIZE];
        record[..8].copy_from_slice(&offset.to_le_bytes());
        record[8..12].copy_from_slice(&(length as u32).to_le_bytes());
        output.write_all(&record)?;
        output.write_all(&buffer[..length])?;
        hasher.update(&record);
        hasher.update(&buffer[..length]);
        non_zero_chunks += 1;

        written_bytes = written_bytes
            .checked_add(length as u64)
            .ok_or_else(|| anyhow!("canonical sparse artifact written size overflow"))?;
    }

    output.sync_all()?;

    let encoded_size = output.metadata()?.len();

    Ok(SparseArtifact {
        logical_size,
        encoded_size,
        sha256: digest_hex(hasher),
        non_zero_chunks,
        stats: SparseEncodeStats {
            candidate_chunks,
            candidate_bytes,
            written_bytes,
        },
    })
}

pub(in crate::sandbox::checkpoint_restore) fn decode_sparse_artifact(
    source: &Path,
    destination: &Path,
    expected: SparseMeta<'_>,
    mode: u32,
    sync_destination: bool,
) -> Result<SparseDecodeStats> {
    let result = (|| -> Result<SparseDecodeStats> {
        let mut input = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(source)?;

        let input_metadata = input.metadata()?;
        if !input_metadata.is_file() || input_metadata.len() != expected.encoded_size {
            return Err(anyhow!("canonical sparse artifact encoded size mismatch"));
        }

        let mut hasher = Sha256Context::new(&SHA256);
        let mut header = [0_u8; SPARSE_HEADER_SIZE];

        input.read_exact(&mut header)?;
        hasher.update(&header);
        let logical_size = parse_header(&header)?;
        if logical_size != expected.logical_size {
            return Err(anyhow!("canonical sparse artifact logical size mismatch"));
        }

        let output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .custom_flags(libc::O_NOFOLLOW)
            .open(destination)?;
        output.set_len(logical_size)?;

        let (free_sender, free_receiver) = sync_channel(2);
        free_sender
            .send(vec![0_u8; SPARSE_CHUNK_SIZE])
            .map_err(|_| anyhow!("initialize sparse decode free buffer pool"))?;
        free_sender
            .send(vec![0_u8; SPARSE_CHUNK_SIZE])
            .map_err(|_| anyhow!("initialize sparse decode free buffer pool"))?;

        let (ready_sender, ready_receiver) = sync_channel::<SparseWriteChunk>(2);
        let writer = thread::spawn(move || -> Result<fs::File> {
            while let Ok(chunk) = ready_receiver.recv() {
                write_all_at(&output, &chunk.payload[..chunk.length], chunk.offset)?;
                free_sender
                    .send(chunk.payload)
                    .map_err(|_| anyhow!("return sparse decode buffer"))?;
            }

            Ok(output)
        });

        let reader = (|| -> Result<SparseReaderResult> {
            let mut previous_offset = None;
            let mut encoded_read = SPARSE_HEADER_SIZE as u64;
            let mut records = 0_u64;
            let mut written_bytes = 0_u64;

            loop {
                let mut record = [0_u8; SPARSE_RECORD_SIZE];
                let first = input.read(&mut record[..1])?;
                if first == 0 {
                    break;
                }

                input.read_exact(&mut record[1..])?;
                hasher.update(&record);
                encoded_read = encoded_read
                    .checked_add(SPARSE_RECORD_SIZE as u64)
                    .ok_or_else(|| anyhow!("canonical sparse artifact encoded size overflow"))?;

                let offset = u64::from_le_bytes(record[..8].try_into().unwrap());
                let length = u32::from_le_bytes(record[8..12].try_into().unwrap()) as usize;
                let expected_length = logical_size
                    .saturating_sub(offset)
                    .min(SPARSE_CHUNK_SIZE as u64) as usize;
                if record[12..].iter().any(|byte| *byte != 0)
                    || offset % SPARSE_CHUNK_SIZE as u64 != 0
                    || previous_offset.is_some_and(|previous| offset <= previous)
                    || length == 0
                    || length != expected_length
                {
                    return Err(CommonError::InvalidSandboxOperation(
                        "non-canonical sparse artifact record".to_string(),
                    )
                    .into());
                }

                let mut payload = free_receiver
                    .recv()
                    .map_err(|_| anyhow!("sparse decode writer stopped"))?;
                input.read_exact(&mut payload[..length])?;
                hasher.update(&payload[..length]);
                encoded_read = encoded_read
                    .checked_add(length as u64)
                    .ok_or_else(|| anyhow!("canonical sparse artifact encoded size overflow"))?;
                if payload[..length].iter().all(|byte| *byte == 0) {
                    return Err(CommonError::InvalidSandboxOperation(
                        "canonical sparse artifact contains an all-zero record".to_string(),
                    )
                    .into());
                }

                ready_sender
                    .send(SparseWriteChunk {
                        offset,
                        length,
                        payload,
                    })
                    .map_err(|_| anyhow!("sparse decode writer stopped"))?;
                previous_offset = Some(offset);
                records = records
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("canonical sparse artifact record count overflow"))?;
                written_bytes = written_bytes
                    .checked_add(length as u64)
                    .ok_or_else(|| anyhow!("canonical sparse artifact written size overflow"))?;
            }

            let digest = digest_hex(hasher);
            let valid = encoded_read == expected.encoded_size && digest == expected.sha256;
            if !valid {
                return Err(CommonError::InvalidSandboxOperation(
                    "canonical sparse artifact failed size or SHA-256 validation".to_string(),
                )
                .into());
            }

            Ok(SparseReaderResult {
                records,
                written_bytes,
            })
        })();

        drop(ready_sender);

        let writer = writer
            .join()
            .map_err(|_| anyhow!("canonical sparse artifact writer panicked"))?;

        drop(free_receiver);

        let (reader, output) = match (reader, writer) {
            (_, Err(err)) => return Err(err.context("write canonical sparse artifact")),
            (Err(err), Ok(_)) => return Err(err),
            (Ok(reader), Ok(output)) => (reader, output),
        };

        if sync_destination {
            output.sync_all()?;
        }

        Ok(SparseDecodeStats {
            records: reader.records,
            written_bytes: reader.written_bytes,
        })
    })();

    if result.is_err() {
        let _ = fs::remove_file(destination);
    }

    result
}

#[cfg(test)]
mod tests {
    use std::io::{Seek, SeekFrom};

    use super::*;

    fn sha256(bytes: &[u8]) -> String {
        let mut hasher = Sha256Context::new(&SHA256);
        hasher.update(bytes);
        digest_hex(hasher)
    }

    #[test]
    fn encoding_is_canonical_and_round_trips_sparse_content() {
        let directory = tempfile::tempdir().unwrap();
        let sparse = directory.path().join("sparse.raw");
        let dense = directory.path().join("dense.raw");
        let encoded_sparse = directory.path().join("sparse.artifact");
        let encoded_file = directory.path().join("file.artifact");
        let encoded_dense = directory.path().join("dense.artifact");
        let restored = directory.path().join("restored.raw");
        let mut contents = vec![0_u8; 3 * SPARSE_CHUNK_SIZE + 17];
        contents[7] = 1;
        contents[2 * SPARSE_CHUNK_SIZE + 11] = 2;
        fs::write(&dense, &contents).unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&sparse)
            .unwrap();
        file.set_len(contents.len() as u64).unwrap();
        file.seek(SeekFrom::Start(7)).unwrap();
        file.write_all(&[1]).unwrap();
        file.seek(SeekFrom::Start((2 * SPARSE_CHUNK_SIZE + 11) as u64))
            .unwrap();
        file.write_all(&[2]).unwrap();

        let sparse_info = encode_sparse_artifact(&sparse, &encoded_sparse).unwrap();
        let file_info = encode_sparse_file(&mut file, &encoded_file).unwrap();
        let dense_info = encode_sparse_artifact(&dense, &encoded_dense).unwrap();
        assert_eq!(
            fs::read(&encoded_sparse).unwrap(),
            fs::read(&encoded_dense).unwrap()
        );
        assert_eq!(
            fs::read(&encoded_sparse).unwrap(),
            fs::read(&encoded_file).unwrap()
        );
        assert_eq!(sparse_info.sha256, dense_info.sha256);
        assert_eq!(sparse_info.sha256, file_info.sha256);
        assert_eq!(sparse_info.non_zero_chunks, 2);
        assert_eq!(
            sparse_info.stats.written_bytes,
            2 * SPARSE_CHUNK_SIZE as u64
        );
        assert!(sparse_info.stats.candidate_chunks >= sparse_info.non_zero_chunks);
        let decoded = decode_sparse_artifact(
            &encoded_sparse,
            &restored,
            SparseMeta {
                encoded_size: sparse_info.encoded_size,
                logical_size: sparse_info.logical_size,
                sha256: &sparse_info.sha256,
            },
            0o600,
            false,
        )
        .unwrap();
        assert_eq!(decoded.records, 2);
        assert_eq!(decoded.written_bytes, 2 * SPARSE_CHUNK_SIZE as u64);
        assert_eq!(fs::read(restored).unwrap(), contents);
    }

    #[test]
    fn decoder_rejects_corruption_and_removes_partial_output() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.raw");
        let artifact = directory.path().join("source.sparse");
        let restored = directory.path().join("restored.raw");
        fs::write(&source, b"not zero").unwrap();
        let info = encode_sparse_artifact(&source, &artifact).unwrap();
        let mut bytes = fs::read(&artifact).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&artifact, bytes).unwrap();
        assert!(decode_sparse_artifact(
            &artifact,
            &restored,
            SparseMeta {
                encoded_size: info.encoded_size,
                logical_size: info.logical_size,
                sha256: &info.sha256,
            },
            0o600,
            false
        )
        .is_err());
        assert!(!restored.exists());
    }

    #[test]
    fn decoder_rejects_non_canonical_duplicate_record() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.raw");
        let artifact = directory.path().join("source.sparse");
        let malformed = directory.path().join("malformed.sparse");
        let restored = directory.path().join("restored.raw");
        fs::write(&source, b"not zero").unwrap();
        let info = encode_sparse_artifact(&source, &artifact).unwrap();
        let mut bytes = fs::read(&artifact).unwrap();
        bytes.extend_from_within(SPARSE_HEADER_SIZE..);
        let malformed_digest = sha256(&bytes);
        fs::write(&malformed, &bytes).unwrap();
        assert!(decode_sparse_artifact(
            &malformed,
            &restored,
            SparseMeta {
                encoded_size: bytes.len() as u64,
                logical_size: info.logical_size,
                sha256: &malformed_digest,
            },
            0o600,
            false
        )
        .is_err());
        assert!(!restored.exists());
    }

    #[test]
    fn encoder_rejects_non_regular_source() {
        let directory = tempfile::tempdir().unwrap();
        let artifact = directory.path().join("source.sparse");
        assert!(encode_sparse_artifact(directory.path(), &artifact).is_err());
        assert!(!artifact.exists());
    }
}
