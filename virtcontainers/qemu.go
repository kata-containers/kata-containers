// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/kata-containers/runtime/virtcontainers/pkg/uuid"
	"github.com/sirupsen/logrus"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

type qmpChannel struct {
	ctx     context.Context
	path    string
	qmp     *govmmQemu.QMP
	disconn chan struct{}
}

// CPUDevice represents a CPU device which was hot-added in a running VM
type CPUDevice struct {
	// ID is used to identify this CPU in the hypervisor options.
	ID string
}

// QemuState keeps Qemu's state
type QemuState struct {
	Bridges []Bridge
	// HotpluggedCPUs is the list of CPUs that were hot-added
	HotpluggedVCPUs  []CPUDevice
	HotpluggedMemory int
	UUID             string
}

// qemu is an Hypervisor interface implementation for the Linux qemu hypervisor.
type qemu struct {
	id string

	vmConfig Resources

	storage resourceStorage

	config HypervisorConfig

	qmpMonitorCh qmpChannel

	qemuConfig govmmQemu.Config

	state QemuState

	arch qemuArch

	// fds is a list of file descriptors inherited by QEMU process
	// they'll be closed once QEMU process is running
	fds []*os.File
}

const (
	consoleSocket = "console.sock"
	qmpSocket     = "qmp.sock"

	qmpCapErrMsg                      = "Failed to negoatiate QMP capabilities"
	qmpCapMigrationBypassSharedMemory = "bypass-shared-memory"
	qmpExecCatCmd                     = "exec:cat"

	scsiControllerID = "scsi0"
)

var qemuMajorVersion int
var qemuMinorVersion int

// agnostic list of kernel parameters
var defaultKernelParameters = []Param{
	{"panic", "1"},
}

type operation int

const (
	addDevice operation = iota
	removeDevice
)

type qmpLogger struct {
	logger *logrus.Entry
}

func newQMPLogger() qmpLogger {
	return qmpLogger{
		logger: virtLog.WithField("subsystem", "qmp"),
	}
}

func (l qmpLogger) V(level int32) bool {
	if level != 0 {
		return true
	}

	return false
}

func (l qmpLogger) Infof(format string, v ...interface{}) {
	l.logger.Infof(format, v...)
}

func (l qmpLogger) Warningf(format string, v ...interface{}) {
	l.logger.Warnf(format, v...)
}

func (l qmpLogger) Errorf(format string, v ...interface{}) {
	l.logger.Errorf(format, v...)
}

// Logger returns a logrus logger appropriate for logging qemu messages
func (q *qemu) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "qemu")
}

func (q *qemu) kernelParameters() string {
	// get a list of arch kernel parameters
	params := q.arch.kernelParameters(q.config.Debug)

	// use default parameters
	params = append(params, defaultKernelParameters...)

	// set the maximum number of vCPUs
	params = append(params, Param{"nr_cpus", fmt.Sprintf("%d", q.config.DefaultMaxVCPUs)})

	// add the params specified by the provided config. As the kernel
	// honours the last parameter value set and since the config-provided
	// params are added here, they will take priority over the defaults.
	params = append(params, q.config.KernelParams...)

	paramsStr := SerializeParams(params, "=")

	return strings.Join(paramsStr, " ")
}

// Adds all capabilities supported by qemu implementation of hypervisor interface
func (q *qemu) capabilities() capabilities {
	return q.arch.capabilities()
}

// get the QEMU binary path
func (q *qemu) qemuPath() (string, error) {
	p, err := q.config.HypervisorAssetPath()
	if err != nil {
		return "", err
	}

	if p == "" {
		p, err = q.arch.qemuPath()
		if err != nil {
			return "", err
		}
	}

	if _, err = os.Stat(p); os.IsNotExist(err) {
		return "", fmt.Errorf("QEMU path (%s) does not exist", p)
	}

	return p, nil
}

// init intializes the Qemu structure.
func (q *qemu) init(id string, hypervisorConfig *HypervisorConfig, vmConfig Resources, storage resourceStorage) error {
	err := hypervisorConfig.valid()
	if err != nil {
		return err
	}

	q.id = id
	q.storage = storage
	q.vmConfig = vmConfig
	q.config = *hypervisorConfig
	q.arch = newQemuArch(q.config)

	if err = q.storage.fetchHypervisorState(q.id, &q.state); err != nil {
		q.Logger().Debug("Creating bridges")
		q.state.Bridges = q.arch.bridges(q.config.DefaultBridges)

		q.Logger().Debug("Creating UUID")
		q.state.UUID = uuid.Generate().String()

		// The path might already exist, but in case of VM templating,
		// we have to create it since the sandbox has not created it yet.
		if err = os.MkdirAll(filepath.Join(runStoragePath, id), dirMode); err != nil {
			return err
		}

		if err = q.storage.storeHypervisorState(q.id, q.state); err != nil {
			return err
		}
	}

	nested, err := RunningOnVMM(procCPUInfo)
	if err != nil {
		return err
	}

	if !q.config.DisableNestingChecks && nested {
		q.arch.enableNestingChecks()
	} else {
		q.Logger().WithField("inside-vm", fmt.Sprintf("%t", nested)).Debug("Disable nesting environment checks")
		q.arch.disableNestingChecks()
	}

	return nil
}

