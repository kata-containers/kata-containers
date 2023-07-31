//go:build linux

// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"

	"github.com/pkg/errors"
	"github.com/prometheus/procfs"
	"github.com/sirupsen/logrus"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	hv "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// acrnTracingTags defines tags for the trace span
var acrnTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "hypervisor",
	"type":      "acrn",
}

// AcrnState keeps track of VM UUID, PID.
type AcrnState struct {
	PID int
}

// Acrn is an Hypervisor interface implementation for the Linux acrn hypervisor.
type Acrn struct {
	sandbox    *Sandbox
	ctx        context.Context
	arch       acrnArch
	store      persistapi.PersistDriver
	id         string
	acrnConfig Config
	config     HypervisorConfig
	state      AcrnState
}

const (
	acrnConsoleSocket          = "console.sock"
	acrnStopSandboxTimeoutSecs = 15
)

// agnostic list of kernel parameters
var acrnDefaultKernelParameters = []Param{
	{"panic", "1"},
}

func (a *Acrn) kernelParameters() string {
	// get a list of arch kernel parameters
	params := a.arch.kernelParameters(a.config.Debug)

	// use default parameters
	params = append(params, acrnDefaultKernelParameters...)

	// set the maximum number of vCPUs
	params = append(params, Param{"maxcpus", fmt.Sprintf("%d", a.config.DefaultMaxVCPUs)})

	// add the params specified by the provided config. As the kernel
	// honours the last parameter value set and since the config-provided
	// params are added here, they will take priority over the defaults.
	params = append(params, a.config.KernelParams...)

	paramsStr := SerializeParams(params, "=")

	return strings.Join(paramsStr, " ")
}

// Adds all capabilities supported by Acrn implementation of hypervisor interface
func (a *Acrn) Capabilities(ctx context.Context) types.Capabilities {
	span, _ := katatrace.Trace(ctx, a.Logger(), "Capabilities", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	return a.arch.capabilities(a.config)
}

func (a *Acrn) HypervisorConfig() HypervisorConfig {
	return a.config
}

// Get cpu apicid to identify vCPU that will be assigned for a VM by reading `proc/cpuinfo`
func (a *Acrn) getNextApicid() (string, error) {
	fs, err := procfs.NewFS("/proc")
	if err != nil {
		return "", err
	}

	cpuinfo, err := fs.CPUInfo()
	if err != nil {
		return "", err
	}

	prevIdx := -1
	fileName := filepath.Join(a.config.VMStorePath, "cpu_affinity_idx")
	_, err = os.Stat(fileName)
	if err == nil {
		data, err := os.ReadFile(fileName)
		if err != nil {
			a.Logger().Error("Loading cpu affinity index from file failed!")
			return "", err
		}

		prevIdx, err = strconv.Atoi(string(data))
		if err != nil {
			a.Logger().Error("CreateVM: Convert from []byte to integer failed!")
			return "", err
		}

		if prevIdx >= (len(cpuinfo) - 1) {
			prevIdx = -1
		}
	}

	currentIdx := prevIdx + 1
	err = os.WriteFile(fileName, []byte(strconv.Itoa(currentIdx)), defaultFilePerms)
	if err != nil {
		a.Logger().Error("Storing cpu affinity index from file failed!")
		return "", err
	}

	return cpuinfo[currentIdx].APICID, nil
}

// get the acrn binary path
func (a *Acrn) acrnPath() (string, error) {
	p, err := a.config.HypervisorAssetPath()
	if err != nil {
		return "", err
	}

	if p == "" {
		p, err = a.arch.acrnPath()
		if err != nil {
			return "", err
		}
	}

	if _, err = os.Stat(p); os.IsNotExist(err) {
		return "", fmt.Errorf("acrn path (%s) does not exist", p)
	}

	return p, nil
}

// get the ACRNCTL binary path
func (a *Acrn) acrnctlPath() (string, error) {
	ctlpath, err := a.config.HypervisorCtlAssetPath()
	if err != nil {
		return "", err
	}

	if ctlpath == "" {
		ctlpath, err = a.arch.acrnctlPath()
		if err != nil {
			return "", err
		}
	}

	if _, err = os.Stat(ctlpath); os.IsNotExist(err) {
		return "", fmt.Errorf("acrnctl path (%s) does not exist", ctlpath)
	}

	return ctlpath, nil
}

// Logger returns a logrus logger appropriate for logging acrn messages
func (a *Acrn) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "acrn")
}

func (a *Acrn) memoryTopology() (Memory, error) {
	memMb := uint64(a.config.MemorySize)

	return a.arch.memoryTopology(memMb), nil
}

