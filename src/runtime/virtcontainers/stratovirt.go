//go:build linux

// Copyright (c) 2023 Huawei Technologies Co.,Ltd.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
	hv "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/uuid"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"

	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
)

// stratovirtTracingTags defines tags for the trace span
var stratovirtTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "hypervisor",
	"type":      "stratovirt",
}

// Constants and type definitions related to StratoVirt hypervisor
const (
	stratovirtStopSandboxTimeoutSecs              = 15
	defaultStratoVirt                             = "/usr/bin/stratovirt"
	defaultStratoVirtMachineType                  = "microvm"
	apiSocket                                     = "qmp.socket"
	debugSocket                                   = "console.socket"
	virtiofsSocket                                = "virtiofs_kata.socket"
	nydusdSock                                    = "nydusd_kata.socket"
	maxMmioBlkCount                               = 4
	machineTypeMicrovm                            = "microvm"
	mmioBus                          VirtioDriver = "mmio"
)

var defaultKernelParames = []Param{
	{"reboot", "k"},
	{"panic", "1"},
	{"net.ifnames", "0"},
	{"ramdom.trust_cpu", "on"},
}

var defaultMicroVMParames = []Param{
	{"pci", "off"},
	{"iommu", "off"},
	{"acpi", "off"},
}

var (
	blkDriver = map[VirtioDriver]string{
		mmioBus: "virtio-blk-device",
	}
	netDriver = map[VirtioDriver]string{
		mmioBus: "virtio-net-device",
	}
	virtiofsDriver = map[VirtioDriver]string{
		mmioBus: "vhost-user-fs-device",
	}
	vsockDriver = map[VirtioDriver]string{
		mmioBus: "vhost-vsock-device",
	}
	rngDriver = map[VirtioDriver]string{
		mmioBus: "virtio-rng-device",
	}
	consoleDriver = map[VirtioDriver]string{
		mmioBus: "virtio-serial-device",
	}
)

// VirtioDev is the StratoVirt device interface.
type VirtioDev interface {
	getParams(config *StratovirtConfig) []string
}

type VirtioDriver string

type blkDevice struct {
	id       string
	filePath string
	driver   VirtioDriver
	deviceID string
}

func (b blkDevice) getParams(config *StratovirtConfig) []string {
	var params []string
	var driveParams []Param
	var devParams []Param

	driver := blkDriver[b.driver]
	driveParams = append(driveParams, Param{"id", b.id})
	driveParams = append(driveParams, Param{"file", b.filePath})
	driveParams = append(driveParams, Param{"readonly", "on"})
	driveParams = append(driveParams, Param{"direct", "off"})

	devParams = append(devParams, Param{"drive", b.id})
	devParams = append(devParams, Param{"id", b.deviceID})

	params = append(params, "-drive", strings.Join(SerializeParams(driveParams, "="), ","))
	params = append(params, "-device", fmt.Sprintf("%s,%s", driver, strings.Join(SerializeParams(devParams, "="), ",")))
	return params
}

type netDevice struct {
	devType  string
	id       string
	ifname   string
	driver   VirtioDriver
	netdev   string
	deviceID string
	FDs      []*os.File
	mac      string
}

func (n netDevice) getParams(config *StratovirtConfig) []string {
	var params []string
	var netdevParams []Param
	var devParams []Param

	driver := netDriver[n.driver]
	netdevParams = append(netdevParams, Param{"id", n.id})
	if len(n.FDs) > 0 {
		var fdParams []string

		FDs := config.appendFDs(n.FDs)
		for _, fd := range FDs {
			fdParams = append(fdParams, fmt.Sprintf("%d", fd))
		}
		netdevParams = append(netdevParams, Param{"fds", strings.Join(fdParams, ":")})
	} else if n.ifname != "" {
		netdevParams = append(netdevParams, Param{"ifname", n.ifname})
	}

	devParams = append(devParams, Param{"netdev", n.id})
	devParams = append(devParams, Param{"id", n.deviceID})
	if n.mac != "" {
		devParams = append(devParams, Param{"mac", n.mac})
	}

	params = append(params, "-netdev", fmt.Sprintf("%s,%s", n.devType, strings.Join(SerializeParams(netdevParams, "="), ",")))
	params = append(params, "-device", fmt.Sprintf("%s,%s", driver, strings.Join(SerializeParams(devParams, "="), ",")))
	return params
}

type virtioFs struct {
	driver   VirtioDriver
	backend  string
	charID   string
	charDev  string
	tag      string
	deviceID string
}

func (v virtioFs) getParams(config *StratovirtConfig) []string {
	var params []string
	var charParams []Param
	var fsParams []Param

	driver := virtiofsDriver[v.driver]
	charParams = append(charParams, Param{"id", v.charID})
	charParams = append(charParams, Param{"path", config.fsSockPath})

	fsParams = append(fsParams, Param{"chardev", v.charDev})
	fsParams = append(fsParams, Param{"tag", v.tag})
	fsParams = append(fsParams, Param{"id", v.deviceID})

	params = append(params, "-chardev", fmt.Sprintf("%s,%s,server,nowait", v.backend, strings.Join(SerializeParams(charParams, "="), ",")))
	params = append(params, "-device", fmt.Sprintf("%s,%s", driver, strings.Join(SerializeParams(fsParams, "="), ",")))
	return params
}