func (q *qemu) cpuTopology() govmmQemu.SMP {
	return q.arch.cpuTopology(q.config.DefaultVCPUs, q.config.DefaultMaxVCPUs)
}

func (q *qemu) hostMemMB() (uint64, error) {
	hostMemKb, err := getHostMemorySizeKb(procMemInfo)
	if err != nil {
		return 0, fmt.Errorf("Unable to read memory info: %s", err)
	}
	if hostMemKb == 0 {
		return 0, fmt.Errorf("Error host memory size 0")
	}

	return hostMemKb / 1024, nil
}

func (q *qemu) memoryTopology() (govmmQemu.Memory, error) {
	hostMemMb, err := q.hostMemMB()
	if err != nil {
		return govmmQemu.Memory{}, err
	}

	memMb := uint64(q.config.DefaultMemSz)
	if q.vmConfig.Memory > 0 {
		memMb = uint64(q.vmConfig.Memory)
	}

	return q.arch.memoryTopology(memMb, hostMemMb), nil
}

func (q *qemu) qmpSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(RunVMStoragePath, id, qmpSocket)
}

func (q *qemu) getQemuMachine() (govmmQemu.Machine, error) {
	machine, err := q.arch.machine()
	if err != nil {
		return govmmQemu.Machine{}, err
	}

	accelerators := q.config.MachineAccelerators
	if accelerators != "" {
		if !strings.HasPrefix(accelerators, ",") {
			accelerators = fmt.Sprintf(",%s", accelerators)
		}
		machine.Options += accelerators
	}

	return machine, nil
}

func (q *qemu) appendImage(devices []govmmQemu.Device) ([]govmmQemu.Device, error) {
	imagePath, err := q.config.ImageAssetPath()
	if err != nil {
		return nil, err
	}

	if imagePath != "" {
		devices, err = q.arch.appendImage(devices, imagePath)
		if err != nil {
			return nil, err
		}
	}

	return devices, nil
}

func (q *qemu) createQmpSocket() ([]govmmQemu.QMPSocket, error) {
	monitorSockPath, err := q.qmpSocketPath(q.id)
	if err != nil {
		return nil, err
	}

	q.qmpMonitorCh = qmpChannel{
		ctx:  context.Background(),
		path: monitorSockPath,
	}

	return []govmmQemu.QMPSocket{
		{
			Type:   "unix",
			Name:   q.qmpMonitorCh.path,
			Server: true,
			NoWait: true,
		},
	}, nil
}

func (q *qemu) buildDevices(initrdPath string) ([]govmmQemu.Device, *govmmQemu.IOThread, error) {
	var devices []govmmQemu.Device

	console, err := q.getSandboxConsole(q.id)
	if err != nil {
		return nil, nil, err
	}

	// Add bridges before any other devices. This way we make sure that
	// bridge gets the first available PCI address i.e bridgePCIStartAddr
	devices = q.arch.appendBridges(devices, q.state.Bridges)

	devices = q.arch.appendConsole(devices, console)

	if initrdPath == "" {
		devices, err = q.appendImage(devices)
		if err != nil {
			return nil, nil, err
		}
	}

	var ioThread *govmmQemu.IOThread
	if q.config.BlockDeviceDriver == VirtioSCSI {
		devices, ioThread = q.arch.appendSCSIController(devices, q.config.EnableIOThreads)
	}

	return devices, ioThread, nil

}

func (q *qemu) setupTemplate(knobs *govmmQemu.Knobs, memory *govmmQemu.Memory) govmmQemu.Incoming {
	incoming := govmmQemu.Incoming{}

	if q.config.BootToBeTemplate || q.config.BootFromTemplate {
		knobs.FileBackedMem = true
		memory.Path = q.config.MemoryPath

		if q.config.BootToBeTemplate {
			knobs.FileBackedMemShared = true
		}

		if q.config.BootFromTemplate {
			incoming.MigrationType = govmmQemu.MigrationExec
			incoming.Exec = "cat " + q.config.DevicesStatePath
		}
	}

	return incoming
}

