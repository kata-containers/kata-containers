// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"time"
	"unsafe"

	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/label"
	otelTrace "go.opentelemetry.io/otel/trace"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/uuid"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// Since ACRN is using the store in a quite abnormal way, let's first draw it back from store to here

// UUIDPathSuffix is the suffix used for uuid storage
const (
	UUIDPathSuffix = "uuid"
	uuidFile       = "uuid.json"
)

// ACRN currently supports only known UUIDs for security
// reasons (FuSa). When launching VM, only these pre-defined
// UUID should be used else VM launch will fail. The main
// of purpose UUID is is not used for image identification
// but generating vSeed (virtual seed which takes UUID
// as one of the parameter) which is used during VM boot.

// acrnUUIDsToIdx lists Idx corresponding to the UUID
var acrnUUIDsToIdx = map[string]uint8{
	"a7ada506-1ab0-4b6b-a0da-e513ca9b8c2f": 0,
	"dbeae168-26e4-4084-9227-622193e56325": 1,
	"18ed60cd-e9ea-4bf4-8f87-8523fc8347a3": 2,
	"3f90b6f8-449a-4e72-b99c-063a889fc422": 3,
	"1ae8587b-e599-4b59-8260-6d14ac166a55": 4,
	"75f3b94b-49ed-48fc-b019-577ef45adf2b": 5,
	"ca62cf3c-8359-47e8-a3f7-de2d682dfb02": 6,
	"e3189497-c3f6-4b97-9e2c-18ac0ab9064d": 7,
}

// acrnIdxToUUIDs lists UUIDs corresponding to the Idx
var acrnIdxToUUIDs = map[uint8]string{
	0: "a7ada506-1ab0-4b6b-a0da-e513ca9b8c2f",
	1: "dbeae168-26e4-4084-9227-622193e56325",
	2: "18ed60cd-e9ea-4bf4-8f87-8523fc8347a3",
	3: "3f90b6f8-449a-4e72-b99c-063a889fc422",
	4: "1ae8587b-e599-4b59-8260-6d14ac166a55",
	5: "75f3b94b-49ed-48fc-b019-577ef45adf2b",
	6: "ca62cf3c-8359-47e8-a3f7-de2d682dfb02",
	7: "e3189497-c3f6-4b97-9e2c-18ac0ab9064d",
}

// AcrnInfo keeps track of UUID availability
type AcrnInfo struct {
	UUIDAvailability [8]uint8
}

// AcrnState keeps track of VM UUID, PID.
type AcrnState struct {
	UUID string
	PID  int
}

// Acrn is an Hypervisor interface implementation for the Linux acrn hypervisor.
type Acrn struct {
	id         string
	config     HypervisorConfig
	acrnConfig Config
	state      AcrnState
	info       AcrnInfo
	arch       acrnArch
	ctx        context.Context
	store      persistapi.PersistDriver
	sandbox    *Sandbox
}

type acrnPlatformInfo struct {
	cpuNum            uint16     //nolint
	reserved0         [126]uint8 //nolint
	maxVCPUsPerVM     uint16     //nolint
	maxKataContainers uint8
	reserved1         [125]uint8 //nolint
}

const acrnDevice = "/dev/acrn_vhm"

// ioctl_ACRN_CREATE_VM is the IOCTL to create VM in ACRN.
// Current Linux mainstream kernel doesn't have support for ACRN.
// Due to this several macros are not defined in Linux headers.
// Until the support is available, directly use the value instead
// of macros.
//https://github.com/kata-containers/runtime/issues/1784
const ioctl_ACRN_GET_PLATFORM_INFO = 0x43000003 //nolint

const (
	acrnConsoleSocket          = "console.sock"
	acrnStopSandboxTimeoutSecs = 15
)

//UUIDBusy marks a particular UUID as busy
const UUIDBusy = 1

//UUIDFree marks a particular UUID as free
const UUIDFree = 0

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
func (a *Acrn) capabilities(ctx context.Context) types.Capabilities {
	span, _ := a.trace(ctx, "capabilities")
	defer span.End()

	return a.arch.capabilities()
}

