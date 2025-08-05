use std::{
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use async_compression::tokio::bufread::GzipDecoder;
use futures::{stream::FuturesUnordered, StreamExt};
use slog::{info, Logger};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// Magic number of initdata device
pub const INITDATA_MAGIC_NUMBER: &[u8] = b"initdata";

const INITDATA_PATH_BY_ID: &str = "/dev/disk/by-id/virtio-initdata";

/// It's designed to be run in a separate tokio task to check if a the potential device is the initdata device.
async fn check_initdata_device(logger: &Logger, path: PathBuf) -> Result<Option<PathBuf>> {
    let metadata = tokio::fs::metadata(&path)
        .await
        .context(format!("stat'ing file {path:?}"))?;

    if !metadata.file_type().is_block_device() {
        return Ok(None);
    }

    info!(logger, "Initdata find a potential device: `{path:?}`");

    let mut file = tokio::fs::File::open(&path)
        .await
        .context(format!("opening device {path:?}"))?;

    let mut magic = [0; 8];
    file.read_exact(&mut magic)
        .await
        .context(format!("reading from device {path:?}"))?;
    let result = if magic == INITDATA_MAGIC_NUMBER {
        Some(path)
    } else {
        None
    };
    Ok(result)
}

/// Concurrently locate devices using `tokio::spawn`.
pub async fn locate_device_concurrently(logger: &Logger) -> Result<Option<PathBuf>> {
    // On systems with udev, the device should be available under a by-id symlink.
    match check_initdata_device(logger, INITDATA_PATH_BY_ID.into()).await {
        Ok(_) => return Ok(Some(INITDATA_PATH_BY_ID.into())),
        Err(e) => {
            info!(
                logger,
                "Could not find udev symlink for initdata device: {:?}", e
            )
        }
    }

    // Otherwise, we iterate over all devices and try to find a matching candidate.
    let dev_dir = Path::new("/dev");
    let mut read_dir = tokio::fs::read_dir(dev_dir).await?;

    let mut tasks = FuturesUnordered::new();

    while let Some(entry) = read_dir.next_entry().await? {
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();

        // Just check the file starting with 'vd'
        if !filename_str.starts_with("vd") {
            continue;
        }

        // For each potential device, spawn a new task to check it.
        tasks.push(check_initdata_device(logger, entry.path()));
    }

    let mut errors = Vec::new();
    while let Some(result) = tasks.next().await {
        match result {
            Ok(Some(path)) => return Ok(Some(path)),
            Ok(None) => continue,
            Err(e) => {
                errors.push(e);
                continue;
            }
        }
    }
    if errors.len() > 0 {
        Err(MultiError { errors }.into())
    } else {
        Ok(None)
    }
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

#[derive(Debug)]
struct MultiError {
    pub errors: Vec<anyhow::Error>,
}

impl std::fmt::Display for MultiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for err in &self.errors {
            writeln!(f, "{err}")?;
        }
        Ok(())
    }
}

impl std::error::Error for MultiError {}
