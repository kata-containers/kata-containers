// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2014,2015,2016,2017 Docker, Inc.
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"syscall"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/manager"
	deviceManager "github.com/kata-containers/kata-containers/src/runtime/pkg/device/manager"
	volume "github.com/kata-containers/kata-containers/src/runtime/pkg/direct-volume"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"

	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
)

// tracingTags defines tags for the trace span
var containerTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "container",
}

// https://github.com/torvalds/linux/blob/master/include/uapi/linux/major.h
// This file has definitions for major device numbers.
var cdromMajors = map[int64]string{
	11: "SCSI_CDROM_MAJOR",
	15: "CDU31A_CDROM_MAJOR",
	16: "GOLDSTAR_CDROM_MAJOR",
	17: "OPTICS_CDROM_MAJOR",
	18: "SANYO_CDROM_MAJOR",
	20: "MITSUMI_X_CDROM_MAJOR",
	23: "MITSUMI_CDROM_MAJOR",
	24: "CDU535_CDROM_MAJOR",
	25: "MATSUSHITA_CDROM_MAJOR",
	26: "MATSUSHITA_CDROM2_MAJOR",
	27: "MATSUSHITA_CDROM3_MAJOR",
	28: "MATSUSHITA_CDROM4_MAJOR",
	29: "AZTECH_CDROM_MAJOR",
	32: "CM206_CDROM_MAJOR",
}

// https://github.com/torvalds/linux/blob/master/include/uapi/linux/major.h
// #define FLOPPY_MAJOR		2
const floppyMajor = int64(2)

// Process gathers data related to a container process.
type Process struct {
	StartTime time.Time

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
}

// ContainerStatus describes a container status.
type ContainerStatus struct {
	Spec *specs.Spec

	// Annotations allow clients to store arbitrary values,
	// for example to add additional status values required
	// to support particular specifications.
	Annotations map[string]string

	ID        string
	RootFs    string
	StartTime time.Time
	State     types.ContainerState

	PID int
}

// ThrottlingData gather the date related to container cpu throttling.
type ThrottlingData struct {
	// Number of periods with throttling active
	Periods uint64 `json:"periods,omitempty"`
	// Number of periods when the container hit its throttling limit.
	ThrottledPeriods uint64 `json:"throttled_periods,omitempty"`
	// Aggregate time the container was throttled for in nanoseconds.
	ThrottledTime uint64 `json:"throttled_time,omitempty"`
}

// CPUUsage denotes the usage of a CPU.
// All CPU stats are aggregate since container inception.
type CPUUsage struct {
	// Total CPU time consumed per core.
	// Units: nanoseconds.
	PercpuUsage []uint64 `json:"percpu_usage,omitempty"`
	// Total CPU time consumed.
	// Units: nanoseconds.
	TotalUsage uint64 `json:"total_usage,omitempty"`
	// Time spent by tasks of the cgroup in kernel mode.
	// Units: nanoseconds.
	UsageInKernelmode uint64 `json:"usage_in_kernelmode"`
	// Time spent by tasks of the cgroup in user mode.
	// Units: nanoseconds.
	UsageInUsermode uint64 `json:"usage_in_usermode"`
}

// CPUStats describes the cpu stats
type CPUStats struct {
	CPUUsage       CPUUsage       `json:"cpu_usage,omitempty"`
	ThrottlingData ThrottlingData `json:"throttling_data,omitempty"`
}

// MemoryData gather the data related to memory
type MemoryData struct {
	Usage    uint64 `json:"usage,omitempty"`
	MaxUsage uint64 `json:"max_usage,omitempty"`
	Failcnt  uint64 `json:"failcnt"`
	Limit    uint64 `json:"limit"`
}

// MemoryStats describes the memory stats
type MemoryStats struct {
	Stats map[string]uint64 `json:"stats,omitempty"`
	// usage of memory
	Usage MemoryData `json:"usage,omitempty"`
	// usage of memory  swap
	SwapUsage MemoryData `json:"swap_usage,omitempty"`
	// usage of kernel memory
	KernelUsage MemoryData `json:"kernel_usage,omitempty"`
	// usage of kernel TCP memory
	KernelTCPUsage MemoryData `json:"kernel_tcp_usage,omitempty"`
	// memory used for cache
	Cache uint64 `json:"cache,omitempty"`
	// if true, memory usage is accounted for throughout a hierarchy of cgroups.
	UseHierarchy bool `json:"use_hierarchy"`
}

// PidsStats describes the pids stats
type PidsStats struct {
	// number of pids in the cgroup
	Current uint64 `json:"current,omitempty"`
	// active pids hard limit
	Limit uint64 `json:"limit,omitempty"`
}

// BlkioStatEntry gather date related to a block device
type BlkioStatEntry struct {
	Op    string `json:"op,omitempty"`
	Major uint64 `json:"major,omitempty"`
	Minor uint64 `json:"minor,omitempty"`
	Value uint64 `json:"value,omitempty"`
}

// BlkioStats describes block io stats
type BlkioStats struct {
	// number of bytes tranferred to and from the block device
	IoServiceBytesRecursive []BlkioStatEntry `json:"io_service_bytes_recursive,omitempty"`
	IoServicedRecursive     []BlkioStatEntry `json:"io_serviced_recursive,omitempty"`
	IoQueuedRecursive       []BlkioStatEntry `json:"io_queue_recursive,omitempty"`
	IoServiceTimeRecursive  []BlkioStatEntry `json:"io_service_time_recursive,omitempty"`
	IoWaitTimeRecursive     []BlkioStatEntry `json:"io_wait_time_recursive,omitempty"`
	IoMergedRecursive       []BlkioStatEntry `json:"io_merged_recursive,omitempty"`
	IoTimeRecursive         []BlkioStatEntry `json:"io_time_recursive,omitempty"`
	SectorsRecursive        []BlkioStatEntry `json:"sectors_recursive,omitempty"`
}

// HugetlbStats describes hugetable memory stats
type HugetlbStats struct {
	// current res_counter usage for hugetlb
	Usage uint64 `json:"usage,omitempty"`
	// maximum usage ever recorded.
	MaxUsage uint64 `json:"max_usage,omitempty"`
	// number of times hugetlb usage allocation failure.
	Failcnt uint64 `json:"failcnt"`
}

