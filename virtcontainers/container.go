// +build linux
// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2014,2015,2016,2017 Docker, Inc.
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"syscall"
	"time"

	"github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

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
	// Total CPU time consumed.
	// Units: nanoseconds.
	TotalUsage uint64 `json:"total_usage,omitempty"`
	// Total CPU time consumed per core.
	// Units: nanoseconds.
	PercpuUsage []uint64 `json:"percpu_usage,omitempty"`
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
	// memory used for cache
	Cache uint64 `json:"cache,omitempty"`
	// usage of memory
	Usage MemoryData `json:"usage,omitempty"`
	// usage of memory  swap
	SwapUsage MemoryData `json:"swap_usage,omitempty"`
	// usage of kernel memory
	KernelUsage MemoryData `json:"kernel_usage,omitempty"`
	// usage of kernel TCP memory
	KernelTCPUsage MemoryData `json:"kernel_tcp_usage,omitempty"`
	// if true, memory usage is accounted for throughout a hierarchy of cgroups.
	UseHierarchy bool `json:"use_hierarchy"`

	Stats map[string]uint64 `json:"stats,omitempty"`
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
	Major uint64 `json:"major,omitempty"`
	Minor uint64 `json:"minor,omitempty"`
	Op    string `json:"op,omitempty"`
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
	CPUStats    CPUStats    `json:"cpu_stats,omitempty"`
	MemoryStats MemoryStats `json:"memory_stats,omitempty"`
	PidsStats   PidsStats   `json:"pids_stats,omitempty"`
	BlkioStats  BlkioStats  `json:"blkio_stats,omitempty"`
	// the map is in the format "size of hugepage: stats of the hugepage"
	HugetlbStats map[string]HugetlbStats `json:"hugetlb_stats,omitempty"`
}

// ContainerStats describes a container stats.
type ContainerStats struct {
	CgroupStats *CgroupStats
}

// ContainerResources describes container resources
type ContainerResources struct {
	// VCPUs are the number of vCPUs that are being used by the container
	VCPUs uint32

	// Mem is the memory that is being used by the container
	Mem uint32
}

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
	DeviceInfos []config.DeviceInfo

	// Resources container resources
	Resources ContainerResources
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

// Container is composed of a set of containers and a runtime environment.
// A Container can be created, deleted, started, stopped, listed, entered, paused and restored.
type Container struct {
	id        string
	sandboxID string

	rootFs string

	config *ContainerConfig

	sandbox *Sandbox

	runPath       string
	configPath    string
	containerPath string

	state State

	process Process

	mounts []Mount

	devices []api.Device

	systemMountsInfo SystemMountsInfo
}

// ID returns the container identifier string.
func (c *Container) ID() string {
	return c.id
}

