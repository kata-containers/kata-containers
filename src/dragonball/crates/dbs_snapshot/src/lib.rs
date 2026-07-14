// Copyright (C) 2026 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Snapshot (checkpoint/restore) contract shared by dragonball components.
//!
//! This crate exists so that every layer of the VMM — vCPUs, guest memory,
//! virtio devices and their managers — can agree on one save/restore
//! vocabulary. The device crates cannot depend on the VMM crate, so the
//! contract lives below both of them here.
//!
//! # Compatibility policy
//!
//! Snapshot state structs are serialized with `serde`. Newly-added fields
//! must use `#[serde(default)]`; fields must never be removed or repurposed
//! (deprecate-don't-delete). A genuinely incompatible change is signalled by
//! bumping the producer's format epoch, so old snapshots are refused loudly
//! instead of being misinterpreted.

#![deny(missing_docs)]

/// A component whose state can be captured into a snapshot and restored from
/// one.
///
/// Implemented across every layer of the VMM so the snapshot subsystem shares
/// one vocabulary. Components differ in what they need beyond the receiver, so
/// the extra inputs are associated types rather than fixed parameters:
/// capturing guest memory needs the memory file, capturing a vCPU needs the
/// MSR index list, and a device needs nothing (`()`).
///
/// Snapshots are a tree: a manager implements this trait by aggregating the
/// state of the components it owns, each of which implements it in turn.
///
/// [`restore_state`](Self::restore_state) applies state to an *existing*
/// instance already re-created from the same configuration the snapshot was
/// taken with; it is not a constructor. Implementations validate the state
/// against the live object and refuse a mismatch rather than silently
/// misinterpreting it.
///
/// The `'a` lifetime lets implementations borrow in their argument types
/// (e.g. `&'a mut File`).
pub trait Persist<'a> {
    /// Serializable state captured from this component.
    type State;
    /// Extra input needed to capture the state.
    type SaveArgs;
    /// Extra input needed to apply a previously captured state.
    type RestoreArgs;
    /// Error reported by both directions.
    type Error;

    /// Capture the state of this component.
    fn save_state(&mut self, args: Self::SaveArgs) -> Result<Self::State, Self::Error>;

    /// Apply a previously captured state to this component.
    fn restore_state(
        &mut self,
        state: &Self::State,
        args: Self::RestoreArgs,
    ) -> Result<(), Self::Error>;
}

/// Errors related to saving/loading a snapshot state file.
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
         regenerate the snapshot/template with this version"
    )]
    EpochMismatch {
        /// Epoch found in the snapshot file.
        found: u64,
        /// Epoch supported by this build.
        supported: u16,
    },
}

/// Refuse a snapshot whose format epoch this build cannot interpret.
pub fn check_epoch(found: u64, supported: u16) -> Result<(), PersistError> {
    if found != u64::from(supported) {
        return Err(PersistError::EpochMismatch { found, supported });
    }
    Ok(())
}
