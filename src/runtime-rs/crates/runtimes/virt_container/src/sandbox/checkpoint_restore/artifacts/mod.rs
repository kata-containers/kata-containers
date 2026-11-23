// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
use anyhow::{anyhow, Context, Result};
use hypervisor::VmSaveResult;
use ring::digest::{Context as Sha256Context, SHA256};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufReader, Read},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
pub(super) mod rootfs;
pub(super) mod vm;
pub(super) const VM_ARTIFACT_RAW_FORMAT: &str = "raw";
pub(super) fn collect_vm_artifacts(
    state_dir: &Path,
    encoded: &std::collections::HashMap<PathBuf, EncodedArtifact>,
) -> Result<Vec<VmCheckpointArtifact>> {
    let mut entries = fs::read_dir(state_dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    if entries.is_empty() {
        return Err(ArtifactError::Invalid(
            "hypervisor produced no VM checkpoint artifacts".into(),
        )
        .into());
    }

    let mut artifacts = Vec::with_capacity(entries.len());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ArtifactError::Invalid(format!(
                "VM checkpoint artifact {} is not a regular file",
                path.display()
            ))
            .into());
        }

        fs::set_permissions(&path, fs::Permissions::from_mode(0o400))?;

        let file = entry.file_name().to_string_lossy().to_string();
        let artifact = if let Some(encoded) = encoded.get(&path) {
            VmCheckpointArtifact {
                file,
                format: encoded.format.clone(),
                logical_size: encoded.logical_size,
                encoded_size: encoded.encoded_size,
                sha256: encoded.sha256.clone(),
            }
        } else {
            let (size, sha256) = file_integrity(&path)?;
            VmCheckpointArtifact {
                file,
                format: VM_ARTIFACT_RAW_FORMAT.into(),
                logical_size: size,
                encoded_size: size,
                sha256,
            }
        };
        artifacts.push(artifact);
    }

    fs::File::open(state_dir)?.sync_all()?;

    Ok(artifacts)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct VmCheckpointArtifact {
    pub file: String,
    pub format: String,
    pub logical_size: u64,
    pub encoded_size: u64,
    pub sha256: String,
}

pub(super) struct EncodedArtifact {
    pub format: String,
    pub logical_size: u64,
    pub encoded_size: u64,
    pub sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ArtifactError {
    #[error("{0}")]
    Invalid(String),
}

pub(super) fn file_integrity(path: &Path) -> Result<(u64, String)> {
    let file =
        fs::File::open(path).context(format!("open checkpoint artifact {}", path.display()))?;
    let size = file
        .metadata()
        .context(format!("inspect checkpoint artifact {}", path.display()))?
        .len();

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256Context::new(&SHA256);

    // This helper runs from Tokio worker tasks whose stacks are small.
    // Keep the large I/O buffer on the heap to avoid aborting a deeply
    // nested Restore call with a worker-stack overflow.
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .context(format!("read checkpoint artifact {}", path.display()))?;
        if count == 0 {
            break;
        }

        hasher.update(&buffer[..count]);
    }

    let digest = hasher.finish();
    let sha256 = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    Ok((size, sha256))
}

pub(super) fn validate_raw_save(saved: &VmSaveResult) -> Result<()> {
    if saved.transient_memory.is_some()
        || saved.delta_ranges.is_some()
        || saved.memory_base.is_some()
    {
        return Err(anyhow!("raw VM save returned a native memory capture"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_save_rejects_transient_memory() {
        VmSaveResult::default().validate_template().unwrap();
        let saved = VmSaveResult {
            transient_memory: Some(tempfile::tempfile().unwrap()),
            ..Default::default()
        };
        assert!(saved.validate_template().is_err());
    }
}
