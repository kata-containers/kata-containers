// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"context"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"
	"unsafe"

	govmmQemu "github.com/kata-containers/govmm/qemu"
	"github.com/opencontainers/selinux/go-selinux/label"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	"go.opentelemetry.io/otel"
	otelLabel "go.opentelemetry.io/otel/label"
	otelTrace "go.opentelemetry.io/otel/trace"
	"golang.org/x/sys/unix"

	pkgUtils "github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/uuid"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// romFile is the file name of the ROM that can be used for virtio-pci devices.
// If this file name is empty, this means we expect the firmware used by Qemu,
// such as SeaBIOS or OVMF for instance, to handle this directly.
const romFile = ""

// disable-modern is a option to QEMU that will fall back to using 0.9 version
// of virtio. Since moving to QEMU4.0, we can start using virtio 1.0 version.
// Default value is false.
const defaultDisableModern = false

type qmpChannel struct {
	sync.Mutex
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
	Bridges []types.Bridge
	// HotpluggedCPUs is the list of CPUs that were hot-added
	HotpluggedVCPUs      []CPUDevice
	HotpluggedMemory     int
	UUID                 string
	HotplugVFIOOnRootBus bool
	VirtiofsdPid         int
	PCIeRootPort         int
}

// qemu is an Hypervisor interface implementation for the Linux qemu hypervisor.
type qemu struct {
	id string

	config HypervisorConfig

	qmpMonitorCh qmpChannel

	qemuConfig govmmQemu.Config

	state QemuState

	arch qemuArch

	// fds is a list of file descriptors inherited by QEMU process
	// they'll be closed once QEMU process is running
	fds []*os.File

	ctx context.Context

	nvdimmCount int

	stopped bool

	store persistapi.PersistDriver

	// if in memory dump progress
	memoryDumpFlag sync.Mutex

	virtiofsd Virtiofsd
}

const (
	consoleSocket = "console.sock"
	qmpSocket     = "qmp.sock"
	vhostFSSocket = "vhost-fs.sock"

	// memory dump format will be set to elf
	memoryDumpFormat = "elf"

	qmpCapErrMsg  = "Failed to negotiate QMP capabilities"
	qmpExecCatCmd = "exec:cat"

	scsiControllerID         = "scsi0"
	rngID                    = "rng0"
	fallbackFileBackedMemDir = "/dev/shm"

	qemuStopSandboxTimeoutSecs = 15
)

// agnostic list of kernel parameters
var defaultKernelParameters = []Param{
	{"panic", "1"},
}

type qmpLogger struct {
	logger *logrus.Entry
}

func newQMPLogger() qmpLogger {
	return qmpLogger{
		logger: virtLog.WithField("subsystem", "qmp"),
	}
}