// createSandbox is the Hypervisor sandbox creation implementation for govmmQemu.
func (q *qemu) createSandbox() error {
	machine, err := q.getQemuMachine()
	if err != nil {
		return err
	}

	smp := q.cpuTopology()

	memory, err := q.memoryTopology()
	if err != nil {
		return err
	}

	knobs := govmmQemu.Knobs{
		NoUserConfig: true,
		NoDefaults:   true,
		NoGraphic:    true,
		Daemonize:    true,
		MemPrealloc:  q.config.MemPrealloc,
		HugePages:    q.config.HugePages,
		Realtime:     q.config.Realtime,
		Mlock:        q.config.Mlock,
	}

	kernelPath, err := q.config.KernelAssetPath()
	if err != nil {
		return err
	}

	initrdPath, err := q.config.InitrdAssetPath()
	if err != nil {
		return err
	}

	kernel := govmmQemu.Kernel{
		Path:       kernelPath,
		InitrdPath: initrdPath,
		Params:     q.kernelParameters(),
	}

	incoming := q.setupTemplate(&knobs, &memory)

	rtc := govmmQemu.RTC{
		Base:     "utc",
		DriftFix: "slew",
	}

	if q.state.UUID == "" {
		return fmt.Errorf("UUID should not be empty")
	}

	qmpSockets, err := q.createQmpSocket()
	if err != nil {
		return err
	}

	devices, ioThread, err := q.buildDevices(initrdPath)
	if err != nil {
		return err
	}

	cpuModel := q.arch.cpuModel()

	firmwarePath, err := q.config.FirmwareAssetPath()
	if err != nil {
		return err
	}

	qemuPath, err := q.qemuPath()
	if err != nil {
		return err
	}

	qemuConfig := govmmQemu.Config{
		Name:        fmt.Sprintf("sandbox-%s", q.id),
		UUID:        q.state.UUID,
		Path:        qemuPath,
		Ctx:         q.qmpMonitorCh.ctx,
		Machine:     machine,
		SMP:         smp,
		Memory:      memory,
		Devices:     devices,
		CPUModel:    cpuModel,
		Kernel:      kernel,
		RTC:         rtc,
		QMPSockets:  qmpSockets,
		Knobs:       knobs,
		Incoming:    incoming,
		VGA:         "none",
		GlobalParam: "kvm-pit.lost_tick_policy=discard",
		Bios:        firmwarePath,
	}

	if ioThread != nil {
		qemuConfig.IOThreads = []govmmQemu.IOThread{*ioThread}
	}

	q.qemuConfig = qemuConfig

	return nil
}

// startSandbox will start the Sandbox's VM.
func (q *qemu) startSandbox() error {
	if q.config.Debug {
		params := q.arch.kernelParameters(q.config.Debug)
		strParams := SerializeParams(params, "=")
		formatted := strings.Join(strParams, " ")

		// The name of this field matches a similar one generated by
		// the runtime and allows users to identify which parameters
		// are set here, which come from the runtime and which are set
		// by the user.
		q.Logger().WithField("default-kernel-parameters", formatted).Debug()
	}

	defer func() {
		for _, fd := range q.fds {
			if err := fd.Close(); err != nil {
				q.Logger().WithError(err).Error("After launching Qemu")
			}
		}
	}()

	vmPath := filepath.Join(RunVMStoragePath, q.id)
	err := os.MkdirAll(vmPath, dirMode)
	if err != nil {
		return err
	}
	defer func() {
		if err != nil {
			if err := os.RemoveAll(vmPath); err != nil {
				q.Logger().WithError(err).Error("Fail to clean up vm directory")
			}
		}
	}()

	var strErr string
	strErr, err = govmmQemu.LaunchQemu(q.qemuConfig, newQMPLogger())
	if err != nil {
		return fmt.Errorf("%s", strErr)
	}

	return nil
}

