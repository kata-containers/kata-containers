// Copyright (C) 2026 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Checkpoint/restore (snapshot) support for Dragonball.
//!
//! This module provides the [`Persist`] trait implemented by stateful VMM
//! components, plus the top-level [`MicrovmState`] aggregating the states of
//! all components and helpers to save/load it as a JSON state file.
//!
//! The snapshot consists of two files:
//! - a *state file* (JSON, produced from [`MicrovmState`]) holding vCPU,
//!   device and VM metadata state, and
//! - a *memory file* holding the guest RAM contents (managed by the address
//!   space manager, not this module).
//!
//! # Compatibility policy
//!
//! State structs are serialized with `serde_json`. Snapshots are expected to
//! be produced and consumed by the same Dragonball version (regenerate the
//! template whenever Dragonball is updated). Newly-added fields must use
//! `#[serde(default)]`; fields must never be removed or repurposed
//! (deprecate-don't-delete). [`FORMAT_EPOCH`] is bumped only on a genuinely
//! incompatible change, causing old snapshots to be refused loudly instead of
//! being misinterpreted.

use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

use serde_derive::{Deserialize, Serialize};

use crate::address_space_manager::GuestMemoryState;
use crate::vcpu::VcpuState;

/// Current snapshot format epoch.
///
/// Bump this only for a deliberately incompatible change to the persisted
/// state format that append-only evolution cannot absorb. On restore, an
/// epoch mismatch causes the snapshot to be refused with a clear error.
pub const FORMAT_EPOCH: u16 = 1;

/// A component that can serialize its runtime state (`save`) and be
/// reconstructed from that state (`restore`).
///
/// - `State` is a plain-old-data struct holding the serializable state,
///   deriving `Serialize`/`Deserialize`.
/// - `ConstructorArgs` carries the live, non-serializable dependencies needed
///   to rebuild `Self` (fds, guest memory handles, managers, ...). They are
///   re-created by the caller and injected on restore.
pub trait Persist<'a>
where
    Self: Sized,
{
    /// Serializable state of the component.
    type State;
    /// Live dependencies needed to reconstruct the component.
    type ConstructorArgs;
    /// Error type returned by save/restore.
    type Error;

    /// Capture the current state of the component.
    fn save(&self) -> std::result::Result<Self::State, Self::Error>;

    /// Rebuild the component from a previously saved state plus live
    /// constructor arguments.
    fn restore(
        constructor_args: Self::ConstructorArgs,
        state: &Self::State,
    ) -> std::result::Result<Self, Self::Error>;
}

/// Errors related to saving/loading a microVM snapshot.
#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    /// Snapshot file I/O failure.
    #[error("snapshot I/O error")]
    Io(#[from] std::io::Error),

    /// Snapshot (de)serialization failure.
    #[error("snapshot serialization error")]
    Json(#[from] serde_json::Error),

    /// The snapshot was produced with an incompatible format epoch.
    #[error(
        "incompatible snapshot: found format epoch {found}, supported {supported}; \
         regenerate the snapshot/template with this Dragonball version"
    )]
    EpochMismatch {
        /// Epoch found in the snapshot file.
        found: u64,
        /// Epoch supported by this Dragonball build.
        supported: u16,
    },
}

/// Header identifying a snapshot state file.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SnapshotHeader {
    /// Snapshot format epoch, checked against [`FORMAT_EPOCH`] on load.
    pub format_epoch: u16,
    /// Version of the Dragonball crate that produced the snapshot.
    /// Diagnostics only; not used for compatibility decisions.
    #[serde(default)]
    pub producer_version: String,
}

