# Loading kernel modules

A new feature for loading kernel modules was introduced in Kata Containers 1.9.
The list of kernel modules and their parameters can be provided using the
configuration file or OCI annotations. The [Kata runtime][1] gives that
information to the [Kata Agent][2] through gRPC when the sandbox is created.
The [Kata Agent][2] will insert the kernel modules using `modprobe(8)`, hence
modules dependencies are resolved automatically.

The sandbox will not be started when:

  * A kernel module is specified and the `modprobe(8)` command is not installed in
    the guest or it fails loading the module.
  * The module is not available in the guest or it doesn't meet the guest kernel
    requirements, like architecture and version.

In the following sections are documented the different ways that exist for
loading kernel modules in Kata Containers.

- [Using Kata Configuration file](#using-kata-configuration-file)
- [Using annotations](#using-annotations)

# Using Kata Configuration file

```
NOTE: Use this method, only if you need to pass the kernel modules to all
containers. Please use annotations described below to set per pod annotations.
```

The list of kernel modules and parameters can be set in the `kernel_modules`
option as a coma separated list, where each entry in the list specifies a kernel
module and its parameters. Each list element comprises one or more space separated
fields. The first field specifies the module name and subsequent fields specify
individual parameters for the module.

The following example specifies two modules to load: `e1000e` and `i915`. Two parameters
are specified for the `e1000` module: `InterruptThrottleRate` (which takes an array
of integer values) and `EEE` (which requires a single integer value).

```toml
kernel_modules=["e1000e InterruptThrottleRate=3000,3000,3000 EEE=1", "i915"]
```

Not all the container managers allow users provide custom annotations, hence
this is the only way that Kata Containers provide for loading modules when
custom annotations are not supported.

There are some limitations with this approach:

* Write access to the Kata configuration file is required.
* The configuration file must be updated when a new container is created,
  otherwise the same list of modules is used, even if they are not needed in the
  container.

# Using annotations

As was mentioned above, not all containers need the same modules, therefore using
the configuration file for specifying the list of kernel modules per [POD][3] can
be a pain.
Unlike the configuration file, [annotations](how-to-set-sandbox-config-kata.md)
provide a way to specify custom configurations per POD.

The list of kernel modules and parameters can be set using the annotation
`io.katacontainers.config.agent.kernel_modules` as a semicolon separated
list, where the first word of each element is considered as the module name and
the rest as its parameters.

In the following example two PODs are created, but the kernel modules `e1000e`
and `i915` are inserted only in the POD `pod1`.


```yaml
apiVersion: v1
kind: Pod
metadata:
  name: pod1
  annotations:
    io.katacontainers.config.agent.kernel_modules: "e1000e EEE=1; i915"
spec:
  runtimeClassName: kata
  containers:
  - name: c1
    image: busybox
    command:
      - sh
    stdin: true
    tty: true

---
apiVersion: v1
kind: Pod
metadata:
  name: pod2
spec:
  runtimeClassName: kata
  containers:
  - name: c2
    image: busybox
    command:
      - sh
    stdin: true
    tty: true
```

> **Note**: To pass annotations to Kata containers, [CRI-O must be configured correctly](how-to-set-sandbox-config-kata.md#cri-o-configuration)

[1]: ../../src/runtime
[2]: ../../src/agent
[3]: https://kubernetes.io/docs/concepts/workloads/pods/pod/