// CgroupStats describes all cgroup subsystem stats
type CgroupStats struct {
	// the map is in the format "size of hugepage: stats of the hugepage"
	HugetlbStats map[string]HugetlbStats `json:"hugetlb_stats,omitempty"`
	BlkioStats   BlkioStats              `json:"blkio_stats,omitempty"`
	CPUStats     CPUStats                `json:"cpu_stats,omitempty"`
	MemoryStats  MemoryStats             `json:"memory_stats,omitempty"`
	PidsStats    PidsStats               `json:"pids_stats,omitempty"`
}

// NetworkStats describe all network stats.
type NetworkStats struct {
	// Name is the name of the network interface.
	Name string `json:"name,omitempty"`

	RxBytes   uint64 `json:"rx_bytes,omitempty"`
	RxPackets uint64 `json:"rx_packets,omitempty"`
	RxErrors  uint64 `json:"rx_errors,omitempty"`
	RxDropped uint64 `json:"rx_dropped,omitempty"`
	TxBytes   uint64 `json:"tx_bytes,omitempty"`
	TxPackets uint64 `json:"tx_packets,omitempty"`
	TxErrors  uint64 `json:"tx_errors,omitempty"`
	TxDropped uint64 `json:"tx_dropped,omitempty"`
}

// ContainerStats describes a container stats.
type ContainerStats struct {
	CgroupStats  *CgroupStats
	NetworkStats []*NetworkStats
}

// ContainerResources describes container resources
type ContainerResources struct {
	// VCPUs are the number of vCPUs that are being used by the container
	VCPUs uint32

	// Mem is the memory that is being used by the container
	MemByte int64
}

// ContainerConfig describes one container runtime configuration.
type ContainerConfig struct {
	// Device configuration for devices that must be available within the container.
	DeviceInfos []config.DeviceInfo

	Mounts []Mount

	// Raw OCI specification, it won't be saved to disk.
	CustomSpec *specs.Spec `json:"-"`

	// Annotations allow clients to store arbitrary values,
	// for example to add additional status values required
	// to support particular specifications.
	Annotations map[string]string

	ID string

	// Resources container resources
	Resources specs.LinuxResources

	// Cmd specifies the command to run on a container
	Cmd types.Cmd

	// RootFs is the container workload image on the host.
	RootFs RootFs

	// ReadOnlyRootfs indicates if the rootfs should be mounted readonly
	ReadonlyRootfs bool
}

// valid checks that the container configuration is valid.
func (c *ContainerConfig) valid() bool {
	if c == nil {
		return false
	}

	if c.ID == "" {
		return false
	}

	return true
}

// SystemMountsInfo describes additional information for system mounts that the agent
// needs to handle
type SystemMountsInfo struct {
	// Indicates if /dev has been passed as a bind mount for the host /dev
	BindMountDev bool

	// Size of /dev/shm assigned on the host.
	DevShmSize uint
}

// ContainerDevice describes a device associated with container
type ContainerDevice struct {
	// ID is device id referencing the device from sandbox's device manager
	ID string

	// ContainerPath is device path displayed in container
	ContainerPath string

	// FileMode permission bits for the device.
	FileMode os.FileMode

	// UID is user ID in the container namespace
	UID uint32

	// GID is group ID in the container namespace
	GID uint32
}

// RootFs describes the container's rootfs.
type RootFs struct {
	// Source specifies the BlockDevice path
	Source string
	// Target specify where the rootfs is mounted if it has been mounted
	Target string
	// Type specifies the type of filesystem to mount.
	Type string
	// Options specifies zero or more fstab style mount options.
	Options []string
	// Mounted specifies whether the rootfs has be mounted or not
	Mounted bool
}

// Container is composed of a set of containers and a runtime environment.
// A Container can be created, deleted, started, stopped, listed, entered, paused and restored.
type Container struct {
	ctx context.Context

	config  *ContainerConfig
	sandbox *Sandbox

	id            string
	sandboxID     string
	containerPath string
	rootfsSuffix  string

	mounts []Mount

	devices []ContainerDevice

	state types.ContainerState

	process Process

	rootFs RootFs

	systemMountsInfo SystemMountsInfo
}

// ID returns the container identifier string.
func (c *Container) ID() string {
	return c.id
}

// Logger returns a logrus logger appropriate for logging Container messages
func (c *Container) Logger() *logrus.Entry {
	return virtLog.WithFields(logrus.Fields{
		"subsystem": "container",
		"sandbox":   c.sandboxID,
		"container": c.id,
	})
}

// Sandbox returns the sandbox handler related to this container.
func (c *Container) Sandbox() VCSandbox {
	return c.sandbox
}

// Process returns the container process.
func (c *Container) Process() Process {
	return c.process
}

// GetToken returns the token related to this container's process.
func (c *Container) GetToken() string {
	return c.process.Token
}

// GetPid returns the pid related to this container's process.
func (c *Container) GetPid() int {
	return c.process.Pid
}

func (c *Container) setStateFstype(fstype string) error {
	c.state.Fstype = fstype

	return nil
}

// GetAnnotations returns container's annotations
func (c *Container) GetAnnotations() map[string]string {
	return c.config.Annotations
}

// GetPatchedOCISpec returns container's OCI specification
// This OCI specification was patched when the sandbox was created
// by containerCapabilities(), SetEphemeralStorageType() and others
// in order to support:
// * Capabilities
// * Ephemeral storage
// * k8s empty dir
// If you need the original (vanilla) OCI spec,
// use compatoci.GetContainerSpec() instead.
func (c *Container) GetPatchedOCISpec() *specs.Spec {
	return c.config.CustomSpec
}

// setContainerState sets both the in-memory and on-disk state of the
// container.
func (c *Container) setContainerState(state types.StateString) error {
	if state == "" {
		return types.ErrNeedState
	}

	c.Logger().Debugf("Setting container state from %v to %v", c.state.State, state)
	// update in-memory state
	c.state.State = state

	// flush data to storage
	if err := c.sandbox.Save(); err != nil {
		return err
	}

	return nil
}

