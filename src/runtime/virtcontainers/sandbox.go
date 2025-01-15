// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2020 Adobe Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"bytes"
	"context"
	"fmt"
	"io"
	"math"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"syscall"

	v1 "github.com/containerd/cgroups/stats/v1"
	v2 "github.com/containerd/cgroups/v2/stats"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"

	cri "github.com/containerd/containerd/pkg/cri/annotations"
	crio "github.com/containers/podman/v4/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/drivers"
	deviceManager "github.com/kata-containers/kata-containers/src/runtime/pkg/device/manager"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	resCtrl "github.com/kata-containers/kata-containers/src/runtime/pkg/resourcecontrol"
	exp "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/experimental"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/cpuset"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"

	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

// sandboxTracingTags defines tags for the trace span
var sandboxTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "sandbox",
}

const (
	// VmStartTimeout represents the time in seconds a sandbox can wait before
	// to consider the VM starting operation failed.
	VmStartTimeout = 10

	// DirMode is the permission bits used for creating a directory
	DirMode = os.FileMode(0750) | os.ModeDir

	mkswapPath = "/sbin/mkswap"
	rwm        = "rwm"

	// When the Kata overhead threads (I/O, VMM, etc) are not
	// placed in the sandbox resource controller (A cgroup on Linux),
	// they are moved to a specific, unconstrained resource controller.
	// On Linux, assuming the cgroup mount point is at /sys/fs/cgroup/,
	// on a cgroup v1 system, the Kata overhead memory cgroup will be at
	// /sys/fs/cgroup/memory/kata_overhead/$CGPATH where $CGPATH is
	// defined by the orchestrator.
	resCtrlKataOverheadID = "/kata_overhead/"

	sandboxMountsDir = "sandbox-mounts"

	// Restricted permission for shared directory managed by virtiofs
	sharedDirMode = os.FileMode(0700) | os.ModeDir

	// hotplug factor indicates how much memory can be hotplugged relative to the amount of
	// RAM provided to the guest. This is a conservative heuristic based on needing 64 bytes per
	// 4KiB page of hotplugged memory.
	//
	// As an example: 12 GiB hotplugged -> 3 Mi pages -> 192 MiBytes overhead (3Mi x 64B).
	// This is approximately what should be free in a relatively unloaded 256 MiB guest (75% of available memory). So, 256 Mi x 48 => 12 Gi
	acpiMemoryHotplugFactor = 48
)

var (
	errSandboxNotRunning = errors.New("Sandbox not running")
)

// HypervisorPidKey is the context key for hypervisor pid
type HypervisorPidKey struct{}

// SandboxStatus describes a sandbox status.
type SandboxStatus struct {
	Annotations      map[string]string
	ID               string
	Hypervisor       HypervisorType
	ContainersStatus []ContainerStatus
	State            types.SandboxState
	HypervisorConfig HypervisorConfig
}

// SandboxStats describes a sandbox's stats
type SandboxStats struct {
	CgroupStats CgroupStats
	Cpus        int
}

type SandboxResourceSizing struct {
	// The number of CPUs required for the sandbox workload(s)
	WorkloadCPUs float32
	// The base number of CPUs for the VM that are assigned as overhead
	BaseCPUs float32
	// The amount of memory required for the sandbox workload(s)
	WorkloadMemMB uint32
	// The base amount of memory required for that VM that is assigned as overhead
	BaseMemMB uint32
}

// SandboxConfig is a Sandbox configuration.
type SandboxConfig struct {
	// Annotations keys must be unique strings and must be name-spaced
	Annotations map[string]string

	// Custom SELinux security policy to the container process inside the VM
	GuestSeLinuxLabel string

	HypervisorType HypervisorType

	ID string

	Hostname string

	// SandboxBindMounts - list of paths to mount into guest
	SandboxBindMounts []string

	// Experimental features enabled
	Experimental []exp.Feature

	// Containers describe the list of containers within a Sandbox.
	// This list can be empty and populated by adding containers
	// to the Sandbox a posteriori.
	// TODO: this should be a map to avoid duplicated containers
	Containers []ContainerConfig

	Volumes []types.Volume

	NetworkConfig NetworkConfig

	AgentConfig KataAgentConfig

	HypervisorConfig HypervisorConfig

	ShmSize uint64

	SandboxResources SandboxResourceSizing

	VfioMode config.VFIOModeType

	// StaticResourceMgmt indicates if the shim should rely on statically sizing the sandbox (VM)
	StaticResourceMgmt bool

	// SharePidNs sets all containers to share the same sandbox level pid namespace.
	SharePidNs bool
	// SystemdCgroup enables systemd cgroup support
	SystemdCgroup bool
	// SandboxCgroupOnly enables cgroup only at podlevel in the host
	SandboxCgroupOnly bool

	// DisableGuestSeccomp disable seccomp within the guest
	DisableGuestSeccomp bool

	// EnableVCPUsPinning controls whether each vCPU thread should be scheduled to a fixed CPU
	EnableVCPUsPinning bool

	// Create container timeout which, if provided, indicates the create container timeout
	// needed for the workload(s)
	CreateContainerTimeout uint64
}

// valid checks that the sandbox configuration is valid.
func (sandboxConfig *SandboxConfig) valid() bool {
	if sandboxConfig.ID == "" {
		return false
	}

	if _, err := NewHypervisor(sandboxConfig.HypervisorType); err != nil {
		sandboxConfig.HypervisorType = QemuHypervisor
	}

	// validate experimental features
	for _, f := range sandboxConfig.Experimental {
		if exp.Get(f.Name) == nil {
			return false
		}
	}
	return true
}

// Sandbox is composed of a set of containers and a runtime environment.
// A Sandbox can be created, deleted, started, paused, stopped, listed, entered, and restored.
type Sandbox struct {
	ctx        context.Context
	devManager api.DeviceManager
	factory    Factory
	hypervisor Hypervisor
	agent      agent
	store      persistapi.PersistDriver
	fsShare    FilesystemSharer

	swapDevices []*config.BlockDrive
	volumes     []types.Volume

	monitor         *monitor
	config          *SandboxConfig
	annotationsLock *sync.RWMutex
	wg              *sync.WaitGroup
	cw              *consoleWatcher

	sandboxController  resCtrl.ResourceController
	overheadController resCtrl.ResourceController

	containers map[string]*Container

	id string

	network Network

	state types.SandboxState

	sync.Mutex

	swapSizeBytes int64
	shmSize       uint64
	swapDeviceNum uint

	sharePidNs        bool
	seccompSupported  bool
	disableVMShutdown bool
	isVCPUsPinningOn  bool

	// hotplugNetworkConfigApplied prevents network config API being called
	// multiple times for hot-plugged network device when Sandbox has multiple
	// containers.
	hotplugNetworkConfigApplied bool
}

// ID returns the sandbox identifier string.
func (s *Sandbox) ID() string {
	return s.id
}

// Logger returns a logrus logger appropriate for logging Sandbox messages
func (s *Sandbox) Logger() *logrus.Entry {
	return virtLog.WithFields(logrus.Fields{
		"subsystem": "sandbox",
		"sandbox":   s.id,
	})
}

// Annotations returns any annotation that a user could have stored through the sandbox.
func (s *Sandbox) Annotations(key string) (string, error) {
	s.annotationsLock.RLock()
	defer s.annotationsLock.RUnlock()

	value, exist := s.config.Annotations[key]
	if !exist {
		return "", fmt.Errorf("Annotations key %s does not exist", key)
	}

	return value, nil
}

// SetAnnotations sets or adds an annotations
func (s *Sandbox) SetAnnotations(annotations map[string]string) error {
	s.annotationsLock.Lock()
	defer s.annotationsLock.Unlock()

	for k, v := range annotations {
		s.config.Annotations[k] = v
	}
	return nil
}

// GetAnnotations returns sandbox's annotations
func (s *Sandbox) GetAnnotations() map[string]string {
	s.annotationsLock.RLock()
	defer s.annotationsLock.RUnlock()

	return s.config.Annotations
}

// GetNetNs returns the network namespace of the current sandbox.
func (s *Sandbox) GetNetNs() string {
	return s.network.NetworkID()
}

// GetHypervisorPid returns the hypervisor's pid.
func (s *Sandbox) GetHypervisorPid() (int, error) {
	pids := s.hypervisor.GetPids()
	if len(pids) == 0 || pids[0] == 0 {
		return -1, fmt.Errorf("Invalid hypervisor PID: %+v", pids)
	}

	return pids[0], nil
}

// GetAllContainers returns all containers.
func (s *Sandbox) GetAllContainers() []VCContainer {
	ifa := make([]VCContainer, len(s.containers))

	i := 0
	for _, v := range s.containers {
		ifa[i] = v
		i++
	}

	return ifa
}

// GetContainer returns the container named by the containerID.
func (s *Sandbox) GetContainer(containerID string) VCContainer {
	if c, ok := s.containers[containerID]; ok {
		return c
	}
	return nil
}

// Release closes the agent connection.
func (s *Sandbox) Release(ctx context.Context) error {
	s.Logger().Info("release sandbox")
	if s.monitor != nil {
		s.monitor.stop()
	}
	s.fsShare.StopFileEventWatcher(ctx)
	s.hypervisor.Disconnect(ctx)
	return s.agent.disconnect(ctx)
}

// Status gets the status of the sandbox
func (s *Sandbox) Status() SandboxStatus {
	var contStatusList []ContainerStatus
	for _, c := range s.containers {
		rootfs := c.config.RootFs.Source
		if c.config.RootFs.Mounted {
			rootfs = c.config.RootFs.Target
		}

		contStatusList = append(contStatusList, ContainerStatus{
			ID:          c.id,
			State:       c.state,
			PID:         c.process.Pid,
			StartTime:   c.process.StartTime,
			RootFs:      rootfs,
			Annotations: c.config.Annotations,
		})
	}

	return SandboxStatus{
		ID:               s.id,
		State:            s.state,
		Hypervisor:       s.config.HypervisorType,
		HypervisorConfig: s.config.HypervisorConfig,
		ContainersStatus: contStatusList,
		Annotations:      s.config.Annotations,
	}
}

// Monitor returns a error channel for watcher to watch at
func (s *Sandbox) Monitor(ctx context.Context) (chan error, error) {
	if s.state.State != types.StateRunning {
		return nil, errSandboxNotRunning
	}

	s.Lock()
	if s.monitor == nil {
		s.monitor = newMonitor(s)
	}
	s.Unlock()

	return s.monitor.newWatcher(ctx)
}

// WaitProcess waits on a container process and return its exit code
func (s *Sandbox) WaitProcess(ctx context.Context, containerID, processID string) (int32, error) {
	if s.state.State != types.StateRunning {
		return 0, errSandboxNotRunning
	}

	c, err := s.findContainer(containerID)
	if err != nil {
		return 0, err
	}

	return c.wait(ctx, processID)
}

// SignalProcess sends a signal to a process of a container when all is false.
// When all is true, it sends the signal to all processes of a container.
func (s *Sandbox) SignalProcess(ctx context.Context, containerID, processID string, signal syscall.Signal, all bool) error {
	if s.state.State != types.StateRunning {
		return errSandboxNotRunning
	}

	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	return c.signalProcess(ctx, processID, signal, all)
}

// WinsizeProcess resizes the tty window of a process
func (s *Sandbox) WinsizeProcess(ctx context.Context, containerID, processID string, height, width uint32) error {
	if s.state.State != types.StateRunning {
		return errSandboxNotRunning
	}

	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	return c.winsizeProcess(ctx, processID, height, width)
}

