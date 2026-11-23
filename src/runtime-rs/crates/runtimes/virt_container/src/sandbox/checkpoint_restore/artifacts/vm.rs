// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
use super::{collect_vm_artifacts, ArtifactError, VmCheckpointArtifact};
use anyhow::{Context, Result};
use common::error::Error as CommonError;
use hypervisor::VmSaveResult;
use std::{collections::HashMap, path::PathBuf};
pub(in crate::sandbox::checkpoint_restore) fn map_artifact_error(
    err: anyhow::Error,
) -> anyhow::Error {
    match err.downcast_ref::<ArtifactError>() {
        Some(ArtifactError::Invalid(message)) => {
            let message = message.clone();
            err.context(CommonError::InvalidSandboxOperation(message))
        }
        None => err,
    }
}

pub(in crate::sandbox::checkpoint_restore) async fn finalize_checkpoint_artifacts(
    _hypervisor_name: &str,
    saved: VmSaveResult,
    state_dir: PathBuf,
) -> Result<Vec<VmCheckpointArtifact>> {
    tokio::task::spawn_blocking(move || {
        super::validate_raw_save(&saved)?;

        collect_vm_artifacts(&state_dir, &HashMap::new())
    })
    .await
    .context("checkpoint VM artifact worker failed")?
    .map_err(map_artifact_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn raw_vm_artifacts_publish_for_qemu_and_clh() {
        for backend in [hypervisor::HYPERVISOR_QEMU, hypervisor::HYPERVISOR_NAME_CH] {
            let directory = tempfile::tempdir().unwrap();
            let checkpoint_root = directory.path().join("checkpoint");
            let state_dir = checkpoint_root.join("vm");
            fs::create_dir_all(&state_dir).unwrap();
            fs::write(state_dir.join("state"), b"native VM state").unwrap();
            if backend == hypervisor::HYPERVISOR_QEMU {
                fs::write(state_dir.join("memory-topology.json"), b"{}").unwrap();
            }
            let artifacts =
                finalize_checkpoint_artifacts(backend, VmSaveResult::default(), state_dir.clone())
                    .await
                    .unwrap();
            let state = artifacts
                .iter()
                .find(|artifact| artifact.file == "state")
                .unwrap();
            assert_eq!(state.logical_size, b"native VM state".len() as u64);
            assert_eq!(state.encoded_size, state.logical_size);
            assert_eq!(state.sha256.len(), 64);
        }
    }

    #[test]
    fn native_errors_keep_sandbox_status_categories() {
        let invalid = map_artifact_error(
            anyhow::Error::from(ArtifactError::Invalid("bad digest".into()))
                .context("verify native memory"),
        );
        assert!(matches!(
            invalid.downcast_ref::<CommonError>(),
            Some(CommonError::InvalidSandboxOperation(_))
        ));
        let io =
            map_artifact_error(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into());
        assert!(io.downcast_ref::<CommonError>().is_none());
        assert!(io.downcast_ref::<std::io::Error>().is_some());
    }
}
