// Copyright (c) 2019 Ericsson Eurolab Deutschland GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"

	"github.com/containerd/console"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	chclient "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/cloud-hypervisor/client"
	"github.com/opencontainers/selinux/go-selinux/label"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/pkg/errors"
	log "github.com/sirupsen/logrus"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

//
// Constants and type definitions related to cloud hypervisor
//

type clhState uint8

const (
	clhNotReady clhState = iota
	clhReady
)

const (
	clhStateCreated = "Created"
	clhStateRunning = "Running"
)

const (
	// Values are mandatory by http API
	// Values based on:
	clhTimeout    = 10
	clhAPITimeout = 1
	// Timeout for hot-plug - hotplug devices can take more time, than usual API calls
	// Use longer time timeout for it.
	clhHotPlugAPITimeout  = 5
	clhStopSandboxTimeout = 3
	clhSocket             = "clh.sock"
	clhAPISocket          = "clh-api.sock"
	virtioFsSocket        = "virtiofsd.sock"
	supportedMajorVersion = 0
	supportedMinorVersion = 5
	defaultClhPath        = "/usr/local/bin/cloud-hypervisor"
	virtioFsCacheAlways   = "always"
)

// Interface that hides the implementation of openAPI client
// If the client changes  its methods, this interface should do it as well,
// The main purpose is to hide the client in an interface to allow mock testing.
// This is an interface that has to match with OpenAPI CLH client
type clhClient interface {
	// Check for the REST API availability
	VmmPingGet(ctx context.Context) (chclient.VmmPingResponse, *http.Response, error)
	// Shut the VMM down
	ShutdownVMM(ctx context.Context) (*http.Response, error)
	// Create the VM
	CreateVM(ctx context.Context, vmConfig chclient.VmConfig) (*http.Response, error)
	// Dump the VM information
	// No lint: golint suggest to rename to VMInfoGet.
	VmInfoGet(ctx context.Context) (chclient.VmInfo, *http.Response, error) //nolint:golint
	// Boot the VM
	BootVM(ctx context.Context) (*http.Response, error)
	// Add/remove CPUs to/from the VM
	VmResizePut(ctx context.Context, vmResize chclient.VmResize) (*http.Response, error)
	// Add VFIO PCI device to the VM
	VmAddDevicePut(ctx context.Context, vmAddDevice chclient.VmAddDevice) (chclient.PciDeviceInfo, *http.Response, error)
	// Add a new disk device to the VM
	VmAddDiskPut(ctx context.Context, diskConfig chclient.DiskConfig) (chclient.PciDeviceInfo, *http.Response, error)
	// Remove a device from the VM
	VmRemoveDevicePut(ctx context.Context, vmRemoveDevice chclient.VmRemoveDevice) (*http.Response, error)
}

type CloudHypervisorVersion struct {
	Major    int
	Minor    int
	Revision int
}

//
// Cloud hypervisor state
//
type CloudHypervisorState struct {
	state        clhState
	PID          int
	VirtiofsdPID int
	apiSocket    string
}

func (s *CloudHypervisorState) reset() {
	s.PID = 0
	s.VirtiofsdPID = 0
	s.state = clhNotReady
}

type cloudHypervisor struct {
	id        string
	state     CloudHypervisorState
	config    HypervisorConfig
	ctx       context.Context
	APIClient clhClient
	version   CloudHypervisorVersion
	vmconfig  chclient.VmConfig
	virtiofsd Virtiofsd
	store     persistapi.PersistDriver
	console   console.Console
}

var clhKernelParams = []Param{

	{"root", "/dev/pmem0p1"},
	{"panic", "1"},         // upon kernel panic wait 1 second before reboot
	{"no_timer_check", ""}, // do not check broken timer IRQ resources
	{"noreplace-smp", ""},  // do not replace SMP instructions
	{"rootflags", "data=ordered,errors=remount-ro ro"}, // mount the root filesystem as readonly
	{"rootfstype", "ext4"},
}

var clhDebugKernelParams = []Param{

	{"console", "ttyS0,115200n8"},     // enable serial console
	{"systemd.log_target", "console"}, // send loggng to the console
}

//###########################################################
//
// hypervisor interface implementation for cloud-hypervisor
//
//###########################################################

func (clh *cloudHypervisor) checkVersion() error {
	if clh.version.Major < supportedMajorVersion || (clh.version.Major == supportedMajorVersion && clh.version.Minor < supportedMinorVersion) {
		errorMessage := fmt.Sprintf("Unsupported version: cloud-hypervisor %d.%d not supported by this driver version (%d.%d)",
			clh.version.Major,
			clh.version.Minor,
			supportedMajorVersion,
			supportedMinorVersion)
		return errors.New(errorMessage)
	}
	return nil
}