func (l qmpLogger) V(level int32) bool {
	return level != 0
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
func (q *qemu) capabilities(ctx context.Context) types.Capabilities {
	span, _ := q.trace(ctx, "capabilities")
	defer span.End()

	return q.arch.capabilities()
}

func (q *qemu) hypervisorConfig() HypervisorConfig {
	return q.config
}

// get the QEMU binary path
func (q *qemu) qemuPath() (string, error) {
	p, err := q.config.HypervisorAssetPath()
	if err != nil {
		return "", err
	}

	if p == "" {
		p = q.arch.qemuPath()
	}

	if _, err = os.Stat(p); os.IsNotExist(err) {
		return "", fmt.Errorf("QEMU path (%s) does not exist", p)
	}

	return p, nil
}

func (q *qemu) trace(parent context.Context, name string) (otelTrace.Span, context.Context) {
	if parent == nil {
		q.Logger().WithField("type", "bug").Error("trace called before context set")
		parent = context.Background()
	}

	tracer := otel.Tracer("kata")
	ctx, span := tracer.Start(parent, name, otelTrace.WithAttributes(otelLabel.String("source", "runtime"), otelLabel.String("package", "virtcontainers"), otelLabel.String("subsystem", "hypervisor"), otelLabel.String("type", "qemu"), otelLabel.String("sandbox_id", q.id)))

	return span, ctx
}

// setup sets the Qemu structure up.
func (q *qemu) setup(ctx context.Context, id string, hypervisorConfig *HypervisorConfig) error {
	span, _ := q.trace(ctx, "setup")
	defer span.End()

	err := hypervisorConfig.valid()
	if err != nil {
		return err
	}

	q.id = id
	q.config = *hypervisorConfig
	q.arch, err = newQemuArch(q.config)
	if err != nil {
		return err
	}

	initrdPath, err := q.config.InitrdAssetPath()
	if err != nil {
		return err
	}
	imagePath, err := q.config.ImageAssetPath()
	if err != nil {
		return err
	}
	if initrdPath == "" && imagePath != "" && !q.config.DisableImageNvdimm {
		q.nvdimmCount = 1
	} else {
		q.nvdimmCount = 0
	}

	var create bool
	if q.state.UUID == "" {
		create = true
	}

	q.arch.setBridges(q.state.Bridges)
	q.arch.setPFlash(q.config.PFlash)

	if create {
		q.Logger().Debug("Creating bridges")
		q.arch.bridges(q.config.DefaultBridges)

		q.Logger().Debug("Creating UUID")
		q.state.UUID = uuid.Generate().String()

		q.state.HotplugVFIOOnRootBus = q.config.HotplugVFIOOnRootBus
		q.state.PCIeRootPort = int(q.config.PCIeRootPort)

		// The path might already exist, but in case of VM templating,
		// we have to create it since the sandbox has not created it yet.
		if err = os.MkdirAll(filepath.Join(q.store.RunStoragePath(), id), DirMode); err != nil {
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

	if !q.config.DisableVhostNet {
		q.arch.enableVhostNet()
	} else {
		q.Logger().Debug("Disable vhost_net")
		q.arch.disableVhostNet()
	}

	return nil
}

func (q *qemu) cpuTopology() govmmQemu.SMP {
	return q.arch.cpuTopology(q.config.NumVCPUs, q.config.DefaultMaxVCPUs)
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

	memMb := uint64(q.config.MemorySize)

	return q.arch.memoryTopology(memMb, hostMemMb, uint8(q.config.MemSlots)), nil
}

func (q *qemu) qmpSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(q.store.RunVMStoragePath(), id, qmpSocket)
}

func (q *qemu) getQemuMachine() (govmmQemu.Machine, error) {
	machine := q.arch.machine()

	accelerators := q.config.MachineAccelerators
	if accelerators != "" {
		if !strings.HasPrefix(accelerators, ",") {
			accelerators = fmt.Sprintf(",%s", accelerators)
		}
		machine.Options += accelerators
	}

	return machine, nil
}

func (q *qemu) appendImage(ctx context.Context, devices []govmmQemu.Device) ([]govmmQemu.Device, error) {
	imagePath, err := q.config.ImageAssetPath()
	if err != nil {
		return nil, err
	}

	if imagePath != "" {
		devices, err = q.arch.appendImage(ctx, devices, imagePath)
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
		ctx:  q.ctx,
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

func (q *qemu) buildDevices(ctx context.Context, initrdPath string) ([]govmmQemu.Device, *govmmQemu.IOThread, error) {
	var devices []govmmQemu.Device

	_, console, err := q.getSandboxConsole(ctx, q.id)
	if err != nil {
		return nil, nil, err
	}

	// Add bridges before any other devices. This way we make sure that
	// bridge gets the first available PCI address i.e bridgePCIStartAddr
	devices = q.arch.appendBridges(devices)

	devices, err = q.arch.appendConsole(ctx, devices, console)
	if err != nil {
		return nil, nil, err
	}

	if initrdPath == "" {
		devices, err = q.appendImage(ctx, devices)
		if err != nil {
			return nil, nil, err
		}
	}

	if q.config.IOMMU {
		devices, err = q.arch.appendIOMMU(devices)
		if err != nil {
			return nil, nil, err
		}
	}

	if q.config.IfPVPanicEnabled() {
		// there should have no errors for pvpanic device
		devices, _ = q.arch.appendPVPanicDevice(devices)
	}

	var ioThread *govmmQemu.IOThread
	if q.config.BlockDeviceDriver == config.VirtioSCSI {
		return q.arch.appendSCSIController(ctx, devices, q.config.EnableIOThreads)
	}

	return devices, ioThread, nil
}

func (q *qemu) setupTemplate(knobs *govmmQemu.Knobs, memory *govmmQemu.Memory) govmmQemu.Incoming {
	incoming := govmmQemu.Incoming{}

	if q.config.BootToBeTemplate || q.config.BootFromTemplate {
		knobs.FileBackedMem = true
		memory.Path = q.config.MemoryPath

		if q.config.BootToBeTemplate {
			knobs.MemShared = true
		}

		if q.config.BootFromTemplate {
			incoming.MigrationType = govmmQemu.MigrationDefer
		}
	}

	return incoming
}

func (q *qemu) setupFileBackedMem(knobs *govmmQemu.Knobs, memory *govmmQemu.Memory) {
	var target string
	if q.config.FileBackedMemRootDir != "" {
		target = q.config.FileBackedMemRootDir
	} else {
		target = fallbackFileBackedMemDir
	}
	if _, err := os.Stat(target); err != nil {
		q.Logger().WithError(err).Error("File backed memory location does not exist")
		return
	}

	knobs.FileBackedMem = true
	knobs.MemShared = true
	memory.Path = target
}

// createSandbox is the Hypervisor sandbox creation implementation for govmmQemu.
func (q *qemu) createSandbox(ctx context.Context, id string, networkNS NetworkNamespace, hypervisorConfig *HypervisorConfig) error {
	// Save the tracing context
	q.ctx = ctx

	span, ctx := q.trace(ctx, "createSandbox")
	defer span.End()

	if err := q.setup(ctx, id, hypervisorConfig); err != nil {
		return err
	}

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
		NoUserConfig:  true,
		NoDefaults:    true,
		NoGraphic:     true,
		NoReboot:      true,
		Daemonize:     true,
		MemPrealloc:   q.config.MemPrealloc,
		HugePages:     q.config.HugePages,
		Realtime:      q.config.Realtime,
		Mlock:         q.config.Mlock,
		IOMMUPlatform: q.config.IOMMUPlatform,
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

	// With the current implementations, VM templating will not work with file
	// based memory (stand-alone) or virtiofs. This is because VM templating
	// builds the first VM with file-backed memory and shared=on and the
	// subsequent ones with shared=off. virtio-fs always requires shared=on for
	// memory.
	if q.config.SharedFS == config.VirtioFS || q.config.FileBackedMemRootDir != "" {
		if !(q.config.BootToBeTemplate || q.config.BootFromTemplate) {
			q.setupFileBackedMem(&knobs, &memory)
		} else {
			return errors.New("VM templating has been enabled with either virtio-fs or file backed memory and this configuration will not work")
		}
		if q.config.HugePages {
			knobs.MemPrealloc = true
		}
	}

	// Vhost-user-blk/scsi process which can improve performance, like SPDK,
	// requires shared-on hugepage to work with Qemu.
	if q.config.EnableVhostUserStore {
		if !q.config.HugePages {
			return errors.New("Vhost-user-blk/scsi is enabled without HugePages. This configuration will not work")
		}
		knobs.MemShared = true
	}

	rtc := govmmQemu.RTC{
		Base:     govmmQemu.UTC,
		Clock:    govmmQemu.Host,
		DriftFix: govmmQemu.Slew,
	}

	if q.state.UUID == "" {
		return fmt.Errorf("UUID should not be empty")
	}

	qmpSockets, err := q.createQmpSocket()
	if err != nil {
		return err
	}

	devices, ioThread, err := q.buildDevices(ctx, initrdPath)
	if err != nil {
		return err
	}

	cpuModel := q.arch.cpuModel()
	cpuModel += "," + q.config.CPUFeatures

	firmwarePath, err := q.config.FirmwareAssetPath()
	if err != nil {
		return err
	}

	pflash, err := q.arch.getPFlash()
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
		PFlash:      pflash,
		PidFile:     filepath.Join(q.store.RunVMStoragePath(), q.id, "pid"),
	}

	qemuConfig.Devices, qemuConfig.Bios, err = q.arch.appendProtectionDevice(qemuConfig.Devices, firmwarePath)
	if err != nil {
		return err
	}

	if ioThread != nil {
		qemuConfig.IOThreads = []govmmQemu.IOThread{*ioThread}
	}
	// Add RNG device to hypervisor
	rngDev := config.RNGDev{
		ID:       rngID,
		Filename: q.config.EntropySource,
	}
	qemuConfig.Devices, err = q.arch.appendRNGDevice(ctx, qemuConfig.Devices, rngDev)
	if err != nil {
		return err
	}

	// Add PCIe Root Port devices to hypervisor
	// The pcie.0 bus do not support hot-plug, but PCIe device can be hot-plugged into PCIe Root Port.
	// For more details, please see https://github.com/qemu/qemu/blob/master/docs/pcie.txt
	if hypervisorConfig.PCIeRootPort > 0 {
		qemuConfig.Devices = q.arch.appendPCIeRootPortDevice(qemuConfig.Devices, hypervisorConfig.PCIeRootPort)
	}

	q.qemuConfig = qemuConfig

	virtiofsdSocketPath, err := q.vhostFSSocketPath(q.id)
	if err != nil {
		return err
	}

	q.virtiofsd = &virtiofsd{
		path:       q.config.VirtioFSDaemon,
		sourcePath: filepath.Join(getSharePath(q.id)),
		socketPath: virtiofsdSocketPath,
		extraArgs:  q.config.VirtioFSExtraArgs,
		debug:      q.config.Debug,
		cache:      q.config.VirtioFSCache,
	}

	return nil
}

func (q *qemu) vhostFSSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(q.store.RunVMStoragePath(), id, vhostFSSocket)
}

func (q *qemu) setupVirtiofsd(ctx context.Context) (err error) {
	pid, err := q.virtiofsd.Start(ctx, func() {
		q.stopSandbox(ctx, false)
	})
	if err != nil {
		return err
	}
	q.state.VirtiofsdPid = pid

	return nil
}

func (q *qemu) stopVirtiofsd(ctx context.Context) (err error) {
	if q.state.VirtiofsdPid == 0 {
		return errors.New("invalid virtiofsd PID(0)")
	}

	err = q.virtiofsd.Stop(ctx)
	if err != nil {
		return err
	}
	q.state.VirtiofsdPid = 0
	return nil
}

func (q *qemu) getMemArgs() (bool, string, string, error) {
	share := false
	target := ""
	memoryBack := "memory-backend-ram"

	if q.qemuConfig.Knobs.HugePages {
		// we are setting all the bits that govmm sets when hugepages are enabled.
		// https://github.com/intel/govmm/blob/master/qemu/qemu.go#L1677
		target = "/dev/hugepages"
		memoryBack = "memory-backend-file"
		share = true
	} else {
		if q.config.EnableVhostUserStore {
			// Vhost-user-blk/scsi process which can improve performance, like SPDK,
			// requires shared-on hugepage to work with Qemu.
			return share, target, "", fmt.Errorf("Vhost-user-blk/scsi requires hugepage memory")
		}

		if q.config.SharedFS == config.VirtioFS || q.config.FileBackedMemRootDir != "" {
			target = q.qemuConfig.Memory.Path
			memoryBack = "memory-backend-file"
		}
	}

	if q.qemuConfig.Knobs.MemShared {
		share = true
	}

	return share, target, memoryBack, nil
}

func (q *qemu) setupVirtioMem() error {
	maxMem, err := q.hostMemMB()
	if err != nil {
		return err
	}
	// backend memory size must be multiple of 2Mib
	sizeMB := (int(maxMem) - int(q.config.MemorySize)) >> 2 << 2

	share, target, memoryBack, err := q.getMemArgs()
	if err != nil {
		return err
	}

	if err = q.qmpSetup(); err != nil {
		return err
	}
	err = q.qmpMonitorCh.qmp.ExecMemdevAdd(q.qmpMonitorCh.ctx, memoryBack, "virtiomem", target, sizeMB, share, "virtio-mem-pci", "virtiomem0")
	if err == nil {
		q.config.VirtioMem = true
		q.Logger().Infof("Setup %dMB virtio-mem-pci success", sizeMB)
	} else {
		help := ""
		if strings.Contains(err.Error(), "Cannot allocate memory") {
			help = ".  Please use command \"echo 1 > /proc/sys/vm/overcommit_memory\" handle it."
		}
		err = fmt.Errorf("Add %dMB virtio-mem-pci fail %s%s", sizeMB, err.Error(), help)
	}

	return err
}

// startSandbox will start the Sandbox's VM.
func (q *qemu) startSandbox(ctx context.Context, timeout int) error {
	span, ctx := q.trace(ctx, "startSandbox")
	defer span.End()

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
		q.fds = []*os.File{}
	}()

	vmPath := filepath.Join(q.store.RunVMStoragePath(), q.id)
	err := os.MkdirAll(vmPath, DirMode)
	if err != nil {
		return err
	}
	// append logfile only on debug
	if q.config.Debug {
		q.qemuConfig.LogFile = filepath.Join(vmPath, "qemu.log")
	}

	defer func() {
		if err != nil {
			if err := os.RemoveAll(vmPath); err != nil {
				q.Logger().WithError(err).Error("Fail to clean up vm directory")
			}
		}
	}()

	// This needs to be done as late as possible, just before launching
	// virtiofsd are executed by kata-runtime after this call, run with
	// the SELinux label. If these processes require privileged, we do
	// notwant to run them under confinement.
	if err := label.SetProcessLabel(q.config.SELinuxProcessLabel); err != nil {
		return err
	}
	defer label.SetProcessLabel("")

	if q.config.SharedFS == config.VirtioFS {
		err = q.setupVirtiofsd(ctx)
		if err != nil {
			return err
		}
		defer func() {
			if err != nil {
				if shutdownErr := q.stopVirtiofsd(ctx); shutdownErr != nil {
					q.Logger().WithError(shutdownErr).Warn("failed to stop virtiofsd")
				}
			}
		}()

	}

	var strErr string
	strErr, err = govmmQemu.LaunchQemu(q.qemuConfig, newQMPLogger())
	if err != nil {
		if q.config.Debug && q.qemuConfig.LogFile != "" {
			b, err := ioutil.ReadFile(q.qemuConfig.LogFile)
			if err == nil {
				strErr += string(b)
			}
		}
		q.Logger().WithError(err).Errorf("failed to launch qemu: %s", strErr)
		return fmt.Errorf("failed to launch qemu: %s, error messages from qemu log: %s", err, strErr)
	}

	err = q.waitSandbox(ctx, timeout)
	if err != nil {
		return err
	}

	if q.config.BootFromTemplate {
		if err = q.bootFromTemplate(); err != nil {
			return err
		}
	}

	if q.config.VirtioMem {
		err = q.setupVirtioMem()
	}

	return err
}

func (q *qemu) bootFromTemplate() error {
	if err := q.qmpSetup(); err != nil {
		return err
	}
	defer q.qmpShutdown()

	err := q.arch.setIgnoreSharedMemoryMigrationCaps(q.qmpMonitorCh.ctx, q.qmpMonitorCh.qmp)
	if err != nil {
		q.Logger().WithError(err).Error("set migration ignore shared memory")
		return err
	}
	uri := fmt.Sprintf("exec:cat %s", q.config.DevicesStatePath)
	err = q.qmpMonitorCh.qmp.ExecuteMigrationIncoming(q.qmpMonitorCh.ctx, uri)
	if err != nil {
		return err
	}
	return q.waitMigration()
}

// waitSandbox will wait for the Sandbox's VM to be up and running.
func (q *qemu) waitSandbox(ctx context.Context, timeout int) error {
	span, _ := q.trace(ctx, "waitSandbox")
	defer span.End()

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

		if int(time.Since(timeStart).Seconds()) > timeout {
			return fmt.Errorf("Failed to connect to QEMU instance (timeout %ds): %v", timeout, err)
		}

		time.Sleep(time.Duration(50) * time.Millisecond)
	}
	q.qmpMonitorCh.qmp = qmp
	q.qmpMonitorCh.disconn = disconnectCh
	defer q.qmpShutdown()

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
func (q *qemu) stopSandbox(ctx context.Context, waitOnly bool) error {
	span, _ := q.trace(ctx, "stopSandbox")
	defer span.End()

	q.Logger().Info("Stopping Sandbox")
	if q.stopped {
		q.Logger().Info("Already stopped")
		return nil
	}

	defer func() {
		q.cleanupVM()
		q.stopped = true
	}()

	if q.config.Debug && q.qemuConfig.LogFile != "" {
		f, err := os.OpenFile(q.qemuConfig.LogFile, os.O_RDONLY, 0)
		if err == nil {
			scanner := bufio.NewScanner(f)
			for scanner.Scan() {
				q.Logger().Debug(scanner.Text())
			}
			if err := scanner.Err(); err != nil {
				q.Logger().WithError(err).Debug("read qemu log failed")
			}
		}
	}

	if err := q.qmpSetup(); err != nil {
		return err
	}

	if waitOnly {
		pids := q.getPids()
		if len(pids) == 0 {
			return errors.New("cannot determine QEMU PID")
		}

		pid := pids[0]

		err := utils.WaitLocalProcess(pid, qemuStopSandboxTimeoutSecs, syscall.Signal(0), q.Logger())
		if err != nil {
			return err
		}
	} else {
		err := q.qmpMonitorCh.qmp.ExecuteQuit(q.qmpMonitorCh.ctx)
		if err != nil {
			q.Logger().WithError(err).Error("Fail to execute qmp QUIT")
			return err
		}
	}

	return nil
}

func (q *qemu) cleanupVM() error {

	// cleanup vm path
	dir := filepath.Join(q.store.RunVMStoragePath(), q.id)

	// If it's a symlink, remove both dir and the target.
	// This can happen when vm template links a sandbox to a vm.
	link, err := filepath.EvalSymlinks(dir)
	if err != nil {
		// Well, it's just cleanup failure. Let's ignore it.
		q.Logger().WithError(err).WithField("dir", dir).Warn("failed to resolve vm path")
	}
	q.Logger().WithField("link", link).WithField("dir", dir).Infof("cleanup vm path")

	if err := os.RemoveAll(dir); err != nil {
		q.Logger().WithError(err).Warnf("failed to remove vm path %s", dir)
	}
	if link != dir && link != "" {
		if err := os.RemoveAll(link); err != nil {
			q.Logger().WithError(err).WithField("link", link).Warn("failed to remove resolved vm path")
		}
	}

	if q.config.VMid != "" {
		dir = filepath.Join(q.store.RunStoragePath(), q.config.VMid)
		if err := os.RemoveAll(dir); err != nil {
			q.Logger().WithError(err).WithField("path", dir).Warnf("failed to remove vm path")
		}
	}

	return nil
}

func (q *qemu) togglePauseSandbox(ctx context.Context, pause bool) error {
	span, _ := q.trace(ctx, "togglePauseSandbox")
	defer span.End()

	if err := q.qmpSetup(); err != nil {
		return err
	}

	if pause {
		return q.qmpMonitorCh.qmp.ExecuteStop(q.qmpMonitorCh.ctx)
	}
	return q.qmpMonitorCh.qmp.ExecuteCont(q.qmpMonitorCh.ctx)
}

func (q *qemu) qmpSetup() error {
	q.qmpMonitorCh.Lock()
	defer q.qmpMonitorCh.Unlock()

	if q.qmpMonitorCh.qmp != nil {
		return nil
	}

	events := make(chan govmmQemu.QMPEvent)
	go q.loopQMPEvent(events)

	cfg := govmmQemu.QMPConfig{
		Logger:  newQMPLogger(),
		EventCh: events,
	}

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

func (q *qemu) loopQMPEvent(event chan govmmQemu.QMPEvent) {
	for e := range event {
		q.Logger().WithField("event", e).Debug("got QMP event")
		if e.Name == "GUEST_PANICKED" {
			go q.handleGuestPanic()
		}
	}
	q.Logger().Infof("QMP event channel closed")
}

func (q *qemu) handleGuestPanic() {
	if err := q.dumpGuestMemory(q.config.GuestMemoryDumpPath); err != nil {
		q.Logger().WithError(err).Error("failed to dump guest memory")
	}

	// TODO: how to notify the upper level sandbox to handle the error
	// to do a fast fail(shutdown or others).
	// tracked by https://github.com/kata-containers/kata-containers/issues/1026
}

// canDumpGuestMemory check if can do a guest memory dump operation.
// for now it only ensure there must be double of VM size for free disk spaces
func (q *qemu) canDumpGuestMemory(dumpSavePath string) error {
	fs := unix.Statfs_t{}
	if err := unix.Statfs(dumpSavePath, &fs); err != nil {
		q.Logger().WithError(err).WithField("dumpSavePath", dumpSavePath).Error("failed to call Statfs")
		return nil
	}
	availSpaceInBytes := fs.Bavail * uint64(fs.Bsize)
	q.Logger().WithFields(
		logrus.Fields{
			"dumpSavePath":      dumpSavePath,
			"availSpaceInBytes": availSpaceInBytes,
		}).Info("get avail space")

	// get guest memory size
	guestMemorySizeInBytes := (uint64(q.config.MemorySize) + uint64(q.state.HotpluggedMemory)) << utils.MibToBytesShift
	q.Logger().WithField("guestMemorySizeInBytes", guestMemorySizeInBytes).Info("get guest memory size")

	// default we want ensure there are at least double of VM memory size free spaces available,
	// this may complete one dump operation for one sandbox
	exceptMemorySize := guestMemorySizeInBytes * 2
	if availSpaceInBytes >= exceptMemorySize {
		return nil
	}
	return fmt.Errorf("there are not enough free space to store memory dump file. Except %d bytes, but only %d bytes available", exceptMemorySize, availSpaceInBytes)
}

// dumpSandboxMetaInfo save meta information for debug purpose, includes:
// hypervisor version, sandbox/container state, hypervisor config
func (q *qemu) dumpSandboxMetaInfo(dumpSavePath string) {
	dumpStatePath := filepath.Join(dumpSavePath, "state")

	// copy state from /run/vc/sbs to memory dump directory
	statePath := filepath.Join(q.store.RunStoragePath(), q.id)
	command := []string{"/bin/cp", "-ar", statePath, dumpStatePath}
	q.Logger().WithField("command", command).Info("try to save sandbox state")
	if output, err := pkgUtils.RunCommandFull(command, true); err != nil {
		q.Logger().WithError(err).WithField("output", output).Error("failed to save state")
	}
	// save hypervisor meta information
	fileName := filepath.Join(dumpSavePath, "hypervisor.conf")
	data, _ := json.MarshalIndent(q.config, "", " ")
	if err := ioutil.WriteFile(fileName, data, defaultFilePerms); err != nil {
		q.Logger().WithError(err).WithField("hypervisor.conf", data).Error("write to hypervisor.conf file failed")
	}

	// save hypervisor version
	hyperVisorVersion, err := pkgUtils.RunCommand([]string{q.config.HypervisorPath, "--version"})
	if err != nil {
		q.Logger().WithError(err).WithField("HypervisorPath", data).Error("failed to get hypervisor version")
	}

	fileName = filepath.Join(dumpSavePath, "hypervisor.version")
	if err := ioutil.WriteFile(fileName, []byte(hyperVisorVersion), defaultFilePerms); err != nil {
		q.Logger().WithError(err).WithField("hypervisor.version", data).Error("write to hypervisor.version file failed")
	}
}

func (q *qemu) dumpGuestMemory(dumpSavePath string) error {
	if dumpSavePath == "" {
		return nil
	}

	q.memoryDumpFlag.Lock()
	defer q.memoryDumpFlag.Unlock()

	q.Logger().WithField("dumpSavePath", dumpSavePath).Info("try to dump guest memory")

	dumpSavePath = filepath.Join(dumpSavePath, q.id)
	dumpStatePath := filepath.Join(dumpSavePath, "state")
	if err := pkgUtils.EnsureDir(dumpStatePath, DirMode); err != nil {
		return err
	}

	// save meta information for sandbox
	q.dumpSandboxMetaInfo(dumpSavePath)
	q.Logger().Info("dump sandbox meta information completed")

	// check device free space and estimated dump size
	if err := q.canDumpGuestMemory(dumpSavePath); err != nil {
		q.Logger().Warnf("can't dump guest memory: %s", err.Error())
		return err
	}

	// dump guest memory
	protocol := fmt.Sprintf("file:%s/vmcore-%s.%s", dumpSavePath, time.Now().Format("20060102150405.999"), memoryDumpFormat)
	q.Logger().Infof("try to dump guest memory to %s", protocol)

	if err := q.qmpSetup(); err != nil {
		q.Logger().WithError(err).Error("setup manage QMP failed")
		return err
	}

	if err := q.qmpMonitorCh.qmp.ExecuteDumpGuestMemory(q.qmpMonitorCh.ctx, protocol, q.config.GuestMemoryDumpPaging, memoryDumpFormat); err != nil {
		q.Logger().WithError(err).Error("dump guest memory failed")
		return err
	}

	q.Logger().Info("dump guest memory completed")
	return nil
}

func (q *qemu) qmpShutdown() {
	q.qmpMonitorCh.Lock()
	defer q.qmpMonitorCh.Unlock()

	if q.qmpMonitorCh.qmp != nil {
		q.qmpMonitorCh.qmp.Shutdown()
		// wait on disconnected channel to be sure that the qmp channel has
		// been closed cleanly.
		<-q.qmpMonitorCh.disconn
		q.qmpMonitorCh.qmp = nil
		q.qmpMonitorCh.disconn = nil
	}
}

func (q *qemu) hotplugAddBlockDevice(ctx context.Context, drive *config.BlockDrive, op operation, devID string) (err error) {
	// drive can be a pmem device, in which case it's used as backing file for a nvdimm device
	if q.config.BlockDeviceDriver == config.Nvdimm || drive.Pmem {
		var blocksize int64
		file, err := os.Open(drive.File)
		if err != nil {
			return err
		}
		defer file.Close()

		st, err := file.Stat()
		if err != nil {
			return fmt.Errorf("failed to get information from nvdimm device %v: %v", drive.File, err)
		}

		// regular files do not support syscall BLKGETSIZE64
		if st.Mode().IsRegular() {
			blocksize = st.Size()
		} else if _, _, err := syscall.Syscall(syscall.SYS_IOCTL, file.Fd(), unix.BLKGETSIZE64, uintptr(unsafe.Pointer(&blocksize))); err != 0 {
			return err
		}

		if err = q.qmpMonitorCh.qmp.ExecuteNVDIMMDeviceAdd(q.qmpMonitorCh.ctx, drive.ID, drive.File, blocksize, &drive.Pmem); err != nil {
			q.Logger().WithError(err).Errorf("Failed to add NVDIMM device %s", drive.File)
			return err
		}
		drive.NvdimmID = strconv.Itoa(q.nvdimmCount)
		q.nvdimmCount++
		return nil
	}

	if q.config.BlockDeviceCacheSet {
		err = q.qmpMonitorCh.qmp.ExecuteBlockdevAddWithCache(q.qmpMonitorCh.ctx, drive.File, drive.ID, q.config.BlockDeviceCacheDirect, q.config.BlockDeviceCacheNoflush, drive.ReadOnly)
	} else {
		err = q.qmpMonitorCh.qmp.ExecuteBlockdevAdd(q.qmpMonitorCh.ctx, drive.File, drive.ID, drive.ReadOnly)
	}
	if err != nil {
		return err
	}

	defer func() {
		if err != nil {
			q.qmpMonitorCh.qmp.ExecuteBlockdevDel(q.qmpMonitorCh.ctx, drive.ID)
		}
	}()

	switch {
	case q.config.BlockDeviceDriver == config.VirtioBlockCCW:
		driver := "virtio-blk-ccw"

		addr, bridge, err := q.arch.addDeviceToBridge(ctx, drive.ID, types.CCW)
		if err != nil {
			return err
		}
		var devNoHotplug string
		devNoHotplug, err = bridge.AddressFormatCCW(addr)
		if err != nil {
			return err
		}
		drive.DevNo, err = bridge.AddressFormatCCWForVirtServer(addr)
		if err != nil {
			return err
		}
		if err = q.qmpMonitorCh.qmp.ExecuteDeviceAdd(q.qmpMonitorCh.ctx, drive.ID, devID, driver, devNoHotplug, "", true, false); err != nil {
			return err
		}
	case q.config.BlockDeviceDriver == config.VirtioBlock:
		driver := "virtio-blk-pci"
		addr, bridge, err := q.arch.addDeviceToBridge(ctx, drive.ID, types.PCI)
		if err != nil {
			return err
		}

		defer func() {
			if err != nil {
				q.arch.removeDeviceFromBridge(drive.ID)
			}
		}()

		bridgeSlot, err := vcTypes.PciSlotFromInt(bridge.Addr)
		if err != nil {
			return err
		}
		devSlot, err := vcTypes.PciSlotFromString(addr)
		if err != nil {
			return err
		}
		drive.PCIPath, err = vcTypes.PciPathFromSlots(bridgeSlot, devSlot)
		if err != nil {
			return err
		}

		if err = q.qmpMonitorCh.qmp.ExecutePCIDeviceAdd(q.qmpMonitorCh.ctx, drive.ID, devID, driver, addr, bridge.ID, romFile, 0, true, defaultDisableModern); err != nil {
			return err
		}
	case q.config.BlockDeviceDriver == config.VirtioSCSI:
		driver := "scsi-hd"

		// Bus exposed by the SCSI Controller
		bus := scsiControllerID + ".0"

		// Get SCSI-id and LUN based on the order of attaching drives.
		scsiID, lun, err := utils.GetSCSIIdLun(drive.Index)
		if err != nil {
			return err
		}

		if err = q.qmpMonitorCh.qmp.ExecuteSCSIDeviceAdd(q.qmpMonitorCh.ctx, drive.ID, devID, driver, bus, romFile, scsiID, lun, true, defaultDisableModern); err != nil {
			return err
		}
	default:
		return fmt.Errorf("Block device %s not recognized", q.config.BlockDeviceDriver)
	}

	return nil
}

func (q *qemu) hotplugAddVhostUserBlkDevice(ctx context.Context, vAttr *config.VhostUserDeviceAttrs, op operation, devID string) (err error) {
	err = q.qmpMonitorCh.qmp.ExecuteCharDevUnixSocketAdd(q.qmpMonitorCh.ctx, vAttr.DevID, vAttr.SocketPath, false, false)
	if err != nil {
		return err
	}

	defer func() {
		if err != nil {
			q.qmpMonitorCh.qmp.ExecuteChardevDel(q.qmpMonitorCh.ctx, vAttr.DevID)
		}
	}()

	driver := "vhost-user-blk-pci"
	addr, bridge, err := q.arch.addDeviceToBridge(ctx, vAttr.DevID, types.PCI)
	if err != nil {
		return err
	}

	defer func() {
		if err != nil {
			q.arch.removeDeviceFromBridge(vAttr.DevID)
		}
	}()

	bridgeSlot, err := vcTypes.PciSlotFromInt(bridge.Addr)
	if err != nil {
		return err
	}
	devSlot, err := vcTypes.PciSlotFromString(addr)
	if err != nil {
		return err
	}
	vAttr.PCIPath, err = vcTypes.PciPathFromSlots(bridgeSlot, devSlot)

	if err = q.qmpMonitorCh.qmp.ExecutePCIVhostUserDevAdd(q.qmpMonitorCh.ctx, driver, devID, vAttr.DevID, addr, bridge.ID); err != nil {
		return err
	}

	return nil
}

func (q *qemu) hotplugBlockDevice(ctx context.Context, drive *config.BlockDrive, op operation) error {
	if err := q.qmpSetup(); err != nil {
		return err
	}

	devID := "virtio-" + drive.ID

	if op == addDevice {
		return q.hotplugAddBlockDevice(ctx, drive, op, devID)
	}
	if q.config.BlockDeviceDriver == config.VirtioBlock {
		if err := q.arch.removeDeviceFromBridge(drive.ID); err != nil {
			return err
		}
	}

	if err := q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, devID); err != nil {
		return err
	}

	return q.qmpMonitorCh.qmp.ExecuteBlockdevDel(q.qmpMonitorCh.ctx, drive.ID)
}

func (q *qemu) hotplugVhostUserDevice(ctx context.Context, vAttr *config.VhostUserDeviceAttrs, op operation) error {
	if err := q.qmpSetup(); err != nil {
		return err
	}

	devID := "virtio-" + vAttr.DevID

	if op == addDevice {
		switch vAttr.Type {
		case config.VhostUserBlk:
			return q.hotplugAddVhostUserBlkDevice(ctx, vAttr, op, devID)
		default:
			return fmt.Errorf("Incorrect vhost-user device type found")
		}
	} else {
		if err := q.arch.removeDeviceFromBridge(vAttr.DevID); err != nil {
			return err
		}

		if err := q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, devID); err != nil {
			return err
		}

		return q.qmpMonitorCh.qmp.ExecuteChardevDel(q.qmpMonitorCh.ctx, vAttr.DevID)
	}
}