// IOStream returns stdin writer, stdout reader and stderr reader of a process
func (s *Sandbox) IOStream(containerID, processID string) (io.WriteCloser, io.Reader, io.Reader, error) {
	if s.state.State != types.StateRunning {
		return nil, nil, nil, errSandboxNotRunning
	}

	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, nil, nil, err
	}

	return c.ioStream(processID)
}

func createAssets(ctx context.Context, sandboxConfig *SandboxConfig) error {
	span, _ := katatrace.Trace(ctx, nil, "createAssets", sandboxTracingTags, map[string]string{"sandbox_id": sandboxConfig.ID})
	defer span.End()

	for _, name := range types.AssetTypes() {
		annotation, _, err := name.Annotations()
		if err != nil {
			return err
		}
		// For remote hypervisor donot check for Absolute Path incase of ImagePath, as it denotes the name of the image.
		if sandboxConfig.HypervisorType == RemoteHypervisor && annotation == annotations.ImagePath {
			value := sandboxConfig.Annotations[annotation]
			if value != "" {
				sandboxConfig.HypervisorConfig.ImagePath = value
			}
		} else {
			a, err := types.NewAsset(sandboxConfig.Annotations, name)
			if err != nil {
				return err
			}

			if err := sandboxConfig.HypervisorConfig.AddCustomAsset(a); err != nil {
				return err
			}
		}
	}

	_, imageErr := sandboxConfig.HypervisorConfig.assetPath(types.ImageAsset)
	_, initrdErr := sandboxConfig.HypervisorConfig.assetPath(types.InitrdAsset)

	if imageErr != nil && initrdErr != nil {
		return fmt.Errorf("%s and %s cannot be both set", types.ImageAsset, types.InitrdAsset)
	}

	return nil
}

func (s *Sandbox) getAndStoreGuestDetails(ctx context.Context) error {
	guestDetailRes, err := s.agent.getGuestDetails(ctx, &grpc.GuestDetailsRequest{
		MemBlockSize:    true,
		MemHotplugProbe: true,
	})
	if err != nil {
		return err
	}

	if guestDetailRes != nil {
		s.state.GuestMemoryBlockSizeMB = uint32(guestDetailRes.MemBlockSizeBytes >> 20)
		if guestDetailRes.AgentDetails != nil {
			s.seccompSupported = guestDetailRes.AgentDetails.SupportsSeccomp
		}
		s.state.GuestMemoryHotplugProbe = guestDetailRes.SupportMemHotplugProbe
	}

	return nil
}

// createSandbox creates a sandbox from a sandbox description, the containers list, the hypervisor
// and the agent passed through the Config structure.
// It will create and store the sandbox structure, and then ask the hypervisor
// to physically create that sandbox i.e. starts a VM for that sandbox to eventually
// be started.
func createSandbox(ctx context.Context, sandboxConfig SandboxConfig, factory Factory) (*Sandbox, error) {
	span, ctx := katatrace.Trace(ctx, nil, "createSandbox", sandboxTracingTags, map[string]string{"sandbox_id": sandboxConfig.ID})
	defer span.End()

	if err := createAssets(ctx, &sandboxConfig); err != nil {
		return nil, err
	}

	s, err := newSandbox(ctx, sandboxConfig, factory)
	if err != nil {
		return nil, err
	}

	if len(s.config.Experimental) != 0 {
		s.Logger().WithField("features", s.config.Experimental).Infof("Enable experimental features")
	}

	// Sandbox state has been loaded from storage.
	// If the Stae is not empty, this is a re-creation, i.e.
	// we don't need to talk to the guest's agent, but only
	// want to create the sandbox and its containers in memory.
	if s.state.State != "" {
		return s, nil
	}

	// The code below only gets called when initially creating a sandbox, not when restoring or
	// re-creating it. The above check for the sandbox state enforces that.

	if err := s.fsShare.Prepare(ctx); err != nil {
		return nil, err
	}

	if err := s.agent.createSandbox(ctx, s); err != nil {
		return nil, err
	}

	// Set sandbox state
	if err := s.setSandboxState(types.StateReady); err != nil {
		return nil, err
	}

	return s, nil
}

func newSandbox(ctx context.Context, sandboxConfig SandboxConfig, factory Factory) (sb *Sandbox, retErr error) {
	span, ctx := katatrace.Trace(ctx, nil, "newSandbox", sandboxTracingTags, map[string]string{"sandbox_id": sandboxConfig.ID})
	defer span.End()

	if !sandboxConfig.valid() {
		return nil, fmt.Errorf("Invalid sandbox configuration")
	}

	// create agent instance
	agent := getNewAgentFunc(ctx)()

	hypervisor, err := NewHypervisor(sandboxConfig.HypervisorType)
	if err != nil {
		return nil, err
	}

	network, err := NewNetwork(&sandboxConfig.NetworkConfig)
	if err != nil {
		return nil, err
	}

	s := &Sandbox{
		id:              sandboxConfig.ID,
		factory:         factory,
		hypervisor:      hypervisor,
		agent:           agent,
		config:          &sandboxConfig,
		volumes:         sandboxConfig.Volumes,
		containers:      map[string]*Container{},
		state:           types.SandboxState{BlockIndexMap: make(map[int]struct{})},
		annotationsLock: &sync.RWMutex{},
		wg:              &sync.WaitGroup{},
		shmSize:         sandboxConfig.ShmSize,
		sharePidNs:      sandboxConfig.SharePidNs,
		network:         network,
		ctx:             ctx,
		swapDeviceNum:   0,
		swapSizeBytes:   0,
		swapDevices:     []*config.BlockDrive{},
	}

	fsShare, err := NewFilesystemShare(s)
	if err != nil {
		return nil, err
	}
	s.fsShare = fsShare

	if s.store, err = persist.GetDriver(); err != nil || s.store == nil {
		return nil, fmt.Errorf("failed to get fs persist driver: %v", err)
	}
	defer func() {
		if retErr != nil {
			s.Logger().WithError(retErr).Error("Create new sandbox failed")
			s.store.Destroy(s.id)
		}
	}()

	sandboxConfig.HypervisorConfig.VMStorePath = s.store.RunVMStoragePath()
	sandboxConfig.HypervisorConfig.RunStorePath = s.store.RunStoragePath()

	spec := s.GetPatchedOCISpec()
	if spec != nil && spec.Process.SelinuxLabel != "" {
		sandboxConfig.HypervisorConfig.SELinuxProcessLabel = spec.Process.SelinuxLabel
	}

	s.devManager = deviceManager.NewDeviceManager(sandboxConfig.HypervisorConfig.BlockDeviceDriver,
		sandboxConfig.HypervisorConfig.EnableVhostUserStore,
		sandboxConfig.HypervisorConfig.VhostUserStorePath, sandboxConfig.HypervisorConfig.VhostUserDeviceReconnect, nil)

	// Create the sandbox resource controllers.
	if err := s.createResourceController(); err != nil {
		return nil, err
	}

	// Ignore the error. Restore can fail for a new sandbox
	if err := s.Restore(); err != nil {
		s.Logger().WithError(err).Debug("restore sandbox failed")
	}

	if err := validateHypervisorConfig(&sandboxConfig.HypervisorConfig); err != nil {
		return nil, err
	}

	// Start the event loop if not already started when fs sharing is not used
	if sandboxConfig.HypervisorConfig.SharedFS == config.NoSharedFS {
		// Start the StartFileEventWatcher method as a goroutine
		// to monitor the file events.
		go func() {
			if err := s.fsShare.StartFileEventWatcher(ctx); err != nil {
				s.Logger().WithError(err).Error("Failed to start file event watcher")
				return
			}
		}()

		// Stop the file event watcher on error
		defer func() {
			if retErr != nil {
				s.Logger().WithError(retErr).Error("Stopping File Event Watcher")
				s.fsShare.StopFileEventWatcher(ctx)
			}
		}()

	}

	setHypervisorConfigAnnotations(&sandboxConfig)

	coldPlugVFIO, err := s.coldOrHotPlugVFIO(&sandboxConfig)
	if err != nil {
		return nil, err
	}

	// store doesn't require hypervisor to be stored immediately
	if err = s.hypervisor.CreateVM(ctx, s.id, s.network, &sandboxConfig.HypervisorConfig); err != nil {
		return nil, err
	}

	if s.disableVMShutdown, err = s.agent.init(ctx, s, sandboxConfig.AgentConfig); err != nil {
		return nil, err
	}

	if !coldPlugVFIO {
		return s, nil
	}

	for _, dev := range sandboxConfig.HypervisorConfig.VFIODevices {
		s.Logger().Info("cold-plug device: ", dev)
		_, err := s.AddDevice(ctx, dev)
		if err != nil {
			s.Logger().WithError(err).Debug("Cannot cold-plug add device")
			return nil, err
		}
	}
	return s, nil
}

func setHypervisorConfigAnnotations(sandboxConfig *SandboxConfig) {
	if len(sandboxConfig.Containers) > 0 {
		// These values are required by remote hypervisor
		for _, a := range []string{cri.SandboxName, crio.SandboxName} {
			if value, ok := sandboxConfig.Containers[0].Annotations[a]; ok {
				sandboxConfig.HypervisorConfig.SandboxName = value
			}
		}

		for _, a := range []string{cri.SandboxNamespace, crio.Namespace} {
			if value, ok := sandboxConfig.Containers[0].Annotations[a]; ok {
				sandboxConfig.HypervisorConfig.SandboxNamespace = value
			}
		}
	}
}