// mountSharedDirMounts handles bind-mounts by bindmounting to the host shared
// directory which is mounted through virtiofs/9pfs in the VM.
// It also updates the container mount list with the HostPath info, and store
// container mounts to the storage. This way, we will have the HostPath info
// available when we will need to unmount those mounts.
func (c *Container) mountSharedDirMounts(ctx context.Context, sharedDirMounts, ignoredMounts map[string]Mount) (storages []*grpc.Storage, err error) {
	var devicesToDetach []string
	defer func() {
		if err != nil {
			for _, id := range devicesToDetach {
				c.sandbox.devManager.DetachDevice(ctx, id, c.sandbox)
			}
		}
	}()

	for idx, m := range c.mounts {
		// Skip mounting certain system paths from the source on the host side
		// into the container as it does not make sense to do so.
		// Example sources could be /sys/fs/cgroup etc.
		if isSystemMount(m.Source) {
			continue
		}

		// Check if mount is a block device file. If it is, the block device will be attached to the host
		// instead of passing this as a shared mount:
		if len(m.BlockDeviceID) > 0 {
			// Attach this block device, all other devices passed in the config have been attached at this point
			if err = c.sandbox.devManager.AttachDevice(ctx, m.BlockDeviceID, c.sandbox); err != nil {
				return storages, err
			}
			devicesToDetach = append(devicesToDetach, m.BlockDeviceID)
			continue
		}

		// For non-block based mounts, we are only interested in bind mounts
		if m.Type != "bind" {
			continue
		}

		// We need to treat /dev/shm as a special case. This is passed as a bind mount in the spec,
		// but it does not make sense to pass this as a 9p mount from the host side.
		// This needs to be handled purely in the guest, by allocating memory for this inside the VM.
		if m.Destination == "/dev/shm" {
			continue
		}

		// Ignore /dev, directories and all other device files. We handle
		// only regular files in /dev. It does not make sense to pass the host
		// device nodes to the guest.
		if isHostDevice(m.Destination) {
			continue
		}

		sharedFile, err := c.sandbox.fsShare.ShareFile(ctx, c, &c.mounts[idx])
		if err != nil {
			return storages, err
		}

		// Expand the list of mounts to ignore.
		if sharedFile == nil {
			ignoredMounts[m.Source] = Mount{Source: m.Source}
			continue
		}
		sharedDirMount := Mount{
			Source:      sharedFile.guestPath,
			Destination: m.Destination,
			Type:        m.Type,
			Options:     m.Options,
			ReadOnly:    m.ReadOnly,
		}

		// virtiofs does not support inotify. To workaround this limitation, we want to special case
		// mounts that are commonly 'watched'. "watchable" mounts include:
		//  - Kubernetes configmap
		//  - Kubernetes secret
		// If we identify one of these, we'll need to carry out polling in the guest in order to present the
		// container with a mount that supports inotify. To do this, we create a Storage object for
		// the "watchable-bind" driver. This will have the agent create a new mount that is watchable,
		// who's effective source is the original mount (the agent will poll the original mount for changes and
		// manually update the path that is mounted into the container).
		// Based on this, let's make sure we update the sharedDirMount structure with the new watchable-mount as
		// the source (this is what is utilized to update the OCI spec).
		caps := c.sandbox.hypervisor.Capabilities(ctx)
		if isWatchableMount(m.Source) && caps.IsFsSharingSupported() {

			// Create path in shared directory for creating watchable mount:
			watchableHostPath := filepath.Join(getMountPath(c.sandboxID), "watchable")
			if err := os.MkdirAll(watchableHostPath, DirMode); err != nil {
				return storages, fmt.Errorf("unable to create watchable path: %s: %v", watchableHostPath, err)
			}

			watchableGuestMount := filepath.Join(kataGuestSharedDir(), "watchable", filepath.Base(sharedFile.guestPath))

			storage := &grpc.Storage{
				Driver:     kataWatchableBindDevType,
				Source:     sharedFile.guestPath,
				Fstype:     "bind",
				MountPoint: watchableGuestMount,
				Options:    m.Options,
			}
			storages = append(storages, storage)

			// Update the sharedDirMount, in order to identify what will
			// change in the OCI spec.
			sharedDirMount.Source = watchableGuestMount
		}

		sharedDirMounts[sharedDirMount.Destination] = sharedDirMount
	}

	return storages, nil
}

func (c *Container) unmountHostMounts(ctx context.Context) error {
	span, ctx := katatrace.Trace(ctx, c.Logger(), "unmountHostMounts", containerTracingTags, map[string]string{"container_id": c.id})
	defer span.End()

	unmountFunc := func(m Mount) (err error) {
		span, _ := katatrace.Trace(ctx, c.Logger(), "unmount", containerTracingTags, map[string]string{"container_id": c.id, "host-path": m.HostPath})
		defer func() {
			if err != nil {
				katatrace.AddTags(span, "error", err)
			}
			span.End()
		}()

		if err = c.sandbox.fsShare.UnshareFile(ctx, c, &m); err != nil {
			c.Logger().WithFields(logrus.Fields{
				"host-path": m.HostPath,
				"error":     err,
			}).Warn("Could not umount")
			return err
		}

		return nil
	}

	for _, m := range c.mounts {
		if m.HostPath != "" {
			if err := unmountFunc(m); err != nil {
				return err
			}
		}
	}

	return nil
}

func filterDevices(c *Container, devices []ContainerDevice) (ret []ContainerDevice) {
	for _, dev := range devices {
		major, _ := c.sandbox.devManager.GetDeviceByID(dev.ID).GetMajorMinor()
		if _, ok := cdromMajors[major]; ok {
			c.Logger().WithFields(logrus.Fields{
				"device": dev.ContainerPath,
			}).Info("Not attach device because it is a CDROM")
			continue
		}

		if major == floppyMajor {
			c.Logger().WithFields(logrus.Fields{
				"device": dev.ContainerPath,
			}).Info("Not attaching device because it is a floppy drive")
			continue
		}

		ret = append(ret, dev)
	}
	return
}

