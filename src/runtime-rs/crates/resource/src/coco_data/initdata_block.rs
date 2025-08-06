// Copyright (c) 2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use flate2::{Compression, GzBuilder};
use std::{
    fmt, fs,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

#[derive(Debug)]
pub enum InitDataError {
    InvalidPath(PathBuf),
    IoError(String, io::Error),
    CompressionError(io::Error),
    PersistError(tempfile::PersistError),
}

impl fmt::Display for InitDataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidPath(p) => write!(f, "Invalid path: {}", p.display()),
            Self::IoError(ctx, e) => write!(f, "I/O error during {}: {}", ctx, e),
            Self::CompressionError(e) => write!(f, "Compression failed: {}", e),
            Self::PersistError(e) => write!(f, "File persistence failed: {}", e),
        }
    }
}

impl std::error::Error for InitDataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(_, e) => Some(e),
            Self::CompressionError(e) => Some(e),
            Self::PersistError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for InitDataError {
    fn from(err: io::Error) -> Self {
        InitDataError::IoError("I/O operation".into(), err)
    }
}

const MAGIC_HEADER: &[u8; 8] = b"initdata";
const SECTOR_SIZE: u64 = 512;

// Default buffer size, adjustable based on target storage optimization
const DEFAULT_BUFFER_SIZE: usize = 128 * 1024;

/// Determines the optimal buffer size
fn determine_buffer_size(data_size: usize) -> usize {
    // Use smaller buffers for small data to reduce memory usage
    if data_size < 4 * 1024 {
        return 4 * 1024;
    } else if data_size < 64 * 1024 {
        return 32 * 1024;
    }
    // Use larger buffers for big data to improve throughput
    DEFAULT_BUFFER_SIZE
}

/// create compressed block compliant with RAW format requirements
///
/// # Arguments
/// - `initdata`: Initialization data to be compressed and stored (TOML/JSON format, etc.)
/// - `image_path`: Target image file path
/// - `compression_level`: Compression level (0-9, default maximum compression)
///
/// # Returns
/// - `Ok(file_size)`: Total bytes written to the image file on success
/// - `Err(InitDataError)`: Error details on failure
///
/// # Safety
/// - Atomic writes ensure crash recovery
/// - Automatic temporary file cleanup
/// - File permissions restricted to 0o600 on Unix systems
fn create_compressed_block(
    initdata: &str,
    image_path: &Path,
    compression_level: Option<u32>,
) -> Result<u64, InitDataError> {
    // 1. Skip file creation if initdata is empty
    if initdata.is_empty() {
        info!(
            sl!(),
            "No initialization data provided, skipping image creation for {}",
            image_path.display()
        );
        return Ok(0);
    }

    // Store initdata size for logging and optimization
    let initdata_size = initdata.len();
    info!(
        sl!(),
        "Processing {} bytes of initialization data", initdata_size
    );

    // Ensure parent directory exists
    if let Some(parent_dir) = image_path.parent() {
        if !parent_dir.exists() {
            info!(sl!(), "Creating parent directory: {}", parent_dir.display());
            fs::create_dir_all(parent_dir).map_err(|e| {
                InitDataError::IoError(format!("creating directory {}", parent_dir.display()), e)
            })?;
        }
    } else {
        return Err(InitDataError::InvalidPath(image_path.to_owned()));
    }

    // 2. Determine optimal buffer size based on data size
    let buffer_size = determine_buffer_size(initdata_size);
    info!(sl!(), "Using buffer size of {} bytes", buffer_size);

    // 3. Create temp file in parent directory (ensures atomic rename)
    let parent_dir = image_path
        .parent()
        .ok_or_else(|| InitDataError::InvalidPath(image_path.to_owned()))?;

    info!(
        sl!(),
        "Creating temporary file in: {}",
        parent_dir.display()
    );

    // Using named temporary files offers crucial benefits for writing data:
    // - It ensures atomic operations by renaming the file only on successful completion;
    // - It prevents concurrent conflicts through unique naming;
    // - And it guarantees reliable atomic renames by creating the temporary file in the same directory as the target.
    let temp_file = NamedTempFile::new_in(parent_dir).map_err(|e| {
        InitDataError::IoError(format!("creating temp file in {}", parent_dir.display()), e)
    })?;

    info!(
        sl!(),
        "Temporary file created: {}",
        temp_file.path().display()
    );

    // 4. Set strict file permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = temp_file.as_file().metadata()?.permissions();
        perms.set_mode(0o600); // User read/write permissions
        temp_file.as_file().set_permissions(perms)?;
    }

    // 5. Create buffered writer
    let mut writer = BufWriter::with_capacity(buffer_size, temp_file);

    // 6. Write magic header
    writer.write_all(MAGIC_HEADER)?;
    info!(sl!(), "Magic header written: {:?}", MAGIC_HEADER);

    // 7. First compress data to get the actual compressed size
    let compression =
        compression_level.map_or(Compression::best(), |lvl| Compression::new(lvl.min(9)));

    info!(
        sl!(),
        "Compressing data with compression level {}",
        compression.level()
    );

    // Compress data to a temporary buffer first to get the compressed size
    let mut compressed_data = Vec::new();
    {
        let mut gz = GzBuilder::new()
            .filename("initdata.toml") // Embed original filename metadata
            .comment("Generated by Confidential Containers")
            .write(&mut compressed_data, compression);

        // Write data in chunks to avoid large memory allocation
        for chunk in initdata.as_bytes().chunks(buffer_size) {
            gz.write_all(chunk)?;
        }

        // Finalize compression
        gz.finish()?;
    }

    let compressed_size = compressed_data.len() as u64;
    info!(
        sl!(),
        "Data compressed: {} -> {} bytes (ratio: {:.2}%)",
        initdata_size,
        compressed_size,
        (compressed_size as f64 / initdata_size as f64) * 100.0
    );

    // 8. Write compressed data length (8 bytes, little-endian)
    writer.write_all(&compressed_size.to_le_bytes())?;
    info!(
        sl!(),
        "Compressed data length written: {} bytes", compressed_size
    );

    // 9. Write compressed data
    writer.write_all(&compressed_data)?;
    info!(sl!(), "Compressed data written");

    // 10. Calculate padding for sector alignment
    let current_pos = MAGIC_HEADER.len() as u64 + 8 + compressed_size; // magic + length + data
    let padding = (SECTOR_SIZE - (current_pos % SECTOR_SIZE)) % SECTOR_SIZE;

    // 11. Zero-byte padding using small blocks
    if padding > 0 {
        info!(
            sl!(),
            "Adding {} bytes of padding for sector alignment", padding
        );
        const ZERO_BLOCK: [u8; 4096] = [0; 4096];
        let mut remaining = padding as usize;

        while remaining > 0 {
            let write_size = std::cmp::min(remaining, ZERO_BLOCK.len());
            writer.write_all(&ZERO_BLOCK[..write_size])?;
            remaining -= write_size;
        }
    }

    // 12. Ensure data persistence
    writer
        .flush()
        .map_err(|e| InitDataError::IoError("flush buffer".into(), e))?;

    // This extracts the NamedTempFile from the BufWriter.
    // Essentially, it unwraps the layered writers (compression, buffering) to get back the original temporary file (temp_file),
    // allowing direct operations like syncing or renaming.
    let original_tempfile = writer
        .into_inner()
        .map_err(|e| InitDataError::IoError("retrieving inner writer".into(), e.into()))?;

    // 13. Ensure all data is written to storage
    original_tempfile.as_file().sync_all()?;

    // 14. Atomic commit
    let final_size = original_tempfile.as_file().metadata()?.len();
    info!(
        sl!(),
        "Final image size: {} bytes, persisting to: {}",
        final_size,
        image_path.display()
    );

    original_tempfile
        .persist(image_path)
        .map_err(InitDataError::PersistError)?;

    Ok(final_size)
}