// For cloudHypervisor this call only sets the internal structure up.
// The VM will be created and started through startSandbox().
func (clh *cloudHypervisor) createSandbox(ctx context.Context, id string, networkNS NetworkNamespace, hypervisorConfig *HypervisorConfig) error {
	clh.ctx = ctx

	span, _ := clh.trace("createSandbox")
	defer span.Finish()

	err := hypervisorConfig.valid()
	if err != nil {
		return err
	}

	clh.id = id
	clh.config = *hypervisorConfig
	clh.state.state = clhNotReady

	// version check only applicable to 'cloud-hypervisor' executable
	clhPath, perr := clh.clhPath()
	if perr != nil {
		return perr

	}
	if strings.HasSuffix(clhPath, "cloud-hypervisor") {
		err = clh.getAvailableVersion()
		if err != nil {
			return err

		}

		if err := clh.checkVersion(); err != nil {
			return err
		}

	}

	clh.Logger().WithField("function", "createSandbox").Info("creating Sandbox")

	virtiofsdSocketPath, err := clh.virtioFsSocketPath(clh.id)
	if err != nil {
		return nil

	}

	if clh.state.PID > 0 {
		clh.Logger().WithField("function", "createSandbox").Info("Sandbox already exist, loading from state")
		clh.virtiofsd = &virtiofsd{
			PID:        clh.state.VirtiofsdPID,
			sourcePath: filepath.Join(getSharePath(clh.id)),
			debug:      clh.config.Debug,
			socketPath: virtiofsdSocketPath,
		}
		return nil
	}

	// No need to return an error from there since there might be nothing
	// to fetch if this is the first time the hypervisor is created.
	clh.Logger().WithField("function", "createSandbox").WithError(err).Info("Sandbox not found creating ")

	// Set initial memomory size of the virtual machine
	// Convert to int64 openApiClient only support int64
	clh.vmconfig.Memory.Size = int64((utils.MemUnit(clh.config.MemorySize) * utils.MiB).ToBytes())
	// shared memory should be enabled if using vhost-user(kata uses virtiofsd)
	clh.vmconfig.Memory.Shared = true
	hostMemKb, err := getHostMemorySizeKb(procMemInfo)
	if err != nil {
		return nil
	}

	// OpenAPI only supports int64 values
	clh.vmconfig.Memory.HotplugSize = int64((utils.MemUnit(hostMemKb) * utils.KiB).ToBytes())
	// Set initial amount of cpu's for the virtual machine
	clh.vmconfig.Cpus = chclient.CpusConfig{
		// cast to int32, as openAPI has a limitation that it does not support unsigned values
		BootVcpus: int32(clh.config.NumVCPUs),
		MaxVcpus:  int32(clh.config.DefaultMaxVCPUs),
	}

	// Add the kernel path
	kernelPath, err := clh.config.KernelAssetPath()
	if err != nil {
		return err
	}
	clh.vmconfig.Kernel = chclient.KernelConfig{
		Path: kernelPath,
	}

	// First take the default parameters defined by this driver
	params := clhKernelParams

	// Followed by extra debug parameters if debug enabled in configuration file
	if clh.config.Debug {
		params = append(params, clhDebugKernelParams...)
	}

	// Followed by extra debug parameters defined in the configuration file
	params = append(params, clh.config.KernelParams...)

	clh.vmconfig.Cmdline.Args = kernelParamsToString(params)

	// set random device generator to hypervisor
	clh.vmconfig.Rng = chclient.RngConfig{
		Src: clh.config.EntropySource,
	}

	// set the initial root/boot disk of hypervisor
	imagePath, err := clh.config.ImageAssetPath()
	if err != nil {
		return err
	}

	if imagePath == "" {
		return errors.New("image path is empty")
	}

	pmem := chclient.PmemConfig{
		File:          imagePath,
		DiscardWrites: true,
	}
	clh.vmconfig.Pmem = append(clh.vmconfig.Pmem, pmem)

	// set the serial console to the cloud hypervisor
	if clh.config.Debug {
		clh.vmconfig.Serial = chclient.ConsoleConfig{
			Mode: cctTTY,
		}

	} else {
		clh.vmconfig.Serial = chclient.ConsoleConfig{
			Mode: cctNULL,
		}
	}

	clh.vmconfig.Console = chclient.ConsoleConfig{
		Mode: cctOFF,
	}

	clh.vmconfig.Cpus.Topology = chclient.CpuTopology{
		ThreadsPerCore: 1,
		CoresPerDie:    int32(clh.config.DefaultMaxVCPUs),
		DiesPerPackage: 1,
		Packages:       1,
	}
	// Overwrite the default value of HTTP API socket path for cloud hypervisor
	apiSocketPath, err := clh.apiSocketPath(id)
	if err != nil {
		clh.Logger().Info("Invalid api socket path for cloud-hypervisor")
		return nil
	}
	clh.state.apiSocket = apiSocketPath

	clh.virtiofsd = &virtiofsd{
		path:       clh.config.VirtioFSDaemon,
		sourcePath: filepath.Join(getSharePath(clh.id)),
		socketPath: virtiofsdSocketPath,
		extraArgs:  clh.config.VirtioFSExtraArgs,
		debug:      clh.config.Debug,
		cache:      clh.config.VirtioFSCache,
	}

	return nil
}

