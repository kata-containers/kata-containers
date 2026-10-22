# Run envd inside a container

Kata's Go and Rust runtimes can start an optional envd process in the first
non-pause container of a sandbox. Standalone containers are also eligible.

Provide an executable, statically linked envd inside the guest. By default,
the runtime uses `/usr/libexec/kata-containers/kata-envd`. The runtime adds a
read-only bind mount at `/dev/.kata-containers/kata-envd` inside the selected
container. Placing the destination on `/dev` also supports a read-only
container root filesystem. The executable does not need to exist on the host
or in the workload image.

Set this annotation on the **pod sandbox**, or on the standalone container:

```yaml
io.katacontainers.config.runtime.envd: '{"port":18080,"args":["serve","--addr",":{port}"]}'
```

For containerd CRI, include `io.katacontainers.config.runtime.envd` in the
runtime's `pod_annotations` allowlist so containerd forwards it to Kata.

The annotation must be a JSON object containing both required fields:

- `port`: an integer between 1 and 65535.
- `args`: the complete argument array excluding the executable name. An empty
  array is valid. NUL bytes are rejected. Every `{port}` placeholder is replaced
  with the port value, preserving argument boundaries and empty strings.

The optional `guest_path` field overrides the executable's source path inside
the guest. It must be an absolute path without NUL bytes; empty and relative
paths are rejected. Omitting it uses `/usr/libexec/kata-containers/kata-envd`.
The container destination and execution path remain fixed.
Before mounting a non-default source, the agent checks that it resolves to a
regular file with an executable permission bit; otherwise container creation
fails. The default path is trusted as part of the operator-provided guest image.

For example, an envd supplied by a [guest extension](../design/composable-vm-images.md)
can be selected with:

```yaml
io.katacontainers.config.runtime.envd: '{"port":18080,"args":["serve","--addr",":{port}"],"guest_path":"/run/kata-extensions/envd/bin/kata-envd"}'
```

The extension must be configured and mounted before container creation. Other
provisioning methods are also supported, provided the executable is available
at the specified guest path. The runtime does not inspect the image contents
or start processes through the extension manifest.

Omitting the annotation disables envd. Container-level annotations in a pod
cannot enable or override the sandbox request. Pause containers are skipped;
init containers count as workloads. Selection happens during container creation,
so later containers do not receive envd while an earlier workload remains
registered in the sandbox. If all earlier workloads are deleted, the next
workload becomes eligible again.

The agent forks envd immediately before the selected container's init process
executes, after the Start FIFO, hooks, and security setup. It runs the argument
vector directly, without a shell, in the container's namespaces, cgroup,
working directory, environment, and user context. Standard input is `/dev/null`;
standard output and error are inherited from the container. Exec processes do
not start another envd.

Kata does not supervise envd, wait for readiness, restart it, or configure host
port forwarding. The caller must arrange connectivity and readiness checks.
Fork or exec errors are reported on container stderr and do not prevent the
original init process from executing. A missing guest executable fails bind
mount setup and therefore fails container creation. Guest seccomp policies
must permit the additional fork and envd's system calls; a policy that kills
the init process on fork can prevent the workload from starting. Agent policies
must allow the injected annotation and mount.