type vhostVsock struct {
	driver  VirtioDriver
	id      string
	guestID string
	VHostFD *os.File
}

func (v vhostVsock) getParams(config *StratovirtConfig) []string {
	var params []string
	var devParams []Param

	driver := vsockDriver[v.driver]
	devParams = append(devParams, Param{"id", v.id})
	devParams = append(devParams, Param{"guest-cid", v.guestID})

	if v.VHostFD != nil {
		FDs := config.appendFDs([]*os.File{v.VHostFD})
		devParams = append(devParams, Param{"vhostfd", fmt.Sprintf("%d", FDs[0])})
	}

	params = append(params, "-device", fmt.Sprintf("%s,%s", driver, strings.Join(SerializeParams(devParams, "="), ",")))
	return params
}

type rngDevice struct {
	id       string
	fileName string
	driver   VirtioDriver
	deviceID string
	rng      string
}

func (r rngDevice) getParams(config *StratovirtConfig) []string {
	var params []string
	var objParams []Param
	var devParams []Param

	driver := rngDriver[r.driver]
	objParams = append(objParams, Param{"id", r.id})
	objParams = append(objParams, Param{"filename", r.fileName})

	devParams = append(devParams, Param{"rng", r.rng})
	devParams = append(devParams, Param{"id", r.deviceID})

	params = append(params, "-object", fmt.Sprintf("rng-random,%s", strings.Join(SerializeParams(objParams, "="), ",")))
	params = append(params, "-device", fmt.Sprintf("%s,%s", driver, strings.Join(SerializeParams(devParams, "="), ",")))
	return params
}

type consoleDevice struct {
	driver   VirtioDriver
	id       string
	backend  string
	charID   string
	devType  string
	charDev  string
	deviceID string
}

func (c consoleDevice) getParams(config *StratovirtConfig) []string {
	var params []string
	var devParams []Param
	var charParams []Param
	var conParams []Param

	driver := consoleDriver[c.driver]
	if c.id != "" {
		devParams = append(devParams, Param{"id", c.id})
	}

	conParams = append(conParams, Param{"chardev", c.charDev})
	conParams = append(conParams, Param{"id", c.deviceID})
	params = append(params, "-device", fmt.Sprintf("%s,%s", driver, strings.Join(SerializeParams(devParams, "="), ",")))

	charParams = append(charParams, Param{"id", c.charID})
	charParams = append(charParams, Param{"path", config.consolePath})
	params = append(params, "-chardev", fmt.Sprintf("%s,%s,server,nowait", c.backend, strings.Join(SerializeParams(charParams, "="), ",")))
	params = append(params, "-device", fmt.Sprintf("%s,%s,nr=0", c.devType, strings.Join(SerializeParams(conParams, "="), ",")))
	return params
}

// StratovirtConfig keeps the custom settings and parameters to start virtual machine.
type StratovirtConfig struct {
	name                   string
	uuid                   string
	machineType            string
	vmPath                 string
	smp                    uint32
	memory                 uint64
	kernelPath             string
	kernelAdditionalParams string
	rootfsPath             string
	initrdPath             string
	devices                []VirtioDev
	qmpSocketPath          govmmQemu.QMPSocket
	consolePath            string
	fsSockPath             string
	fds                    []*os.File
}

func (config *StratovirtConfig) appendFDs(fds []*os.File) []int {
	var fdInts []int

	oldLen := len(config.fds)

	config.fds = append(config.fds, fds...)

	// The magic 3 offset comes from https://golang.org/src/os/exec/exec.go:
	//     ExtraFiles specifies additional open files to be inherited by the
	//     new process. It does not include standard input, standard output, or
	//     standard error. If non-nil, entry i becomes file descriptor 3+i.
	// This means that arbitrary file descriptors fd0, fd1... fdN passed in
	// the array will be presented to the guest as consecutive descriptors
	// 3, 4... N+3. The golang library internally relies on dup2() to do
	// the renumbering.
	for i := range fds {
		fdInts = append(fdInts, oldLen+3+i)
	}

	return fdInts
}

// State keeps StratoVirt device and pids state.
type State struct {
	mmioBlkSlots [maxMmioBlkCount]bool
	pid          int
	virtiofsPid  int
}

type stratovirt struct {
	id             string
	path           string
	ctx            context.Context
	fds            []*os.File
	config         HypervisorConfig
	qmpMonitorCh   qmpChannel
	svConfig       StratovirtConfig
	state          State
	stopped        atomic.Bool
	virtiofsDaemon VirtiofsDaemon
}

