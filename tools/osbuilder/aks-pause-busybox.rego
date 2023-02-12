package coco_policy

import future.keywords.in
import future.keywords.every

######################################################################
# Default values:
#
# - true for requests that are allowed by default.
# - false for requests that have additional policy rules, defined below.
# - Requests that are not listed here get rejected by default.

# More detailed policy rules are below.
default CreateContainerRequest := false

# Requests that are always allowed.
default CreateSandboxRequest := true
default DestroySandboxRequest := true
default GetOOMEventRequest := true
default GuestDetailsRequest := true
default OnlineCPUMemRequest := true
default ReadStreamRequest := true
default RemoveContainerRequest := true
default SignalProcessRequest := true
default StartContainerRequest := true
default StatsContainerRequest := true
default TtyWinResizeRequest := true
default UpdateInterfaceRequest := true
default UpdateRoutesRequest := true
default WaitProcessRequest := true
default WriteStreamRequest := true


# Image service should make is_allowed!() calls.
#
# Might use policy metadata to reject images that were
# not referenced by config.json.
#default PullImageRequest := false


######################################################################
CreateContainerRequest {
    input_container := input.oci

    # Enforce container creation order.
    policy_container := policy_containers[input.index]

    policy_container.ociVersion     == input_container.ociVersion
    policy_container.root.readonly  == input_container.root.readonly

    allow_annotations(policy_container, input_container)
    allow_process(policy_container, input_container)
    allow_linux(policy_container, input_container)
}

######################################################################
# Rules for, and/or based, on annotations

allow_annotations(policy_container, input_container) {
    allow_by_container_types(policy_container, input_container)
    allow_by_bundle_id(policy_container, input_container)
    allow_sandbox_namespace(policy_container, input_container)
    allow_sandbox_id(policy_container, input_container)
}

######################################################################
# - Check that the "io.kubernetes.cri.container-type" and
#   "io.katacontainers.pkg.oci.container_type" annotations
#   designate the expected type = either a "sandbox" or a
#   "container" type.
#
# - Then, validate other annotations based on the expected
#   "sandbox" or "container" value.

allow_by_container_types(policy_container, input_container) {
    policy_cri_container_type := policy_container.annotations["io.kubernetes.cri.container-type"]
    input_cri_container_type := input_container.annotations["io.kubernetes.cri.container-type"]

    policy_cri_container_type == input_cri_container_type

    allow_by_container_type(input_cri_container_type, policy_container, input_container)
}

# Rules applicable to the "sandbox" container type
allow_by_container_type(input_cri_container_type, policy_container, input_container) {
    input_cri_container_type == "sandbox"

    input_kata_container_type := input_container.annotations["io.katacontainers.pkg.oci.container_type"]
    input_kata_container_type == "pod_sandbox"

    alow_container_name_for_sandbox(policy_container, input_container)
    alow_image_name_for_sandbox(policy_container, input_container)
    alow_network_namespace_for_sandbox(policy_container, input_container)
    allow_log_directory_for_sandbox(policy_container, input_container)
    alow_sandbox_memory_for_sandbox(input_container)
}

# Rules applicable to the "container" container type
allow_by_container_type(input_cri_container_type, policy_container, input_container) {
    input_cri_container_type == "container"

    input_kata_container_type := input_container.annotations["io.katacontainers.pkg.oci.container_type"]
    input_kata_container_type == "pod_container"

    alow_container_name_for_container(policy_container, input_container)
    alow_image_name_for_container(policy_container, input_container)
    alow_network_namespace_for_container(policy_container, input_container)
    allow_log_directory_for_container(policy_container, input_container)
    alow_sandbox_memory_for_container(input_container)
}

######################################################################
# "io.kubernetes.cri.image-name" annotation

alow_image_name_for_sandbox(policy_container, input_container) {
    allow_sandbox_annotation(policy_container, input_container, "io.kubernetes.cri.image-name")
}

alow_image_name_for_container(policy_container, input_container) {
    allow_container_annotation(policy_container, input_container, "io.kubernetes.cri.image-name")
}

