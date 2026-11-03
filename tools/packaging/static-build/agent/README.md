# Static agent builds

`make agent-tarball` uses Kata's normal agent settings.
`make agent-substrate-tarball` produces a separate agent for Substrate.
The agent runs as PID 1 and uses a separate Cargo output directory.

The Substrate profile follows Ziyue Yan's agent build and
[option-by-option explanation](https://github.com/agent-substrate/substrate/pull/1967#issuecomment-6029284639).
The settings preserve the choices made after those upstream experiments:

| Setting | Reason |
| --- | --- |
| `LIBC=gnu` | Follow the upstream choice of GNU libc for allocator and string operations in running agents. Its musl build booted faster, but steady-state performance was not measured. |
| `SECCOMP=yes` | Keep enforcement of container seccomp profiles inside the guest. This is independent of the agent policy engine. |
| `AGENT_POLICY=no`, `INIT_DATA=no` | Omit the policy engine and init-data feature that Substrate does not use. |
| `USE_DEVMAPPER=no` | Omit dm-verity support, which this Substrate guest does not use. |
| `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1` | Compile each crate as one codegen unit, allowing more optimization within that crate at the cost of compilation parallelism. |
| `CARGO_PROFILE_RELEASE_LTO=true` | Use fat LTO to optimize across crates. This can reduce binary size and improve optimization at the cost of link time. |
| `CARGO_PROFILE_RELEASE_OPT_LEVEL=3` | Retain speed optimizations. The upstream size-optimized build improved boot time, but lacked steady-state benchmarks for exec, I/O and RPCs after restore. |
| `CARGO_PROFILE_RELEASE_STRIP=symbols` | Remove symbols from the shipped binary to reduce its size. |

See the [upstream codegen discussion](https://github.com/agent-substrate/substrate/pull/1967#discussion_r4457059534)
and [Cargo profile reference](https://doc.rust-lang.org/cargo/reference/profiles.html).
These settings are explicit environment overrides. Cargo reads profile settings
from the workspace root, so the release profile in `src/agent/Cargo.toml` does
not configure workspace builds.

The upstream measurements covered cold boot and snapshot size with Kata 4.1.0
on GKE. They did not measure steady-state agent throughput or RPC latency under
load, and do not establish performance gains for this Kata build. Revisit GNU
libc and optimization level 3 only with measurements of those workloads too.

See [Substrate tests](../../../../tests/integration/substrate/README.md) for
checking the guest image and running the microVM e2e suites.