func (q *qemu) hotplugVFIODevice(ctx context.Context, device *config.VFIODev, op operation) (err error) {
	if err = q.qmpSetup(); err != nil {
		return err
	}

	devID := device.ID
	machineType := q.hypervisorConfig().HypervisorMachineType

	if op == addDevice {

		buf, _ := json.Marshal(device)
		q.Logger().WithFields(logrus.Fields{
			"machine-type":             machineType,
			"hotplug-vfio-on-root-bus": q.state.HotplugVFIOOnRootBus,
			"pcie-root-port":           q.state.PCIeRootPort,
			"device-info":              string(buf),
		}).Info("Start hot-plug VFIO device")

		// In case HotplugVFIOOnRootBus is true, devices are hotplugged on the root bus
		// for pc machine type instead of bridge. This is useful for devices that require
		// a large PCI BAR which is a currently a limitation with PCI bridges.
		if q.state.HotplugVFIOOnRootBus {

			// In case MachineType is q35, a PCIe device is hotplugged on a PCIe Root Port.
			switch machineType {
			case QemuQ35:
				if device.IsPCIe && q.state.PCIeRootPort <= 0 {
					q.Logger().WithField("dev-id", device.ID).Warn("VFIO device is a PCIe device. It's recommended to add the PCIe Root Port by setting the pcie_root_port parameter in the configuration for q35")
					device.Bus = ""
				}
			default:
				device.Bus = ""
			}

			switch device.Type {
			case config.VFIODeviceNormalType:
				return q.qmpMonitorCh.qmp.ExecuteVFIODeviceAdd(q.qmpMonitorCh.ctx, devID, device.BDF, device.Bus, romFile)
			case config.VFIODeviceMediatedType:
				if utils.IsAPVFIOMediatedDevice(device.SysfsDev) {
					return q.qmpMonitorCh.qmp.ExecuteAPVFIOMediatedDeviceAdd(q.qmpMonitorCh.ctx, device.SysfsDev)
				}
				return q.qmpMonitorCh.qmp.ExecutePCIVFIOMediatedDeviceAdd(q.qmpMonitorCh.ctx, devID, device.SysfsDev, "", device.Bus, romFile)
			default:
				return fmt.Errorf("Incorrect VFIO device type found")
			}
		}

		addr, bridge, err := q.arch.addDeviceToBridge(ctx, devID, types.PCI)
		if err != nil {
			return err
		}

		defer func() {
			if err != nil {
				q.arch.removeDeviceFromBridge(devID)
			}
		}()

		switch device.Type {
		case config.VFIODeviceNormalType:
			return q.qmpMonitorCh.qmp.ExecutePCIVFIODeviceAdd(q.qmpMonitorCh.ctx, devID, device.BDF, addr, bridge.ID, romFile)
		case config.VFIODeviceMediatedType:
			if utils.IsAPVFIOMediatedDevice(device.SysfsDev) {
				return q.qmpMonitorCh.qmp.ExecuteAPVFIOMediatedDeviceAdd(q.qmpMonitorCh.ctx, device.SysfsDev)
			}
			return q.qmpMonitorCh.qmp.ExecutePCIVFIOMediatedDeviceAdd(q.qmpMonitorCh.ctx, devID, device.SysfsDev, addr, bridge.ID, romFile)
		default:
			return fmt.Errorf("Incorrect VFIO device type found")
		}
	} else {
		q.Logger().WithField("dev-id", devID).Info("Start hot-unplug VFIO device")

		if !q.state.HotplugVFIOOnRootBus {
			if err := q.arch.removeDeviceFromBridge(devID); err != nil {
				return err
			}
		}

		return q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, devID)
	}
}