// Add any mount based block devices to the device manager and Save the
// device ID for the particular mount. This'll occur when the mountpoint source
// is a block device.
func (c *Container) createBlockDevices(ctx context.Context) error {
	if !c.checkBlockDeviceSupport(ctx) {
		c.Logger().Warn("Block device not supported")
		return nil
	}

	// iterate all mounts and create block device if it's block based.
	for i := range c.mounts {
		if len(c.mounts[i].BlockDeviceID) > 0 {
			// Non-empty m.BlockDeviceID indicates there's already one device
			// associated with the mount,so no need to create a new device for it
			// and we only create block device for bind mount
			continue
		}

		isBlockFile := HasOption(c.mounts[i].Options, vcAnnotations.IsFileBlockDevice)
		if c.mounts[i].Type != "bind" && !isBlockFile {
			// We only handle for bind and block device mounts.
			continue
		}

		// Handle directly assigned volume. Update the mount info based on the mount info json.
		mntInfo, e := volume.VolumeMountInfo(c.mounts[i].Source)
		if e != nil && !os.IsNotExist(e) {
			c.Logger().WithError(e).WithField("mount-source", c.mounts[i].Source).
				Error("failed to parse the mount info file for a direct assigned volume")
			continue
		}

		if mntInfo != nil {
			// Write out sandbox info file on the mount source to allow CSI to communicate with the runtime
			if err := volume.RecordSandboxId(c.sandboxID, c.mounts[i].Source); err != nil {
				c.Logger().WithError(err).Error("error writing sandbox info")
			}

			readonly := false
			for _, flag := range mntInfo.Options {
				if flag == "ro" {
					readonly = true
					break
				}
			}

			c.mounts[i].Source = mntInfo.Device
			c.mounts[i].Type = mntInfo.FsType
			c.mounts[i].Options = mntInfo.Options
			c.mounts[i].ReadOnly = readonly

			for key, value := range mntInfo.Metadata {
				switch key {
				case volume.FSGroupMetadataKey:
					gid, err := strconv.Atoi(value)
					if err != nil {
						c.Logger().WithError(err).Errorf("invalid group id value %s provided for key %s", value, volume.FSGroupMetadataKey)
						continue
					}
					c.mounts[i].FSGroup = &gid
				case volume.FSGroupChangePolicyMetadataKey:
					if _, exists := mntInfo.Metadata[volume.FSGroupMetadataKey]; !exists {
						c.Logger().Errorf("%s specified without provding the group id with key %s", volume.FSGroupChangePolicyMetadataKey, volume.FSGroupMetadataKey)
						continue
					}
					c.mounts[i].FSGroupChangePolicy = volume.FSGroupChangePolicy(value)
				default:
					c.Logger().Warnf("Ignoring unsupported direct-assignd volume metadata key: %s, value: %s", key, value)
				}
			}
		}

		// Check if mount is a block device file. If it is, the block device will be attached to the host
		// instead of passing this as a shared mount.
		di, err := c.createDeviceInfo(c.mounts[i].Source, c.mounts[i].Destination, c.mounts[i].ReadOnly, isBlockFile)
		if err == nil && di != nil {
			b, err := c.sandbox.devManager.NewDevice(*di)
			if err != nil {
				// Do not return an error, try to create
				// devices for other mounts
				c.Logger().WithError(err).WithField("mount-source", c.mounts[i].Source).
					Error("device manager failed to create new device")
				continue

			}

			c.mounts[i].BlockDeviceID = b.DeviceID()
		}
	}

	return nil
}

func (c *Container) initConfigResourcesMemory() {
	ociSpec := c.GetPatchedOCISpec()
	c.config.Resources.Memory = &specs.LinuxMemory{}
	ociSpec.Linux.Resources.Memory = c.config.Resources.Memory
}

// newContainer creates a Container structure from a sandbox and a container configuration.
func newContainer(ctx context.Context, sandbox *Sandbox, contConfig *ContainerConfig) (*Container, error) {
	span, ctx := katatrace.Trace(ctx, nil, "newContainer", containerTracingTags, map[string]string{"container_id": contConfig.ID, "sandbox_id": sandbox.id})
	defer span.End()

	if !contConfig.valid() {
		return &Container{}, fmt.Errorf("Invalid container configuration")
	}

	c := &Container{
		id:            contConfig.ID,
		sandboxID:     sandbox.id,
		rootFs:        contConfig.RootFs,
		config:        contConfig,
		sandbox:       sandbox,
		containerPath: filepath.Join(sandbox.id, contConfig.ID),
		rootfsSuffix:  "rootfs",
		state:         types.ContainerState{},
		process:       Process{},
		mounts:        contConfig.Mounts,
		ctx:           sandbox.ctx,
	}

	// Set the Annotations of SWAP to Resources
	if resourceSwappinessStr, ok := c.config.Annotations[vcAnnotations.ContainerResourcesSwappiness]; ok {
		resourceSwappiness, err := strconv.ParseUint(resourceSwappinessStr, 0, 64)
		if err == nil && resourceSwappiness > 200 {
			err = fmt.Errorf("swapiness should not bigger than 200")
		}
		if err != nil {
			return &Container{}, fmt.Errorf("Invalid container configuration Annotations %s %v", vcAnnotations.ContainerResourcesSwappiness, err)
		}
		if c.config.Resources.Memory == nil {
			c.initConfigResourcesMemory()
		}
		c.config.Resources.Memory.Swappiness = &resourceSwappiness
	}
	if resourceSwapInBytesStr, ok := c.config.Annotations[vcAnnotations.ContainerResourcesSwapInBytes]; ok {
		resourceSwapInBytesInUint, err := strconv.ParseUint(resourceSwapInBytesStr, 0, 64)
		if err != nil {
			return &Container{}, fmt.Errorf("Invalid container configuration Annotations %s %v", vcAnnotations.ContainerResourcesSwapInBytes, err)
		}
		if c.config.Resources.Memory == nil {
			c.initConfigResourcesMemory()
		}
		resourceSwapInBytes := int64(resourceSwapInBytesInUint)
		c.config.Resources.Memory.Swap = &resourceSwapInBytes
	}

	// experimental runtime use "persist.json" instead of legacy "state.json" as storage
	err := c.Restore()
	if err == nil {
		//container restored
		return c, nil
	}

	// Unexpected error
	if !os.IsNotExist(err) && err != errContainerPersistNotExist {
		return nil, err
	}

	// If mounts are block devices, add to devmanager
	if err := c.createMounts(ctx); err != nil {
		return nil, err
	}

	// Add container's devices to sandbox's device-manager
	if err := c.createDevices(contConfig); err != nil {
		return nil, err
	}

	return c, nil
}

// Create Device Information about the block device
func (c *Container) createDeviceInfo(source, destination string, readonly, isBlockFile bool) (*config.DeviceInfo, error) {
	var stat unix.Stat_t
	if err := unix.Stat(source, &stat); err != nil {
		return nil, fmt.Errorf("stat %q failed: %v", source, err)
	}

	var di *config.DeviceInfo
	var err error

	if stat.Mode&unix.S_IFMT == unix.S_IFBLK {
		di = &config.DeviceInfo{
			HostPath:      source,
			ContainerPath: destination,
			DevType:       "b",
			Major:         int64(unix.Major(uint64(stat.Rdev))),
			Minor:         int64(unix.Minor(uint64(stat.Rdev))),
			ReadOnly:      readonly,
		}
	} else if isBlockFile && stat.Mode&unix.S_IFMT == unix.S_IFREG {
		di = &config.DeviceInfo{
			HostPath:      source,
			ContainerPath: destination,
			DevType:       "b",
			Major:         -1,
			Minor:         0,
			ReadOnly:      readonly,
		}
		// Check whether source can be used as a pmem device
	} else if di, err = config.PmemDeviceInfo(source, destination); err != nil {
		c.Logger().WithError(err).
			WithField("mount-source", source).
			Debug("no loop device")
	}
	return di, err
}