func (s *Sandbox) coldOrHotPlugVFIO(sandboxConfig *SandboxConfig) (bool, error) {
	// If we have a confidential guest we need to cold-plug the PCIe VFIO devices
	// until we have TDISP/IDE PCIe support.
	coldPlugVFIO := (sandboxConfig.HypervisorConfig.ColdPlugVFIO != config.NoPort)
	// Aggregate all the containner devices for hot-plug and use them to dedcue
	// the correct amount of ports to reserve for the hypervisor.
	hotPlugVFIO := (sandboxConfig.HypervisorConfig.HotPlugVFIO != config.NoPort)

	//modeIsGK := (sandboxConfig.VfioMode == config.VFIOModeGuestKernel)
	// modeIsVFIO is needed at the container level not the sandbox level.
	// modeIsVFIO := (sandboxConfig.VfioMode == config.VFIOModeVFIO)

	var vfioDevices []config.DeviceInfo
	// vhost-user-block device is a PCIe device in Virt, keep track of it
	// for correct number of PCIe root ports.
	var vhostUserBlkDevices []config.DeviceInfo

	//io.katacontainers.pkg.oci.container_type:pod_sandbox

	for cnt, container := range sandboxConfig.Containers {
		// Do not alter the original spec, we do not want to inject
		// CDI devices into the sandbox container, were using the CDI
		// devices as additional information to determine the number of
		// PCIe root ports to reserve for the hypervisor.
		// A single_container type will have the CDI devices injected
		// only do this if we're a pod_sandbox type.
		if container.Annotations["io.katacontainers.pkg.oci.container_type"] == "pod_sandbox" && container.CustomSpec != nil {
			cdiSpec := container.CustomSpec
			// We can provide additional directories where to search for
			// CDI specs if needed. immutable OS's only have specific
			// directories where applications can write too. For instance /opt/cdi
			//
			// _, err = withCDI(ociSpec.Annotations, []string{"/opt/cdi"}, ociSpec)
			//
			_, err := config.WithCDI(cdiSpec.Annotations, []string{}, cdiSpec)
			if err != nil {
				return coldPlugVFIO, fmt.Errorf("adding CDI devices failed")
			}

			for _, dev := range cdiSpec.Linux.Devices {
				isVFIODevice := deviceManager.IsVFIODevice(dev.Path)
				if hotPlugVFIO && isVFIODevice {
					vfioDev := config.DeviceInfo{
						ColdPlug:      true,
						ContainerPath: dev.Path,
						Port:          sandboxConfig.HypervisorConfig.HotPlugVFIO,
						DevType:       dev.Type,
						Major:         dev.Major,
						Minor:         dev.Minor,
					}
					if dev.FileMode != nil {
						vfioDev.FileMode = *dev.FileMode
					}
					if dev.UID != nil {
						vfioDev.UID = *dev.UID
					}
					if dev.GID != nil {
						vfioDev.GID = *dev.GID
					}

					vfioDevices = append(vfioDevices, vfioDev)
					continue
				}
				if coldPlugVFIO && isVFIODevice {
					vfioDev := config.DeviceInfo{
						ColdPlug:      true,
						ContainerPath: dev.Path,
						Port:          sandboxConfig.HypervisorConfig.ColdPlugVFIO,
						DevType:       dev.Type,
						Major:         dev.Major,
						Minor:         dev.Minor,
					}
					if dev.FileMode != nil {
						vfioDev.FileMode = *dev.FileMode
					}
					if dev.UID != nil {
						vfioDev.UID = *dev.UID
					}
					if dev.GID != nil {
						vfioDev.GID = *dev.GID
					}

					vfioDevices = append(vfioDevices, vfioDev)
					continue
				}
			}
		}
		// As stated before the single_container will have the  CDI
		// devices injected by the runtime. For the pod_container use-case
		// see container.go how cold and hot-plug are handled.
		for dev, device := range container.DeviceInfos {
			if deviceManager.IsVhostUserBlk(device) {
				vhostUserBlkDevices = append(vhostUserBlkDevices, device)
				continue
			}
			isVFIODevice := deviceManager.IsVFIODevice(device.ContainerPath)
			if hotPlugVFIO && isVFIODevice {
				device.ColdPlug = false
				device.Port = sandboxConfig.HypervisorConfig.HotPlugVFIO
				vfioDevices = append(vfioDevices, device)
				sandboxConfig.Containers[cnt].DeviceInfos[dev].Port = sandboxConfig.HypervisorConfig.HotPlugVFIO
				continue
			}
			if coldPlugVFIO && isVFIODevice {
				device.ColdPlug = true
				device.Port = sandboxConfig.HypervisorConfig.ColdPlugVFIO
				vfioDevices = append(vfioDevices, device)
				sandboxConfig.Containers[cnt].DeviceInfos[dev].Port = sandboxConfig.HypervisorConfig.ColdPlugVFIO
				continue
			}
		}
	}

	sandboxConfig.HypervisorConfig.VFIODevices = vfioDevices
	sandboxConfig.HypervisorConfig.VhostUserBlkDevices = vhostUserBlkDevices

	return coldPlugVFIO, nil
}

func (s *Sandbox) createResourceController() error {
	var err error
	cgroupPath := ""

	// Do not change current cgroup configuration.
	// Create a spec without constraints
	resources := specs.LinuxResources{}

	if s.config == nil {
		return fmt.Errorf("Could not create %s resource controller manager: empty sandbox configuration", s.sandboxController)
	}

	spec := s.GetPatchedOCISpec()
	if spec != nil && spec.Linux != nil {
		cgroupPath = spec.Linux.CgroupsPath

		// Kata relies on the resource controller (cgroups on Linux) parent created and configured by the
		// container engine by default. The exception is for devices whitelist as well as sandbox-level CPUSet.
		// For the sandbox controllers we create and manage, rename the base of the controller ID to
		// include "kata_"
		if !resCtrl.IsSystemdCgroup(cgroupPath) { // don't add prefix when cgroups are managed by systemd
			cgroupPath, err = resCtrl.RenameCgroupPath(cgroupPath)
			if err != nil {
				return err
			}
		}

		if spec.Linux.Resources != nil {
			resources.Devices = spec.Linux.Resources.Devices

			intptr := func(i int64) *int64 { return &i }
			// Determine if device /dev/null and /dev/urandom exist, and add if they don't
			nullDeviceExist := false
			urandomDeviceExist := false
			ptmxDeviceExist := false
			for _, device := range resources.Devices {
				if device.Type == "c" && device.Major == intptr(1) && device.Minor == intptr(3) {
					nullDeviceExist = true
				}

				if device.Type == "c" && device.Major == intptr(1) && device.Minor == intptr(9) {
					urandomDeviceExist = true
				}

				if device.Type == "c" && device.Major == intptr(5) && device.Minor == intptr(2) {
					ptmxDeviceExist = true
				}
			}

			if !nullDeviceExist {
				// "/dev/null"
				resources.Devices = append(resources.Devices, []specs.LinuxDeviceCgroup{
					{Type: "c", Major: intptr(1), Minor: intptr(3), Access: rwm, Allow: true},
				}...)
			}
			if !urandomDeviceExist {
				// "/dev/urandom"
				resources.Devices = append(resources.Devices, []specs.LinuxDeviceCgroup{
					{Type: "c", Major: intptr(1), Minor: intptr(9), Access: rwm, Allow: true},
				}...)
			}

			// If the hypervisor debug console is enabled and
			// sandbox_cgroup_only are configured, then the vmm needs access to
			// /dev/ptmx.  Add this to the device allowlist if it is not
			// already present in the config.
			if s.config.HypervisorConfig.Debug && s.config.SandboxCgroupOnly && !ptmxDeviceExist {
				// "/dev/ptmx"
				resources.Devices = append(resources.Devices, []specs.LinuxDeviceCgroup{
					{Type: "c", Major: intptr(5), Minor: intptr(2), Access: rwm, Allow: true},
				}...)

			}

			if spec.Linux.Resources.CPU != nil {
				resources.CPU = &specs.LinuxCPU{
					Cpus: spec.Linux.Resources.CPU.Cpus,
				}
			}
		}

		//TODO: in Docker or Podman use case, it is reasonable to set a constraint. Need to add a flag
		// to allow users to configure Kata to constrain CPUs and Memory in this alternative
		// scenario. See https://github.com/kata-containers/runtime/issues/2811
	}

	if s.devManager != nil {
		for _, d := range s.devManager.GetAllDevices() {
			dev, err := resCtrl.DeviceToLinuxDevice(d.GetHostPath())
			if err != nil {
				s.Logger().WithError(err).WithField("device", d.GetHostPath()).Warn("Could not add device to sandbox resources")
				continue
			}
			resources.Devices = append(resources.Devices, dev)
		}
	}

	// Create the sandbox resource controller (cgroups on Linux).
	// Depending on the SandboxCgroupOnly value, this cgroup
	// will either hold all the pod threads (SandboxCgroupOnly is true)
	// or only the virtual CPU ones (SandboxCgroupOnly is false).
	s.sandboxController, err = resCtrl.NewSandboxResourceController(cgroupPath, &resources, s.config.SandboxCgroupOnly)
	if err != nil {
		return fmt.Errorf("Could not create the sandbox resource controller %v", err)
	}

	// Now that the sandbox resource controller is created, we can set the state controller paths.
	s.state.SandboxCgroupPath = s.sandboxController.ID()
	s.state.OverheadCgroupPath = ""

	if s.config.SandboxCgroupOnly {
		s.overheadController = nil
	} else {
		// The shim configuration is requesting that we do not put all threads
		// into the sandbox resource controller.
		// We're creating an overhead controller, with no constraints. Everything but
		// the vCPU threads will eventually make it there.
		overheadController, err := resCtrl.NewResourceController(fmt.Sprintf("%s%s", resCtrlKataOverheadID, s.id), &specs.LinuxResources{})
		// TODO: support systemd cgroups overhead cgroup
		// https://github.com/kata-containers/kata-containers/issues/2963
		if err != nil {
			return err
		}
		s.overheadController = overheadController
		s.state.OverheadCgroupPath = s.overheadController.ID()
	}

	return nil
}