######################################################################
# "io.kubernetes.cri.container-name" annotation

alow_container_name_for_sandbox(policy_container, input_container) {
    allow_sandbox_annotation(policy_container, input_container, "io.kubernetes.cri.container-name")
}

alow_container_name_for_container(policy_container, input_container) {
    allow_container_annotation(policy_container, input_container, "io.kubernetes.cri.container-name")
}

######################################################################
# Annotions required for "container" type, and not allowed for "sandbox" type.

allow_sandbox_annotation(policy_container, input_container, annotation_key) {
    not policy_container.annotations[annotation_key]
    not input_container.annotations[annotation_key]
}

allow_container_annotation(policy_container, input_container, annotation_key) {
    policy_value := input_container.annotations[annotation_key]
    input_value := input_container.annotations[annotation_key]

    policy_value == input_value
}

######################################################################
# "io.kubernetes.cri.sandbox-memory" annotation

alow_sandbox_memory_for_sandbox(input_container) {
    sandbox_memory := input_container.annotations["io.kubernetes.cri.sandbox-memory"]
    to_number(sandbox_memory) >= 0
}
alow_sandbox_memory_for_container(input_container) {
    not input_container.annotations["io.kubernetes.cri.sandbox-memory"]
}

######################################################################
# "nerdctl/network-namespace" annotation

alow_network_namespace_for_sandbox(policy_container, input_container) {
    policy_network_namespace := policy_container.annotations["nerdctl/network-namespace"]
    input_network_namespace := input_container.annotations["nerdctl/network-namespace"]

    regex.match(policy_network_namespace, input_network_namespace)
}

alow_network_namespace_for_container(policy_container, input_container) {
    not policy_container.annotations["nerdctl/network-namespace"]
    not input_container.annotations["nerdctl/network-namespace"]
}

######################################################################
# "io.kubernetes.cri.sandbox-log-directory" and "io.kubernetes.cri.sandbox-name" annotations

allow_log_directory_for_sandbox(policy_container, input_container) {
    policy_sandbox_name := policy_container.annotations["io.kubernetes.cri.sandbox-name"]
    input_sandbox_name := input_container.annotations["io.kubernetes.cri.sandbox-name"]

    policy_sandbox_name == input_sandbox_name

    policy_log_directory := policy_container.annotations["io.kubernetes.cri.sandbox-log-directory"]
    directory_regex := replace(policy_log_directory, "$(sandbox-name)", policy_sandbox_name)

    input_log_directory := input_container.annotations["io.kubernetes.cri.sandbox-log-directory"]
    regex.match(directory_regex, input_log_directory)
}

allow_log_directory_for_container(policy_container, input_container) {
    not policy_container.annotations["io.kubernetes.cri.sandbox-log-directory"]
    not input_container.annotations["io.kubernetes.cri.sandbox-log-directory"]
}

######################################################################
# "io.kubernetes.cri.sandbox-namespace" annotation

allow_sandbox_namespace(policy_container, input_container) {
    policy_namespace := policy_container.annotations["io.kubernetes.cri.sandbox-namespace"]
    input_namespace := input_container.annotations["io.kubernetes.cri.sandbox-namespace"]

    policy_namespace == input_namespace
}


######################################################################
# "io.kubernetes.cri.sandbox-id" annotation

allow_sandbox_id(policy_container, input_container) {
    policy_sandbox_regex := policy_container.annotations["io.kubernetes.cri.sandbox-id"]
    regex.match(policy_sandbox_regex, input_container.annotations["io.kubernetes.cri.sandbox-id"])
}

######################################################################
# linux fields

allow_linux(policy_container, input_container) {
    policy_container.linux.namespaces == input_container.linux.namespaces
    policy_container.linux.maskedPaths == input_container.linux.maskedPaths
    policy_container.linux.readonlyPaths == input_container.linux.readonlyPaths
}

######################################################################
# Get the input bundle_id from "io.katacontainers.pkg.oci.bundle_path"
# and check its consistency with other rules.

