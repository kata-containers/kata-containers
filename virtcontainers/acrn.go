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
	"strings"
	"syscall"
	"time"

	opentracing "github.com/opentracing/opentracing-go"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
	persistapi "github.com/kata-containers/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/runtime/virtcontainers/store"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

// AcrnState keeps Acrn's state
type AcrnState struct {
	UUID string
}

// AcrnInfo keeps PID of the hypervisor
type AcrnInfo struct {
	PID int
}

// acrn is an Hypervisor interface implementation for the Linux acrn hypervisor.
type acrn struct {
	id string

	store      *store.VCStore
	config     HypervisorConfig
	acrnConfig Config
	state      AcrnState
	info       AcrnInfo
	arch       acrnArch
	ctx        context.Context
}

const (
	acrnConsoleSocket          = "console.sock"
	acrnStopSandboxTimeoutSecs = 15
)

// agnostic list of kernel parameters
var acrnDefaultKernelParameters = []Param{
	{"panic", "1"},
}

func (a *acrn) kernelParameters() string {
	// get a list of arch kernel parameters
	params := a.arch.kernelParameters(a.config.Debug)

	// use default parameters
	params = append(params, acrnDefaultKernelParameters...)

	// set the maximum number of vCPUs
	params = append(params, Param{"maxcpus", fmt.Sprintf("%d", a.config.NumVCPUs)})

	// add the params specified by the provided config. As the kernel
	// honours the last parameter value set and since the config-provided
	// params are added here, they will take priority over the defaults.
	params = append(params, a.config.KernelParams...)

	paramsStr := SerializeParams(params, "=")

	return strings.Join(paramsStr, " ")
}

// Adds all capabilities supported by acrn implementation of hypervisor interface
func (a *acrn) capabilities() types.Capabilities {
	span, _ := a.trace("capabilities")
	defer span.Finish()

	return a.arch.capabilities()
}

func (a *acrn) hypervisorConfig() HypervisorConfig {
	return a.config
}

// get the acrn binary path
func (a *acrn) acrnPath() (string, error) {
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
func (a *acrn) acrnctlPath() (string, error) {
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
func (a *acrn) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "acrn")
}

func (a *acrn) trace(name string) (opentracing.Span, context.Context) {
	if a.ctx == nil {
		a.Logger().WithField("type", "bug").Error("trace called before context set")
		a.ctx = context.Background()
	}

	span, ctx := opentracing.StartSpanFromContext(a.ctx, name)

	span.SetTag("subsystem", "hypervisor")
	span.SetTag("type", "acrn")

	return span, ctx
}

func (a *acrn) memoryTopology() (Memory, error) {
	memMb := uint64(a.config.MemorySize)

	return a.arch.memoryTopology(memMb), nil
}

func (a *acrn) appendImage(devices []Device, imagePath string) ([]Device, error) {
	if imagePath == "" {
		return nil, fmt.Errorf("Image path is empty: %s", imagePath)
	}

	// Get sandbox and increment the globalIndex.
	// This is to make sure the VM rootfs occupies
	// the first Index which is /dev/vda.
	sandbox, err := globalSandboxList.lookupSandbox(a.id)
	if sandbox == nil && err != nil {
		return nil, err
	}
	sandbox.GetAndSetSandboxBlockIndex()

	devices, err = a.arch.appendImage(devices, imagePath)
	if err != nil {
		return nil, err
	}

	return devices, nil
}

func (a *acrn) buildDevices(imagePath string) ([]Device, error) {
	var devices []Device

	if imagePath == "" {
		return nil, fmt.Errorf("Image Path should not be empty: %s", imagePath)
	}

	console, err := a.getSandboxConsole(a.id)
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
	devices, err = a.createDummyVirtioBlkDev(devices)
	if err != nil {
		return nil, err
	}

	return devices, nil
}

