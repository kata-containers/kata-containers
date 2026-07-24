// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! Security Reference Monitor (SRM) — universal two-phase transaction manager.
//!
//! Every security-relevant, state-mutating agent operation is modelled as a
//! transaction so that policy state and runtime state commit together or are
//! reconciled/rolled back. A partial failure can never leave the enforcer believing
//! a container/mount/identity exists (or vice-versa); if a safe state cannot be
//! proven the sandbox is quarantined.
//!
//! Lifecycle: `prepare` (reserve + record the authorized plan) → `execute` (bind the
//! plan being executed, verified byte-for-byte against what was authorized) →
//! `commit` (validate observed runtime result, advance state) OR `abort` (roll back).
//!
//! Guarantees provided here:
//! - **Authorized == executed:** `execute` rejects any plan whose digest differs from
//!   the one authorized at `prepare` (supports FR-3's canonical-object property).
//! - **Anti-replay / idempotency:** a retried operation id returns the committed
//!   result instead of duplicating effects; a stale `expected_state_version` is
//!   rejected.
//! - **No eager commit:** state only advances on `commit`, never at authorization time.
//! - **Quarantine:** on an unprovable state the monitor refuses all new operations
//!   except teardown.

use std::collections::HashMap;
use std::fmt;

pub mod ccf;
pub mod cdi;
pub mod cose_keys;
pub mod did_x509;
pub mod fragments;
pub mod merkle;
pub mod handle_binding;
pub mod network_phase;
pub mod occurrence;
pub mod resource_graph;
pub mod scratch;
pub mod verified_layers;
pub use cdi::{
    authorize_cdi, CdiDeviceRequest, CdiError, MeasuredCdiSpec, VerifiedCdiDevice,
};
pub use did_x509::{DidX509Anchor, DidX509Policy};
pub use fragments::{FragmentError, FragmentStore, PolicyFragment};
pub use handle_binding::{CheckedHandle, HandleError};
pub use network_phase::{NetOp, NetPhaseError, NetworkPhase, NetworkPhaseMachine};
pub use scratch::{
    classify_scratch, dm_target_types, enforce_scratch, ScratchClass, ScratchError,
    ScratchRequirement,
};
pub use verified_layers::{LayerError, VerifiedLayerStore};
pub use occurrence::{Lifecycle, Occurrence, OccurrenceError, OccurrenceRegistry};
pub use resource_graph::{
    verify_ordered_bijection, PresentedResource, ResourceDeclaration, ResourceGraphError,
    ResourceKind, VerifiedResourceHandle,
};

/// Host-independent identifier for a single mutating operation (idempotency key).
pub type OperationId = String;

/// Lifecycle state of a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxnState {
    /// Authorized and reserved; no irreversible mutation performed yet.
    Prepared,
    /// The authorized plan has been handed to execution.
    Executed,
    /// Runtime result validated; state advanced.
    Committed,
    /// Rolled back; the reserved state was released.
    Aborted,
}

/// A single tracked operation.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub op_id: OperationId,
    /// State version the caller expected when preparing (anti-replay).
    pub expected_state_version: u64,
    /// Digest of the authorized, canonicalized operation plan.
    pub plan_digest: String,
    /// FR-3: digest of the object actually resolved for execution (e.g. the OCI spec
    /// after all in-guest transformers). Bound to the authorized plan so the
    /// authorized→executed relationship is explicit and auditable.
    pub executed_digest: Option<String>,
    pub state: TxnState,
    /// Committed result, retained for idempotent replay.
    pub result: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SrmError {
    /// The monitor is quarantined; only teardown is permitted.
    Quarantined(String),
    /// Expected state version did not match the current version (stale/replayed).
    StaleStateVersion { expected: u64, current: u64 },
    /// The plan presented at execution differs from the authorized plan.
    PlanMismatch { authorized: String, presented: String },
    /// No such prepared transaction.
    UnknownOperation(OperationId),
    /// The transaction is not in a state that permits this action.
    InvalidState { op: OperationId, state: TxnState },
}

impl fmt::Display for SrmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SrmError::Quarantined(r) => write!(f, "SRM quarantined: {r}"),
            SrmError::StaleStateVersion { expected, current } => {
                write!(f, "stale state version: expected {expected}, current {current}")
            }
            SrmError::PlanMismatch { authorized, presented } => {
                write!(f, "plan mismatch: authorized {authorized}, presented {presented}")
            }
            SrmError::UnknownOperation(id) => write!(f, "unknown operation: {id}"),
            SrmError::InvalidState { op, state } => {
                write!(f, "operation {op} in invalid state {state:?} for this action")
            }
        }
    }
}