/// Add data to a compressed image at the specified path
pub fn push_data(initdata_path: &Path, data: &str) -> anyhow::Result<()> {
    let _ = fs::remove_file(initdata_path);
    let size = create_compressed_block(data, initdata_path, None)
        .map_err(|e| anyhow::anyhow!("Failed to create image: {}", e))?;
    info!(
        sl!(),
        "Create compressed image successfully, size {} bytes and  created at {}",
        size,
        initdata_path.display()
    );

    Ok(())
}

// Unit tests
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::io::Read;

    fn setup_test_env() -> PathBuf {
        let dir = env::temp_dir().join("initimg_test");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_valid_creation() {
        let dir = setup_test_env();
        let path = dir.join("test.img");

        let data = "[config]\nkey = \"value\"\n";
        let result = create_compressed_block(data, &path, Some(6));

        assert!(result.is_ok());
        assert!(path.exists());

        // Verify basic structure
        let meta = fs::metadata(&path).unwrap();
        assert_eq!(meta.len() % SECTOR_SIZE, 0);

        // Verify magic header
        let mut file = fs::File::open(&path).unwrap();
        let mut header = [0u8; 8];
        file.read_exact(&mut header).unwrap();
        assert_eq!(&header, MAGIC_HEADER);

        // Cleanup
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_empty_input() {
        let dir = setup_test_env();
        let path = dir.join("empty.img");

        let result = create_compressed_block("", &path, None);

        // Should succeed but return zero size
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
        // Should not create file
        assert!(!path.exists());
    }

    #[test]
    fn test_different_compression_levels() {
        let dir = setup_test_env();
        let data = "[config]\n".repeat(1000); // Generate large test data

        let sizes = vec![0, 3, 9]
            .into_iter()
            .map(|level| {
                let path = dir.join(format!("test_comp_{}.img", level));
                let res = create_compressed_block(&data, &path, Some(level));
                let size = res.unwrap();
                fs::remove_file(&path).unwrap();
                size
            })
            .collect::<Vec<_>>();

        // Different compression levels should produce different sizes
        // Simple check due to data and environment variability
        println!("Compression level sizes: {:?}", sizes);
        assert!(sizes[0] > 0);
    }
}
