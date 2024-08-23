//go:build linux

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
	"io"
	"math"
	"net"
	"os"
	"os/exec"
	"os/user"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"syscall"
	"time"
	"unsafe"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"

	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
	"github.com/opencontainers/selinux/go-selinux/label"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/drivers"
	hv "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	pkgUtils "github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/uuid"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// qemuTracingTags defines tags for the trace span
var qemuTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "hypervisor",
	"type":      "qemu",
}

// romFile is the file name of the ROM that can be used for virtio-pci devices.
// If this file name is empty, this means we expect the firmware used by Qemu,
// such as SeaBIOS or OVMF for instance, to handle this directly.
const romFile = ""

// disable-modern is a option to QEMU that will fall back to using 0.9 version
// of virtio. Since moving to QEMU4.0, we can start using virtio 1.0 version.
// Default value is false.
const defaultDisableModern = false

type qmpChannel struct {
	qmp     *govmmQemu.QMP
	ctx     context.Context
	disconn chan struct{}
	path    string
	sync.Mutex
}

// QemuState keeps Qemu's state
type QemuState struct {
	UUID              string
	HotPlugVFIO       config.PCIePort
	Bridges           []types.Bridge
	HotpluggedVCPUs   []hv.CPUDevice
	HotpluggedMemory  int
	VirtiofsDaemonPid int
	HotplugVFIO       config.PCIePort
	ColdPlugVFIO      config.PCIePort
	PCIeRootPort      uint32
	PCIeSwitchPort    uint32
}

// qemu is an Hypervisor interface implementation for the Linux qemu hypervisor.
// nolint: govet
type qemu struct {
	arch qemuArch

	virtiofsDaemon VirtiofsDaemon

	ctx context.Context

	// fds is a list of file descriptors inherited by QEMU process
	// they'll be closed once QEMU process is running
	fds []*os.File

	id string

	state QemuState

	qmpMonitorCh qmpChannel

	qemuConfig govmmQemu.Config

	config HypervisorConfig

	// if in memory dump progress
	memoryDumpFlag sync.Mutex

	nvdimmCount int

	stopped int32

	mu sync.Mutex
}

const (
	consoleSocket      = "console.sock"
	qmpSocket          = "qmp.sock"
	extraMonitorSocket = "extra-monitor.sock"
	vhostFSSocket      = "vhost-fs.sock"
	nydusdAPISock      = "nydusd-api.sock"

	// memory dump format will be set to elf
	memoryDumpFormat = "elf"

	qmpCapErrMsg  = "Failed to negotiate QMP Capabilities"
	qmpExecCatCmd = "exec:cat"

	scsiControllerID         = "scsi0"
	rngID                    = "rng0"
	fallbackFileBackedMemDir = "/dev/shm"

	qemuStopSandboxTimeoutSecs = 15

	qomPathPrefix = "/machine/peripheral/"
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
		logger: hvLogger.WithField("subsystem", "qmp"),
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
	return hvLogger.WithField("subsystem", "qemu")
}

func (q *qemu) kernelParameters() string {
	// get a list of arch kernel parameters
	params := q.arch.kernelParameters(q.config.Debug)

	// use default parameters
	params = append(params, defaultKernelParameters...)

	// set the maximum number of vCPUs
	params = append(params, Param{"nr_cpus", fmt.Sprintf("%d", q.config.DefaultMaxVCPUs)})

	// set the SELinux params in accordance with the runtime configuration, disable_guest_selinux.
	if q.config.DisableGuestSeLinux {
		q.Logger().Info("Set selinux=0 to kernel params because SELinux on the guest is disabled")
		params = append(params, Param{"selinux", "0"})
	} else {
		q.Logger().Info("Set selinux=1 to kernel params because SELinux on the guest is enabled")
		params = append(params, Param{"selinux", "1"})
	}

	// add the params specified by the provided config. As the kernel
	// honours the last parameter value set and since the config-provided
	// params are added here, they will take priority over the defaults.
	params = append(params, q.config.KernelParams...)

	paramsStr := SerializeParams(params, "=")

	return strings.Join(paramsStr, " ")
}

// Adds all capabilities supported by qemu implementation of hypervisor interface
func (q *qemu) Capabilities(ctx context.Context) types.Capabilities {
	span, _ := katatrace.Trace(ctx, q.Logger(), "Capabilities", qemuTracingTags, map[string]string{"sandbox_id": q.id})
	defer span.End()

	return q.arch.capabilities(q.config)
}