// call hypervisor to create device about KataVirtualVolume.
func (c *Container) createVirtualVolumeDevices() ([]config.DeviceInfo, error) {
	var deviceInfos []config.DeviceInfo
	for _, o := range c.rootFs.Options {
		if strings.HasPrefix(o, VirtualVolumePrefix) {
			virtVolume, err := types.ParseKataVirtualVolume(strings.TrimPrefix(o, VirtualVolumePrefix))
			if err != nil {
				return nil, err
			}
			c.Logger().Infof("KataVirtualVolume volumetype = %s", virtVolume.VolumeType)
		}
	}
	return deviceInfos, nil
}

func (c *Container) createMounts(ctx context.Context) error {
	// Create block devices for newly created container
	return c.createBlockDevices(ctx)
}

func (c *Container) createDevices(contConfig *ContainerConfig) error {
	// If devices were not found in storage, create Device implementations
	// from the configuration. This should happen at create.
	var storedDevices []ContainerDevice
	virtualVolumesDeviceInfos, err := c.createVirtualVolumeDevices()
	if err != nil {
		return err
	}
	deviceInfos := append(virtualVolumesDeviceInfos, contConfig.DeviceInfos...)

	// If we have a confidential guest we need to cold-plug the PCIe VFIO devices
	// until we have TDISP/IDE PCIe support.
	coldPlugVFIO := (c.sandbox.config.HypervisorConfig.ColdPlugVFIO != config.NoPort)
	// Aggregate all the containner devices for hot-plug and use them to dedcue
	// the correct amount of ports to reserve for the hypervisor.
	hotPlugVFIO := (c.sandbox.config.HypervisorConfig.HotPlugVFIO != config.NoPort)

	hotPlugDevices := []config.DeviceInfo{}
	coldPlugDevices := []config.DeviceInfo{}

	for i, vfio := range deviceInfos {
		// Only considering VFIO updates for Port and ColdPlug or
		// HotPlug updates
		isVFIODevice := deviceManager.IsVFIODevice(vfio.ContainerPath)
		if hotPlugVFIO && isVFIODevice {
			deviceInfos[i].ColdPlug = false
			deviceInfos[i].Port = c.sandbox.config.HypervisorConfig.HotPlugVFIO
			hotPlugDevices = append(hotPlugDevices, deviceInfos[i])
			continue
		}
		// Device is already cold-plugged at sandbox creation time
		// ignore it for the container creation
		if coldPlugVFIO && isVFIODevice {
			coldPlugDevices = append(coldPlugDevices, deviceInfos[i])
			continue
		}
		hotPlugDevices = append(hotPlugDevices, deviceInfos[i])
	}

	// If modeVFIO is enabled we need 1st to attach the VFIO control group
	// device /dev/vfio/vfio an 2nd the actuall device(s) afterwards.
	// Sort the devices starting with device #1 being the VFIO control group
	// device and the next the actuall device(s) /dev/vfio/<group>
	if coldPlugVFIO && c.sandbox.config.VfioMode == config.VFIOModeVFIO {
		// DeviceInfo should still be added to the sandbox's device manager
		// if vfio_mode is VFIO and coldPlugVFIO is true (e.g. vfio-ap-cold).
		// This ensures that ociSpec.Linux.Devices is updated with
		// this information before the container is created on the guest.
		deviceInfos = sortContainerVFIODevices(coldPlugDevices)
	} else {
		deviceInfos = sortContainerVFIODevices(hotPlugDevices)
	}

	for _, info := range deviceInfos {
		dev, err := c.sandbox.devManager.NewDevice(info)
		if err != nil {
			return err
		}

		storedDevices = append(storedDevices, ContainerDevice{
			ID:            dev.DeviceID(),
			ContainerPath: info.ContainerPath,
			FileMode:      info.FileMode,
			UID:           info.UID,
			GID:           info.GID,
		})
	}
	c.devices = filterDevices(c, storedDevices)

	// If we're hot-plugging this will be a no-op because at this stage
	// no devices are attached to the root-port or switch-port
	c.annotateContainerWithVFIOMetadata(coldPlugDevices)

	return nil
}

// rollbackFailingContainerCreation rolls back important steps that might have
// been performed before the container creation failed.
// - Unplug CPU and memory resources from the VM.
// - Unplug devices from the VM.
func (c *Container) rollbackFailingContainerCreation(ctx context.Context) {
	if err := c.detachDevices(ctx); err != nil {
		c.Logger().WithError(err).Error("rollback failed detachDevices()")
	}
	if err := c.removeDrive(ctx); err != nil {
		c.Logger().WithError(err).Error("rollback failed removeDrive()")
	}
	if err := c.unmountHostMounts(ctx); err != nil {
		c.Logger().WithError(err).Error("rollback failed unmountHostMounts()")
	}

	if IsNydusRootFSType(c.rootFs.Type) {
		if err := nydusContainerCleanup(ctx, getMountPath(c.sandbox.id), c); err != nil {
			c.Logger().WithError(err).Error("rollback failed nydusContainerCleanup()")
		}
	} else {
		if err := c.sandbox.fsShare.UnshareRootFilesystem(ctx, c); err != nil {
			c.Logger().WithError(err).Error("rollback failed UnshareRootFilesystem()")
		}
	}
}

func (c *Container) checkBlockDeviceSupport(ctx context.Context) bool {
	if !c.sandbox.config.HypervisorConfig.DisableBlockDeviceUse {
		agentCaps := c.sandbox.agent.capabilities()
		hypervisorCaps := c.sandbox.hypervisor.Capabilities(ctx)

		if agentCaps.IsBlockDeviceSupported() && hypervisorCaps.IsBlockDeviceHotplugSupported() {
			return true
		}
	}

	return false
}

// Sort the devices starting with device #1 being the VFIO control group
// device and the next the actuall device(s) e.g. /dev/vfio/<group>
func sortContainerVFIODevices(devices []config.DeviceInfo) []config.DeviceInfo {
	var vfioDevices []config.DeviceInfo

	for _, device := range devices {
		if deviceManager.IsVFIOControlDevice(device.ContainerPath) {
			vfioDevices = append([]config.DeviceInfo{device}, vfioDevices...)
			continue
		}
		vfioDevices = append(vfioDevices, device)
	}
	return vfioDevices
}