impl Default for SnapshotHeader {
    fn default() -> Self {
        SnapshotHeader {
            format_epoch: FORMAT_EPOCH,
            producer_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Aggregated state of a microVM.
///
/// Components are added progressively: a component whose `Persist` support is
/// not implemented yet simply keeps its `Default` value in the snapshot and
/// is ignored on restore.
#[derive(Default, Deserialize, Serialize)]
pub struct MicrovmState {
    /// Snapshot header.
    #[serde(default)]
    pub header: SnapshotHeader,
    /// Per-vCPU state, ordered by vCPU id.
    #[serde(default)]
    pub vcpu_states: Vec<VcpuState>,
    /// Guest RAM contents state, present once memory has been saved.
    #[serde(default)]
    pub memory_state: Option<GuestMemoryState>,
}

impl MicrovmState {
    /// Serialize the state to a JSON file at `path`.
    pub fn save_to_file(&self, path: &Path) -> std::result::Result<(), PersistError> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, self)?;
        // Flush explicitly: BufWriter's drop swallows I/O errors (e.g.
        // ENOSPC), which would report a truncated state file as success.
        writer.flush()?;
        Ok(())
    }

    /// Load a state previously written by [`MicrovmState::save_to_file`].
    ///
    /// The format epoch is validated *before* deserializing the full state so
    /// that an incompatible snapshot fails with [`PersistError::EpochMismatch`]
    /// instead of an obscure deserialization error.
    pub fn load_from_file(path: &Path) -> std::result::Result<Self, PersistError> {
        let file = File::open(path)?;
        let value: serde_json::Value = serde_json::from_reader(BufReader::new(file))?;
        let found = value
            .get("header")
            .and_then(|h| h.get("format_epoch"))
            .and_then(|e| e.as_u64())
            .unwrap_or(0);
        if found != u64::from(FORMAT_EPOCH) {
            return Err(PersistError::EpochMismatch {
                found,
                supported: FORMAT_EPOCH,
            });
        }
        Ok(serde_json::from_value(value)?)
    }
}

#[cfg(test)]
mod tests {
    use vmm_sys_util::tempfile::TempFile;

    use super::*;

    #[test]
    fn test_microvm_state_json_roundtrip() {
        let state = MicrovmState::default();
        let file = TempFile::new().unwrap();
        state.save_to_file(file.as_path()).unwrap();

        let loaded = MicrovmState::load_from_file(file.as_path()).unwrap();
        assert_eq!(loaded.header.format_epoch, FORMAT_EPOCH);
        assert_eq!(loaded.header.producer_version, env!("CARGO_PKG_VERSION"));
        assert!(loaded.vcpu_states.is_empty());
    }

    #[test]
    fn test_microvm_state_epoch_mismatch() {
        let file = TempFile::new().unwrap();
        let json = serde_json::json!({
            "header": { "format_epoch": FORMAT_EPOCH + 1 },
            "vcpu_states": [],
        });
        serde_json::to_writer(File::create(file.as_path()).unwrap(), &json).unwrap();

        match MicrovmState::load_from_file(file.as_path()) {
            Err(PersistError::EpochMismatch { found, supported }) => {
                assert_eq!(found, u64::from(FORMAT_EPOCH + 1));
                assert_eq!(supported, FORMAT_EPOCH);
            }
            other => panic!("expected EpochMismatch, got {:?}", other.map(|_| ())),
        }
    }

    #[test]
    fn test_microvm_state_epoch_not_truncated() {
        // An epoch congruent to FORMAT_EPOCH mod 2^16 must still be refused.
        let file = TempFile::new().unwrap();
        let epoch = u64::from(FORMAT_EPOCH) + (1 << 16);
        let json = serde_json::json!({
            "header": { "format_epoch": epoch },
            "vcpu_states": [],
        });
        serde_json::to_writer(File::create(file.as_path()).unwrap(), &json).unwrap();

        assert!(matches!(
            MicrovmState::load_from_file(file.as_path()),
            Err(PersistError::EpochMismatch { found, .. }) if found == epoch
        ));
    }

    #[test]
    fn test_microvm_state_missing_header_refused() {
        // A state file without a header (e.g. produced by a pre-epoch build)
        // must be refused, not silently defaulted.
        let file = TempFile::new().unwrap();
        serde_json::to_writer(
            File::create(file.as_path()).unwrap(),
            &serde_json::json!({ "vcpu_states": [] }),
        )
        .unwrap();

        assert!(matches!(
            MicrovmState::load_from_file(file.as_path()),
            Err(PersistError::EpochMismatch { found: 0, .. })
        ));
    }
}