func (q *qemu) hotAddNetDevice(name, hardAddr string, VMFds, VhostFds []*os.File) error {
	var (
		VMFdNames    []string
		VhostFdNames []string
	)
	for i, VMFd := range VMFds {
		fdName := fmt.Sprintf("fd%d", i)
		if err := q.qmpMonitorCh.qmp.ExecuteGetFD(q.qmpMonitorCh.ctx, fdName, VMFd); err != nil {
			return err
		}
		VMFdNames = append(VMFdNames, fdName)
	}
	for i, VhostFd := range VhostFds {
		fdName := fmt.Sprintf("vhostfd%d", i)
		if err := q.qmpMonitorCh.qmp.ExecuteGetFD(q.qmpMonitorCh.ctx, fdName, VhostFd); err != nil {
			return err
		}
		VhostFd.Close()
		VhostFdNames = append(VhostFdNames, fdName)
	}
	return q.qmpMonitorCh.qmp.ExecuteNetdevAddByFds(q.qmpMonitorCh.ctx, "tap", name, VMFdNames, VhostFdNames)
}

func (q *qemu) hotplugNetDevice(ctx context.Context, endpoint Endpoint, op operation) (err error) {
	if err = q.qmpSetup(); err != nil {
		return err
	}
	var tap TapInterface

	switch endpoint.Type() {
	case VethEndpointType:
		drive := endpoint.(*VethEndpoint)
		tap = drive.NetPair.TapInterface
	case TapEndpointType:
		drive := endpoint.(*TapEndpoint)
		tap = drive.TapInterface
	default:
		return fmt.Errorf("this endpoint is not supported")
	}

	devID := "virtio-" + tap.ID
	if op == addDevice {
		if err = q.hotAddNetDevice(tap.Name, endpoint.HardwareAddr(), tap.VMFds, tap.VhostFds); err != nil {
			return err
		}

		defer func() {
			if err != nil {
				q.qmpMonitorCh.qmp.ExecuteNetdevDel(q.qmpMonitorCh.ctx, tap.Name)
			}
		}()

		addr, bridge, err := q.arch.addDeviceToBridge(ctx, tap.ID, types.PCI)
		if err != nil {
			return err
		}

		defer func() {
			if err != nil {
				q.arch.removeDeviceFromBridge(tap.ID)
			}
		}()

		bridgeSlot, err := vcTypes.PciSlotFromInt(bridge.Addr)
		if err != nil {
			return err
		}
		devSlot, err := vcTypes.PciSlotFromString(addr)
		if err != nil {
			return err
		}
		pciPath, err := vcTypes.PciPathFromSlots(bridgeSlot, devSlot)
		endpoint.SetPciPath(pciPath)

		var machine govmmQemu.Machine
		machine, err = q.getQemuMachine()
		if err != nil {
			return err
		}
		if machine.Type == QemuCCWVirtio {
			devNoHotplug := fmt.Sprintf("fe.%x.%x", bridge.Addr, addr)
			return q.qmpMonitorCh.qmp.ExecuteNetCCWDeviceAdd(q.qmpMonitorCh.ctx, tap.Name, devID, endpoint.HardwareAddr(), devNoHotplug, int(q.config.NumVCPUs))
		}
		return q.qmpMonitorCh.qmp.ExecuteNetPCIDeviceAdd(q.qmpMonitorCh.ctx, tap.Name, devID, endpoint.HardwareAddr(), addr, bridge.ID, romFile, int(q.config.NumVCPUs), defaultDisableModern)

	}

	if err := q.arch.removeDeviceFromBridge(tap.ID); err != nil {
		return err
	}

	if err := q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, devID); err != nil {
		return err
	}

	return q.qmpMonitorCh.qmp.ExecuteNetdevDel(q.qmpMonitorCh.ctx, tap.Name)
}

