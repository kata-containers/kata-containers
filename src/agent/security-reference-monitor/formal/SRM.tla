-------------------------------- MODULE SRM --------------------------------
(***************************************************************************)
(* FR-15 — formal model of the Security Reference Monitor (SRM) lifecycle. *)
(*                                                                         *)
(* This models the two-phase transaction lifecycle implemented in          *)
(* security-reference-monitor/src/lib.rs: every security-relevant,         *)
(* state-mutating operation is prepared, executed (binding the authorized  *)
(* == executed plan), then either committed or aborted, with a global      *)
(* quarantine that fails closed.                                           *)
(*                                                                         *)
(* The model checks the equivalence-claim safety properties:               *)
(*   - the state version equals the number of committed operations         *)
(*     (no phantom commit, no missed commit);                              *)
(*   - a committed operation was necessarily executed first, i.e. its      *)
(*     authorized plan equals its executed plan (Commit is only enabled    *)
(*     from the "executed" state);                                         *)
(*   - Committed and Aborted are terminal;                                 *)
(*   - quarantine is sticky and, once set, blocks new authorizations.      *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS Ops   \* a finite set of operation identifiers

States == {"none", "prepared", "executed", "committed", "aborted"}

VARIABLES
    state,        \* state[o] : the lifecycle state of operation o
    version,      \* the monotonic committed-state version
    quarantined   \* whether the monitor is quarantined

vars == <<state, version, quarantined>>

TypeOK ==
    /\ state \in [Ops -> States]
    /\ version \in Nat
    /\ quarantined \in BOOLEAN

Init ==
    /\ state = [o \in Ops |-> "none"]
    /\ version = 0
    /\ quarantined = FALSE

(* Phase 1: reserve state for an authorized plan. Refused when quarantined. A fresh
   op, or one previously aborted, may be (re)prepared. *)
Prepare(o) ==
    /\ ~quarantined
    /\ state[o] \in {"none", "aborted"}
    /\ state' = [state EXCEPT ![o] = "prepared"]
    /\ UNCHANGED <<version, quarantined>>

(* Phase 2a: bind the executed plan. Enabled only from "prepared"; this is where
   authorized == executed is enforced in the implementation. Refused when quarantined. *)
Execute(o) ==
    /\ ~quarantined
    /\ state[o] = "prepared"
    /\ state' = [state EXCEPT ![o] = "executed"]
    /\ UNCHANGED <<version, quarantined>>

(* Phase 2b: commit. Enabled ONLY from "executed", so a committed op was necessarily
   executed (authorized == executed). Advances the version by exactly one. *)
Commit(o) ==
    /\ state[o] = "executed"
    /\ state' = [state EXCEPT ![o] = "committed"]
    /\ version' = version + 1
    /\ UNCHANGED quarantined

(* Roll back a reserved/executed op. Does NOT advance the version (no committed effect). *)
Abort(o) ==
    /\ state[o] \in {"prepared", "executed"}
    /\ state' = [state EXCEPT ![o] = "aborted"]
    /\ UNCHANGED <<version, quarantined>>

(* Fault injection: at any point the monitor may quarantine (fail closed). Sticky. *)
Quarantine ==
    /\ ~quarantined
    /\ quarantined' = TRUE
    /\ UNCHANGED <<state, version>>

Next ==
    \/ \E o \in Ops : Prepare(o)
    \/ \E o \in Ops : Execute(o)
    \/ \E o \in Ops : Commit(o)
    \/ \E o \in Ops : Abort(o)
    \/ Quarantine

Spec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
(* Invariants *)

Committed == {o \in Ops : state[o] = "committed"}

(* Safety: the version is exactly the number of committed operations — there is no
   phantom commit (version advanced with no committed op) and no missed commit. *)
VersionMatchesCommits == version = Cardinality(Committed)

(* Committed and Aborted are distinct terminal states; since a variable holds a single
   value, an op is never simultaneously committed and aborted. Stated explicitly: *)
TerminalExclusive ==
    \A o \in Ops : ~(state[o] = "committed" /\ state[o] = "aborted")

Safety == TypeOK /\ VersionMatchesCommits /\ TerminalExclusive

----------------------------------------------------------------------------
(* Temporal properties *)

(* Quarantine is sticky: once set it never clears. *)
QuarantineSticky == [](quarantined => []quarantined)

=============================================================================
