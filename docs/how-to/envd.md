# Run envd inside a container

Kata's Rust runtime (`runtime-rs`) can start an optional envd process in the first
non-pause container of a sandbox. Standalone containers are also eligible.

Install an executable, statically linked `kata-envd` at
`/usr/libexec/kata-containers/kata-envd` in the guest image. The runtime adds a
read-only bind mount at `/dev/.kata-containers/kata-envd` inside the selected
container. Placing the destination on `/dev` also supports a read-only
container root filesystem. The binary comes from the guest image; it does
not need to exist on the host or in the workload image.

Set this annotation on the **pod sandbox**, or on the standalone container:

```yaml
io.katacontainers.config.runtime.envd: '{"port":18080,"args":["serve","--addr",":{port}"]}'
```

For containerd CRI, include `io.katacontainers.config.runtime.envd` in the
runtime's `pod_annotations` allowlist so containerd forwards it to Kata.

The annotation must be a JSON object containing both fields:

- `port`: an integer between 1 and 65535.
- `args`: the complete argument array excluding the executable name. An empty
  array is valid. NUL bytes are rejected. Every `{port}` placeholder is replaced
  with the port value, preserving argument boundaries and empty strings.

Omitting the annotation disables envd. Container-level annotations in a pod
cannot enable or override the sandbox request. Pause containers are skipped;
init containers count as workloads. Selection happens during container creation,
so later containers do not receive envd while an earlier workload remains
registered in the sandbox. If all earlier workloads are deleted, the next
workload becomes eligible again.

The agent forks envd immediately before the selected container's init process
executes, after the Start FIFO, hooks, and security setup. It runs the argument
vector directly, without a shell, in the container's namespaces, cgroup,
working directory, environment, and user context. Standard input, output, and
error are connected to `/dev/null`. This prevents envd and its descendants from
holding workload output pipes open after init exits, including with shared PID
namespaces and passfd I/O. Envd logs are discarded. Exec processes do not start
another envd.

Kata does not supervise envd, wait for readiness, restart it, or configure host
port forwarding. The caller must arrange connectivity and readiness checks.
Fork or exec errors are reported on container stderr and do not prevent the
original init process from executing. A missing guest executable fails bind
mount setup and therefore fails container creation. Guest seccomp policies
must permit the additional fork and envd's system calls; a policy that kills
the init process on fork can prevent the workload from starting. Agent policies
must allow the injected annotation and mount.