// startSandbox will start the VMM and boot the virtual machine for the given sandbox.
func (clh *cloudHypervisor) startSandbox(timeout int) error {
	span, _ := clh.trace("startSandbox")
	defer span.Finish()

	ctx, cancel := context.WithTimeout(context.Background(), clhAPITimeout*time.Second)
	defer cancel()

	clh.Logger().WithField("function", "startSandbox").Info("starting Sandbox")

	vmPath := filepath.Join(clh.store.RunVMStoragePath(), clh.id)
	err := os.MkdirAll(vmPath, DirMode)
	if err != nil {
		return err
	}

	if clh.virtiofsd == nil {
		return errors.New("Missing virtiofsd configuration")
	}

	// This needs to be done as late as possible, just before launching
	// virtiofsd are executed by kata-runtime after this call, run with
	// the SELinux label. If these processes require privileged, we do
	// notwant to run them under confinement.
	if err := label.SetProcessLabel(clh.config.SELinuxProcessLabel); err != nil {
		return err
	}
	defer label.SetProcessLabel("")

	if clh.config.SharedFS == config.VirtioFS {
		clh.Logger().WithField("function", "startSandbox").Info("Starting virtiofsd")
		pid, err := clh.virtiofsd.Start(ctx)
		if err != nil {
			return err
		}
		clh.state.VirtiofsdPID = pid
	} else {
		return errors.New("cloud-hypervisor only supports virtio based file sharing")
	}

	pid, err := clh.LaunchClh()
	if err != nil {
		if shutdownErr := clh.virtiofsd.Stop(); shutdownErr != nil {
			clh.Logger().WithField("error", shutdownErr).Warn("error shutting down Virtiofsd")
		}
		return fmt.Errorf("failed to launch cloud-hypervisor: %q", err)
	}
	clh.state.PID = pid

	if err := clh.bootVM(ctx); err != nil {
		return err
	}

	clh.state.state = clhReady
	return nil
}

// getSandboxConsole builds the path of the console where we can read
// logs coming from the sandbox.
func (clh *cloudHypervisor) getSandboxConsole(id string) (string, string, error) {
	clh.Logger().WithField("function", "getSandboxConsole").WithField("id", id).Info("Get Sandbox Console")
	master, slave, err := console.NewPty()
	if err != nil {
		clh.Logger().Debugf("Error create pseudo tty: %v", err)
		return consoleProtoPty, "", err
	}
	clh.console = master

	return consoleProtoPty, slave, nil
}

func (clh *cloudHypervisor) disconnect() {
	clh.Logger().WithField("function", "disconnect").Info("Disconnecting Sandbox Console")
}

func (clh *cloudHypervisor) getThreadIDs() (vcpuThreadIDs, error) {

	clh.Logger().WithField("function", "getThreadIDs").Info("get thread ID's")

	var vcpuInfo vcpuThreadIDs

	vcpuInfo.vcpus = make(map[int]int)

	return vcpuInfo, nil
}

func clhDriveIndexToID(i int) string {
	return "clh_drive_" + strconv.Itoa(i)
}

func (clh *cloudHypervisor) hotplugAddBlockDevice(drive *config.BlockDrive) error {
	if clh.config.BlockDeviceDriver != config.VirtioBlock {
		return fmt.Errorf("incorrect hypervisor configuration on 'block_device_driver':"+
			" using '%v' but only support '%v'", clh.config.BlockDeviceDriver, config.VirtioBlock)
	}

	var err error

	cl := clh.client()
	ctx, cancel := context.WithTimeout(context.Background(), clhHotPlugAPITimeout*time.Second)
	defer cancel()

	driveID := clhDriveIndexToID(drive.Index)

	//Explicitly set PCIAddr to NULL, so that VirtPath can be used
	drive.PCIAddr = ""

	if drive.Pmem {
		err = fmt.Errorf("pmem device hotplug not supported")
	} else {
		blkDevice := chclient.DiskConfig{
			Path:      drive.File,
			Readonly:  drive.ReadOnly,
			VhostUser: false,
			Id:        driveID,
		}
		_, _, err = cl.VmAddDiskPut(ctx, blkDevice)
	}

	if err != nil {
		err = fmt.Errorf("failed to hotplug block device %+v %s", drive, openAPIClientError(err))
	}
	return err
}

func (clh *cloudHypervisor) hotPlugVFIODevice(device config.VFIODev) error {
	cl := clh.client()
	ctx, cancel := context.WithTimeout(context.Background(), clhHotPlugAPITimeout*time.Second)
	defer cancel()

	_, _, err := cl.VmAddDevicePut(ctx, chclient.VmAddDevice{Path: device.SysfsDev, Id: device.ID})
	if err != nil {
		err = fmt.Errorf("Failed to hotplug device %+v %s", device, openAPIClientError(err))
	}
	return err
}

func (clh *cloudHypervisor) hotplugAddDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	span, _ := clh.trace("hotplugAddDevice")
	defer span.Finish()

	switch devType {
	case blockDev:
		drive := devInfo.(*config.BlockDrive)
		return nil, clh.hotplugAddBlockDevice(drive)
	case vfioDev:
		device := devInfo.(*config.VFIODev)
		return nil, clh.hotPlugVFIODevice(*device)
	default:
		return nil, fmt.Errorf("cannot hotplug device: unsupported device type '%v'", devType)
	}

}