// storeSandbox stores a sandbox config.
func (s *Sandbox) storeSandbox(ctx context.Context) error {
	span, _ := katatrace.Trace(ctx, s.Logger(), "storeSandbox", sandboxTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	// flush data to storage
	if err := s.Save(); err != nil {
		return err
	}
	return nil
}

func rwLockSandbox(sandboxID string) (func() error, error) {
	store, err := persist.GetDriver()
	if err != nil {
		return nil, fmt.Errorf("failed to get fs persist driver: %v", err)
	}

	return store.Lock(sandboxID, true)
}

// findContainer returns a container from the containers list held by the
// sandbox structure, based on a container ID.
func (s *Sandbox) findContainer(containerID string) (*Container, error) {
	if s == nil {
		return nil, types.ErrNeedSandbox
	}

	if containerID == "" {
		return nil, types.ErrNeedContainerID
	}

	if c, ok := s.containers[containerID]; ok {
		return c, nil
	}

	return nil, errors.Wrapf(types.ErrNoSuchContainer, "Could not find the container %q from the sandbox %q containers list",
		containerID, s.id)
}

// removeContainer removes a container from the containers list held by the
// sandbox structure, based on a container ID.
func (s *Sandbox) removeContainer(containerID string) error {
	if s == nil {
		return types.ErrNeedSandbox
	}

	if containerID == "" {
		return types.ErrNeedContainerID
	}

	if _, ok := s.containers[containerID]; !ok {
		return errors.Wrapf(types.ErrNoSuchContainer, "Could not remove the container %q from the sandbox %q containers list",
			containerID, s.id)
	}

	delete(s.containers, containerID)

	return nil
}

// Delete deletes an already created sandbox.
// The VM in which the sandbox is running will be shut down.
func (s *Sandbox) Delete(ctx context.Context) error {
	if s.state.State != types.StateReady &&
		s.state.State != types.StatePaused &&
		s.state.State != types.StateStopped {
		return fmt.Errorf("Sandbox not ready, paused or stopped, impossible to delete")
	}

	for _, c := range s.containers {
		if err := c.delete(ctx); err != nil {
			s.Logger().WithError(err).WithField("container`", c.id).Debug("failed to delete container")
		}
	}

	if !rootless.IsRootless() {
		if err := s.resourceControllerDelete(); err != nil {
			s.Logger().WithError(err).Errorf("failed to cleanup the %s resource controllers", s.sandboxController)
		}
	}

	if s.monitor != nil {
		s.monitor.stop()
	}

	if err := s.hypervisor.Cleanup(ctx); err != nil {
		s.Logger().WithError(err).Error("failed to Cleanup hypervisor")
	}

	if err := s.fsShare.Cleanup(ctx); err != nil {
		s.Logger().WithError(err).Error("failed to cleanup share files")
	}

	return s.store.Destroy(s.id)
}

func (s *Sandbox) createNetwork(ctx context.Context) error {
	if s.config.NetworkConfig.DisableNewNetwork ||
		s.config.NetworkConfig.NetworkID == "" {
		return nil
	}

	// docker container needs the hypervisor process ID to find out the container netns,
	// which means that the hypervisor has to support network device hotplug so that docker
	// can use the prestart hooks to set up container netns.
	caps := s.hypervisor.Capabilities(ctx)
	if !caps.IsNetworkDeviceHotplugSupported() {
		spec := s.GetPatchedOCISpec()
		if utils.IsDockerContainer(spec) {
			return errors.New("docker container needs network device hotplug but the configured hypervisor does not support it")
		}
	}

	span, ctx := katatrace.Trace(ctx, s.Logger(), "createNetwork", sandboxTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()
	katatrace.AddTags(span, "network", s.network, "NetworkConfig", s.config.NetworkConfig)

	// In case there is a factory, network interfaces are hotplugged
	// after the vm is started.
	if s.factory != nil {
		return nil
	}

	// Add all the networking endpoints.
	if _, err := s.network.AddEndpoints(ctx, s, nil, false); err != nil {
		return err
	}

	return nil
}

func (s *Sandbox) postCreatedNetwork(ctx context.Context) error {
	if s.factory != nil {
		return nil
	}

	if s.network.Endpoints() == nil {
		return nil
	}

	for _, endpoint := range s.network.Endpoints() {
		netPair := endpoint.NetworkPair()
		if netPair == nil {
			continue
		}
		if netPair.VhostFds != nil {
			for _, VhostFd := range netPair.VhostFds {
				VhostFd.Close()
			}
		}
	}

	return nil
}

func (s *Sandbox) removeNetwork(ctx context.Context) error {
	span, ctx := katatrace.Trace(ctx, s.Logger(), "removeNetwork", sandboxTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	return s.network.RemoveEndpoints(ctx, s, nil, false)
}

func (s *Sandbox) generateNetInfo(inf *pbTypes.Interface) (NetworkInfo, error) {
	hw, err := net.ParseMAC(inf.HwAddr)
	if err != nil {
		return NetworkInfo{}, err
	}

	var addrs []netlink.Addr
	for _, addr := range inf.IPAddresses {
		netlinkAddrStr := fmt.Sprintf("%s/%s", addr.Address, addr.Mask)
		netlinkAddr, err := netlink.ParseAddr(netlinkAddrStr)
		if err != nil {
			return NetworkInfo{}, fmt.Errorf("could not parse %q: %v", netlinkAddrStr, err)
		}

		addrs = append(addrs, *netlinkAddr)
	}

	return NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: netlink.LinkAttrs{
				Name:         inf.Name,
				HardwareAddr: hw,
				MTU:          int(inf.Mtu),
			},
			Type: inf.Type,
		},
		Addrs: addrs,
	}, nil
}

// AddInterface adds new nic to the sandbox.
func (s *Sandbox) AddInterface(ctx context.Context, inf *pbTypes.Interface) (*pbTypes.Interface, error) {
	netInfo, err := s.generateNetInfo(inf)
	if err != nil {
		return nil, err
	}

	endpoints, err := s.network.AddEndpoints(ctx, s, []NetworkInfo{netInfo}, true)
	if err != nil {
		return nil, err
	}

	defer func() {
		if err != nil {
			eps := s.network.Endpoints()
			// The newly added endpoint is last.
			added_ep := eps[len(eps)-1]
			if errDetach := s.network.RemoveEndpoints(ctx, s, []Endpoint{added_ep}, true); err != nil {
				s.Logger().WithField("endpoint-type", added_ep.Type()).WithError(errDetach).Error("rollback hot attaching endpoint failed")
			}
		}
	}()

	// Add network for vm
	inf.PciPath = endpoints[0].PciPath().String()
	result, err := s.agent.updateInterface(ctx, inf)
	if err != nil {
		return nil, err
	}

	// Update the sandbox storage
	if err = s.Save(); err != nil {
		return nil, err
	}

	return result, nil
}

// RemoveInterface removes a nic of the sandbox.
func (s *Sandbox) RemoveInterface(ctx context.Context, inf *pbTypes.Interface) (*pbTypes.Interface, error) {
	for _, endpoint := range s.network.Endpoints() {
		if endpoint.HardwareAddr() == inf.HwAddr {
			s.Logger().WithField("endpoint-type", endpoint.Type()).Info("Hot detaching endpoint")
			if err := s.network.RemoveEndpoints(ctx, s, []Endpoint{endpoint}, true); err != nil {
				return inf, err
			}

			if err := s.Save(); err != nil {
				return inf, err
			}

			break
		}
	}
	return nil, nil
}

// ListInterfaces lists all nics and their configurations in the sandbox.
func (s *Sandbox) ListInterfaces(ctx context.Context) ([]*pbTypes.Interface, error) {
	return s.agent.listInterfaces(ctx)
}

// UpdateRoutes updates the sandbox route table (e.g. for portmapping support).
func (s *Sandbox) UpdateRoutes(ctx context.Context, routes []*pbTypes.Route) ([]*pbTypes.Route, error) {
	return s.agent.updateRoutes(ctx, routes)
}

// ListRoutes lists all routes and their configurations in the sandbox.
func (s *Sandbox) ListRoutes(ctx context.Context) ([]*pbTypes.Route, error) {
	return s.agent.listRoutes(ctx)
}

const (
	// unix socket type of console
	consoleProtoUnix = "unix"

	// pty type of console.
	consoleProtoPty = "pty"
)

// console watcher is designed to monitor guest console output.
type consoleWatcher struct {
	conn       net.Conn
	ptyConsole *os.File
	proto      string
	consoleURL string
}

func newConsoleWatcher(ctx context.Context, s *Sandbox) (*consoleWatcher, error) {
	var (
		err error
		cw  consoleWatcher
	)

	cw.proto, cw.consoleURL, err = s.hypervisor.GetVMConsole(ctx, s.id)
	if err != nil {
		return nil, err
	}

	return &cw, nil
}

// start the console watcher
func (cw *consoleWatcher) start(s *Sandbox) (err error) {
	if cw.consoleWatched() {
		return fmt.Errorf("console watcher has already watched for sandbox %s", s.id)
	}

	var scanner *bufio.Scanner

	switch cw.proto {
	case consoleProtoUnix:
		cw.conn, err = net.Dial("unix", cw.consoleURL)
		if err != nil {
			return err
		}
		scanner = bufio.NewScanner(cw.conn)
	case consoleProtoPty:
		// read-only
		cw.ptyConsole, _ = os.Open(cw.consoleURL)
		scanner = bufio.NewScanner(cw.ptyConsole)
	default:
		return fmt.Errorf("unknown console proto %s", cw.proto)
	}

	go func() {
		for scanner.Scan() {
			text := scanner.Text()
			if text != "" {
				s.Logger().WithFields(logrus.Fields{
					"console-protocol": cw.proto,
					"console-url":      cw.consoleURL,
					"sandbox":          s.id,
					"vmconsole":        text,
				}).Debug("reading guest console")
			}
		}

		if err := scanner.Err(); err != nil {
			s.Logger().WithError(err).WithFields(logrus.Fields{
				"console-protocol": cw.proto,
				"console-url":      cw.consoleURL,
				"sandbox":          s.id,
			}).Error("Failed to read guest console logs")
		} else { // The error is `nil` in case of io.EOF
			s.Logger().Info("console watcher quits")
		}
	}()

	return nil
}

// Check if the console watcher has already watched the vm console.
func (cw *consoleWatcher) consoleWatched() bool {
	return cw.conn != nil || cw.ptyConsole != nil
}

// stop the console watcher.
func (cw *consoleWatcher) stop() {
	if cw.conn != nil {
		cw.conn.Close()
		cw.conn = nil
	}

	if cw.ptyConsole != nil {
		cw.ptyConsole.Close()
		cw.ptyConsole = nil
	}
}

func (s *Sandbox) addSwap(ctx context.Context, swapID string, size int64) (*config.BlockDrive, error) {
	swapFile := filepath.Join(getSandboxPath(s.id), swapID)

	swapFD, err := os.OpenFile(swapFile, os.O_CREATE, 0600)
	if err != nil {
		err = fmt.Errorf("creat swapfile %s fail %s", swapFile, err.Error())
		s.Logger().WithError(err).Error("addSwap")
		return nil, err
	}
	swapFD.Close()
	defer func() {
		if err != nil {
			os.Remove(swapFile)
		}
	}()

	// Check the size
	pagesize := os.Getpagesize()
	// mkswap refuses areas smaller than 10 pages.
	size = int64(math.Max(float64(size), float64(pagesize*10)))
	// Swapfile need a page to store the metadata
	size += int64(pagesize)

	err = os.Truncate(swapFile, size)
	if err != nil {
		err = fmt.Errorf("truncate swapfile %s fail %s", swapFile, err.Error())
		s.Logger().WithError(err).Error("addSwap")
		return nil, err
	}

	var outbuf, errbuf bytes.Buffer
	cmd := exec.CommandContext(ctx, mkswapPath, swapFile)
	cmd.Stdout = &outbuf
	cmd.Stderr = &errbuf
	err = cmd.Run()
	if err != nil {
		err = fmt.Errorf("mkswap swapfile %s fail %s stdout %s stderr %s", swapFile, err.Error(), outbuf.String(), errbuf.String())
		s.Logger().WithError(err).Error("addSwap")
		return nil, err
	}

	blockDevice := &config.BlockDrive{
		File:   swapFile,
		Format: "raw",
		ID:     swapID,
		Swap:   true,
	}
	_, err = s.hypervisor.HotplugAddDevice(ctx, blockDevice, BlockDev)
	if err != nil {
		err = fmt.Errorf("add swapfile %s device to VM fail %s", swapFile, err.Error())
		s.Logger().WithError(err).Error("addSwap")
		return nil, err
	}
	defer func() {
		if err != nil {
			_, e := s.hypervisor.HotplugRemoveDevice(ctx, blockDevice, BlockDev)
			if e != nil {
				s.Logger().Errorf("remove swapfile %s to VM fail %s", swapFile, e.Error())
			}
		}
	}()

	err = s.agent.addSwap(ctx, blockDevice.PCIPath)
	if err != nil {
		err = fmt.Errorf("agent add swapfile %s PCIPath %+v to VM fail %s", swapFile, blockDevice.PCIPath, err.Error())
		s.Logger().WithError(err).Error("addSwap")
		return nil, err
	}

	s.Logger().Infof("add swapfile %s size %d PCIPath %+v to VM success", swapFile, size, blockDevice.PCIPath)

	return blockDevice, nil
}

func (s *Sandbox) removeSwap(ctx context.Context, blockDevice *config.BlockDrive) error {
	err := os.Remove(blockDevice.File)
	if err != nil {
		err = fmt.Errorf("remove swapfile %s fail %s", blockDevice.File, err.Error())
		s.Logger().WithError(err).Error("removeSwap")
	} else {
		s.Logger().Infof("remove swapfile %s success", blockDevice.File)
	}
	return err
}

func (s *Sandbox) setupSwap(ctx context.Context, sizeBytes int64) error {
	if sizeBytes > s.swapSizeBytes {
		dev, err := s.addSwap(ctx, fmt.Sprintf("swap%d", s.swapDeviceNum), sizeBytes-s.swapSizeBytes)
		if err != nil {
			return err
		}

		s.swapDeviceNum += 1
		s.swapSizeBytes = sizeBytes
		s.swapDevices = append(s.swapDevices, dev)
	}

	return nil
}

func (s *Sandbox) cleanSwap(ctx context.Context) {
	for _, dev := range s.swapDevices {
		err := s.removeSwap(ctx, dev)
		if err != nil {
			s.Logger().Warnf("remove swap device %+v got error %s", dev, err)
		}
	}
}

func (s *Sandbox) runPrestartHooks(ctx context.Context, prestartHookFunc func(context.Context) error) error {
	hid, _ := s.GetHypervisorPid()
	// Ignore errors here as hypervisor might not have been started yet, likely in FC case.
	if hid > 0 {
		s.Logger().Infof("sandbox %s hypervisor pid is %v", s.id, hid)
		ctx = context.WithValue(ctx, HypervisorPidKey{}, hid)
	}

	if err := prestartHookFunc(ctx); err != nil {
		s.Logger().Errorf("fail to run prestartHook for sandbox %s: %s", s.id, err)
		return err
	}

	return nil
}

// startVM starts the VM.
func (s *Sandbox) startVM(ctx context.Context, prestartHookFunc func(context.Context) error) (err error) {
	span, ctx := katatrace.Trace(ctx, s.Logger(), "startVM", sandboxTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	s.Logger().Info("Starting VM")

	if s.config.HypervisorConfig.Debug {
		// create console watcher
		consoleWatcher, err := newConsoleWatcher(ctx, s)
		if err != nil {
			return err
		}
		s.cw = consoleWatcher
	}

	defer func() {
		if err != nil {
			// Log error, otherwise nobody might see it - StopVM could kill this process.
			s.Logger().WithError(err).Error("Cannot start VM")
			s.hypervisor.StopVM(ctx, false)
		}
	}()

	caps := s.hypervisor.Capabilities(ctx)
	// If the hypervisor does not support device hotplug, run prestart hooks
	// before spawning the VM so that it is possible to let the hooks set up
	// netns and thus network devices are set up statically.
	if !caps.IsNetworkDeviceHotplugSupported() && prestartHookFunc != nil {
		err = s.runPrestartHooks(ctx, prestartHookFunc)
		if err != nil {
			return err
		}
	}

	if err := s.network.Run(ctx, func() error {
		if s.factory != nil {
			vm, err := s.factory.GetVM(ctx, VMConfig{
				HypervisorType:   s.config.HypervisorType,
				HypervisorConfig: s.config.HypervisorConfig,
				AgentConfig:      s.config.AgentConfig,
			})
			if err != nil {
				return err
			}

			return vm.assignSandbox(s)
		}

		return s.hypervisor.StartVM(ctx, VmStartTimeout)
	}); err != nil {
		return err
	}

	if caps.IsNetworkDeviceHotplugSupported() && prestartHookFunc != nil {
		err = s.runPrestartHooks(ctx, prestartHookFunc)
		if err != nil {
			return err
		}
	}

	// 1. Do not scan the netns if we want no network for the vmm
	// 2. Do not scan the netns if the vmm does not support device hotplug, in which case
	//    the network is already set up statically
	// 3. In case of vm factory, scan the netns to hotplug interfaces after vm is started.
	// 4. In case of prestartHookFunc, network config might have been changed. We need to
	//    rescan and handle the change.
	if !s.config.NetworkConfig.DisableNewNetwork &&
		caps.IsNetworkDeviceHotplugSupported() &&
		(s.factory != nil || prestartHookFunc != nil) {
		if _, err := s.network.AddEndpoints(ctx, s, nil, true); err != nil {
			return err
		}
	}

	s.Logger().Info("VM started")

	if s.cw != nil {
		s.Logger().Debug("console watcher starts")
		if err := s.cw.start(s); err != nil {
			s.cw.stop()
			return err
		}
	}

	// Once the hypervisor is done starting the sandbox,
	// we want to guarantee that it is manageable.
	// For that we need to ask the agent to start the
	// sandbox inside the VM.
	if err := s.agent.startSandbox(ctx, s); err != nil {
		return err
	}

	s.Logger().Info("Agent started in the sandbox")

	defer func() {
		if err != nil {
			if e := s.agent.stopSandbox(ctx, s); e != nil {
				s.Logger().WithError(e).WithField("sandboxid", s.id).Warning("Agent did not stop sandbox")
			}
		}
	}()

	return nil
}

// stopVM: stop the sandbox's VM
func (s *Sandbox) stopVM(ctx context.Context) error {
	span, ctx := katatrace.Trace(ctx, s.Logger(), "stopVM", sandboxTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	s.Logger().Info("Stopping sandbox in the VM")
	if err := s.agent.stopSandbox(ctx, s); err != nil {
		s.Logger().WithError(err).WithField("sandboxid", s.id).Warning("Agent did not stop sandbox")
	}

	s.Logger().Info("Stopping VM")

	return s.hypervisor.StopVM(ctx, s.disableVMShutdown)
}

func (s *Sandbox) addContainer(c *Container) error {
	if _, ok := s.containers[c.id]; ok {
		return fmt.Errorf("Duplicated container: %s", c.id)
	}
	s.containers[c.id] = c

	return nil
}

// CreateContainer creates a new container in the sandbox
// This should be called only when the sandbox is already created.
// It will add new container config to sandbox.config.Containers
func (s *Sandbox) CreateContainer(ctx context.Context, contConfig ContainerConfig) (VCContainer, error) {
	// Update sandbox config to include the new container's config
	s.config.Containers = append(s.config.Containers, contConfig)

	var err error

	defer func() {
		if err != nil {
			if len(s.config.Containers) > 0 {
				// delete container config
				s.config.Containers = s.config.Containers[:len(s.config.Containers)-1]
			}
		}
	}()

	// Create the container object, add devices to the sandbox's device-manager:
	c, err := newContainer(ctx, s, &s.config.Containers[len(s.config.Containers)-1])
	if err != nil {
		return nil, err
	}
	// create and start the container
	if err = c.create(ctx); err != nil {
		return nil, err
	}

	// Add the container to the containers list in the sandbox.
	if err = s.addContainer(c); err != nil {
		return nil, err
	}

	defer func() {
		// Rollback if error happens.
		if err != nil {
			logger := s.Logger().WithFields(logrus.Fields{"container": c.id, "sandbox": s.id, "rollback": true})
			logger.WithError(err).Error("Cleaning up partially created container")

			if errStop := c.stop(ctx, true); errStop != nil {
				logger.WithError(errStop).Error("Could not stop container")
			}

			logger.Debug("Removing stopped container from sandbox store")
			s.removeContainer(c.id)
		}
	}()

	// Sandbox is responsible to update VM resources needed by Containers
	// Update resources after having added containers to the sandbox, since
	// container status is requiered to know if more resources should be added.
	if err = s.updateResources(ctx); err != nil {
		return nil, err
	}

	if err = s.resourceControllerUpdate(ctx); err != nil {
		return nil, err
	}

	if err = s.checkVCPUsPinning(ctx); err != nil {
		return nil, err
	}

	if err = s.storeSandbox(ctx); err != nil {
		return nil, err
	}

	return c, nil
}

// StartContainer starts a container in the sandbox
func (s *Sandbox) StartContainer(ctx context.Context, containerID string) (VCContainer, error) {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	// Start it.
	if err = c.start(ctx); err != nil {
		return nil, err
	}

	if err = s.storeSandbox(ctx); err != nil {
		return nil, err
	}

	s.Logger().WithField("container", containerID).Info("Container is started")

	// Update sandbox resources in case a stopped container
	// is started
	if err = s.updateResources(ctx); err != nil {
		return nil, err
	}

	if err = s.checkVCPUsPinning(ctx); err != nil {
		return nil, err
	}

	return c, nil
}

// StopContainer stops a container in the sandbox
func (s *Sandbox) StopContainer(ctx context.Context, containerID string, force bool) (VCContainer, error) {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	// Stop it.
	if err := c.stop(ctx, force); err != nil {
		return nil, err
	}

	if err = s.storeSandbox(ctx); err != nil {
		return nil, err
	}
	return c, nil
}

// KillContainer signals a container in the sandbox
func (s *Sandbox) KillContainer(ctx context.Context, containerID string, signal syscall.Signal, all bool) error {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	// Send a signal to the process.
	err = c.kill(ctx, signal, all)

	// SIGKILL should never fail otherwise it is
	// impossible to clean things up.
	if signal == syscall.SIGKILL {
		return nil
	}

	return err
}

// DeleteContainer deletes a container from the sandbox
func (s *Sandbox) DeleteContainer(ctx context.Context, containerID string) (VCContainer, error) {
	if containerID == "" {
		return nil, types.ErrNeedContainerID
	}

	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	// Delete it.
	if err = c.delete(ctx); err != nil {
		return nil, err
	}

	// Update sandbox config
	for idx, contConfig := range s.config.Containers {
		if contConfig.ID == containerID {
			s.config.Containers = append(s.config.Containers[:idx], s.config.Containers[idx+1:]...)
			break
		}
	}

	// update the sandbox resource controller
	if err = s.resourceControllerUpdate(ctx); err != nil {
		return nil, err
	}

	if err = s.checkVCPUsPinning(ctx); err != nil {
		return nil, err
	}

	if err = s.storeSandbox(ctx); err != nil {
		return nil, err
	}
	return c, nil
}

// StatusContainer gets the status of a container
func (s *Sandbox) StatusContainer(containerID string) (ContainerStatus, error) {
	if containerID == "" {
		return ContainerStatus{}, types.ErrNeedContainerID
	}

	if c, ok := s.containers[containerID]; ok {
		rootfs := c.config.RootFs.Source
		if c.config.RootFs.Mounted {
			rootfs = c.config.RootFs.Target
		}

		return ContainerStatus{
			ID:          c.id,
			State:       c.state,
			PID:         c.process.Pid,
			StartTime:   c.process.StartTime,
			RootFs:      rootfs,
			Annotations: c.config.Annotations,
		}, nil
	}

	return ContainerStatus{}, types.ErrNoSuchContainer
}

// EnterContainer is the virtcontainers container command execution entry point.
// EnterContainer enters an already running container and runs a given command.
func (s *Sandbox) EnterContainer(ctx context.Context, containerID string, cmd types.Cmd) (VCContainer, *Process, error) {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, nil, err
	}

	// Enter it.
	process, err := c.enter(ctx, cmd)
	if err != nil {
		return nil, nil, err
	}

	return c, process, nil
}

// UpdateContainer update a running container.
func (s *Sandbox) UpdateContainer(ctx context.Context, containerID string, resources specs.LinuxResources) error {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	if err = c.update(ctx, resources); err != nil {
		return err
	}

	if err := s.resourceControllerUpdate(ctx); err != nil {
		return err
	}

	if err = s.checkVCPUsPinning(ctx); err != nil {
		return err
	}

	if err = s.storeSandbox(ctx); err != nil {
		return err
	}
	return nil
}

// StatsContainer return the stats of a running container
func (s *Sandbox) StatsContainer(ctx context.Context, containerID string) (ContainerStats, error) {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return ContainerStats{}, err
	}

	stats, err := c.stats(ctx)
	if err != nil {
		return ContainerStats{}, err
	}
	return *stats, nil
}

// Stats returns the stats of a running sandbox
func (s *Sandbox) Stats(ctx context.Context) (SandboxStats, error) {

	metrics, err := s.sandboxController.Stat()
	if err != nil {
		return SandboxStats{}, err
	}

	stats := SandboxStats{}

	// TODO Do we want to aggregate the overhead cgroup stats to the sandbox ones?
	switch mt := metrics.(type) {
	case *v1.Metrics:
		stats.CgroupStats.CPUStats.CPUUsage.TotalUsage = mt.CPU.Usage.Total
		stats.CgroupStats.MemoryStats.Usage.Usage = mt.Memory.Usage.Usage
	case *v2.Metrics:
		stats.CgroupStats.CPUStats.CPUUsage.TotalUsage = mt.CPU.UsageUsec
		stats.CgroupStats.MemoryStats.Usage.Usage = mt.Memory.Usage
	default:
		return SandboxStats{}, fmt.Errorf("unknown metrics type %T", mt)
	}

	tids, err := s.hypervisor.GetThreadIDs(ctx)
	if err != nil {
		return stats, err
	}
	stats.Cpus = len(tids.vcpus)

	return stats, nil
}

// PauseContainer pauses a running container.
func (s *Sandbox) PauseContainer(ctx context.Context, containerID string) error {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	// Pause the container.
	if err := c.pause(ctx); err != nil {
		return err
	}

	if err = s.storeSandbox(ctx); err != nil {
		return err
	}
	return nil
}

// ResumeContainer resumes a paused container.
func (s *Sandbox) ResumeContainer(ctx context.Context, containerID string) error {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	// Resume the container.
	if err := c.resume(ctx); err != nil {
		return err
	}

	if err = s.storeSandbox(ctx); err != nil {
		return err
	}
	return nil
}

// createContainers registers all containers, create the
// containers in the guest.
func (s *Sandbox) createContainers(ctx context.Context) error {
	span, ctx := katatrace.Trace(ctx, s.Logger(), "createContainers", sandboxTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	for i := range s.config.Containers {
		c, err := newContainer(ctx, s, &s.config.Containers[i])
		if err != nil {
			return err
		}
		if err := c.create(ctx); err != nil {
			return err
		}

		if err := s.addContainer(c); err != nil {
			return err
		}
	}

	// Update resources after having added containers to the sandbox, since
	// container status is required to know if more resources should be added.
	if err := s.updateResources(ctx); err != nil {
		return err
	}
	if err := s.resourceControllerUpdate(ctx); err != nil {
		return err
	}

	if err := s.checkVCPUsPinning(ctx); err != nil {
		return err
	}

	if err := s.storeSandbox(ctx); err != nil {
		return err
	}
	return nil
}

// Start starts a sandbox. The containers that are making the sandbox
// will be started.
func (s *Sandbox) Start(ctx context.Context) error {
	if err := s.state.ValidTransition(s.state.State, types.StateRunning); err != nil {
		return err
	}

	prevState := s.state.State

	if err := s.setSandboxState(types.StateRunning); err != nil {
		return err
	}

	var startErr error
	defer func() {
		if startErr != nil {
			s.setSandboxState(prevState)
		}
	}()
	for _, c := range s.containers {
		if startErr = c.start(ctx); startErr != nil {
			return startErr
		}
	}

	if err := s.storeSandbox(ctx); err != nil {
		return err
	}

	s.Logger().Info("Sandbox is started")

	return nil
}

// Stop stops a sandbox. The containers that are making the sandbox
// will be destroyed.
// When force is true, ignore guest related stop failures.
func (s *Sandbox) Stop(ctx context.Context, force bool) error {
	span, ctx := katatrace.Trace(ctx, s.Logger(), "Stop", sandboxTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	if s.state.State == types.StateStopped {
		s.Logger().Info("sandbox already stopped")
		return nil
	}

	if err := s.state.ValidTransition(s.state.State, types.StateStopped); err != nil {
		return err
	}

	for _, c := range s.containers {
		if err := c.stop(ctx, force); err != nil {
			return err
		}
	}

	if err := s.stopVM(ctx); err != nil && !force {
		return err
	}

	// shutdown console watcher if exists
	if s.cw != nil {
		s.Logger().Debug("stop the console watcher")
		s.cw.stop()
	}

	if err := s.setSandboxState(types.StateStopped); err != nil {
		return err
	}

	// Remove the network.
	if err := s.removeNetwork(ctx); err != nil && !force {
		return err
	}

	if err := s.storeSandbox(ctx); err != nil {
		return err
	}

	// Stop communicating with the agent.
	if err := s.agent.disconnect(ctx); err != nil && !force {
		return err
	}

	s.cleanSwap(ctx)

	return nil
}

// setSandboxState sets the in-memory state of the sandbox.
func (s *Sandbox) setSandboxState(state types.StateString) error {
	if state == "" {
		return types.ErrNeedState
	}

	// update in-memory state
	s.state.State = state

	return nil
}

const maxBlockIndex = 65535

// getAndSetSandboxBlockIndex retrieves an unused sandbox block index from
// the BlockIndexMap and marks it as used. This index is used to maintain the
// index at which a block device is assigned to a container in the sandbox.
func (s *Sandbox) getAndSetSandboxBlockIndex() (int, error) {
	currentIndex := -1
	for i := 0; i < maxBlockIndex; i++ {
		if _, ok := s.state.BlockIndexMap[i]; !ok {
			currentIndex = i
			break
		}
	}
	if currentIndex == -1 {
		return -1, errors.New("no available block index")
	}
	s.state.BlockIndexMap[currentIndex] = struct{}{}

	return currentIndex, nil
}

// unsetSandboxBlockIndex deletes the current sandbox block index from BlockIndexMap.
// This is used to recover from failure while adding a block device.
func (s *Sandbox) unsetSandboxBlockIndex(index int) error {
	var err error
	original := index
	delete(s.state.BlockIndexMap, index)
	defer func() {
		if err != nil {
			s.state.BlockIndexMap[original] = struct{}{}
		}
	}()

	return nil
}

// HotplugAddDevice is used for add a device to sandbox
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) HotplugAddDevice(ctx context.Context, device api.Device, devType config.DeviceType) error {
	span, ctx := katatrace.Trace(ctx, s.Logger(), "HotplugAddDevice", sandboxTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	if s.sandboxController != nil {
		if err := s.sandboxController.AddDevice(device.GetHostPath()); err != nil {
			s.Logger().WithError(err).WithField("device", device).
				Warnf("Could not add device to the %s controller", s.sandboxController)
		}
	}

	switch devType {
	case config.DeviceVFIO:
		vfioDevices, ok := device.GetDeviceInfo().([]*config.VFIODev)
		if !ok {
			return fmt.Errorf("device type mismatch, expect device type to be %s", devType)
		}

		// adding a group of VFIO devices
		for _, dev := range vfioDevices {
			if _, err := s.hypervisor.HotplugAddDevice(ctx, dev, VfioDev); err != nil {
				s.Logger().
					WithFields(logrus.Fields{
						"sandbox":         s.id,
						"vfio-device-ID":  dev.ID,
						"vfio-device-BDF": dev.BDF,
					}).WithError(err).Error("failed to hotplug VFIO device")
				return err
			}
		}
		return nil
	case config.DeviceBlock:
		blockDevice, ok := device.(*drivers.BlockDevice)
		if !ok {
			return fmt.Errorf("device type mismatch, expect device type to be %s", devType)
		}
		_, err := s.hypervisor.HotplugAddDevice(ctx, blockDevice.BlockDrive, BlockDev)
		return err
	case config.VhostUserBlk:
		vhostUserBlkDevice, ok := device.(*drivers.VhostUserBlkDevice)

		if !ok {
			return fmt.Errorf("device type mismatch, expect device type to be %s", devType)
		}
		_, err := s.hypervisor.HotplugAddDevice(ctx, vhostUserBlkDevice.VhostUserDeviceAttrs, VhostuserDev)
		return err
	case config.DeviceGeneric:
		// TODO: what?
		return nil
	}
	return nil
}

// HotplugRemoveDevice is used for removing a device from sandbox
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) HotplugRemoveDevice(ctx context.Context, device api.Device, devType config.DeviceType) error {
	defer func() {
		if s.sandboxController != nil {
			if err := s.sandboxController.RemoveDevice(device.GetHostPath()); err != nil {
				s.Logger().WithError(err).WithField("device", device).
					Warnf("Could not add device to the %s controller", s.sandboxController)
			}
		}
	}()

	switch devType {
	case config.DeviceVFIO:
		vfioDevices, ok := device.GetDeviceInfo().([]*config.VFIODev)
		if !ok {
			return fmt.Errorf("device type mismatch, expect device type to be %s", devType)
		}

		// remove a group of VFIO devices
		for _, dev := range vfioDevices {
			if _, err := s.hypervisor.HotplugRemoveDevice(ctx, dev, VfioDev); err != nil {
				s.Logger().WithError(err).
					WithFields(logrus.Fields{
						"sandbox":         s.id,
						"vfio-device-ID":  dev.ID,
						"vfio-device-BDF": dev.BDF,
					}).Error("failed to hot unplug VFIO device")
				return err
			}
		}
		return nil
	case config.DeviceBlock:
		blockDrive, ok := device.GetDeviceInfo().(*config.BlockDrive)
		if !ok {
			return fmt.Errorf("device type mismatch, expect device type to be %s", devType)
		}
		// PMEM devices cannot be hot removed
		if blockDrive.Pmem {
			s.Logger().WithField("path", blockDrive.File).Infof("Skip device: cannot hot remove PMEM devices")
			return nil
		}
		_, err := s.hypervisor.HotplugRemoveDevice(ctx, blockDrive, BlockDev)
		return err
	case config.VhostUserBlk:
		vhostUserDeviceAttrs, ok := device.GetDeviceInfo().(*config.VhostUserDeviceAttrs)
		if !ok {
			return fmt.Errorf("device type mismatch, expect device type to be %s", devType)
		}
		_, err := s.hypervisor.HotplugRemoveDevice(ctx, vhostUserDeviceAttrs, VhostuserDev)
		return err
	case config.DeviceGeneric:
		// TODO: what?
		return nil
	}
	return nil
}

// GetAndSetSandboxBlockIndex is used for getting and setting virtio-block indexes
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) GetAndSetSandboxBlockIndex() (int, error) {
	return s.getAndSetSandboxBlockIndex()
}

// UnsetSandboxBlockIndex unsets block indexes
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) UnsetSandboxBlockIndex(index int) error {
	return s.unsetSandboxBlockIndex(index)
}