// waitSandbox will wait for the Sandbox's VM to be up and running.
func (q *qemu) waitSandbox(timeout int) error {
	if timeout < 0 {
		return fmt.Errorf("Invalid timeout %ds", timeout)
	}

	cfg := govmmQemu.QMPConfig{Logger: newQMPLogger()}

	var qmp *govmmQemu.QMP
	var disconnectCh chan struct{}
	var ver *govmmQemu.QMPVersion
	var err error

	// clear any possible old state before trying to connect again.
	q.qmpShutdown()
	timeStart := time.Now()
	for {
		disconnectCh = make(chan struct{})
		qmp, ver, err = govmmQemu.QMPStart(q.qmpMonitorCh.ctx, q.qmpMonitorCh.path, cfg, disconnectCh)
		if err == nil {
			break
		}

		if int(time.Now().Sub(timeStart).Seconds()) > timeout {
			return fmt.Errorf("Failed to connect to QEMU instance (timeout %ds): %v", timeout, err)
		}

		time.Sleep(time.Duration(50) * time.Millisecond)
	}
	q.qmpMonitorCh.qmp = qmp
	q.qmpMonitorCh.disconn = disconnectCh
	defer q.qmpShutdown()

	qemuMajorVersion = ver.Major
	qemuMinorVersion = ver.Minor

	q.Logger().WithFields(logrus.Fields{
		"qmp-major-version": ver.Major,
		"qmp-minor-version": ver.Minor,
		"qmp-micro-version": ver.Micro,
		"qmp-capabilities":  strings.Join(ver.Capabilities, ","),
	}).Infof("QMP details")

	if err = q.qmpMonitorCh.qmp.ExecuteQMPCapabilities(q.qmpMonitorCh.ctx); err != nil {
		q.Logger().WithError(err).Error(qmpCapErrMsg)
		return err
	}

	return nil
}

// stopSandbox will stop the Sandbox's VM.
func (q *qemu) stopSandbox() error {
	q.Logger().Info("Stopping Sandbox")

	err := q.qmpSetup()
	if err != nil {
		return err
	}

	err = q.qmpMonitorCh.qmp.ExecuteQuit(q.qmpMonitorCh.ctx)
	if err != nil {
		q.Logger().WithError(err).Error("Fail to execute qmp QUIT")
		return err
	}

	err = os.RemoveAll(filepath.Join(RunVMStoragePath, q.id))
	if err != nil {
		q.Logger().WithError(err).Error("Fail to clean up vm directory")
	}

	return nil
}

func (q *qemu) togglePauseSandbox(pause bool) error {
	err := q.qmpSetup()
	if err != nil {
		return err
	}

	if pause {
		err = q.qmpMonitorCh.qmp.ExecuteStop(q.qmpMonitorCh.ctx)
	} else {
		err = q.qmpMonitorCh.qmp.ExecuteCont(q.qmpMonitorCh.ctx)
	}

	if err != nil {
		return err
	}

	return nil
}

func (q *qemu) qmpSetup() error {
	if q.qmpMonitorCh.qmp != nil {
		return nil
	}

	cfg := govmmQemu.QMPConfig{Logger: newQMPLogger()}

	// Auto-closed by QMPStart().
	disconnectCh := make(chan struct{})

	qmp, _, err := govmmQemu.QMPStart(q.qmpMonitorCh.ctx, q.qmpMonitorCh.path, cfg, disconnectCh)
	if err != nil {
		q.Logger().WithError(err).Error("Failed to connect to QEMU instance")
		return err
	}

	err = qmp.ExecuteQMPCapabilities(q.qmpMonitorCh.ctx)
	if err != nil {
		qmp.Shutdown()
		q.Logger().WithError(err).Error(qmpCapErrMsg)
		return err
	}
	q.qmpMonitorCh.qmp = qmp
	q.qmpMonitorCh.disconn = disconnectCh

	return nil
}

func (q *qemu) qmpShutdown() {
	if q.qmpMonitorCh.qmp != nil {
		q.qmpMonitorCh.qmp.Shutdown()
		// wait on disconnected channel to be sure that the qmp channel has
		// been closed cleanly.
		<-q.qmpMonitorCh.disconn
		q.qmpMonitorCh.qmp = nil
		q.qmpMonitorCh.disconn = nil
	}
}

func (q *qemu) addDeviceToBridge(ID string) (string, Bridge, error) {
	var err error
	var addr uint32

	// looking for an empty address in the bridges
	for _, b := range q.state.Bridges {
		addr, err = b.addDevice(ID)
		if err == nil {
			return fmt.Sprintf("%02x", addr), b, nil
		}
	}

	return "", Bridge{}, err
}

func (q *qemu) removeDeviceFromBridge(ID string) error {
	var err error
	for _, b := range q.state.Bridges {
		err = b.removeDevice(ID)
		if err == nil {
			// device was removed correctly
			return nil
		}
	}

	return err
}

