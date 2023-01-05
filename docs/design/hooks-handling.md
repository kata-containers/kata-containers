# Kata Containers support for `Hooks`

## Introduction

During container's lifecycle, different Hooks can be executed to do custom actions. In Kata Containers, we support two types of Hooks, `OCI Hooks` and `Kata Hooks`.

### OCI Hooks

The OCI Spec stipulates six hooks that can be executed at different time points and namespaces, including `Prestart Hooks`, `CreateRuntime Hooks`, `CreateContainer Hooks`, `StartContainer Hooks`, `Poststart Hooks` and `Poststop Hooks`. We support these types of Hooks as compatible as possible in Kata Containers.

The path and arguments of these hooks will be passed to Kata for execution via `bundle/config.json`. For example:
```
...
"hooks": {
  "prestart": [
    {
      "path": "/usr/bin/prestart-hook",
      "args": ["prestart-hook", "arg1", "arg2"],
      "env":  [ "key1=value1"]
    }
  ],
  "createRuntime": [
    {
      "path": "/usr/bin/createRuntime-hook",
      "args": ["createRuntime-hook", "arg1", "arg2"],
      "env":  [ "key1=value1"]
    }
  ]
}
...
```

### Kata Hooks

In Kata, we support another three kinds of hooks executed in guest VM, including `Guest Prestart Hook`, `Guest Poststart Hook`, `Guest Poststop Hook`.

The executable files for Kata Hooks must be packaged in the *guest rootfs*. The file path to those guest hooks should be specified in the configuration file, and guest hooks must be stored in a subdirectory of `guest_hook_path` according to their hook type. For example:

+ In configuration file:
```
guest_hook_path="/usr/share/hooks"
```
+ In guest rootfs, prestart-hook is stored in `/usr/share/hooks/prestart/prestart-hook`.

## Execution
The table below summarized when and where those different hooks will be executed in Kata Containers:

| Hook Name | Hook Type | Hook Path | Exec Place | Exec Time |
|---|---|---|---|---|
| `Prestart(deprecated)` | OCI hook | host runtime namespace | host runtime namespace | After VM is started, before container is created. |
| `CreateRuntime` | OCI hook | host runtime namespace | host runtime namespace | After VM is started, before container is created, after `Prestart` hooks. |
| `CreateContainer` | OCI hook | host runtime namespace | host vmm namespace* | After VM is started, before container is created, after `CreateRuntime` hooks. |
| `StartContainer` | OCI hook | guest container namespace | guest container namespace | After container is created, before container is started. |
| `Poststart` | OCI hook | host runtime namespace | host runtime namespace | After container is started, before start operation returns. |
| `Poststop` | OCI hook | host runtime namespace | host runtime namespace | After container is deleted, before delete operation returns. |
| `Guest Prestart` | Kata hook | guest agent namespace | guest agent namespace | During start operation, before container command is executed. |
| `Guest Poststart` | Kata hook | guest agent namespace | guest agent namespace | During start operation, after container command is executed, before start operation returns. |
| `Guest Poststop` | Kata hook | guest agent namespace | guest agent namespace | During delete operation, after container is deleted, before delete operation returns. |

+ `Hook Path` specifies where hook's path be resolved.
+ `Exec Place` specifies in which namespace those hooks can be executed.
  + For `CreateContainer` Hooks, OCI requires to run them inside the container namespace while the hook executable path is in the host runtime, which is a non-starter for VM-based containers. So we design to keep them running in the *host vmm namespace.* 
+ `Exec Time` specifies at which time point those hooks can be executed.