// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! FR-9 — container *occurrence* tracking.
//!
//! The host-supplied `container_id` is an untrusted alias: the host chooses it and
//! can reuse, forge or replay it. The trusted enforcer therefore mints its own
//! *occurrence handle* for every container it creates and drives that occurrence
//! through an explicit lifecycle state machine. All lifecycle-mutating RPCs
//! (Start/Exec/Signal/Pause/Resume/Remove) are gated on the occurrence state, so a
//! host cannot start a container that was never created, exec into a container that
//! is not running, or replay a stale/removed occurrence.
//!
//! Two abuses from the analysis are closed here:
//!  - **Illegal lifecycle transitions** (start-before-create, exec-on-unknown-id,
//!    signal a non-running occurrence, operate on a removed occurrence).
//!  - **Cardinality** (optional, opt-in per declaration): a policy declaration that
//!    is meant to admit exactly N containers cannot be satisfied by more than N
//!    distinct occurrences (Attack #15 — two distinct ids satisfying one
//!    declaration).
//!
//! Replay of a *previous* generation of an alias (host reuses a container_id after
//! the occurrence was removed) is rejected: a fresh create re-mints the occurrence
//! with a new, monotonically increasing generation; operations that carry an older
//! generation are refused.

use std::collections::HashMap;
use std::fmt;

/// Lifecycle state of a container occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    /// Created (bundle/rootfs prepared) but the init process has not been started.
    Created,
    /// The init process has been started and is running.
    Running,
    /// The init process has exited / been stopped but the occurrence is not yet removed.
    Stopped,
    /// The occurrence has been torn down; its alias may not be operated on again.
    Removed,
}

impl fmt::Display for Lifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Lifecycle::Created => "created",
            Lifecycle::Running => "running",
            Lifecycle::Stopped => "stopped",
            Lifecycle::Removed => "removed",
        };
        f.write_str(s)
    }
}

/// A single tracked container occurrence.
#[derive(Debug, Clone)]
pub struct Occurrence {
    /// Enforcer-minted, host-independent handle for this occurrence.
    pub handle: String,
    /// The host-chosen alias (container_id) currently bound to this occurrence.
    pub alias: String,
    /// Monotonic generation for this alias (bumped every time the alias is (re)created).
    pub generation: u64,
    /// Current lifecycle state.
    pub state: Lifecycle,
    /// Policy declaration index this occurrence was admitted against (FR-4A binding /
    /// cardinality accounting). `None` if declarations are not indexed.
    pub declaration_index: Option<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum OccurrenceError {
    /// No occurrence is bound to this alias (e.g. start/exec before create).
    UnknownAlias(String),
    /// The alias exists but the requested transition is illegal from its current state.
    IllegalTransition {
        alias: String,
        from: Lifecycle,
        action: &'static str,
    },
    /// An occurrence already exists for this alias and has not been removed.
    AliasInUse(String),
    /// Admitting this occurrence would exceed the declaration's allowed cardinality.
    CardinalityExceeded {
        declaration_index: usize,
        allowed: usize,
    },
    /// The operation referenced a stale generation of the alias (replay of a prior
    /// occurrence after the alias was recreated).
    StaleGeneration {
        alias: String,
        presented: u64,
        current: u64,
    },
}

impl fmt::Display for OccurrenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OccurrenceError::UnknownAlias(a) => write!(f, "no occurrence for alias {a}"),
            OccurrenceError::IllegalTransition { alias, from, action } => write!(
                f,
                "illegal lifecycle transition: {action} on alias {alias} while {from}"
            ),
            OccurrenceError::AliasInUse(a) => {
                write!(f, "alias {a} is already bound to a live occurrence")
            }
            OccurrenceError::CardinalityExceeded {
                declaration_index,
                allowed,
            } => write!(
                f,
                "declaration {declaration_index} admits at most {allowed} occurrence(s)"
            ),
            OccurrenceError::StaleGeneration {
                alias,
                presented,
                current,
            } => write!(
                f,
                "stale generation for alias {alias}: presented {presented}, current {current}"
            ),
        }
    }
}

impl std::error::Error for OccurrenceError {}

/// Registry of container occurrences and their lifecycle states.
#[derive(Debug, Default)]
pub struct OccurrenceRegistry {
    /// Live and stopped occurrences, keyed by host alias.
    by_alias: HashMap<String, Occurrence>,
    /// Last generation seen for every alias ever created (retained after removal so a
    /// replayed alias cannot re-use an old generation number).
    generations: HashMap<String, u64>,
    /// Count of live (non-removed) occurrences admitted per declaration index.
    declaration_counts: HashMap<usize, usize>,
    /// Monotonic counter used to mint unique occurrence handles.
    next_handle: u64,
}