type DeviceRelation struct {
	Bus   string
	Path  string
	Index int
}

// Depending on the HW we might need to inject metadata into the container
// In this case for the NV GPU we need to provide the correct mapping from
// VFIO-<NUM> to GPU index inside of the VM when vfio_mode="guest-kernel",
// otherwise we do not know which GPU is which.
func (c *Container) annotateContainerWithVFIOMetadata(devices interface{}) {

	modeIsGK := (c.sandbox.config.VfioMode == config.VFIOModeGuestKernel)

	if modeIsGK {
		// Hot plug is done let's update meta information about the
		// hot plugged devices especially VFIO devices in modeIsGK
		siblings := make([]DeviceRelation, 0)
		// In the sandbox we first create the root-ports and secondly
		// the switch-ports. The range over map is not deterministic
		// so lets first iterate over all root-port devices and then
		// switch-port devices no special handling for bridge-port (PCI)
		for _, dev := range config.PCIeDevicesPerPort["root-port"] {
			// For the NV GPU we need special handling let's use only those
			if dev.VendorID == "0x10de" && strings.Contains(dev.Class, "0x030") {
				siblings = append(siblings, DeviceRelation{Bus: dev.Bus, Path: dev.HostPath})
			}
		}
		for _, dev := range config.PCIeDevicesPerPort["switch-port"] {
			// For the NV GPU we need special handling let's use only those
			if dev.VendorID == "0x10de" && strings.Contains(dev.Class, "0x030") {
				siblings = append(siblings, DeviceRelation{Bus: dev.Bus, Path: dev.HostPath})
			}
		}
		// We need to sort the VFIO devices by bus to get the correct
		// ordering root-port < switch-port
		sort.Slice(siblings, func(i, j int) bool {
			return siblings[i].Bus < siblings[j].Bus
		})

		for i := range siblings {
			siblings[i].Index = i
		}

		// Now that we have the index lets connect the /dev/vfio/<num>
		// to the correct index
		if devices, ok := devices.([]ContainerDevice); ok {
			for _, dev := range devices {
				c.siblingAnnotation(dev.ContainerPath, siblings)
			}
		}

		if devices, ok := devices.([]config.DeviceInfo); ok {
			for _, dev := range devices {
				c.siblingAnnotation(dev.ContainerPath, siblings)
			}

		}

	}
}
func (c *Container) siblingAnnotation(devPath string, siblings []DeviceRelation) {
	for _, sibling := range siblings {
		if sibling.Path == devPath {
			vfioNum := filepath.Base(devPath)
			annoKey := fmt.Sprintf("cdi.k8s.io/vfio%s", vfioNum)
			annoValue := fmt.Sprintf("nvidia.com/gpu=%d", sibling.Index)
			if c.config.CustomSpec.Annotations == nil {
				c.config.CustomSpec.Annotations = make(map[string]string)
			}
			c.config.CustomSpec.Annotations[annoKey] = annoValue
			c.Logger().Infof("annotated container with %s: %s", annoKey, annoValue)
		}
	}
}

// create creates and starts a container inside a Sandbox. It has to be
// called only when a new container, not known by the sandbox, has to be created.
func (c *Container) create(ctx context.Context) (err error) {
	// In case the container creation fails, the following takes care
	// of rolling back all the actions previously performed.
	defer func() {
		if err != nil {
			c.Logger().WithError(err).Error("container create failed")
			c.rollbackFailingContainerCreation(ctx)
		}
	}()

	if c.checkBlockDeviceSupport(ctx) && !IsNydusRootFSType(c.rootFs.Type) {
		// If the rootfs is backed by a block device, go ahead and hotplug it to the guest
		if err = c.hotplugDrive(ctx); err != nil {
			return
		}
	}

	c.Logger().WithFields(logrus.Fields{
		"devices": c.devices,
	}).Info("Attach devices")
	if err = c.attachDevices(ctx); err != nil {
		return
	}

	c.annotateContainerWithVFIOMetadata(c.devices)

	// Deduce additional system mount info that should be handled by the agent
	// inside the VM
	c.getSystemMountInfo()

	process, err := c.sandbox.agent.createContainer(ctx, c.sandbox, c)
	if err != nil {
		return err
	}
	c.process = *process

	if err = c.setContainerState(types.StateReady); err != nil {
		return
	}

	return nil
}

func (c *Container) delete(ctx context.Context) error {
	if c.state.State != types.StateReady &&
		c.state.State != types.StateStopped {
		return fmt.Errorf("Container not ready or stopped, impossible to delete")
	}

	// Remove the container from sandbox structure
	if err := c.sandbox.removeContainer(c.id); err != nil {
		return err
	}

	return c.sandbox.storeSandbox(ctx)
}

// checkSandboxRunning validates the container state.
//
// cmd specifies the operation (or verb) that the retrieval is destined
// for and is only used to make the returned error as descriptive as
// possible.
func (c *Container) checkSandboxRunning(cmd string) error {
	if cmd == "" {
		return fmt.Errorf("Cmd cannot be empty")
	}

	if c.sandbox.state.State != types.StateRunning {
		return fmt.Errorf("Sandbox not running, impossible to %s the container", cmd)
	}

	return nil
}

func (c *Container) getSystemMountInfo() {
	// Check if /dev needs to be bind mounted from host /dev
	c.systemMountsInfo.BindMountDev = false

	for _, m := range c.mounts {
		if m.Source == "/dev" && m.Destination == "/dev" && m.Type == "bind" {
			c.systemMountsInfo.BindMountDev = true
		}
	}

	// TODO Deduce /dev/shm size. See https://github.com/clearcontainers/runtime/issues/138
}

func (c *Container) start(ctx context.Context) error {
	if err := c.checkSandboxRunning("start"); err != nil {
		return err
	}

	if c.state.State != types.StateReady &&
		c.state.State != types.StateStopped {
		return fmt.Errorf("Container not ready or stopped, impossible to start")
	}

	if err := c.state.ValidTransition(c.state.State, types.StateRunning); err != nil {
		return err
	}

	if err := c.sandbox.agent.startContainer(ctx, c.sandbox, c); err != nil {
		c.Logger().WithError(err).Error("Failed to start container")

		if err := c.stop(ctx, true); err != nil {
			c.Logger().WithError(err).Warn("Failed to stop container")
		}
		return err
	}

	return c.setContainerState(types.StateRunning)
}