func (q *qemu) hotplugDevice(ctx context.Context, devInfo interface{}, devType deviceType, op operation) (interface{}, error) {
	switch devType {
	case blockDev:
		drive := devInfo.(*config.BlockDrive)
		return nil, q.hotplugBlockDevice(ctx, drive, op)
	case cpuDev:
		vcpus := devInfo.(uint32)
		return q.hotplugCPUs(vcpus, op)
	case vfioDev:
		device := devInfo.(*config.VFIODev)
		return nil, q.hotplugVFIODevice(ctx, device, op)
	case memoryDev:
		memdev := devInfo.(*memoryDevice)
		return q.hotplugMemory(memdev, op)
	case netDev:
		device := devInfo.(Endpoint)
		return nil, q.hotplugNetDevice(ctx, device, op)
	case vhostuserDev:
		vAttr := devInfo.(*config.VhostUserDeviceAttrs)
		return nil, q.hotplugVhostUserDevice(ctx, vAttr, op)
	default:
		return nil, fmt.Errorf("cannot hotplug device: unsupported device type '%v'", devType)
	}
}

func (q *qemu) hotplugAddDevice(ctx context.Context, devInfo interface{}, devType deviceType) (interface{}, error) {
	span, ctx := q.trace(ctx, "hotplugAddDevice")
	defer span.End()

	data, err := q.hotplugDevice(ctx, devInfo, devType, addDevice)
	if err != nil {
		return data, err
	}

	return data, nil
}