func (s *stratovirt) getKernelParams(machineType string, initrdPath string) (string, error) {
	var kernelParams []Param

	if initrdPath == "" {
		params, err := GetKernelRootParams(s.config.RootfsType, true, false)
		if err != nil {
			return "", err
		}
		kernelParams = params
	}

	// Take the default parameters.
	kernelParams = append(kernelParams, defaultKernelParames...)
	if machineType == "microvm" {
		kernelParams = append(kernelParams, defaultMicroVMParames...)
	}

	if s.config.Debug {
		kernelParams = append(kernelParams, []Param{
			{"debug", ""},
			{"console", "hvc0"},
		}...)
	} else {
		kernelParams = append(kernelParams, []Param{
			{"quiet", ""},
			{"8250.nr_uarts", "0"},
			{"agent.log_vport", fmt.Sprintf("%d", vSockLogsPort)},
		}...)
	}

	kernelParams = append(s.config.KernelParams, kernelParams...)
	strParams := SerializeParams(kernelParams, "=")

	return strings.Join(strParams, " "), nil
}

func (s *stratovirt) createQMPSocket(vmPath string) govmmQemu.QMPSocket {
	socketPath := filepath.Join(vmPath, apiSocket)

	s.qmpMonitorCh = qmpChannel{
		ctx:  s.ctx,
		path: socketPath,
	}

	return govmmQemu.QMPSocket{
		Type:   "unix",
		Name:   s.qmpMonitorCh.path,
		Server: true,
		NoWait: true,
	}
}

// Logger returns a logrus logger appropriate for logging StratoVirt messages
func (s *stratovirt) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "stratovirt")
}

func (s *stratovirt) consoleSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(s.config.VMStorePath, id, debugSocket)
}

func (s *stratovirt) virtiofsSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(s.config.VMStorePath, id, virtiofsSocket)
}

func (s *stratovirt) nydusdSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(s.config.VMStorePath, id, nydusdSock)
}

func (s *stratovirt) qmpSetup() error {
	s.qmpMonitorCh.Lock()
	defer s.qmpMonitorCh.Unlock()

	if s.qmpMonitorCh.qmp != nil {
		return nil
	}

	events := make(chan govmmQemu.QMPEvent)
	go s.loopQMPEvent(events)

	cfg := govmmQemu.QMPConfig{
		Logger:  newQMPLogger(),
		EventCh: events,
	}

	// Auto-closed by QMPStart().
	disconnectCh := make(chan struct{})

	qmp, _, err := govmmQemu.QMPStart(s.qmpMonitorCh.ctx, s.qmpMonitorCh.path, cfg, disconnectCh)
	if err != nil {
		s.Logger().WithError(err).Error("Failed to connect to StratoVirt instance")
		return err
	}

	err = qmp.ExecuteQMPCapabilities(s.qmpMonitorCh.ctx)
	if err != nil {
		qmp.Shutdown()
		s.Logger().WithError(err).Error(qmpCapErrMsg)
		return err
	}
	s.qmpMonitorCh.qmp = qmp
	s.qmpMonitorCh.disconn = disconnectCh

	return nil
}

func (s *stratovirt) loopQMPEvent(event chan govmmQemu.QMPEvent) {
	for e := range event {
		s.Logger().WithField("event", e).Debug("got QMP event")
	}
	s.Logger().Infof("QMP event channel closed")
}

func (s *stratovirt) qmpShutdown() {
	s.qmpMonitorCh.Lock()
	defer s.qmpMonitorCh.Unlock()

	if s.qmpMonitorCh.qmp != nil {
		s.qmpMonitorCh.qmp.Shutdown()
		// wait on disconnected channel to be sure that the qmp
		// been closed cleanly.
		<-s.qmpMonitorCh.disconn
		s.qmpMonitorCh.qmp = nil
		s.qmpMonitorCh.disconn = nil
	}
}

func (s *stratovirt) createDevices() []VirtioDev {
	var devices []VirtioDev
	ctx := s.ctx

	// Set random device.
	devices = s.appendRng(ctx, devices)

	// Set serial console device for Debug.
	if s.config.Debug {
		devices = s.appendConsole(ctx, devices)
	}

	if s.svConfig.initrdPath == "" {
		devices = s.appendBlock(ctx, devices)
		if s.svConfig.machineType == machineTypeMicrovm {
			s.state.mmioBlkSlots[0] = true
		}
	}

	return devices
}

func (s *stratovirt) appendBlock(ctx context.Context, devices []VirtioDev) []VirtioDev {
	devices = append(devices, blkDevice{
		id:       "rootfs",
		filePath: s.svConfig.rootfsPath,
		deviceID: "virtio-blk0",
		driver:   mmioBus,
	})

	return devices
}

func (s *stratovirt) appendRng(ctx context.Context, devices []VirtioDev) []VirtioDev {
	devices = append(devices, rngDevice{
		id:       "objrng0",
		fileName: s.config.EntropySource,
		rng:      "objrng0",
		deviceID: "virtio-rng0",
		driver:   mmioBus,
	})

	return devices
}

func (s *stratovirt) appendConsole(ctx context.Context, devices []VirtioDev) []VirtioDev {
	devices = append(devices, consoleDevice{
		id:       "virtio-serial0",
		backend:  "socket",
		charID:   "charconsole0",
		devType:  "virtconsole",
		charDev:  "charconsole0",
		deviceID: "virtio-console0",
		driver:   mmioBus,
	})

	return devices
}