func (clh *cloudHypervisor) hotplugRemoveBlockDevice(drive *config.BlockDrive) error {
	cl := clh.client()
	ctx, cancel := context.WithTimeout(context.Background(), clhHotPlugAPITimeout*time.Second)
	defer cancel()

	driveID := clhDriveIndexToID(drive.Index)

	if drive.Pmem {
		return fmt.Errorf("pmem device hotplug remove not supported")
	}

	_, err := cl.VmRemoveDevicePut(ctx, chclient.VmRemoveDevice{Id: driveID})

	if err != nil {
		err = fmt.Errorf("failed to hotplug remove block device %+v %s", drive, openAPIClientError(err))
	}

	return err
}

func (clh *cloudHypervisor) hotplugRemoveVfioDevice(device *config.VFIODev) error {
	cl := clh.client()
	ctx, cancel := context.WithTimeout(context.Background(), clhHotPlugAPITimeout*time.Second)
	defer cancel()

	_, err := cl.VmRemoveDevicePut(ctx, chclient.VmRemoveDevice{Id: device.ID})

	if err != nil {
		err = fmt.Errorf("failed to hotplug remove vfio device %+v %s", device, openAPIClientError(err))
	}

	return err
}

func (clh *cloudHypervisor) hotplugRemoveDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	span, _ := clh.trace("hotplugRemoveDevice")
	defer span.Finish()

	switch devType {
	case blockDev:
		return nil, clh.hotplugRemoveBlockDevice(devInfo.(*config.BlockDrive))
	case vfioDev:
		return nil, clh.hotplugRemoveVfioDevice(devInfo.(*config.VFIODev))
	default:
		clh.Logger().WithFields(log.Fields{"devInfo": devInfo,
			"deviceType": devType}).Error("hotplugRemoveDevice: unsupported device")
		return nil, fmt.Errorf("Could not hot remove device: unsupported device: %v, type: %v",
			devInfo, devType)
	}
}

func (clh *cloudHypervisor) hypervisorConfig() HypervisorConfig {
	return clh.config
}

func (clh *cloudHypervisor) resizeMemory(reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, memoryDevice, error) {

	// TODO: Add support for virtio-mem

	if probe {
		return 0, memoryDevice{}, errors.New("probe memory is not supported for cloud-hypervisor")
	}

	if reqMemMB == 0 {
		// This is a corner case if requested to resize to 0 means something went really wrong.
		return 0, memoryDevice{}, errors.New("Can not resize memory to 0")
	}

	info, err := clh.vmInfo()
	if err != nil {
		return 0, memoryDevice{}, err
	}

	currentMem := utils.MemUnit(info.Config.Memory.Size) * utils.Byte
	newMem := utils.MemUnit(reqMemMB) * utils.MiB

	// Early check to verify if boot memory is the same as requested
	if currentMem == newMem {
		clh.Logger().WithField("memory", reqMemMB).Debugf("VM already has requested memory")
		return uint32(currentMem.ToMiB()), memoryDevice{}, nil
	}

	if currentMem > newMem {
		clh.Logger().Warn("Remove memory is not supported, nothing to do")
		return uint32(currentMem.ToMiB()), memoryDevice{}, nil
	}

	blockSize := utils.MemUnit(memoryBlockSizeMB) * utils.MiB
	hotplugSize := (newMem - currentMem).AlignMem(blockSize)

	// Update memory request to increase memory aligned block
	alignedRequest := currentMem + hotplugSize
	if newMem != alignedRequest {
		clh.Logger().WithFields(log.Fields{"request": newMem, "aligned-request": alignedRequest}).Debug("aligning VM memory request")
		newMem = alignedRequest
	}

	// Check if memory is the same as requested, a second check is done
	// to consider the memory request now that is updated to be memory aligned
	if currentMem == newMem {
		clh.Logger().WithFields(log.Fields{"current-memory": currentMem, "new-memory": newMem}).Debug("VM already has requested memory(after alignment)")
		return uint32(currentMem.ToMiB()), memoryDevice{}, nil
	}

	cl := clh.client()
	ctx, cancelResize := context.WithTimeout(context.Background(), clhAPITimeout*time.Second)
	defer cancelResize()

	// OpenApi does not support uint64, convert to int64
	resize := chclient.VmResize{DesiredRam: int64(newMem.ToBytes())}
	clh.Logger().WithFields(log.Fields{"current-memory": currentMem, "new-memory": newMem}).Debug("updating VM memory")
	if _, err = cl.VmResizePut(ctx, resize); err != nil {
		clh.Logger().WithFields(log.Fields{"current-memory": currentMem, "new-memory": newMem}).Warnf("failed to update memory %s", openAPIClientError(err))
		err = fmt.Errorf("Failed to resize memory from %d to %d: %s", currentMem, newMem, openAPIClientError(err))
		return uint32(currentMem.ToMiB()), memoryDevice{}, openAPIClientError(err)
	}

	return uint32(newMem.ToMiB()), memoryDevice{sizeMB: int(hotplugSize.ToMiB())}, nil
}

