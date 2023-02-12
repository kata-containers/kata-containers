package coco_policy

import future.keywords.in
import future.keywords.every

default AddARPNeighborsRequest := true
default AddSwapRequest := true
default CloseStdinRequest := true

# Seems risky for confidentiality.
#default CopyFileRequest := false

# Detailed policy rules are below.
default CreateContainerRequest := false

# Required.
default CreateSandboxRequest := true

# No apparent reasons to reject this request.
default DestroySandboxRequest := true

# Haven't found a use case for it.
#default ExecProcessRequest := false

# Useful for diagnostics, but can be disabled in production.
default GetMetricsRequest := false

# No apparent reasons to reject these requests.
default GetOOMEventRequest := true
default GuestDetailsRequest := true

# Haven't found use cases for these.
#default ListInterfacesRequest := false
#default ListRoutesRequest := false
#default MemHotplugByProbeRequest := false

# Is there a good reason to reject OnlineCPUMemRequest?
default OnlineCPUMemRequest := true

# Haven't found a use case for it.
#default PauseContainerRequest := false

# Image service should make is_allowed!() calls.
#
# Might use policy metadata to reject images that were
# not referenced by config.json.
#default PullImageRequest := true

# Could check that "terminal": true.
default ReadStreamRequest := true

# Could validate container_id and/or timeout.
default RemoveContainerRequest := true

# Haven't found a use case for these.
#default ReseedRandomDevRequest := false
#default ResumeContainerRequest := false
#default SetGuestDateTimeRequest := false

# Could validate container_id and/or exec_id.
default SignalProcessRequest := true

# Could validate container_id.
default StartContainerRequest := true

# Not found in agent.proto.
#default StartTracingRequest := false

# Could validate container_id.
# Could disable if ctr + containerd don't need these stats.
default StatsContainerRequest := true

# Not found in agent.proto.
#default StopTracingRequest := false

# Could check that "terminal": true.
default TtyWinResizeRequest := true

# Haven't found a use case for it.
#default UpdateContainerRequest := false

# Could validate the format and/or consistency of fields.
default UpdateInterfaceRequest := true

# Could validate the format and/or consistency of fields.
default UpdateRoutesRequest := true

# Could validate container_id and/or exec_id.
default WaitProcessRequest := true

# Could check that "terminal": true.
default WriteStreamRequest := true

CreateContainerRequest {
    input_container := input.oci
    input_index := input.index
    input_index == 0

    policy_container := policy_containers[input_index]

    policy_container.ociVersion     == input_container.ociVersion

    allow_annotations(policy_container, input_container)

    policy_process := policy_container.process
    input_process := input_container.process

    policy_process.terminal         == input_process.terminal
    policy_process.user             == input_process.user
    policy_process.args             == input_process.args

    # Ignore any policy environment variables that are not
    # present in the input.
    every env_var in input_process.env {
        policy_process.env[_] == env_var
    }

    policy_process.cwd              == input_process.cwd
    policy_process.capabilities     == input_process.capabilities
    policy_process.rlimits          == input_process.rlimits
    policy_process.noNewPrivileges  == input_process.noNewPrivileges
    policy_process.oomScoreAdj      == input_process.oomScoreAdj
   
    regex.match(policy_container.root.path, input_container.root.path)
    policy_container.root.readonly  == input_container.root.readonly

    policy_container.mounts         == input_container.mounts
    allow_linux(policy_container, input_container)
}

######################################################################
# No annotations allowed for ctr based containers.

allow_annotations(policy_container, input_container) {
    not policy_container.annotations
    not input_container.annotations
}

######################################################################
# linux fields

allow_linux(policy_container, input_container) {
    policy_container.linux.namespaces == input_container.linux.namespaces
    policy_container.linux.maskedPaths == input_container.linux.maskedPaths
    policy_container.linux.readonlyPaths == input_container.linux.readonlyPaths
}

######################################################################
policy_containers := [
    {
        "ociVersion": "1.0.2-dev",
        "process": {
            "terminal": true,
            "user": {
                "uid": 0,
                "gid": 0,
                "additionalGids": [
                    10
                ]
            },
            "args": [
                "/bin/sh"
            ],
            "env": [
                "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                "TERM=xterm"
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
            "rlimits": [
                {
                    "type": "RLIMIT_NOFILE",
                    "hard": 1024,
                    "soft": 1024
                }
            ],
            "noNewPrivileges": true,
            "oomScoreAdj": 0
        },
        "root": {
            "path": "^/run/kata-containers/shared/containers/[a-zA-Z0-9]*/rootfs$",
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
                "destination": "/run",
                "type": "tmpfs",
                "source": "tmpfs",
                "options": [
                    "nosuid",
                    "strictatime",
                    "mode=755",
                    "size=65536k"
                ]
            }
        ],
        "linux": {
            "resources": {
                "cpu": {
                    "shares": 1024,
                    "quota": 0,
                    "period": 0,
                    "realtimeRuntime": 0,
                    "realtimePeriod": 0
                }
            },
            "cgroupsPath": "/default/hello6",
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
    }
]
