# virtcontainers 1.0 API

The virtcontainers 1.0 API operates on two high level objects:
[Sandboxes](#sandbox-api) and [containers](#container-api):

* [Sandbox API](#sandbox-api)
* [Container API](#container-api)
* [Examples](#examples)

## Sandbox API

The virtcontainers 1.0 sandbox API manages hardware virtualized
[sandbox lifecycles](#sandbox-functions). The virtcontainers sandbox
semantics strictly follow the
[Kubernetes](https://kubernetes.io/docs/concepts/workloads/pods/pod-overview/) ones.

The sandbox API allows callers to [create](#createsandbox) VM (Virtual Machine) based sandboxes.

To initially create a sandbox, the API caller must prepare a
[`SandboxConfig`](#sandboxconfig) and pass it to either [`CreateSandbox`](#createsandbox). Upon successful sandbox creation, the virtcontainers
API will return a [`VCSandbox`](#vcsandbox) interface back to the caller.

The `VCSandbox` interface is a sandbox abstraction hiding the internal and private
virtcontainers sandbox structure. It is a handle for API callers to manage the
sandbox lifecycle through the rest of the [sandbox API](#sandbox-functions).

* [Structures](#sandbox-structures)
* [Functions](#sandbox-functions)

### Sandbox Structures

* [`SandboxConfig`](#sandboxconfig)
  * [`HypervisorType`](#hypervisortype)
  * [`HypervisorConfig`](#hypervisorconfig)
  * [`NetworkConfig`](#networkconfig)
    * [`NetInterworkingModel`](#netinterworkingmodel)
  * [`Volume`](#volume)
  * [`Sandbox ContainerConfig`](#sandbox-containerconfig)
    * [`Sandbox Cmd`](#sandbox-cmd)
    * [`Sandbox Mount`](#sandbox-mount)
    * [`Sandbox DeviceInfo`](#sandbox-deviceinfo)
* [`VCSandbox`](#vcsandbox)

#### `SandboxConfig`
```Go
// SandboxConfig is a Sandbox configuration.
type SandboxConfig struct {
	ID string

	Hostname string

	HypervisorType   HypervisorType
	HypervisorConfig HypervisorConfig

	AgentConfig KataAgentConfig

	NetworkConfig NetworkConfig

	// Volumes is a list of shared volumes between the host and the Sandbox.
	Volumes []types.Volume

	// Containers describe the list of containers within a Sandbox.
	// This list can be empty and populated by adding containers
	// to the Sandbox a posteriori.
	Containers []ContainerConfig

	// Annotations keys must be unique strings and must be name-spaced
	// with e.g. reverse domain notation (org.clearlinux.key).
	Annotations map[string]string

	ShmSize uint64

	// SharePidNs sets all containers to share the same sandbox level pid namespace.
	SharePidNs bool

	// SystemdCgroup enables systemd cgroup support
	SystemdCgroup bool

	// SandboxCgroupOnly enables cgroup only at podlevel in the host
	SandboxCgroupOnly bool

	DisableGuestSeccomp bool

	// Experimental features enabled
	Experimental []exp.Feature

	// Cgroups specifies specific cgroup settings for the various subsystems that the container is
	// placed into to limit the resources the container has available
	Cgroups *configs.Cgroup
}
```

##### `HypervisorType`
```Go
// HypervisorType describes an hypervisor type.
type HypervisorType string

const (
	// FirecrackerHypervisor is the FC hypervisor.
	FirecrackerHypervisor HypervisorType = "firecracker"

	// QemuHypervisor is the QEMU hypervisor.
	QemuHypervisor HypervisorType = "qemu"

	// AcrnHypervisor is the ACRN hypervisor.
	AcrnHypervisor HypervisorType = "acrn"

	// ClhHypervisor is the ICH hypervisor.
	ClhHypervisor HypervisorType = "clh"

	// MockHypervisor is a mock hypervisor for testing purposes
	MockHypervisor HypervisorType = "mock"
)
```

##### `HypervisorConfig`
```Go
// HypervisorConfig is the hypervisor configuration.
type HypervisorConfig struct {
	// NumVCPUs specifies default number of vCPUs for the VM.
	NumVCPUs uint32

	//DefaultMaxVCPUs specifies the maximum number of vCPUs for the VM.
	DefaultMaxVCPUs uint32

	// DefaultMem specifies default memory size in MiB for the VM.
	MemorySize uint32

	// DefaultBridges specifies default number of bridges for the VM.
	// Bridges can be used to hot plug devices
	DefaultBridges uint32

	// Msize9p is used as the msize for 9p shares
	Msize9p uint32

	// MemSlots specifies default memory slots the VM.
	MemSlots uint32

	// MemOffset specifies memory space for nvdimm device
	MemOffset uint32

	// VirtioFSCacheSize is the DAX cache size in MiB
	VirtioFSCacheSize uint32

	// KernelParams are additional guest kernel parameters.
	KernelParams []Param

	// HypervisorParams are additional hypervisor parameters.
	HypervisorParams []Param

	// KernelPath is the guest kernel host path.
	KernelPath string

	// ImagePath is the guest image host path.
	ImagePath string

	// InitrdPath is the guest initrd image host path.
	// ImagePath and InitrdPath cannot be set at the same time.
	InitrdPath string

	// FirmwarePath is the bios host path
	FirmwarePath string

	// MachineAccelerators are machine specific accelerators
	MachineAccelerators string

	// CPUFeatures are cpu specific features
	CPUFeatures string

	// HypervisorPath is the hypervisor executable host path.
	HypervisorPath string

	// HypervisorPathList is the list of hypervisor paths names allowed in annotations
	HypervisorPathList []string

	// HypervisorCtlPathList is the list of hypervisor control paths names allowed in annotations
	HypervisorCtlPathList []string

	// HypervisorCtlPath is the hypervisor ctl executable host path.
	HypervisorCtlPath string

	// JailerPath is the jailer executable host path.
	JailerPath string

	// JailerPathList is the list of jailer paths names allowed in annotations
	JailerPathList []string

	// BlockDeviceDriver specifies the driver to be used for block device
	// either VirtioSCSI or VirtioBlock with the default driver being defaultBlockDriver
	BlockDeviceDriver string

	// HypervisorMachineType specifies the type of machine being
	// emulated.
	HypervisorMachineType string

	// MemoryPath is the memory file path of VM memory. Used when either BootToBeTemplate or
	// BootFromTemplate is true.
	MemoryPath string

	// DevicesStatePath is the VM device state file path. Used when either BootToBeTemplate or
	// BootFromTemplate is true.
	DevicesStatePath string

	// EntropySource is the path to a host source of
	// entropy (/dev/random, /dev/urandom or real hardware RNG device)
	EntropySource string

	// Shared file system type:
	//   - virtio-9p (default)
	//   - virtio-fs
	SharedFS string

	// VirtioFSDaemon is the virtio-fs vhost-user daemon path
	VirtioFSDaemon string

	// VirtioFSDaemonList is the list of valid virtiofs names for annotations
	VirtioFSDaemonList []string

	// VirtioFSCache cache mode for fs version cache or "none"
	VirtioFSCache string

	// VirtioFSExtraArgs passes options to virtiofsd daemon
	VirtioFSExtraArgs []string

	// File based memory backend root directory
	FileBackedMemRootDir string

	// FileBackedMemRootList is the list of valid root directories values for annotations
	FileBackedMemRootList []string

	// PFlash image paths
	PFlash []string

	// customAssets is a map of assets.
	// Each value in that map takes precedence over the configured assets.
	// For example, if there is a value for the "kernel" key in this map,
	// it will be used for the sandbox's kernel path instead of KernelPath.
	customAssets map[types.AssetType]*types.Asset

	// BlockDeviceCacheSet specifies cache-related options will be set to block devices or not.
	BlockDeviceCacheSet bool

	// BlockDeviceCacheDirect specifies cache-related options for block devices.
	// Denotes whether use of O_DIRECT (bypass the host page cache) is enabled.
	BlockDeviceCacheDirect bool

	// BlockDeviceCacheNoflush specifies cache-related options for block devices.
	// Denotes whether flush requests for the device are ignored.
	BlockDeviceCacheNoflush bool

	// DisableBlockDeviceUse disallows a block device from being used.
	DisableBlockDeviceUse bool

	// EnableIOThreads enables IO to be processed in a separate thread.
	// Supported currently for virtio-scsi driver.
	EnableIOThreads bool

	// Debug changes the default hypervisor and kernel parameters to
	// enable debug output where available.
	Debug bool

	// MemPrealloc specifies if the memory should be pre-allocated
	MemPrealloc bool

	// HugePages specifies if the memory should be pre-allocated from huge pages
	HugePages bool

	// VirtioMem is used to enable/disable virtio-mem
	VirtioMem bool

	// IOMMU specifies if the VM should have a vIOMMU
	IOMMU bool

	// IOMMUPlatform is used to indicate if IOMMU_PLATFORM is enabled for supported devices
	IOMMUPlatform bool

	// Realtime Used to enable/disable realtime
	Realtime bool

	// Mlock is used to control memory locking when Realtime is enabled
	// Realtime=true and Mlock=false, allows for swapping out of VM memory
	// enabling higher density
	Mlock bool

	// DisableNestingChecks is used to override customizations performed
	// when running on top of another VMM.
	DisableNestingChecks bool

	// DisableImageNvdimm is used to disable guest rootfs image nvdimm devices
	DisableImageNvdimm bool

	// HotplugVFIOOnRootBus is used to indicate if devices need to be hotplugged on the
	// root bus instead of a bridge.
	HotplugVFIOOnRootBus bool

	// PCIeRootPort is used to indicate the number of PCIe Root Port devices
	// The PCIe Root Port device is used to hot-plug the PCIe device
	PCIeRootPort uint32

	// BootToBeTemplate used to indicate if the VM is created to be a template VM
	BootToBeTemplate bool

	// BootFromTemplate used to indicate if the VM should be created from a template VM
	BootFromTemplate bool

	// DisableVhostNet is used to indicate if host supports vhost_net
	DisableVhostNet bool

	// EnableVhostUserStore is used to indicate if host supports vhost-user-blk/scsi
	EnableVhostUserStore bool

	// VhostUserStorePath is the directory path where vhost-user devices
	// related folders, sockets and device nodes should be.
	VhostUserStorePath string

	// VhostUserStorePathList is the list of valid values for vhost-user paths
	VhostUserStorePathList []string

	// GuestHookPath is the path within the VM that will be used for 'drop-in' hooks
	GuestHookPath string

	// VMid is the id of the VM that create the hypervisor if the VM is created by the factory.
	// VMid is "" if the hypervisor is not created by the factory.
	VMid string

	// SELinux label for the VM
	SELinuxProcessLabel string

	// RxRateLimiterMaxRate is used to control network I/O inbound bandwidth on VM level.
	RxRateLimiterMaxRate uint64

	// TxRateLimiterMaxRate is used to control network I/O outbound bandwidth on VM level.
	TxRateLimiterMaxRate uint64

	// SGXEPCSize specifies the size in bytes for the EPC Section.
	// Enable SGX. Hardware-based isolation and memory encryption.
	SGXEPCSize int64

	// Enable annotations by name
	EnableAnnotations []string

	// GuestCoredumpPath is the path in host for saving guest memory dump
	GuestMemoryDumpPath string

	// GuestMemoryDumpPaging is used to indicate if enable paging
	// for QEMU dump-guest-memory command
	GuestMemoryDumpPaging bool
}
```

##### `NetworkConfig`
```Go
// NetworkConfig is the network configuration related to a network.
type NetworkConfig struct {
	NetNSPath         string
	NetNsCreated      bool
	DisableNewNetNs   bool
	NetmonConfig      NetmonConfig
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

	// NetXConnectMacVtapModel can be used when the Container network
	// interface can be bridged using macvtap
	NetXConnectMacVtapModel

	// NetXConnectTCFilterModel redirects traffic from the network interface
	// provided by the network plugin to a tap interface.
	// This works for ipvlan and macvlan as well.
	NetXConnectTCFilterModel

	// NetXConnectNoneModel can be used when the VM is in the host network namespace
	NetXConnectNoneModel

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

##### Sandbox `ContainerConfig`
```Go
// ContainerConfig describes one container runtime configuration.
type ContainerConfig struct {
	ID string

	// RootFs is the container workload image on the host.
	RootFs RootFs

	// ReadOnlyRootfs indicates if the rootfs should be mounted readonly
	ReadonlyRootfs bool

	// Cmd specifies the command to run on a container
	Cmd types.Cmd

	// Annotations allow clients to store arbitrary values,
	// for example to add additional status values required
	// to support particular specifications.
	Annotations map[string]string

	Mounts []Mount

	// Device configuration for devices that must be available within the container.
	DeviceInfos []config.DeviceInfo

	// Resources container resources
	Resources specs.LinuxResources

	// Raw OCI specification, it won't be saved to disk.
	CustomSpec *specs.Spec `json:"-"`
}
```

###### Sandbox `Cmd`
```Go
// Cmd represents a command to execute in a running container.
type Cmd struct {
	Args                []string
	Envs                []EnvVar
	SupplementaryGroups []string

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
	User         string
	PrimaryGroup string
	WorkDir      string
	Console      string
	Capabilities *specs.LinuxCapabilities

	Interactive     bool
	Detach          bool
	NoNewPrivileges bool
}
```

###### Sandbox `Mount`
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

	// BlockDeviceID represents block device that is attached to the
	// VM in case this mount is a block device file or a directory
	// backed by a block device.
	BlockDeviceID string
}
```

###### Sandbox `DeviceInfo`
```Go
// DeviceInfo is an embedded type that contains device data common to all types of devices.
type DeviceInfo struct {
	// Hostpath is device path on host
	HostPath string

	// ContainerPath is device path inside container
	ContainerPath string `json:"-"`

	// Type of device: c, b, u or p
	// c , u - character(unbuffered)
	// p - FIFO
	// b - block(buffered) special file
	// More info in mknod(1).
	DevType string

	// Major, minor numbers for device.
	Major int64
	Minor int64

	// Pmem enabled persistent memory. Use HostPath as backing file
	// for a nvdimm device in the guest.
	Pmem bool

	// If applicable, should this device be considered RO
	ReadOnly bool

	// ColdPlug specifies whether the device must be cold plugged (true)
	// or hot plugged (false).
	ColdPlug bool

	// FileMode permission bits for the device.
	FileMode os.FileMode

	// id of the device owner.
	UID uint32

	// id of the device group.
	GID uint32

	// ID for the device that is passed to the hypervisor.
	ID string

	// DriverOptions is specific options for each device driver
	// for example, for BlockDevice, we can set DriverOptions["blockDriver"]="virtio-blk"
	DriverOptions map[string]string
}
```

#### `VCSandbox`
```Go
// VCSandbox is the Sandbox interface
// (required since virtcontainers.Sandbox only contains private fields)
type VCSandbox interface {
	Annotations(key string) (string, error)
	GetNetNs() string
	GetAllContainers() []VCContainer
	GetAnnotations() map[string]string
	GetContainer(containerID string) VCContainer
	ID() string
	SetAnnotations(annotations map[string]string) error

	Stats() (SandboxStats, error)

	Start() error
	Stop(force bool) error
	Release() error
	Monitor() (chan error, error)
	Delete() error
	Status() SandboxStatus
	CreateContainer(contConfig ContainerConfig) (VCContainer, error)
	DeleteContainer(containerID string) (VCContainer, error)
	StartContainer(containerID string) (VCContainer, error)
	StopContainer(containerID string, force bool) (VCContainer, error)
	KillContainer(containerID string, signal syscall.Signal, all bool) error
	StatusContainer(containerID string) (ContainerStatus, error)
	StatsContainer(containerID string) (ContainerStats, error)
	PauseContainer(containerID string) error
	ResumeContainer(containerID string) error
	EnterContainer(containerID string, cmd types.Cmd) (VCContainer, *Process, error)
	UpdateContainer(containerID string, resources specs.LinuxResources) error
	WaitProcess(containerID, processID string) (int32, error)
	SignalProcess(containerID, processID string, signal syscall.Signal, all bool) error
	WinsizeProcess(containerID, processID string, height, width uint32) error
	IOStream(containerID, processID string) (io.WriteCloser, io.Reader, io.Reader, error)

	AddDevice(info config.DeviceInfo) (api.Device, error)

	AddInterface(inf *pbTypes.Interface) (*pbTypes.Interface, error)
	RemoveInterface(inf *pbTypes.Interface) (*pbTypes.Interface, error)
	ListInterfaces() ([]*pbTypes.Interface, error)
	UpdateRoutes(routes []*pbTypes.Route) ([]*pbTypes.Route, error)
	ListRoutes() ([]*pbTypes.Route, error)

	GetOOMEvent() (string, error)

	UpdateRuntimeMetrics() error
	GetAgentMetrics() (string, error)
	GetAgentURL() (string, error)
}
```

### Sandbox Functions

* [`CreateSandbox`](#createsandbox)
* [`CleanupContainer`](#cleanupcontainer)
* [`SetFactory`](#setfactory)
* [`SetLogger`](#setlogger)

#### `CreateSandbox`
```Go
// CreateSandbox is the virtcontainers sandbox creation entry point.
// CreateSandbox creates a sandbox and its containers. It does not start them.
func CreateSandbox(sandboxConfig SandboxConfig) (VCSandbox, error)
```

#### `CleanupContainer`
```Go
// CleanupContainer is used by shimv2 to stop and delete a container exclusively, once there is no container
// in the sandbox left, do stop the sandbox and delete it. Those serial operations will be done exclusively by
// locking the sandbox.
func CleanupContainer(ctx context.Context, sandboxID, containerID string, force bool) error
```

#### `SetFactory`
```Go
// SetFactory implements the VC function of the same name.
func SetFactory(ctx context.Context, factory Factory)
```

#### `SetLogger`
```Go
// SetLogger implements the VC function of the same name.
func SetLogger(ctx context.Context, logger *logrus.Entry)
```

## Container API

The virtcontainers 1.0 container API manages sandbox
[container lifecycles](#container-functions).

A virtcontainers container is process running inside a containerized
environment, as part of a hardware virtualized context. In other words,
a virtcontainers container is just a regular container running inside a
virtual machine's guest OS.

A virtcontainers container always belong to one and only one
virtcontainers sandbox, again following the
[Kubernetes](https://kubernetes.io/docs/concepts/workloads/pods/pod-overview).
logic and semantics.

The container API allows callers to [create](#createcontainer),
[delete](#deletecontainer), [start](#startcontainer), [stop](#stopcontainer),
[kill](#killcontainer) and [observe](#statuscontainer) containers. It also
allows for running [additional processes](#entercontainer) inside a
specific container.

As a virtcontainers container is always linked to a sandbox, the entire container
API always takes a sandbox ID as its first argument.

To create a container, the API caller must prepare a
[`ContainerConfig`](#sandbox-containerconfig) and pass it to
[`CreateContainer`](#createcontainer) together with a sandbox ID. Upon successful
container creation, the virtcontainers API will return a
[`VCContainer`](#vccontainer) interface back to the caller.

The `VCContainer` interface is a container abstraction hiding the internal
and private virtcontainers container structure. It is a handle for API callers
to manage the container lifecycle through the rest of the
[container API](#container-functions).

* [Structures](#container-structures)
* [Functions](#container-functions)

### Container Structures

* [Container `ContainerConfig`](#container-containerconfig)
  * [Container `Cmd`](#container-cmd)
  * [Container `Mount`](#container-mount)
  * [Container `DeviceInfo`](#container-deviceinfo)
* [`Process`](#process)
* [`ContainerStatus`](#containerstatus)
* [`ProcessListOptions`](#processlistoptions)
* [`VCContainer`](#vccontainer)


#### Container `ContainerConfig`
```Go
// ContainerConfig describes one container runtime configuration.
type ContainerConfig struct {
	ID string

	// RootFs is the container workload image on the host.
	RootFs RootFs

	// ReadOnlyRootfs indicates if the rootfs should be mounted readonly
	ReadonlyRootfs bool

	// Cmd specifies the command to run on a container
	Cmd types.Cmd

	// Annotations allow clients to store arbitrary values,
	// for example to add additional status values required
	// to support particular specifications.
	Annotations map[string]string

	Mounts []Mount

	// Device configuration for devices that must be available within the container.
	DeviceInfos []config.DeviceInfo

	// Resources container resources
	Resources specs.LinuxResources

	// Raw OCI specification, it won't be saved to disk.
	CustomSpec *specs.Spec `json:"-"`
}
```

##### Container `Cmd`
```Go
// Cmd represents a command to execute in a running container.
type Cmd struct {
	Args                []string
	Envs                []EnvVar
	SupplementaryGroups []string

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
	User         string
	PrimaryGroup string
	WorkDir      string
	Console      string
	Capabilities *specs.LinuxCapabilities

	Interactive     bool
	Detach          bool
	NoNewPrivileges bool
}
```

##### Container `Mount`
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

	// BlockDeviceID represents block device that is attached to the
	// VM in case this mount is a block device file or a directory
	// backed by a block device.
	BlockDeviceID string
}
```

##### Container `DeviceInfo`
```Go
// DeviceInfo is an embedded type that contains device data common to all types of devices.
type DeviceInfo struct {
	// Hostpath is device path on host
	HostPath string

	// ContainerPath is device path inside container
	ContainerPath string `json:"-"`

	// Type of device: c, b, u or p
	// c , u - character(unbuffered)
	// p - FIFO
	// b - block(buffered) special file
	// More info in mknod(1).
	DevType string

	// Major, minor numbers for device.
	Major int64
	Minor int64

	// Pmem enabled persistent memory. Use HostPath as backing file
	// for a nvdimm device in the guest.
	Pmem bool

	// If applicable, should this device be considered RO
	ReadOnly bool

	// ColdPlug specifies whether the device must be cold plugged (true)
	// or hot plugged (false).
	ColdPlug bool

	// FileMode permission bits for the device.
	FileMode os.FileMode

	// id of the device owner.
	UID uint32

	// id of the device group.
	GID uint32

	// ID for the device that is passed to the hypervisor.
	ID string

	// DriverOptions is specific options for each device driver
	// for example, for BlockDevice, we can set DriverOptions["blockDriver"]="virtio-blk"
	DriverOptions map[string]string
}
```

#### `Process`
```Go
// Process gathers data related to a container process.
type Process struct {
	// Token is the process execution context ID. It must be
	// unique per sandbox.
	// Token is used to manipulate processes for containers
	// that have not started yet, and later identify them
	// uniquely within a sandbox.
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
	State     types.ContainerState
	PID       int
	StartTime time.Time
	RootFs    string
	Spec      *specs.Spec

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
	Sandbox() VCSandbox
	Process() Process
}
```

### Container Functions

* [`CreateContainer`](#createcontainer)
* [`DeleteContainer`](#deletecontainer)
* [`StartContainer`](#startcontainer)
* [`StopContainer`](#stopcontainer)
* [`EnterContainer`](#entercontainer)
* [`StatusContainer`](#statuscontainer)
* [`KillContainer`](#killcontainer)
* [`StatsContainer`](#statscontainer)
* [`PauseContainer`](#pausecontainer)
* [`ResumeContainer`](#resumecontainer)
* [`UpdateContainer`](#updatecontainer)
* [`WaitProcess`](#waitprocess)
* [`SignalProcess`](#signalprocess)
* [`WinsizeProcess`](#winsizeprocess)
* [`IOStream`](#iostream)

#### `CreateContainer`
```Go
// CreateContainer is the virtcontainers container creation entry point.
// CreateContainer creates a container on a given sandbox.
func CreateContainer(sandboxID string, containerConfig ContainerConfig) (VCSandbox, VCContainer, error)
```

#### `DeleteContainer`
```Go
// DeleteContainer is the virtcontainers container deletion entry point.
// DeleteContainer deletes a Container from a Sandbox. If the container is running,
// it needs to be stopped first.
func DeleteContainer(sandboxID, containerID string) (VCContainer, error)
```

#### `StartContainer`
```Go
// StartContainer is the virtcontainers container starting entry point.
// StartContainer starts an already created container.
func StartContainer(sandboxID, containerID string) (VCContainer, error)
```

#### `StopContainer`
```Go
// StopContainer is the virtcontainers container stopping entry point.
// StopContainer stops an already running container.
func StopContainer(sandboxID, containerID string) (VCContainer, error)
```

#### `EnterContainer`
```Go
// EnterContainer is the virtcontainers container command execution entry point.
// EnterContainer enters an already running container and runs a given command.
func EnterContainer(sandboxID, containerID string, cmd Cmd) (VCSandbox, VCContainer, *Process, error)
```

#### `StatusContainer`
```Go
// StatusContainer is the virtcontainers container status entry point.
// StatusContainer returns a detailed container status.
func StatusContainer(sandboxID, containerID string) (ContainerStatus, error)
```

#### `KillContainer`
```Go
// KillContainer is the virtcontainers entry point to send a signal
// to a container running inside a sandbox. If all is true, all processes in
// the container will be sent the signal.
func KillContainer(sandboxID, containerID string, signal syscall.Signal, all bool) error
```

#### `StatsContainer`
```Go
// StatsContainer return the stats of a running container
func StatsContainer(containerID string) (ContainerStats, error)
```

#### `PauseContainer`
```Go
// PauseContainer pauses a running container.
func PauseContainer(containerID string) error
```

#### `ResumeContainer`
```Go
// ResumeContainer resumes a paused container.
func ResumeContainer(containerID string) error
```

#### `UpdateContainer`
```Go
// UpdateContainer update a running container.
func UpdateContainer(containerID string, resources specs.LinuxResources) error
```

#### `WaitProcess`
```Go
// WaitProcess waits on a container process and return its exit code
func WaitProcess(containerID, processID string) (int32, error)
```

#### `SignalProcess`
```Go
// SignalProcess sends a signal to a process of a container when all is false.
// When all is true, it sends the signal to all processes of a container.
func SignalProcess(containerID, processID string, signal syscall.Signal, all bool) error
```

#### `WinsizeProcess`
```Go
// WinsizeProcess resizes the tty window of a process
func WinsizeProcess(containerID, processID string, height, width uint32) error
```

#### `IOStream`
```Go
// IOStream returns stdin writer, stdout reader and stderr reader of a process
func IOStream(containerID, processID string) (io.WriteCloser, io.Reader, io.Reader, error)
```

## Examples

### Preparing and running a sandbox

```Go

import (
    "context"
    "fmt"
    "strings"

    vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
    "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

var containerRootfs = vc.RootFs{Target: "/var/lib/container/bundle/", Mounted: true}

// This example creates and starts a single container sandbox,
// using qemu as the hypervisor and kata as the VM agent.
func Example_createAndStartSandbox() {
	envs := []types.EnvVar{
		{
			Var:   "PATH",
			Value: "/bin:/usr/bin:/sbin:/usr/sbin",
		},
	}

	cmd := types.Cmd{
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
		KernelPath:     "/usr/share/kata-containers/vmlinux.container",
		ImagePath:      "/usr/share/kata-containers/clear-containers.img",
		HypervisorPath: "/usr/bin/qemu-system-x86_64",
		MemorySize:     1024,
		MemSlots:       10,
	}

	// Use kata default values for the agent.
	agConfig := vc.KataAgentConfig{}

	// The sandbox configuration:
	// - One container
	// - Hypervisor is QEMU
	// - Agent is kata
	sandboxConfig := vc.SandboxConfig{
		ID:               "sandbox-abc",
		HypervisorType:   vc.QemuHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentConfig: agConfig,

		Containers: []vc.ContainerConfig{container},
	}

	// Create the sandbox
	s, err := vc.CreateSandbox(context.Background(), sandboxConfig, nil)
	if err != nil {
		fmt.Printf("Could not create sandbox: %s", err)
		return
	}

	// Start the sandbox
	err = s.Start()
	if err != nil {
		fmt.Printf("Could not start sandbox: %s", err)
	}
}
```