func (q *qemu) hotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType deviceType) (interface{}, error) {
	span, ctx := q.trace(ctx, "hotplugRemoveDevice")
	defer span.End()

	data, err := q.hotplugDevice(ctx, devInfo, devType, removeDevice)
	if err != nil {
		return data, err
	}

	return data, nil
}

func (q *qemu) hotplugCPUs(vcpus uint32, op operation) (uint32, error) {
	if vcpus == 0 {
		q.Logger().Warnf("cannot hotplug 0 vCPUs")
		return 0, nil
	}

	if err := q.qmpSetup(); err != nil {
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

	machine := q.arch.machine()

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
		dieID := fmt.Sprintf("%d", hc.Properties.Die)
		coreID := fmt.Sprintf("%d", hc.Properties.Core)
		threadID := fmt.Sprintf("%d", hc.Properties.Thread)

		// If CPU type is IBM pSeries or Z, we do not set socketID and threadID
		if machine.Type == "pseries" || machine.Type == "s390-ccw-virtio" {
			socketID = ""
			threadID = ""
			dieID = ""
		}

		if err := q.qmpMonitorCh.qmp.ExecuteCPUDeviceAdd(q.qmpMonitorCh.ctx, driver, cpuID, socketID, dieID, coreID, threadID, romFile); err != nil {
			// don't fail, let's try with other CPU
			continue
		}

		// a new vCPU was added, update list of hotplugged vCPUs and check if all vCPUs were added
		q.state.HotpluggedVCPUs = append(q.state.HotpluggedVCPUs, CPUDevice{cpuID})
		hotpluggedVCPUs++
		if hotpluggedVCPUs == amount {
			// All vCPUs were hotplugged
			return amount, nil
		}
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
			return i, fmt.Errorf("failed to hotunplug CPUs, only %d CPUs were hotunplugged: %v", i, err)
		}

		// remove from the list the vCPU hotunplugged
		q.state.HotpluggedVCPUs = q.state.HotpluggedVCPUs[:len(q.state.HotpluggedVCPUs)-1]
	}

	return amount, nil
}

func (q *qemu) hotplugMemory(memDev *memoryDevice, op operation) (int, error) {

	if !q.arch.supportGuestMemoryHotplug() {
		return 0, fmt.Errorf("guest memory hotplug not supported")
	}
	if memDev.sizeMB < 0 {
		return 0, fmt.Errorf("cannot hotplug negative size (%d) memory", memDev.sizeMB)
	}
	memLog := q.Logger().WithField("hotplug", "memory")

	memLog.WithField("hotplug-memory-mb", memDev.sizeMB).Debug("requested memory hotplug")
	if err := q.qmpSetup(); err != nil {
		return 0, err
	}

	currentMemory := int(q.config.MemorySize) + q.state.HotpluggedMemory

	if memDev.sizeMB == 0 {
		memLog.Debug("hotplug is not required")
		return 0, nil
	}

	switch op {
	case removeDevice:
		memLog.WithField("operation", "remove").Debugf("Requested to remove memory: %d MB", memDev.sizeMB)
		// Dont fail but warn that this is not supported.
		memLog.Warn("hot-remove VM memory not supported")
		return 0, nil
	case addDevice:
		memLog.WithField("operation", "add").Debugf("Requested to add memory: %d MB", memDev.sizeMB)
		maxMem, err := q.hostMemMB()
		if err != nil {
			return 0, err
		}

		// Don't exceed the maximum amount of memory
		if currentMemory+memDev.sizeMB > int(maxMem) {
			// Fixme: return a typed error
			return 0, fmt.Errorf("Unable to hotplug %d MiB memory, the SB has %d MiB and the maximum amount is %d MiB",
				memDev.sizeMB, currentMemory, maxMem)
		}
		memoryAdded, err := q.hotplugAddMemory(memDev)
		if err != nil {
			return memoryAdded, err
		}
		return memoryAdded, nil
	default:
		return 0, fmt.Errorf("invalid operation %v", op)
	}

}

func (q *qemu) hotplugAddMemory(memDev *memoryDevice) (int, error) {
	memoryDevices, err := q.qmpMonitorCh.qmp.ExecQueryMemoryDevices(q.qmpMonitorCh.ctx)
	if err != nil {
		return 0, fmt.Errorf("failed to query memory devices: %v", err)
	}

	if len(memoryDevices) != 0 {
		maxSlot := -1
		for _, device := range memoryDevices {
			if maxSlot < device.Data.Slot {
				maxSlot = device.Data.Slot
			}
		}
		memDev.slot = maxSlot + 1
	}

	share, target, memoryBack, err := q.getMemArgs()
	if err != nil {
		return 0, err
	}

	err = q.qmpMonitorCh.qmp.ExecHotplugMemory(q.qmpMonitorCh.ctx, memoryBack, "mem"+strconv.Itoa(memDev.slot), target, memDev.sizeMB, share)
	if err != nil {
		q.Logger().WithError(err).Error("hotplug memory")
		return 0, err
	}
	// if guest kernel only supports memory hotplug via probe interface, we need to get address of hot-add memory device
	if memDev.probe {
		memoryDevices, err := q.qmpMonitorCh.qmp.ExecQueryMemoryDevices(q.qmpMonitorCh.ctx)
		if err != nil {
			return 0, fmt.Errorf("failed to query memory devices: %v", err)
		}
		if len(memoryDevices) != 0 {
			q.Logger().WithField("addr", fmt.Sprintf("0x%x", memoryDevices[len(memoryDevices)-1].Data.Addr)).Debug("recently hot-add memory device")
			memDev.addr = memoryDevices[len(memoryDevices)-1].Data.Addr
		} else {
			return 0, fmt.Errorf("failed to probe address of recently hot-add memory device, no device exists")
		}
	}
	q.state.HotpluggedMemory += memDev.sizeMB
	return memDev.sizeMB, nil
}

func (q *qemu) pauseSandbox(ctx context.Context) error {
	span, ctx := q.trace(ctx, "pauseSandbox")
	defer span.End()

	return q.togglePauseSandbox(ctx, true)
}

func (q *qemu) resumeSandbox(ctx context.Context) error {
	span, ctx := q.trace(ctx, "resumeSandbox")
	defer span.End()

	return q.togglePauseSandbox(ctx, false)
}

