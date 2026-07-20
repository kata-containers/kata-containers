# SRM formal model (FR-15)

`SRM.tla` is a TLA+ model of the Security Reference Monitor's two-phase transaction
lifecycle implemented in `../src/lib.rs` (`prepare` → `execute` → `commit`/`abort`, with a
fail-closed `quarantine`).

It model-checks the equivalence-claim safety properties over all interleavings of a small
finite set of operations:

- **`VersionMatchesCommits`** — the state version equals the number of committed
  operations (no phantom commit, no missed commit).
- **authorized == executed** — `Commit` is enabled only from the `executed` state, so a
  committed operation was necessarily executed first.
- **`TerminalExclusive`** — `committed` and `aborted` are terminal and mutually exclusive.
- **`QuarantineSticky`** — once quarantined the monitor never clears; `prepare`/`execute`
  are disabled while quarantined (fail closed).

## Run

```bash
./run-tlc.sh          # fetches tla2tools.jar if needed, runs TLC
```

Last result: `Model checking completed. No error has been found.` (250 distinct states).
Deadlock checking is disabled because the model legitimately terminates (all operations
reach a terminal state or the monitor quarantines).