// AppendDevice can only handle vhost user device currently, it adds a
// vhost user device to sandbox
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) AppendDevice(ctx context.Context, device api.Device) error {
	switch device.DeviceType() {
	case config.VhostUserSCSI, config.VhostUserNet, config.VhostUserBlk, config.VhostUserFS:
		return s.hypervisor.AddDevice(ctx, device.GetDeviceInfo().(*config.VhostUserDeviceAttrs), VhostuserDev)
	case config.DeviceVFIO:
		vfioDevs := device.GetDeviceInfo().([]*config.VFIODev)
		for _, d := range vfioDevs {
			return s.hypervisor.AddDevice(ctx, *d, VfioDev)
		}
	default:
		s.Logger().WithField("device-type", device.DeviceType()).
			Warn("Could not append device: unsupported device type")
	}

	return fmt.Errorf("unsupported device type")
}

// AddDevice will add a device to sandbox
func (s *Sandbox) AddDevice(ctx context.Context, info config.DeviceInfo) (api.Device, error) {
	if s.devManager == nil {
		return nil, fmt.Errorf("device manager isn't initialized")
	}

	var err error
	add, err := s.devManager.NewDevice(info)
	if err != nil {
		return nil, err
	}
	defer func() {
		if err != nil {
			s.devManager.RemoveDevice(add.DeviceID())
		}
	}()

	if err = s.devManager.AttachDevice(ctx, add.DeviceID(), s); err != nil {
		return nil, err
	}
	defer func() {
		if err != nil {
			s.devManager.DetachDevice(ctx, add.DeviceID(), s)
		}
	}()

	return add, nil
}

