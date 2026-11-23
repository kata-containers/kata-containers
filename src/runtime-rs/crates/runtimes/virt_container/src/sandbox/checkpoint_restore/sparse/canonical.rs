// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
use super::{
    data_chunks, digest_hex, header, read_chunk, SparseArtifact, SparseEncodeStats,
    SPARSE_CHUNK_SIZE, SPARSE_RECORD_SIZE,
};
use anyhow::{anyhow, Result};
use ring::digest::{Context as Sha256Context, SHA256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom};

    #[test]
    fn encoding_is_canonical_for_sparse_and_dense_sources() {
        let directory = tempfile::tempdir().unwrap();
        let sparse = directory.path().join("sparse.raw");
        let dense = directory.path().join("dense.raw");
        let encoded_sparse = directory.path().join("sparse.artifact");
        let encoded_file = directory.path().join("file.artifact");
        let encoded_dense = directory.path().join("dense.artifact");
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
    }

    #[test]
    fn encoder_rejects_non_regular_source() {
        let directory = tempfile::tempdir().unwrap();
        let artifact = directory.path().join("source.sparse");
        assert!(encode_sparse_artifact(directory.path(), &artifact).is_err());
        assert!(!artifact.exists());
    }
}