// Logger returns a logrus logger appropriate for logging Container messages
func (c *Container) Logger() *logrus.Entry {
	return virtLog.WithFields(logrus.Fields{
		"subsystem":    "container",
		"container-id": c.id,
		"sandbox-id":   c.sandboxID,
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

// SetPid sets and stores the given pid as the pid of container's process.
func (c *Container) SetPid(pid int) error {
	c.process.Pid = pid

	return c.storeProcess()
}

func (c *Container) setStateBlockIndex(index int) error {
	c.state.BlockIndex = index

	err := c.sandbox.storage.storeContainerResource(c.sandbox.id, c.id, stateFileType, c.state)
	if err != nil {
		return err
	}

	return nil
}

func (c *Container) setStateFstype(fstype string) error {
	c.state.Fstype = fstype

	err := c.sandbox.storage.storeContainerResource(c.sandbox.id, c.id, stateFileType, c.state)
	if err != nil {
		return err
	}

	return nil
}

func (c *Container) setStateHotpluggedDrive(hotplugged bool) error {
	c.state.HotpluggedDrive = hotplugged

	err := c.sandbox.storage.storeContainerResource(c.sandbox.id, c.id, stateFileType, c.state)
	if err != nil {
		return err
	}

	return nil
}

func (c *Container) setContainerRootfsPCIAddr(addr string) error {
	c.state.RootfsPCIAddr = addr

	err := c.sandbox.storage.storeContainerResource(c.sandbox.id, c.id, stateFileType, c.state)
	if err != nil {
		return err
	}

	return nil
}

// GetAnnotations returns container's annotations
func (c *Container) GetAnnotations() map[string]string {
	return c.config.Annotations
}

func (c *Container) storeProcess() error {
	return c.sandbox.storage.storeContainerProcess(c.sandboxID, c.id, c.process)
}

func (c *Container) storeMounts() error {
	return c.sandbox.storage.storeContainerMounts(c.sandboxID, c.id, c.mounts)
}

func (c *Container) fetchMounts() ([]Mount, error) {
	return c.sandbox.storage.fetchContainerMounts(c.sandboxID, c.id)
}

func (c *Container) storeDevices() error {
	return c.sandbox.storage.storeContainerDevices(c.sandboxID, c.id, c.devices)
}

func (c *Container) fetchDevices() ([]api.Device, error) {
	return c.sandbox.storage.fetchContainerDevices(c.sandboxID, c.id)
}

// storeContainer stores a container config.
func (c *Container) storeContainer() error {
	fs := filesystem{}
	err := fs.storeContainerResource(c.sandbox.id, c.id, configFileType, *(c.config))
	if err != nil {
		return err
	}

	return nil
}

// setContainerState sets both the in-memory and on-disk state of the
// container.
func (c *Container) setContainerState(state stateString) error {
	if state == "" {
		return errNeedState
	}

	// update in-memory state
	c.state.State = state

	// update on-disk state
	err := c.sandbox.storage.storeContainerResource(c.sandbox.id, c.id, stateFileType, c.state)
	if err != nil {
		return err
	}

	return nil
}

func (c *Container) createContainersDirs() error {
	err := os.MkdirAll(c.runPath, dirMode)
	if err != nil {
		return err
	}

	err = os.MkdirAll(c.configPath, dirMode)
	if err != nil {
		c.sandbox.storage.deleteContainerResources(c.sandboxID, c.id, nil)
		return err
	}

	return nil
}

// mountSharedDirMounts handles bind-mounts by bindmounting to the host shared
// directory which is mounted through 9pfs in the VM.
// It also updates the container mount list with the HostPath info, and store
// container mounts to the storage. This way, we will have the HostPath info
// available when we will need to unmount those mounts.
func (c *Container) mountSharedDirMounts(hostSharedDir, guestSharedDir string) ([]Mount, error) {
	var sharedDirMounts []Mount
	for idx, m := range c.mounts {
		if isSystemMount(m.Destination) || m.Type != "bind" {
			continue
		}

		// We need to treat /dev/shm as a special case. This is passed as a bind mount in the spec,
		// but it does not make sense to pass this as a 9p mount from the host side.
		// This needs to be handled purely in the guest, by allocating memory for this inside the VM.
		if m.Destination == "/dev/shm" {
			continue
		}

		var stat unix.Stat_t
		if err := unix.Stat(m.Source, &stat); err != nil {
			return nil, err
		}

		// Check if mount is a block device file. If it is, the block device will be attached to the host
		// instead of passing this as a shared mount.
		if c.checkBlockDeviceSupport() && stat.Mode&unix.S_IFBLK == unix.S_IFBLK {
			// TODO: remove dependency of package drivers
			b := &drivers.BlockDevice{
				DevType: config.DeviceBlock,
				DeviceInfo: config.DeviceInfo{
					HostPath:      m.Source,
					ContainerPath: m.Destination,
					DevType:       "b",
				},
			}

			// Attach this block device, all other devices passed in the config have been attached at this point
			if err := b.Attach(c.sandbox); err != nil {
				return nil, err
			}

			c.mounts[idx].BlockDevice = b
			continue
		}

		// Ignore /dev, directories and all other device files. We handle
		// only regular files in /dev. It does not make sense to pass the host
		// device nodes to the guest.
		if isHostDevice(m.Destination) {
			continue
		}

		randBytes, err := utils.GenerateRandomBytes(8)
		if err != nil {
			return nil, err
		}

		// These mounts are created in the shared dir
		filename := fmt.Sprintf("%s-%s-%s", c.id, hex.EncodeToString(randBytes), filepath.Base(m.Destination))
		mountDest := filepath.Join(hostSharedDir, c.sandbox.id, filename)

		if err := bindMount(m.Source, mountDest, false); err != nil {
			return nil, err
		}

		// Save HostPath mount value into the mount list of the container.
		c.mounts[idx].HostPath = mountDest

		// Check if mount is readonly, let the agent handle the readonly mount
		// within the VM.
		readonly := false
		for _, flag := range m.Options {
			if flag == "ro" {
				readonly = true
			}
		}

		sharedDirMount := Mount{
			Source:      filepath.Join(guestSharedDir, filename),
			Destination: m.Destination,
			Type:        m.Type,
			Options:     m.Options,
			ReadOnly:    readonly,
		}

		sharedDirMounts = append(sharedDirMounts, sharedDirMount)
	}

	if err := c.storeMounts(); err != nil {
		return nil, err
	}

	return sharedDirMounts, nil
}

func (c *Container) unmountHostMounts() error {
	for _, m := range c.mounts {
		if m.HostPath != "" {
			logger := c.Logger().WithField("host-path", m.HostPath)
			if err := syscall.Unmount(m.HostPath, 0); err != nil {
				// Unable to unmount paths could be a really big problem here
				// we need to make sure cause 'less damage' if things are
				// really broken. For further, we need to give admins more of
				// a chance to diagnose the problem. As the rules of `fail fast`,
				// here we return an error as soon as we get it.
				logger.WithError(err).Warn("Could not umount")
				return err
			} else if err := os.RemoveAll(m.HostPath); err != nil {
				// since the mounts related to the shared dir is umounted
				// we need to remove the host path to avoid resource remaining
				logger.WithError(err).Warn("Could not be removed")
				return err
			}
		}
	}

	return nil
}

// newContainer creates a Container structure from a sandbox and a container configuration.
func newContainer(sandbox *Sandbox, contConfig ContainerConfig) (*Container, error) {
	if contConfig.valid() == false {
		return &Container{}, fmt.Errorf("Invalid container configuration")
	}

	c := &Container{
		id:            contConfig.ID,
		sandboxID:     sandbox.id,
		rootFs:        contConfig.RootFs,
		config:        &contConfig,
		sandbox:       sandbox,
		runPath:       filepath.Join(runStoragePath, sandbox.id, contConfig.ID),
		configPath:    filepath.Join(configStoragePath, sandbox.id, contConfig.ID),
		containerPath: filepath.Join(sandbox.id, contConfig.ID),
		state:         State{},
		process:       Process{},
		mounts:        contConfig.Mounts,
	}

	state, err := c.sandbox.storage.fetchContainerState(c.sandboxID, c.id)
	if err == nil {
		c.state = state
	}

	process, err := c.sandbox.storage.fetchContainerProcess(c.sandboxID, c.id)
	if err == nil {
		c.process = process
	}

	mounts, err := c.fetchMounts()
	if err == nil {
		c.mounts = mounts
	}

	// Devices will be found in storage after create stage has completed.
	// We fetch devices from storage at all other stages.
	storedDevices, err := c.fetchDevices()
	if err == nil {
		c.devices = storedDevices
	} else {
		// If devices were not found in storage, create Device implementations
		// from the configuration. This should happen at create.

		devices, err := sandbox.devManager.NewDevices(contConfig.DeviceInfos)
		if err != nil {
			return &Container{}, err
		}
		c.devices = devices
	}
	return c, nil
}

// rollbackFailingContainerCreation rolls back important steps that might have
// been performed before the container creation failed.
// - Unplug CPU and memory resources from the VM.
// - Unplug devices from the VM.
func (c *Container) rollbackFailingContainerCreation() {
	if err := c.removeResources(); err != nil {
		c.Logger().WithError(err).Error("rollback failed removeResources()")
	}
	if err := c.detachDevices(); err != nil {
		c.Logger().WithError(err).Error("rollback failed detachDevices()")
	}
	if err := c.removeDrive(); err != nil {
		c.Logger().WithError(err).Error("rollback failed removeDrive()")
	}
}

func (c *Container) checkBlockDeviceSupport() bool {
	if !c.sandbox.config.HypervisorConfig.DisableBlockDeviceUse {
		agentCaps := c.sandbox.agent.capabilities()
		hypervisorCaps := c.sandbox.hypervisor.capabilities()

		if agentCaps.isBlockDeviceSupported() && hypervisorCaps.isBlockDeviceHotplugSupported() {
			return true
		}
	}

	return false
}

// createContainer creates and start a container inside a Sandbox. It has to be
// called only when a new container, not known by the sandbox, has to be created.
func createContainer(sandbox *Sandbox, contConfig ContainerConfig) (c *Container, err error) {
	if sandbox == nil {
		return nil, errNeedSandbox
	}

	c, err = newContainer(sandbox, contConfig)
	if err != nil {
		return
	}

	if err = c.createContainersDirs(); err != nil {
		return
	}

	// In case the container creation fails, the following takes care
	// of rolling back all the actions previously performed.
	defer func() {
		if err != nil {
			c.rollbackFailingContainerCreation()
		}
	}()

	if c.checkBlockDeviceSupport() {
		if err = c.hotplugDrive(); err != nil {
			return
		}
	}

	// Attach devices
	if err = c.attachDevices(); err != nil {
		return
	}

	if err = c.addResources(); err != nil {
		return
	}

	// Deduce additional system mount info that should be handled by the agent
	// inside the VM
	c.getSystemMountInfo()

	if err = c.storeDevices(); err != nil {
		return
	}

	process, err := sandbox.agent.createContainer(c.sandbox, c)
	if err != nil {
		return c, err
	}
	c.process = *process

	// If this is a sandbox container, store the pid for sandbox
	ann := c.GetAnnotations()
	if ann[annotations.ContainerTypeKey] == string(PodSandbox) {
		sandbox.setSandboxPid(c.process.Pid)
	}

	// Store the container process returned by the agent.
	if err = c.storeProcess(); err != nil {
		return
	}

	if err = c.setContainerState(StateReady); err != nil {
		return
	}

	return c, nil
}

func (c *Container) delete() error {
	if c.state.State != StateReady &&
		c.state.State != StateStopped {
		return fmt.Errorf("Container not ready or stopped, impossible to delete")
	}

	// Remove the container from sandbox structure
	if err := c.sandbox.removeContainer(c.id); err != nil {
		return err
	}

	return c.sandbox.storage.deleteContainerResources(c.sandboxID, c.id, nil)
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

	if c.sandbox.state.State != StateRunning {
		return fmt.Errorf("Sandbox not running, impossible to %s the container", cmd)
	}

	return nil
}

func (c *Container) getSystemMountInfo() {
	// check if /dev needs to be bind mounted from host /dev
	c.systemMountsInfo.BindMountDev = false

	for _, m := range c.mounts {
		if m.Source == "/dev" && m.Destination == "/dev" && m.Type == "bind" {
			c.systemMountsInfo.BindMountDev = true
		}
	}

	// TODO Deduce /dev/shm size. See https://github.com/clearcontainers/runtime/issues/138
}

func (c *Container) start() error {
	if err := c.checkSandboxRunning("start"); err != nil {
		return err
	}

	if c.state.State != StateReady &&
		c.state.State != StateStopped {
		return fmt.Errorf("Container not ready or stopped, impossible to start")
	}

	if err := c.state.validTransition(c.state.State, StateRunning); err != nil {
		return err
	}

	if err := c.sandbox.agent.startContainer(c.sandbox, c); err != nil {
		c.Logger().WithError(err).Error("Failed to start container")

		if err := c.stop(); err != nil {
			c.Logger().WithError(err).Warn("Failed to stop container")
		}
		return err
	}

	return c.setContainerState(StateRunning)
}

func (c *Container) stop() error {
	// In case the container status has been updated implicitly because
	// the container process has terminated, it might be possible that
	// someone try to stop the container, and we don't want to issue an
	// error in that case. This should be a no-op.
	//
	// This has to be handled before the transition validation since this
	// is an exception.
	if c.state.State == StateStopped {
		c.Logger().Info("Container already stopped")
		return nil
	}

	if c.sandbox.state.State != StateReady && c.sandbox.state.State != StateRunning {
		return fmt.Errorf("Sandbox not ready or running, impossible to stop the container")
	}

	if err := c.state.validTransition(c.state.State, StateStopped); err != nil {
		return err
	}

	defer func() {
		// If shim is still running something went wrong
		// Make sure we stop the shim process
		if running, _ := isShimRunning(c.process.Pid); running {
			l := c.Logger()
			l.Error("Failed to stop container so stopping dangling shim")
			if err := stopShim(c.process.Pid); err != nil {
				l.WithError(err).Warn("failed to stop shim")
			}
		}

	}()

	// Here we expect that stop() has been called because the container
	// process returned or because it received a signal. In case of a
	// signal, we want to give it some time to end the container process.
	// However, if the signal didn't reach its goal, the caller still
	// expects this container to be stopped, that's why we should not
	// return an error, but instead try to kill it forcefully.
	if err := waitForShim(c.process.Pid); err != nil {
		// Force the container to be killed.
		if err := c.kill(syscall.SIGKILL, true); err != nil {
			return err
		}

		// Wait for the end of container process. We expect this call
		// to succeed. Indeed, we have already given a second chance
		// to the container by trying to kill it with SIGKILL, there
		// is no reason to try to go further if we got an error.
		if err := waitForShim(c.process.Pid); err != nil {
			return err
		}
	}

	// Force the container to be killed. For most of the cases, this
	// should not matter and it should return an error that will be
	// ignored.
	// But for the specific case where the shim has been SIGKILL'ed,
	// the container is still running inside the VM. And this is why
	// this signal will ensure the container will get killed to match
	// the state of the shim. This will allow the following call to
	// stopContainer() to succeed in such particular case.
	c.kill(syscall.SIGKILL, true)

	if err := c.sandbox.agent.stopContainer(c.sandbox, *c); err != nil {
		return err
	}

	if err := c.removeResources(); err != nil {
		return err
	}

	if err := c.detachDevices(); err != nil {
		return err
	}

	if err := c.removeDrive(); err != nil {
		return err
	}

	return c.setContainerState(StateStopped)
}

func (c *Container) enter(cmd Cmd) (*Process, error) {
	if err := c.checkSandboxRunning("enter"); err != nil {
		return nil, err
	}

	if c.state.State != StateReady &&
		c.state.State != StateRunning {
		return nil, fmt.Errorf("Container not ready or running, " +
			"impossible to enter")
	}

	process, err := c.sandbox.agent.exec(c.sandbox, *c, cmd)
	if err != nil {
		return nil, err
	}

	return process, nil
}

func (c *Container) wait(processID string) (int32, error) {
	if c.state.State != StateReady &&
		c.state.State != StateRunning {
		return 0, fmt.Errorf("Container not ready or running, " +
			"impossible to wait")
	}

	return c.sandbox.agent.waitProcess(c, processID)
}

func (c *Container) kill(signal syscall.Signal, all bool) error {
	return c.signalProcess(c.process.Token, signal, all)
}

func (c *Container) signalProcess(processID string, signal syscall.Signal, all bool) error {
	if c.sandbox.state.State != StateReady && c.sandbox.state.State != StateRunning {
		return fmt.Errorf("Sandbox not ready or running, impossible to signal the container")
	}

	if c.state.State != StateReady && c.state.State != StateRunning && c.state.State != StatePaused {
		return fmt.Errorf("Container not ready, running or paused, impossible to signal the container")
	}

	return c.sandbox.agent.signalProcess(c, processID, signal, all)
}

func (c *Container) winsizeProcess(processID string, height, width uint32) error {
	if c.state.State != StateReady && c.state.State != StateRunning {
		return fmt.Errorf("Container not ready or running, impossible to signal the container")
	}

	return c.sandbox.agent.winsizeProcess(c, processID, height, width)
}

func (c *Container) ioStream(processID string) (io.WriteCloser, io.Reader, io.Reader, error) {
	if c.state.State != StateReady && c.state.State != StateRunning {
		return nil, nil, nil, fmt.Errorf("Container not ready or running, impossible to signal the container")
	}

	stream := newIOStream(c.sandbox, c, processID)

	return stream.stdin(), stream.stdout(), stream.stderr(), nil
}

func (c *Container) processList(options ProcessListOptions) (ProcessList, error) {
	if err := c.checkSandboxRunning("ps"); err != nil {
		return nil, err
	}

	if c.state.State != StateRunning {
		return nil, fmt.Errorf("Container not running, impossible to list processes")
	}

	return c.sandbox.agent.processListContainer(c.sandbox, *c, options)
}

func (c *Container) stats() (*ContainerStats, error) {
	if err := c.checkSandboxRunning("stats"); err != nil {
		return nil, err
	}
	return c.sandbox.agent.statsContainer(c.sandbox, *c)
}

func (c *Container) update(resources specs.LinuxResources) error {
	if err := c.checkSandboxRunning("update"); err != nil {
		return err
	}

	if c.state.State != StateRunning {
		return fmt.Errorf("Container not running, impossible to update")
	}

	// fetch current configuration
	currentConfig, err := c.sandbox.storage.fetchContainerConfig(c.sandbox.id, c.id)
	if err != nil {
		return err
	}

	newResources := ContainerResources{
		VCPUs: uint32(utils.ConstraintsToVCPUs(*resources.CPU.Quota, *resources.CPU.Period)),
	}

	if err := c.updateResources(currentConfig.Resources, newResources); err != nil {
		return err
	}

	return c.sandbox.agent.updateContainer(c.sandbox, *c, resources)
}

func (c *Container) pause() error {
	if err := c.checkSandboxRunning("pause"); err != nil {
		return err
	}

	if c.state.State != StateRunning && c.state.State != StateReady {
		return fmt.Errorf("Container not running or ready, impossible to pause")
	}

	if err := c.sandbox.agent.pauseContainer(c.sandbox, *c); err != nil {
		return err
	}

	return c.setContainerState(StatePaused)
}

func (c *Container) resume() error {
	if err := c.checkSandboxRunning("resume"); err != nil {
		return err
	}

	if c.state.State != StatePaused {
		return fmt.Errorf("Container not paused, impossible to resume")
	}

	if err := c.sandbox.agent.resumeContainer(c.sandbox, *c); err != nil {
		return err
	}

	return c.setContainerState(StateRunning)
}

func (c *Container) hotplugDrive() error {
	dev, err := getDeviceForPath(c.rootFs)

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

	isDM, err := checkStorageDriver(dev.major, dev.minor)
	if err != nil {
		return err
	}

	if !isDM {
		return nil
	}

	// If device mapper device, then fetch the full path of the device
	devicePath, fsType, err := getDevicePathAndFsType(dev.mountPoint)
	if err != nil {
		return err
	}

	c.Logger().WithFields(logrus.Fields{
		"device-path": devicePath,
		"fs-type":     fsType,
	}).Info("Block device detected")

	driveIndex, err := c.sandbox.getAndSetSandboxBlockIndex()
	if err != nil {
		return err
	}

	// Add drive with id as container id
	devID := utils.MakeNameID("drive", c.id, maxDevIDSize)
	drive := drivers.Drive{
		File:   devicePath,
		Format: "raw",
		ID:     devID,
		Index:  driveIndex,
	}

	if _, err := c.sandbox.hypervisor.hotplugAddDevice(&drive, blockDev); err != nil {
		return err
	}

	if drive.PCIAddr != "" {
		c.setContainerRootfsPCIAddr(drive.PCIAddr)
	}

	c.setStateHotpluggedDrive(true)

	if err := c.setStateBlockIndex(driveIndex); err != nil {
		return err
	}

	return c.setStateFstype(fsType)
}

// isDriveUsed checks if a drive has been used for container rootfs
func (c *Container) isDriveUsed() bool {
	if c.state.Fstype == "" {
		return false
	}
	return true
}

func (c *Container) removeDrive() (err error) {
	if c.isDriveUsed() && c.state.HotpluggedDrive {
		c.Logger().Info("unplugging block device")

		devID := utils.MakeNameID("drive", c.id, maxDevIDSize)
		drive := &drivers.Drive{
			ID: devID,
		}

		l := c.Logger().WithField("device-id", devID)
		l.Info("Unplugging block device")

		if _, err := c.sandbox.hypervisor.hotplugRemoveDevice(drive, blockDev); err != nil {
			l.WithError(err).Info("Failed to unplug block device")
			return err
		}
	}

	return nil
}

func (c *Container) attachDevices() error {
	for _, device := range c.devices {
		if err := device.Attach(c.sandbox); err != nil {
			return err
		}
	}

	return nil
}

func (c *Container) detachDevices() error {
	for _, device := range c.devices {
		if err := device.Detach(c.sandbox); err != nil {
			return err
		}
	}

	return nil
}

func (c *Container) addResources() error {
	//TODO add support for memory, Issue: https://github.com/containers/virtcontainers/issues/578
	if c.config == nil {
		return nil
	}

	// Container is being created, try to add the number of vCPUs specified
	vCPUs := c.config.Resources.VCPUs
	if vCPUs != 0 {
		virtLog.Debugf("hot adding %d vCPUs", vCPUs)
		data, err := c.sandbox.hypervisor.hotplugAddDevice(vCPUs, cpuDev)
		if err != nil {
			return err
		}

		vcpusAdded, ok := data.(uint32)
		if !ok {
			return fmt.Errorf("Could not get the number of vCPUs added, got %+v", data)
		}

		// A different number of vCPUs was added, we have to update
		// the resources in order to don't remove vCPUs used by other containers.
		if vcpusAdded != vCPUs {
			// Set and save container's config
			c.config.Resources.VCPUs = vcpusAdded
			if err := c.storeContainer(); err != nil {
				return err
			}
		}

		return c.sandbox.agent.onlineCPUMem(vcpusAdded)
	}

	return nil
}

func (c *Container) removeResources() error {
	//TODO add support for memory, Issue: https://github.com/containers/virtcontainers/issues/578
	if c.config == nil {
		return nil
	}

	// In order to don't remove vCPUs used by other containers, we have to remove
	// only the vCPUs assigned to the container
	config, err := c.sandbox.storage.fetchContainerConfig(c.sandbox.id, c.id)
	if err != nil {
		// don't fail, let's use the default configuration
		config = *c.config
	}

	vCPUs := config.Resources.VCPUs
	if vCPUs != 0 {
		virtLog.Debugf("hot removing %d vCPUs", vCPUs)
		if _, err := c.sandbox.hypervisor.hotplugRemoveDevice(vCPUs, cpuDev); err != nil {
			return err
		}
	}

	return nil
}

func (c *Container) updateResources(oldResources, newResources ContainerResources) error {
	//TODO add support for memory, Issue: https://github.com/containers/virtcontainers/issues/578
	var vCPUs uint32
	oldVCPUs := oldResources.VCPUs
	newVCPUs := newResources.VCPUs

	// Update vCPUs is not possible if period and/or quota are not set or
	// oldVCPUs and newVCPUs are equal.
	// Don't fail, the constraint still can be applied in the cgroup.
	if newVCPUs == 0 || oldVCPUs == newVCPUs {
		c.Logger().WithFields(logrus.Fields{
			"old-vcpus": fmt.Sprintf("%d", oldVCPUs),
			"new-vcpus": fmt.Sprintf("%d", newVCPUs),
		}).Debug("the actual number of vCPUs will not be modified")
		return nil
	}

	if oldVCPUs < newVCPUs {
		// hot add vCPUs
		vCPUs = newVCPUs - oldVCPUs
		virtLog.Debugf("hot adding %d vCPUs", vCPUs)
		data, err := c.sandbox.hypervisor.hotplugAddDevice(vCPUs, cpuDev)
		if err != nil {
			return err
		}
		vcpusAdded, ok := data.(uint32)
		if !ok {
			return fmt.Errorf("Could not get the number of vCPUs added, got %+v", data)
		}
		// recalculate the actual number of vCPUs if a different number of vCPUs was added
		newResources.VCPUs = oldVCPUs + vcpusAdded
		if err := c.sandbox.agent.onlineCPUMem(vcpusAdded); err != nil {
			return err
		}
	} else {
		// hot remove vCPUs
		vCPUs = oldVCPUs - newVCPUs
		virtLog.Debugf("hot removing %d vCPUs", vCPUs)
		data, err := c.sandbox.hypervisor.hotplugRemoveDevice(vCPUs, cpuDev)
		if err != nil {
			return err
		}
		vcpusRemoved, ok := data.(uint32)
		if !ok {
			return fmt.Errorf("Could not get the number of vCPUs removed, got %+v", data)
		}
		// recalculate the actual number of vCPUs if a different number of vCPUs was removed
		newResources.VCPUs = oldVCPUs - vcpusRemoved
	}

	// Set and save container's config
	c.config.Resources = newResources
	return c.storeContainer()
}