func (clh *cloudHypervisor) resizeVCPUs(reqVCPUs uint32) (currentVCPUs uint32, newVCPUs uint32, err error) {
	cl := clh.client()

	// Retrieve the number of current vCPUs via HTTP API
	info, err := clh.vmInfo()
	if err != nil {
		clh.Logger().WithField("function", "resizeVCPUs").WithError(err).Info("[clh] vmInfo failed")
		return 0, 0, openAPIClientError(err)
	}

	currentVCPUs = uint32(info.Config.Cpus.BootVcpus)
	newVCPUs = currentVCPUs

	// Sanity check
	if reqVCPUs == 0 {
		clh.Logger().WithField("function", "resizeVCPUs").Debugf("Cannot resize vCPU to 0")
		return currentVCPUs, newVCPUs, fmt.Errorf("Cannot resize vCPU to 0")
	}
	if reqVCPUs > uint32(info.Config.Cpus.MaxVcpus) {
		clh.Logger().WithFields(log.Fields{
			"function":    "resizeVCPUs",
			"reqVCPUs":    reqVCPUs,
			"clhMaxVCPUs": info.Config.Cpus.MaxVcpus,
		}).Warn("exceeding the 'clhMaxVCPUs' (resizing to 'clhMaxVCPUs')")

		reqVCPUs = uint32(info.Config.Cpus.MaxVcpus)
	}

	// Resize (hot-plug) vCPUs via HTTP API
	ctx, cancel := context.WithTimeout(context.Background(), clhAPITimeout*time.Second)
	defer cancel()
	if _, err = cl.VmResizePut(ctx, chclient.VmResize{DesiredVcpus: int32(reqVCPUs)}); err != nil {
		return currentVCPUs, newVCPUs, errors.Wrap(err, "[clh] VmResizePut failed")
	}

	newVCPUs = reqVCPUs

	return currentVCPUs, newVCPUs, nil
}

func (clh *cloudHypervisor) cleanup() error {
	clh.Logger().WithField("function", "cleanup").Info("cleanup")
	return nil
}

func (clh *cloudHypervisor) pauseSandbox() error {
	clh.Logger().WithField("function", "pauseSandbox").Info("Pause Sandbox")
	return nil
}

func (clh *cloudHypervisor) saveSandbox() error {
	clh.Logger().WithField("function", "saveSandboxC").Info("Save Sandbox")
	return nil
}

func (clh *cloudHypervisor) resumeSandbox() error {
	clh.Logger().WithField("function", "resumeSandbox").Info("Resume Sandbox")
	return nil
}

// stopSandbox will stop the Sandbox's VM.
func (clh *cloudHypervisor) stopSandbox() (err error) {
	span, _ := clh.trace("stopSandbox")
	defer span.Finish()
	clh.Logger().WithField("function", "stopSandbox").Info("Stop Sandbox")
	return clh.terminate()
}

func (clh *cloudHypervisor) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	return errors.New("cloudHypervisor is not supported by VM cache")
}

func (clh *cloudHypervisor) toGrpc() ([]byte, error) {
	return nil, errors.New("cloudHypervisor is not supported by VM cache")
}

func (clh *cloudHypervisor) save() (s persistapi.HypervisorState) {
	s.Pid = clh.state.PID
	s.Type = string(ClhHypervisor)
	s.VirtiofsdPid = clh.state.VirtiofsdPID
	s.APISocket = clh.state.apiSocket
	return
}

func (clh *cloudHypervisor) load(s persistapi.HypervisorState) {
	clh.state.PID = s.Pid
	clh.state.VirtiofsdPID = s.VirtiofsdPid
	clh.state.apiSocket = s.APISocket
}

func (clh *cloudHypervisor) check() error {
	cl := clh.client()
	ctx, cancel := context.WithTimeout(context.Background(), clhAPITimeout*time.Second)
	defer cancel()

	_, _, err := cl.VmmPingGet(ctx)
	return err
}

func (clh *cloudHypervisor) getPids() []int {

	var pids []int
	pids = append(pids, clh.state.PID)

	return pids
}

func (clh *cloudHypervisor) addDevice(devInfo interface{}, devType deviceType) error {
	span, _ := clh.trace("addDevice")
	defer span.Finish()

	var err error

	switch v := devInfo.(type) {
	case Endpoint:
		if err := clh.addNet(v); err != nil {
			return err
		}
	case types.HybridVSock:
		clh.addVSock(defaultGuestVSockCID, v.UdsPath)
	case types.Volume:
		err = clh.addVolume(v)
	default:
		clh.Logger().WithField("function", "addDevice").Warnf("Add device of type %v is not supported.", v)
		return fmt.Errorf("Not implemented support for %s", v)
	}

	return err
}

//###########################################################################
//
// Local helper methods related to the hypervisor interface implementation
//
//###########################################################################

func (clh *cloudHypervisor) Logger() *log.Entry {
	return virtLog.WithField("subsystem", "cloudHypervisor")
}

// Adds all capabilities supported by cloudHypervisor implementation of hypervisor interface
func (clh *cloudHypervisor) capabilities() types.Capabilities {
	span, _ := clh.trace("capabilities")
	defer span.Finish()

	clh.Logger().WithField("function", "capabilities").Info("get Capabilities")
	var caps types.Capabilities
	caps.SetFsSharingSupport()
	caps.SetBlockDeviceHotplugSupport()
	return caps
}