allow_by_bundle_id(policy_container, input_container) {
    bundle_path := input_container.annotations["io.katacontainers.pkg.oci.bundle_path"]
    bundle_id := replace(bundle_path, "/run/containerd/io.containerd.runtime.v2.task/k8s.io/", "")

    allow_root_path(policy_container, input_container, bundle_id)

    every input_mount in input.oci.mounts {
        allow_mount(input_mount, policy_container, bundle_id)
    }
}

######################################################################
# Validate the config.json process fields.

allow_process(policy_container, input_container) {
    policy_process := policy_container.process
    input_process := input_container.process

    policy_process.terminal         == input_process.terminal
    policy_process.user             == input_process.user

    allow_args(policy_process, input_process)

    # - Reject environment variables that are present in the input,
    #   but not explicitly allowed by the policy.
    #
    # - Ignore any environment variables that are allowed by the
    #   policy but not present in the input.
    every env_var in input_process.env {
        policy_process.env[_] == env_var
    }

    policy_process.cwd              == input_process.cwd
    policy_process.capabilities     == input_process.capabilities
    policy_process.noNewPrivileges  == input_process.noNewPrivileges
}

######################################################################
# args

allow_args(policy_process, input_process) {
    # Neither policy nor input include any args.
    not policy_process.args
    not input_process.args
}
allow_args(policy_process, input_process) {
    # Both policy and input include identical args.
    policy_process.args == input_process.args
}

######################################################################
# root.path

allow_root_path(policy_container, input_container, bundle_id) {
    # Example policy: "path": "/run/kata-containers/shared/containers/$(bundle-id)/rootfs",
    policy_root_path := replace(policy_container.root.path, "$(bundle-id)", bundle_id)
    policy_root_path == input_container.root.path
}

######################################################################
# mounts

allow_mount(input_mount, policy_container, bundle_id) {
    # At least one policy mount rule allows the input mount.
    some policy_mount in policy_container.mounts
    policy_mount_allows(policy_mount, input_mount, bundle_id)
}

policy_mount_allows(policy_mount, input_mount, bundle_id) {
    # Exact match of policy and input mounts.
    policy_mount == input_mount
}
policy_mount_allows(policy_mount, input_mount, bundle_id) {
    policy_mount.destination == input_mount.destination
    policy_mount.type == input_mount.type
    policy_mount.options == input_mount.options

    # Policy mount source regex including $(bundle-id) alows the input mount - e.g.:
    #
    # "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-resolv.conf$",
    policy_source_regex := replace(policy_mount.source, "$(bundle-id)", bundle_id)
    regex.match(policy_source_regex, input_mount.source)
}

######################################################################
# containers

