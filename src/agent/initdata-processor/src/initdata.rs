use std::{
    io::{ErrorKind, Read as _, Seek as _},
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use slog::{info, Logger};

/// Magic number of initdata device
pub const INITDATA_MAGIC_NUMBER: &[u8] = b"initdata";

const INITDATA_PATH_BY_ID: &str = "/dev/disk/by-id/virtio-initdata";

fn is_initdata_device(path: &Path) -> Result<bool> {
    let mut file = std::fs::File::open(path).context(format!("opening device {path:?}"))?;

    let mut magic = [0; 8];
    match file.read_exact(&mut magic) {
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
            // Device is shorter than magic, thus it's not an initdata device.
            Ok(false)
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // Most likely, we're looking for a hard-coded path that's not present in this VM. If
            // it's not there, it can't be an initdata device.
            Ok(false)
        }
        Err(e) => Err(e).context(format!("reading from device {path:?}")),
        Ok(()) => Ok(magic == INITDATA_MAGIC_NUMBER),
    }
}

/// Locates the initdata device within /dev.
pub fn locate_device(path: &Path, logger: &Logger) -> Result<Option<PathBuf>> {
    // On systems with udev, the device should be available under a by-id symlink.
    let mut device_candidates = vec![INITDATA_PATH_BY_ID.into()];

    let mut errors = Vec::new();
    // Otherwise, we iterate over all devices and try to find a matching candidate.
    for entry in std::fs::read_dir(path).context(format!("read_dir({path:?})"))? {
        let entry = entry?;

        // Just check the file starting with 'vd' (virtio-blk) or 'sd' (virtio-scsi)
        let file_name_osstr = entry.file_name();
        let file_name = file_name_osstr.to_string_lossy();
        if !file_name.starts_with("vd") && !file_name.starts_with("sd") {
            continue;
        }

        match std::fs::metadata(entry.path()).context(format!("stat'ing file {:?}", entry.path())) {
            Ok(metadata) => {
                if metadata.file_type().is_block_device() {
                    info!(
                        logger,
                        "Found a potential initdata device: {:?}",
                        entry.path()
                    );
                    device_candidates.push(entry.path());
                }
            }
            Err(e) => {
                errors.push(e);
                continue;
            }
        }
    }

    for path in device_candidates {
        match is_initdata_device(&path) {
            Ok(true) => return Ok(Some(path)),
            Ok(false) => continue,
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
pub fn read_initdata(device_path: &Path) -> Result<Vec<u8>> {
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

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use flate2::{write::GzEncoder, Compression};

    use crate::initdata::{read_initdata, INITDATA_MAGIC_NUMBER};

    use super::is_initdata_device;

    #[test]
    fn test_is_initdata_device() {
        let dir = tempfile::tempdir().expect("should be able to create a temp dir");

        let result = is_initdata_device(&dir.path().join("does-not-exist"));
        assert!(result.is_err());

        let file_path = dir.path().join("not-initdata");
        std::fs::write(&file_path, "hello").expect("should be able to write a temp file");
        let is_initdata =
            is_initdata_device(&file_path).expect("reading this file should not fail");
        assert!(!is_initdata);

        let file_path = dir.path().join("initdata");
        std::fs::write(&file_path, b"initdata").expect("should be able to write a temp file");
        let is_initdata =
            is_initdata_device(&file_path).expect("reading this file should not fail");
        assert!(is_initdata);
    }

    #[test]
    fn test_read_initdata() {
        let dir = tempfile::tempdir().expect("should be able to create a temp dir");

        let result = read_initdata(&dir.path().join("does-not-exist"));
        assert!(result.is_err());

        let file_path = dir.path().join("not-initdata");
        std::fs::write(&file_path, "hello").expect("should be able to write a temp file");
        let result = read_initdata(&file_path);
        assert!(result.is_err());

        let expected_content = br#"
        algorithm = "sha384"
        version = "0.1.0"
        "#
        .to_vec();

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(expected_content.as_slice())
            .expect("should be able to write into memory");
        encoder
            .try_finish()
            .expect("should be able to flush to memory");

        let mut raw_content = Vec::new();
        raw_content.extend_from_slice(INITDATA_MAGIC_NUMBER);
        raw_content.extend_from_slice(&(encoder.get_ref().len() as u64).to_le_bytes());
        raw_content.extend(encoder.get_ref());
        // Add some garbage.
        raw_content.extend_from_slice(&[0u8; 256]);

        let file_path = dir.path().join("initdata.toml.gz");
        std::fs::write(&file_path, raw_content).expect("should be able to write a temp file");

        let content = read_initdata(&file_path).expect("reading valid initdata should succeed");
        assert_eq!(expected_content, content);
    }
}
