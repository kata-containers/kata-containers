# Envd in workload containers

[Envd](https://github.com/e2b-dev/runtime/tree/main/packages/envd) is E2B's
open-source guest agent for running commands and accessing files in a sandbox.
Its APIs are emerging as a de facto interface for E2B-compatible agent sandboxes,
as seen in the
[Kubernetes SIGs Agent Sandbox examples](https://github.com/kubernetes-sigs/agent-sandbox/blob/main/examples/envd-sandbox/README.md)
and [CubeSandbox](https://github.com/TencentCloud/CubeSandbox/issues/1227).

This integration runs envd inside the workload container so those operations use
the workload's filesystem, namespaces, and user credentials.

## Design goals

Bring envd's APIs to existing Kubernetes workloads with zero changes to the
application image or entrypoint. Kata starts the guest-provided envd binary
alongside the workload in the application's own container. The original
entrypoint runs as the container's main process and proceeds without waiting for
envd to initialize.

Get each sandbox ready to use as quickly as possible. Kata forks envd directly
from container init, avoiding a separate exec request from the orchestrator.
The sandbox is ready to hand off as soon as the workload has started and envd
can serve requests.

## Configuration

Operators provide envd at a fixed path in the guest image or a
[guest extension](composable-vm-images.md). The experimental sandbox annotation
carries only the argument vector, including any listen address.

Each container whose effective OCI environment sets
`KATA_ENVD_ENABLED=true` starts its own envd with the sandbox arguments.
Kata checks this environment at container creation time.
Selection is independent of the order in which containers start. See the
[configuration example](../how-to/how-to-use-kata-envd.md).

## Startup

The agent opens envd in the guest before switching to the workload's root
filesystem. The file descriptor remains valid after the switch, so envd can
run in the workload's context without a bind mount.

The container init forks a child to run envd from that descriptor. The parent
keeps its PID and executes the workload without waiting for envd.

## Lifecycle

Recreating an opted-in container starts a new envd instance. In a private PID
namespace, the kernel terminates envd when PID 1 exits. In a shared PID
namespace, container teardown cleans up remaining processes through the
existing cgroup cleanup.

The caller configures connectivity, checks readiness, and handles recovery if
envd exits. Kata starts envd with the container and leaves independent restarts
to the caller.