func (q *qemu) HypervisorConfig() HypervisorConfig {
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

// setup sets the Qemu structure up.
func (q *qemu) setup(ctx context.Context, id string, hypervisorConfig *HypervisorConfig) error {
	span, _ := katatrace.Trace(ctx, q.Logger(), "setup", qemuTracingTags, map[string]string{"sandbox_id": q.id})
	defer span.End()

	if err := q.setConfig(hypervisorConfig); err != nil {
		return err
	}

	q.id = id

	var err error

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
		q.state.HotPlugVFIO = q.config.HotPlugVFIO
		q.state.ColdPlugVFIO = q.config.ColdPlugVFIO
		q.state.PCIeRootPort = q.config.PCIeRootPort
		q.state.PCIeSwitchPort = q.config.PCIeSwitchPort

		// The path might already exist, but in case of VM templating,
		// we have to create it since the sandbox has not created it yet.
		if err = utils.MkdirAllWithInheritedOwner(filepath.Join(q.config.RunStorePath, id), DirMode); err != nil {
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
	return q.arch.cpuTopology(q.config.NumVCPUs(), q.config.DefaultMaxVCPUs)
}

func (q *qemu) memoryTopology() (govmmQemu.Memory, error) {
	hostMemMb := q.config.DefaultMaxMemorySize
	memMb := uint64(q.config.MemorySize)

	return q.arch.memoryTopology(memMb, hostMemMb, uint8(q.config.MemSlots)), nil
}

func (q *qemu) qmpSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(q.config.VMStorePath, id, qmpSocket)
}

func (q *qemu) extraMonitorSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(q.config.VMStorePath, id, extraMonitorSocket)
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

func (q *qemu) createQmpSocket() ([]govmmQemu.QMPSocket, error) {
	monitorSockPath, err := q.qmpSocketPath(q.id)
	if err != nil {
		return nil, err
	}

	q.qmpMonitorCh = qmpChannel{
		ctx:  q.ctx,
		path: monitorSockPath,
	}

	var sockets []govmmQemu.QMPSocket

	sockets = append(sockets, govmmQemu.QMPSocket{
		Type:     "unix",
		Protocol: govmmQemu.Qmp,
		Server:   true,
		NoWait:   true,
	})

	// The extra monitor socket allows an external user to take full
	// control on Qemu and silently break the VM in all possible ways.
	// It should only ever be used for debugging purposes, hence the
	// check on Debug.
	if q.HypervisorConfig().Debug && q.config.ExtraMonitorSocket != "" {
		extraMonitorSockPath, err := q.extraMonitorSocketPath(q.id)
		if err != nil {
			return nil, err
		}

		sockets = append(sockets, govmmQemu.QMPSocket{
			Type:     "unix",
			Protocol: q.config.ExtraMonitorSocket,
			Name:     extraMonitorSockPath,
			Server:   true,
			NoWait:   true,
		})

		q.Logger().Warn("QEMU configured to start with an untrusted monitor")
	}

	return sockets, nil
}

func (q *qemu) buildDevices(ctx context.Context, kernelPath string) ([]govmmQemu.Device, *govmmQemu.IOThread, *govmmQemu.Kernel, error) {
	var devices []govmmQemu.Device

	kernel := &govmmQemu.Kernel{
		Path: kernelPath,
	}

	_, console, err := q.GetVMConsole(ctx, q.id)
	if err != nil {
		return nil, nil, nil, err
	}

	// Add bridges before any other devices. This way we make sure that
	// bridge gets the first available PCI address i.e bridgePCIStartAddr
	devices = q.arch.appendBridges(devices)

	devices, err = q.arch.appendConsole(ctx, devices, console)
	if err != nil {
		return nil, nil, nil, err
	}

	assetPath, assetType, err := q.config.ImageOrInitrdAssetPath()
	if err != nil {
		return nil, nil, nil, err
	}

	if assetType == types.ImageAsset {
		devices, err = q.arch.appendImage(ctx, devices, assetPath)
		if err != nil {
			return nil, nil, nil, err
		}
	} else if assetType == types.InitrdAsset {
		// InitrdAsset, need to set kernel initrd path
		kernel.InitrdPath = assetPath
	} else if assetType == types.SecureBootAsset {
		// SecureBootAsset, no need to set image or initrd path
		q.Logger().Info("For IBM Z Secure Execution, initrd path should not be set")
		kernel.InitrdPath = ""
	}

	if q.config.IOMMU {
		devices, err = q.arch.appendIOMMU(devices)
		if err != nil {
			return nil, nil, nil, err
		}
	}

	if q.config.IfPVPanicEnabled() {
		// there should have no errors for pvpanic device
		devices, _ = q.arch.appendPVPanicDevice(devices)
	}

	var ioThread *govmmQemu.IOThread
	if q.config.BlockDeviceDriver == config.VirtioSCSI {
		devices, ioThread, err = q.arch.appendSCSIController(ctx, devices, q.config.EnableIOThreads)
		if err != nil {
			return nil, nil, nil, err
		}
	}

	return devices, ioThread, kernel, nil
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

func (q *qemu) setConfig(config *HypervisorConfig) error {
	q.config = *config

	return nil
}

func (q *qemu) createVirtiofsDaemon(sharedPath string) (VirtiofsDaemon, error) {
	virtiofsdSocketPath, err := q.vhostFSSocketPath(q.id)
	if err != nil {
		return nil, err
	}

	if q.config.SharedFS == config.VirtioFSNydus {
		apiSockPath, err := q.nydusdAPISocketPath(q.id)
		if err != nil {
			return nil, err
		}
		nd := &nydusd{
			path:        q.config.VirtioFSDaemon,
			sockPath:    virtiofsdSocketPath,
			apiSockPath: apiSockPath,
			sourcePath:  sharedPath,
			debug:       q.config.Debug,
			extraArgs:   q.config.VirtioFSExtraArgs,
			startFn:     startInShimNS,
		}
		nd.setupShareDirFn = nd.setupPassthroughFS
		return nd, nil
	}

	// Set the xattr option for virtiofsd daemon to enable extended attributes
	// in virtiofs if SELinux on the guest side is enabled.
	if !q.config.DisableGuestSeLinux {
		q.Logger().Info("Set the xattr option for virtiofsd")
		q.config.VirtioFSExtraArgs = append(q.config.VirtioFSExtraArgs, "--xattr")
	}

	// default use virtiofsd
	return &virtiofsd{
		path:       q.config.VirtioFSDaemon,
		sourcePath: sharedPath,
		socketPath: virtiofsdSocketPath,
		extraArgs:  q.config.VirtioFSExtraArgs,
		cache:      q.config.VirtioFSCache,
	}, nil
}

// CreateVM is the Hypervisor VM creation implementation for govmmQemu.
func (q *qemu) CreateVM(ctx context.Context, id string, network Network, hypervisorConfig *HypervisorConfig) error {
	// Save the tracing context
	q.ctx = ctx

	span, ctx := katatrace.Trace(ctx, q.Logger(), "CreateVM", qemuTracingTags, map[string]string{"VM_ID": q.id})
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
		MemPrealloc:   q.config.MemPrealloc,
		HugePages:     q.config.HugePages,
		IOMMUPlatform: q.config.IOMMUPlatform,
	}

	incoming := q.setupTemplate(&knobs, &memory)

	// With the current implementations, VM templating will not work with file
	// based memory (stand-alone) or virtiofs. This is because VM templating
	// builds the first VM with file-backed memory and shared=on and the
	// subsequent ones with shared=off. virtio-fs always requires shared=on for
	// memory.
	if q.config.SharedFS == config.VirtioFS || q.config.SharedFS == config.VirtioFSNydus ||
		q.config.FileBackedMemRootDir != "" {
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

	kernelPath, err := q.config.KernelAssetPath()
	if err != nil {
		return err
	}

	devices, ioThread, kernel, err := q.buildDevices(ctx, kernelPath)
	if err != nil {
		return err
	}

	cpuModel := q.arch.cpuModel()
	cpuModel += "," + q.config.CPUFeatures

	firmwarePath, err := q.config.FirmwareAssetPath()
	if err != nil {
		return err
	}

	firmwareVolumePath, err := q.config.FirmwareVolumeAssetPath()
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

	// some devices configuration may also change kernel params, make sure this is called afterwards
	kernel.Params = q.kernelParameters()
	q.checkBpfEnabled()

	qemuConfig := govmmQemu.Config{
		Name:           fmt.Sprintf("sandbox-%s", q.id),
		UUID:           q.state.UUID,
		Path:           qemuPath,
		Ctx:            q.qmpMonitorCh.ctx,
		Uid:            q.config.Uid,
		Gid:            q.config.Gid,
		Groups:         q.config.Groups,
		Machine:        machine,
		SMP:            smp,
		Memory:         memory,
		Devices:        devices,
		CPUModel:       cpuModel,
		SeccompSandbox: q.config.SeccompSandbox,
		Kernel:         *kernel,
		RTC:            rtc,
		QMPSockets:     qmpSockets,
		Knobs:          knobs,
		Incoming:       incoming,
		VGA:            "none",
		GlobalParam:    "kvm-pit.lost_tick_policy=discard",
		Bios:           firmwarePath,
		PFlash:         pflash,
		PidFile:        filepath.Join(q.config.VMStorePath, q.id, "pid"),
		Debug:          hypervisorConfig.Debug,
	}

	qemuConfig.Devices, qemuConfig.Bios, err = q.arch.appendProtectionDevice(qemuConfig.Devices, firmwarePath, firmwareVolumePath)
	if err != nil {
		return err
	}

	if ioThread != nil {
		qemuConfig.IOThreads = []govmmQemu.IOThread{*ioThread}
	}
	// Add RNG device to hypervisor
	// Skip for s390x as CPACF is used
	if machine.Type != QemuCCWVirtio {
		rngDev := config.RNGDev{
			ID:       rngID,
			Filename: q.config.EntropySource,
		}
		qemuConfig.Devices, err = q.arch.appendRNGDevice(ctx, qemuConfig.Devices, rngDev)
		if err != nil {
			return err
		}
	}

	if machine.Type == QemuQ35 || machine.Type == QemuVirt {
		if err := q.createPCIeTopology(&qemuConfig, hypervisorConfig, machine.Type, network); err != nil {
			q.Logger().WithError(err).Errorf("Cannot create PCIe topology")
			return err
		}
	}
	q.qemuConfig = qemuConfig

	q.virtiofsDaemon, err = q.createVirtiofsDaemon(hypervisorConfig.SharedPath)
	return err
}

func (q *qemu) checkBpfEnabled() {
	if q.config.SeccompSandbox != "" {
		out, err := os.ReadFile("/proc/sys/net/core/bpf_jit_enable")
		if err != nil {
			q.Logger().WithError(err).Warningf("failed to get bpf_jit_enable status")
			return
		}
		enabled, err := strconv.Atoi(strings.TrimSpace(string(out)))
		if err != nil {
			q.Logger().WithError(err).Warningf("failed to convert bpf_jit_enable status to integer")
			return
		}
		if enabled == 0 {
			q.Logger().Warningf("bpf_jit_enable is disabled. " +
				"It's recommended to turn on bpf_jit_enable to reduce the performance impact of QEMU seccomp sandbox.")
		}
	}
}

// If a user uses 8 GPUs with 4 devices in each IOMMU Group that means we need
// to hotplug 32 devices. We do not have enough PCIe root bus slots to
// accomplish this task. Kata will use already some slots for vfio-xxxx-pci
// devices.
// Max PCI slots per root bus is 32
// Max PCIe root ports is 16
// Max PCIe switch ports is 16
// There is only 64kB of IO memory each root,switch port will consume 4k hence
// only 16 ports possible.
func (q *qemu) createPCIeTopology(qemuConfig *govmmQemu.Config, hypervisorConfig *HypervisorConfig, machineType string, network Network) error {

	// If no-port set just return no need to add PCIe Root Port or PCIe Switches
	if hypervisorConfig.HotPlugVFIO == config.NoPort && hypervisorConfig.ColdPlugVFIO == config.NoPort && machineType == QemuQ35 {
		return nil
	}

	// Add PCIe Root Port or PCIe Switches to the hypervisor
	// The pcie.0 bus do not support hot-plug, but PCIe device can be hot-plugged
	// into a PCIe Root Port or PCIe Switch.
	// For more details, please see https://github.com/qemu/qemu/blob/master/docs/pcie.txt

	// Deduce the right values for mem-reserve and pref-64-reserve memory regions
	memSize32bit, memSize64bit := q.arch.getBARsMaxAddressableMemory()

	// The default OVMF MMIO aperture is too small for some PCIe devices
	// with huge BARs so we need to increase it.
	// memSize64bit is in bytes, convert to MB, OVMF expects MB as a string
	if strings.Contains(strings.ToLower(hypervisorConfig.FirmwarePath), "ovmf") {
		pciMmio64Mb := fmt.Sprintf("%d", (memSize64bit / 1024 / 1024))
		fwCfg := govmmQemu.FwCfg{
			Name: "opt/ovmf/X-PciMmio64Mb",
			Str:  pciMmio64Mb,
		}
		qemuConfig.FwCfg = append(qemuConfig.FwCfg, fwCfg)
	}

	// Get the number of hot(cold)-pluggable ports needed from the provided
	// VFIO devices
	var numOfPluggablePorts uint32 = 0

	// Fow now, pcie native hotplug is the only way for Arm to hotadd pci device.
	if machineType == QemuVirt {
		epNum, err := network.GetEndpointsNum()
		if err != nil {
			q.Logger().Warn("Fail to get network endpoints number")
		}
		virtPcieRootPortNum := len(hypervisorConfig.VhostUserBlkDevices) + epNum
		if hypervisorConfig.VirtioMem {
			virtPcieRootPortNum++
		}
		numOfPluggablePorts += uint32(virtPcieRootPortNum)
	}
	for _, dev := range hypervisorConfig.VFIODevices {
		var err error
		dev.HostPath, err = config.GetHostPath(dev, false, "")
		if err != nil {
			return fmt.Errorf("Cannot get host path for device: %v err: %v", dev, err)
		}

		devicesPerIOMMUGroup, err := drivers.GetAllVFIODevicesFromIOMMUGroup(dev)
		if err != nil {
			return fmt.Errorf("Cannot get all VFIO devices from IOMMU group with device: %v err: %v", dev, err)
		}
		for _, vfioDevice := range devicesPerIOMMUGroup {
			if drivers.IsPCIeDevice(vfioDevice.BDF) {
				numOfPluggablePorts = numOfPluggablePorts + 1
			}
		}
	}
	vfioOnRootPort := (q.state.HotPlugVFIO == config.RootPort || q.state.ColdPlugVFIO == config.RootPort)
	vfioOnSwitchPort := (q.state.HotPlugVFIO == config.SwitchPort || q.state.ColdPlugVFIO == config.SwitchPort)

	// If the devices are not advertised via CRI or cold-plugged we need to
	// get the number of pluggable root/switch ports from the config
	numPCIeRootPorts := hypervisorConfig.PCIeRootPort
	numPCIeSwitchPorts := hypervisorConfig.PCIeSwitchPort

	// If number of PCIe root ports > 16 then bail out otherwise we may
	// use up all slots or IO memory on the root bus and vfio-XXX-pci devices
	// cannot be added which are crucial for Kata max slots on root bus is 32
	// max slots on the complete pci(e) topology is 256 in QEMU
	if vfioOnRootPort {
		if numOfPluggablePorts < numPCIeRootPorts {
			numOfPluggablePorts = numPCIeRootPorts
		}
		if numOfPluggablePorts > maxPCIeRootPort {
			return fmt.Errorf("Number of PCIe Root Ports exceeed allowed max of %d", maxPCIeRootPort)
		}
		qemuConfig.Devices = q.arch.appendPCIeRootPortDevice(qemuConfig.Devices, numOfPluggablePorts, memSize32bit, memSize64bit)
		return nil
	}
	if vfioOnSwitchPort {
		if numOfPluggablePorts < numPCIeSwitchPorts {
			numOfPluggablePorts = numPCIeSwitchPorts
		}
		if numOfPluggablePorts > maxPCIeSwitchPort {
			return fmt.Errorf("Number of PCIe Switch Ports exceeed allowed max of %d", maxPCIeSwitchPort)
		}
		qemuConfig.Devices = q.arch.appendPCIeSwitchPortDevice(qemuConfig.Devices, numOfPluggablePorts, memSize32bit, memSize64bit)
		return nil
	}
	// If both Root Port and Switch Port are not enabled, check if QemuVirt need add pcie root port.
	if machineType == QemuVirt {
		qemuConfig.Devices = q.arch.appendPCIeRootPortDevice(qemuConfig.Devices, numOfPluggablePorts, memSize32bit, memSize64bit)
	}
	return nil
}

func (q *qemu) vhostFSSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(q.config.VMStorePath, id, vhostFSSocket)
}

func (q *qemu) nydusdAPISocketPath(id string) (string, error) {
	return utils.BuildSocketPath(q.config.VMStorePath, id, nydusdAPISock)
}

func (q *qemu) setupVirtiofsDaemon(ctx context.Context) (err error) {
	pid, err := q.virtiofsDaemon.Start(ctx, func() {
		q.StopVM(ctx, false)
	})
	if err != nil {
		return err
	}
	q.state.VirtiofsDaemonPid = pid

	return nil
}

func (q *qemu) stopVirtiofsDaemon(ctx context.Context) (err error) {
	if q.state.VirtiofsDaemonPid == 0 {
		q.Logger().Warn("The virtiofsd had stopped")
		return nil
	}

	err = q.virtiofsDaemon.Stop(ctx)
	if err != nil {
		return err
	}
	q.state.VirtiofsDaemonPid = 0
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

		if q.config.SharedFS == config.VirtioFS || q.config.SharedFS == config.VirtioFSNydus ||
			q.config.FileBackedMemRootDir != "" {
			target = q.qemuConfig.Memory.Path
			memoryBack = "memory-backend-file"
		}
	}

	if q.qemuConfig.Knobs.MemShared {
		share = true
	}

	return share, target, memoryBack, nil
}

