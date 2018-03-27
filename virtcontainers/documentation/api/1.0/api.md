# virtcontainers 1.0 API

The virtcontainers 1.0 API operates on two high level objects:
[Pods](#pod-api) and [containers](#container-api):

* [Pod API](#pod-api)
* [Container API](#container-api)
* [Examples](#examples)

## Pod API

The virtcontainers 1.0 pod API manages hardware virtualized
[pod lifecycles](#pod-functions). The virtcontainers pod
semantics strictly follow the
[Kubernetes](https://kubernetes.io/docs/concepts/workloads/pods/pod/) ones.

The pod API allows callers to [create](#createpod), [delete](#deletepod),
[start](#startpod), [stop](#stoppod), [run](#runpod), [pause](#pausepod),
[resume](resumepod) and [list](#listpod) VM (Virtual Machine) based pods.

To initially create a pod, the API caller must prepare a
[`PodConfig`](#podconfig) and pass it to either [`CreatePod`](#createpod)
or [`RunPod`](#runpod). Upon successful pod creation, the virtcontainers
API will return a [`VCPod`](#vcpod) interface back to the caller.

The `VCPod` interface is a pod abstraction hiding the internal and private
virtcontainers pod structure. It is a handle for API callers to manage the
pod lifecycle through the rest of the [pod API](#pod-functions).

* [Structures](#pod-structures)
* [Functions](#pod-functions)

### Pod Structures

* [PodConfig](#podconfig)
  * [Resources](#resources)
  * [HypervisorType](#hypervisortype)
  * [HypervisorConfig](#hypervisorconfig)
  * [AgentType](#agenttype)
  * [ProxyType](#proxytype)
  * [ProxyConfig](#proxyconfig)
  * [ShimType](#shimtype)
  * [NetworkModel](#networkmodel)
  * [NetworkConfig](#networkconfig)
    * [NetInterworkingModel](#netinterworkingmodel)
  * [Volume](#volume)
  * [ContainerConfig](#containerconfig)
    * [Cmd](#cmd)
    * [Mount](#mount)
    * [DeviceInfo](#deviceinfo)
* [VCPod](#vcpod)

#### `PodConfig`
```Go
// PodConfig is a Pod configuration.
type PodConfig struct {
	ID string

	Hostname string

	// Field specific to OCI specs, needed to setup all the hooks
	Hooks Hooks

	// VMConfig is the VM configuration to set for this pod.
	VMConfig Resources

	HypervisorType   HypervisorType
	HypervisorConfig HypervisorConfig

	AgentType   AgentType
	AgentConfig interface{}

	ProxyType   ProxyType
	ProxyConfig ProxyConfig

	ShimType   ShimType
	ShimConfig interface{}

	NetworkModel  NetworkModel
	NetworkConfig NetworkConfig

	// Volumes is a list of shared volumes between the host and the Pod.
	Volumes []Volume

	// Containers describe the list of containers within a Pod.
	// This list can be empty and populated by adding containers
	// to the Pod a posteriori.
	Containers []ContainerConfig

	// Annotations keys must be unique strings and must be name-spaced
	// with e.g. reverse domain notation (org.clearlinux.key).
	Annotations map[string]string
}
```
##### `Resources`
```Go
// Resources describes VM resources configuration.
type Resources struct {
	// VCPUs is the number of available virtual CPUs.
	VCPUs uint

	// Memory is the amount of available memory in MiB.
	Memory uint
}
```

##### `HypervisorType`
```Go
// HypervisorType describes an hypervisor type.
type HypervisorType string

const (
	// QemuHypervisor is the QEMU hypervisor.
	QemuHypervisor HypervisorType = "qemu"

	// MockHypervisor is a mock hypervisor for testing purposes
	MockHypervisor HypervisorType = "mock"
)
```

##### `HypervisorConfig`
```Go
// HypervisorConfig is the hypervisor configuration.
type HypervisorConfig struct {
	// KernelPath is the guest kernel host path.
	KernelPath string

	// ImagePath is the guest image host path.
	ImagePath string

	// FirmwarePath is the bios host path
	FirmwarePath string

	// MachineAccelerators are machine specific accelerators
	MachineAccelerators string

	// HypervisorPath is the hypervisor executable host path.
	HypervisorPath string

	// DisableBlockDeviceUse disallows a block device from being used.
	DisableBlockDeviceUse bool

	// KernelParams are additional guest kernel parameters.
	KernelParams []Param

	// HypervisorParams are additional hypervisor parameters.
	HypervisorParams []Param

	// HypervisorMachineType specifies the type of machine being
	// emulated.
	HypervisorMachineType string

	// Debug changes the default hypervisor and kernel parameters to
	// enable debug output where available.
	Debug bool

	// DefaultVCPUs specifies default number of vCPUs for the VM.
	// Pod configuration VMConfig.VCPUs overwrites this.
	DefaultVCPUs uint32

	// DefaultMem specifies default memory size in MiB for the VM.
	// Pod configuration VMConfig.Memory overwrites this.
	DefaultMemSz uint32

	// DefaultBridges specifies default number of bridges for the VM.
	// Bridges can be used to hot plug devices
	DefaultBridges uint32

	// MemPrealloc specifies if the memory should be pre-allocated
	MemPrealloc bool

	// HugePages specifies if the memory should be pre-allocated from huge pages
	HugePages bool

	// Realtime Used to enable/disable realtime
	Realtime bool

	// Mlock is used to control memory locking when Realtime is enabled
	// Realtime=true and Mlock=false, allows for swapping out of VM memory
	// enabling higher density
	Mlock bool

	// DisableNestingChecks is used to override customizations performed
	// when running on top of another VMM.
	DisableNestingChecks bool
}
```

##### `AgentType`
```Go
// AgentType describes the type of guest agent a Pod should run.
type AgentType string

const (
	// NoopAgentType is the No-Op agent.
	NoopAgentType AgentType = "noop"

	// HyperstartAgent is the Hyper hyperstart agent.
	HyperstartAgent AgentType = "hyperstart"

	// KataContainersAgent is the Kata Containers agent.
	KataContainersAgent AgentType = "kata"

	// SocketTypeVSOCK is a VSOCK socket type for talking to an agent.
	SocketTypeVSOCK = "vsock"

	// SocketTypeUNIX is a UNIX socket type for talking to an agent.
	// It typically means the agent is living behind a host proxy.
	SocketTypeUNIX = "unix"
)
```

##### `ProxyType`
```Go
// ProxyType describes a proxy type.
type ProxyType string

const (
	// NoopProxyType is the noopProxy.
	NoopProxyType ProxyType = "noopProxy"

	// NoProxyType is the noProxy.
	NoProxyType ProxyType = "noProxy"

	// CCProxyType is the ccProxy.
	CCProxyType ProxyType = "ccProxy"

	// KataProxyType is the kataProxy.
	KataProxyType ProxyType = "kataProxy"
)
```

##### `ProxyConfig`
```Go
// ProxyConfig is a structure storing information needed from any
// proxy in order to be properly initialized.
type ProxyConfig struct {
	Path  string
	Debug bool
}
```

##### `ShimType`
```Go
// ShimType describes a shim type.
type ShimType string

const (
	// CCShimType is the ccShim.
	CCShimType ShimType = "ccShim"

	// NoopShimType is the noopShim.
	NoopShimType ShimType = "noopShim"

	// KataShimType is the Kata Containers shim type.
	KataShimType ShimType = "kataShim"
)
```

##### `NetworkModel`
```Go
// NetworkModel describes the type of network specification.
type NetworkModel string

const (
	// NoopNetworkModel is the No-Op network.
	NoopNetworkModel NetworkModel = "noop"

	// CNINetworkModel is the CNI network.
	CNINetworkModel NetworkModel = "CNI"

	// CNMNetworkModel is the CNM network.
	CNMNetworkModel NetworkModel = "CNM"
)
```

##### `NetworkConfig`
```Go
// NetworkConfig is the network configuration related to a network.
type NetworkConfig struct {
	NetNSPath         string
	NumInterfaces     int
	InterworkingModel NetInterworkingModel
}
```
###### `NetInterworkingModel`
```Go
// NetInterworkingModel defines the network model connecting
// the network interface to the virtual machine.
type NetInterworkingModel int

const (
	// NetXConnectDefaultModel Ask to use DefaultNetInterworkingModel
	NetXConnectDefaultModel NetInterworkingModel = iota

	// NetXConnectBridgedModel uses a linux bridge to interconnect
	// the container interface to the VM. This is the
	// safe default that works for most cases except
	// macvlan and ipvlan
	NetXConnectBridgedModel

	// NetXConnectMacVtapModel can be used when the Container network
	// interface can be bridged using macvtap
	NetXConnectMacVtapModel

	// NetXConnectEnlightenedModel can be used when the Network plugins
	// are enlightened to create VM native interfaces
	// when requested by the runtime
	// This will be used for vethtap, macvtap, ipvtap
	NetXConnectEnlightenedModel

	// NetXConnectInvalidModel is the last item to check valid values by IsValid()
	NetXConnectInvalidModel
)
```

##### `Volume`
```Go
// Volume is a shared volume between the host and the VM,
// defined by its mount tag and its host path.
type Volume struct {
	// MountTag is a label used as a hint to the guest.
	MountTag string

	// HostPath is the host filesystem path for this volume.
	HostPath string
}
```

##### `ContainerConfig`
```Go
// ContainerConfig describes one container runtime configuration.
type ContainerConfig struct {
	ID string

	// RootFs is the container workload image on the host.
	RootFs string

	// ReadOnlyRootfs indicates if the rootfs should be mounted readonly
	ReadonlyRootfs bool

	// Cmd specifies the command to run on a container
	Cmd Cmd

	// Annotations allow clients to store arbitrary values,
	// for example to add additional status values required
	// to support particular specifications.
	Annotations map[string]string

	Mounts []Mount

	// Device configuration for devices that must be available within the container.
	DeviceInfos []DeviceInfo
}
```

###### `Cmd`
```Go
// Cmd represents a command to execute in a running container.
type Cmd struct {
	Args    []string
	Envs    []EnvVar
	WorkDir string

	// Note that these fields *MUST* remain as strings.
	//
	// The reason being that we want runtimes to be able to support CLI
	// operations like "exec --user=". That option allows the
	// specification of a user (either as a string username or a numeric
	// UID), and may optionally also include a group (groupame or GID).
	//
	// Since this type is the interface to allow the runtime to specify
	// the user and group the workload can run as, these user and group
	// fields cannot be encoded as integer values since that would imply
	// the runtime itself would need to perform a UID/GID lookup on the
	// user-specified username/groupname. But that isn't practically
	// possible given that to do so would require the runtime to access
	// the image to allow it to interrogate the appropriate databases to
	// convert the username/groupnames to UID/GID values.
	//
	// Note that this argument applies solely to the _runtime_ supporting
	// a "--user=" option when running in a "standalone mode" - there is
	// no issue when the runtime is called by a container manager since
	// all the user and group mapping is handled by the container manager
	// and specified to the runtime in terms of UID/GID's in the
	// configuration file generated by the container manager.
	User                string
	PrimaryGroup        string
	SupplementaryGroups []string

	Interactive     bool
	Console         string
	Detach          bool
	NoNewPrivileges bool
	Capabilities    LinuxCapabilities
}
```

###### `Mount`
```Go
// Mount describes a container mount.
type Mount struct {
	Source      string
	Destination string

	// Type specifies the type of filesystem to mount.
	Type string

	// Options list all the mount options of the filesystem.
	Options []string

	// HostPath used to store host side bind mount path
	HostPath string

	// ReadOnly specifies if the mount should be read only or not
	ReadOnly bool
}
```

###### `DeviceInfo`
```Go
// DeviceInfo is an embedded type that contains device data common to all types of devices.
type DeviceInfo struct {
	// Device path on host
	HostPath string

	// Device path inside the container
	ContainerPath string

	// Type of device: c, b, u or p
	// c , u - character(unbuffered)
	// p - FIFO
	// b - block(buffered) special file
	// More info in mknod(1).
	DevType string

	// Major, minor numbers for device.
	Major int64
	Minor int64

	// FileMode permission bits for the device.
	FileMode os.FileMode

	// id of the device owner.
	UID uint32

	// id of the device group.
	GID uint32

	// Hotplugged is used to store device state indicating if the
	// device was hotplugged.
	Hotplugged bool

	// ID for the device that is passed to the hypervisor.
	ID string
}
```

#### `VCPod`
```Go
// VCPod is the Pod interface
// (required since virtcontainers.Pod only contains private fields)
type VCPod interface {
	Annotations(key string) (string, error)
	GetAllContainers() []VCContainer
	GetAnnotations() map[string]string
	GetContainer(containerID string) VCContainer
	ID() string
	SetAnnotations(annotations map[string]string) error
}
```

### Pod Functions

* [CreatePod](#createpod)
* [DeletePod](#deletepod)
* [StartPod](#startpod)
* [StopPod](#stoppod)
* [RunPod](#runpod)
* [ListPod](#listpod)
* [StatusPod](#statuspod)
* [PausePod](#pausepod)
* [ResumePod](#resumepod)

#### `CreatePod`
```Go
// CreatePod is the virtcontainers pod creation entry point.
// CreatePod creates a pod and its containers. It does not start them.
func CreatePod(podConfig PodConfig) (VCPod, error)
```

#### `DeletePod`
```Go
// DeletePod is the virtcontainers pod deletion entry point.
// DeletePod will stop an already running container and then delete it.
func DeletePod(podID string) (VCPod, error)
```

#### `StartPod`
```Go
// StartPod is the virtcontainers pod starting entry point.
// StartPod will talk to the given hypervisor to start an existing
// pod and all its containers.
func StartPod(podID string) (VCPod, error)
```

#### `StopPod`
```Go
// StopPod is the virtcontainers pod stopping entry point.
// StopPod will talk to the given agent to stop an existing pod
// and destroy all containers within that pod.
func StopPod(podID string) (VCPod, error)
```

#### `RunPod`
```Go
// RunPod is the virtcontainers pod running entry point.
// RunPod creates a pod and its containers and then it starts them.
func RunPod(podConfig PodConfig) (VCPod, error)
```

#### `ListPod`
```Go
// ListPod is the virtcontainers pod listing entry point.
func ListPod() ([]PodStatus, error)
```

#### `StatusPod`
```Go
// StatusPod is the virtcontainers pod status entry point.
func StatusPod(podID string) (PodStatus, error)
```

#### `PausePod`
```Go
// PausePod is the virtcontainers pausing entry point which pauses an
// already running pod.
func PausePod(podID string) (VCPod, error)
```

#### `ResumePod`
```Go
// ResumePod is the virtcontainers resuming entry point which resumes
// (or unpauses) and already paused pod.
func ResumePod(podID string) (VCPod, error)
```

## Container API

The virtcontainers 1.0 container API manages pod
[container lifecycles](#container-functions).

A virtcontainers container is process running inside a containerized
environment, as part of a hardware virtualized context. In other words,
a virtcontainers container is just a regular container running inside a
virtual machine's guest OS.

A virtcontainers container always belong to one and only one
virtcontainers pod, again following the
[Kubernetes](https://kubernetes.io/docs/concepts/workloads/pods/pod-overview/)
logic and semantics.

The container API allows callers to [create](#createcontainer),
[delete](#deletecontainer), [start](#startcontainer), [stop](#stopcontainer),
[kill](#killcontainer) and [observe](#statuscontainer) containers. It also
allows for running [additional processes](#entercontainer) inside a
specific container.

As a virtcontainers container is always linked to a pod, the entire container
API always takes a pod ID as its first argument.

To create a container, the API caller must prepare a
[`ContainerConfig`](#containerconfig) and pass it to
[`CreateContainer`](#createcontainer) together with a pod ID. Upon successful
container creation, the virtcontainers API will return a
[`VCContainer`](#vccontainer) interface back to the caller.

The `VCContainer` interface is a container abstraction hiding the internal
and private virtcontainers container structure. It is a handle for API callers
to manage the container lifecycle through the rest of the
[container API](#container-functions).

* [Structures](#container-structures)
* [Functions](#container-functions)

### Container Structures

* [ContainerConfig](#containerconfig-1)
  * [Cmd](#cmd-1)
  * [Mount](#mount-1)
  * [DeviceInfo](#deviceinfo-1)
* [Process](#process)
* [ContainerStatus](#containerstatus)
* [ProcessListOptions](#processlistoptions)
* [VCContainer](#vccontainer)


#### `ContainerConfig`
```Go
// ContainerConfig describes one container runtime configuration.
type ContainerConfig struct {
	ID string

	// RootFs is the container workload image on the host.
	RootFs string

	// ReadOnlyRootfs indicates if the rootfs should be mounted readonly
	ReadonlyRootfs bool

	// Cmd specifies the command to run on a container
	Cmd Cmd

	// Annotations allow clients to store arbitrary values,
	// for example to add additional status values required
	// to support particular specifications.
	Annotations map[string]string

	Mounts []Mount

	// Device configuration for devices that must be available within the container.
	DeviceInfos []DeviceInfo
}
```

##### `Cmd`
```Go
// Cmd represents a command to execute in a running container.
type Cmd struct {
	Args    []string
	Envs    []EnvVar
	WorkDir string

	// Note that these fields *MUST* remain as strings.
	//
	// The reason being that we want runtimes to be able to support CLI
	// operations like "exec --user=". That option allows the
	// specification of a user (either as a string username or a numeric
	// UID), and may optionally also include a group (groupame or GID).
	//
	// Since this type is the interface to allow the runtime to specify
	// the user and group the workload can run as, these user and group
	// fields cannot be encoded as integer values since that would imply
	// the runtime itself would need to perform a UID/GID lookup on the
	// user-specified username/groupname. But that isn't practically
	// possible given that to do so would require the runtime to access
	// the image to allow it to interrogate the appropriate databases to
	// convert the username/groupnames to UID/GID values.
	//
	// Note that this argument applies solely to the _runtime_ supporting
	// a "--user=" option when running in a "standalone mode" - there is
	// no issue when the runtime is called by a container manager since
	// all the user and group mapping is handled by the container manager
	// and specified to the runtime in terms of UID/GID's in the
	// configuration file generated by the container manager.
	User                string
	PrimaryGroup        string
	SupplementaryGroups []string

	Interactive     bool
	Console         string
	Detach          bool
	NoNewPrivileges bool
	Capabilities    LinuxCapabilities
}
```

##### `Mount`
```Go
// Mount describes a container mount.
type Mount struct {
	Source      string
	Destination string

	// Type specifies the type of filesystem to mount.
	Type string

	// Options list all the mount options of the filesystem.
	Options []string

	// HostPath used to store host side bind mount path
	HostPath string

	// ReadOnly specifies if the mount should be read only or not
	ReadOnly bool
}
```

##### `DeviceInfo`
```Go
// DeviceInfo is an embedded type that contains device data common to all types of devices.
type DeviceInfo struct {
	// Device path on host
	HostPath string

	// Device path inside the container
	ContainerPath string

	// Type of device: c, b, u or p
	// c , u - character(unbuffered)
	// p - FIFO
	// b - block(buffered) special file
	// More info in mknod(1).
	DevType string

	// Major, minor numbers for device.
	Major int64
	Minor int64

	// FileMode permission bits for the device.
	FileMode os.FileMode

	// id of the device owner.
	UID uint32

	// id of the device group.
	GID uint32

	// Hotplugged is used to store device state indicating if the
	// device was hotplugged.
	Hotplugged bool

	// ID for the device that is passed to the hypervisor.
	ID string
}
```

#### `Process`
```Go
// Process gathers data related to a container process.
type Process struct {
	// Token is the process execution context ID. It must be
	// unique per pod.
	// Token is used to manipulate processes for containers
	// that have not started yet, and later identify them
	// uniquely within a pod.
	Token string

	// Pid is the process ID as seen by the host software
	// stack, e.g. CRI-O, containerd. This is typically the
	// shim PID.
	Pid int

	StartTime time.Time
}
```

#### `ContainerStatus`
```Go
// ContainerStatus describes a container status.
type ContainerStatus struct {
	ID        string
	State     State
	PID       int
	StartTime time.Time
	RootFs    string

	// Annotations allow clients to store arbitrary values,
	// for example to add additional status values required
	// to support particular specifications.
	Annotations map[string]string
}
```

#### `ProcessListOptions`
```Go
// ProcessListOptions contains the options used to list running
// processes inside the container
type ProcessListOptions struct {
	// Format describes the output format to list the running processes.
	// Formats are unrelated to ps(1) formats, only two formats can be specified:
	// "json" and "table"
	Format string

	// Args contains the list of arguments to run ps(1) command.
	// If Args is empty the agent will use "-ef" as options to ps(1).
	Args []string
}
```

#### `VCContainer`
```Go
// VCContainer is the Container interface
// (required since virtcontainers.Container only contains private fields)
type VCContainer interface {
	GetAnnotations() map[string]string
	GetPid() int
	GetToken() string
	ID() string
	Pod() VCPod
	Process() Process
	SetPid(pid int) error
}
```

### Container Functions

* [CreateContainer](#createcontainer)
* [DeleteContainer](#deletecontainer)
* [StartContainer](#startcontainer)
* [StopContainer](#stopcontainer)
* [EnterContainer](#entercontainer)
* [StatusContainer](#statuscontainer)
* [KillContainer](#killcontainer)
* [ProcessListContainer](#processlistcontainer)

#### `CreateContainer`
```Go
// CreateContainer is the virtcontainers container creation entry point.
// CreateContainer creates a container on a given pod.
func CreateContainer(podID string, containerConfig ContainerConfig) (VCPod, VCContainer, error)
```

#### `DeleteContainer`
```Go
// DeleteContainer is the virtcontainers container deletion entry point.
// DeleteContainer deletes a Container from a Pod. If the container is running,
// it needs to be stopped first.
func DeleteContainer(podID, containerID string) (VCContainer, error)
```

#### `StartContainer`
```Go
// StartContainer is the virtcontainers container starting entry point.
// StartContainer starts an already created container.
func StartContainer(podID, containerID string) (VCContainer, error)
```

#### `StopContainer`
```Go
// StopContainer is the virtcontainers container stopping entry point.
// StopContainer stops an already running container.
func StopContainer(podID, containerID string) (VCContainer, error)
```

#### `EnterContainer`
```Go
// EnterContainer is the virtcontainers container command execution entry point.
// EnterContainer enters an already running container and runs a given command.
func EnterContainer(podID, containerID string, cmd Cmd) (VCPod, VCContainer, *Process, error)
```

#### `StatusContainer`
```Go
// StatusContainer is the virtcontainers container status entry point.
// StatusContainer returns a detailed container status.
func StatusContainer(podID, containerID string) (ContainerStatus, error)
```

#### `KillContainer`
```Go
// KillContainer is the virtcontainers entry point to send a signal
// to a container running inside a pod. If all is true, all processes in
// the container will be sent the signal.
func KillContainer(podID, containerID string, signal syscall.Signal, all bool) error
```

#### `ProcessListContainer`
```Go
// ProcessListContainer is the virtcontainers entry point to list
// processes running inside a container
func ProcessListContainer(podID, containerID string, options ProcessListOptions) (ProcessList, error)
```

## Examples

### Preparing and running a pod

```Go

// This example creates and starts a single container pod,
// using qemu as the hypervisor and hyperstart as the VM agent.
func Example_createAndStartPod() {
	envs := []vc.EnvVar{
		{
			Var:   "PATH",
			Value: "/bin:/usr/bin:/sbin:/usr/sbin",
		},
	}

	cmd := vc.Cmd{
		Args:    strings.Split("/bin/sh", " "),
		Envs:    envs,
		WorkDir: "/",
	}

	// Define the container command and bundle.
	container := vc.ContainerConfig{
		ID:     "1",
		RootFs: containerRootfs,
		Cmd:    cmd,
	}

	// Sets the hypervisor configuration.
	hypervisorConfig := vc.HypervisorConfig{
		KernelPath:     "/usr/share/clear-containers/vmlinux.container",
		ImagePath:      "/usr/share/clear-containers/clear-containers.img",
		HypervisorPath: "/usr/bin/qemu-system-x86_64",
	}

	// Use hyperstart default values for the agent.
	agConfig := vc.HyperConfig{}

	// VM resources
	vmConfig := vc.Resources{
		VCPUs:  4,
		Memory: 1024,
	}

	// The pod configuration:
	// - One container
	// - Hypervisor is QEMU
	// - Agent is hyperstart
	podConfig := vc.PodConfig{
		VMConfig: vmConfig,

		HypervisorType:   vc.QemuHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   vc.HyperstartAgent,
		AgentConfig: agConfig,

		Containers: []vc.ContainerConfig{container},
	}

	_, err := vc.RunPod(podConfig)
	if err != nil {
		fmt.Printf("Could not run pod: %s", err)
	}

	return
}
```