func (c *Container) stop(ctx context.Context, force bool) error {
	span, ctx := katatrace.Trace(ctx, c.Logger(), "stop", containerTracingTags, map[string]string{"container_id": c.id})
	defer span.End()

	// In case the container status has been updated implicitly because
	// the container process has terminated, it might be possible that
	// someone try to stop the container, and we don't want to issue an
	// error in that case. This should be a no-op.
	//
	// This has to be handled before the transition validation since this
	// is an exception.
	if c.state.State == types.StateStopped {
		c.Logger().Info("Container already stopped")
		return nil
	}

	if err := c.state.ValidTransition(c.state.State, types.StateStopped); err != nil {
		return err
	}

	// Force the container to be killed. For most of the cases, this
	// should not matter and it should return an error that will be
	// ignored.
	c.kill(ctx, syscall.SIGKILL, true)

	// Since the agent has supported the MultiWaitProcess, it's better to
	// wait the process here to make sure the process has exited before to
	// issue stopContainer, otherwise the RemoveContainerRequest in it will
	// get failed if the process hasn't exited.
	c.sandbox.agent.waitProcess(ctx, c, c.id)

	defer func() {
		// Save device and drive data.
		// TODO: can we merge this saving with setContainerState()?
		if err := c.sandbox.Save(); err != nil {
			c.Logger().WithError(err).Info("Save container state failed")
		}
	}()

	if err := c.sandbox.agent.stopContainer(ctx, c.sandbox, *c); err != nil && !force {
		return err
	}

	if err := c.unmountHostMounts(ctx); err != nil && !force {
		return err
	}

	if IsNydusRootFSType(c.rootFs.Type) {
		if err := nydusContainerCleanup(ctx, getMountPath(c.sandbox.id), c); err != nil && !force {
			return err
		}
	} else {
		if err := c.sandbox.fsShare.UnshareRootFilesystem(ctx, c); err != nil && !force {
			return err
		}
	}

	if err := c.sandbox.agent.removeStaleVirtiofsShareMounts(ctx); err != nil && !force {
		return err
	}

	if err := c.detachDevices(ctx); err != nil && !force {
		return err
	}

	if err := c.removeDrive(ctx); err != nil && !force {
		return err
	}

	// container was killed by force, container MUST change its state
	// as soon as possible just in case one of below operations fail leaving
	// the containers in a bad state.
	if err := c.setContainerState(types.StateStopped); err != nil {
		return err
	}

	return nil
}

func (c *Container) enter(ctx context.Context, cmd types.Cmd) (*Process, error) {
	if err := c.checkSandboxRunning("enter"); err != nil {
		return nil, err
	}

	if c.state.State != types.StateReady &&
		c.state.State != types.StateRunning {
		return nil, fmt.Errorf("Container not ready or running, " +
			"impossible to enter")
	}

	process, err := c.sandbox.agent.exec(ctx, c.sandbox, *c, cmd)
	if err != nil {
		return nil, err
	}

	return process, nil
}

func (c *Container) wait(ctx context.Context, processID string) (int32, error) {
	if c.state.State != types.StateReady &&
		c.state.State != types.StateRunning {
		return 0, fmt.Errorf("Container not ready or running, " +
			"impossible to wait")
	}

	return c.sandbox.agent.waitProcess(ctx, c, processID)
}

func (c *Container) kill(ctx context.Context, signal syscall.Signal, all bool) error {
	return c.signalProcess(ctx, c.process.Token, signal, all)
}

func (c *Container) signalProcess(ctx context.Context, processID string, signal syscall.Signal, all bool) error {
	if c.sandbox.state.State != types.StateReady && c.sandbox.state.State != types.StateRunning {
		return fmt.Errorf("Sandbox not ready or running, impossible to signal the container")
	}

	if c.state.State != types.StateReady && c.state.State != types.StateRunning && c.state.State != types.StatePaused {
		return fmt.Errorf("Container not ready, running or paused, impossible to signal the container")
	}

	// kill(2) method can return ESRCH in certain cases, which is not handled by containerd cri server in container_stop.go.
	// CRIO server also doesn't handle ESRCH. So kata runtime will swallow it here.
	var err error
	if err = c.sandbox.agent.signalProcess(ctx, c, processID, signal, all); err != nil &&
		strings.Contains(err.Error(), "ESRCH: No such process") {
		c.Logger().WithFields(logrus.Fields{
			"container":  c.id,
			"process-id": processID,
		}).Warn("signal encounters ESRCH, process already finished")
		return nil
	}
	return err
}

func (c *Container) winsizeProcess(ctx context.Context, processID string, height, width uint32) error {
	if c.state.State != types.StateReady && c.state.State != types.StateRunning {
		return fmt.Errorf("Container not ready or running, impossible to signal the container")
	}

	return c.sandbox.agent.winsizeProcess(ctx, c, processID, height, width)
}

func (c *Container) ioStream(processID string) (io.WriteCloser, io.Reader, io.Reader, error) {
	if c.state.State != types.StateReady && c.state.State != types.StateRunning {
		return nil, nil, nil, fmt.Errorf("Container not ready or running, impossible to signal the container")
	}

	stream := newIOStream(c.sandbox, c, processID)

	return stream.stdin(), stream.stdout(), stream.stderr(), nil
}

func (c *Container) stats(ctx context.Context) (*ContainerStats, error) {
	if err := c.checkSandboxRunning("stats"); err != nil {
		return nil, err
	}
	return c.sandbox.agent.statsContainer(ctx, c.sandbox, *c)
}

func (c *Container) update(ctx context.Context, resources specs.LinuxResources) error {
	if err := c.checkSandboxRunning("update"); err != nil {
		return err
	}

	if state := c.state.State; !(state == types.StateRunning || state == types.StateReady) {
		return fmt.Errorf("Container(%s) not running or ready, impossible to update", state)
	}

	if c.config.Resources.CPU == nil {
		c.config.Resources.CPU = &specs.LinuxCPU{}
	}

	if cpu := resources.CPU; cpu != nil {
		if p := cpu.Period; p != nil && *p != 0 {
			c.config.Resources.CPU.Period = p
		}
		if q := cpu.Quota; q != nil && *q != 0 {
			c.config.Resources.CPU.Quota = q
		}
		if cpu.Cpus != "" {
			c.config.Resources.CPU.Cpus = cpu.Cpus
		}
		if cpu.Mems != "" {
			c.config.Resources.CPU.Mems = cpu.Mems
		}
	}

	if c.config.Resources.Memory == nil {
		c.config.Resources.Memory = &specs.LinuxMemory{}
	}

	if mem := resources.Memory; mem != nil && mem.Limit != nil {
		c.config.Resources.Memory.Limit = mem.Limit
	}

	if err := c.sandbox.updateResources(ctx); err != nil {
		return err
	}

	// There currently isn't a notion of cpusets.cpus or mems being tracked
	// inside of the guest. Make sure we clear these before asking agent to update
	// the container's cgroups.
	if resources.CPU != nil {
		resources.CPU.Mems = ""
		resources.CPU.Cpus = ""
	}

	return c.sandbox.agent.updateContainer(ctx, c.sandbox, *c, resources)
}