func (a *Acrn) appendImage(devices []Device, imagePath string) ([]Device, error) {
	if imagePath == "" {
		return nil, fmt.Errorf("Image path is empty: %s", imagePath)
	}

	var err error
	devices, err = a.arch.appendImage(devices, imagePath)
	if err != nil {
		return nil, err
	}

	return devices, nil
}

func (a *Acrn) buildDevices(ctx context.Context, imagePath string) ([]Device, error) {
	var devices []Device

	if imagePath == "" {
		return nil, fmt.Errorf("Image Path should not be empty: %s", imagePath)
	}

	_, console, err := a.GetVMConsole(ctx, a.id)
	if err != nil {
		return nil, err
	}

	// Add bridges before any other devices. This way we make sure that
	// bridge gets the first available PCI address.
	devices = a.arch.appendBridges(devices)

	//Add LPC device to the list of other devices.
	devices = a.arch.appendLPC(devices)

	devices = a.arch.appendConsole(devices, console)

	devices, err = a.appendImage(devices, imagePath)
	if err != nil {
		return nil, err
	}

	// Create virtio blk devices with dummy backend as a place
	// holder for container rootfs (as acrn doesn't support hot-plug).
	// Once the container rootfs is known, replace the dummy backend
	// with actual path (using block rescan feature in acrn)
	devices, err = a.createDummyVirtioBlkDev(ctx, devices)
	if err != nil {
		return nil, err
	}

	return devices, nil
}