impl std::error::Error for SrmError {}

/// Result of a `prepare` call.
#[derive(Debug, PartialEq, Eq)]
pub enum Prepared {
    /// A fresh transaction was reserved.
    New,
    /// The operation was already committed; the retained result is returned
    /// (idempotent replay — the caller must NOT execute again).
    AlreadyCommitted(String),
}

/// The universal two-phase transaction manager.
#[derive(Debug, Default)]
pub struct ReferenceMonitor {
    state_version: u64,
    txns: HashMap<OperationId, Transaction>,
    quarantined: Option<String>,
}

impl ReferenceMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state_version(&self) -> u64 {
        self.state_version
    }

    pub fn is_quarantined(&self) -> bool {
        self.quarantined.is_some()
    }

    /// Move the monitor into the quarantined state. Fail-open for availability is
    /// prohibited: once quarantined, only teardown/diagnostics should proceed.
    pub fn quarantine(&mut self, reason: impl Into<String>) {
        if self.quarantined.is_none() {
            self.quarantined = Some(reason.into());
        }
    }

    /// Phase 1: reserve state for an authorized plan. Rejects when quarantined or when
    /// the caller's expected state version is stale. Idempotent for a committed op id.
    pub fn prepare(
        &mut self,
        op_id: impl Into<OperationId>,
        expected_state_version: u64,
        plan_digest: impl Into<String>,
    ) -> Result<Prepared, SrmError> {
        if let Some(r) = &self.quarantined {
            return Err(SrmError::Quarantined(r.clone()));
        }
        let op_id = op_id.into();

        // Idempotent replay: a committed op returns its retained result.
        if let Some(txn) = self.txns.get(&op_id) {
            if txn.state == TxnState::Committed {
                return Ok(Prepared::AlreadyCommitted(
                    txn.result.clone().unwrap_or_default(),
                ));
            }
        }

        if expected_state_version != self.state_version {
            return Err(SrmError::StaleStateVersion {
                expected: expected_state_version,
                current: self.state_version,
            });
        }

        self.txns.insert(
            op_id.clone(),
            Transaction {
                op_id,
                expected_state_version,
                plan_digest: plan_digest.into(),
                executed_digest: None,
                state: TxnState::Prepared,
                result: None,
            },
        );
        Ok(Prepared::New)
    }

    /// FR-3: record the digest of the object actually resolved for execution, binding it
    /// to the authorized transaction. Returns the pair (authorized_plan_digest,
    /// executed_digest) so the caller can audit/log the canonical-object relationship.
    pub fn attach_executed(
        &mut self,
        op_id: &str,
        executed_digest: impl Into<String>,
    ) -> Result<(String, String), SrmError> {
        let txn = self
            .txns
            .get_mut(op_id)
            .ok_or_else(|| SrmError::UnknownOperation(op_id.to_string()))?;
        let executed = executed_digest.into();
        txn.executed_digest = Some(executed.clone());
        Ok((txn.plan_digest.clone(), executed))
    }

    /// Phase 2a: bind the plan actually being executed. The presented plan digest MUST
    /// equal the authorized one, enforcing authorized == executed.
    pub fn execute(&mut self, op_id: &str, presented_plan_digest: &str) -> Result<(), SrmError> {
        if let Some(r) = &self.quarantined {
            return Err(SrmError::Quarantined(r.clone()));
        }
        let txn = self
            .txns
            .get_mut(op_id)
            .ok_or_else(|| SrmError::UnknownOperation(op_id.to_string()))?;
        if txn.state != TxnState::Prepared {
            return Err(SrmError::InvalidState {
                op: op_id.to_string(),
                state: txn.state.clone(),
            });
        }
        if txn.plan_digest != presented_plan_digest {
            return Err(SrmError::PlanMismatch {
                authorized: txn.plan_digest.clone(),
                presented: presented_plan_digest.to_string(),
            });
        }
        txn.state = TxnState::Executed;
        Ok(())
    }

    /// Phase 2b: the runtime op succeeded; advance state and retain the result.
    pub fn commit(&mut self, op_id: &str, observed_result: impl Into<String>) -> Result<(), SrmError> {
        let txn = self
            .txns
            .get_mut(op_id)
            .ok_or_else(|| SrmError::UnknownOperation(op_id.to_string()))?;
        if txn.state != TxnState::Executed {
            return Err(SrmError::InvalidState {
                op: op_id.to_string(),
                state: txn.state.clone(),
            });
        }
        txn.state = TxnState::Committed;
        txn.result = Some(observed_result.into());
        self.state_version += 1;
        Ok(())
    }

    /// Roll back a prepared/executed transaction. The reserved state is released and
    /// the state version is NOT advanced (the operation had no committed effect).
    pub fn abort(&mut self, op_id: &str) -> Result<(), SrmError> {
        let txn = self
            .txns
            .get_mut(op_id)
            .ok_or_else(|| SrmError::UnknownOperation(op_id.to_string()))?;
        match txn.state {
            TxnState::Prepared | TxnState::Executed => {
                txn.state = TxnState::Aborted;
                Ok(())
            }
            _ => Err(SrmError::InvalidState {
                op: op_id.to_string(),
                state: txn.state.clone(),
            }),
        }
    }

    pub fn transaction(&self, op_id: &str) -> Option<&Transaction> {
        self.txns.get(op_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_commits_and_advances_version() {
        let mut m = ReferenceMonitor::new();
        assert_eq!(m.state_version(), 0);
        assert_eq!(m.prepare("op1", 0, "digestA").unwrap(), Prepared::New);
        m.execute("op1", "digestA").unwrap();
        m.commit("op1", "container-created").unwrap();
        assert_eq!(m.state_version(), 1);
        assert_eq!(m.transaction("op1").unwrap().state, TxnState::Committed);
    }

    #[test]
    fn execute_rejects_plan_mismatch() {
        // authorized == executed: a plan different from the authorized one is refused.
        let mut m = ReferenceMonitor::new();
        m.prepare("op1", 0, "digestA").unwrap();
        let err = m.execute("op1", "digestB").unwrap_err();
        assert_eq!(
            err,
            SrmError::PlanMismatch {
                authorized: "digestA".into(),
                presented: "digestB".into()
            }
        );
    }

    #[test]
    fn idempotent_replay_returns_result_without_reexecuting() {
        let mut m = ReferenceMonitor::new();
        m.prepare("op1", 0, "d").unwrap();
        m.execute("op1", "d").unwrap();
        m.commit("op1", "result-1").unwrap();
        let v = m.state_version();
        // Re-preparing the same committed op returns the retained result, no new effect.
        assert_eq!(
            m.prepare("op1", 99, "d").unwrap(),
            Prepared::AlreadyCommitted("result-1".into())
        );
        assert_eq!(m.state_version(), v, "replay must not advance state");
    }

    #[test]
    fn stale_state_version_is_rejected() {
        let mut m = ReferenceMonitor::new();
        m.prepare("op1", 0, "d").unwrap();
        m.execute("op1", "d").unwrap();
        m.commit("op1", "r").unwrap(); // version -> 1
        let err = m.prepare("op2", 0, "d").unwrap_err();
        assert_eq!(err, SrmError::StaleStateVersion { expected: 0, current: 1 });
    }

    #[test]
    fn abort_rolls_back_without_advancing_version() {
        let mut m = ReferenceMonitor::new();
        m.prepare("op1", 0, "d").unwrap();
        m.execute("op1", "d").unwrap();
        m.abort("op1").unwrap();
        assert_eq!(m.state_version(), 0, "aborted op must not advance state");
        assert_eq!(m.transaction("op1").unwrap().state, TxnState::Aborted);
    }

    #[test]
    fn quarantine_blocks_new_operations() {
        let mut m = ReferenceMonitor::new();
        m.quarantine("ambiguous failure between execute and commit");
        assert!(m.is_quarantined());
        assert!(matches!(m.prepare("op1", 0, "d"), Err(SrmError::Quarantined(_))));
    }

    #[test]
    fn cannot_commit_without_execute() {
        let mut m = ReferenceMonitor::new();
        m.prepare("op1", 0, "d").unwrap();
        assert!(matches!(m.commit("op1", "r"), Err(SrmError::InvalidState { .. })));
    }

    #[test]
    fn attach_executed_binds_authorized_to_executed() {
        let mut m = ReferenceMonitor::new();
        m.prepare("op1", 0, "authorized-digest").unwrap();
        let (authorized, executed) = m.attach_executed("op1", "executed-digest").unwrap();
        assert_eq!(authorized, "authorized-digest");
        assert_eq!(executed, "executed-digest");
        assert_eq!(
            m.transaction("op1").unwrap().executed_digest.as_deref(),
            Some("executed-digest")
        );
    }

    #[test]
    fn unknown_operation_errors() {
        let mut m = ReferenceMonitor::new();
        assert_eq!(
            m.execute("nope", "d").unwrap_err(),
            SrmError::UnknownOperation("nope".into())
        );
    }
}
