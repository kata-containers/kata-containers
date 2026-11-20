# Run envd inside a container

Kata's Rust runtime (`runtime-rs`) can start envd inside a workload container.
The [envd design](../design/envd.md) explains how envd follows the container's
lifecycle.

Provide a statically linked `kata-envd` executable in one of these locations:

- For a [guest extension](../design/composable-vm-images.md) named `kata-envd`,
  place the executable at `bin/kata-envd` relative to the extension image root.
  Its full guest path is `/run/kata-extensions/kata-envd/bin/kata-envd`.
- In the guest image, install it at `/usr/libexec/kata-containers/kata-envd`.

For Dragonball, register the extension image in the host's `configuration.toml`.

```toml
[[hypervisor.dragonball.guest_extension_images]]
name = "kata-envd"
path = "/opt/kata/share/kata-containers/kata-envd-extension.img"
```

The agent uses the guest image path if the extension executable is absent.
It checks that the selected file is a regular executable and retains a file
descriptor to it across the root switch. A missing or invalid executable fails
container creation for an opted-in container.

Set the experimental annotation on the Pod and set `KATA_ENVD_ENABLED` to
`"true"` in the target container's environment. The annotation may change in
future releases.

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: example
  annotations:
    io.katacontainers.config.runtime.experimental_envd: >-
      {"args":["serve","--addr",":49983"]}
spec:
  runtimeClassName: kata
  containers:
    - name: app
      image: example/app:latest
      env:
        - name: KATA_ENVD_ENABLED
          value: "true"
```

For containerd CRI, include `io.katacontainers.config.runtime.experimental_envd` in the
runtime's `pod_annotations` allowlist so containerd forwards it to Kata.
For a standalone container, set both the annotation and the environment
variable in its OCI spec.

The annotation is a JSON object with a required `args` array. Provide the full
argument vector, excluding the executable name. Include the listen address in
`args` if envd needs one. Empty arrays and empty arguments are allowed. NUL
bytes are rejected.

Both settings are required. Set `KATA_ENVD_ENABLED` only on the intended
workload container. The runtime checks the final OCI environment, which can
also include variables from the image.
When that container is recreated with the same settings, envd starts again.

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
original init process from executing. Guest seccomp policies must permit the
additional fork, `execveat`, and envd's subsequent system calls. A policy that
kills the init process on fork can prevent the workload from starting. Agent
policies must allow `io.katacontainers.config.runtime.experimental_envd` in the
create request for an opted-in container.