// setup sets the Acrn structure up.
func (a *acrn) setup(id string, hypervisorConfig *HypervisorConfig, vcStore *store.VCStore) error {
	span, _ := a.trace("setup")
	defer span.Finish()

	err := hypervisorConfig.valid()
	if err != nil {
		return err
	}

	a.id = id
	a.store = vcStore
	a.config = *hypervisorConfig
	a.arch = newAcrnArch(a.config)

	var create bool

	if a.store != nil { //use old store
		if err = a.store.Load(store.Hypervisor, &a.info); err != nil {
			create = true
		}
	} else if a.info.PID == 0 { // new store
		create = true
	}

	if create {
		// acrn currently supports only known UUIDs for security
		// reasons (FuSa). When launching VM, only these pre-defined
		// UUID should be used else VM launch will fail. acrn team is
		// working on a solution to expose these pre-defined UUIDs
		// to Kata in order for it to launch VMs successfully.
		// Until this support is available, Kata is limited to
		// launch 1 VM (using the default UUID).
		// https://github.com/kata-containers/runtime/issues/1785

		// The path might already exist, but in case of VM templating,
		// we have to create it since the sandbox has not created it yet.
		if err = os.MkdirAll(store.SandboxRuntimeRootPath(id), store.DirMode); err != nil {
			return err
		}

		if err = a.storeInfo(); err != nil {
			return err
		}
	}

	return nil
}

