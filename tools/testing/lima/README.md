# Lima development lab

Create a disposable Ubuntu VM, provision Kubernetes using Kata's CI scripts,
and run a pod with a Kata RuntimeClass. The controller runs on Linux and macOS;
the local VM must support nested virtualization. Existing Linux hosts can also
be used locally or over SSH.

See the [usage guide](../../../docs/how-to/test-kata-with-lima.md) for host
requirements, setup, runtime selection, examples, and troubleshooting.

From this directory:

```sh
nix develop
just plan
just doctor
just up
just test
just logs
```

Nix supplies optional controller tools, including Lima; it does not require
NixOS. With the tools already installed, start directly with `just plan`.
The default source is this Kata checkout, shared through writable virtio-fs on
macOS and Linux. Source edits are visible immediately.
The default Kata binaries come from CI's `kata-deploy-ci:kata-containers-latest`.
Compare a specific release in a separate VM with:

```sh
just release=4.3.0 up
just release=4.3.0 test
just release=4.3.0 down
```

Choose a runtime directly, without an additional JSON profile:

```sh
just runtime=qemu-nvidia-cpu-runtime-rs up
just runtime=qemu-nvidia-cpu-runtime-rs test
```

Combine `runtime` and `release` to test that runtime in a release comparison VM.
`JUST_KATA_RUNTIME` persists the choice for the current shell.

Select another checkout, branch, or pull request with:

```sh
just kata_source=/path/to/kata-containers up
just branch=topic/my-change up
just pr=14100 up
```

Branch and PR selections create reusable Git worktrees. PRs also select their
matching CI image; that build must have been published for the target architecture.

For an existing Kubernetes cluster, install Kata on the current Linux node or
on an SSH node that already has a Kata checkout:

```sh
just backend=baremetal install-kata
just backend=baremetal host=user@node kata_source=/path/on/node/kata-containers install-kata
```

Use `install-cluster` instead to install Kubernetes and Kata on a fresh Ubuntu
26.04 node. Use the same selection with `test` and `logs` afterward.

The local Mac target requires an **Apple M3 or later** and **macOS 15 or later**.
M1, M2, and Intel Macs can control an existing remote Linux host instead.
`just doctor` queries Apple's nested virtualization capability before VM creation.

## Files

| Path | Purpose |
| --- | --- |
| `justfile` | Command interface |
| `scripts/lab.py` | Lima lifecycle, shared checkouts, SSH transport, and CI dispatch |
| `scripts/guest-ci.sh` | Fresh-host preparation, CI adapter, and smoke test |
| `scripts/guest.sh` | KVM hardware checks and diagnostics |
| `targets/*.json` | Local Lima VM profiles |
| `ci/default.json` | CI defaults; optional custom files can override advanced settings |
| `ci/apple-values.yaml` | Shared Mac nesting overrides for both tested runtime-rs handlers |
| `pins.json` | Ubuntu image checksums and smoke image digest |
| `flake.nix` | Optional controller development shell |
| `tests/test_lab.py` | Controller tests; no VM required |

## Check changes

```sh
just check
```

Without just, the equivalent checks are:

```sh
python3 -m unittest discover -s tests -v
bash -n scripts/guest.sh
bash -n scripts/guest-ci.sh
shellcheck scripts/guest.sh scripts/guest-ci.sh
```

The unit tests verify lifecycle ownership, failed-operation recovery, dry-run
behavior, shared checkout mounts, and CI argument forwarding. VM execution is
a separate integration check using `just up` and `just test`.