func (a *Acrn) hypervisorConfig() HypervisorConfig {
	return a.config
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

func (a *Acrn) trace(parent context.Context, name string) (otelTrace.Span, context.Context) {
	if parent == nil {
		a.Logger().WithField("type", "bug").Error("trace called before context set")
		parent = context.Background()
	}

	tracer := otel.Tracer("kata")
	ctx, span := tracer.Start(parent, name, otelTrace.WithAttributes(label.String("source", "runtime"), label.String("package", "virtcontainers"), label.String("subsystem", "hypervisor"), label.String("type", "acrn"), label.String("sandbox_id", a.id)))

	return span, ctx
}

func (a *Acrn) memoryTopology() (Memory, error) {
	memMb := uint64(a.config.MemorySize)

	return a.arch.memoryTopology(memMb), nil
}

func (a *Acrn) appendImage(devices []Device, imagePath string) ([]Device, error) {
	if imagePath == "" {
		return nil, fmt.Errorf("Image path is empty: %s", imagePath)
	}

	// Get sandbox and increment the globalIndex.
	// This is to make sure the VM rootfs occupies
	// the first Index which is /dev/vda.
	var err error

	if _, err = a.sandbox.GetAndSetSandboxBlockIndex(); err != nil {
		return nil, err
	}

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

	_, console, err := a.getSandboxConsole(ctx, a.id)
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
	span, _ := a.trace(ctx, "setup")
	defer span.End()

	err := hypervisorConfig.valid()
	if err != nil {
		return err
	}

	a.id = id
	a.config = *hypervisorConfig
	a.arch = newAcrnArch(a.config)

	var create bool
	var uuid string

	if a.state.UUID == "" {
		create = true
	}

	if create {
		a.Logger().Debug("Setting UUID")
		if uuid, err = a.GetNextAvailableUUID(); err != nil {
			return err
		}
		a.state.UUID = uuid
		Idx := acrnUUIDsToIdx[uuid]
		a.info.UUIDAvailability[Idx] = UUIDBusy

		// The path might already exist, but in case of VM templating,
		// we have to create it since the sandbox has not created it yet.
		if err = os.MkdirAll(filepath.Join(a.store.RunStoragePath(), id), DirMode); err != nil {
			return err
		}

		if err = a.storeInfo(); err != nil {
			return err
		}
	}

	return nil
}

func (a *Acrn) createDummyVirtioBlkDev(ctx context.Context, devices []Device) ([]Device, error) {
	span, _ := a.trace(ctx, "createDummyVirtioBlkDev")
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

// createSandbox is the Hypervisor sandbox creation.
func (a *Acrn) createSandbox(ctx context.Context, id string, networkNS NetworkNamespace, hypervisorConfig *HypervisorConfig) error {
	// Save the tracing context
	a.ctx = ctx

	var span otelTrace.Span
	span, ctx = a.trace(ctx, "createSandbox")
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

	if a.state.UUID == "" {
		return fmt.Errorf("ACRN UUID should not be empty")
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

	acrnConfig := Config{
		UUID:     a.state.UUID,
		ACPIVirt: true,
		Path:     acrnPath,
		CtlPath:  acrnctlPath,
		Memory:   memory,
		Devices:  devices,
		Kernel:   kernel,
		Name:     fmt.Sprintf("sandbox-%s", a.id),
	}

	a.acrnConfig = acrnConfig

	return nil
}

// startSandbox will start the Sandbox's VM.
func (a *Acrn) startSandbox(ctx context.Context, timeoutSecs int) error {
	span, ctx := a.trace(ctx, "startSandbox")
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

	vmPath := filepath.Join(a.store.RunVMStoragePath(), a.id)
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
	PID, strErr, err = LaunchAcrn(a.acrnConfig, virtLog.WithField("subsystem", "acrn-dm"))
	if err != nil {
		return fmt.Errorf("%s", strErr)
	}
	a.state.PID = PID

	if err = a.waitSandbox(ctx, timeoutSecs); err != nil {
		a.Logger().WithField("acrn wait failed:", err).Debug()
		return err
	}

	return nil
}

// waitSandbox will wait for the Sandbox's VM to be up and running.
func (a *Acrn) waitSandbox(ctx context.Context, timeoutSecs int) error {
	span, _ := a.trace(ctx, "waitSandbox")
	defer span.End()

	if timeoutSecs < 0 {
		return fmt.Errorf("Invalid timeout %ds", timeoutSecs)
	}

	time.Sleep(time.Duration(timeoutSecs) * time.Second)

	return nil
}

// stopSandbox will stop the Sandbox's VM.
func (a *Acrn) stopSandbox(ctx context.Context, waitOnly bool) (err error) {
	span, _ := a.trace(ctx, "stopSandbox")
	defer span.End()

	a.Logger().Info("Stopping acrn VM")

	defer func() {
		if err != nil {
			a.Logger().Info("stopSandbox failed")
		} else {
			a.Logger().Info("acrn VM stopped")
		}
	}()

	// Mark the UUID as free
	uuid := a.state.UUID
	Idx := acrnUUIDsToIdx[uuid]

	if err = a.loadInfo(); err != nil {
		a.Logger().Info("Failed to load UUID availabiity info")
		return err
	}

	a.info.UUIDAvailability[Idx] = UUIDFree

	if err = a.storeInfo(); err != nil {
		a.Logger().Info("Failed to store UUID availabiity info")
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
	var err error
	if drive.File == "" || drive.Index >= AcrnBlkDevPoolSz {
		return fmt.Errorf("Empty filepath or invalid drive index, Dive ID:%s, Drive Index:%d",
			drive.ID, drive.Index)
	}

	slot := AcrnBlkdDevSlot[drive.Index]

	//Explicitly set PCIPath to NULL, so that VirtPath can be used
	drive.PCIPath = vcTypes.PciPath{}

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

func (a *Acrn) hotplugAddDevice(ctx context.Context, devInfo interface{}, devType deviceType) (interface{}, error) {
	span, _ := a.trace(ctx, "hotplugAddDevice")
	defer span.End()

	switch devType {
	case blockDev:
		//The drive placeholder has to exist prior to Update
		return nil, a.updateBlockDevice(devInfo.(*config.BlockDrive))
	default:
		return nil, fmt.Errorf("hotplugAddDevice: unsupported device: devInfo:%v, deviceType%v",
			devInfo, devType)
	}
}

func (a *Acrn) hotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType deviceType) (interface{}, error) {
	span, _ := a.trace(ctx, "hotplugRemoveDevice")
	defer span.End()

	// Not supported. return success

	return nil, nil
}

func (a *Acrn) pauseSandbox(ctx context.Context) error {
	span, _ := a.trace(ctx, "pauseSandbox")
	defer span.End()

	// Not supported. return success

	return nil
}

func (a *Acrn) resumeSandbox(ctx context.Context) error {
	span, _ := a.trace(ctx, "resumeSandbox")
	defer span.End()

	// Not supported. return success

	return nil
}

// addDevice will add extra devices to acrn command line.
func (a *Acrn) addDevice(ctx context.Context, devInfo interface{}, devType deviceType) error {
	var err error
	span, _ := a.trace(ctx, "addDevice")
	defer span.End()

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
func (a *Acrn) getSandboxConsole(ctx context.Context, id string) (string, string, error) {
	span, _ := a.trace(ctx, "getSandboxConsole")
	defer span.End()

	consoleURL, err := utils.BuildSocketPath(a.store.RunVMStoragePath(), id, acrnConsoleSocket)
	if err != nil {
		return consoleProtoUnix, "", err
	}

	return consoleProtoUnix, consoleURL, nil
}

func (a *Acrn) saveSandbox() error {
	a.Logger().Info("save sandbox")

	// Not supported. return success

	return nil
}

func (a *Acrn) disconnect(ctx context.Context) {
	span, _ := a.trace(ctx, "disconnect")
	defer span.End()

	// Not supported.
}

func (a *Acrn) getThreadIDs(ctx context.Context) (vcpuThreadIDs, error) {
	span, _ := a.trace(ctx, "getThreadIDs")
	defer span.End()

	// Not supported. return success
	//Just allocating an empty map

	return vcpuThreadIDs{}, nil
}

func (a *Acrn) resizeMemory(ctx context.Context, reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, memoryDevice, error) {
	return 0, memoryDevice{}, nil
}

func (a *Acrn) resizeVCPUs(ctx context.Context, reqVCPUs uint32) (currentVCPUs uint32, newVCPUs uint32, err error) {
	return 0, 0, nil
}

func (a *Acrn) cleanup(ctx context.Context) error {
	span, _ := a.trace(ctx, "cleanup")
	defer span.End()

	return nil
}

func (a *Acrn) getPids() []int {
	return []int{a.state.PID}
}

func (a *Acrn) getVirtioFsPid() *int {
	return nil
}

func (a *Acrn) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	return errors.New("acrn is not supported by VM cache")
}

func (a *Acrn) toGrpc(ctx context.Context) ([]byte, error) {
	return nil, errors.New("acrn is not supported by VM cache")
}

func (a *Acrn) save() (s persistapi.HypervisorState) {
	s.Pid = a.state.PID
	s.Type = string(AcrnHypervisor)
	s.UUID = a.state.UUID
	return
}

func (a *Acrn) load(s persistapi.HypervisorState) {
	a.state.PID = s.Pid
	a.state.UUID = s.UUID
}

func (a *Acrn) check() error {
	if err := syscall.Kill(a.state.PID, syscall.Signal(0)); err != nil {
		return errors.Wrapf(err, "failed to ping acrn process")
	}

	return nil
}

func (a *Acrn) generateSocket(id string) (interface{}, error) {
	return generateVMSocket(id, a.store.RunVMStoragePath())
}

// GetACRNUUIDBytes returns UUID bytes that is used for VM creation
func (a *Acrn) GetACRNUUIDBytes(uid string) (uuid.UUID, error) {
	return uuid.Parse(uid)
}

// GetNextAvailableUUID returns next available UUID VM creation
// If no valid UUIDs are available it returns err.
func (a *Acrn) GetNextAvailableUUID() (string, error) {
	var MaxVMSupported uint8
	var Idx uint8
	var uuidStr string
	var err error

	if err = a.loadInfo(); err != nil {
		a.Logger().Infof("Load UUID store failed")
	}

	if MaxVMSupported, err = a.GetMaxSupportedACRNVM(); err != nil {
		return "", fmt.Errorf("IOCTL GetMaxSupportedACRNVM failed")
	}

	for Idx = 0; Idx < MaxVMSupported; Idx++ {
		if a.info.UUIDAvailability[Idx] == UUIDFree {
			uuidStr = acrnIdxToUUIDs[Idx]
			break
		}
	}

	if uuidStr == "" {
		return "", fmt.Errorf("Invalid UUID: Max VMs reached")
	}

	return uuidStr, nil
}

// GetMaxSupportedACRNVM checks the max number of VMs that can be
// launched from kata-runtime.
func (a *Acrn) GetMaxSupportedACRNVM() (uint8, error) {
	flags := syscall.O_RDWR | syscall.O_CLOEXEC

	f, err := syscall.Open(acrnDevice, flags, 0)
	if err != nil {
		return 0, err
	}
	defer syscall.Close(f)

	var platformInfo acrnPlatformInfo

	ret, _, errno := syscall.Syscall(syscall.SYS_IOCTL,
		uintptr(f),
		uintptr(ioctl_ACRN_GET_PLATFORM_INFO),
		uintptr(unsafe.Pointer(&platformInfo)))
	if ret != 0 || errno != 0 {
		return 0, errno
	}

	return platformInfo.maxKataContainers, nil
}

func (a *Acrn) storeInfo() error {
	relPath := filepath.Join(UUIDPathSuffix, uuidFile)

	jsonOut, err := json.Marshal(a.info)
	if err != nil {
		return fmt.Errorf("Could not marshal data: %s", err)
	}

	if err := a.store.GlobalWrite(relPath, jsonOut); err != nil {
		return fmt.Errorf("failed to write uuid to file: %v", err)
	}

	return nil
}

func (a *Acrn) loadInfo() error {
	relPath := filepath.Join(UUIDPathSuffix, uuidFile)

	data, err := a.store.GlobalRead(relPath)
	if err != nil {
		return fmt.Errorf("failed to read uuid from file: %v", err)
	}

	if err := json.Unmarshal(data, &a.info); err != nil {
		return fmt.Errorf("failed to unmarshal uuid info: %v", err)
	}
	return nil
}

func (a *Acrn) isRateLimiterBuiltin() bool {
	return false
}

func (a *Acrn) setSandbox(sandbox *Sandbox) {
	a.sandbox = sandbox
}