// setup sets the Acrn structure up.
func (a *Acrn) setup(ctx context.Context, id string, hypervisorConfig *HypervisorConfig) error {
	span, _ := katatrace.Trace(ctx, a.Logger(), "setup", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	if err := a.setConfig(hypervisorConfig); err != nil {
		return err
	}

	a.id = id
	var err error
	a.arch, err = newAcrnArch(a.config)
	if err != nil {
		return err
	}

	return nil
}

func (a *Acrn) createDummyVirtioBlkDev(ctx context.Context, devices []Device) ([]Device, error) {
	span, _ := katatrace.Trace(ctx, a.Logger(), "createDummyVirtioBlkDev", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	// Since acrn doesn't support hot-plug, dummy virtio-blk
	// devices are added and later replaced with container-rootfs.
	// Starting from driveIndex 1, as 0 is allocated for VM rootfs.
	for driveIndex := 1; driveIndex <= AcrnBlkDevPoolSz; driveIndex++ {
		drive := config.BlockDrive{
			File:  "nodisk",
			Index: driveIndex,
		}

		devices = a.arch.appendBlockDevice(devices, drive)
	}

	return devices, nil
}

func (a *Acrn) setConfig(config *HypervisorConfig) error {
	a.config = *config

	return nil
}

// CreateVM is the VM creation
func (a *Acrn) CreateVM(ctx context.Context, id string, network Network, hypervisorConfig *HypervisorConfig) error {
	// Save the tracing context
	a.ctx = ctx

	span, ctx := katatrace.Trace(ctx, a.Logger(), "CreateVM", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	if err := a.setup(ctx, id, hypervisorConfig); err != nil {
		return err
	}

	memory, err := a.memoryTopology()
	if err != nil {
		return err
	}

	kernelPath, err := a.config.KernelAssetPath()
	if err != nil {
		return err
	}

	imagePath, err := a.config.ImageAssetPath()
	if err != nil {
		return err
	}

	kernel := Kernel{
		Path:      kernelPath,
		ImagePath: imagePath,
		Params:    a.kernelParameters(),
	}

	devices, err := a.buildDevices(ctx, imagePath)
	if err != nil {
		return err
	}

	acrnPath, err := a.acrnPath()
	if err != nil {
		return err
	}

	acrnctlPath, err := a.acrnctlPath()
	if err != nil {
		return err
	}

	vmName := fmt.Sprintf("sbx-%s", a.id)
	if len(vmName) > 15 {
		return fmt.Errorf("VM Name len is %d but ACRN supports max VM name len of 15.", len(vmName))
	}

	apicID, err := a.getNextApicid()
	if err != nil {
		return err
	}

	acrnConfig := Config{
		ACPIVirt: true,
		Path:     acrnPath,
		CtlPath:  acrnctlPath,
		Memory:   memory,
		Devices:  devices,
		Kernel:   kernel,
		Name:     vmName,
		ApicID:   apicID,
	}

	a.acrnConfig = acrnConfig

	return nil
}

// StartVM will start the Sandbox's VM.
func (a *Acrn) StartVM(ctx context.Context, timeoutSecs int) error {
	span, ctx := katatrace.Trace(ctx, a.Logger(), "StartVM", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	if a.config.Debug {
		params := a.arch.kernelParameters(a.config.Debug)
		strParams := SerializeParams(params, "=")
		formatted := strings.Join(strParams, " ")

		// The name of this field matches a similar one generated by
		// the runtime and allows users to identify which parameters
		// are set here, which come from the runtime and which are set
		// by the user.
		a.Logger().WithField("default-kernel-parameters", formatted).Debug()
	}

	vmPath := filepath.Join(a.config.VMStorePath, a.id)
	err := os.MkdirAll(vmPath, DirMode)
	if err != nil {
		return err
	}
	defer func() {
		if err != nil {
			if err := os.RemoveAll(vmPath); err != nil {
				a.Logger().WithError(err).Error("Failed to clean up vm directory")
			}
		}
	}()

	var strErr string
	var PID int
	a.Logger().Error("StartVM: LaunchAcrn() function called")
	PID, strErr, err = LaunchAcrn(a.acrnConfig, virtLog.WithField("subsystem", "acrn-dm"))
	if err != nil {
		return fmt.Errorf("%s", strErr)
	}
	a.state.PID = PID

	if err = a.waitVM(ctx, timeoutSecs); err != nil {
		a.Logger().WithField("acrn wait failed:", err).Debug()
		return err
	}

	return nil
}

// waitVM will wait for the Sandbox's VM to be up and running.
func (a *Acrn) waitVM(ctx context.Context, timeoutSecs int) error {
	span, _ := katatrace.Trace(ctx, a.Logger(), "waitVM", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	if timeoutSecs < 0 {
		return fmt.Errorf("Invalid timeout %ds", timeoutSecs)
	}

	time.Sleep(time.Duration(timeoutSecs) * time.Second)

	return nil
}

// StopVM will stop the Sandbox's VM.
func (a *Acrn) StopVM(ctx context.Context, waitOnly bool) (err error) {
	span, _ := katatrace.Trace(ctx, a.Logger(), "StopVM", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	a.Logger().Info("Stopping acrn VM")

	defer func() {
		if err != nil {
			a.Logger().Info("StopVM failed")
		} else {
			a.Logger().Info("acrn VM stopped")
		}
	}()

	fileName := filepath.Join(a.config.VMStorePath, "cpu_affinity_idx")
	data, err := os.ReadFile(fileName)
	if err != nil {
		a.Logger().Error("Loading cpu affinity index from file failed!")
		return err
	}

	currentIdx, err := strconv.Atoi(string(data))
	if err != nil {
		a.Logger().Error("Converting from []byte to integer failed!")
		return err
	}

	currentIdx = currentIdx - 1
	err = os.WriteFile(fileName, []byte(strconv.Itoa(currentIdx)), defaultFilePerms)
	if err != nil {
		a.Logger().Error("Storing cpu affinity index from file failed!")
		return err
	}

	pid := a.state.PID

	shutdownSignal := syscall.SIGINT

	if waitOnly {
		// NOP
		shutdownSignal = syscall.Signal(0)
	}

	return utils.WaitLocalProcess(pid, acrnStopSandboxTimeoutSecs, shutdownSignal, a.Logger())
}

func (a *Acrn) updateBlockDevice(drive *config.BlockDrive) error {
	if drive.Swap {
		return fmt.Errorf("Acrn doesn't support swap")
	}

	var err error
	if drive.File == "" || drive.Index >= AcrnBlkDevPoolSz {
		return fmt.Errorf("Empty filepath or invalid drive index, Dive ID:%s, Drive Index:%d",
			drive.ID, drive.Index)
	}

	slot := AcrnBlkdDevSlot[drive.Index]

	//Explicitly set PCIPath to NULL, so that VirtPath can be used
	drive.PCIPath = types.PciPath{}

	args := []string{"blkrescan", a.acrnConfig.Name, fmt.Sprintf("%d,%s", slot, drive.File)}

	a.Logger().WithFields(logrus.Fields{
		"drive": drive,
		"path":  a.config.HypervisorCtlPath,
	}).Info("updateBlockDevice with acrnctl path")
	cmd := exec.Command(a.config.HypervisorCtlPath, args...)
	if err := cmd.Run(); err != nil {
		a.Logger().WithError(err).Error("updating Block device with newFile path")
	}

	return err
}

func (a *Acrn) HotplugAddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	span, _ := katatrace.Trace(ctx, a.Logger(), "HotplugAddDevice", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	switch devType {
	case BlockDev:
		//The drive placeholder has to exist prior to Update
		return nil, a.updateBlockDevice(devInfo.(*config.BlockDrive))
	default:
		return nil, fmt.Errorf("HotplugAddDevice: unsupported device: devInfo:%v, deviceType%v",
			devInfo, devType)
	}
}

func (a *Acrn) HotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	span, _ := katatrace.Trace(ctx, a.Logger(), "HotplugRemoveDevice", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	// Not supported. return success

	return nil, nil
}

func (a *Acrn) PauseVM(ctx context.Context) error {
	span, _ := katatrace.Trace(ctx, a.Logger(), "PauseVM", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	// Not supported. return success

	return nil
}

func (a *Acrn) AttestVM(ctx context.Context) error {
	span, _ := katatrace.Trace(ctx, a.Logger(), "AttestVM", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	a.Logger().Warning("AttestVM: unimplemented")

	return nil
}

func (a *Acrn) ResumeVM(ctx context.Context) error {
	span, _ := katatrace.Trace(ctx, a.Logger(), "ResumeVM", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	// Not supported. return success

	return nil
}

// AddDevice will add extra devices to acrn command line.
func (a *Acrn) AddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) error {
	var err error
	span, _ := katatrace.Trace(ctx, a.Logger(), "AddDevice", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	switch v := devInfo.(type) {
	case types.Volume:
		// Not supported. return success
		err = nil
	case types.Socket:
		a.acrnConfig.Devices = a.arch.appendSocket(a.acrnConfig.Devices, v)
	case types.VSock:
		a.acrnConfig.Devices = a.arch.appendVSock(a.acrnConfig.Devices, v)
	case Endpoint:
		a.acrnConfig.Devices = a.arch.appendNetwork(a.acrnConfig.Devices, v)
	case config.BlockDrive:
		a.acrnConfig.Devices = a.arch.appendBlockDevice(a.acrnConfig.Devices, v)
	case config.VhostUserDeviceAttrs:
		// Not supported. return success
		err = nil
	case config.VFIODev:
		// Not supported. return success
		err = nil
	default:
		err = nil
		a.Logger().WithField("unknown-device-type", devInfo).Error("Adding device")
	}

	return err
}

// GetVMConsole builds the path of the console where we can read logs coming
// from the sandbox.
func (a *Acrn) GetVMConsole(ctx context.Context, id string) (string, string, error) {
	span, _ := katatrace.Trace(ctx, a.Logger(), "GetVMConsole", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	consoleURL, err := utils.BuildSocketPath(a.config.VMStorePath, id, acrnConsoleSocket)

	if err != nil {
		a.Logger().Error("GetVMConsole returned error")
		return consoleProtoUnix, "", err
	}

	return consoleProtoUnix, consoleURL, nil
}

func (a *Acrn) SaveVM() error {
	a.Logger().Info("Save sandbox")

	// Not supported. return success

	return nil
}

func (a *Acrn) Disconnect(ctx context.Context) {
	span, _ := katatrace.Trace(ctx, a.Logger(), "Disconnect", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	// Not supported.
}

func (a *Acrn) GetThreadIDs(ctx context.Context) (VcpuThreadIDs, error) {
	span, _ := katatrace.Trace(ctx, a.Logger(), "GetThreadIDs", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	// Not supported. return success
	//Just allocating an empty map

	return VcpuThreadIDs{}, nil
}

func (a *Acrn) GetTotalMemoryMB(ctx context.Context) uint32 {
	return a.config.MemorySize
}

func (a *Acrn) ResizeMemory(ctx context.Context, reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, MemoryDevice, error) {
	return 0, MemoryDevice{}, nil
}

func (a *Acrn) ResizeVCPUs(ctx context.Context, reqVCPUs uint32) (currentVCPUs uint32, newVCPUs uint32, err error) {
	return 0, 0, nil
}

func (a *Acrn) Cleanup(ctx context.Context) error {
	span, _ := katatrace.Trace(ctx, a.Logger(), "Cleanup", acrnTracingTags, map[string]string{"sandbox_id": a.id})
	defer span.End()

	return nil
}

func (a *Acrn) GetPids() []int {
	return []int{a.state.PID}
}

func (a *Acrn) GetVirtioFsPid() *int {
	return nil
}

func (a *Acrn) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	return errors.New("acrn is not supported by VM cache")
}

func (a *Acrn) toGrpc(ctx context.Context) ([]byte, error) {
	return nil, errors.New("acrn is not supported by VM cache")
}

func (a *Acrn) Save() (s hv.HypervisorState) {
	s.Pid = a.state.PID
	s.Type = string(AcrnHypervisor)
	return
}

func (a *Acrn) Load(s hv.HypervisorState) {
	a.state.PID = s.Pid
}

func (a *Acrn) Check() error {
	if err := syscall.Kill(a.state.PID, syscall.Signal(0)); err != nil {
		return errors.Wrapf(err, "failed to ping acrn process")
	}

	return nil
}

func (a *Acrn) GenerateSocket(id string) (interface{}, error) {
	socket, err := generateVMSocket(id, a.config.VMStorePath)
	if err != nil {
		return "", err
	}
	vsock, _ := socket.(types.VSock)
	vsock.VhostFd.Close()

	return socket, err
}

func (a *Acrn) IsRateLimiterBuiltin() bool {
	return false
}