func (s *stratovirt) appendVhostVsock(ctx context.Context, devices []VirtioDev, vsock types.VSock) []VirtioDev {
	devices = append(devices, vhostVsock{
		id:      "vsock-id",
		guestID: fmt.Sprintf("%d", vsock.ContextID),
		VHostFD: vsock.VhostFd,
		driver:  mmioBus,
	})

	return devices
}

func (s *stratovirt) appendNetwork(ctx context.Context, devices []VirtioDev, endpoint Endpoint) []VirtioDev {
	name := endpoint.Name()

	devices = append(devices, netDevice{
		devType:  "tap",
		id:       name,
		ifname:   endpoint.NetworkPair().TapInterface.TAPIface.Name,
		netdev:   name,
		deviceID: name,
		FDs:      endpoint.NetworkPair().TapInterface.VMFds,
		mac:      endpoint.HardwareAddr(),
		driver:   mmioBus,
	})

	return devices
}

func (s *stratovirt) appendVirtioFs(ctx context.Context, devices []VirtioDev, volume types.Volume) []VirtioDev {
	if s.config.SharedFS != config.VirtioFS && s.config.SharedFS != config.VirtioFSNydus {
		return devices
	}
	name := "virtio_fs"

	devices = append(devices, virtioFs{
		backend: "socket",
		// Virtio-fs must be bound to unique charDev, it uses the same name.
		charID:   name,
		charDev:  name,
		tag:      volume.MountTag,
		deviceID: "virtio-fs0",
		driver:   mmioBus,
	})

	return devices
}