func (clh *cloudHypervisor) trace(name string) (opentracing.Span, context.Context) {

	if clh.ctx == nil {
		clh.Logger().WithField("type", "bug").Error("trace called before context set")
		clh.ctx = context.Background()
	}

	span, ctx := opentracing.StartSpanFromContext(clh.ctx, name)

	span.SetTag("subsystem", "cloudHypervisor")
	span.SetTag("type", "clh")

	return span, ctx
}

func (clh *cloudHypervisor) terminate() (err error) {
	span, _ := clh.trace("terminate")
	defer span.Finish()

	pid := clh.state.PID
	pidRunning := true
	if pid == 0 {
		pidRunning = false
	}

	clh.Logger().WithField("PID", pid).Info("Stopping Cloud Hypervisor")

	if pidRunning {
		clhRunning, _ := clh.isClhRunning(clhStopSandboxTimeout)
		if clhRunning {
			ctx, cancel := context.WithTimeout(context.Background(), clhStopSandboxTimeout*time.Second)
			defer cancel()
			if _, err = clh.client().ShutdownVMM(ctx); err != nil {
				return err
			}
		}
	}

	// At this point the VMM was stop nicely, but need to check if PID is still running
	// Wait for the VM process to terminate
	tInit := time.Now()
	for {
		if err = syscall.Kill(pid, syscall.Signal(0)); err != nil {
			pidRunning = false
			break
		}

		if time.Since(tInit).Seconds() >= clhStopSandboxTimeout {
			pidRunning = true
			clh.Logger().Warnf("VM still running after waiting %ds", clhStopSandboxTimeout)
			break
		}

		// Let's avoid to run a too busy loop
		time.Sleep(time.Duration(50) * time.Millisecond)
	}

	// Let's try with a hammer now, a SIGKILL should get rid of the
	// VM process.
	if pidRunning {
		if err = syscall.Kill(pid, syscall.SIGKILL); err != nil {
			return fmt.Errorf("Fatal, failed to kill hypervisor process, error: %s", err)
		}
	}

	if clh.virtiofsd == nil {
		return errors.New("virtiofsd config is nil, failed to stop it")
	}

	if err := clh.cleanupVM(true); err != nil {
		return err
	}

	return clh.virtiofsd.Stop()
}

func (clh *cloudHypervisor) reset() {
	clh.state.reset()
}

func (clh *cloudHypervisor) generateSocket(id string) (interface{}, error) {
	udsPath, err := clh.vsockSocketPath(id)
	if err != nil {
		clh.Logger().Info("Can't generate socket path for cloud-hypervisor")
		return types.HybridVSock{}, err
	}

	return types.HybridVSock{
		UdsPath: udsPath,
		Port:    uint32(vSockPort),
	}, nil
}

func (clh *cloudHypervisor) virtioFsSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(clh.store.RunVMStoragePath(), id, virtioFsSocket)
}

func (clh *cloudHypervisor) vsockSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(clh.store.RunVMStoragePath(), id, clhSocket)
}

func (clh *cloudHypervisor) apiSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(clh.store.RunVMStoragePath(), id, clhAPISocket)
}

func (clh *cloudHypervisor) waitVMM(timeout uint) error {
	clhRunning, err := clh.isClhRunning(timeout)

	if err != nil {
		return err
	}

	if !clhRunning {
		return fmt.Errorf("CLH is not running")
	}

	return nil
}

func (clh *cloudHypervisor) clhPath() (string, error) {
	p, err := clh.config.HypervisorAssetPath()
	if err != nil {
		return "", err
	}

	if p == "" {
		p = defaultClhPath
	}

	if _, err = os.Stat(p); os.IsNotExist(err) {
		return "", fmt.Errorf("Cloud-Hypervisor path (%s) does not exist", p)
	}

	return p, nil
}

func (clh *cloudHypervisor) getAvailableVersion() error {

	clhPath, err := clh.clhPath()
	if err != nil {
		return err

	}

	cmd := exec.Command(clhPath, "--version")
	out, err := cmd.CombinedOutput()
	if err != nil {
		return err
	}

	words := strings.Fields(string(out))
	if len(words) != 2 {
		return errors.New("Failed to parse cloud-hypervisor version response. Illegal length")

	}
	versionSplit := strings.SplitN(words[1], ".", -1)
	if len(versionSplit) != 3 {
		return errors.New("Failed to parse cloud-hypervisor version field. Illegal length")

	}

	// Remove 'v' prefix if has one
	versionSplit[0] = strings.TrimLeft(versionSplit[0], "v")
	major, err := strconv.ParseUint(versionSplit[0], 10, 64)
	if err != nil {
		return err

	}
	minor, err := strconv.ParseUint(versionSplit[1], 10, 64)
	if err != nil {
		return err

	}

	// revision could have aditional commit information separated by '-'
	revisionSplit := strings.SplitN(versionSplit[2], "-", -1)
	if len(revisionSplit) < 1 {
		return errors.Errorf("Failed parse cloud-hypervisor revision %s", versionSplit[2])
	}
	revision, err := strconv.ParseUint(revisionSplit[0], 10, 64)
	if err != nil {
		return err
	}

	clh.version = CloudHypervisorVersion{
		Major:    int(major),
		Minor:    int(minor),
		Revision: int(revision),
	}
	return nil

}