policy_containers := [
{
        "ociVersion": "1.0.2-dev",
        "process": {
            "terminal": false,
            "user": {
                "uid": 65535,
                "gid": 65535
            },
            "args": [
                "/pause"
            ],
            "env": [
                "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
            ],
            "cwd": "/",
            "capabilities": {
                "bounding": [
                    "CAP_CHOWN",
                    "CAP_DAC_OVERRIDE",
                    "CAP_FSETID",
                    "CAP_FOWNER",
                    "CAP_MKNOD",
                    "CAP_NET_RAW",
                    "CAP_SETGID",
                    "CAP_SETUID",
                    "CAP_SETFCAP",
                    "CAP_SETPCAP",
                    "CAP_NET_BIND_SERVICE",
                    "CAP_SYS_CHROOT",
                    "CAP_KILL",
                    "CAP_AUDIT_WRITE"
                ],
                "effective": [
                    "CAP_CHOWN",
                    "CAP_DAC_OVERRIDE",
                    "CAP_FSETID",
                    "CAP_FOWNER",
                    "CAP_MKNOD",
                    "CAP_NET_RAW",
                    "CAP_SETGID",
                    "CAP_SETUID",
                    "CAP_SETFCAP",
                    "CAP_SETPCAP",
                    "CAP_NET_BIND_SERVICE",
                    "CAP_SYS_CHROOT",
                    "CAP_KILL",
                    "CAP_AUDIT_WRITE"
                ],
                "permitted": [
                    "CAP_CHOWN",
                    "CAP_DAC_OVERRIDE",
                    "CAP_FSETID",
                    "CAP_FOWNER",
                    "CAP_MKNOD",
                    "CAP_NET_RAW",
                    "CAP_SETGID",
                    "CAP_SETUID",
                    "CAP_SETFCAP",
                    "CAP_SETPCAP",
                    "CAP_NET_BIND_SERVICE",
                    "CAP_SYS_CHROOT",
                    "CAP_KILL",
                    "CAP_AUDIT_WRITE"
                ]
            },
            "noNewPrivileges": true,
            "oomScoreAdj": -998
        },
        "root": {
            "path": "/run/kata-containers/shared/containers/$(bundle-id)/rootfs",
            "readonly": true
        },
        "hostname": "busybox-cc",
        "mounts": [
            {
                "destination": "/proc",
                "type": "proc",
                "source": "proc",
                "options": [
                    "nosuid",
                    "noexec",
                    "nodev"
                ]
            },
            {
                "destination": "/dev",
                "type": "tmpfs",
                "source": "tmpfs",
                "options": [
                    "nosuid",
                    "strictatime",
                    "mode=755",
                    "size=65536k"
                ]
            },
            {
                "destination": "/dev/pts",
                "type": "devpts",
                "source": "devpts",
                "options": [
                    "nosuid",
                    "noexec",
                    "newinstance",
                    "ptmxmode=0666",
                    "mode=0620",
                    "gid=5"
                ]
            },
            {
                "destination": "/dev/shm",
                "type": "bind",
                "source": "/run/kata-containers/sandbox/shm",
                "options": [
                    "rbind"
                ]
            },
            {
                "destination": "/dev/mqueue",
                "type": "mqueue",
                "source": "mqueue",
                "options": [
                    "nosuid",
                    "noexec",
                    "nodev"
                ]
            },
            {
                "destination": "/sys",
                "type": "sysfs",
                "source": "sysfs",
                "options": [
                    "nosuid",
                    "noexec",
                    "nodev",
                    "ro"
                ]
            },
            {
                "destination": "/dev/shm",
                "type": "bind",
                "source": "/run/kata-containers/sandbox/shm",
                "options": [
                    "rbind"
                ]
            },
            {
                "destination": "/etc/resolv.conf",
                "type": "bind",
                "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-resolv.conf$",
                "options": [
                    "rbind",
                    "ro"
                ]
            }
        ],
        "annotations": {
            "io.kubernetes.cri.sandbox-id": "^[a-z0-9]{64}$",
            "io.kubernetes.cri.container-type": "sandbox",
            "io.kubernetes.cri.sandbox-memory": "0",
            "nerdctl/network-namespace": "^/var/run/netns/cni-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
            "io.kubernetes.cri.sandbox-log-directory": "^/var/log/pods/default_$(sandbox-name)_[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
            "io.kubernetes.cri.sandbox-cpu-shares": "2",
            "io.katacontainers.pkg.oci.container_type": "pod_sandbox",
            "io.kubernetes.cri.sandbox-namespace": "default",
            "io.kubernetes.cri.sandbox-name": "busybox-cc",
            "io.kubernetes.cri.sandbox-cpu-period": "100000",
            "io.kubernetes.cri.sandbox-cpu-quota": "0",
            "io.katacontainers.pkg.oci.bundle_path": "/run/containerd/io.containerd.runtime.v2.task/k8s.io/$(bundle-id)"
        },
        "linux": {
            "resources": {
                "cpu": {
                    "shares": 2,
                    "quota": 0,
                    "period": 0,
                    "realtimeRuntime": 0,
                    "realtimePeriod": 0
                }
            },
            "cgroupsPath": "/kubepods/besteffort/pod47f1fbee-9c44-4968-8a6a-373887167617/521dcee15a4b51edb91f5678d61372d7516e2efa045d9704c9fb1b433a4d41b4",
            "namespaces": [
                {
                    "type": "ipc"
                },
                {
                    "type": "uts"
                },
                {
                    "type": "mount"
                }
            ],
            "maskedPaths": [
                "/proc/acpi",
                "/proc/asound",
                "/proc/kcore",
                "/proc/keys",
                "/proc/latency_stats",
                "/proc/timer_list",
                "/proc/timer_stats",
                "/proc/sched_debug",
                "/sys/firmware",
                "/proc/scsi"
            ],
            "readonlyPaths": [
                "/proc/bus",
                "/proc/fs",
                "/proc/irq",
                "/proc/sys",
                "/proc/sysrq-trigger"
            ]
        }
    },
    {
        "ociVersion": "1.0.2-dev",
        "process": {
            "terminal": false,
            "user": {
                "uid": 0,
                "gid": 0
            },
            "env": [
                "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                "HOSTNAME=busybox-cc",
                "KUBERNETES_PORT_443_TCP_PROTO=tcp",
                "KUBERNETES_PORT_443_TCP_PORT=443",
                "KUBERNETES_PORT_443_TCP_ADDR=10.0.0.1",
                "KUBERNETES_SERVICE_HOST=10.0.0.1",
                "KUBERNETES_SERVICE_PORT=443",
                "KUBERNETES_SERVICE_PORT_HTTPS=443",
                "KUBERNETES_PORT=tcp://10.0.0.1:443",
                "KUBERNETES_PORT_443_TCP=tcp://10.0.0.1:443"
            ],
            "cwd": "/",
            "capabilities": {
                "bounding": [
                    "CAP_CHOWN",
                    "CAP_DAC_OVERRIDE",
                    "CAP_FSETID",
                    "CAP_FOWNER",
                    "CAP_MKNOD",
                    "CAP_NET_RAW",
                    "CAP_SETGID",
                    "CAP_SETUID",
                    "CAP_SETFCAP",
                    "CAP_SETPCAP",
                    "CAP_NET_BIND_SERVICE",
                    "CAP_SYS_CHROOT",
                    "CAP_KILL",
                    "CAP_AUDIT_WRITE"
                ],
                "effective": [
                    "CAP_CHOWN",
                    "CAP_DAC_OVERRIDE",
                    "CAP_FSETID",
                    "CAP_FOWNER",
                    "CAP_MKNOD",
                    "CAP_NET_RAW",
                    "CAP_SETGID",
                    "CAP_SETUID",
                    "CAP_SETFCAP",
                    "CAP_SETPCAP",
                    "CAP_NET_BIND_SERVICE",
                    "CAP_SYS_CHROOT",
                    "CAP_KILL",
                    "CAP_AUDIT_WRITE"
                ],
                "permitted": [
                    "CAP_CHOWN",
                    "CAP_DAC_OVERRIDE",
                    "CAP_FSETID",
                    "CAP_FOWNER",
                    "CAP_MKNOD",
                    "CAP_NET_RAW",
                    "CAP_SETGID",
                    "CAP_SETUID",
                    "CAP_SETFCAP",
                    "CAP_SETPCAP",
                    "CAP_NET_BIND_SERVICE",
                    "CAP_SYS_CHROOT",
                    "CAP_KILL",
                    "CAP_AUDIT_WRITE"
                ]
            },
            "noNewPrivileges": false,
            "apparmorProfile": "cri-containerd.apparmor.d",
            "oomScoreAdj": 1000
        },
        "root": {
            "path": "/run/kata-containers/shared/containers/$(bundle-id)/rootfs",
            "readonly": false
        },
        "mounts": [
            {
                "destination": "/proc",
                "type": "proc",
                "source": "proc",
                "options": [
                    "nosuid",
                    "noexec",
                    "nodev"
                ]
            },
            {
                "destination": "/dev",
                "type": "tmpfs",
                "source": "tmpfs",
                "options": [
                    "nosuid",
                    "strictatime",
                    "mode=755",
                    "size=65536k"
                ]
            },
            {
                "destination": "/dev/pts",
                "type": "devpts",
                "source": "devpts",
                "options": [
                    "nosuid",
                    "noexec",
                    "newinstance",
                    "ptmxmode=0666",
                    "mode=0620",
                    "gid=5"
                ]
            },
            {
                "destination": "/dev/mqueue",
                "type": "mqueue",
                "source": "mqueue",
                "options": [
                    "nosuid",
                    "noexec",
                    "nodev"
                ]
            },
            {
                "destination": "/sys",
                "type": "sysfs",
                "source": "sysfs",
                "options": [
                    "nosuid",
                    "noexec",
                    "nodev",
                    "ro"
                ]
            },
            {
                "destination": "/sys/fs/cgroup",
                "type": "cgroup",
                "source": "cgroup",
                "options": [
                    "nosuid",
                    "noexec",
                    "nodev",
                    "relatime",
                    "ro"
                ]
            },
            {
                "destination": "/etc/hosts",
                "type": "bind",
                "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-hosts$",
                "options": [
                    "rbind",
                    "rprivate",
                    "rw"
                ]
            },
            {
                "destination": "/dev/termination-log",
                "type": "bind",
                "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-termination-log$",
                "options": [
                    "rbind",
                    "rprivate",
                    "rw"
                ]
            },
            {
                "destination": "/etc/hostname",
                "type": "bind",
                "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-hostname$",
                "options": [
                    "rbind",
                    "rprivate",
                    "rw"
                ]
            },
            {
                "destination": "/etc/resolv.conf",
                "type": "bind",
                "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-resolv.conf$",
                "options": [
                    "rbind",
                    "rprivate",
                    "rw"
                ]
            },
            {
                "destination": "/dev/shm",
                "type": "bind",
                "source": "/run/kata-containers/sandbox/shm",
                "options": [
                    "rbind"
                ]
            },
            {
                "destination": "/var/run/secrets/kubernetes.io/serviceaccount",
                "type": "bind",
                "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-serviceaccount$",
                "options": [
                    "rbind",
                    "rprivate",
                    "ro"
                ]
            }
        ],
        "annotations": {
            "io.kubernetes.cri.image-name": "docker.io/library/busybox:latest",
            "io.kubernetes.cri.container-name": "busybox",
            "io.katacontainers.pkg.oci.bundle_path": "/run/containerd/io.containerd.runtime.v2.task/k8s.io/$(bundle-id)",
            "io.kubernetes.cri.sandbox-id": "^[a-z0-9]{64}$",
            "io.katacontainers.pkg.oci.container_type": "pod_container",
            "io.kubernetes.cri.container-type": "container",
            "io.kubernetes.cri.sandbox-namespace": "default",
            "io.kubernetes.cri.sandbox-name": "busybox-cc"
        },
        "linux": {
            "resources": {
                "memory": {
                    "limit": 0,
                    "reservation": 0,
                    "swap": 0,
                    "kernel": 0,
                    "kernelTCP": 0,
                    "swappiness": 0,
                    "disableOOMKiller": false
                },
                "cpu": {
                    "shares": 2,
                    "quota": 0,
                    "period": 100000,
                    "realtimeRuntime": 0,
                    "realtimePeriod": 0
                }
            },
            "cgroupsPath": "/kubepods/besteffort/pod47f1fbee-9c44-4968-8a6a-373887167617/$(bundle-id)",
            "namespaces": [
                {
                    "type": "ipc"
                },
                {
                    "type": "uts"
                },
                {
                    "type": "mount"
                }
            ],
            "maskedPaths": [
                "/proc/acpi",
                "/proc/kcore",
                "/proc/keys",
                "/proc/latency_stats",
                "/proc/timer_list",
                "/proc/timer_stats",
                "/proc/sched_debug",
                "/proc/scsi",
                "/sys/firmware"
            ],
            "readonlyPaths": [
                "/proc/asound",
                "/proc/bus",
                "/proc/fs",
                "/proc/irq",
                "/proc/sys",
                "/proc/sysrq-trigger"
            ]
        }
    }
]
