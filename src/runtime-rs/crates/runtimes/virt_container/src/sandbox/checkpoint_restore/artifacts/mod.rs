// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

mod copy;
pub(super) mod rootfs;
#[cfg(all(
    feature = "dragonball",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub(super) mod sparse_memory;
pub(super) mod vm;
mod worker;

pub(super) use copy::{clone_verified_artifacts, RestoreArtifact};
pub(super) use rootfs::IMMUTABLE_ROOTFS_OPTION;
pub(super) use worker::{
    run_artifact_worker, RESTORE_ARTIFACT_CONCURRENCY, RESTORE_ARTIFACT_SEMAPHORE,
};

#[derive(Debug, thiserror::Error)]
pub(super) enum ArtifactError {
    #[error("{0}")]
    Invalid(String),
}