// addDevice will add extra devices to Qemu command line.
func (q *qemu) addDevice(ctx context.Context, devInfo interface{}, devType deviceType) error {
	var err error
	span, _ := q.trace(ctx, "addDevice")
	defer span.End()

	switch v := devInfo.(type) {
	case types.Volume:
		if q.config.SharedFS == config.VirtioFS {
			q.Logger().WithField("volume-type", "virtio-fs").Info("adding volume")

			var randBytes []byte
			randBytes, err = utils.GenerateRandomBytes(8)
			if err != nil {
				return err
			}
			id := hex.EncodeToString(randBytes)

			var sockPath string
			sockPath, err = q.vhostFSSocketPath(q.id)
			if err != nil {
				return err
			}

			vhostDev := config.VhostUserDeviceAttrs{
				Tag:       v.MountTag,
				Type:      config.VhostUserFS,
				CacheSize: q.config.VirtioFSCacheSize,
				Cache:     q.config.VirtioFSCache,
			}
			vhostDev.SocketPath = sockPath
			vhostDev.DevID = id

			q.qemuConfig.Devices, err = q.arch.appendVhostUserDevice(ctx, q.qemuConfig.Devices, vhostDev)
		} else {
			q.Logger().WithField("volume-type", "virtio-9p").Info("adding volume")
			q.qemuConfig.Devices, err = q.arch.append9PVolume(ctx, q.qemuConfig.Devices, v)
		}
	case types.Socket:
		q.qemuConfig.Devices = q.arch.appendSocket(q.qemuConfig.Devices, v)
	case types.VSock:
		q.fds = append(q.fds, v.VhostFd)
		q.qemuConfig.Devices, err = q.arch.appendVSock(ctx, q.qemuConfig.Devices, v)
	case Endpoint:
		q.qemuConfig.Devices, err = q.arch.appendNetwork(ctx, q.qemuConfig.Devices, v)
	case config.BlockDrive:
		q.qemuConfig.Devices, err = q.arch.appendBlockDevice(ctx, q.qemuConfig.Devices, v)
	case config.VhostUserDeviceAttrs:
		q.qemuConfig.Devices, err = q.arch.appendVhostUserDevice(ctx, q.qemuConfig.Devices, v)
	case config.VFIODev:
		q.qemuConfig.Devices = q.arch.appendVFIODevice(q.qemuConfig.Devices, v)
	default:
		q.Logger().WithField("dev-type", v).Warn("Could not append device: unsupported device type")
	}

	return err
}

// getSandboxConsole builds the path of the console where we can read
// logs coming from the sandbox.
func (q *qemu) getSandboxConsole(ctx context.Context, id string) (string, string, error) {
	span, _ := q.trace(ctx, "getSandboxConsole")
	defer span.End()

	consoleURL, err := utils.BuildSocketPath(q.store.RunVMStoragePath(), id, consoleSocket)
	if err != nil {
		return consoleProtoUnix, "", err
	}

	return consoleProtoUnix, consoleURL, nil
}

func (q *qemu) saveSandbox() error {
	q.Logger().Info("save sandbox")

	if err := q.qmpSetup(); err != nil {
		return err
	}

	// BootToBeTemplate sets the VM to be a template that other VMs can clone from. We would want to
	// bypass shared memory when saving the VM to a local file through migration exec.
	if q.config.BootToBeTemplate {
		err := q.arch.setIgnoreSharedMemoryMigrationCaps(q.qmpMonitorCh.ctx, q.qmpMonitorCh.qmp)
		if err != nil {
			q.Logger().WithError(err).Error("set migration ignore shared memory")
			return err
		}
	}

	err := q.qmpMonitorCh.qmp.ExecSetMigrateArguments(q.qmpMonitorCh.ctx, fmt.Sprintf("%s>%s", qmpExecCatCmd, q.config.DevicesStatePath))
	if err != nil {
		q.Logger().WithError(err).Error("exec migration")
		return err
	}

	return q.waitMigration()
}

func (q *qemu) waitMigration() error {
	t := time.NewTimer(qmpMigrationWaitTimeout)
	defer t.Stop()
	for {
		status, err := q.qmpMonitorCh.qmp.ExecuteQueryMigration(q.qmpMonitorCh.ctx)
		if err != nil {
			q.Logger().WithError(err).Error("failed to query migration status")
			return err
		}
		if status.Status == "completed" {
			break
		}

		select {
		case <-t.C:
			q.Logger().WithField("migration-status", status).Error("timeout waiting for qemu migration")
			return fmt.Errorf("timed out after %d seconds waiting for qemu migration", qmpMigrationWaitTimeout)
		default:
			// migration in progress
			q.Logger().WithField("migration-status", status).Debug("migration in progress")
			time.Sleep(100 * time.Millisecond)
		}
	}

	return nil
}

func (q *qemu) disconnect(ctx context.Context) {
	span, _ := q.trace(ctx, "disconnect")
	defer span.End()

	q.qmpShutdown()
}

// resizeMemory get a request to update the VM memory to reqMemMB
// Memory update is managed with two approaches
// Add memory to VM:
// When memory is required to be added we hotplug memory
// Remove Memory from VM/ Return memory to host.
//
// Memory unplug can be slow and it cannot be guaranteed.
// Additionally, the unplug has not small granularly it has to be
// the memory to remove has to be at least the size of one slot.
// To return memory back we are resizing the VM memory balloon.
// A longer term solution is evaluate solutions like virtio-mem
func (q *qemu) resizeMemory(ctx context.Context, reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, memoryDevice, error) {

	currentMemory := q.config.MemorySize + uint32(q.state.HotpluggedMemory)
	if err := q.qmpSetup(); err != nil {
		return 0, memoryDevice{}, err
	}
	var addMemDevice memoryDevice
	if q.config.VirtioMem && currentMemory != reqMemMB {
		q.Logger().WithField("hotplug", "memory").Debugf("resize memory from %dMB to %dMB", currentMemory, reqMemMB)
		sizeByte := uint64(reqMemMB - q.config.MemorySize)
		sizeByte = sizeByte * 1024 * 1024
		err := q.qmpMonitorCh.qmp.ExecQomSet(q.qmpMonitorCh.ctx, "virtiomem0", "requested-size", sizeByte)
		if err != nil {
			return 0, memoryDevice{}, err
		}
		q.state.HotpluggedMemory = int(sizeByte / 1024 / 1024)
		return reqMemMB, memoryDevice{}, nil
	}

	switch {
	case currentMemory < reqMemMB:
		//hotplug
		addMemMB := reqMemMB - currentMemory
		memHotplugMB, err := calcHotplugMemMiBSize(addMemMB, memoryBlockSizeMB)
		if err != nil {
			return currentMemory, memoryDevice{}, err
		}

		addMemDevice.sizeMB = int(memHotplugMB)
		addMemDevice.probe = probe

		data, err := q.hotplugAddDevice(ctx, &addMemDevice, memoryDev)
		if err != nil {
			return currentMemory, addMemDevice, err
		}
		memoryAdded, ok := data.(int)
		if !ok {
			return currentMemory, addMemDevice, fmt.Errorf("Could not get the memory added, got %+v", data)
		}
		currentMemory += uint32(memoryAdded)
	case currentMemory > reqMemMB:
		//hotunplug
		addMemMB := currentMemory - reqMemMB
		memHotunplugMB, err := calcHotplugMemMiBSize(addMemMB, memoryBlockSizeMB)
		if err != nil {
			return currentMemory, memoryDevice{}, err
		}

		addMemDevice.sizeMB = int(memHotunplugMB)
		addMemDevice.probe = probe

		data, err := q.hotplugRemoveDevice(ctx, &addMemDevice, memoryDev)
		if err != nil {
			return currentMemory, addMemDevice, err
		}
		memoryRemoved, ok := data.(int)
		if !ok {
			return currentMemory, addMemDevice, fmt.Errorf("Could not get the memory removed, got %+v", data)
		}
		//FIXME: This is to check memory hotplugRemoveDevice reported 0, as this is not supported.
		// In the future if this is implemented this validation should be removed.
		if memoryRemoved != 0 {
			return currentMemory, addMemDevice, fmt.Errorf("memory hot unplug is not supported, something went wrong")
		}
		currentMemory -= uint32(memoryRemoved)
	}

	// currentMemory is the current memory (updated) of the VM, return to caller to allow verify
	// the current VM memory state.
	return currentMemory, addMemDevice, nil
}