func (q *qemu) hotplugBlockDevice(drive *config.BlockDrive, op operation) error {
	err := q.qmpSetup()
	if err != nil {
		return err
	}

	devID := "virtio-" + drive.ID

	if op == addDevice {
		if err := q.qmpMonitorCh.qmp.ExecuteBlockdevAdd(q.qmpMonitorCh.ctx, drive.File, drive.ID); err != nil {
			return err
		}

		if q.config.BlockDeviceDriver == VirtioBlock {
			driver := "virtio-blk-pci"
			addr, bridge, err := q.addDeviceToBridge(drive.ID)
			if err != nil {
				return err
			}

			// PCI address is in the format bridge-addr/device-addr eg. "03/02"
			drive.PCIAddr = fmt.Sprintf("%02x", bridge.Addr) + "/" + addr

			if err = q.qmpMonitorCh.qmp.ExecutePCIDeviceAdd(q.qmpMonitorCh.ctx, drive.ID, devID, driver, addr, bridge.ID); err != nil {
				return err
			}
		} else {
			driver := "scsi-hd"

			// Bus exposed by the SCSI Controller
			bus := scsiControllerID + ".0"

			// Get SCSI-id and LUN based on the order of attaching drives.
			scsiID, lun, err := utils.GetSCSIIdLun(drive.Index)
			if err != nil {
				return err
			}

			if err = q.qmpMonitorCh.qmp.ExecuteSCSIDeviceAdd(q.qmpMonitorCh.ctx, drive.ID, devID, driver, bus, scsiID, lun); err != nil {
				return err
			}
		}
	} else {
		if q.config.BlockDeviceDriver == VirtioBlock {
			if err := q.removeDeviceFromBridge(drive.ID); err != nil {
				return err
			}
		}

		if err := q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, devID); err != nil {
			return err
		}

		if err := q.qmpMonitorCh.qmp.ExecuteBlockdevDel(q.qmpMonitorCh.ctx, drive.ID); err != nil {
			return err
		}
	}

	return nil
}

func (q *qemu) hotplugVFIODevice(device *config.VFIODev, op operation) error {
	err := q.qmpSetup()
	if err != nil {
		return err
	}

	devID := device.ID

	if op == addDevice {
		addr, bridge, err := q.addDeviceToBridge(devID)
		if err != nil {
			return err
		}

		if err := q.qmpMonitorCh.qmp.ExecutePCIVFIODeviceAdd(q.qmpMonitorCh.ctx, devID, device.BDF, addr, bridge.ID); err != nil {
			return err
		}
	} else {
		if err := q.removeDeviceFromBridge(devID); err != nil {
			return err
		}

		if err := q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, devID); err != nil {
			return err
		}
	}

	return nil
}

func (q *qemu) hotplugMacvtap(drive VirtualEndpoint) error {
	var (
		VMFdNames    []string
		VhostFdNames []string
	)
	for i, VMFd := range drive.NetPair.VMFds {
		fdName := fmt.Sprintf("fd%d", i)
		err := q.qmpMonitorCh.qmp.ExecuteGetFD(q.qmpMonitorCh.ctx, fdName, VMFd)
		if err != nil {
			return err
		}
		VMFdNames = append(VMFdNames, fdName)
	}
	for i, VhostFd := range drive.NetPair.VhostFds {
		fdName := fmt.Sprintf("vhostfd%d", i)
		err := q.qmpMonitorCh.qmp.ExecuteGetFD(q.qmpMonitorCh.ctx, fdName, VhostFd)
		if err != nil {
			return err
		}
		VhostFdNames = append(VhostFdNames, fdName)
	}
	return q.qmpMonitorCh.qmp.ExecuteNetdevAddByFds(q.qmpMonitorCh.ctx, "tap", drive.NetPair.Name, VMFdNames, VhostFdNames)
}

func (q *qemu) hotplugNetDevice(drive VirtualEndpoint, op operation) error {
	defer func(qemu *qemu) {
		if q.qmpMonitorCh.qmp != nil {
			q.qmpMonitorCh.qmp.Shutdown()
		}
	}(q)

	err := q.qmpSetup()
	if err != nil {
		return err
	}
	devID := "virtio-" + drive.NetPair.ID

	if op == addDevice {
		switch drive.NetPair.NetInterworkingModel {
		case NetXConnectBridgedModel:
			if err := q.qmpMonitorCh.qmp.ExecuteNetdevAdd(q.qmpMonitorCh.ctx, "tap", drive.NetPair.Name, drive.NetPair.TAPIface.Name, "no", "no", defaultQueues); err != nil {
				return err
			}
		case NetXConnectMacVtapModel:
			if err := q.hotplugMacvtap(drive); err != nil {
				return err
			}
		default:
			return fmt.Errorf("this net interworking model is not supported")
		}
		addr, bridge, err := q.addDeviceToBridge(drive.NetPair.ID)
		if err != nil {
			return err
		}
		drive.PCIAddr = fmt.Sprintf("%02x/%s", bridge.Addr, addr)
		if err = q.qmpMonitorCh.qmp.ExecuteNetPCIDeviceAdd(q.qmpMonitorCh.ctx, drive.NetPair.Name, devID, drive.NetPair.TAPIface.HardAddr, addr, bridge.ID); err != nil {
			return err
		}
	} else {
		if err := q.removeDeviceFromBridge(drive.NetPair.ID); err != nil {
			return err
		}
		if err := q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, devID); err != nil {
			return err
		}
		if err := q.qmpMonitorCh.qmp.ExecuteNetdevDel(q.qmpMonitorCh.ctx, drive.NetPair.Name); err != nil {
			return err
		}
	}
	return nil
}