func (clh *cloudHypervisor) LaunchClh() (int, error) {

	clhPath, err := clh.clhPath()
	if err != nil {
		return -1, err
	}

	args := []string{cscAPIsocket, clh.state.apiSocket}
	if clh.config.Debug {
		// Cloud hypervisor log levels
		// 'v' occurrences increase the level
		//0 =>  Error
		//1 =>  Warn
		//2 =>  Info
		//3 =>  Debug
		//4+ => Trace
		// Use Info, the CI runs with debug enabled
		// a high level of logging increases the boot time
		// and in a nested environment this could increase
		// the chances to fail because agent is not
		// ready on time.
		args = append(args, "-vv")
	}

	// Disable the 'seccomp' option in clh for now.
	// In this way, we can separate the periodic failures caused
	// by incomplete `seccomp` filters from other failures.
	// We will bring it back after completing the `seccomp` filter.
	args = append(args, "--seccomp", "false")

	clh.Logger().WithField("path", clhPath).Info()
	clh.Logger().WithField("args", strings.Join(args, " ")).Info()

	cmdHypervisor := exec.Command(clhPath, args...)
	if clh.config.Debug {
		cmdHypervisor.Env = os.Environ()
		cmdHypervisor.Env = append(cmdHypervisor.Env, "RUST_BACKTRACE=full")
		if clh.console != nil {
			cmdHypervisor.Stderr = clh.console
			cmdHypervisor.Stdout = clh.console
		}
	}

	cmdHypervisor.Stderr = cmdHypervisor.Stdout

	err = utils.StartCmd(cmdHypervisor)
	if err != nil {
		return -1, err
	}

	if err := clh.waitVMM(clhTimeout); err != nil {
		clh.Logger().WithField("error", err).Warn("cloud-hypervisor init failed")
		return -1, err
	}

	return cmdHypervisor.Process.Pid, nil
}

//###########################################################################
//
// Cloud-hypervisor CLI builder
//
//###########################################################################

const (
	cctOFF  string = "Off"
	cctNULL string = "Null"
	cctTTY  string = "Tty"
)

const (
	cscAPIsocket string = "--api-socket"
)

//****************************************
// The kernel command line
//****************************************

func kernelParamsToString(params []Param) string {

	var paramBuilder strings.Builder
	for _, p := range params {
		paramBuilder.WriteString(p.Key)
		if len(p.Value) > 0 {

			paramBuilder.WriteString("=")
			paramBuilder.WriteString(p.Value)
		}
		paramBuilder.WriteString(" ")
	}
	return strings.TrimSpace(paramBuilder.String())
}

//****************************************
// API calls
//****************************************
func (clh *cloudHypervisor) isClhRunning(timeout uint) (bool, error) {

	pid := clh.state.PID

	// Check if clh process is running, in case it is not, let's
	// return from here.
	if err := syscall.Kill(pid, syscall.Signal(0)); err != nil {
		return false, nil
	}

	timeStart := time.Now()
	cl := clh.client()
	for {
		ctx, cancel := context.WithTimeout(context.Background(), clhAPITimeout*time.Second)
		defer cancel()
		_, _, err := cl.VmmPingGet(ctx)
		if err == nil {
			return true, nil
		}

		if time.Since(timeStart).Seconds() > float64(timeout) {
			return false, fmt.Errorf("Failed to connect to API (timeout %ds): %s", timeout, openAPIClientError(err))
		}

		time.Sleep(time.Duration(10) * time.Millisecond)
	}

}

func (clh *cloudHypervisor) client() clhClient {
	if clh.APIClient == nil {
		clh.APIClient = clh.newAPIClient()
	}

	return clh.APIClient
}

func (clh *cloudHypervisor) newAPIClient() *chclient.DefaultApiService {

	cfg := chclient.NewConfiguration()

	socketTransport := &http.Transport{
		DialContext: func(ctx context.Context, network, path string) (net.Conn, error) {
			addr, err := net.ResolveUnixAddr("unix", clh.state.apiSocket)
			if err != nil {
				return nil, err

			}

			return net.DialUnix("unix", nil, addr)
		},
	}

	cfg.HTTPClient = http.DefaultClient
	cfg.HTTPClient.Transport = socketTransport

	return chclient.NewAPIClient(cfg).DefaultApi
}

func openAPIClientError(err error) error {

	if err == nil {
		return nil
	}

	reason := ""
	if apierr, ok := err.(chclient.GenericOpenAPIError); ok {
		reason = string(apierr.Body())
	}

	return fmt.Errorf("error: %v reason: %s", err, reason)
}

