use std::{
    io::{Read as _, Seek as _},
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use slog::{info, Logger};

/// Magic number of initdata device
pub const INITDATA_MAGIC_NUMBER: &[u8] = b"initdata";

const INITDATA_PATH_BY_ID: &str = "/dev/disk/by-id/virtio-initdata";

/// It's designed to be run in a separate tokio task to check if a the potential device is the initdata device.
fn check_initdata_device(logger: &Logger, path: PathBuf) -> Result<Option<PathBuf>> {
    let metadata = std::fs::metadata(&path).context(format!("stat'ing file {path:?}"))?;

    if !metadata.file_type().is_block_device() {
        return Ok(None);
    }

    info!(logger, "Initdata find a potential device: `{path:?}`");

    let mut file = std::fs::File::open(&path).context(format!("opening device {path:?}"))?;

    let mut magic = [0; 8];
    file.read_exact(&mut magic)
        .context(format!("reading from device {path:?}"))?;
    let result = if magic == INITDATA_MAGIC_NUMBER {
        Some(path)
    } else {
        None
    };
    Ok(result)
}

/// Locates the initdata device within /dev.
pub fn locate_device(logger: &Logger) -> Result<Option<PathBuf>> {
    // On systems with udev, the device should be available under a by-id symlink.
    match check_initdata_device(logger, INITDATA_PATH_BY_ID.into()) {
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
    let read_dir = std::fs::read_dir(dev_dir)?;

    let mut errors = Vec::new();
    for entry in read_dir {
        let entry = entry?;

        // Just check the file starting with 'vd'
        if !entry.file_name().to_string_lossy().starts_with("vd") {
            continue;
        }

        match check_initdata_device(logger, entry.path()) {
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
pub fn read_initdata(device_path: &PathBuf) -> Result<Vec<u8>> {
    let initdata_device = std::fs::File::open(device_path)?;
    let mut buf_reader = std::io::BufReader::new(initdata_device);
    // skip the magic number "initdata" with 8 bytes
    buf_reader.seek(std::io::SeekFrom::Start(8))?;

    let mut len_buf = [0u8; 8];
    buf_reader.read_exact(&mut len_buf)?;
    let length = u64::from_le_bytes(len_buf) as usize;

    // Take a limited view of the reader for the compressed data
    let compressed_reader = buf_reader.take(length as u64);
    // Decompress data directly from the reader stream.
    let mut gzip_decoder = flate2::read::GzDecoder::new(compressed_reader);

    let mut initdata = Vec::new();
    let _ = gzip_decoder.read_to_end(&mut initdata)?;
    Ok(initdata)
}

#[derive(Debug)]
struct MultiError {
    pub errors: Vec<anyhow::Error>,
}

impl std::fmt::Display for MultiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for err in &self.errors {
            writeln!(f, "{err:?}")?;
        }
        Ok(())
    }
}

impl std::error::Error for MultiError {}