impl OccurrenceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn mint_handle(&mut self) -> String {
        self.next_handle += 1;
        format!("occ-{}", self.next_handle)
    }

    /// Create (register) a new occurrence for `alias`.
    ///
    /// * Rejects a duplicate alias that is still bound to a live (non-removed) occurrence.
    /// * If `declaration_index` and `max_cardinality` are supplied, rejects the create
    ///   when admitting it would exceed the declaration's allowed count.
    ///
    /// Returns the minted occurrence handle.
    pub fn create(
        &mut self,
        alias: impl Into<String>,
        declaration_index: Option<usize>,
        max_cardinality: Option<usize>,
    ) -> Result<String, OccurrenceError> {
        let alias = alias.into();

        if let Some(existing) = self.by_alias.get(&alias) {
            if existing.state != Lifecycle::Removed {
                return Err(OccurrenceError::AliasInUse(alias));
            }
        }

        if let (Some(idx), Some(max)) = (declaration_index, max_cardinality) {
            let live = *self.declaration_counts.get(&idx).unwrap_or(&0);
            if live >= max {
                return Err(OccurrenceError::CardinalityExceeded {
                    declaration_index: idx,
                    allowed: max,
                });
            }
        }

        let generation = self.generations.get(&alias).map_or(0, |g| g + 1);
        self.generations.insert(alias.clone(), generation);

        let handle = self.mint_handle();
        if let Some(idx) = declaration_index {
            *self.declaration_counts.entry(idx).or_insert(0) += 1;
        }

        self.by_alias.insert(
            alias.clone(),
            Occurrence {
                handle: handle.clone(),
                alias,
                generation,
                state: Lifecycle::Created,
                declaration_index,
            },
        );
        Ok(handle)
    }

    fn get_live(&self, alias: &str) -> Result<&Occurrence, OccurrenceError> {
        match self.by_alias.get(alias) {
            Some(o) if o.state != Lifecycle::Removed => Ok(o),
            _ => Err(OccurrenceError::UnknownAlias(alias.to_string())),
        }
    }

    /// Current lifecycle state for an alias, if it has a live occurrence.
    pub fn state(&self, alias: &str) -> Option<Lifecycle> {
        self.get_live(alias).ok().map(|o| o.state)
    }

    /// Current generation for an alias, if it has a live occurrence.
    pub fn generation(&self, alias: &str) -> Option<u64> {
        self.get_live(alias).ok().map(|o| o.generation)
    }

    /// Occurrence handle bound to an alias, if it has a live occurrence.
    pub fn handle(&self, alias: &str) -> Option<&str> {
        self.get_live(alias).ok().map(|o| o.handle.as_str())
    }

    /// Assert that `alias` refers to the given generation (replay guard). Callers that
    /// carry an occurrence generation across an operation use this to reject a replayed
    /// or recreated alias.
    pub fn assert_generation(&self, alias: &str, generation: u64) -> Result<(), OccurrenceError> {
        let o = self.get_live(alias)?;
        if o.generation != generation {
            return Err(OccurrenceError::StaleGeneration {
                alias: alias.to_string(),
                presented: generation,
                current: o.generation,
            });
        }
        Ok(())
    }

    fn transition(
        &mut self,
        alias: &str,
        action: &'static str,
        allowed_from: &[Lifecycle],
        to: Lifecycle,
    ) -> Result<(), OccurrenceError> {
        let o = match self.by_alias.get_mut(alias) {
            Some(o) if o.state != Lifecycle::Removed => o,
            _ => return Err(OccurrenceError::UnknownAlias(alias.to_string())),
        };
        if !allowed_from.contains(&o.state) {
            return Err(OccurrenceError::IllegalTransition {
                alias: alias.to_string(),
                from: o.state,
                action,
            });
        }
        let decl = o.declaration_index;
        o.state = to;
        if to == Lifecycle::Removed {
            if let Some(idx) = decl {
                if let Some(c) = self.declaration_counts.get_mut(&idx) {
                    *c = c.saturating_sub(1);
                }
            }
        }
        Ok(())
    }

    /// Start: `Created` → `Running`. Rejects start-before-create and double-start.
    pub fn start(&mut self, alias: &str) -> Result<(), OccurrenceError> {
        self.transition(alias, "start", &[Lifecycle::Created], Lifecycle::Running)
    }

    /// Require that an alias refers to a running occurrence (exec/signal gating).
    pub fn require_running(&self, alias: &str, action: &'static str) -> Result<(), OccurrenceError> {
        let o = self.get_live(alias)?;
        if o.state != Lifecycle::Running {
            return Err(OccurrenceError::IllegalTransition {
                alias: alias.to_string(),
                from: o.state,
                action,
            });
        }
        Ok(())
    }

    /// Stop: `Running` → `Stopped` (idempotent if already stopped).
    pub fn stop(&mut self, alias: &str) -> Result<(), OccurrenceError> {
        self.transition(
            alias,
            "stop",
            &[Lifecycle::Running, Lifecycle::Stopped],
            Lifecycle::Stopped,
        )
    }

    /// Remove: any live state → `Removed`. Frees the declaration's cardinality slot.
    pub fn remove(&mut self, alias: &str) -> Result<(), OccurrenceError> {
        self.transition(
            alias,
            "remove",
            &[Lifecycle::Created, Lifecycle::Running, Lifecycle::Stopped],
            Lifecycle::Removed,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_start_exec_stop_remove_happy_path() {
        let mut r = OccurrenceRegistry::new();
        let h = r.create("c1", None, None).unwrap();
        assert!(h.starts_with("occ-"));
        assert_eq!(r.state("c1"), Some(Lifecycle::Created));
        r.start("c1").unwrap();
        assert_eq!(r.state("c1"), Some(Lifecycle::Running));
        r.require_running("c1", "exec").unwrap();
        r.stop("c1").unwrap();
        assert_eq!(r.state("c1"), Some(Lifecycle::Stopped));
        r.remove("c1").unwrap();
        assert_eq!(r.state("c1"), None);
    }

    #[test]
    fn start_before_create_is_denied() {
        let mut r = OccurrenceRegistry::new();
        assert_eq!(
            r.start("ghost").unwrap_err(),
            OccurrenceError::UnknownAlias("ghost".into())
        );
    }

    #[test]
    fn exec_on_unknown_id_is_denied() {
        let r = OccurrenceRegistry::new();
        assert_eq!(
            r.require_running("ghost", "exec").unwrap_err(),
            OccurrenceError::UnknownAlias("ghost".into())
        );
    }

    #[test]
    fn exec_before_running_is_denied() {
        let mut r = OccurrenceRegistry::new();
        r.create("c1", None, None).unwrap();
        // still Created, not Running
        assert!(matches!(
            r.require_running("c1", "exec").unwrap_err(),
            OccurrenceError::IllegalTransition { .. }
        ));
    }

    #[test]
    fn double_start_is_denied() {
        let mut r = OccurrenceRegistry::new();
        r.create("c1", None, None).unwrap();
        r.start("c1").unwrap();
        assert!(matches!(
            r.start("c1").unwrap_err(),
            OccurrenceError::IllegalTransition { .. }
        ));
    }

    #[test]
    fn duplicate_live_alias_is_denied() {
        let mut r = OccurrenceRegistry::new();
        r.create("c1", None, None).unwrap();
        assert_eq!(
            r.create("c1", None, None).unwrap_err(),
            OccurrenceError::AliasInUse("c1".into())
        );
    }

    #[test]
    fn removed_alias_can_be_recreated_with_new_generation() {
        let mut r = OccurrenceRegistry::new();
        r.create("c1", None, None).unwrap();
        assert_eq!(r.generation("c1"), Some(0));
        r.remove("c1").unwrap();
        r.create("c1", None, None).unwrap();
        assert_eq!(r.generation("c1"), Some(1));
    }

    #[test]
    fn stale_generation_is_rejected() {
        let mut r = OccurrenceRegistry::new();
        r.create("c1", None, None).unwrap();
        r.remove("c1").unwrap();
        r.create("c1", None, None).unwrap(); // now generation 1
        assert_eq!(
            r.assert_generation("c1", 0).unwrap_err(),
            OccurrenceError::StaleGeneration {
                alias: "c1".into(),
                presented: 0,
                current: 1,
            }
        );
        r.assert_generation("c1", 1).unwrap();
    }

    #[test]
    fn cardinality_denies_second_occurrence_for_one_declaration() {
        let mut r = OccurrenceRegistry::new();
        // declaration 0 admits exactly one occurrence
        r.create("c1", Some(0), Some(1)).unwrap();
        assert_eq!(
            r.create("c2", Some(0), Some(1)).unwrap_err(),
            OccurrenceError::CardinalityExceeded {
                declaration_index: 0,
                allowed: 1,
            }
        );
        // removing the first frees the slot
        r.remove("c1").unwrap();
        r.create("c2", Some(0), Some(1)).unwrap();
    }

    #[test]
    fn operating_on_removed_alias_is_denied() {
        let mut r = OccurrenceRegistry::new();
        r.create("c1", None, None).unwrap();
        r.remove("c1").unwrap();
        assert_eq!(
            r.start("c1").unwrap_err(),
            OccurrenceError::UnknownAlias("c1".into())
        );
    }
}
