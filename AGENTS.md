# Kata Containers Agent Guide

This file tells an AI coding agent how to work in this repository: what the
tree contains, which checks to run before proposing a change, how commits must
be formatted, and which mistakes are made over and over again here.

Humans are welcome to read it too. Everything below is also good advice for a
first-time contributor, and it complements rather than replaces
[`CONTRIBUTING.md`](CONTRIBUTING.md), which owns the project-wide process, and
[`docs/code-pr-advice.md`](docs/code-pr-advice.md), which owns code style.

If you only remember four things, remember these:

1. Nothing you do becomes visible to anyone else, and nothing irreversible
   happens, without the user asking for it. See
   [What only the user may do](#what-only-the-user-may-do).
2. Every commit must build and must pass the checks for the components it
   touches. Not the branch tip: **every commit**.
3. Every commit must follow the [commit format](#commit-format). The format is
   enforced by CI and a malformed commit blocks the whole pull request.
4. Kata has **two runtimes**. New functionality goes into `runtime-rs` only,
   but a bug fix in one runtime is usually incomplete until you have checked
   the other for the same bug. See
   [Two runtimes, one behaviour](#two-runtimes-one-behaviour).

## What only the user may do

These are hard rules, and they matter more than anything else in this file.
Everything else here is about not wasting a reviewer's time; these are about not
speaking in somebody else's name and not destroying their work.

**Never post anything to GitHub.** Not a reply to a review comment, not a pull
request comment, not a review, not an approval, not an issue, not a comment on an
issue, not a label or a milestone. Not "on the user's behalf", and not with a
disclaimer saying an agent wrote it. A reviewer is having a conversation with a
person, and an agent that answers in that person's account misrepresents what
they think and signs them up to positions they have never read. Draft the reply,
show it in the chat, and let the user post it if they agree with it. This applies
just as much when the reply is obviously correct and the change is already made.

**Never rewrite or discard history that exists outside your own session.** A
force push, in either form; a rebase of a branch that has already been pushed; an
amend of a commit that is already on a remote. Each of these can silently destroy
commits the user made somewhere else, and a force push additionally detaches
every line comment on the open pull request from the code it referred to. Prepare
the branch locally, say what pushing it will require, and let the user push.
Consent is per push: a "yes" earlier in the conversation is not a standing
permission.

**Never take any other irreversible action unasked**, whether or not it is listed
above: deleting branches or tags, closing or merging pull requests, changing
repository settings, or pushing anywhere other than where the user told you to.

The shape of the rule is simple. Read anything. Change files in the working tree
and make local commits freely. Anything that other people can see, or that cannot
be undone with a command you know how to run, is the user's decision and the
user's keystroke.

## Project overview

Kata Containers runs each pod inside a lightweight virtual machine so that
container workloads get hardware-backed isolation while still looking and
behaving like ordinary containers to Kubernetes and containerd.

The moving parts you will most often touch:

| Part | Runs where | Language | Path |
| --- | --- | --- | --- |
| Shim (`containerd-shim-kata-v2`) | Host | Go | `src/runtime` |
| Shim (`containerd-shim-kata-v2`) | Host | Rust | `src/runtime-rs` |
| Agent (`kata-agent`) | Guest, as PID 1 or under an init | Rust | `src/agent` |
| Dragonball hypervisor | Host, in-process VMM | Rust | `src/dragonball` |
| Shared Rust libraries | Both | Rust | `src/libs` |
| Policy generator (`genpolicy`) | Host, build time | Rust | `src/tools/genpolicy` |
| Guest image and asset builders | Host, build time | Shell | `tools/packaging` |
| Deployment (`kata-deploy`, Helm chart) | Kubernetes | Rust, YAML | `tools/packaging/kata-deploy` |
| Tests | CI and locally | Shell, Go, Rust | `tests` |
| Documentation | - | Markdown | `docs` |

Two shims exist because Kata is midway through a rewrite. Since Kata 4.0
`runtime-rs` is the default shim, and the Go runtime is deprecated: still
shipped and still supported, but closed to new functionality.

## Repository map

Beyond `src/` and `tools/`, four things at the top level matter more than their
size suggests:

- `versions.yaml` — the single source of truth for every external dependency
  version (Go, Rust, QEMU, kernel, `golangci-lint`, and so on). Never hardcode
  a version elsewhere; read it from here.
- `Makefile` and `utils.mk` — the per-component `build`/`check`/`test`/`install`
  rules, plus `standard_rust_check`, the shared `cargo fmt` and `cargo clippy`
  invocation.
- `tools/packaging/kata-deploy/local-build/Makefile` — every `*-tarball` target
  that produces a shippable artefact. It is included by the root `Makefile`, so
  those targets work from the repository root.
- `.github/workflows/` — the definitive answer to "what will CI actually run on
  my pull request". `static-checks.yaml` and `build-checks.yaml` gate almost
  every change.

## Commit format

This is not stylistic advice. `.github/workflows/commit-message-check.yaml`
rejects pull requests whose commits do not comply, and it checks every commit,
not just the last one. [`CONTRIBUTING.md`](CONTRIBUTING.md#patch-format) is the
canonical statement of the format; what follows is the operational version of
it, with the mechanically enforced parts called out.

```text
subsystem: Short summary in the imperative mood

A real paragraph of prose that explains why the change is needed and, if it
is not obvious from the diff, how it works. Wrap at 72 characters. Somebody
bisecting to this commit in three years should be able to understand it
without opening a browser.

Fixes: #12345

Generated-By: <tool name, and the model if you know it>
Signed-off-by: Your Name <you@example.com>
```

The `Fixes:` line above is optional. The first five rules below are enforced by
CI; the rest are project convention, and reviewers will ask for them.

- **Subsystem prefix.** The subject must start with `something:`. It is a hint
  to the reader, not a validated enum, so pick the most specific thing that is
  true: `runtime-rs:`, `runtime:`, `agent:`, `libs:`, `kata-types:`,
  `dragonball:`, `genpolicy:`, `kata-deploy:`, `packaging:`, `tests:`, `ci:`,
  `docs:`, `versions:`. Use `runtimes:` when a single commit deliberately
  changes both shims. Nested prefixes such as `runtime-rs/qemu:` are common and
  fine. To see the ones actually in use, run
  `git log --no-merges --pretty=%s | cut -d: -f1 | sort | uniq -c | sort -rn | head -20`;
  the long tail below that is not a set of prefixes worth copying.
- **Subject length: 75 characters or fewer**, including the prefix.
- **A body is required.** A subject-only commit fails CI. The body is a
  standalone paragraph, not a continuation of the subject line.
- **Body lines: 150 characters or fewer.** Lines that start with a
  non-alphabetic character, lines without spaces, and sign-off lines are
  exempt, which is how log excerpts and long URLs get through.
- **`Signed-off-by:` is required** on every commit (DCO). Use `git commit -s`
  and let git read the identity from your `~/.gitconfig`.
- **`Fixes: #NNN`** is no longer required, but add it when the change resolves
  a reported issue: it closes the issue automatically and gives the next reader
  the background that the diff cannot. One commit in the series carries it,
  normally the main one. Use `Related: #NNN` when the change is connected to an
  issue it does not close.
- **AI involvement must be disclosed.** Kata follows the
  [OpenInfra Foundation AI policy](https://openinfra.org/legal/ai-policy/), as
  [`CONTRIBUTING.md`](CONTRIBUTING.md#ai-generated-code) spells out: use
  `Generated-By:` when a tool produced a substantial part of the change, and
  `Assisted-By:` when it only assisted, such as auto-complete or a small edit.
  Name the tool, and the model when you know it. Do not use `Co-authored-by:`
  for a tool; that trailer is for humans and carries a DCO implication. Put the
  disclosure *above* `Signed-off-by:`, so that the human sign-off comes last and
  reads as what it is: a statement that a person understands the change and
  takes responsibility for it.

### Writing the body

Write the way a human reviewer writes. Prose paragraphs, present tense,
explaining the problem and the reasoning. Do not dump a bullet list of the
files you touched; the diff already says that. Bullets are acceptable when the
content is genuinely a list, such as a set of measurements or an enumeration of
affected configuration flavours.

Do not paste tool output, task lists, or a narration of your own process into a
commit message.

### Splitting a series

One commit does one thing. A pull request may carry several commits, but each
one has to stand on its own: it must build, it must pass the checks for what it
touches, and its message has to make sense in isolation. Refactoring that
enables a fix belongs in its own commit, before the fix.

### Verifying before you push

```bash
base=upstream/main   # or whatever you branched from
rc=0
for commit in $(git rev-list --no-merges "${base}..HEAD"); do
	subject=$(git show -s --format=%s "${commit}")
	body=$(git show -s --format=%b "${commit}")
	id="${commit:0:12}"
	[[ "${#subject}" -le 75 ]] || { echo "${id}: subject is ${#subject} characters, the limit is 75"; rc=1; }
	[[ "${subject}" =~ ^[^:[:space:]]+: ]] || { echo "${id}: subject has no 'subsystem:' prefix"; rc=1; }
	grep -q '^Signed-off-by:' <<<"${body}" || { echo "${id}: no Signed-off-by trailer"; rc=1; }
	grep -qE '^[^[:space:]]' <<<"$(grep -viE '^([a-z-]+-by|fixes|related|closes):|^$' <<<"${body}")" \
		|| { echo "${id}: no commit body, only a subject"; rc=1; }
	awk -v id="${id}" 'length > 150 && /^[a-zA-Z]/ { print id": body line is " length " characters, the limit is 150" }' <<<"${body}"
done
exit "${rc}"
```

## Before you submit

Work down this ladder. Each rung is cheap relative to the one below it, and
skipping a rung is how a pull request ends up with a red CI run over a
missing newline.

### Rung 0: the components you touched

CI runs exactly three commands in each component directory
(`.github/workflows/build-checks.yaml`), so run the same ones:

```bash
make -C src/agent check          # cargo fmt --check + cargo clippy -D warnings
make -C src/agent test           # cargo test
sudo -E PATH="$PATH" make -C src/agent test
```

Substitute the component you changed. `make check-all` and `make test-all` from
the repository root fan out to most components, but not to
`src/tools/genpolicy` even though CI checks it, so run
`make -C src/tools/genpolicy check test` yourself if you touched it.

Two exceptions worth knowing:

- **`make -C src/runtime check` does nothing.** The target is declared `.PHONY`
  and `make -C src/runtime help` advertises it, but it has no recipe. For the
  Go runtime run `make -C src/runtime lint`, which builds everything and then
  runs `golangci-lint` against `tests/.golangci.yml`, and
  `make -C src/runtime test`. `make -C src/runtime pre-commit` is `lint` plus a
  faster test pass. `golangci-lint` has to be on your `PATH` already; the
  version is pinned in `versions.yaml`.
- **`runtime-rs` and `dragonball` tests need virtualization**, and CI runs them
  differently:

  ```bash
  sudo -E env PATH="$PATH" LIBC=gnu SUPPORT_VIRTUALIZATION=true make -C src/runtime-rs test
  sudo -E env PATH="$PATH" LIBC=gnu SUPPORT_VIRTUALIZATION=true make -C src/dragonball test
  ```

The `kata-deploy` crates under `tools/packaging/kata-deploy/binary` and
`tools/packaging/kata-deploy/job-dispatcher` are not part of `make check-all`,
and have their own CI job:

```bash
cargo fmt -p kata-deploy --check
cargo clippy -p kata-deploy --all-targets --all-features -- -D warnings
RUSTFLAGS="-D warnings" cargo test -p kata-deploy -- --test-threads=1
```

### Rung 1: whole-tree static checks

```bash
make static-checks
```

This is `tests/static-checks.sh`, and it is the check that surprises people
most often. Two things to know before running it:

- It needs `moreutils` (for `chronic`), and depending on which checks fire it
  also wants `yamllint`, `jq`, `xmllint`, `hadolint`, `opa`, and `regorus`.
  `tests/install_opa.sh` and `tests/install_regorus.sh` handle the last two.
  `golangci-lint` is downloaded automatically if missing.
- It works out what you changed by diffing against `origin/<branch>`. The
  remote name `origin` is hardcoded and no flag overrides it; `--branch` only
  chooses the branch, which otherwise defaults to whatever `origin`'s HEAD
  points at. In CI `origin` is the upstream repository, so this works out. On a
  developer machine `origin` is usually your fork, and anything upstream has
  that your fork's default branch does not is counted as part of your change.
  Point the tracking ref at upstream before running the script:

  ```bash
  git fetch <your-upstream-remote> '+main:refs/remotes/origin/main'
  ```

  That rewrites only the local `origin/main` tracking ref, not your fork.

`make static-checks` assumes the CI checkout layout, `$GOPATH/src/github.com/kata-containers/kata-containers`.
From a checkout anywhere else, drive the script directly:

```bash
test_path=tests bash tests/static-checks.sh --repo-path "$(pwd)"
```

To iterate quickly, run one check at a time. `bash tests/static-checks.sh --list`
prints them all; the useful flags are:

| Flag | What it checks |
| --- | --- |
| `--golang` | `golangci-lint` over every changed Go package |
| `--scripts` | `bash -n` over changed shell scripts |
| `--licenses` | SPDX and copyright headers on new files (Markdown is exempt) |
| `--json`, `--xml` | That changed data files still parse |
| `--dockerfiles` | `hadolint` |
| `--rego` | `opa parse` and `regorus parse` over policy files |
| `--versions` | `yamllint versions.yaml` |
| `--force --files` | TODO and FIXME comments without a linked issue |

Beware: `--commits` and `--docs` appear in the help output but are not wired
up. Use the commit snippet above instead.

### Rung 2: the checks that only fire for certain changes

These are separate CI jobs, and each one is a single local command:

| If you changed | Run | Why |
| --- | --- | --- |
| Any Go file, `go.mod`, `go.sum`, or `versions.yaml` | `find . -name go.mod -execdir go mod tidy \; && git diff --exit-code` | The `go-mod-tidy` job fails on any diff |
| `src/libs/protocols/protos/**` | `make -C src/agent generate-protocols && git diff --exit-code` | The `codegen` job regenerates and diffs the bindings |
| Agent RPC surface or the policy | `bash ci/check_agent_policy_coverage.sh` | Every agent endpoint needs policy coverage |
| `Cargo.toml` or `Cargo.lock` | `cargo deny check bans licenses sources` | The `cargo-deny` job |
| Anything under `tools/packaging/kernel/` other than its README | Bump `tools/packaging/kernel/kata_config_version` | A dedicated job enforces the bump |
| Any `.md` file | `make docs-lint` | Runs `cspell` and `editorconfig-checker` in Docker |
| A `.github/workflows/` file | `actionlint` and `zizmor` | Workflow linting and workflow security auditing |
| Any shell script | `shellcheck` at severity `style` | `shellcheck_required.yaml` |

Two notes on documentation. New project-specific vocabulary has to be added to
`tests/spellcheck/kata-dictionary.txt` or `cspell` will reject it, though
anything inside backticks is ignored. And new pages under `docs/` must be
registered in `docs/.nav.yml`; the repository ships a skill at
`.claude/skills/mkdocs-docs/SKILL.md` describing the documentation conventions.

### Rung 3: build the artefacts

Every commit must build. For a shim change that means, from the repository
root:

```bash
make shim-v2-rust-tarball     # runtime-rs
make shim-v2-go-tarball       # the Go runtime
```

Other useful targets from the same Makefile: `make agent-tarball`,
`make kernel-tarball`, `make rootfs-image-tarball`, `make genpolicy-tarball`,
and `make kata-tarball` for the complete bundle. They all build inside Docker,
so you need a working Docker daemon and enough disk.

To validate a whole series rather than just its tip:

```bash
git rebase --exec 'make shim-v2-rust-tarball' upstream/main
```

Stop there. Installing a build onto the host and running the end-to-end suites
under `tests/integration/kubernetes` need a machine you are willing to
reconfigure and a cluster to talk to. Leave both to CI unless the user asks for
them, and if they do ask, follow `tests/README.md` rather than improvising.

## Two runtimes, one behaviour

`src/runtime` (Go) and `src/runtime-rs` (Rust) are two implementations of the
same architecture. They share no business logic. They do share a configuration
file shape, an annotation namespace, an agent protocol, and a set of user
expectations.

The Go runtime is deprecated, which makes the rule asymmetric. Getting the
direction right matters:

- **New functionality goes into `runtime-rs` only.** Do not add a feature, a
  configuration option or an annotation to the Go runtime. If a request seems
  to call for that, say that the Go runtime is closed to new functionality and
  implement it in `runtime-rs` instead.
- **Bug fixes go wherever the bug is, which is usually both runtimes.** The two
  implementations were written from the same design, so a logic error, a
  missing validation or a wrong default in one is very often present in the
  other.

**So: before you consider a bug fix finished, look for the same bug in the
other runtime.** This is the single most common source of incomplete Kata
patches. If you are an agent working from a bug report, treat it as a required
step rather than a nicety.

### How to look

```bash
# Where does the same identifier, config key or log message live elsewhere?
git grep -n '<identifier>' -- src/runtime src/runtime-rs src/libs

# History of the concept, whichever tree it was touched in
git log --oneline -S'<string you changed>'

# Commits that deliberately changed both shims at once, as worked examples
git log --oneline --grep='^runtimes:'
```

Then read the mirror of the file you edited:

| Concern | Go | Rust |
| --- | --- | --- |
| Shim entry point | `src/runtime/cmd/containerd-shim-kata-v2/` | `src/runtime-rs/crates/shim/` |
| Sandbox and container lifecycle | `src/runtime/virtcontainers/sandbox.go`, `container.go` | `src/runtime-rs/crates/runtimes/virt_container/src/` |
| Hypervisor drivers | `src/runtime/virtcontainers/{qemu,clh,fc,remote}*.go` | `src/runtime-rs/crates/hypervisor/src/{qemu,ch,firecracker,remote}/` |
| Device management | `src/runtime/pkg/device/` | `src/runtime-rs/crates/hypervisor/src/device/` |
| Network endpoints | `src/runtime/virtcontainers/*_endpoint.go` | `src/runtime-rs/crates/resource/src/network/endpoint/` |
| Storage, mounts, cgroups | `src/runtime/virtcontainers/`, `src/runtime/pkg/resourcecontrol/` | `src/runtime-rs/crates/resource/` |
| Agent client | `src/runtime/virtcontainers/kata_agent.go` | `src/runtime-rs/crates/agent/src/kata/` |
| Sandbox state persistence | `src/runtime/virtcontainers/persist/` | `src/runtime-rs/crates/persist/` |
| Configuration parsing | `src/runtime/pkg/katautils/config.go` | `src/libs/kata-types/src/config/` |
| Annotations | `src/runtime/virtcontainers/pkg/annotations/annotations.go` | `src/libs/kata-types/src/annotations/mod.rs` |
| Configuration templates | `src/runtime/config/configuration-*.toml.in` | `src/runtime-rs/config/configuration-*-runtime-rs.toml.in` |

Note the last three rows: the `runtime-rs` side of configuration and
annotations does not live under `src/runtime-rs/` at all. It lives in
`src/libs/kata-types/`, which is shared. Agents routinely miss this and
conclude, wrongly, that `runtime-rs` has no equivalent.

### Configuration options and annotations

A *new* option is `runtime-rs` only, so the touch list is:

1. The templates in `src/runtime-rs/config/`, for every hypervisor flavour the
   option applies to (`qemu`, `clh`, `fc`, `remote`, `dragonball`, plus the
   confidential and NVIDIA GPU variants).
2. The parser in `src/libs/kata-types/src/config/`, including the
   per-hypervisor `validate()` and `adjust_config()` under
   `src/libs/kata-types/src/config/hypervisor/`.
3. `docs/how-to/how-to-set-sandbox-config-kata.md` if users can set it, and
   `docs/migrating-config-go-runtime-to-runtime-rs.md`, since a new
   `runtime-rs` option widens the documented gap between the two runtimes.
4. Tests under `src/libs/kata-types/tests/`.

*Fixing* how an existing option is parsed, validated or defaulted is a
different matter. That option almost certainly exists on the Go side too, so
check `src/runtime/pkg/katautils/config.go`, the struct in
`src/runtime/virtcontainers/hypervisor.go`, the templates under
`src/runtime/config/`, and `src/runtime/pkg/katautils/config_test.go`.

Annotations split the same way. A new one is a constant in
`src/libs/kata-types/src/annotations/mod.rs`, the code that applies it, a
documentation row, and an entry in `enable_annotations` in the affected
templates. A fix to an existing annotation should also be checked against
`src/runtime/virtcontainers/pkg/annotations/annotations.go`.

### What is deliberately not mirrored

Do not manufacture parity where none is intended:

- **Dragonball** is a real hypervisor only in `runtime-rs`. The Go runtime
  returns a mock for it.
- **StratoVirt** exists only in the Go runtime.
- **`kata-monitor` and the `kata-runtime` CLI** live in `src/runtime` and serve
  the Go runtime. The successor to `kata-runtime` is `kata-ctl`, a Rust rewrite
  in `src/tools/kata-ctl` that is its own component rather than part of either
  shim. `shim-ctl` in `runtime-rs` is not a CLI for users; it is a development
  tool for driving the shim without containerd.
- **VM cache and the `[factory]` machinery** are Go-only and deprecated.
- **`mem-agent`, `use_passfd_io`, and component plugin selection** are
  `runtime-rs`-only.
- **`ppc64le`** ships only the Go runtime.
- **NUMA mapping and network rate limiters** are not implemented in
  `runtime-rs` yet. Closing that gap is legitimate work, but it is a feature in
  its own right, not something to slip in alongside a bug fix.

`docs/migrating-config-go-runtime-to-runtime-rs.md` is the authoritative list
of intentional divergences. Read it before claiming a difference is a bug.

### When you genuinely cannot do both

Sometimes the counterpart change needs hardware you do not have, or is large
enough to deserve its own review. That is fine, but say so:

- Note it explicitly in the commit message or the pull request description.
- Suggest that the user open a GitHub issue for the other runtime, with the
  detail [any issue needs](#issues), and reference it with `Related: #NNN`.

What is not fine is fixing a bug in one runtime, saying nothing, and leaving
the identical bug in the other for somebody else to rediscover in production.

## Code conventions

Read [`docs/code-pr-advice.md`](docs/code-pr-advice.md): it covers error
handling, the narrow cases where Rust `unwrap()` and `expect()` are acceptable,
Go struct construction, comments, logging and architecture-specific code. Three
things it does not say, which a check here does enforce:

- **Copyright and SPDX headers** on every new file, required by the
  `--licenses` static check for everything except Markdown. Write the copyright
  line **without a year**, as in `Copyright (c) Example Corporation`: a year is
  not needed and only goes stale.
- **A `TODO` or `FIXME` must link to a GitHub issue**, or the
  `--force --files` static check flags it.
- **Dependency versions come from `versions.yaml`.** Nowhere else.

## Traps

Things that have cost real time in this repository:

- **`make -C src/runtime check` silently does nothing.** Running it proves
  nothing. Use `lint` and `test`.
- **`cargo` commands run against the root workspace.** Most crates, including
  `src/agent`, `src/libs/*`, `src/runtime-rs`, and the tools, are members of
  the workspace defined by the root `Cargo.toml`. Running `cargo clippy` inside
  a component directory still resolves upward, so `-p <crate>` is usually what
  you want.
- **The default Rust target is musl** (`LIBC=musl` in `utils.mk`), overridden
  to gnu on `ppc64le`, `riscv64`, and `s390x`. A build that works for you with
  the default host toolchain can still fail in CI. Install `musl-tools`.
- **`clippy` runs with `-D warnings`.** A warning is a build failure. This
  includes lints such as `too_many_arguments`, which will make you restructure
  a function signature rather than suppress it.
- **`Cargo.lock` is checked in and `clippy` runs with `--locked`.** Adding a
  dependency means committing the lock file update, and `cargo deny` then has
  an opinion about the licence.
- **Generated files are diffed by CI.** Protocol bindings and the generated
  configuration files must be regenerated and committed, not left for CI.
- **Configuration templates are `.toml.in`, not `.toml`.** Editing a generated
  `.toml` under a build directory changes nothing.
- **Falling behind upstream inflates the "changed files" set.** Both
  `tests/static-checks.sh` and `tools/testing/gatekeeper/skips.py` derive it
  from a diff against `origin/<branch>`, so a branch that is behind gets linted
  over files it never touched and, in CI, has gatekeeper schedule test suites
  the change does not need. Rebase on upstream `main` as you go rather than once
  at the end, and expect unrelated CI failures when you do not.
- **`shim-v2-tarball` no longer exists.** It was split into
  `shim-v2-go-tarball` and `shim-v2-rust-tarball`.

## Issues

You do not file, close or comment on issues; see
[What only the user may do](#what-only-the-user-may-do). You draft them.

What you draft still has to be actionable, because an issue without these two
things will sit untouched until somebody asks for them:

- **A reproducer.** The smallest set of commands or the pod YAML that triggers
  the problem, along with the Kata version, the hypervisor, the container
  manager, and the host kernel and distribution.
- **Logs captured with debug enabled.** Turn on full debug first, following
  [`docs/Developer-Guide.md`](docs/Developer-Guide.md#enable-full-debug), then
  reproduce, then include the output of `kata-collect-data.sh`, which gathers
  the configuration and the runtime, agent and hypervisor logs in one go.

## Pull requests

- Base your branch on an up-to-date `main`, rebase rather than merge, and rebase
  again when it falls behind.
- Keep unfinished work as a draft pull request. The `wip` and `do-not-merge`
  labels and a `WIP` in the title also block the merge, and CI recognises them.
- New features need documentation that says what the feature does, why it is
  useful, how to use it, and what its limitations are.
- Read [`docs/code-pr-advice.md`](docs/code-pr-advice.md) and the
  [PR review guide](https://github.com/kata-containers/community/blob/main/PR-Review-Guide.md)
  to see what a reviewer will be looking for.

## Further reading

- [`docs/Developer-Guide.md`](docs/Developer-Guide.md) — building and installing from source
- [`docs/code-pr-advice.md`](docs/code-pr-advice.md) — what reviewers expect from code
- [`docs/Unit-Test-Advice.md`](docs/Unit-Test-Advice.md) — the table-driven test style used here
- [`docs/migrating-config-go-runtime-to-runtime-rs.md`](docs/migrating-config-go-runtime-to-runtime-rs.md) — how the two runtimes differ
- [`docs/how-to/how-to-set-sandbox-config-kata.md`](docs/how-to/how-to-set-sandbox-config-kata.md) — the annotation reference
- [`docs/Documentation-Requirements.md`](docs/Documentation-Requirements.md) — documentation style rules
- [`tests/README.md`](tests/README.md) — the test suites and how to run them
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — the project-wide process, patch format and review flow
