use std::{
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
};

use anyhow::Result;
use async_compression::tokio::bufread::GzipDecoder;
use slog::{error, info, Logger};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::task::JoinHandle;

/// Magic number of initdata device
pub const INITDATA_MAGIC_NUMBER: &[u8] = b"initdata";

/// It's designed to be run in a separate tokio task to check if a the potential device is the initdata device.
async fn check_initdata_device(logger: Logger, path: PathBuf) -> Result<Option<String>> {
    let metadata = match tokio::fs::metadata(&path).await {
        Ok(m) => m,
        Err(_) => return Ok(None),
    };

    if !metadata.file_type().is_block_device() {
        return Ok(None);
    }

    info!(logger, "Initdata find a potential device: `{path:?}`");

    let mut file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) => {
            error!(
                logger,
                "Could not open the potential device `{path:?}`: {e}"
            );
            return Ok(None);
        }
    };

    let mut magic = [0; 8];
    match file.read_exact(&mut magic).await {
        Ok(_) if magic == INITDATA_MAGIC_NUMBER => {
            let device_path = path.to_string_lossy().into_owned();
            info!(logger, "Found initdata device {device_path}");
            Ok(Some(device_path))
        }
        _ => {
            // This covers both the case where magic doesn't match and read errors.
            // We don't need to bubble up read errors as failures for the whole process.
            Ok(None)
        }
    }
}

/// Concurrently locate devices using `tokio::spawn`.
pub async fn locate_device_concurrently(logger: &Logger) -> Result<Option<String>> {
    let dev_dir = Path::new("/dev");
    let mut read_dir = tokio::fs::read_dir(dev_dir).await?;

    // The `handles` to store the concurrent checking tasks.
    let mut handles: Vec<JoinHandle<Result<Option<String>>>> = Vec::new();

    while let Some(entry) = read_dir.next_entry().await? {
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();

        // Just check the file starting with 'vd'
        if !filename_str.starts_with("vd") {
            continue;
        }

        // For each potential device, spawn a new task to check it.
        let path = entry.path();
        let logger_clone = logger.clone();
        let handle = tokio::spawn(async move { check_initdata_device(logger_clone, path).await });

        handles.push(handle);
    }

    for handle in handles {
        match handle.await? {
            Ok(Some(device_path)) => {
                // Found it, return immediately.
                return Ok(Some(device_path));
            }
            Ok(None) => {
                continue;
            }
            Err(e) => {
                error!(logger, "A device check task failed: {e:?}");
                continue;
            }
        }
    }

    Ok(None)
}

/// Open and decompresses data from the initdata device.
pub async fn read_initdata(device_path: &PathBuf) -> Result<Vec<u8>> {
    let initdata_device = tokio::fs::File::open(device_path).await?;
    let mut buf_reader = tokio::io::BufReader::new(initdata_device);
    // skip the magic number "initdata" with 8 bytes
    buf_reader.seek(std::io::SeekFrom::Start(8)).await?;

    let mut len_buf = [0u8; 8];
    buf_reader.read_exact(&mut len_buf).await?;
    let length = u64::from_le_bytes(len_buf) as usize;

    // Take a limited view of the reader for the compressed data
    let compressed_reader = buf_reader.take(length as u64);
    // Decompress data directly from the reader stream.
    let mut gzip_decoder = GzipDecoder::new(compressed_reader);

    let mut initdata = Vec::new();
    let _ = gzip_decoder.read_to_end(&mut initdata).await?;
    Ok(initdata)
}
