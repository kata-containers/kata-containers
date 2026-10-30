# Kata Deploy

kata-deploy installs Kata Containers on Kubernetes nodes and creates Kubernetes
`RuntimeClass` resources. See the website documentation for:

- [Installation](https://kata-containers.github.io/kata-containers/installation/#install-on-kubernetes-with-helm-recommended)
- [RuntimeClass selection](https://kata-containers.github.io/kata-containers/kata-deploy-runtimeclasses/)
- [Helm configuration](https://kata-containers.github.io/kata-containers/helm-configuration/)

Select containerd's sandbox controller with
`shims.<name>.containerd.sandboxer`: `podsandbox` is the default; `shim` selects
SandboxAPI when supported by the runtime and containerd 2 or newer. If unset,
empty, or unavailable, kata-deploy uses `podsandbox` and logs a fallback reason
for an unavailable request. The NVIDIA
CPU/GPU Helm profiles select `shim` for their Rust runtimes, including GPU
SNP/TDX profiles, and retain `podsandbox` for Go runtimes. Override a custom runtime with
`customRuntimes.runtimes.<name>.containerd.sandboxer`; omission inherits its base
shim's setting. Debug and devkit variants inherit their base setting too.

For SandboxAPI, kata-deploy sets `runtime_type` to the absolute Rust shim path:
containerd's sandbox controller does not use the CRI `runtime_path` override
when starting the shim. This supports custom installation directories and
handler names without adding shim aliases to the containerd service's PATH.
Each handler retains its own `options.ConfigPath`.

Availability requires a supported Rust implementation, containerd 2 or newer,
and a successful native query confirming the running daemon's `shim` sandbox
controller initialized successfully. Version lookup and the socket probe are
bounded. Missing, disabled, failed, unreachable, or unknown controllers cause a
logged fallback to `podsandbox`; no `ctr` installation is required. A workload
failure does not trigger a controller change or retry.

When SandboxAPI is requested, the install CRI stage mounts the socket's parent
directory read-only and probes the distro-aware `containerd.runtimeSocket`.
Missing sockets do not prevent the install pod from starting. SELinux denial
of the probe also logs a fallback; this change does not broaden SELinux policy.
Explicitly empty custom-runtime settings select `podsandbox`; omission inherits
the base setting.

When switching an existing deployment to SandboxAPI with agent policy
enforcement, regenerate its policies with the matching genpolicy version before
starting new pods. Older policies reject the guest-local resolver and hostname
sources under `/run/kata-containers/sandbox/`. The agent copies these files into
each container's bundle before mounting them, so writable mounts cannot change
the sandbox templates or sibling containers' files. Explicit workload mounts
take precedence.

Genpolicy currently produces policies compatible with both Task API and
SandboxAPI. They permit both system-file source patterns and retain the pause
container entry even when SandboxAPI does not create a pause container. This is
compatibility behavior, rather than a policy restricted to a single API.
The `CreateSandboxRequest` policy does not constrain DNS or hostname values;
validating mount paths does not authenticate the contents supplied by the host.
Confidential deployments must account for this existing limitation when
deciding which host-provided network configuration to trust.