func (clh *cloudHypervisor) bootVM(ctx context.Context) error {

	cl := clh.client()

	if clh.config.Debug {
		bodyBuf, err := json.Marshal(clh.vmconfig)
		if err != nil {
			return err
		}
		clh.Logger().WithField("body", string(bodyBuf)).Debug("VM config")
	}
	_, err := cl.CreateVM(ctx, clh.vmconfig)

	if err != nil {
		return openAPIClientError(err)
	}

	info, err := clh.vmInfo()

	if err != nil {
		return err
	}

	clh.Logger().Debugf("VM state after create: %#v", info)

	if info.State != clhStateCreated {
		return fmt.Errorf("VM state is not 'Created' after 'CreateVM'")
	}

	clh.Logger().Debug("Booting VM")
	_, err = cl.BootVM(ctx)

	if err != nil {
		return openAPIClientError(err)
	}

	info, err = clh.vmInfo()

	if err != nil {
		return err
	}

	clh.Logger().Debugf("VM state after boot: %#v", info)

	if info.State != clhStateRunning {
		return fmt.Errorf("VM state is not 'Running' after 'BootVM'")
	}

	return nil
}

func (clh *cloudHypervisor) addVSock(cid int64, path string) {
	clh.Logger().WithFields(log.Fields{
		"path": path,
		"cid":  cid,
	}).Info("Adding HybridVSock")

	clh.vmconfig.Vsock = chclient.VsockConfig{Cid: cid, Socket: path}
}

func (clh *cloudHypervisor) addNet(e Endpoint) error {
	clh.Logger().WithField("endpoint-type", e).Debugf("Adding Endpoint of type %v", e)

	mac := e.HardwareAddr()
	netPair := e.NetworkPair()

	if netPair == nil {
		return errors.New("net Pair to be added is nil, needed to get TAP path")
	}

	tapPath := netPair.TapInterface.TAPIface.Name

	if tapPath == "" {
		return errors.New("TAP path in network pair is empty")
	}

	clh.Logger().WithFields(log.Fields{
		"mac": mac,
		"tap": tapPath,
	}).Info("Adding Net")

	clh.vmconfig.Net = append(clh.vmconfig.Net, chclient.NetConfig{Mac: mac, Tap: tapPath})
	return nil
}

// Add shared Volume using virtiofs
func (clh *cloudHypervisor) addVolume(volume types.Volume) error {
	if clh.config.SharedFS != config.VirtioFS {
		return fmt.Errorf("shared fs method not supported %s", clh.config.SharedFS)
	}

	vfsdSockPath, err := clh.virtioFsSocketPath(clh.id)
	if err != nil {
		return err
	}

	if clh.config.VirtioFSCache == virtioFsCacheAlways {
		clh.vmconfig.Fs = []chclient.FsConfig{
			{
				Tag:       volume.MountTag,
				CacheSize: int64(clh.config.VirtioFSCacheSize << 20),
				Socket:    vfsdSockPath,
			},
		}
	} else {
		clh.vmconfig.Fs = []chclient.FsConfig{
			{
				Tag:    volume.MountTag,
				Socket: vfsdSockPath,
			},
		}

	}

	clh.Logger().Debug("Adding share volume to hypervisor: ", volume.MountTag)
	return nil
}

// cleanupVM will remove generated files and directories related with the virtual machine
func (clh *cloudHypervisor) cleanupVM(force bool) error {

	if clh.id == "" {
		return errors.New("Hypervisor ID is empty")
	}

	clh.Logger().Debug("removing vm sockets")

	path, err := clh.vsockSocketPath(clh.id)
	if err == nil {
		if err := os.Remove(path); err != nil {
			clh.Logger().WithField("path", path).Warn("removing vm socket failed")
		}
	}

	// cleanup vm path
	dir := filepath.Join(clh.store.RunVMStoragePath(), clh.id)

	// If it's a symlink, remove both dir and the target.
	link, err := filepath.EvalSymlinks(dir)
	if err != nil {
		clh.Logger().WithError(err).WithField("dir", dir).Warn("failed to resolve vm path")
	}

	clh.Logger().WithFields(log.Fields{
		"link": link,
		"dir":  dir,
	}).Infof("cleanup vm path")

	if err := os.RemoveAll(dir); err != nil {
		if !force {
			return err
		}
		clh.Logger().WithError(err).Warnf("failed to remove vm path %s", dir)
	}
	if link != dir && link != "" {
		if err := os.RemoveAll(link); err != nil {
			if !force {
				return err
			}
			clh.Logger().WithError(err).WithField("link", link).Warn("failed to remove resolved vm path")
		}
	}

	if clh.config.VMid != "" {
		dir = filepath.Join(clh.store.RunStoragePath(), clh.config.VMid)
		if err := os.RemoveAll(dir); err != nil {
			if !force {
				return err
			}
			clh.Logger().WithError(err).WithField("path", dir).Warnf("failed to remove vm path")
		}
	}

	clh.reset()

	return nil
}

// vmInfo ask to hypervisor for current VM status
func (clh *cloudHypervisor) vmInfo() (chclient.VmInfo, error) {
	cl := clh.client()
	ctx, cancelInfo := context.WithTimeout(context.Background(), clhAPITimeout*time.Second)
	defer cancelInfo()

	info, _, err := cl.VmInfoGet(ctx)
	if err != nil {
		clh.Logger().WithError(openAPIClientError(err)).Warn("VmInfoGet failed")
	}
	return info, openAPIClientError(err)

}

func (clh *cloudHypervisor) isRateLimiterBuiltin() bool {
	return false
}