// genericAppendBridges appends to devices the given bridges
// nolint: unused, deadcode
func genericAppendBridges(devices []govmmQemu.Device, bridges []types.Bridge, machineType string) []govmmQemu.Device {
	bus := defaultPCBridgeBus
	switch machineType {
	case QemuQ35, QemuVirt:
		bus = defaultBridgeBus
	}

	for idx, b := range bridges {
		t := govmmQemu.PCIBridge
		if b.Type == types.PCIE {
			t = govmmQemu.PCIEBridge
		}
		if b.Type == types.CCW {
			continue
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

func genericBridges(number uint32, machineType string) []types.Bridge {
	var bridges []types.Bridge
	var bt types.Type

	switch machineType {
	case QemuQ35:
		// currently only pci bridges are supported
		// qemu-2.10 will introduce pcie bridges
		fallthrough
	case QemuPC:
		bt = types.PCI
	case QemuVirt:
		bt = types.PCI
	case QemuPseries:
		bt = types.PCI
	case QemuCCWVirtio:
		bt = types.CCW
	default:
		return nil
	}

	for i := uint32(0); i < number; i++ {
		bridges = append(bridges, types.NewBridge(bt, fmt.Sprintf("%s-bridge-%d", bt, i), make(map[uint32]string), 0))
	}

	return bridges
}

// nolint: unused, deadcode
func genericMemoryTopology(memoryMb, hostMemoryMb uint64, slots uint8, memoryOffset uint32) govmmQemu.Memory {
	// image NVDIMM device needs memory space 1024MB
	// See https://github.com/clearcontainers/runtime/issues/380
	memoryOffset += 1024

	memMax := fmt.Sprintf("%dM", hostMemoryMb+uint64(memoryOffset))

	mem := fmt.Sprintf("%dM", memoryMb)

	memory := govmmQemu.Memory{
		Size:   mem,
		Slots:  slots,
		MaxMem: memMax,
	}

	return memory
}

// genericAppendPCIeRootPort appends to devices the given pcie-root-port
func genericAppendPCIeRootPort(devices []govmmQemu.Device, number uint32, machineType string) []govmmQemu.Device {
	var (
		bus           string
		chassis       string
		multiFunction bool
		addr          string
	)
	switch machineType {
	case QemuQ35:
		bus = defaultBridgeBus
		chassis = "0"
		multiFunction = false
		addr = "0"
	default:
		return devices
	}

	for i := uint32(0); i < number; i++ {
		devices = append(devices,
			govmmQemu.PCIeRootPortDevice{
				ID:            fmt.Sprintf("%s%d", pcieRootPortPrefix, i),
				Bus:           bus,
				Chassis:       chassis,
				Slot:          strconv.FormatUint(uint64(i), 10),
				Multifunction: multiFunction,
				Addr:          addr,
			},
		)
	}
	return devices
}

func (q *qemu) getThreadIDs(ctx context.Context) (vcpuThreadIDs, error) {
	span, _ := q.trace(ctx, "getThreadIDs")
	defer span.End()

	tid := vcpuThreadIDs{}
	if err := q.qmpSetup(); err != nil {
		return tid, err
	}

	cpuInfos, err := q.qmpMonitorCh.qmp.ExecQueryCpus(q.qmpMonitorCh.ctx)
	if err != nil {
		q.Logger().WithError(err).Error("failed to query cpu infos")
		return tid, err
	}

	tid.vcpus = make(map[int]int, len(cpuInfos))
	for _, i := range cpuInfos {
		if i.ThreadID > 0 {
			tid.vcpus[i.CPU] = i.ThreadID
		}
	}
	return tid, nil
}

func calcHotplugMemMiBSize(mem uint32, memorySectionSizeMB uint32) (uint32, error) {
	if memorySectionSizeMB == 0 {
		return mem, nil
	}

	return uint32(math.Ceil(float64(mem)/float64(memorySectionSizeMB))) * memorySectionSizeMB, nil
}

func (q *qemu) resizeVCPUs(ctx context.Context, reqVCPUs uint32) (currentVCPUs uint32, newVCPUs uint32, err error) {

	currentVCPUs = q.config.NumVCPUs + uint32(len(q.state.HotpluggedVCPUs))
	newVCPUs = currentVCPUs
	switch {
	case currentVCPUs < reqVCPUs:
		//hotplug
		addCPUs := reqVCPUs - currentVCPUs
		data, err := q.hotplugAddDevice(ctx, addCPUs, cpuDev)
		if err != nil {
			return currentVCPUs, newVCPUs, err
		}
		vCPUsAdded, ok := data.(uint32)
		if !ok {
			return currentVCPUs, newVCPUs, fmt.Errorf("Could not get the vCPUs added, got %+v", data)
		}
		newVCPUs += vCPUsAdded
	case currentVCPUs > reqVCPUs:
		//hotunplug
		removeCPUs := currentVCPUs - reqVCPUs
		data, err := q.hotplugRemoveDevice(ctx, removeCPUs, cpuDev)
		if err != nil {
			return currentVCPUs, newVCPUs, err
		}
		vCPUsRemoved, ok := data.(uint32)
		if !ok {
			return currentVCPUs, newVCPUs, fmt.Errorf("Could not get the vCPUs removed, got %+v", data)
		}
		newVCPUs -= vCPUsRemoved
	}
	return currentVCPUs, newVCPUs, nil
}

func (q *qemu) cleanup(ctx context.Context) error {
	span, _ := q.trace(ctx, "cleanup")
	defer span.End()

	for _, fd := range q.fds {
		if err := fd.Close(); err != nil {
			q.Logger().WithError(err).Warn("failed closing fd")
		}
	}
	q.fds = []*os.File{}

	return nil
}

func (q *qemu) getPids() []int {
	data, err := ioutil.ReadFile(q.qemuConfig.PidFile)
	if err != nil {
		q.Logger().WithError(err).Error("Could not read qemu pid file")
		return []int{0}
	}

	pid, err := strconv.Atoi(strings.Trim(string(data), "\n\t "))
	if err != nil {
		q.Logger().WithError(err).Error("Could not convert string to int")
		return []int{0}
	}

	var pids []int
	pids = append(pids, pid)
	if q.state.VirtiofsdPid != 0 {
		pids = append(pids, q.state.VirtiofsdPid)
	}

	return pids
}

func (q *qemu) getVirtioFsPid() *int {
	return &q.state.VirtiofsdPid
}

type qemuGrpc struct {
	ID             string
	QmpChannelpath string
	State          QemuState
	NvdimmCount    int

	// Most members of q.qemuConfig are just to generate
	// q.qemuConfig.qemuParams that is used by LaunchQemu except
	// q.qemuConfig.SMP.
	// So just transport q.qemuConfig.SMP from VM Cache server to runtime.
	QemuSMP govmmQemu.SMP
}

func (q *qemu) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	var qp qemuGrpc
	err := json.Unmarshal(j, &qp)
	if err != nil {
		return err
	}

	q.id = qp.ID
	q.config = *hypervisorConfig
	q.qmpMonitorCh.ctx = ctx
	q.qmpMonitorCh.path = qp.QmpChannelpath
	q.qemuConfig.Ctx = ctx
	q.state = qp.State
	q.arch, err = newQemuArch(q.config)
	if err != nil {
		return err
	}
	q.ctx = ctx
	q.nvdimmCount = qp.NvdimmCount

	q.qemuConfig.SMP = qp.QemuSMP

	q.arch.setBridges(q.state.Bridges)
	return nil
}

func (q *qemu) toGrpc(ctx context.Context) ([]byte, error) {
	q.qmpShutdown()

	q.cleanup(ctx)
	qp := qemuGrpc{
		ID:             q.id,
		QmpChannelpath: q.qmpMonitorCh.path,
		State:          q.state,
		NvdimmCount:    q.nvdimmCount,

		QemuSMP: q.qemuConfig.SMP,
	}

	return json.Marshal(&qp)
}

func (q *qemu) save() (s persistapi.HypervisorState) {

	// If QEMU isn't even running, there isn't any state to save
	if q.stopped {
		return
	}

	pids := q.getPids()
	if len(pids) != 0 {
		s.Pid = pids[0]
	}
	s.VirtiofsdPid = q.state.VirtiofsdPid
	s.Type = string(QemuHypervisor)
	s.UUID = q.state.UUID
	s.HotpluggedMemory = q.state.HotpluggedMemory
	s.HotplugVFIOOnRootBus = q.state.HotplugVFIOOnRootBus
	s.PCIeRootPort = q.state.PCIeRootPort

	for _, bridge := range q.arch.getBridges() {
		s.Bridges = append(s.Bridges, persistapi.Bridge{
			DeviceAddr: bridge.Devices,
			Type:       string(bridge.Type),
			ID:         bridge.ID,
			Addr:       bridge.Addr,
		})
	}

	for _, cpu := range q.state.HotpluggedVCPUs {
		s.HotpluggedVCPUs = append(s.HotpluggedVCPUs, persistapi.CPUDevice{
			ID: cpu.ID,
		})
	}
	return
}

func (q *qemu) load(s persistapi.HypervisorState) {
	q.state.UUID = s.UUID
	q.state.HotpluggedMemory = s.HotpluggedMemory
	q.state.HotplugVFIOOnRootBus = s.HotplugVFIOOnRootBus
	q.state.VirtiofsdPid = s.VirtiofsdPid
	q.state.PCIeRootPort = s.PCIeRootPort

	for _, bridge := range s.Bridges {
		q.state.Bridges = append(q.state.Bridges, types.NewBridge(types.Type(bridge.Type), bridge.ID, bridge.DeviceAddr, bridge.Addr))
	}

	for _, cpu := range s.HotpluggedVCPUs {
		q.state.HotpluggedVCPUs = append(q.state.HotpluggedVCPUs, CPUDevice{
			ID: cpu.ID,
		})
	}
}

func (q *qemu) check() error {
	q.memoryDumpFlag.Lock()
	defer q.memoryDumpFlag.Unlock()

	if err := q.qmpSetup(); err != nil {
		return err
	}

	status, err := q.qmpMonitorCh.qmp.ExecuteQueryStatus(q.qmpMonitorCh.ctx)
	if err != nil {
		return err
	}

	if status.Status == "internal-error" || status.Status == "guest-panicked" {
		return errors.Errorf("guest failure: %s", status.Status)
	}

	return nil
}

func (q *qemu) generateSocket(id string) (interface{}, error) {
	return generateVMSocket(id, q.store.RunVMStoragePath())
}

func (q *qemu) isRateLimiterBuiltin() bool {
	return false
}

func (q *qemu) setSandbox(sandbox *Sandbox) {
}