func (q *qemu) hotplugDevice(devInfo interface{}, devType deviceType, op operation) (interface{}, error) {
	switch devType {
	case blockDev:
		drive := devInfo.(*config.BlockDrive)
		return nil, q.hotplugBlockDevice(drive, op)
	case cpuDev:
		vcpus := devInfo.(uint32)
		return q.hotplugCPUs(vcpus, op)
	case vfioDev:
		device := devInfo.(*config.VFIODev)
		return nil, q.hotplugVFIODevice(device, op)
	case memoryDev:
		memdev := devInfo.(*memoryDevice)
		return nil, q.hotplugMemory(memdev, op)
	case netDev:
		device := devInfo.(VirtualEndpoint)
		return nil, q.hotplugNetDevice(device, op)
	default:
		return nil, fmt.Errorf("cannot hotplug device: unsupported device type '%v'", devType)
	}
}

func (q *qemu) hotplugAddDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	data, err := q.hotplugDevice(devInfo, devType, addDevice)
	if err != nil {
		return data, err
	}

	return data, q.storage.storeHypervisorState(q.id, q.state)
}

func (q *qemu) hotplugRemoveDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	data, err := q.hotplugDevice(devInfo, devType, removeDevice)
	if err != nil {
		return data, err
	}

	return data, q.storage.storeHypervisorState(q.id, q.state)
}

func (q *qemu) hotplugCPUs(vcpus uint32, op operation) (uint32, error) {
	if vcpus == 0 {
		q.Logger().Warnf("cannot hotplug 0 vCPUs")
		return 0, nil
	}

	err := q.qmpSetup()
	if err != nil {
		return 0, err
	}

	if op == addDevice {
		return q.hotplugAddCPUs(vcpus)
	}

	return q.hotplugRemoveCPUs(vcpus)
}

// try to hot add an amount of vCPUs, returns the number of vCPUs added
func (q *qemu) hotplugAddCPUs(amount uint32) (uint32, error) {
	currentVCPUs := q.qemuConfig.SMP.CPUs + uint32(len(q.state.HotpluggedVCPUs))

	// Don't fail if the number of max vCPUs is exceeded, log a warning and hot add the vCPUs needed
	// to reach out max vCPUs
	if currentVCPUs+amount > q.config.DefaultMaxVCPUs {
		q.Logger().Warnf("Cannot hotplug %d CPUs, currently this SB has %d CPUs and the maximum amount of CPUs is %d",
			amount, currentVCPUs, q.config.DefaultMaxVCPUs)
		amount = q.config.DefaultMaxVCPUs - currentVCPUs
	}

	if amount == 0 {
		// Don't fail if no more vCPUs can be added, since cgroups still can be updated
		q.Logger().Warnf("maximum number of vCPUs '%d' has been reached", q.config.DefaultMaxVCPUs)
		return 0, nil
	}

	// get the list of hotpluggable CPUs
	hotpluggableVCPUs, err := q.qmpMonitorCh.qmp.ExecuteQueryHotpluggableCPUs(q.qmpMonitorCh.ctx)
	if err != nil {
		return 0, fmt.Errorf("failed to query hotpluggable CPUs: %v", err)
	}

	var hotpluggedVCPUs uint32
	for _, hc := range hotpluggableVCPUs {
		// qom-path is the path to the CPU, non-empty means that this CPU is already in use
		if hc.QOMPath != "" {
			continue
		}

		// CPU type, i.e host-x86_64-cpu
		driver := hc.Type
		cpuID := fmt.Sprintf("cpu-%d", len(q.state.HotpluggedVCPUs))
		socketID := fmt.Sprintf("%d", hc.Properties.Socket)
		coreID := fmt.Sprintf("%d", hc.Properties.Core)
		threadID := fmt.Sprintf("%d", hc.Properties.Thread)
		if err := q.qmpMonitorCh.qmp.ExecuteCPUDeviceAdd(q.qmpMonitorCh.ctx, driver, cpuID, socketID, coreID, threadID); err != nil {
			// don't fail, let's try with other CPU
			continue
		}

		// a new vCPU was added, update list of hotplugged vCPUs and check if all vCPUs were added
		q.state.HotpluggedVCPUs = append(q.state.HotpluggedVCPUs, CPUDevice{cpuID})
		hotpluggedVCPUs++
		if hotpluggedVCPUs == amount {
			// All vCPUs were hotplugged
			return amount, q.storage.storeHypervisorState(q.id, q.state)
		}
	}

	// All vCPUs were NOT hotplugged
	if err := q.storage.storeHypervisorState(q.id, q.state); err != nil {
		q.Logger().Errorf("failed to save hypervisor state after hotplug %d vCPUs: %v", hotpluggedVCPUs, err)
	}

	return hotpluggedVCPUs, fmt.Errorf("failed to hot add vCPUs: only %d vCPUs of %d were added", hotpluggedVCPUs, amount)
}

