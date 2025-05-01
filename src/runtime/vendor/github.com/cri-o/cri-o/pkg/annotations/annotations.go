package annotations

import (
	"github.com/intel/goresctrl/pkg/rdt"
)

const (
	// UsernsMode is the user namespace mode to use
	UsernsModeAnnotation = "io.kubernetes.cri-o.userns-mode"

	// CgroupRW specifies mounting v2 cgroups as an rw filesystem.
	Cgroup2RWAnnotation = "io.kubernetes.cri-o.cgroup2-mount-hierarchy-rw"

	// UnifiedCgroupAnnotation specifies the unified configuration for cgroup v2
	UnifiedCgroupAnnotation = "io.kubernetes.cri-o.UnifiedCgroup"

	// SpoofedContainer indicates a container was spoofed in the runtime
	SpoofedContainer = "io.kubernetes.cri-o.Spoofed"

	// ShmSizeAnnotation is the K8S annotation used to set custom shm size
	ShmSizeAnnotation = "io.kubernetes.cri-o.ShmSize"

	// DevicesAnnotation is a set of devices to give to the container
	DevicesAnnotation = "io.kubernetes.cri-o.Devices"

	// CPULoadBalancingAnnotation indicates that load balancing should be disabled for CPUs used by the container
	CPULoadBalancingAnnotation = "cpu-load-balancing.crio.io"

	// CPUQuotaAnnotation indicates that CPU quota should be disabled for CPUs used by the container
	CPUQuotaAnnotation = "cpu-quota.crio.io"

	// IRQLoadBalancingAnnotation indicates that IRQ load balancing should be disabled for CPUs used by the container
	IRQLoadBalancingAnnotation = "irq-load-balancing.crio.io"

	// OCISeccompBPFHookAnnotation is the annotation used by the OCI seccomp BPF hook for tracing container syscalls
	OCISeccompBPFHookAnnotation = "io.containers.trace-syscall"

	// TrySkipVolumeSELinuxLabelAnnotation is the annotation used for optionally skipping relabeling a volume
	// with the specified SELinux label.  The relabeling will be skipped if the top layer is already labeled correctly.
	TrySkipVolumeSELinuxLabelAnnotation = "io.kubernetes.cri-o.TrySkipVolumeSELinuxLabel"

	// CPUCStatesAnnotation indicates that c-states should be enabled or disabled for CPUs used by the container
	CPUCStatesAnnotation = "cpu-c-states.crio.io"

	// CPUFreqGovernorAnnotation sets the cpufreq governor for CPUs used by the container
	CPUFreqGovernorAnnotation = "cpu-freq-governor.crio.io"

	// CPUSharedAnnotation indicate that a container which is part of a guaranteed QoS pod,
	// wants access to shared cpus.
	// the container name should be appended at the end of the annotation
	// example:  cpu-shared.crio.io/containerA
	CPUSharedAnnotation = "cpu-shared.crio.io"

	// SeccompNotifierActionAnnotation indicates a container is allowed to use the seccomp notifier feature.
	SeccompNotifierActionAnnotation = "io.kubernetes.cri-o.seccompNotifierAction"

	// UmaskAnnotation is the umask to use in the container init process
	UmaskAnnotation = "io.kubernetes.cri-o.umask"

	// SeccompNotifierActionStop indicates that a container should be stopped if used via the SeccompNotifierActionAnnotation key.
	SeccompNotifierActionStop = "stop"

	// PodLinuxOverhead indicates the overheads associated with the pod
	PodLinuxOverhead = "io.kubernetes.cri-o.PodLinuxOverhead"

	// PodLinuxResources indicates the sum of container resources for this pod
	PodLinuxResources = "io.kubernetes.cri-o.PodLinuxResources"

	// LinkLogsAnnotations indicates that CRI-O should link the pod containers logs into the specified
	// emptyDir volume
	LinkLogsAnnotation = "io.kubernetes.cri-o.LinkLogs"

	// PlatformRuntimePath indicates the runtime path that CRI-O should use for a specific platform.
	PlatformRuntimePath = "io.kubernetes.cri-o.PlatformRuntimePath"

	// SeccompProfileAnnotation can be used to set the seccomp profile for:
	// - a specific container by using: `seccomp-profile.kubernetes.cri-o.io/<CONTAINER_NAME>`
	// - a whole pod by using: `seccomp-profile.kubernetes.cri-o.io/POD`
	// Note that the annotation works on containers as well as on images.
	// For images, the plain annotation `seccomp-profile.kubernetes.cri-o.io`
	// can be used without the required `/POD` suffix or a container name.
	SeccompProfileAnnotation = "seccomp-profile.kubernetes.cri-o.io"

	// DisableFIPSAnnotation is used to disable FIPS mode for a pod within a FIPS-enabled Kubernetes cluster.
	DisableFIPSAnnotation = "io.kubernetes.cri-o.DisableFIPS"
)

var AllAllowedAnnotations = []string{
	UsernsModeAnnotation,
	Cgroup2RWAnnotation,
	UnifiedCgroupAnnotation,
	ShmSizeAnnotation,
	DevicesAnnotation,
	CPULoadBalancingAnnotation,
	CPUQuotaAnnotation,
	IRQLoadBalancingAnnotation,
	OCISeccompBPFHookAnnotation,
	rdt.RdtContainerAnnotation,
	TrySkipVolumeSELinuxLabelAnnotation,
	CPUCStatesAnnotation,
	CPUFreqGovernorAnnotation,
	SeccompNotifierActionAnnotation,
	UmaskAnnotation,
	PodLinuxOverhead,
	PodLinuxResources,
	LinkLogsAnnotation,
	CPUSharedAnnotation,
	SeccompProfileAnnotation,
	DisableFIPSAnnotation,
	// Keep in sync with
	// https://github.com/opencontainers/runc/blob/3db0871f1cf25c7025861ba0d51d25794cb21623/features.go#L67
	// Once runc 1.2 is released, we can use the `runc features` command to get this programmatically,
	// but we should hardcode these for now to prevent misuse.
	"bundle",
	"org.systemd.property.",
	"org.criu.config",

	// Simiarly, keep in sync with
	// https://github.com/containers/crun/blob/475a3fd0be/src/libcrun/container.c#L362-L366
	"module.wasm.image/variant",
	"io.kubernetes.cri.container-type",
	"run.oci.",
}