// GetVfioDeviceGuestPciPath return a device's guest PCI path by its host BDF
func (s *Sandbox) GetVfioDeviceGuestPciPath(hostBDF string) types.PciPath {
	devices := s.devManager.GetAllDevices()
	for _, device := range devices {
		switch device.DeviceType() {
		case config.DeviceVFIO:
			vfioDevices, ok := device.GetDeviceInfo().([]*config.VFIODev)
			if !ok {
				continue
			}
			for _, vfioDev := range vfioDevices {
				if vfioDev.BDF == hostBDF {
					return vfioDev.GuestPciPath
				}
			}
		default:
			continue
		}
	}

	return types.PciPath{}
}

// updateResources will:
// - calculate the resources required for the virtual machine, and adjust the virtual machine
// sizing accordingly. For a given sandbox, it will calculate the number of vCPUs required based
// on the sum of container requests, plus default CPUs for the VM. Similar is done for memory.
// If changes in memory or CPU are made, the VM will be updated and the agent will online the
// applicable CPU and memory.
func (s *Sandbox) updateResources(ctx context.Context) error {
	if s == nil {
		return errors.New("sandbox is nil")
	}

	if s.config == nil {
		return fmt.Errorf("sandbox config is nil")
	}

	if s.config.StaticResourceMgmt {
		s.Logger().Debug("no resources updated: static resource management is set")
		return nil
	}

	sandboxVCPUs, err := s.calculateSandboxCPUs()
	if err != nil {
		return err
	}
	// Add default vcpus for sandbox
	sandboxVCPUs += s.hypervisor.HypervisorConfig().NumVCPUsF

	sandboxMemoryByte, sandboxneedPodSwap, sandboxSwapByte := s.calculateSandboxMemory()

	// Add default / rsvd memory for sandbox.
	hypervisorMemoryByteI64 := int64(s.hypervisor.HypervisorConfig().MemorySize) << utils.MibToBytesShift
	hypervisorMemoryByte := uint64(hypervisorMemoryByteI64)
	sandboxMemoryByte += hypervisorMemoryByte
	if sandboxneedPodSwap {
		sandboxSwapByte += hypervisorMemoryByteI64
	}
	s.Logger().WithField("sandboxMemoryByte", sandboxMemoryByte).WithField("sandboxneedPodSwap", sandboxneedPodSwap).WithField("sandboxSwapByte", sandboxSwapByte).Debugf("updateResources: after calculateSandboxMemory")

	// Setup the SWAP in the guest
	if sandboxSwapByte > 0 {
		err = s.setupSwap(ctx, sandboxSwapByte)
		if err != nil {
			return err
		}
	}

	// Update VCPUs
	s.Logger().WithField("cpus-sandbox", sandboxVCPUs).Debugf("Request to hypervisor to update vCPUs")
	oldCPUs, newCPUs, err := s.hypervisor.ResizeVCPUs(ctx, RoundUpNumVCPUs(sandboxVCPUs))
	if err != nil {
		return err
	}

	s.Logger().Debugf("Request to hypervisor to update oldCPUs/newCPUs: %d/%d", oldCPUs, newCPUs)
	// If the CPUs were increased, ask agent to online them
	if oldCPUs < newCPUs {
		s.Logger().Debugf("Request to onlineCPUMem with %d CPUs", newCPUs)
		if err := s.agent.onlineCPUMem(ctx, newCPUs, true); err != nil {
			return err
		}
	}
	s.Logger().Debugf("Sandbox CPUs: %d", newCPUs)

	// Update Memory --
	// If we're using ACPI hotplug for memory, there's a limitation on the amount of memory which can be hotplugged at a single time.
	// We must have enough free memory in the guest kernel to cover 64bytes per (4KiB) page of memory added for mem_map.
	// See https://github.com/kata-containers/kata-containers/issues/4847 for more details.
	// For a typical pod lifecycle, we expect that each container is added when we start the workloads. Based on this, we'll "assume" that majority
	// of the guest memory is readily available. From experimentation, we see that we can add approximately 48 times what is already provided to
	// the guest workload. For example, a 256 MiB guest should be able to accommodate hotplugging 12 GiB of memory.
	//
	// If virtio-mem is being used, there isn't such a limitation - we can hotplug the maximum allowed memory at a single time.
	//
	newMemoryMB := uint32(sandboxMemoryByte >> utils.MibToBytesShift)
	finalMemoryMB := newMemoryMB

	hconfig := s.hypervisor.HypervisorConfig()

	for {
		currentMemoryMB := s.hypervisor.GetTotalMemoryMB(ctx)

		maxhotPluggableMemoryMB := currentMemoryMB * acpiMemoryHotplugFactor

		// In the case of virtio-mem, we don't have a restriction on how much can be hotplugged at
		// a single time. As a result, the max hotpluggable is only limited by the maximum memory size
		// of the guest.
		if hconfig.VirtioMem {
			maxhotPluggableMemoryMB = uint32(hconfig.DefaultMaxMemorySize) - currentMemoryMB
		}

		deltaMB := int32(finalMemoryMB - currentMemoryMB)

		if deltaMB > int32(maxhotPluggableMemoryMB) {
			s.Logger().Warnf("Large hotplug. Adding %d MB of %d total memory", maxhotPluggableMemoryMB, deltaMB)
			newMemoryMB = currentMemoryMB + maxhotPluggableMemoryMB
		} else {
			newMemoryMB = finalMemoryMB
		}

		// Add the memory to the guest and online the memory:
		if err := s.updateMemory(ctx, newMemoryMB); err != nil {
			return err
		}

		if newMemoryMB == finalMemoryMB {
			break
		}
	}

	tmpfsMounts, err := s.prepareEphemeralMounts(finalMemoryMB)
	if err != nil {
		return err
	}
	if err := s.agent.updateEphemeralMounts(ctx, tmpfsMounts); err != nil {
		// upgrade path: if runtime is newer version, but agent is old
		// then ignore errUnimplemented
		if grpcStatus.Convert(err).Code() == codes.Unimplemented {
			s.Logger().Warnf("agent does not support updateMounts")
			return nil
		}
		return err
	}

	return nil
}