// try to  hot remove an amount of vCPUs, returns the number of vCPUs removed
func (q *qemu) hotplugRemoveCPUs(amount uint32) (uint32, error) {
	hotpluggedVCPUs := uint32(len(q.state.HotpluggedVCPUs))

	// we can only remove hotplugged vCPUs
	if amount > hotpluggedVCPUs {
		return 0, fmt.Errorf("Unable to remove %d CPUs, currently there are only %d hotplugged CPUs", amount, hotpluggedVCPUs)
	}

	for i := uint32(0); i < amount; i++ {
		// get the last vCPUs and try to remove it
		cpu := q.state.HotpluggedVCPUs[len(q.state.HotpluggedVCPUs)-1]
		if err := q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, cpu.ID); err != nil {
			_ = q.storage.storeHypervisorState(q.id, q.state)
			return i, fmt.Errorf("failed to hotunplug CPUs, only %d CPUs were hotunplugged: %v", i, err)
		}

		// remove from the list the vCPU hotunplugged
		q.state.HotpluggedVCPUs = q.state.HotpluggedVCPUs[:len(q.state.HotpluggedVCPUs)-1]
	}

	return amount, q.storage.storeHypervisorState(q.id, q.state)
}

func (q *qemu) hotplugMemory(memDev *memoryDevice, op operation) error {
	if memDev.sizeMB < 0 {
		return fmt.Errorf("cannot hotplug negative size (%d) memory", memDev.sizeMB)
	}

	// We do not support memory hot unplug.
	if op == removeDevice {
		return errors.New("cannot hot unplug memory device")
	}

	maxMem, err := q.hostMemMB()
	if err != nil {
		return err
	}

	// calculate current memory
	currentMemory := int(q.config.DefaultMemSz)
	if q.vmConfig.Memory > 0 {
		currentMemory = int(q.vmConfig.Memory)
	}
	currentMemory += q.state.HotpluggedMemory

	// Don't exceed the maximum amount of memory
	if currentMemory+memDev.sizeMB > int(maxMem) {
		return fmt.Errorf("Unable to hotplug %d MiB memory, the SB has %d MiB and the maximum amount is %d MiB",
			memDev.sizeMB, currentMemory, q.config.DefaultMemSz)
	}

	return q.hotplugAddMemory(memDev)
}

func (q *qemu) hotplugAddMemory(memDev *memoryDevice) error {
	err := q.qmpSetup()
	if err != nil {
		return err
	}

	err = q.qmpMonitorCh.qmp.ExecHotplugMemory(q.qmpMonitorCh.ctx, "memory-backend-ram", "mem"+strconv.Itoa(memDev.slot), "", memDev.sizeMB)
	if err != nil {
		q.Logger().WithError(err).Error("hotplug memory")
		return err
	}

	q.state.HotpluggedMemory += memDev.sizeMB
	return q.storage.storeHypervisorState(q.id, q.state)
}

func (q *qemu) pauseSandbox() error {
	return q.togglePauseSandbox(true)
}

func (q *qemu) resumeSandbox() error {
	return q.togglePauseSandbox(false)
}