func (s *stratovirt) setVMConfig(id string, hypervisorConfig *HypervisorConfig) error {
	span, _ := katatrace.Trace(s.ctx, s.Logger(), "setStratoVirtUp", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	if err := validateHypervisorConfig(hypervisorConfig); err != nil {
		return err
	}

	s.id = id
	if err := s.setConfig(hypervisorConfig); err != nil {
		return err
	}

	machineType := strings.ToLower(s.config.HypervisorMachineType)
	if machineType == "" {
		machineType = defaultStratoVirtMachineType
	}

	initrdPath, err := s.config.InitrdAssetPath()
	if err != nil {
		return err
	}

	imagePath, err := s.config.ImageAssetPath()
	if err != nil {
		return err
	}

	kernelPath, err := s.config.KernelAssetPath()
	if err != nil {
		return err
	}

	kernelParams, err := s.getKernelParams(machineType, initrdPath)
	if err != nil {
		return err
	}

	vmPath := filepath.Join(s.config.VMStorePath, s.id)
	qmpSocket := s.createQMPSocket(vmPath)

	s.svConfig = StratovirtConfig{
		name:                   fmt.Sprintf("sandbox-%s", id),
		uuid:                   uuid.Generate().String(),
		machineType:            machineType,
		vmPath:                 vmPath,
		smp:                    s.config.NumVCPUs(),
		memory:                 uint64(s.config.MemorySize),
		kernelPath:             kernelPath,
		kernelAdditionalParams: kernelParams,
		rootfsPath:             imagePath,
		initrdPath:             initrdPath,
		qmpSocketPath:          qmpSocket,
		consolePath:            filepath.Join(vmPath, debugSocket),
		fsSockPath:             filepath.Join(vmPath, virtiofsSocket),
	}

	s.svConfig.devices = s.createDevices()

	return nil
}

func (s *stratovirt) setupVirtiofsDaemon(ctx context.Context) (err error) {
	if s.config.SharedFS == config.NoSharedFS {
		return nil
	}

	if s.virtiofsDaemon == nil {
		return errors.New("No stratovirt virtiofsDaemon configuration")
	}

	s.Logger().Info("Starting virtiofsDaemon")

	pid, err := s.virtiofsDaemon.Start(ctx, func() {
		s.StopVM(ctx, false)
	})
	if err != nil {
		return err
	}
	s.state.virtiofsPid = pid

	return nil
}

func (s *stratovirt) stopVirtiofsDaemon(ctx context.Context) (err error) {
	if s.state.virtiofsPid == 0 {
		s.Logger().Warn("The virtiofsd had stopped")
		return nil
	}

	err = s.virtiofsDaemon.Stop(ctx)
	if err != nil {
		return err
	}

	s.state.virtiofsPid = 0

	return nil
}

// Get StratoVirt binary path.
func (s *stratovirt) binPath() (string, error) {
	path, err := s.config.HypervisorAssetPath()
	if err != nil {
		return "", err
	}

	if path == "" {
		path = defaultStratoVirt
	}

	if _, err = os.Stat(path); os.IsNotExist(err) {
		return "", fmt.Errorf("StratoVirt path (%s) does not exist", path)
	}
	return path, nil
}

// Log StratoVirt errors and ensure the StratoVirt process is reaped after
// termination
func (s *stratovirt) logAndWait(stratovirtCmd *exec.Cmd, reader io.ReadCloser) {
	s.state.pid = stratovirtCmd.Process.Pid
	s.Logger().Infof("Start logging StratoVirt (Pid=%d)", s.state.pid)
	scanner := bufio.NewScanner(reader)
	infoRE := regexp.MustCompile("([^:]):INFO: ")
	warnRE := regexp.MustCompile("([^:]):WARN: ")
	for scanner.Scan() {
		text := scanner.Text()
		if infoRE.MatchString(text) {
			text = infoRE.ReplaceAllString(text, "$1")
			s.Logger().WithField("StratoVirt Pid", s.state.pid).Info(text)
		} else if warnRE.MatchString(text) {
			text = infoRE.ReplaceAllString(text, "$1")
			s.Logger().WithField("StratoVirt Pid", s.state.pid).Warn(text)
		} else {
			s.Logger().WithField("StratoVirt Pid", s.state.pid).Error(text)
		}
	}
	s.Logger().Infof("Stop logging StratoVirt (Pid=%d)", s.state.pid)
	stratovirtCmd.Wait()
}

// waitVM will wait for the Sandbox's VM to be up and running.
func (s *stratovirt) waitVM(ctx context.Context, timeout int) error {
	span, _ := katatrace.Trace(ctx, s.Logger(), "waitVM", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	if timeout < 0 {
		return fmt.Errorf("Invalid timeout %ds", timeout)
	}

	cfg := govmmQemu.QMPConfig{Logger: newQMPLogger()}

	var qmp *govmmQemu.QMP
	var disconnectCh chan struct{}
	var ver *govmmQemu.QMPVersion
	var err error

	// clear andy possible old state before trying to connect again.
	s.qmpShutdown()
	timeStart := time.Now()
	for {
		disconnectCh = make(chan struct{})
		qmp, ver, err = govmmQemu.QMPStart(s.qmpMonitorCh.ctx, s.qmpMonitorCh.path, cfg, disconnectCh)
		if err == nil {
			break
		}

		if int(time.Since(timeStart).Seconds()) > timeout {
			return fmt.Errorf("Failed to connect StratoVirt instance (timeout %ds): %v", timeout, err)
		}

		time.Sleep(time.Duration(50) * time.Millisecond)
	}
	s.qmpMonitorCh.qmp = qmp
	s.qmpMonitorCh.disconn = disconnectCh
	defer s.qmpShutdown()

	s.Logger().WithFields(logrus.Fields{
		"qmp-major-version": ver.Major,
		"qmp-minor-version": ver.Minor,
		"qmp-micro-version": ver.Micro,
		"qmp-Capabilities":  strings.Join(ver.Capabilities, ","),
	}).Infof("QMP details")

	if err = s.qmpMonitorCh.qmp.ExecuteQMPCapabilities(s.qmpMonitorCh.ctx); err != nil {
		s.Logger().WithError(err).Error(qmpCapErrMsg)
		return err
	}

	return nil
}

func (s *stratovirt) createParams(params *[]string) {
	*params = append(*params, "-name", s.svConfig.name)
	*params = append(*params, "-uuid", s.svConfig.uuid)
	*params = append(*params, "-smp", strconv.Itoa(int(s.svConfig.smp)))
	*params = append(*params, "-m", strconv.Itoa(int(s.svConfig.memory)))
	*params = append(*params, "-kernel", s.svConfig.kernelPath)
	*params = append(*params, "-append", s.svConfig.kernelAdditionalParams)
	*params = append(*params, "-qmp", fmt.Sprintf("%s:%s,server,nowait", s.svConfig.qmpSocketPath.Type, s.svConfig.qmpSocketPath.Name))
	*params = append(*params, "-D")
	*params = append(*params, "-disable-seccomp")

	if s.config.SharedFS == config.VirtioFS || s.config.SharedFS == config.VirtioFSNydus {
		*params = append(*params, "-machine", fmt.Sprintf("type=%s,dump-guest-core=off,mem-share=on", s.svConfig.machineType))
	} else {
		*params = append(*params, "-machine", fmt.Sprintf("type=%s,dump-guest-core=off", s.svConfig.machineType))
	}

	if s.svConfig.initrdPath != "" {
		*params = append(*params, "-initrd", s.svConfig.initrdPath)
	}

	for _, d := range s.svConfig.devices {
		*params = append(*params, d.getParams(&s.svConfig)...)
	}
}

// cleanupVM will remove generated files and directories related with VM.
func (s *stratovirt) cleanupVM(force bool) error {
	link, err := filepath.EvalSymlinks(s.svConfig.vmPath)
	if err != nil {
		s.Logger().WithError(err).Warn("Failed to get evaluation of any symbolic links.")
	}

	s.Logger().WithFields(logrus.Fields{
		"link": link,
		"dir":  s.svConfig.vmPath,
	}).Infof("cleanup vm path")

	if err := os.RemoveAll(s.svConfig.vmPath); err != nil {
		if !force {
			return err
		}
		s.Logger().WithError(err).Warnf("Failed to clean up vm dir %s", s.svConfig.vmPath)
	}

	if link != s.svConfig.vmPath && link != "" {
		if errRemove := os.RemoveAll(link); errRemove != nil {
			if !force {
				return err
			}
			s.Logger().WithError(errRemove).WithField("link", link).Warnf("Failed to remove vm path link %s", link)
		}
	}

	if s.config.VMid != "" {
		dir := filepath.Join(s.config.VMStorePath, s.config.VMid)
		if err := os.RemoveAll(dir); err != nil {
			if !force {
				return err
			}
			s.Logger().WithError(err).WithField("path", dir).Warn("failed to remove vm path")
		}
	}

	return nil
}

func (s *stratovirt) setupMmioSlot(Name string, isPut bool) (int, error) {
	Name = filepath.Base(strings.ToLower(Name))

	if strings.HasPrefix(Name, "vd") {
		charStr := strings.TrimPrefix(Name, "vd")
		if charStr == Name {
			return 0, fmt.Errorf("Could not parse idx from Name %q", Name)
		}

		char := []rune(charStr)
		idx := int(char[0] - 'a')

		if !isPut && s.state.mmioBlkSlots[idx] {
			return 0, fmt.Errorf("failed to setup mmio slot, slot is being used %q", charStr)
		}
		s.state.mmioBlkSlots[idx] = !isPut

		return idx, nil
	}

	return 0, fmt.Errorf("failed to setup mmio slot, Name is invalid %q", Name)
}

func (s *stratovirt) getDevSlot(Name string) (int, error) {
	slot, err := s.setupMmioSlot(Name, false)
	if err != nil {
		return 0, err
	}

	return slot, nil
}

func (s *stratovirt) delDevSlot(Name string) error {
	if _, err := s.setupMmioSlot(Name, true); err != nil {
		return err
	}

	return nil
}

func (s *stratovirt) hotplugBlk(ctx context.Context, drive *config.BlockDrive, op Operation) error {
	err := s.qmpSetup()
	if err != nil {
		return err
	}

	driver := "virtio-blk-mmio"

	defer func() {
		if err != nil {
			s.qmpMonitorCh.qmp.ExecuteBlockdevDel(s.qmpMonitorCh.ctx, drive.ID)
			if errDel := s.delDevSlot(drive.VirtPath); errDel != nil {
				s.Logger().WithError(errDel).Warn("Failed to delete device slot.")
			}
		}
	}()

	switch op {
	case AddDevice:
		sblkDevice := govmmQemu.BlockDevice{
			ID:       drive.ID,
			File:     drive.File,
			ReadOnly: drive.ReadOnly,
			AIO:      govmmQemu.BlockDeviceAIO("native"),
		}
		if err := s.qmpMonitorCh.qmp.ExecuteBlockdevAdd(s.qmpMonitorCh.ctx, &sblkDevice); err != nil {
			return err
		}

		slot, err := s.getDevSlot(drive.VirtPath)
		if err != nil {
			return err
		}

		devAddr := fmt.Sprintf("%d", slot)
		if err := s.qmpMonitorCh.qmp.ExecutePCIDeviceAdd(s.qmpMonitorCh.ctx, drive.ID, drive.ID, driver, devAddr, "", "", 0, false, false); err != nil {
			return err
		}
	case RemoveDevice:
		if errDel := s.delDevSlot(drive.VirtPath); errDel != nil {
			s.Logger().WithError(errDel).Warn("Failed to delete device slot.")
		}
		if err := s.qmpMonitorCh.qmp.ExecuteDeviceDel(s.qmpMonitorCh.ctx, drive.ID); err != nil {
			return err
		}

	default:
		return fmt.Errorf("operation is not supported %d", op)
	}

	return nil
}

func (s *stratovirt) createVirtiofsDaemon(sharedPath string) (VirtiofsDaemon, error) {
	virtiofsdSocketPath, err := s.virtiofsSocketPath(s.id)
	if err != nil {
		return nil, err
	}

	if s.config.SharedFS == config.VirtioFSNydus {
		apiSockPath, err := s.nydusdSocketPath(s.id)
		if err != nil {
			return nil, err
		}
		nd := &nydusd{
			path:        s.config.VirtioFSDaemon,
			sockPath:    virtiofsdSocketPath,
			apiSockPath: apiSockPath,
			sourcePath:  sharedPath,
			debug:       s.config.Debug,
			extraArgs:   s.config.VirtioFSExtraArgs,
			startFn:     startInShimNS,
		}
		nd.setupShareDirFn = nd.setupPassthroughFS
		return nd, nil
	}

	// default use virtiofsd
	return &virtiofsd{
		path:       s.config.VirtioFSDaemon,
		sourcePath: sharedPath,
		socketPath: virtiofsdSocketPath,
		extraArgs:  s.config.VirtioFSExtraArgs,
		cache:      s.config.VirtioFSCache,
	}, nil
}

func (s *stratovirt) CreateVM(ctx context.Context, id string, network Network, hypervisorConfig *HypervisorConfig) error {
	span, _ := katatrace.Trace(ctx, s.Logger(), "CreateVM", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	s.ctx = ctx
	err := s.setVMConfig(id, hypervisorConfig)
	if err != nil {
		return err
	}

	if s.path, err = s.binPath(); err != nil {
		return err
	}

	s.virtiofsDaemon, err = s.createVirtiofsDaemon(hypervisorConfig.SharedPath)
	if err != nil {
		return err
	}

	return nil
}

func launchStratovirt(ctx context.Context, s *stratovirt) (*exec.Cmd, io.ReadCloser, error) {
	var params []string
	s.createParams(&params)

	cmd := exec.CommandContext(ctx, s.path, params...)

	if len(s.fds) > 0 {
		s.Logger().Infof("Adding extra file %v", s.fds)
		cmd.ExtraFiles = s.fds
	}

	if s.config.Debug {
		cmd.Env = []string{"STRATOVIRT_LOG_LEVEL=info"}
	}

	reader, err := cmd.StdoutPipe()
	if err != nil {
		s.Logger().Error("Unable to connect stdout to a pipe")
		return nil, nil, err
	}
	s.Logger().Infof("launching %s with: %v", s.path, params)

	if err := cmd.Start(); err != nil {
		s.Logger().Error("Error starting hypervisor, please check the params")
		return nil, nil, err
	}

	return cmd, reader, nil
}

func (s *stratovirt) StartVM(ctx context.Context, timeout int) error {
	span, _ := katatrace.Trace(ctx, s.Logger(), "StartVM", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	err := utils.MkdirAllWithInheritedOwner(s.svConfig.vmPath, DirMode)
	if err != nil {
		return err
	}

	defer func() {
		if err != nil {
			if s.state.virtiofsPid != 0 {
				syscall.Kill(s.state.virtiofsPid, syscall.SIGILL)
			}
		}
		for _, fd := range s.fds {
			if err := fd.Close(); err != nil {
				s.Logger().WithError(err).Error("After launching StratoVirt")
			}
		}
		s.fds = []*os.File{}
	}()

	if err = s.setupVirtiofsDaemon(ctx); err != nil {
		return err
	}
	defer func() {
		if err != nil {
			if shutdownErr := s.stopVirtiofsDaemon(ctx); shutdownErr != nil {
				s.Logger().WithError(shutdownErr).Warn("Error shutting down the VirtiofsDaemon")
			}
		}
	}()

	stratovirtCmd, reader, err := launchStratovirt(ctx, s)
	if err != nil {
		s.Logger().WithError(err).Error("failed to launch StratoVirt")
		return fmt.Errorf("failed to launch StratoVirt: %s", err)
	}

	go s.logAndWait(stratovirtCmd, reader)

	if err = s.waitVM(s.ctx, timeout); err != nil {
		return err
	}

	return nil
}

func (s *stratovirt) StopVM(ctx context.Context, waitOnly bool) (err error) {
	span, _ := katatrace.Trace(ctx, s.Logger(), "StopVM", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	s.Logger().Info("Stopping Sandbox")
	if s.stopped.Load() {
		s.Logger().Info("Already stopped")
		return nil
	}

	defer func() {
		s.cleanupVM(true)
		if err == nil {
			s.stopped.Store(true)
		}
	}()

	if err := s.qmpSetup(); err != nil {
		return err
	}

	pids := s.GetPids()
	if len(pids) == 0 {
		return errors.New("cannot determine StratoVirt PID")
	}
	pid := pids[0]

	if waitOnly {
		err := utils.WaitLocalProcess(pid, stratovirtStopSandboxTimeoutSecs, syscall.Signal(0), s.Logger())
		if err != nil {
			return err
		}
	} else {
		err = syscall.Kill(pid, syscall.SIGKILL)
		if err != nil {
			s.Logger().WithError(err).Error("Failed to send SIGKILL to stratovirt")
			return err
		}
	}

	if s.config.SharedFS == config.VirtioFS || s.config.SharedFS == config.VirtioFSNydus {
		if err := s.stopVirtiofsDaemon(ctx); err != nil {
			return err
		}
	}

	return nil
}

func (s *stratovirt) PauseVM(ctx context.Context) error {
	return nil
}

func (s *stratovirt) SaveVM() error {
	return nil
}

func (s *stratovirt) ResumeVM(ctx context.Context) error {
	return nil
}

func (s *stratovirt) AddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) error {
	span, _ := katatrace.Trace(ctx, s.Logger(), "AddDevice", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	switch v := devInfo.(type) {
	case types.Socket:
		s.svConfig.devices = s.appendConsole(ctx, s.svConfig.devices)
	case types.VSock:
		s.fds = append(s.fds, v.VhostFd)
		s.svConfig.devices = s.appendVhostVsock(ctx, s.svConfig.devices, v)
	case Endpoint:
		s.fds = append(s.fds, v.NetworkPair().TapInterface.VMFds...)
		s.svConfig.devices = s.appendNetwork(ctx, s.svConfig.devices, v)
	case config.BlockDrive:
		s.svConfig.devices = s.appendBlock(ctx, s.svConfig.devices)
	case types.Volume:
		s.svConfig.devices = s.appendVirtioFs(ctx, s.svConfig.devices, v)
	default:
		s.Logger().WithField("dev-type", v).Warn("Could not append device: unsupported device type")
	}

	return nil
}

func (s *stratovirt) HotplugAddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	span, _ := katatrace.Trace(ctx, s.Logger(), "HotplugAddDevice", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	switch devType {
	case BlockDev:
		return nil, s.hotplugBlk(ctx, devInfo.(*config.BlockDrive), AddDevice)
	default:
		return nil, fmt.Errorf("Hotplug add device: unsupported device type '%v'", devType)
	}
}

func (s *stratovirt) HotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	span, _ := katatrace.Trace(ctx, s.Logger(), "HotplugRemoveDevice", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	switch devType {
	case BlockDev:
		return nil, s.hotplugBlk(ctx, devInfo.(*config.BlockDrive), RemoveDevice)
	default:
		return nil, fmt.Errorf("Hotplug remove device: unsupported device type '%v'", devType)
	}
}

func (s *stratovirt) ResizeMemory(ctx context.Context, reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, MemoryDevice, error) {
	return 0, MemoryDevice{}, nil
}

func (s *stratovirt) ResizeVCPUs(ctx context.Context, reqVCPUs uint32) (currentVCPUs uint32, newVCPUs uint32, err error) {
	return 0, 0, nil
}

func (s *stratovirt) GetVMConsole(ctx context.Context, id string) (string, string, error) {
	span, _ := katatrace.Trace(ctx, s.Logger(), "GetVMConsole", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	consoleURL, err := s.consoleSocketPath(s.id)
	if err != nil {
		return consoleProtoUnix, "", err
	}

	return consoleProtoUnix, consoleURL, nil
}

func (s *stratovirt) Disconnect(ctx context.Context) {
	span, _ := katatrace.Trace(ctx, s.Logger(), "Disconnect", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	s.qmpShutdown()
}

func (s *stratovirt) Capabilities(ctx context.Context) types.Capabilities {
	span, _ := katatrace.Trace(ctx, s.Logger(), "Capabilities", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()
	var caps types.Capabilities
	caps.SetBlockDeviceHotplugSupport()
	if s.config.SharedFS != config.NoSharedFS {
		caps.SetFsSharingSupport()
	}

	return caps
}

func (s *stratovirt) HypervisorConfig() HypervisorConfig {
	return s.config
}

func (s *stratovirt) GetTotalMemoryMB(ctx context.Context) uint32 {
	return s.config.MemorySize
}

func (s *stratovirt) GetThreadIDs(ctx context.Context) (VcpuThreadIDs, error) {
	span, _ := katatrace.Trace(ctx, s.Logger(), "GetThreadIDs", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	tid := VcpuThreadIDs{}
	if err := s.qmpSetup(); err != nil {
		return tid, err
	}

	cpuInfos, err := s.qmpMonitorCh.qmp.ExecQueryCpus(s.qmpMonitorCh.ctx)
	if err != nil {
		s.Logger().WithError(err).Error("failed to query cpu infos")
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

func (s *stratovirt) Cleanup(ctx context.Context) error {
	span, _ := katatrace.Trace(ctx, s.Logger(), "Cleanup", stratovirtTracingTags, map[string]string{"sandbox_id": s.id})
	defer span.End()

	for _, fd := range s.fds {
		if err := fd.Close(); err != nil {
			s.Logger().WithError(err).Warn("failed closing fd")
		}
	}
	s.fds = []*os.File{}

	return nil
}

func (s *stratovirt) setConfig(config *HypervisorConfig) error {
	s.config = *config

	return nil
}

func (s *stratovirt) GetPids() []int {
	var pids []int
	pids = append(pids, s.state.pid)

	return pids
}

func (s *stratovirt) GetVirtioFsPid() *int {
	return &s.state.virtiofsPid
}

func (s *stratovirt) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	return errors.New("StratoVirt is not supported by VM cache")
}

func (s *stratovirt) toGrpc(ctx context.Context) ([]byte, error) {
	return nil, errors.New("StratoVirt is not supported by VM cache")
}

func (s *stratovirt) Check() error {
	if s.stopped.Load() {
		return fmt.Errorf("StratoVirt is not running")
	}

	if err := s.qmpSetup(); err != nil {
		return err
	}

	return nil
}

func (s *stratovirt) Save() (hs hv.HypervisorState) {
	pids := s.GetPids()
	hs.Pid = pids[0]
	hs.VirtiofsDaemonPid = s.state.virtiofsPid
	hs.Type = string(StratovirtHypervisor)
	return
}

func (s *stratovirt) Load(hs hv.HypervisorState) {
	s.state.pid = hs.Pid
	s.state.virtiofsPid = hs.VirtiofsDaemonPid
}

func (s *stratovirt) GenerateSocket(id string) (interface{}, error) {
	return generateVMSocket(id, s.config.VMStorePath)
}

func (s *stratovirt) IsRateLimiterBuiltin() bool {
	return false
}