func (s *Sandbox) prepareEphemeralMounts(memoryMB uint32) ([]*grpc.Storage, error) {
	tmpfsMounts := []*grpc.Storage{}
	for _, c := range s.containers {
		for _, mount := range c.mounts {
			// if a tmpfs ephemeral mount is present
			// update its size to occupy the entire sandbox's memory
			if mount.Type == KataEphemeralDevType {
				sizeLimited := false
				for _, opt := range mount.Options {
					if strings.HasPrefix(opt, "size") {
						sizeLimited = true
					}
				}
				if sizeLimited { // do not resize sizeLimited emptyDirs
					continue
				}

				mountOptions := []string{"remount", fmt.Sprintf("size=%dM", memoryMB)}

				origin_src := mount.Source
				stat := syscall.Stat_t{}
				err := syscall.Stat(origin_src, &stat)
				if err != nil {
					return nil, err
				}

				// if volume's gid isn't root group(default group), this means there's
				// an specific fsGroup is set on this local volume, then it should pass
				// to guest.
				if stat.Gid != 0 {
					mountOptions = append(mountOptions, fmt.Sprintf("%s=%d", fsGid, stat.Gid))
				}

				tmpfsMounts = append(tmpfsMounts, &grpc.Storage{
					Driver:     KataEphemeralDevType,
					MountPoint: filepath.Join(ephemeralPath(), filepath.Base(mount.Source)),
					Source:     "tmpfs",
					Fstype:     "tmpfs",
					Options:    mountOptions,
				})
			}
		}
	}
	return tmpfsMounts, nil
}

func (s *Sandbox) updateMemory(ctx context.Context, newMemoryMB uint32) error {
	// online the memory:
	s.Logger().WithField("memory-sandbox-size-mb", newMemoryMB).Debugf("Request to hypervisor to update memory")
	newMemory, updatedMemoryDevice, err := s.hypervisor.ResizeMemory(ctx, newMemoryMB, s.state.GuestMemoryBlockSizeMB, s.state.GuestMemoryHotplugProbe)
	if err != nil {
		if err == noGuestMemHotplugErr {
			s.Logger().Warnf("%s, memory specifications cannot be guaranteed", err)
		} else {
			return err
		}
	}
	s.Logger().Debugf("Sandbox memory size: %d MB", newMemory)
	if s.state.GuestMemoryHotplugProbe && updatedMemoryDevice.Addr != 0 {
		// notify the guest kernel about memory hot-add event, before onlining them
		s.Logger().Debugf("notify guest kernel memory hot-add event via probe interface, memory device located at 0x%x", updatedMemoryDevice.Addr)
		if err := s.agent.memHotplugByProbe(ctx, updatedMemoryDevice.Addr, uint32(updatedMemoryDevice.SizeMB), s.state.GuestMemoryBlockSizeMB); err != nil {
			return err
		}
	}
	if err := s.agent.onlineCPUMem(ctx, 0, false); err != nil {
		return err
	}
	return nil
}

func (s *Sandbox) calculateSandboxMemory() (uint64, bool, int64) {
	memorySandbox := uint64(0)
	needPodSwap := false
	swapSandbox := int64(0)
	for _, c := range s.config.Containers {
		// Do not hot add again non-running containers resources
		if cont, ok := s.containers[c.ID]; ok && cont.state.State == types.StateStopped {
			s.Logger().WithField("container", c.ID).Debug("Do not taking into account memory resources of not running containers")
			continue
		}

		if m := c.Resources.Memory; m != nil {
			currentLimit := int64(0)
			if m.Limit != nil && *m.Limit > 0 {
				currentLimit = *m.Limit
				memorySandbox += uint64(currentLimit)
				s.Logger().WithField("memory limit", memorySandbox).Info("Memory Sandbox + Memory Limit ")
			}

			// Add hugepages memory
			// HugepageLimit is uint64 - https://github.com/opencontainers/runtime-spec/blob/master/specs-go/config.go#L242
			for _, l := range c.Resources.HugepageLimits {
				memorySandbox += l.Limit
			}

			// Add swap
			if s.config.HypervisorConfig.GuestSwap && m.Swappiness != nil && *m.Swappiness > 0 {
				currentSwap := int64(0)
				if m.Swap != nil {
					currentSwap = *m.Swap
				}
				if currentSwap == 0 {
					if currentLimit == 0 {
						needPodSwap = true
					} else {
						swapSandbox += currentLimit
					}
				} else if currentSwap > currentLimit {
					swapSandbox = currentSwap - currentLimit
				}
			}
		}
	}

	return memorySandbox, needPodSwap, swapSandbox
}

func (s *Sandbox) calculateSandboxCPUs() (float32, error) {
	floatCPU := float32(0)
	cpusetCount := int(0)

	for _, c := range s.config.Containers {
		// Do not hot add again non-running containers resources
		if cont, ok := s.containers[c.ID]; ok && cont.state.State == types.StateStopped {
			s.Logger().WithField("container", c.ID).Debug("Do not taking into account CPU resources of not running containers")
			continue
		}

		if cpu := c.Resources.CPU; cpu != nil {
			if cpu.Period != nil && cpu.Quota != nil {
				floatCPU += utils.CalculateCPUsF(*cpu.Quota, *cpu.Period)
			}

			set, err := cpuset.Parse(cpu.Cpus)
			if err != nil {
				return 0, nil
			}
			cpusetCount += set.Size()
		}
	}

	// If we aren't being constrained, then we could have two scenarios:
	//  1. BestEffort QoS: no proper support today in Kata.
	//  2. We could be constrained only by CPUSets. Check for this:
	if floatCPU == 0 && cpusetCount > 0 {
		return float32(cpusetCount), nil
	}

	return floatCPU, nil
}

// GetHypervisorType is used for getting Hypervisor name currently used.
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) GetHypervisorType() string {
	return string(s.config.HypervisorType)
}

// resourceControllerUpdate updates the sandbox cpuset resource controller
// (Linux cgroup) subsystem.
// Also, if the sandbox has an overhead controller, it updates the hypervisor
// constraints by moving the potentially new vCPU threads back to the sandbox
// controller.
func (s *Sandbox) resourceControllerUpdate(ctx context.Context) error {
	cpuset, memset, err := s.getSandboxCPUSet()
	if err != nil {
		return err
	}

	// We update the sandbox controller with potentially new virtual CPUs.
	if err := s.sandboxController.UpdateCpuSet(cpuset, memset); err != nil {
		return err
	}

	if s.overheadController != nil {
		// If we have an overhead controller, new vCPU threads would start there,
		// as being children of the VMM PID.
		// We need to constrain them by moving them into the sandbox controller.
		if err := s.constrainHypervisor(ctx); err != nil {
			return err
		}
	}

	return nil
}