func (q *qemu) setupVirtioMem(ctx context.Context) error {
	// backend memory size must be multiple of 4Mib
	sizeMB := (int(q.config.DefaultMaxMemorySize) - int(q.config.MemorySize)) >> 2 << 2

	share, target, memoryBack, err := q.getMemArgs()
	if err != nil {
		return err
	}

	if err = q.qmpSetup(); err != nil {
		return err
	}

	addr, bridge, err := q.arch.addDeviceToBridge(ctx, "virtiomem-dev", types.PCI)
	if err != nil {
		return err
	}

	defer func() {
		if err != nil {
			q.arch.removeDeviceFromBridge("virtiomem-dev")
		}
	}()

	bridgeID := bridge.ID

	// Hot add virtioMem dev to pcie-root-port for QemuVirt
	machineType := q.HypervisorConfig().HypervisorMachineType
	if machineType == QemuVirt {
		addr = "00"
		bridgeID = fmt.Sprintf("%s%d", config.PCIeRootPortPrefix, len(config.PCIeDevicesPerPort[config.RootPort]))
		dev := config.VFIODev{ID: "virtiomem"}
		config.PCIeDevicesPerPort[config.RootPort] = append(config.PCIeDevicesPerPort[config.RootPort], dev)
	}

	err = q.qmpMonitorCh.qmp.ExecMemdevAdd(q.qmpMonitorCh.ctx, memoryBack, "virtiomem", target, sizeMB, share, "virtio-mem-pci", "virtiomem0", addr, bridgeID)
	if err == nil {
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

// setupEarlyQmpConnection creates a listener socket to be passed to QEMU
// as a QMP listening endpoint. An initial connection is established, to
// be used as the QMP client socket. This allows to detect an early failure
// of QEMU instead of looping on connect until some timeout expires.
func (q *qemu) setupEarlyQmpConnection() (net.Conn, error) {
	monitorSockPath := q.qmpMonitorCh.path

	qmpListener, err := net.Listen("unix", monitorSockPath)
	if err != nil {
		q.Logger().WithError(err).Errorf("Unable to listen on unix socket address (%s)", monitorSockPath)
		return nil, err
	}

	// A duplicate fd of this socket will be passed to QEMU. We must
	// close the original one when we're done.
	defer qmpListener.Close()

	if rootless.IsRootless() {
		err = syscall.Chown(monitorSockPath, int(q.config.Uid), int(q.config.Gid))
		if err != nil {
			q.Logger().WithError(err).Errorf("Unable to make unix socket (%s) rootless", monitorSockPath)
			return nil, err
		}
	}

	VMFd, err := qmpListener.(*net.UnixListener).File()
	if err != nil {
		return nil, err
	}
	defer func() {
		if err != nil {
			VMFd.Close()
		}
	}()

	// This socket will be used to establish the initial QMP connection
	dialer := net.Dialer{Cancel: q.qmpMonitorCh.ctx.Done()}
	conn, err := dialer.Dial("unix", monitorSockPath)
	if err != nil {
		q.Logger().WithError(err).Errorf("Unable to connect to unix socket (%s)", monitorSockPath)
		return nil, err
	}

	// We need to keep the socket file around to be able to re-connect
	qmpListener.(*net.UnixListener).SetUnlinkOnClose(false)

	// Pass the duplicated fd of the listener socket to QEMU
	q.qemuConfig.QMPSockets[0].FD = VMFd
	q.fds = append(q.fds, q.qemuConfig.QMPSockets[0].FD)

	return conn, nil
}

func (q *qemu) LogAndWait(qemuCmd *exec.Cmd, reader io.ReadCloser) {
	pid := qemuCmd.Process.Pid
	q.Logger().Infof("Start logging QEMU (qemuPid=%d)", pid)
	scanner := bufio.NewScanner(reader)
	warnRE := regexp.MustCompile("(^[^:]+: )warning: ")
	for scanner.Scan() {
		text := scanner.Text()
		if warnRE.MatchString(text) {
			text = warnRE.ReplaceAllString(text, "$1")
			q.Logger().WithField("qemuPid", pid).Warning(text)
		} else {
			q.Logger().WithField("qemuPid", pid).Error(text)
		}
	}
	q.Logger().Infof("Stop logging QEMU (qemuPid=%d)", pid)
	qemuCmd.Wait()
}

// StartVM will start the Sandbox's VM.
func (q *qemu) StartVM(ctx context.Context, timeout int) error {
	span, ctx := katatrace.Trace(ctx, q.Logger(), "StartVM", qemuTracingTags, map[string]string{"sandbox_id": q.id})
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

	vmPath := filepath.Join(q.config.VMStorePath, q.id)
	err := utils.MkdirAllWithInheritedOwner(vmPath, DirMode)
	if err != nil {
		return err
	}
	q.Logger().WithField("vm path", vmPath).Info("created vm path")

	defer func() {
		if err != nil {
			if err := os.RemoveAll(vmPath); err != nil {
				q.Logger().WithError(err).Error("Fail to clean up vm directory")
			}
		}
	}()

	var qmpConn net.Conn
	qmpConn, err = q.setupEarlyQmpConnection()
	if err != nil {
		return err
	}

	// This needs to be done as late as possible, just before launching
	// virtiofsd are executed by kata-runtime after this call, run with
	// the SELinux label. If these processes require privileged, we do
	// notwant to run them under confinement.
	if !q.config.DisableSeLinux {
		if err := label.SetProcessLabel(q.config.SELinuxProcessLabel); err != nil {
			return err
		}
		defer label.SetProcessLabel("")
	}
	if q.config.SharedFS == config.VirtioFS || q.config.SharedFS == config.VirtioFSNydus {
		err = q.setupVirtiofsDaemon(ctx)
		if err != nil {
			return err
		}
		defer func() {
			if err != nil {
				if shutdownErr := q.stopVirtiofsDaemon(ctx); shutdownErr != nil {
					q.Logger().WithError(shutdownErr).Warn("failed to stop virtiofsDaemon")
				}
			}
		}()

	}

	qemuCmd, reader, err := govmmQemu.LaunchQemu(q.qemuConfig, newQMPLogger())
	if err != nil {
		q.Logger().WithError(err).Error("failed to launch qemu")
		return fmt.Errorf("failed to launch qemu: %s", err)
	}

	// Log QEMU errors and ensure the QEMU process is reaped after
	// termination.
	go q.LogAndWait(qemuCmd, reader)

	err = q.waitVM(ctx, qmpConn, timeout)
	if err != nil {
		return err
	}

	if q.config.BootFromTemplate {
		if err = q.bootFromTemplate(); err != nil {
			return err
		}
	}

	if q.config.VirtioMem {
		err = q.setupVirtioMem(ctx)
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

// waitVM will wait for the Sandbox's VM to be up and running.
func (q *qemu) waitVM(ctx context.Context, qmpConn net.Conn, timeout int) error {
	span, _ := katatrace.Trace(ctx, q.Logger(), "waitVM", qemuTracingTags, map[string]string{"sandbox_id": q.id})
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
		qmp, ver, err = govmmQemu.QMPStartWithConn(q.qmpMonitorCh.ctx, qmpConn, cfg, disconnectCh)
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
		"qmp-Capabilities":  strings.Join(ver.Capabilities, ","),
	}).Infof("QMP details")

	if err = q.qmpMonitorCh.qmp.ExecuteQMPCapabilities(q.qmpMonitorCh.ctx); err != nil {
		q.Logger().WithError(err).Error(qmpCapErrMsg)
		return err
	}

	return nil
}

// StopVM will stop the Sandbox's VM.
func (q *qemu) StopVM(ctx context.Context, waitOnly bool) (err error) {
	q.mu.Lock()
	defer q.mu.Unlock()
	span, _ := katatrace.Trace(ctx, q.Logger(), "StopVM", qemuTracingTags, map[string]string{"sandbox_id": q.id})
	defer span.End()

	q.Logger().Info("Stopping Sandbox")
	if atomic.LoadInt32(&q.stopped) != 0 {
		q.Logger().Info("Already stopped")
		return nil
	}

	defer func() {
		q.cleanupVM()
		if err == nil {
			atomic.StoreInt32(&q.stopped, 1)
		}
	}()

	if err := q.qmpSetup(); err != nil {
		return err
	}

	pids := q.GetPids()
	if len(pids) == 0 {
		return errors.New("cannot determine QEMU PID")
	}
	pid := pids[0]
	if pid > 0 {
		if waitOnly {
			err := utils.WaitLocalProcess(pid, qemuStopSandboxTimeoutSecs, syscall.Signal(0), q.Logger())
			if err != nil {
				return err
			}
		} else {
			err = syscall.Kill(pid, syscall.SIGKILL)
			if err != nil {
				q.Logger().WithError(err).Error("Fail to send SIGKILL to qemu")
				return err
			}
		}
	}
	if q.config.SharedFS == config.VirtioFS || q.config.SharedFS == config.VirtioFSNydus {
		if err := q.stopVirtiofsDaemon(ctx); err != nil {
			return err
		}
	}

	return nil
}

func (q *qemu) cleanupVM() error {

	// Cleanup vm path
	dir := filepath.Join(q.config.VMStorePath, q.id)

	// If it's a symlink, remove both dir and the target.
	// This can happen when vm template links a sandbox to a vm.
	link, err := filepath.EvalSymlinks(dir)
	if err != nil {
		// Well, it's just Cleanup failure. Let's ignore it.
		q.Logger().WithError(err).WithField("dir", dir).Warn("failed to resolve vm path")
	}
	q.Logger().WithField("link", link).WithField("dir", dir).Infof("Cleanup vm path")

	if err := os.RemoveAll(dir); err != nil {
		q.Logger().WithError(err).Warnf("failed to remove vm path %s", dir)
	}
	if link != dir && link != "" {
		if err := os.RemoveAll(link); err != nil {
			q.Logger().WithError(err).WithField("link", link).Warn("failed to remove resolved vm path")
		}
	}

	if q.config.VMid != "" {
		dir = filepath.Join(q.config.RunStorePath, q.config.VMid)
		if err := os.RemoveAll(dir); err != nil {
			q.Logger().WithError(err).WithField("path", dir).Warnf("failed to remove vm path")
		}
	}

	if rootless.IsRootless() {
		if _, err := user.Lookup(q.config.User); err != nil {
			q.Logger().WithError(err).WithFields(
				logrus.Fields{
					"user": q.config.User,
					"uid":  q.config.Uid,
				}).Warn("failed to find the user, it might have been removed")
			return nil
		}

		if err := pkgUtils.RemoveVmmUser(q.config.User); err != nil {
			q.Logger().WithError(err).WithFields(
				logrus.Fields{
					"user": q.config.User,
					"uid":  q.config.Uid,
				}).Warn("failed to delete the user")
			return nil
		}
		q.Logger().WithFields(
			logrus.Fields{
				"user": q.config.User,
				"uid":  q.config.Uid,
			}).Debug("successfully removed the non root user")
	}

	return nil
}

func (q *qemu) togglePauseSandbox(ctx context.Context, pause bool) error {
	span, _ := katatrace.Trace(ctx, q.Logger(), "togglePauseSandbox", qemuTracingTags, map[string]string{"sandbox_id": q.id})
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
	statePath := filepath.Join(q.config.RunStorePath, q.id)
	command := []string{"/bin/cp", "-ar", statePath, dumpStatePath}
	q.Logger().WithField("command", command).Info("try to Save sandbox state")
	if output, err := pkgUtils.RunCommandFull(command, true); err != nil {
		q.Logger().WithError(err).WithField("output", output).Error("failed to Save state")
	}
	// Save hypervisor meta information
	fileName := filepath.Join(dumpSavePath, "hypervisor.conf")
	data, _ := json.MarshalIndent(q.config, "", " ")
	if err := os.WriteFile(fileName, data, defaultFilePerms); err != nil {
		q.Logger().WithError(err).WithField("hypervisor.conf", data).Error("write to hypervisor.conf file failed")
	}

	// Save hypervisor version
	hyperVisorVersion, err := pkgUtils.RunCommand([]string{q.config.HypervisorPath, "--version"})
	if err != nil {
		q.Logger().WithError(err).WithField("HypervisorPath", data).Error("failed to get hypervisor version")
	}

	fileName = filepath.Join(dumpSavePath, "hypervisor.version")
	if err := os.WriteFile(fileName, []byte(hyperVisorVersion), defaultFilePerms); err != nil {
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

	// Save meta information for sandbox
	q.dumpSandboxMetaInfo(dumpSavePath)
	q.Logger().Info("dump sandbox meta information completed")

	// Check device free space and estimated dump size
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

func (q *qemu) hotplugAddBlockDevice(ctx context.Context, drive *config.BlockDrive, op Operation, devID string) (err error) {
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

	qblkDevice := govmmQemu.BlockDevice{
		ID:       drive.ID,
		File:     drive.File,
		ReadOnly: drive.ReadOnly,
		AIO:      govmmQemu.BlockDeviceAIO(q.config.BlockDeviceAIO),
	}

	if drive.Swap {
		err = q.qmpMonitorCh.qmp.ExecuteBlockdevAddWithDriverCache(q.qmpMonitorCh.ctx, "file", &qblkDevice, false, false)
	} else if q.config.BlockDeviceCacheSet {
		err = q.qmpMonitorCh.qmp.ExecuteBlockdevAddWithCache(q.qmpMonitorCh.ctx, &qblkDevice, q.config.BlockDeviceCacheDirect, q.config.BlockDeviceCacheNoflush)
	} else {
		err = q.qmpMonitorCh.qmp.ExecuteBlockdevAdd(q.qmpMonitorCh.ctx, &qblkDevice)
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
	case drive.Swap:
		fallthrough
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

		bridgeSlot, err := types.PciSlotFromInt(bridge.Addr)
		if err != nil {
			return err
		}
		devSlot, err := types.PciSlotFromString(addr)
		if err != nil {
			return err
		}
		drive.PCIPath, err = types.PciPathFromSlots(bridgeSlot, devSlot)
		if err != nil {
			return err
		}

		queues := int(q.config.NumVCPUs())

		if err = q.qmpMonitorCh.qmp.ExecutePCIDeviceAdd(q.qmpMonitorCh.ctx, drive.ID, devID, driver, addr, bridge.ID, romFile, queues, true, defaultDisableModern); err != nil {
			return err
		}
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

func (q *qemu) hotplugAddVhostUserBlkDevice(ctx context.Context, vAttr *config.VhostUserDeviceAttrs, op Operation, devID string) (err error) {

	err = q.qmpMonitorCh.qmp.ExecuteCharDevUnixSocketAdd(q.qmpMonitorCh.ctx, vAttr.DevID, vAttr.SocketPath, false, false, vAttr.ReconnectTime)
	if err != nil {
		return err
	}

	defer func() {
		if err != nil {
			q.qmpMonitorCh.qmp.ExecuteChardevDel(q.qmpMonitorCh.ctx, vAttr.DevID)
		}
	}()

	driver := "vhost-user-blk-pci"

	machineType := q.HypervisorConfig().HypervisorMachineType

	switch machineType {
	case QemuVirt:
		//The addr of a dev is corresponding with device:function for PCIe in qemu which starting from 0
		//Since the dev is the first and only one on this bus(root port), it should be 0.
		addr := "00"

		bridgeID := fmt.Sprintf("%s%d", config.PCIeRootPortPrefix, len(config.PCIeDevicesPerPort[config.RootPort]))
		dev := config.VFIODev{ID: devID}
		config.PCIeDevicesPerPort[config.RootPort] = append(config.PCIeDevicesPerPort[config.RootPort], dev)

		bridgeQomPath := fmt.Sprintf("%s%s", qomPathPrefix, bridgeID)
		bridgeSlot, err := q.arch.qomGetSlot(bridgeQomPath, &q.qmpMonitorCh)
		if err != nil {
			return err
		}

		devSlot, err := types.PciSlotFromString(addr)
		if err != nil {
			return err
		}

		vAttr.PCIPath, err = types.PciPathFromSlots(bridgeSlot, devSlot)
		if err != nil {
			return err
		}

		if err = q.qmpMonitorCh.qmp.ExecutePCIVhostUserDevAdd(q.qmpMonitorCh.ctx, driver, devID, vAttr.DevID, addr, bridgeID); err != nil {
			return err
		}

	default:
		addr, bridge, err := q.arch.addDeviceToBridge(ctx, vAttr.DevID, types.PCI)
		if err != nil {
			return err
		}
		defer func() {
			if err != nil {
				q.arch.removeDeviceFromBridge(vAttr.DevID)
			}
		}()

		bridgeSlot, err := types.PciSlotFromInt(bridge.Addr)
		if err != nil {
			return err
		}

		devSlot, err := types.PciSlotFromString(addr)
		if err != nil {
			return err
		}
		vAttr.PCIPath, err = types.PciPathFromSlots(bridgeSlot, devSlot)

		if err = q.qmpMonitorCh.qmp.ExecutePCIVhostUserDevAdd(q.qmpMonitorCh.ctx, driver, devID, vAttr.DevID, addr, bridge.ID); err != nil {
			return err
		}
	}
	return nil
}

func (q *qemu) hotplugBlockDevice(ctx context.Context, drive *config.BlockDrive, op Operation) error {
	if err := q.qmpSetup(); err != nil {
		return err
	}

	devID := "virtio-" + drive.ID

	if op == AddDevice {
		return q.hotplugAddBlockDevice(ctx, drive, op, devID)
	}
	if !drive.Swap && q.config.BlockDeviceDriver == config.VirtioBlock {
		if err := q.arch.removeDeviceFromBridge(drive.ID); err != nil {
			return err
		}
	}

	if err := q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, devID); err != nil {
		return err
	}

	return q.qmpMonitorCh.qmp.ExecuteBlockdevDel(q.qmpMonitorCh.ctx, drive.ID)
}

func (q *qemu) hotplugVhostUserDevice(ctx context.Context, vAttr *config.VhostUserDeviceAttrs, op Operation) error {
	if err := q.qmpSetup(); err != nil {
		return err
	}

	devID := "virtio-" + vAttr.DevID

	if op == AddDevice {
		switch vAttr.Type {
		case config.VhostUserBlk:
			return q.hotplugAddVhostUserBlkDevice(ctx, vAttr, op, devID)
		default:
			return fmt.Errorf("Incorrect vhost-user device type found")
		}
	} else {

		machineType := q.HypervisorConfig().HypervisorMachineType

		if machineType != QemuVirt {
			if err := q.arch.removeDeviceFromBridge(vAttr.DevID); err != nil {
				return err
			}
		}

		if err := q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, devID); err != nil {
			return err
		}

		return q.qmpMonitorCh.qmp.ExecuteChardevDel(q.qmpMonitorCh.ctx, vAttr.DevID)
	}
}

func (q *qemu) hotplugVFIODeviceRootPort(ctx context.Context, device *config.VFIODev) (err error) {
	return q.executeVFIODeviceAdd(device)
}

func (q *qemu) hotplugVFIODeviceSwitchPort(ctx context.Context, device *config.VFIODev) (err error) {
	return q.executeVFIODeviceAdd(device)
}

func (q *qemu) hotplugVFIODeviceBridgePort(ctx context.Context, device *config.VFIODev) (err error) {
	addr, bridge, err := q.arch.addDeviceToBridge(ctx, device.ID, types.PCI)
	if err != nil {
		return err
	}

	defer func() {
		if err != nil {
			q.arch.removeDeviceFromBridge(device.ID)
		}
	}()
	return q.executePCIVFIODeviceAdd(device, addr, bridge.ID)
}

func (q *qemu) executePCIVFIODeviceAdd(device *config.VFIODev, addr string, bridgeID string) error {
	switch device.Type {
	case config.VFIOPCIDeviceNormalType:
		return q.qmpMonitorCh.qmp.ExecutePCIVFIODeviceAdd(q.qmpMonitorCh.ctx, device.ID, device.BDF, addr, bridgeID, romFile)
	case config.VFIOPCIDeviceMediatedType:
		return q.qmpMonitorCh.qmp.ExecutePCIVFIOMediatedDeviceAdd(q.qmpMonitorCh.ctx, device.ID, device.SysfsDev, addr, bridgeID, romFile)
	case config.VFIOAPDeviceMediatedType:
		return q.qmpMonitorCh.qmp.ExecuteAPVFIOMediatedDeviceAdd(q.qmpMonitorCh.ctx, device.SysfsDev, device.ID)
	default:
		return fmt.Errorf("Incorrect VFIO device type found")
	}
}

func (q *qemu) executeVFIODeviceAdd(device *config.VFIODev) error {
	switch device.Type {
	case config.VFIOPCIDeviceNormalType:
		return q.qmpMonitorCh.qmp.ExecuteVFIODeviceAdd(q.qmpMonitorCh.ctx, device.ID, device.BDF, device.Bus, romFile)
	case config.VFIOPCIDeviceMediatedType:
		return q.qmpMonitorCh.qmp.ExecutePCIVFIOMediatedDeviceAdd(q.qmpMonitorCh.ctx, device.ID, device.SysfsDev, "", device.Bus, romFile)
	case config.VFIOAPDeviceMediatedType:
		return q.qmpMonitorCh.qmp.ExecuteAPVFIOMediatedDeviceAdd(q.qmpMonitorCh.ctx, device.SysfsDev, device.ID)
	default:
		return fmt.Errorf("Incorrect VFIO device type found")
	}
}

func (q *qemu) hotplugVFIODevice(ctx context.Context, device *config.VFIODev, op Operation) (err error) {
	if err = q.qmpSetup(); err != nil {
		return err
	}

	if op == AddDevice {
		buf, _ := json.Marshal(device)
		q.Logger().WithFields(logrus.Fields{
			"machine-type":  q.HypervisorConfig().HypervisorMachineType,
			"hot-plug-vfio": q.state.HotPlugVFIO,
			"device-info":   string(buf),
		}).Info("Start hot-plug VFIO device")

		err = fmt.Errorf("Incorrect hot plug configuration %v for device %v found", q.state.HotPlugVFIO, device)
		// In case HotplugVFIOOnRootBus is true, devices are hotplugged on the root bus
		// for pc machine type instead of bridge. This is useful for devices that require
		// a large PCI BAR which is a currently a limitation with PCI bridges.
		if q.state.HotPlugVFIO == config.RootPort {
			err = q.hotplugVFIODeviceRootPort(ctx, device)
		} else if q.state.HotPlugVFIO == config.SwitchPort {
			err = q.hotplugVFIODeviceSwitchPort(ctx, device)
		} else if q.state.HotPlugVFIO == config.BridgePort {
			err = q.hotplugVFIODeviceBridgePort(ctx, device)
		}
		if err != nil {
			return err
		}

		// Depending on whether we're doing root port or
		// bridge hotplug, and how the bridge is set up in
		// other parts of the code, we may or may not already
		// have information about the slot number of the
		// bridge and or the device.  For simplicity, just
		// query both of them back from qemu based on the arch
		device.GuestPciPath, err = q.arch.qomGetPciPath(device.ID, &q.qmpMonitorCh)

		return err
	} else {

		q.Logger().WithField("dev-id", device.ID).Info("Start hot-unplug VFIO device")

		if q.state.HotPlugVFIO == config.BridgePort {
			if err := q.arch.removeDeviceFromBridge(device.ID); err != nil {
				return err
			}
		}

		return q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, device.ID)
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

func (q *qemu) hotplugNetDevice(ctx context.Context, endpoint Endpoint, op Operation) (err error) {
	if err = q.qmpSetup(); err != nil {
		return err
	}
	var tap TapInterface

	switch endpoint.Type() {
	case VethEndpointType, IPVlanEndpointType, MacvlanEndpointType, TuntapEndpointType:
		tap = endpoint.NetworkPair().TapInterface
	case TapEndpointType:
		drive := endpoint.(*TapEndpoint)
		tap = drive.TapInterface
	default:
		return fmt.Errorf("this endpoint is not supported")
	}

	devID := "virtio-" + tap.ID
	machineType := q.HypervisorConfig().HypervisorMachineType
	if op == AddDevice {
		if err = q.hotAddNetDevice(tap.Name, endpoint.HardwareAddr(), tap.VMFds, tap.VhostFds); err != nil {
			return err
		}

		defer func() {
			if err != nil {
				q.qmpMonitorCh.qmp.ExecuteNetdevDel(q.qmpMonitorCh.ctx, tap.Name)
			}
		}()

		// Hotplug net dev to pcie root port for QemuVirt
		if machineType == QemuVirt {
			addr := "00"
			bridgeID := fmt.Sprintf("%s%d", config.PCIeRootPortPrefix, len(config.PCIeDevicesPerPort[config.RootPort]))
			dev := config.VFIODev{ID: devID}
			config.PCIeDevicesPerPort[config.RootPort] = append(config.PCIeDevicesPerPort[config.RootPort], dev)

			return q.qmpMonitorCh.qmp.ExecuteNetPCIDeviceAdd(q.qmpMonitorCh.ctx, tap.Name, devID, endpoint.HardwareAddr(), addr, bridgeID, romFile, int(q.config.NumVCPUs()), defaultDisableModern)
		}

		addr, bridge, err := q.arch.addDeviceToBridge(ctx, tap.ID, types.PCI)
		if err != nil {
			return err
		}

		defer func() {
			if err != nil {
				q.arch.removeDeviceFromBridge(tap.ID)
			}
		}()

		bridgeSlot, err := types.PciSlotFromInt(bridge.Addr)
		if err != nil {
			return err
		}
		devSlot, err := types.PciSlotFromString(addr)
		if err != nil {
			return err
		}
		pciPath, err := types.PciPathFromSlots(bridgeSlot, devSlot)
		endpoint.SetPciPath(pciPath)

		var machine govmmQemu.Machine
		machine, err = q.getQemuMachine()
		if err != nil {
			return err
		}
		if machine.Type == QemuCCWVirtio {
			devNoHotplug := fmt.Sprintf("fe.%x.%x", bridge.Addr, addr)
			return q.qmpMonitorCh.qmp.ExecuteNetCCWDeviceAdd(q.qmpMonitorCh.ctx, tap.Name, devID, endpoint.HardwareAddr(), devNoHotplug, int(q.config.NumVCPUs()))
		}
		return q.qmpMonitorCh.qmp.ExecuteNetPCIDeviceAdd(q.qmpMonitorCh.ctx, tap.Name, devID, endpoint.HardwareAddr(), addr, bridge.ID, romFile, int(q.config.NumVCPUs()), defaultDisableModern)
	}

	if err := q.arch.removeDeviceFromBridge(tap.ID); err != nil {
		return err
	}

	if err := q.qmpMonitorCh.qmp.ExecuteDeviceDel(q.qmpMonitorCh.ctx, devID); err != nil {
		return err
	}

	return q.qmpMonitorCh.qmp.ExecuteNetdevDel(q.qmpMonitorCh.ctx, tap.Name)
}

func (q *qemu) hotplugDevice(ctx context.Context, devInfo interface{}, devType DeviceType, op Operation) (interface{}, error) {
	switch devType {
	case BlockDev:
		drive := devInfo.(*config.BlockDrive)
		return nil, q.hotplugBlockDevice(ctx, drive, op)
	case CpuDev:
		vcpus := devInfo.(uint32)
		return q.hotplugCPUs(vcpus, op)
	case VfioDev:
		device := devInfo.(*config.VFIODev)
		return nil, q.hotplugVFIODevice(ctx, device, op)
	case MemoryDev:
		memdev := devInfo.(*MemoryDevice)
		return q.hotplugMemory(memdev, op)
	case NetDev:
		device := devInfo.(Endpoint)
		return nil, q.hotplugNetDevice(ctx, device, op)
	case VhostuserDev:
		vAttr := devInfo.(*config.VhostUserDeviceAttrs)
		return nil, q.hotplugVhostUserDevice(ctx, vAttr, op)
	default:
		return nil, fmt.Errorf("cannot hotplug device: unsupported device type '%v'", devType)
	}
}

func (q *qemu) HotplugAddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	span, ctx := katatrace.Trace(ctx, q.Logger(), "HotplugAddDevice", qemuTracingTags)
	katatrace.AddTags(span, "sandbox_id", q.id, "device", devInfo)
	defer span.End()

	data, err := q.hotplugDevice(ctx, devInfo, devType, AddDevice)
	if err != nil {
		return data, err
	}

	return data, nil
}

func (q *qemu) HotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	span, ctx := katatrace.Trace(ctx, q.Logger(), "HotplugRemoveDevice", qemuTracingTags)
	katatrace.AddTags(span, "sandbox_id", q.id, "device", devInfo)
	defer span.End()

	data, err := q.hotplugDevice(ctx, devInfo, devType, RemoveDevice)
	if err != nil {
		return data, err
	}

	return data, nil
}

func (q *qemu) hotplugCPUs(vcpus uint32, op Operation) (uint32, error) {
	if vcpus == 0 {
		q.Logger().Warnf("cannot hotplug 0 vCPUs")
		return 0, nil
	}

	if err := q.qmpSetup(); err != nil {
		return 0, err
	}

	if op == AddDevice {
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

		// If CPU type is IBM pSeries, Z or arm virt, we do not set socketID and threadID
		if machine.Type == "pseries" || machine.Type == QemuCCWVirtio || machine.Type == "virt" {
			socketID = ""
			threadID = ""
			dieID = ""
		}

		if err := q.qmpMonitorCh.qmp.ExecuteCPUDeviceAdd(q.qmpMonitorCh.ctx, driver, cpuID, socketID, dieID, coreID, threadID, romFile); err != nil {
			q.Logger().WithField("hotplug", "cpu").Warnf("qmp hotplug cpu, cpuID=%s socketID=%s, error: %v", cpuID, socketID, err)
			// don't fail, let's try with other CPU
			continue
		}

		// a new vCPU was added, update list of hotplugged vCPUs and Check if all vCPUs were added
		q.state.HotpluggedVCPUs = append(q.state.HotpluggedVCPUs, hv.CPUDevice{ID: cpuID})
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

func (q *qemu) hotplugMemory(memDev *MemoryDevice, op Operation) (int, error) {

	if !q.arch.supportGuestMemoryHotplug() {
		return 0, noGuestMemHotplugErr
	}
	if memDev.SizeMB < 0 {
		return 0, fmt.Errorf("cannot hotplug negative size (%d) memory", memDev.SizeMB)
	}
	memLog := q.Logger().WithField("hotplug", "memory")

	memLog.WithField("hotplug-memory-mb", memDev.SizeMB).Debug("requested memory hotplug")
	if err := q.qmpSetup(); err != nil {
		return 0, err
	}

	if memDev.SizeMB == 0 {
		memLog.Debug("hotplug is not required")
		return 0, nil
	}

	switch op {
	case RemoveDevice:
		memLog.WithField("operation", "remove").Debugf("Requested to remove memory: %d MB", memDev.SizeMB)
		// Dont fail but warn that this is not supported.
		memLog.Warn("hot-remove VM memory not supported")
		return 0, nil
	case AddDevice:
		memLog.WithField("operation", "add").Debugf("Requested to add memory: %d MB", memDev.SizeMB)

		memoryAdded, err := q.hotplugAddMemory(memDev)
		if err != nil {
			return memoryAdded, err
		}
		return memoryAdded, nil
	default:
		return 0, fmt.Errorf("invalid operation %v", op)
	}

}

func (q *qemu) hotplugAddMemory(memDev *MemoryDevice) (int, error) {
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
		memDev.Slot = maxSlot + 1
	}

	share, target, memoryBack, err := q.getMemArgs()
	if err != nil {
		return 0, err
	}

	err = q.qmpMonitorCh.qmp.ExecHotplugMemory(q.qmpMonitorCh.ctx, memoryBack, "mem"+strconv.Itoa(memDev.Slot), target, memDev.SizeMB, share)
	if err != nil {
		q.Logger().WithError(err).Error("hotplug memory")
		return 0, err
	}
	// if guest kernel only supports memory hotplug via probe interface, we need to get address of hot-add memory device
	if memDev.Probe {
		memoryDevices, err := q.qmpMonitorCh.qmp.ExecQueryMemoryDevices(q.qmpMonitorCh.ctx)
		if err != nil {
			return 0, fmt.Errorf("failed to query memory devices: %v", err)
		}
		if len(memoryDevices) != 0 {
			q.Logger().WithField("addr", fmt.Sprintf("0x%x", memoryDevices[len(memoryDevices)-1].Data.Addr)).Debug("recently hot-add memory device")
			memDev.Addr = memoryDevices[len(memoryDevices)-1].Data.Addr
		} else {
			return 0, fmt.Errorf("failed to probe address of recently hot-add memory device, no device exists")
		}
	}
	q.state.HotpluggedMemory += memDev.SizeMB
	return memDev.SizeMB, nil
}

func (q *qemu) PauseVM(ctx context.Context) error {
	span, ctx := katatrace.Trace(ctx, q.Logger(), "PauseVM", qemuTracingTags, map[string]string{"sandbox_id": q.id})
	defer span.End()

	return q.togglePauseSandbox(ctx, true)
}

func (q *qemu) ResumeVM(ctx context.Context) error {
	span, ctx := katatrace.Trace(ctx, q.Logger(), "ResumeVM", qemuTracingTags, map[string]string{"sandbox_id": q.id})
	defer span.End()

	return q.togglePauseSandbox(ctx, false)
}

// AddDevice will add extra devices to Qemu command line.
func (q *qemu) AddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) error {
	var err error
	span, _ := katatrace.Trace(ctx, q.Logger(), "AddDevice", qemuTracingTags)
	katatrace.AddTags(span, "sandbox_id", q.id, "device", devInfo)
	defer span.End()

	switch v := devInfo.(type) {
	case types.Volume:
		if q.config.SharedFS == config.VirtioFS || q.config.SharedFS == config.VirtioFSNydus {
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
				QueueSize: q.config.VirtioFSQueueSize,
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

// GetVMConsole builds the path of the console where we can read logs coming
// from the sandbox.
func (q *qemu) GetVMConsole(ctx context.Context, id string) (string, string, error) {
	span, _ := katatrace.Trace(ctx, q.Logger(), "GetVMConsole", qemuTracingTags, map[string]string{"sandbox_id": q.id})
	defer span.End()

	consoleURL, err := utils.BuildSocketPath(q.config.VMStorePath, id, consoleSocket)
	if err != nil {
		return consoleProtoUnix, "", err
	}

	return consoleProtoUnix, consoleURL, nil
}

func (q *qemu) SaveVM() error {
	q.Logger().Info("Save sandbox")

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

func (q *qemu) Disconnect(ctx context.Context) {
	span, _ := katatrace.Trace(ctx, q.Logger(), "Disconnect", qemuTracingTags, map[string]string{"sandbox_id": q.id})
	defer span.End()

	q.qmpShutdown()
}

func (q *qemu) GetTotalMemoryMB(ctx context.Context) uint32 {
	return q.config.MemorySize + uint32(q.state.HotpluggedMemory)
}

// ResizeMemory gets a request to update the VM memory to reqMemMB
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
func (q *qemu) ResizeMemory(ctx context.Context, reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, MemoryDevice, error) {

	currentMemory := q.GetTotalMemoryMB(ctx)
	if err := q.qmpSetup(); err != nil {
		return 0, MemoryDevice{}, err
	}
	var addMemDevice MemoryDevice
	if q.config.VirtioMem && currentMemory != reqMemMB {
		q.Logger().WithField("hotplug", "memory").Debugf("resize memory from %dMB to %dMB", currentMemory, reqMemMB)
		sizeByte := uint64(reqMemMB - q.config.MemorySize)
		sizeByte = sizeByte * 1024 * 1024
		err := q.qmpMonitorCh.qmp.ExecQomSet(q.qmpMonitorCh.ctx, "virtiomem0", "requested-size", sizeByte)
		if err != nil {
			return 0, MemoryDevice{}, err
		}
		q.state.HotpluggedMemory = int(sizeByte / 1024 / 1024)
		return reqMemMB, MemoryDevice{}, nil
	}

	switch {
	case currentMemory < reqMemMB:
		//hotplug
		addMemMB := reqMemMB - currentMemory

		if currentMemory+addMemMB > uint32(q.config.DefaultMaxMemorySize) {
			addMemMB = uint32(q.config.DefaultMaxMemorySize) - currentMemory
		}

		memHotplugMB, err := calcHotplugMemMiBSize(addMemMB, memoryBlockSizeMB)
		if err != nil {
			return currentMemory, MemoryDevice{}, err
		}

		addMemDevice.SizeMB = int(memHotplugMB)
		addMemDevice.Probe = probe

		data, err := q.HotplugAddDevice(ctx, &addMemDevice, MemoryDev)
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
			return currentMemory, MemoryDevice{}, err
		}

		addMemDevice.SizeMB = int(memHotunplugMB)
		addMemDevice.Probe = probe

		data, err := q.HotplugRemoveDevice(ctx, &addMemDevice, MemoryDev)
		if err != nil {
			return currentMemory, addMemDevice, err
		}
		memoryRemoved, ok := data.(int)
		if !ok {
			return currentMemory, addMemDevice, fmt.Errorf("Could not get the memory removed, got %+v", data)
		}
		//FIXME: This is to Check memory HotplugRemoveDevice reported 0, as this is not supported.
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
				SHPC:    false,
				Addr:    strconv.FormatInt(int64(bridges[idx].Addr), 10),
				// Certain guest BIOS versions think
				// !SHPC means no hotplug, and won't
				// reserve the IO and memory windows
				// that will be needed for devices
				// added underneath this bridge.  This
				// will only break for certain
				// combinations of exact qemu, BIOS
				// and guest kernel versions, but for
				// consistency, just hint the usual
				// default windows for a bridge (as
				// the BIOS would use with SHPC) so
				// that we can do ACPI hotplug.
				IOReserve:     "4k",
				MemReserve:    "1m",
				Pref64Reserve: "1m",
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
func genericMemoryTopology(memoryMb, hostMemoryMb uint64, slots uint8, memoryOffset uint64) govmmQemu.Memory {
	// image NVDIMM device needs memory space 1024MB
	// See https://github.com/clearcontainers/runtime/issues/380
	memoryOffset += 1024

	memMax := fmt.Sprintf("%dM", hostMemoryMb+memoryOffset)

	mem := fmt.Sprintf("%dM", memoryMb)

	memory := govmmQemu.Memory{
		Size:   mem,
		Slots:  slots,
		MaxMem: memMax,
	}

	return memory
}

// genericAppendPCIeRootPort appends to devices the given pcie-root-port
func genericAppendPCIeRootPort(devices []govmmQemu.Device, number uint32, machineType string, memSize32bit uint64, memSize64bit uint64) []govmmQemu.Device {
	var (
		bus           string
		chassis       string
		multiFunction bool
		addr          string
	)
	switch machineType {
	case QemuQ35, QemuVirt:
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
				ID:            fmt.Sprintf("%s%d", config.PCIeRootPortPrefix, i),
				Bus:           bus,
				Chassis:       chassis,
				Slot:          strconv.FormatUint(uint64(i), 10),
				Multifunction: multiFunction,
				Addr:          addr,
				MemReserve:    fmt.Sprintf("%dB", memSize32bit),
				Pref64Reserve: fmt.Sprintf("%dB", memSize64bit),
			},
		)
	}
	return devices
}

// gollangci-lint enforces multi-line comments to be a block comment
// not multiple single line comments ...
/*  pcie.0 bus
//  -------------------------------------------------
//                           |
//                     -------------
//                     | Root Port |
//                     -------------
//  -------------------------|------------------------
//  |                 -----------------              |
//  |    PCI Express  | Upstream Port |              |
//  |      Switch     -----------------              |
//  |                  |            |                |
//  |    -------------------    -------------------  |
//  |    | Downstream Port |    | Downstream Port |  |
//  |    -------------------    -------------------  |
//  -------------|-----------------------|------------
//          -------------           --------------
//          | GPU/ACCEL |           | IB/ETH NIC |
//          -------------           --------------
*/
// genericAppendPCIeSwitch adds a PCIe Swtich
func genericAppendPCIeSwitchPort(devices []govmmQemu.Device, number uint32, machineType string, memSize32bit uint64, memSize64bit uint64) []govmmQemu.Device {

	// Q35, Virt have the correct PCIe support,
	// hence ignore all other machines
	if machineType != QemuQ35 && machineType != QemuVirt {
		return devices
	}

	// Using an own ID for the root port, so we do not clash with already
	// existing root ports adding "s" for switch prefix
	pcieRootPort := govmmQemu.PCIeRootPortDevice{
		ID:            fmt.Sprintf("%s%s%d", config.PCIeSwitchPortPrefix, config.PCIeRootPortPrefix, 0),
		Bus:           defaultBridgeBus,
		Chassis:       "1",
		Slot:          strconv.FormatUint(uint64(0), 10),
		Multifunction: false,
		Addr:          "0",
		MemReserve:    fmt.Sprintf("%dB", memSize32bit),
		Pref64Reserve: fmt.Sprintf("%dB", memSize64bit),
	}

	devices = append(devices, pcieRootPort)

	pcieSwitchUpstreamPort := govmmQemu.PCIeSwitchUpstreamPortDevice{
		ID:  fmt.Sprintf("%s%d", config.PCIeSwitchUpstreamPortPrefix, 0),
		Bus: pcieRootPort.ID,
	}
	devices = append(devices, pcieSwitchUpstreamPort)

	currentChassis, err := strconv.Atoi(pcieRootPort.Chassis)
	if err != nil {
		return devices
	}
	nextChassis := currentChassis + 1

	for i := uint32(0); i < number; i++ {

		pcieSwitchDownstreamPort := govmmQemu.PCIeSwitchDownstreamPortDevice{
			ID:      fmt.Sprintf("%s%d", config.PCIeSwitchhDownstreamPortPrefix, i),
			Bus:     pcieSwitchUpstreamPort.ID,
			Chassis: fmt.Sprintf("%d", nextChassis),
			Slot:    strconv.FormatUint(uint64(i), 10),
			// TODO: MemReserve:    fmt.Sprintf("%dB", memSize32bit),
			// TODO: Pref64Reserve: fmt.Sprintf("%dB", memSize64bit),
		}
		devices = append(devices, pcieSwitchDownstreamPort)
	}

	return devices
}

func (q *qemu) GetThreadIDs(ctx context.Context) (VcpuThreadIDs, error) {
	span, _ := katatrace.Trace(ctx, q.Logger(), "GetThreadIDs", qemuTracingTags, map[string]string{"sandbox_id": q.id})
	defer span.End()

	tid := VcpuThreadIDs{}
	if err := q.qmpSetup(); err != nil {
		return tid, err
	}

	cpuInfos, err := q.qmpMonitorCh.qmp.ExecQueryCpusFast(q.qmpMonitorCh.ctx)
	if err != nil {
		q.Logger().WithError(err).Error("failed to query cpu infos")
		return tid, err
	}

	tid.vcpus = make(map[int]int, len(cpuInfos))
	for _, i := range cpuInfos {
		if i.ThreadID > 0 {
			tid.vcpus[i.CPUIndex] = i.ThreadID
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

func (q *qemu) ResizeVCPUs(ctx context.Context, reqVCPUs uint32) (currentVCPUs uint32, newVCPUs uint32, err error) {
	currentVCPUs = q.config.NumVCPUs() + uint32(len(q.state.HotpluggedVCPUs))
	newVCPUs = currentVCPUs

	switch {
	case currentVCPUs < reqVCPUs:
		//hotplug
		addCPUs := reqVCPUs - currentVCPUs
		data, err := q.HotplugAddDevice(ctx, addCPUs, CpuDev)
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
		data, err := q.HotplugRemoveDevice(ctx, removeCPUs, CpuDev)
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

func (q *qemu) Cleanup(ctx context.Context) error {
	span, _ := katatrace.Trace(ctx, q.Logger(), "Cleanup", qemuTracingTags, map[string]string{"sandbox_id": q.id})
	defer span.End()

	for _, fd := range q.fds {
		if err := fd.Close(); err != nil {
			q.Logger().WithError(err).Warn("failed closing fd")
		}
	}
	q.fds = []*os.File{}

	return nil
}

func (q *qemu) GetPids() []int {
	data, err := os.ReadFile(q.qemuConfig.PidFile)
	if err != nil {
		q.Logger().WithError(err).Error("Could not read qemu pid file")
		return []int{0}
	}

	pid, err := strconv.Atoi(strings.Trim(string(data), "\n\t "))
	if err != nil {
		q.Logger().WithError(err).Error("Could not convert string to int")
		return []int{0}
	}

	pids := []int{pid}
	if q.state.VirtiofsDaemonPid != 0 {
		pids = append(pids, q.state.VirtiofsDaemonPid)
	}

	return pids
}

func (q *qemu) GetVirtioFsPid() *int {
	return &q.state.VirtiofsDaemonPid
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

	q.Cleanup(ctx)
	qp := qemuGrpc{
		ID:             q.id,
		QmpChannelpath: q.qmpMonitorCh.path,
		State:          q.state,
		NvdimmCount:    q.nvdimmCount,

		QemuSMP: q.qemuConfig.SMP,
	}

	return json.Marshal(&qp)
}

func (q *qemu) Save() (s hv.HypervisorState) {

	// If QEMU isn't even running, there isn't any state to Save
	if atomic.LoadInt32(&q.stopped) != 0 {
		return
	}

	pids := q.GetPids()
	if len(pids) != 0 {
		s.Pid = pids[0]
	}
	s.VirtiofsDaemonPid = q.state.VirtiofsDaemonPid
	s.Type = string(QemuHypervisor)
	s.UUID = q.state.UUID
	s.HotpluggedMemory = q.state.HotpluggedMemory

	for _, bridge := range q.arch.getBridges() {
		s.Bridges = append(s.Bridges, hv.Bridge{
			DeviceAddr: bridge.Devices,
			Type:       string(bridge.Type),
			ID:         bridge.ID,
			Addr:       bridge.Addr,
		})
	}

	for _, cpu := range q.state.HotpluggedVCPUs {
		s.HotpluggedVCPUs = append(s.HotpluggedVCPUs, hv.CPUDevice{
			ID: cpu.ID,
		})
	}
	return
}

func (q *qemu) Load(s hv.HypervisorState) {
	q.state.UUID = s.UUID
	q.state.HotpluggedMemory = s.HotpluggedMemory
	q.state.VirtiofsDaemonPid = s.VirtiofsDaemonPid

	for _, bridge := range s.Bridges {
		q.state.Bridges = append(q.state.Bridges, types.NewBridge(types.Type(bridge.Type), bridge.ID, bridge.DeviceAddr, bridge.Addr))
	}

	for _, cpu := range s.HotpluggedVCPUs {
		q.state.HotpluggedVCPUs = append(q.state.HotpluggedVCPUs, hv.CPUDevice{
			ID: cpu.ID,
		})
	}
}

func (q *qemu) Check() error {
	if atomic.LoadInt32(&q.stopped) != 0 {
		return fmt.Errorf("qemu is not running")
	}

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

func (q *qemu) GenerateSocket(id string) (interface{}, error) {
	return generateVMSocket(id, q.config.VMStorePath)
}

func (q *qemu) IsRateLimiterBuiltin() bool {
	return false
}
