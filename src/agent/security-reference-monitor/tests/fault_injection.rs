// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! FR-15 — fault injection and property/fuzz tests over the SRM lifecycle.
//!
//! These tests exercise the reference monitor's state machine to establish the
//! equivalence-claim safety property: *no reachable state of the monitor is permissive
//! or phantom*. Concretely:
//!
//!  - Injecting a fault at any phase (prepare / execute / runtime-before-commit) and
//!    reconciling the way the agent does (abort, or quarantine if abort is impossible)
//!    never leaves an operation Committed and never advances the state version for a
//!    failed operation.
//!  - Committed and Aborted are terminal and mutually exclusive: a committed operation
//!    cannot be aborted and an aborted one cannot be committed (no undo-to-phantom).
//!  - Over long randomized sequences of operations, the state version always equals the
//!    number of committed operations, every committed operation passed through Executed
//!    (authorized == executed), and quarantine is sticky and blocks new authorizations.

use kata_security_reference_monitor::{ReferenceMonitor, SrmError, TxnState};

/// The phase at which a simulated operation's fault is injected.
#[derive(Debug, Clone, Copy)]
enum FaultPoint {
    /// Authorization itself fails (e.g. stale version / quarantined).
    Prepare,
    /// The presented plan does not match the authorized one.
    Execute,
    /// Authorization and binding succeed but the runtime operation fails before commit.
    RuntimeBeforeCommit,
}

/// Simulate one operation with a fault at `fp`, reconciling like the agent does, and
/// assert the monitor is left in a safe (non-phantom, non-permissive) state.
fn run_with_fault(fp: FaultPoint) {
    let mut m = ReferenceMonitor::new();
    let version_before = m.state_version();
    let op = "op-fault";
    let digest = "authorized-digest";

    match fp {
        FaultPoint::Prepare => {
            // A stale expected version makes authorization fail: nothing is reserved.
            let err = m.prepare(op, version_before + 99, digest).unwrap_err();
            assert!(matches!(err, SrmError::StaleStateVersion { .. }));
            assert!(m.transaction(op).is_none(), "no txn on failed prepare");
        }
        FaultPoint::Execute => {
            m.prepare(op, version_before, digest).unwrap();
            // The plan presented at execution differs from the authorized one.
            let err = m.execute(op, "tampered-digest").unwrap_err();
            assert!(matches!(err, SrmError::PlanMismatch { .. }));
            // Reconcile: abort the reserved transaction.
            m.abort(op).unwrap();
            assert_eq!(m.transaction(op).unwrap().state, TxnState::Aborted);
        }
        FaultPoint::RuntimeBeforeCommit => {
            m.prepare(op, version_before, digest).unwrap();
            m.execute(op, digest).unwrap();
            // The runtime operation fails; the caller aborts instead of committing.
            m.abort(op).unwrap();
            assert_eq!(m.transaction(op).unwrap().state, TxnState::Aborted);
        }
    }

    // Safety: the operation is never Committed and the state version never advanced.
    if let Some(txn) = m.transaction(op) {
        assert_ne!(
            txn.state,
            TxnState::Committed,
            "a faulted operation must never be Committed"
        );
    }
    assert_eq!(
        m.state_version(),
        version_before,
        "state version must not advance for a faulted operation"
    );
}

#[test]
fn fault_at_every_phase_leaves_safe_state() {
    run_with_fault(FaultPoint::Prepare);
    run_with_fault(FaultPoint::Execute);
    run_with_fault(FaultPoint::RuntimeBeforeCommit);
}

#[test]
fn abort_failure_escalates_to_quarantine() {
    // Model the agent's reconciliation: if a committed op is (incorrectly) asked to abort,
    // abort refuses; the agent then quarantines rather than leaving an unprovable state.
    let mut m = ReferenceMonitor::new();
    m.prepare("op", 0, "d").unwrap();
    m.execute("op", "d").unwrap();
    m.commit("op", "done").unwrap();

    // A late/erroneous abort of a committed op must fail (no undo-to-phantom).
    assert!(matches!(
        m.abort("op").unwrap_err(),
        SrmError::InvalidState { .. }
    ));
    // The agent escalates to quarantine on an unprovable reconciliation.
    m.quarantine("abort of committed op attempted");
    assert!(m.is_quarantined());
    // Quarantine blocks new authorizations.
    let v = m.state_version();
    assert!(matches!(
        m.prepare("op2", v, "d2").unwrap_err(),
        SrmError::Quarantined(_)
    ));
}

#[test]
fn committed_and_aborted_are_terminal_and_exclusive() {
    // Committed cannot be aborted.
    let mut m = ReferenceMonitor::new();
    m.prepare("c", 0, "d").unwrap();
    m.execute("c", "d").unwrap();
    m.commit("c", "r").unwrap();
    assert!(m.abort("c").is_err());

    // Aborted cannot be committed.
    let mut n = ReferenceMonitor::new();
    n.prepare("a", 0, "d").unwrap();
    n.abort("a").unwrap();
    assert!(n.commit("a", "r").is_err());
}

/// A tiny dependency-free deterministic PRNG (SplitMix64) for reproducible fuzzing.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Property/fuzz: over long randomized operation sequences, the monitor's core safety
/// invariants always hold. Runs many seeds for coverage.
#[test]
fn randomized_sequences_preserve_invariants() {
    for seed in 0..200u64 {
        let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D).wrapping_add(1));
        let mut m = ReferenceMonitor::new();

        // Shadow count of committed operations.
        let mut committed: u64 = 0;
        let ids = ["a", "b", "c", "d"];

        for _ in 0..300 {
            let id = ids[rng.below(ids.len() as u64) as usize];
            let action = rng.below(5);
            let ver = m.state_version();
            match action {
                0 => {
                    // prepare with the current (fresh) version
                    let _ = m.prepare(id, ver, "digest");
                }
                1 => {
                    // execute with the correct digest
                    let _ = m.execute(id, "digest");
                }
                2 => {
                    // execute with a wrong digest (must never succeed)
                    assert!(
                        m.execute(id, "WRONG").is_err() || m.transaction(id).is_none(),
                        "execute with mismatched digest must never succeed"
                    );
                }
                3 => {
                    // commit; if it succeeds the op must have been Executed first
                    let was_executed = m
                        .transaction(id)
                        .map(|t| t.state == TxnState::Executed)
                        .unwrap_or(false);
                    if m.commit(id, "result").is_ok() {
                        assert!(
                            was_executed,
                            "commit succeeded on an operation that was not Executed"
                        );
                        committed += 1;
                    }
                }
                _ => {
                    let _ = m.abort(id);
                }
            }

            // Occasionally quarantine and then assert it is sticky and blocks new prepares.
            if rng.below(50) == 0 {
                m.quarantine("fuzz");
            }

            // Global invariants after every step:
            // 1. state version equals the number of committed operations.
            assert_eq!(
                m.state_version(),
                committed,
                "seed {seed}: state version diverged from committed count"
            );
            // 2. quarantine is sticky and blocks new authorization.
            if m.is_quarantined() {
                let v = m.state_version();
                assert!(matches!(
                    m.prepare("z", v, "d").unwrap_err(),
                    SrmError::Quarantined(_)
                ));
            }
            // 3. Committed retains a result; Aborted never does (states are exclusive).
            for id in ids {
                if let Some(t) = m.transaction(id) {
                    if t.state == TxnState::Committed {
                        assert!(t.result.is_some());
                    }
                    if t.state == TxnState::Aborted {
                        assert!(t.result.is_none());
                    }
                }
            }
        }
    }
}