// resourceControllerDelete will move the running processes in the sandbox resource
// cvontroller to the parent and then delete the sandbox controller.
func (s *Sandbox) resourceControllerDelete() error {
	s.Logger().Debugf("Deleting sandbox %s resource controler", s.sandboxController)
	if s.state.SandboxCgroupPath == "" {
		s.Logger().Warnf("sandbox %s resource controler path is empty", s.sandboxController)
		return nil
	}

	sandboxController, err := resCtrl.LoadResourceController(s.state.SandboxCgroupPath)
	if err != nil {
		return err
	}

	resCtrlParent := sandboxController.Parent()
	if err := sandboxController.MoveTo(resCtrlParent); err != nil {
		return err
	}

	if err := sandboxController.Delete(); err != nil {
		return err
	}

	if s.state.OverheadCgroupPath != "" {
		overheadController, err := resCtrl.LoadResourceController(s.state.OverheadCgroupPath)
		if err != nil {
			return err
		}

		resCtrlParent := overheadController.Parent()
		if err := s.overheadController.MoveTo(resCtrlParent); err != nil {
			return err
		}

		if err := overheadController.Delete(); err != nil {
			return err
		}
	}

	return nil
}

// constrainHypervisor will place the VMM and vCPU threads into resource controllers (cgroups on Linux).
func (s *Sandbox) constrainHypervisor(ctx context.Context) error {
	tids, err := s.hypervisor.GetThreadIDs(ctx)
	if err != nil {
		return fmt.Errorf("failed to get thread ids from hypervisor: %v", err)
	}

	// All vCPU threads move to the sandbox controller.
	for _, i := range tids.vcpus {
		if err := s.sandboxController.AddThread(i); err != nil {
			return err
		}
	}

	return nil
}

// setupResourceController adds the runtime process to either the sandbox resource controller or the
// overhead one, depending on the sandbox_cgroup_only configuration setting.
func (s *Sandbox) setupResourceController() error {
	vmmController := s.sandboxController
	if s.overheadController != nil {
		vmmController = s.overheadController
	}

	// By adding the runtime process to either the sandbox or overhead controller, we are making
	// sure that any child process of the runtime (i.e. *all* processes serving a Kata pod)
	// will initially live in this controller. Depending on the sandbox_cgroup settings, we will
	// then move the vCPU threads between resource controllers.
	runtimePid := os.Getpid()
	// Add the runtime to the VMM sandbox resource controller
	if err := vmmController.AddProcess(runtimePid); err != nil {
		return fmt.Errorf("Could not add runtime PID %d to the sandbox %s resource controller: %v", runtimePid, s.sandboxController, err)
	}

	return nil
}

// GetPatchedOCISpec returns sandbox's OCI specification
// This OCI specification was patched when the sandbox was created
// by containerCapabilities(), SetEphemeralStorageType() and others
// in order to support:
// * Capabilities
// * Ephemeral storage
// * k8s empty dir
// If you need the original (vanilla) OCI spec,
// use compatoci.GetContainerSpec() instead.
func (s *Sandbox) GetPatchedOCISpec() *specs.Spec {
	if s.config == nil {
		return nil
	}

	// Get the container associated with the PodSandbox annotation.
	// In Kubernetes, this represents the pause container.
	// In CRI-compliant runtimes like Containerd, this is the container.
	// On Linux, we derive the cgroup path from this container.
	for _, cConfig := range s.config.Containers {
		if ContainerType(cConfig.Annotations[annotations.ContainerTypeKey]).IsSandbox() {
			return cConfig.CustomSpec
		}
	}

	return nil
}

func (s *Sandbox) GetOOMEvent(ctx context.Context) (string, error) {
	return s.agent.getOOMEvent(ctx)
}

func (s *Sandbox) GetAgentURL() (string, error) {
	return s.agent.getAgentURL()
}

// GetIPTables will obtain the iptables from the guest
func (s *Sandbox) GetIPTables(ctx context.Context, isIPv6 bool) ([]byte, error) {
	return s.agent.getIPTables(ctx, isIPv6)
}

// SetIPTables will set the iptables in the guest
func (s *Sandbox) SetIPTables(ctx context.Context, isIPv6 bool, data []byte) error {
	return s.agent.setIPTables(ctx, isIPv6, data)
}

// SetPolicy will set the policy in the guest
func (s *Sandbox) SetPolicy(ctx context.Context, policy string) error {
	return s.agent.setPolicy(ctx, policy)
}

// GuestVolumeStats return the filesystem stat of a given volume in the guest.
func (s *Sandbox) GuestVolumeStats(ctx context.Context, volumePath string) ([]byte, error) {
	guestMountPath, err := s.guestMountPath(volumePath)
	if err != nil {
		return nil, err
	}
	return s.agent.getGuestVolumeStats(ctx, guestMountPath)
}

// ResizeGuestVolume resizes a volume in the guest.
func (s *Sandbox) ResizeGuestVolume(ctx context.Context, volumePath string, size uint64) error {
	// TODO: https://github.com/kata-containers/kata-containers/issues/3694.
	guestMountPath, err := s.guestMountPath(volumePath)
	if err != nil {
		return err
	}
	return s.agent.resizeGuestVolume(ctx, guestMountPath, size)
}

func (s *Sandbox) guestMountPath(volumePath string) (string, error) {
	// verify the device even exists
	if _, err := os.Stat(volumePath); err != nil {
		s.Logger().WithError(err).WithField("volume", volumePath).Error("Cannot get stats for volume that doesn't exist")
		return "", err
	}

	// verify that we have a mount in this sandbox who's source maps to this
	for _, c := range s.containers {
		for _, m := range c.mounts {
			if volumePath == m.Source {
				return m.GuestDeviceMount, nil
			}
		}
	}
	return "", fmt.Errorf("mount %s not found in sandbox", volumePath)
}

// getSandboxCPUSet returns the union of each of the sandbox's containers' CPU sets'
// cpus and mems as a string in canonical linux CPU/mems list format
func (s *Sandbox) getSandboxCPUSet() (string, string, error) {
	if s.config == nil {
		return "", "", nil
	}

	cpuResult := cpuset.NewCPUSet()
	memResult := cpuset.NewCPUSet()
	for _, ctr := range s.config.Containers {
		if ctr.Resources.CPU != nil {
			currCPUSet, err := cpuset.Parse(ctr.Resources.CPU.Cpus)
			if err != nil {
				return "", "", fmt.Errorf("unable to parse CPUset.cpus for container %s: %v", ctr.ID, err)
			}
			cpuResult = cpuResult.Union(currCPUSet)

			currMemSet, err := cpuset.Parse(ctr.Resources.CPU.Mems)
			if err != nil {
				return "", "", fmt.Errorf("unable to parse CPUset.mems for container %s: %v", ctr.ID, err)
			}
			memResult = memResult.Union(currMemSet)
		}
	}

	return cpuResult.String(), memResult.String(), nil
}

// fetchSandbox fetches a sandbox config from a sandbox ID and returns a sandbox.
func fetchSandbox(ctx context.Context, sandboxID string) (sandbox *Sandbox, err error) {
	virtLog.Info("fetch sandbox")
	if sandboxID == "" {
		return nil, types.ErrNeedSandboxID
	}

	var config SandboxConfig

	// Load sandbox config fromld store.
	c, err := loadSandboxConfig(sandboxID)
	if err != nil {
		virtLog.WithError(err).Warning("failed to get sandbox config from store")
		return nil, err
	}

	config = *c

	// fetchSandbox is not suppose to create new sandbox VM.
	sandbox, err = createSandbox(ctx, config, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create sandbox with config %+v: %v", config, err)
	}

	// This sandbox already exists, we don't need to recreate the containers in the guest.
	// We only need to fetch the containers from storage and create the container structs.
	if err := sandbox.fetchContainers(ctx); err != nil {
		return nil, err
	}

	return sandbox, nil
}

// fetchContainers creates new containers structure and
// adds them to the sandbox. It does not create the containers
// in the guest. This should only be used when fetching a
// sandbox that already exists.
func (s *Sandbox) fetchContainers(ctx context.Context) error {
	for i, contConfig := range s.config.Containers {
		// Add spec from bundle path
		spec, err := compatoci.GetContainerSpec(contConfig.Annotations)
		if err != nil {
			return err
		}
		contConfig.CustomSpec = &spec
		s.config.Containers[i] = contConfig

		c, err := newContainer(ctx, s, &s.config.Containers[i])
		if err != nil {
			return err
		}

		if err := s.addContainer(c); err != nil {
			return err
		}
	}

	return nil
}

// checkVCPUsPinning is used to support CPUSet mode of kata container.
// CPUSet mode is on when Sandbox.HypervisorConfig.EnableVCPUsPinning
// is set to true. Then it fetches sandbox's number of vCPU threads
// and number of CPUs in CPUSet. If the two are equal, each vCPU thread
// is then pinned to one fixed CPU in CPUSet.
func (s *Sandbox) checkVCPUsPinning(ctx context.Context) error {
	if s.config == nil {
		return fmt.Errorf("no sandbox config found")
	}
	if !s.config.EnableVCPUsPinning {
		return nil
	}

	// fetch vCPU thread ids and CPUSet
	vCPUThreadsMap, err := s.hypervisor.GetThreadIDs(ctx)
	if err != nil {
		return fmt.Errorf("failed to get vCPU thread ids from hypervisor: %v", err)
	}
	cpuSetStr, _, err := s.getSandboxCPUSet()
	if err != nil {
		return fmt.Errorf("failed to get CPUSet config: %v", err)
	}
	cpuSet, err := cpuset.Parse(cpuSetStr)
	if err != nil {
		return fmt.Errorf("failed to parse CPUSet string: %v", err)
	}
	cpuSetSlice := cpuSet.ToSlice()

	// check if vCPU thread numbers and CPU numbers are equal
	numVCPUs, numCPUs := len(vCPUThreadsMap.vcpus), len(cpuSetSlice)
	// if not equal, we should reset threads scheduling to random pattern
	if numVCPUs != numCPUs {
		if s.isVCPUsPinningOn {
			s.isVCPUsPinningOn = false
			return s.resetVCPUsPinning(ctx, vCPUThreadsMap, cpuSetSlice)
		}
		return nil
	}
	// if equal, we can use vCPU thread pinning
	for i, tid := range vCPUThreadsMap.vcpus {
		if err := resCtrl.SetThreadAffinity(tid, cpuSetSlice[i:i+1]); err != nil {
			if err := s.resetVCPUsPinning(ctx, vCPUThreadsMap, cpuSetSlice); err != nil {
				return err
			}
			return fmt.Errorf("failed to set vcpu thread %d affinity to cpu %d: %v", tid, cpuSetSlice[i], err)
		}
	}
	s.isVCPUsPinningOn = true
	return nil
}

// resetVCPUsPinning cancels current pinning and restores default random vCPU threads scheduling
func (s *Sandbox) resetVCPUsPinning(ctx context.Context, vCPUThreadsMap VcpuThreadIDs, cpuSetSlice []int) error {
	for _, tid := range vCPUThreadsMap.vcpus {
		if err := resCtrl.SetThreadAffinity(tid, cpuSetSlice); err != nil {
			return fmt.Errorf("failed to reset vcpu thread %d affinity: %v", tid, err)
		}
	}
	return nil
}