// addDevice will add extra devices to Qemu command line.
func (q *qemu) addDevice(devInfo interface{}, devType deviceType) error {
	switch v := devInfo.(type) {
	case Volume:
		q.qemuConfig.Devices = q.arch.append9PVolume(q.qemuConfig.Devices, v)
	case Socket:
		q.qemuConfig.Devices = q.arch.appendSocket(q.qemuConfig.Devices, v)
	case kataVSOCK:
		q.fds = append(q.fds, v.vhostFd)
		q.qemuConfig.Devices = q.arch.appendVSockPCI(q.qemuConfig.Devices, v)
	case Endpoint:
		q.qemuConfig.Devices = q.arch.appendNetwork(q.qemuConfig.Devices, v)
	case config.BlockDrive:
		q.qemuConfig.Devices = q.arch.appendBlockDevice(q.qemuConfig.Devices, v)
	case config.VhostUserDeviceAttrs:
		q.qemuConfig.Devices = q.arch.appendVhostUserDevice(q.qemuConfig.Devices, v)
	case config.VFIODev:
		q.qemuConfig.Devices = q.arch.appendVFIODevice(q.qemuConfig.Devices, v)
	default:
		break
	}

	return nil
}

// getSandboxConsole builds the path of the console where we can read
// logs coming from the sandbox.
func (q *qemu) getSandboxConsole(id string) (string, error) {
	return utils.BuildSocketPath(RunVMStoragePath, id, consoleSocket)
}

func (q *qemu) saveSandbox() error {
	q.Logger().Info("save sandbox")

	err := q.qmpSetup()
	if err != nil {
		return err
	}

	// BootToBeTemplate sets the VM to be a template that other VMs can clone from. We would want to
	// bypass shared memory when saving the VM to a local file through migration exec.
	if q.config.BootToBeTemplate {
		err = q.qmpMonitorCh.qmp.ExecSetMigrationCaps(q.qmpMonitorCh.ctx, []map[string]interface{}{
			{
				"capability": qmpCapMigrationBypassSharedMemory,
				"state":      true,
			},
		})
		if err != nil {
			q.Logger().WithError(err).Error("set migration bypass shared memory")
			return err
		}
	}

	err = q.qmpMonitorCh.qmp.ExecSetMigrateArguments(q.qmpMonitorCh.ctx, fmt.Sprintf("%s>%s", qmpExecCatCmd, q.config.DevicesStatePath))
	if err != nil {
		q.Logger().WithError(err).Error("exec migration")
		return err
	}

	return nil
}

func (q *qemu) disconnect() {
	q.qmpShutdown()
}

// genericAppendBridges appends to devices the given bridges
func genericAppendBridges(devices []govmmQemu.Device, bridges []Bridge, machineType string) []govmmQemu.Device {
	bus := defaultPCBridgeBus
	if machineType == QemuQ35 {
		bus = defaultBridgeBus
	}

	for idx, b := range bridges {
		t := govmmQemu.PCIBridge
		if b.Type == pcieBridge {
			t = govmmQemu.PCIEBridge
		}

		bridges[idx].Addr = bridgePCIStartAddr + idx

		devices = append(devices,
			govmmQemu.BridgeDevice{
				Type: t,
				Bus:  bus,
				ID:   b.ID,
				// Each bridge is required to be assigned a unique chassis id > 0
				Chassis: idx + 1,
				SHPC:    true,
				Addr:    strconv.FormatInt(int64(bridges[idx].Addr), 10),
			},
		)
	}

	return devices
}

func genericBridges(number uint32, machineType string) []Bridge {
	var bridges []Bridge
	var bt bridgeType

	switch machineType {

	case QemuQ35:
		// currently only pci bridges are supported
		// qemu-2.10 will introduce pcie bridges
		fallthrough
	case QemuPC:
		bt = pciBridge
	case QemuPseries:
		bt = pciBridge
	default:
		return nil
	}

	for i := uint32(0); i < number; i++ {
		bridges = append(bridges, Bridge{
			Type:    bt,
			ID:      fmt.Sprintf("%s-bridge-%d", bt, i),
			Address: make(map[uint32]string),
		})
	}

	return bridges
}

func genericMemoryTopology(memoryMb, hostMemoryMb uint64) govmmQemu.Memory {
	// NVDIMM device needs memory space 1024MB
	// See https://github.com/clearcontainers/runtime/issues/380
	memoryOffset := 1024

	// add 1G memory space for nvdimm device (vm guest image)
	memMax := fmt.Sprintf("%dM", hostMemoryMb+uint64(memoryOffset))

	mem := fmt.Sprintf("%dM", memoryMb)

	memory := govmmQemu.Memory{
		Size:   mem,
		Slots:  defaultMemSlots,
		MaxMem: memMax,
	}

	return memory
}