func (c *Container) pause(ctx context.Context) error {
	if err := c.checkSandboxRunning("pause"); err != nil {
		return err
	}

	if c.state.State != types.StateRunning {
		return fmt.Errorf("Container not running, impossible to pause")
	}

	if err := c.sandbox.agent.pauseContainer(ctx, c.sandbox, *c); err != nil {
		return err
	}

	return c.setContainerState(types.StatePaused)
}

func (c *Container) resume(ctx context.Context) error {
	if err := c.checkSandboxRunning("resume"); err != nil {
		return err
	}

	if c.state.State != types.StatePaused {
		return fmt.Errorf("Container not paused, impossible to resume")
	}

	if err := c.sandbox.agent.resumeContainer(ctx, c.sandbox, *c); err != nil {
		return err
	}

	return c.setContainerState(types.StateRunning)
}

// hotplugDrive will attempt to hotplug the container rootfs if it is backed by a
// block device
func (c *Container) hotplugDrive(ctx context.Context) error {
	var dev device
	var err error

	// Check to see if the rootfs is an umounted block device (source) or if the
	// mount (target) is backed by a block device:
	if !c.rootFs.Mounted {
		dev, err = getDeviceForPath(c.rootFs.Source)
		// there is no "rootfs" dir on block device backed rootfs
		c.rootfsSuffix = ""
	} else {
		dev, err = getDeviceForPath(c.rootFs.Target)
	}

	if err == errMountPointNotFound {
		return nil
	}

	if err != nil {
		return err
	}

	c.Logger().WithFields(logrus.Fields{
		"device-major": dev.major,
		"device-minor": dev.minor,
		"mount-point":  dev.mountPoint,
	}).Info("device details")

	isBD, err := checkStorageDriver(dev.major, dev.minor)
	if err != nil {
		return err
	}

	if !isBD {
		return nil
	}

	devicePath := c.rootFs.Source
	fsType := c.rootFs.Type
	if c.rootFs.Mounted {
		if dev.mountPoint == c.rootFs.Target {
			c.rootfsSuffix = ""
		}
		// If device mapper device, then fetch the full path of the device
		devicePath, fsType, _, err = utils.GetDevicePathAndFsTypeOptions(dev.mountPoint)
		if err != nil {
			return err
		}
	}

	devicePath, err = filepath.EvalSymlinks(devicePath)
	if err != nil {
		return err
	}

	c.Logger().WithFields(logrus.Fields{
		"device-path": devicePath,
		"fs-type":     fsType,
	}).Info("Block device detected")

	if err = c.plugDevice(ctx, devicePath); err != nil {
		return err
	}

	return c.setStateFstype(fsType)
}

// plugDevice will attach the rootfs if blockdevice is supported (this is rootfs specific)
func (c *Container) plugDevice(ctx context.Context, devicePath string) error {
	var stat unix.Stat_t
	if err := unix.Stat(devicePath, &stat); err != nil {
		return fmt.Errorf("stat %q failed: %v", devicePath, err)
	}

	if c.checkBlockDeviceSupport(ctx) && stat.Mode&unix.S_IFBLK == unix.S_IFBLK {
		b, err := c.sandbox.devManager.NewDevice(config.DeviceInfo{
			HostPath:      devicePath,
			ContainerPath: filepath.Join(kataGuestSharedDir(), c.id),
			DevType:       "b",
			Major:         int64(unix.Major(uint64(stat.Rdev))),
			Minor:         int64(unix.Minor(uint64(stat.Rdev))),
		})
		if err != nil {
			return fmt.Errorf("device manager failed to create rootfs device for %q: %v", devicePath, err)
		}

		c.state.BlockDeviceID = b.DeviceID()

		// attach rootfs device
		if err := c.sandbox.devManager.AttachDevice(ctx, b.DeviceID(), c.sandbox); err != nil {
			return err
		}
	}
	return nil
}

// isDriveUsed checks if a drive has been used for container rootfs
func (c *Container) isDriveUsed() bool {
	return !(c.state.Fstype == "")
}

func (c *Container) removeDrive(ctx context.Context) (err error) {
	if c.isDriveUsed() {
		c.Logger().Info("unplugging block device")

		devID := c.state.BlockDeviceID
		err := c.sandbox.devManager.DetachDevice(ctx, devID, c.sandbox)
		if err != nil && err != manager.ErrDeviceNotAttached {
			return err
		}

		if err = c.sandbox.devManager.RemoveDevice(devID); err != nil {
			c.Logger().WithFields(logrus.Fields{
				"container": c.id,
				"device-id": devID,
			}).WithError(err).Error("remove device failed")

			// ignore the device not exist error
			if err != manager.ErrDeviceNotExist {
				return err
			}
		}
	}

	return nil
}

func (c *Container) attachDevices(ctx context.Context) error {
	// there's no need to do rollback when error happens,
	// because if attachDevices fails, container creation will fail too,
	// and rollbackFailingContainerCreation could do all the rollbacks

	// since devices with large bar space require delayed attachment,
	// the devices need to be split into two lists, normalAttachedDevs and delayAttachedDevs.
	// so c.device is not used here. See issue https://github.com/kata-containers/runtime/issues/2460.
	for _, dev := range c.devices {
		if err := c.sandbox.devManager.AttachDevice(ctx, dev.ID, c.sandbox); err != nil {
			return err
		}
	}
	return nil
}

func (c *Container) detachDevices(ctx context.Context) error {
	for _, dev := range c.devices {
		err := c.sandbox.devManager.DetachDevice(ctx, dev.ID, c.sandbox)
		if err != nil && err != manager.ErrDeviceNotAttached {
			return err
		}

		if err = c.sandbox.devManager.RemoveDevice(dev.ID); err != nil {
			c.Logger().WithFields(logrus.Fields{
				"container": c.id,
				"device-id": dev.ID,
			}).WithError(err).Error("remove device failed")

			// ignore the device not exist error
			if err != manager.ErrDeviceNotExist {
				return err
			}
		}
	}
	return nil
}