func (a *acrn) createDummyVirtioBlkDev(devices []Device) ([]Device, error) {
	span, _ := a.trace("createDummyVirtioBlkDev")
	defer span.Finish()

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

// createSandbox is the Hypervisor sandbox creation.
func (a *acrn) createSandbox(ctx context.Context, id string, networkNS NetworkNamespace, hypervisorConfig *HypervisorConfig, store *store.VCStore) error {
	// Save the tracing context
	a.ctx = ctx

	span, _ := a.trace("createSandbox")
	defer span.Finish()

	if err := a.setup(id, hypervisorConfig, store); err != nil {
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

	// Disabling UUID check until the below is fixed.
	// https://github.com/kata-containers/runtime/issues/1785

	devices, err := a.buildDevices(imagePath)
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

	acrnConfig := Config{
		ACPIVirt: true,
		Path:     acrnPath,
		CtlPath:  acrnctlPath,
		Memory:   memory,
		NumCPU:   a.config.NumVCPUs,
		Devices:  devices,
		Kernel:   kernel,
		Name:     fmt.Sprintf("sandbox-%s", a.id),
	}

	a.acrnConfig = acrnConfig

	return nil
}

// startSandbox will start the Sandbox's VM.
func (a *acrn) startSandbox(timeoutSecs int) error {
	span, _ := a.trace("startSandbox")
	defer span.Finish()

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

	vmPath := filepath.Join(store.RunVMStoragePath, a.id)
	err := os.MkdirAll(vmPath, store.DirMode)
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
	PID, strErr, err = LaunchAcrn(a.acrnConfig, virtLog.WithField("subsystem", "acrn-dm"))
	if err != nil {
		return fmt.Errorf("%s", strErr)
	}
	a.info.PID = PID

	if err = a.waitSandbox(timeoutSecs); err != nil {
		a.Logger().WithField("acrn wait failed:", err).Debug()
		return err
	}

	//Store VMM information
	return a.store.Store(store.Hypervisor, a.info)

}

// waitSandbox will wait for the Sandbox's VM to be up and running.
func (a *acrn) waitSandbox(timeoutSecs int) error {
	span, _ := a.trace("waitSandbox")
	defer span.Finish()

	if timeoutSecs < 0 {
		return fmt.Errorf("Invalid timeout %ds", timeoutSecs)
	}

	time.Sleep(time.Duration(timeoutSecs) * time.Second)

	return nil
}

// stopSandbox will stop the Sandbox's VM.
func (a *acrn) stopSandbox() (err error) {
	span, _ := a.trace("stopSandbox")
	defer span.Finish()

	a.Logger().Info("Stopping acrn VM")

	defer func() {
		if err != nil {
			a.Logger().Info("stopSandbox failed")
		} else {
			a.Logger().Info("acrn VM stopped")
		}
	}()

	pid := a.info.PID

	// Check if VM process is running, in case it is not, let's
	// return from here.
	if err = syscall.Kill(pid, syscall.Signal(0)); err != nil {
		a.Logger().Info("acrn VM already stopped")
		return nil
	}

	// Send signal to the VM process to try to stop it properly
	if err = syscall.Kill(pid, syscall.SIGINT); err != nil {
		a.Logger().Info("Sending signal to stop acrn VM failed")
		return err
	}

	// Wait for the VM process to terminate
	tInit := time.Now()
	for {
		if err = syscall.Kill(pid, syscall.Signal(0)); err != nil {
			a.Logger().Info("acrn VM stopped after sending signal")
			return nil
		}

		if time.Since(tInit).Seconds() >= acrnStopSandboxTimeoutSecs {
			a.Logger().Warnf("VM still running after waiting %ds", acrnStopSandboxTimeoutSecs)
			break
		}

		// Let's avoid to run a too busy loop
		time.Sleep(time.Duration(50) * time.Millisecond)
	}

	// Let's try with a hammer now, a SIGKILL should get rid of the
	// VM process.
	return syscall.Kill(pid, syscall.SIGKILL)

}

func (a *acrn) updateBlockDevice(drive *config.BlockDrive) error {
	var err error
	if drive.File == "" || drive.Index >= AcrnBlkDevPoolSz {
		return fmt.Errorf("Empty filepath or invalid drive index, Dive ID:%s, Drive Index:%d",
			drive.ID, drive.Index)
	}

	slot := AcrnBlkdDevSlot[drive.Index]

	//Explicitly set PCIAddr to NULL, so that VirtPath can be used
	drive.PCIAddr = ""

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

func (a *acrn) hotplugAddDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	span, _ := a.trace("hotplugAddDevice")
	defer span.Finish()

	switch devType {
	case blockDev:
		//The drive placeholder has to exist prior to Update
		return nil, a.updateBlockDevice(devInfo.(*config.BlockDrive))
	default:
		return nil, fmt.Errorf("hotplugAddDevice: unsupported device: devInfo:%v, deviceType%v",
			devInfo, devType)
	}
}

func (a *acrn) hotplugRemoveDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	span, _ := a.trace("hotplugRemoveDevice")
	defer span.Finish()

	// Not supported. return success

	return nil, nil
}

func (a *acrn) pauseSandbox() error {
	span, _ := a.trace("pauseSandbox")
	defer span.Finish()

	// Not supported. return success

	return nil
}

func (a *acrn) resumeSandbox() error {
	span, _ := a.trace("resumeSandbox")
	defer span.Finish()

	// Not supported. return success

	return nil
}

// addDevice will add extra devices to Acrn command line.
func (a *acrn) addDevice(devInfo interface{}, devType deviceType) error {
	var err error
	span, _ := a.trace("addDevice")
	defer span.Finish()

	switch v := devInfo.(type) {
	case types.Volume:
		// Not supported. return success
		err = nil
	case types.Socket:
		a.acrnConfig.Devices = a.arch.appendSocket(a.acrnConfig.Devices, v)
	case types.VSock:
		// Not supported. return success
		err = nil
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

// getSandboxConsole builds the path of the console where we can read
// logs coming from the sandbox.
func (a *acrn) getSandboxConsole(id string) (string, error) {
	span, _ := a.trace("getSandboxConsole")
	defer span.Finish()

	return utils.BuildSocketPath(store.RunVMStoragePath, id, acrnConsoleSocket)
}

func (a *acrn) saveSandbox() error {
	a.Logger().Info("save sandbox")

	// Not supported. return success

	return nil
}

func (a *acrn) disconnect() {
	span, _ := a.trace("disconnect")
	defer span.Finish()

	// Not supported.
}

func (a *acrn) getThreadIDs() (vcpuThreadIDs, error) {
	span, _ := a.trace("getThreadIDs")
	defer span.Finish()

	// Not supported. return success
	//Just allocating an empty map

	return vcpuThreadIDs{}, nil
}

func (a *acrn) resizeMemory(reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, memoryDevice, error) {
	return 0, memoryDevice{}, nil
}

func (a *acrn) resizeVCPUs(reqVCPUs uint32) (currentVCPUs uint32, newVCPUs uint32, err error) {
	return 0, 0, nil
}

func (a *acrn) cleanup() error {
	span, _ := a.trace("cleanup")
	defer span.Finish()

	return nil
}

func (a *acrn) getPids() []int {
	return []int{a.info.PID}
}

func (a *acrn) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, store *store.VCStore, j []byte) error {
	return errors.New("acrn is not supported by VM cache")
}

func (a *acrn) toGrpc() ([]byte, error) {
	return nil, errors.New("acrn is not supported by VM cache")
}

func (a *acrn) storeInfo() error {
	if a.store != nil {
		if err := a.store.Store(store.Hypervisor, a.info); err != nil {
			return err
		}
	}
	return nil
}

func (a *acrn) save() (s persistapi.HypervisorState) {
	s.Pid = a.info.PID
	s.Type = string(AcrnHypervisor)
	s.UUID = a.state.UUID
	return
}

func (a *acrn) load(s persistapi.HypervisorState) {
	a.info.PID = s.Pid
	a.state.UUID = s.UUID
}

func (a *acrn) check() error {
	if err := syscall.Kill(a.info.PID, syscall.Signal(0)); err != nil {
		return errors.Wrapf(err, "failed to ping acrn process")
	}

	return nil
}
