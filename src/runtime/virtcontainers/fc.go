// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/firecracker/client"
	models "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/firecracker/client/models"
	ops "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/firecracker/client/operations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"

	"github.com/blang/semver"
	"github.com/containerd/console"
	"github.com/containerd/fifo"
	httptransport "github.com/go-openapi/runtime/client"
	"github.com/go-openapi/strfmt"
	"github.com/opencontainers/selinux/go-selinux/label"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	"go.opentelemetry.io/otel"
	otelLabel "go.opentelemetry.io/otel/label"
	otelTrace "go.opentelemetry.io/otel/trace"
)

type vmmState uint8

const (
	notReady vmmState = iota
	cfReady
	vmReady
)

const (
	//fcTimeout is the maximum amount of time in seconds to wait for the VMM to respond
	fcTimeout = 10
	fcSocket  = "firecracker.socket"
	//Name of the files within jailer root
	//Having predefined names helps with cleanup
	fcKernel             = "vmlinux"
	fcRootfs             = "rootfs"
	fcStopSandboxTimeout = 15
	// This indicates the number of block devices that can be attached to the
	// firecracker guest VM.
	// We attach a pool of placeholder drives before the guest has started, and then
	// patch the replace placeholder drives with drives with actual contents.
	fcDiskPoolSize           = 8
	defaultHybridVSocketName = "kata.hvsock"

	// This is the first usable vsock context ID. All the vsocks can use the same
	// ID, since it's only used in the guest.
	defaultGuestVSockCID = int64(0x3)

	// This is related to firecracker logging scheme
	fcLogFifo     = "logs.fifo"
	fcMetricsFifo = "metrics.fifo"

	defaultFcConfig = "fcConfig.json"
	// storagePathSuffix mirrors persist/fs/fs.go:storagePathSuffix
	storagePathSuffix = "vc"
)

// Specify the minimum version of firecracker supported
var fcMinSupportedVersion = semver.MustParse("0.21.1")

var fcKernelParams = append(commonVirtioblkKernelRootParams, []Param{
	// The boot source is the first partition of the first block device added
	{"pci", "off"},
	{"reboot", "k"},
	{"panic", "1"},
	{"iommu", "off"},
	{"net.ifnames", "0"},
	{"random.trust_cpu", "on"},

	// Firecracker doesn't support ACPI
	// Fix kernel error "ACPI BIOS Error (bug)"
	{"acpi", "off"},
}...)

func (s vmmState) String() string {
	switch s {
	case notReady:
		return "FC not ready"
	case cfReady:
		return "FC configure ready"
	case vmReady:
		return "FC VM ready"
	}

	return ""
}

// FirecrackerInfo contains information related to the hypervisor that we
// want to store on disk
type FirecrackerInfo struct {
	PID     int
	Version string
}

type firecrackerState struct {
	sync.RWMutex
	state vmmState
}

func (s *firecrackerState) set(state vmmState) {
	s.Lock()
	defer s.Unlock()

	s.state = state
}

// firecracker is an Hypervisor interface implementation for the firecracker VMM.
type firecracker struct {
	id            string //Unique ID per pod. Normally maps to the sandbox id
	vmPath        string //All jailed VM assets need to be under this
	chrootBaseDir string //chroot base for the jailer
	jailerRoot    string
	socketPath    string
	netNSPath     string
	uid           string //UID and GID to be used for the VMM
	gid           string

	info FirecrackerInfo

	firecrackerd *exec.Cmd           //Tracks the firecracker process itself
	connection   *client.Firecracker //Tracks the current active connection

	ctx            context.Context
	config         HypervisorConfig
	pendingDevices []firecrackerDevice // Devices to be added before the FC VM ready

	state  firecrackerState
	jailed bool //Set to true if jailer is enabled

	fcConfigPath string
	fcConfig     *types.FcConfig // Parameters configured before VM starts

	console console.Console
}

type firecrackerDevice struct {
	dev     interface{}
	devType deviceType
}

// Logger returns a logrus logger appropriate for logging firecracker  messages
func (fc *firecracker) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "firecracker")
}

func (fc *firecracker) trace(parent context.Context, name string) (otelTrace.Span, context.Context) {
	if parent == nil {
		fc.Logger().WithField("type", "bug").Error("trace called before context set")
		parent = context.Background()
	}

	tracer := otel.Tracer("kata")
	ctx, span := tracer.Start(parent, name, otelTrace.WithAttributes(otelLabel.String("source", "runtime"), otelLabel.String("package", "virtcontainers"), otelLabel.String("subsystem", "hypervisor"), otelLabel.String("type", "firecracker"), otelLabel.String("sandbox_id", fc.id)))

	return span, ctx
}

//At some cases, when sandbox id is too long, it will incur error of overlong
//firecracker API unix socket(fc.socketPath).
//In Linux, sun_path could maximumly contains 108 bytes in size.
//(http://man7.org/linux/man-pages/man7/unix.7.html)
func (fc *firecracker) truncateID(id string) string {
	if len(id) > 32 {
		//truncate the id to only leave the size of UUID(128bit).
		return id[:32]
	}

	return id
}

// For firecracker this call only sets the internal structure up.
// The sandbox will be created and started through startSandbox().
func (fc *firecracker) createSandbox(ctx context.Context, id string, networkNS NetworkNamespace, hypervisorConfig *HypervisorConfig) error {
	fc.ctx = ctx

	var span otelTrace.Span
	span, _ = fc.trace(ctx, "createSandbox")
	defer span.End()

	//TODO: check validity of the hypervisor config provided
	//https://github.com/kata-containers/runtime/issues/1065
	fc.id = fc.truncateID(id)
	fc.state.set(notReady)
	fc.config = *hypervisorConfig

	// When running with jailer all resources need to be under
	// a specific location and that location needs to have
	// exec permission (i.e. should not be mounted noexec, e.g. /run, /var/run)
	// Also unix domain socket names have a hard limit
	// #define UNIX_PATH_MAX   108
	// Keep it short and live within the jailer expected paths
	// <chroot_base>/<exec_file_name>/<id>/
	// Also jailer based on the id implicitly sets up cgroups under
	// <cgroups_base>/<exec_file_name>/<id>/
	hypervisorName := filepath.Base(hypervisorConfig.HypervisorPath)
	//fs.RunStoragePath cannot be used as we need exec perms
	fc.chrootBaseDir = filepath.Join("/run", storagePathSuffix)

	fc.vmPath = filepath.Join(fc.chrootBaseDir, hypervisorName, fc.id)
	fc.jailerRoot = filepath.Join(fc.vmPath, "root") // auto created by jailer

	// Firecracker and jailer automatically creates default API socket under /run
	// with the name of "firecracker.socket"
	fc.socketPath = filepath.Join(fc.jailerRoot, "run", fcSocket)

	// So we need to repopulate this at startSandbox where it is valid
	fc.netNSPath = networkNS.NetNsPath

	// Till we create lower privileged kata user run as root
	// https://github.com/kata-containers/runtime/issues/1869
	fc.uid = "0"
	fc.gid = "0"

	fc.fcConfig = &types.FcConfig{}
	fc.fcConfigPath = filepath.Join(fc.vmPath, defaultFcConfig)
	return nil
}

func (fc *firecracker) newFireClient(ctx context.Context) *client.Firecracker {
	span, _ := fc.trace(ctx, "newFireClient")
	defer span.End()
	httpClient := client.NewHTTPClient(strfmt.NewFormats())

	socketTransport := &http.Transport{
		DialContext: func(ctx context.Context, network, path string) (net.Conn, error) {
			addr, err := net.ResolveUnixAddr("unix", fc.socketPath)
			if err != nil {
				return nil, err
			}

			return net.DialUnix("unix", nil, addr)
		},
	}

	transport := httptransport.New(client.DefaultHost, client.DefaultBasePath, client.DefaultSchemes)
	transport.SetLogger(fc.Logger())
	transport.SetDebug(fc.Logger().Logger.Level == logrus.DebugLevel)
	transport.Transport = socketTransport
	httpClient.SetTransport(transport)

	return httpClient
}

func (fc *firecracker) vmRunning(ctx context.Context) bool {
	resp, err := fc.client(ctx).Operations.DescribeInstance(nil)
	if err != nil {
		fc.Logger().WithError(err).Error("getting vm status failed")
		return false
	}
	// The current state of the Firecracker instance (swagger:model InstanceInfo)
	return resp.Payload.Started
}

func (fc *firecracker) getVersionNumber() (string, error) {
	args := []string{"--version"}
	checkCMD := exec.Command(fc.config.HypervisorPath, args...)

	data, err := checkCMD.Output()
	if err != nil {
		return "", fmt.Errorf("Running checking FC version command failed: %v", err)
	}

	var version string
	fields := strings.Split(string(data), " ")
	if len(fields) > 1 {
		// The output format of `Firecracker --verion` is as follows
		// Firecracker v0.23.1
		version = strings.TrimPrefix(strings.TrimSpace(fields[1]), "v")
		return version, nil
	}

	return "", errors.New("getting FC version failed, the output is malformed")
}

func (fc *firecracker) checkVersion(version string) error {
	v, err := semver.Make(version)
	if err != nil {
		return fmt.Errorf("Malformed firecracker version: %v", err)
	}

	if v.LT(fcMinSupportedVersion) {
		return fmt.Errorf("version %v is not supported. Minimum supported version of firecracker is %v", v.String(), fcMinSupportedVersion.String())
	}

	return nil
}

// waitVMMRunning will wait for timeout seconds for the VMM to be up and running.
func (fc *firecracker) waitVMMRunning(ctx context.Context, timeout int) error {
	span, _ := fc.trace(ctx, "wait VMM to be running")
	defer span.End()

	if timeout < 0 {
		return fmt.Errorf("Invalid timeout %ds", timeout)
	}

	timeStart := time.Now()
	for {
		if fc.vmRunning(ctx) {
			return nil
		}

		if int(time.Since(timeStart).Seconds()) > timeout {
			return fmt.Errorf("Failed to connect to firecrackerinstance (timeout %ds)", timeout)
		}

		time.Sleep(time.Duration(10) * time.Millisecond)
	}
}

func (fc *firecracker) fcInit(ctx context.Context, timeout int) error {
	span, _ := fc.trace(ctx, "fcInit")
	defer span.End()

	var err error
	//FC version set and check
	if fc.info.Version, err = fc.getVersionNumber(); err != nil {
		return err
	}

	if err := fc.checkVersion(fc.info.Version); err != nil {
		return err
	}

	var cmd *exec.Cmd
	var args []string

	if fc.fcConfigPath, err = fc.fcJailResource(fc.fcConfigPath, defaultFcConfig); err != nil {
		return err
	}

	//https://github.com/firecracker-microvm/firecracker/blob/master/docs/jailer.md#jailer-usage
	//--seccomp-level specifies whether seccomp filters should be installed and how restrictive they should be. Possible values are:
	//0 : disabled.
	//1 : basic filtering. This prohibits syscalls not whitelisted by Firecracker.
	//2 (default): advanced filtering. This adds further checks on some of the parameters of the allowed syscalls.
	if fc.jailed {
		jailedArgs := []string{
			"--id", fc.id,
			"--node", "0", //FIXME: Comprehend NUMA topology or explicit ignore
			"--exec-file", fc.config.HypervisorPath,
			"--uid", "0", //https://github.com/kata-containers/runtime/issues/1869
			"--gid", "0",
			"--chroot-base-dir", fc.chrootBaseDir,
			"--daemonize",
		}
		args = append(args, jailedArgs...)
		if fc.netNSPath != "" {
			args = append(args, "--netns", fc.netNSPath)
		}
		args = append(args, "--", "--config-file", fc.fcConfigPath)

		cmd = exec.Command(fc.config.JailerPath, args...)
	} else {
		args = append(args,
			"--api-sock", fc.socketPath,
			"--config-file", fc.fcConfigPath)
		cmd = exec.Command(fc.config.HypervisorPath, args...)
	}

	if fc.config.Debug {
		cmd.Stderr = fc.console
		cmd.Stdout = fc.console
	}

	fc.Logger().WithField("hypervisor args", args).Debug()
	fc.Logger().WithField("hypervisor cmd", cmd).Debug()

	fc.Logger().Info("Starting VM")
	if err := cmd.Start(); err != nil {
		fc.Logger().WithField("Error starting firecracker", err).Debug()
		return err
	}

	fc.info.PID = cmd.Process.Pid
	fc.firecrackerd = cmd
	fc.connection = fc.newFireClient(ctx)

	if err := fc.waitVMMRunning(ctx, timeout); err != nil {
		fc.Logger().WithField("fcInit failed:", err).Debug()
		return err
	}
	return nil
}

func (fc *firecracker) fcEnd(ctx context.Context, waitOnly bool) (err error) {
	span, _ := fc.trace(ctx, "fcEnd")
	defer span.End()

	fc.Logger().Info("Stopping firecracker VM")

	defer func() {
		if err != nil {
			fc.Logger().Info("fcEnd failed")
		} else {
			fc.Logger().Info("Firecracker VM stopped")
		}
	}()

	pid := fc.info.PID

	shutdownSignal := syscall.SIGTERM

	if waitOnly {
		// NOP
		shutdownSignal = syscall.Signal(0)
	}

	// Wait for the VM process to terminate
	return utils.WaitLocalProcess(pid, fcStopSandboxTimeout, shutdownSignal, fc.Logger())
}

func (fc *firecracker) client(ctx context.Context) *client.Firecracker {
	span, _ := fc.trace(ctx, "client")
	defer span.End()

	if fc.connection == nil {
		fc.connection = fc.newFireClient(ctx)
	}

	return fc.connection
}

func (fc *firecracker) createJailedDrive(name string) (string, error) {
	// Don't bind mount the resource, just create a raw file
	// that can be bind-mounted later
	r := filepath.Join(fc.jailerRoot, name)
	f, err := os.Create(r)
	if err != nil {
		return "", err
	}
	f.Close()

	if fc.jailed {
		// use path relative to the jail
		r = filepath.Join("/", name)
	}

	return r, nil
}

// when running with jailer, firecracker binary will firstly be copied into fc.jailerRoot,
// and then being executed there. Therefore we need to ensure fc.JailerRoot has exec permissions.
func (fc *firecracker) fcRemountJailerRootWithExec() error {
	if err := bindMount(context.Background(), fc.jailerRoot, fc.jailerRoot, false, "shared"); err != nil {
		fc.Logger().WithField("JailerRoot", fc.jailerRoot).Errorf("bindMount failed: %v", err)
		return err
	}

	// /run is normally mounted with rw, nosuid(MS_NOSUID), relatime(MS_RELATIME), noexec(MS_NOEXEC).
	// we re-mount jailerRoot to deliberately leave out MS_NOEXEC.
	if err := remount(context.Background(), syscall.MS_NOSUID|syscall.MS_RELATIME, fc.jailerRoot); err != nil {
		fc.Logger().WithField("JailerRoot", fc.jailerRoot).Errorf("Re-mount failed: %v", err)
		return err
	}

	return nil
}

func (fc *firecracker) fcJailResource(src, dst string) (string, error) {
	if src == "" || dst == "" {
		return "", fmt.Errorf("fcJailResource: invalid jail locations: src:%v, dst:%v",
			src, dst)
	}
	jailedLocation := filepath.Join(fc.jailerRoot, dst)
	if err := bindMount(context.Background(), src, jailedLocation, false, "slave"); err != nil {
		fc.Logger().WithField("bindMount failed", err).Error()
		return "", err
	}

	if !fc.jailed {
		return jailedLocation, nil
	}

	// This is the path within the jailed root
	absPath := filepath.Join("/", dst)
	return absPath, nil
}

func (fc *firecracker) fcSetBootSource(ctx context.Context, path, params string) error {
	span, _ := fc.trace(ctx, "fcSetBootSource")
	defer span.End()
	fc.Logger().WithFields(logrus.Fields{"kernel-path": path,
		"kernel-params": params}).Debug("fcSetBootSource")

	kernelPath, err := fc.fcJailResource(path, fcKernel)
	if err != nil {
		return err
	}

	src := &models.BootSource{
		KernelImagePath: &kernelPath,
		BootArgs:        params,
	}

	fc.fcConfig.BootSource = src

	return nil
}

func (fc *firecracker) fcSetVMRootfs(ctx context.Context, path string) error {
	span, _ := fc.trace(ctx, "fcSetVMRootfs")
	defer span.End()

	jailedRootfs, err := fc.fcJailResource(path, fcRootfs)
	if err != nil {
		return err
	}

	driveID := "rootfs"
	isReadOnly := true
	//Add it as a regular block device
	//This allows us to use a partitoned root block device
	isRootDevice := false
	// This is the path within the jailed root
	drive := &models.Drive{
		DriveID:      &driveID,
		IsReadOnly:   &isReadOnly,
		IsRootDevice: &isRootDevice,
		PathOnHost:   &jailedRootfs,
	}

	fc.fcConfig.Drives = append(fc.fcConfig.Drives, drive)

	return nil
}

func (fc *firecracker) fcSetVMBaseConfig(ctx context.Context, mem int64, vcpus int64, htEnabled bool) {
	span, _ := fc.trace(ctx, "fcSetVMBaseConfig")
	defer span.End()
	fc.Logger().WithFields(logrus.Fields{"mem": mem,
		"vcpus":     vcpus,
		"htEnabled": htEnabled}).Debug("fcSetVMBaseConfig")

	cfg := &models.MachineConfiguration{
		HtEnabled:  &htEnabled,
		MemSizeMib: &mem,
		VcpuCount:  &vcpus,
	}

	fc.fcConfig.MachineConfig = cfg
}

func (fc *firecracker) fcSetLogger(ctx context.Context) error {
	span, _ := fc.trace(ctx, "fcSetLogger")
	defer span.End()

	fcLogLevel := "Error"
	if fc.config.Debug {
		fcLogLevel = "Debug"
	}

	// listen to log fifo file and transfer error info
	jailedLogFifo, err := fc.fcListenToFifo(fcLogFifo, nil)
	if err != nil {
		return fmt.Errorf("Failed setting log: %s", err)
	}

	fc.fcConfig.Logger = &models.Logger{
		Level:   &fcLogLevel,
		LogPath: &jailedLogFifo,
	}

	return err
}

func (fc *firecracker) fcSetMetrics(ctx context.Context) error {
	span, _ := fc.trace(ctx, "fcSetMetrics")
	defer span.End()

	// listen to metrics file and transfer error info
	jailedMetricsFifo, err := fc.fcListenToFifo(fcMetricsFifo, fc.updateMetrics)
	if err != nil {
		return fmt.Errorf("Failed setting log: %s", err)
	}

	fc.fcConfig.Metrics = &models.Metrics{
		MetricsPath: &jailedMetricsFifo,
	}

	return err
}

func (fc *firecracker) updateMetrics(line string) {
	var fm FirecrackerMetrics
	if err := json.Unmarshal([]byte(line), &fm); err != nil {
		fc.Logger().WithError(err).WithField("data", line).Error("failed to unmarshal fc metrics")
		return
	}
	updateFirecrackerMetrics(&fm)
}

type fifoConsumer func(string)

func (fc *firecracker) fcListenToFifo(fifoName string, consumer fifoConsumer) (string, error) {
	fcFifoPath := filepath.Join(fc.vmPath, fifoName)
	fcFifo, err := fifo.OpenFifo(context.Background(), fcFifoPath, syscall.O_CREAT|syscall.O_RDONLY|syscall.O_NONBLOCK, 0)
	if err != nil {
		return "", fmt.Errorf("Failed to open/create fifo file %s", err)
	}

	jailedFifoPath, err := fc.fcJailResource(fcFifoPath, fifoName)
	if err != nil {
		return "", err
	}

	go func() {
		scanner := bufio.NewScanner(fcFifo)
		for scanner.Scan() {
			if consumer != nil {
				consumer(scanner.Text())
			} else {
				fc.Logger().WithFields(logrus.Fields{
					"fifoName": fifoName,
					"contents": scanner.Text()}).Debug("read firecracker fifo")
			}
		}

		if err := scanner.Err(); err != nil {
			fc.Logger().WithError(err).Errorf("Failed reading firecracker fifo file")
		}

		if err := fcFifo.Close(); err != nil {
			fc.Logger().WithError(err).Errorf("Failed closing firecracker fifo file")
		}
	}()

	return jailedFifoPath, nil
}

func (fc *firecracker) fcInitConfiguration(ctx context.Context) error {
	// Firecracker API socket(firecracker.socket) is automatically created
	// under /run dir.
	err := os.MkdirAll(filepath.Join(fc.jailerRoot, "run"), DirMode)
	if err != nil {
		return err
	}
	defer func() {
		if err != nil {
			if err := os.RemoveAll(fc.vmPath); err != nil {
				fc.Logger().WithError(err).Error("Fail to clean up vm directory")
			}
		}
	}()

	if fc.config.JailerPath != "" {
		fc.jailed = true
		if err := fc.fcRemountJailerRootWithExec(); err != nil {
			return err
		}
	}

	fc.fcSetVMBaseConfig(ctx, int64(fc.config.MemorySize),
		int64(fc.config.NumVCPUs), false)

	kernelPath, err := fc.config.KernelAssetPath()
	if err != nil {
		return err
	}

	if fc.config.Debug {
		fcKernelParams = append(fcKernelParams, Param{"console", "ttyS0"})
	} else {
		fcKernelParams = append(fcKernelParams, []Param{
			{"8250.nr_uarts", "0"},
			// Tell agent where to send the logs
			{"agent.log_vport", fmt.Sprintf("%d", vSockLogsPort)},
		}...)
	}

	kernelParams := append(fc.config.KernelParams, fcKernelParams...)
	strParams := SerializeParams(kernelParams, "=")
	formattedParams := strings.Join(strParams, " ")
	if err := fc.fcSetBootSource(ctx, kernelPath, formattedParams); err != nil {
		return err
	}

	image, err := fc.config.InitrdAssetPath()
	if err != nil {
		return err
	}

	if image == "" {
		image, err = fc.config.ImageAssetPath()
		if err != nil {
			return err
		}
	}

	if err := fc.fcSetVMRootfs(ctx, image); err != nil {
		return err
	}

	if err := fc.createDiskPool(ctx); err != nil {
		return err
	}

	if err := fc.fcSetLogger(ctx); err != nil {
		return err
	}

	if err := fc.fcSetMetrics(ctx); err != nil {
		return err
	}

	fc.state.set(cfReady)
	for _, d := range fc.pendingDevices {
		if err := fc.addDevice(ctx, d.dev, d.devType); err != nil {
			return err
		}
	}

	// register firecracker specificed metrics
	registerFirecrackerMetrics()

	return nil
}

// startSandbox will start the hypervisor for the given sandbox.
// In the context of firecracker, this will start the hypervisor,
// for configuration, but not yet start the actual virtual machine
func (fc *firecracker) startSandbox(ctx context.Context, timeout int) error {
	span, _ := fc.trace(ctx, "startSandbox")
	defer span.End()

	if err := fc.fcInitConfiguration(ctx); err != nil {
		return err
	}

	data, errJSON := json.MarshalIndent(fc.fcConfig, "", "\t")
	if errJSON != nil {
		return errJSON
	}

	if err := ioutil.WriteFile(fc.fcConfigPath, data, 0640); err != nil {
		return err
	}

	var err error
	defer func() {
		if err != nil {
			fc.fcEnd(ctx, false)
		}
	}()

	// This needs to be done as late as possible, since all processes that
	// are executed by kata-runtime after this call, run with the SELinux
	// label. If these processes require privileged, we do not want to run
	// them under confinement.
	if err := label.SetProcessLabel(fc.config.SELinuxProcessLabel); err != nil {
		return err
	}
	defer label.SetProcessLabel("")

	err = fc.fcInit(ctx, fcTimeout)
	if err != nil {
		return err
	}

	// make sure 'others' don't have access to this socket
	err = os.Chmod(filepath.Join(fc.jailerRoot, defaultHybridVSocketName), 0640)
	if err != nil {
		return fmt.Errorf("Could not change socket permissions: %v", err)
	}

	fc.state.set(vmReady)
	return nil
}

func fcDriveIndexToID(i int) string {
	return "drive_" + strconv.Itoa(i)
}

func (fc *firecracker) createDiskPool(ctx context.Context) error {
	span, _ := fc.trace(ctx, "createDiskPool")
	defer span.End()

	for i := 0; i < fcDiskPoolSize; i++ {
		driveID := fcDriveIndexToID(i)
		isReadOnly := false
		isRootDevice := false

		// Create a temporary file as a placeholder backend for the drive
		jailedDrive, err := fc.createJailedDrive(driveID)
		if err != nil {
			return err
		}

		drive := &models.Drive{
			DriveID:      &driveID,
			IsReadOnly:   &isReadOnly,
			IsRootDevice: &isRootDevice,
			PathOnHost:   &jailedDrive,
		}

		fc.fcConfig.Drives = append(fc.fcConfig.Drives, drive)
	}

	return nil
}

func (fc *firecracker) umountResource(jailedPath string) {
	hostPath := filepath.Join(fc.jailerRoot, jailedPath)
	fc.Logger().WithField("resource", hostPath).Debug("Unmounting resource")
	err := syscall.Unmount(hostPath, syscall.MNT_DETACH)
	if err != nil {
		fc.Logger().WithError(err).Error("Failed to umount resource")
	}
}

// cleanup all jail artifacts
func (fc *firecracker) cleanupJail(ctx context.Context) {
	span, _ := fc.trace(ctx, "cleanupJail")
	defer span.End()

	fc.umountResource(fcKernel)
	fc.umountResource(fcRootfs)
	fc.umountResource(fcLogFifo)
	fc.umountResource(fcMetricsFifo)
	fc.umountResource(defaultFcConfig)
	// if running with jailer, we also need to umount fc.jailerRoot
	if fc.config.JailerPath != "" {
		if err := syscall.Unmount(fc.jailerRoot, syscall.MNT_DETACH); err != nil {
			fc.Logger().WithField("JailerRoot", fc.jailerRoot).WithError(err).Error("Failed to umount")
		}
	}

	fc.Logger().WithField("cleaningJail", fc.vmPath).Info()
	if err := os.RemoveAll(fc.vmPath); err != nil {
		fc.Logger().WithField("cleanupJail failed", err).Error()
	}
}

// stopSandbox will stop the Sandbox's VM.
func (fc *firecracker) stopSandbox(ctx context.Context, waitOnly bool) (err error) {
	span, _ := fc.trace(ctx, "stopSandbox")
	defer span.End()

	return fc.fcEnd(ctx, waitOnly)
}

func (fc *firecracker) pauseSandbox(ctx context.Context) error {
	return nil
}

func (fc *firecracker) saveSandbox() error {
	return nil
}

func (fc *firecracker) resumeSandbox(ctx context.Context) error {
	return nil
}

func (fc *firecracker) fcAddVsock(ctx context.Context, hvs types.HybridVSock) {
	span, _ := fc.trace(ctx, "fcAddVsock")
	defer span.End()

	udsPath := hvs.UdsPath
	if fc.jailed {
		udsPath = filepath.Join("/", defaultHybridVSocketName)
	}

	vsockID := "root"
	ctxID := defaultGuestVSockCID
	vsock := &models.Vsock{
		GuestCid: &ctxID,
		UdsPath:  &udsPath,
		VsockID:  &vsockID,
	}

	fc.fcConfig.Vsock = vsock
}

func (fc *firecracker) fcAddNetDevice(ctx context.Context, endpoint Endpoint) {
	span, _ := fc.trace(ctx, "fcAddNetDevice")
	defer span.End()

	ifaceID := endpoint.Name()

	// The implementation of rate limiter is based on TBF.
	// Rate Limiter defines a token bucket with a maximum capacity (size) to store tokens, and an interval for refilling purposes (refill_time).
	// The refill-rate is derived from size and refill_time, and it is the constant rate at which the tokens replenish.
	refillTime := uint64(1000)
	var rxRateLimiter models.RateLimiter
	rxSize := fc.config.RxRateLimiterMaxRate
	if rxSize > 0 {
		fc.Logger().Info("Add rx rate limiter")

		// kata-defined rxSize is in bits with scaling factors of 1000, but firecracker-defined
		// rxSize is in bytes with scaling factors of 1024, need reversion.
		rxSize = revertBytes(rxSize / 8)
		rxTokenBucket := models.TokenBucket{
			RefillTime: &refillTime,
			Size:       &rxSize,
		}
		rxRateLimiter = models.RateLimiter{
			Bandwidth: &rxTokenBucket,
		}
	}

	var txRateLimiter models.RateLimiter
	txSize := fc.config.TxRateLimiterMaxRate
	if txSize > 0 {
		fc.Logger().Info("Add tx rate limiter")

		// kata-defined txSize is in bits with scaling factors of 1000, but firecracker-defined
		// txSize is in bytes with scaling factors of 1024, need reversion.
		txSize = revertBytes(txSize / 8)
		txTokenBucket := models.TokenBucket{
			RefillTime: &refillTime,
			Size:       &txSize,
		}
		txRateLimiter = models.RateLimiter{
			Bandwidth: &txTokenBucket,
		}
	}

	ifaceCfg := &models.NetworkInterface{
		AllowMmdsRequests: false,
		GuestMac:          endpoint.HardwareAddr(),
		IfaceID:           &ifaceID,
		HostDevName:       &endpoint.NetworkPair().TapInterface.TAPIface.Name,
		RxRateLimiter:     &rxRateLimiter,
		TxRateLimiter:     &txRateLimiter,
	}

	fc.fcConfig.NetworkInterfaces = append(fc.fcConfig.NetworkInterfaces, ifaceCfg)
}

func (fc *firecracker) fcAddBlockDrive(ctx context.Context, drive config.BlockDrive) error {
	span, _ := fc.trace(ctx, "fcAddBlockDrive")
	defer span.End()

	driveID := drive.ID
	isReadOnly := false
	isRootDevice := false

	jailedDrive, err := fc.fcJailResource(drive.File, driveID)
	if err != nil {
		fc.Logger().WithField("fcAddBlockDrive failed", err).Error()
		return err
	}
	driveFc := &models.Drive{
		DriveID:      &driveID,
		IsReadOnly:   &isReadOnly,
		IsRootDevice: &isRootDevice,
		PathOnHost:   &jailedDrive,
	}

	fc.fcConfig.Drives = append(fc.fcConfig.Drives, driveFc)

	return nil
}

// Firecracker supports replacing the host drive used once the VM has booted up
func (fc *firecracker) fcUpdateBlockDrive(ctx context.Context, path, id string) error {
	span, _ := fc.trace(ctx, "fcUpdateBlockDrive")
	defer span.End()

	// Use the global block index as an index into the pool of the devices
	// created for firecracker.
	driveParams := ops.NewPatchGuestDriveByIDParams()
	driveParams.SetDriveID(id)

	driveFc := &models.PartialDrive{
		DriveID:    &id,
		PathOnHost: &path, //This is the only property that can be modified
	}

	driveParams.SetBody(driveFc)
	if _, err := fc.client(ctx).Operations.PatchGuestDriveByID(driveParams); err != nil {
		return err
	}

	return nil
}

// addDevice will add extra devices to firecracker.  Limited to configure before the
// virtual machine starts.  Devices include drivers and network interfaces only.
func (fc *firecracker) addDevice(ctx context.Context, devInfo interface{}, devType deviceType) error {
	span, _ := fc.trace(ctx, "addDevice")
	defer span.End()

	fc.state.RLock()
	defer fc.state.RUnlock()

	if fc.state.state == notReady {
		dev := firecrackerDevice{
			dev:     devInfo,
			devType: devType,
		}
		fc.Logger().Info("FC not ready, queueing device")
		fc.pendingDevices = append(fc.pendingDevices, dev)
		return nil
	}

	var err error
	switch v := devInfo.(type) {
	case Endpoint:
		fc.Logger().WithField("device-type-endpoint", devInfo).Info("Adding device")
		fc.fcAddNetDevice(ctx, v)
	case config.BlockDrive:
		fc.Logger().WithField("device-type-blockdrive", devInfo).Info("Adding device")
		err = fc.fcAddBlockDrive(ctx, v)
	case types.HybridVSock:
		fc.Logger().WithField("device-type-hybrid-vsock", devInfo).Info("Adding device")
		fc.fcAddVsock(ctx, v)
	default:
		fc.Logger().WithField("unknown-device-type", devInfo).Error("Adding device")
	}

	return err
}

// hotplugBlockDevice supported in Firecracker VMM
// hot add or remove a block device.
func (fc *firecracker) hotplugBlockDevice(ctx context.Context, drive config.BlockDrive, op operation) (interface{}, error) {
	var path string
	var err error
	driveID := fcDriveIndexToID(drive.Index)

	if op == addDevice {
		//The drive placeholder has to exist prior to Update
		path, err = fc.fcJailResource(drive.File, driveID)
		if err != nil {
			fc.Logger().WithError(err).WithField("resource", drive.File).Error("Could not jail resource")
			return nil, err
		}
	} else {
		// umount the disk, it's no longer needed.
		fc.umountResource(driveID)
		// use previous raw file created at createDiskPool, that way
		// the resource is released by firecracker and it can be destroyed in the host
		path = filepath.Join(fc.jailerRoot, driveID)
	}

	return nil, fc.fcUpdateBlockDrive(ctx, path, driveID)
}

// hotplugAddDevice supported in Firecracker VMM
func (fc *firecracker) hotplugAddDevice(ctx context.Context, devInfo interface{}, devType deviceType) (interface{}, error) {
	span, _ := fc.trace(ctx, "hotplugAddDevice")
	defer span.End()

	switch devType {
	case blockDev:
		return fc.hotplugBlockDevice(ctx, *devInfo.(*config.BlockDrive), addDevice)
	default:
		fc.Logger().WithFields(logrus.Fields{"devInfo": devInfo,
			"deviceType": devType}).Warn("hotplugAddDevice: unsupported device")
		return nil, fmt.Errorf("Could not hot add device: unsupported device: %v, type: %v",
			devInfo, devType)
	}
}

// hotplugRemoveDevice supported in Firecracker VMM
func (fc *firecracker) hotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType deviceType) (interface{}, error) {
	span, _ := fc.trace(ctx, "hotplugRemoveDevice")
	defer span.End()

	switch devType {
	case blockDev:
		return fc.hotplugBlockDevice(ctx, *devInfo.(*config.BlockDrive), removeDevice)
	default:
		fc.Logger().WithFields(logrus.Fields{"devInfo": devInfo,
			"deviceType": devType}).Error("hotplugRemoveDevice: unsupported device")
		return nil, fmt.Errorf("Could not hot remove device: unsupported device: %v, type: %v",
			devInfo, devType)
	}
}

// getSandboxConsole builds the path of the console where we can read
// logs coming from the sandbox.
func (fc *firecracker) getSandboxConsole(ctx context.Context, id string) (string, string, error) {
	master, slave, err := console.NewPty()
	if err != nil {
		fc.Logger().Debugf("Error create pseudo tty: %v", err)
		return consoleProtoPty, "", err
	}
	fc.console = master

	return consoleProtoPty, slave, nil
}

func (fc *firecracker) disconnect(ctx context.Context) {
	fc.state.set(notReady)
}

// Adds all capabilities supported by firecracker implementation of hypervisor interface
func (fc *firecracker) capabilities(ctx context.Context) types.Capabilities {
	span, _ := fc.trace(ctx, "capabilities")
	defer span.End()
	var caps types.Capabilities
	caps.SetBlockDeviceHotplugSupport()

	return caps
}

func (fc *firecracker) hypervisorConfig() HypervisorConfig {
	return fc.config
}

func (fc *firecracker) resizeMemory(ctx context.Context, reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, memoryDevice, error) {
	return 0, memoryDevice{}, nil
}

func (fc *firecracker) resizeVCPUs(ctx context.Context, reqVCPUs uint32) (currentVCPUs uint32, newVCPUs uint32, err error) {
	return 0, 0, nil
}

// This is used to apply cgroup information on the host.
//
// As suggested by https://github.com/firecracker-microvm/firecracker/issues/718,
// let's use `ps -T -p <pid>` to get fc vcpu info.
func (fc *firecracker) getThreadIDs(ctx context.Context) (vcpuThreadIDs, error) {
	var vcpuInfo vcpuThreadIDs

	vcpuInfo.vcpus = make(map[int]int)
	parent, err := utils.NewProc(fc.info.PID)
	if err != nil {
		return vcpuInfo, err
	}
	children, err := parent.Children()
	if err != nil {
		return vcpuInfo, err
	}
	for _, child := range children {
		comm, err := child.Comm()
		if err != nil {
			return vcpuInfo, errors.New("Invalid fc thread info")
		}
		if !strings.HasPrefix(comm, "fc_vcpu") {
			continue
		}
		cpus := strings.SplitAfter(comm, "fc_vcpu")
		if len(cpus) != 2 {
			return vcpuInfo, errors.Errorf("Invalid fc thread info: %v", comm)
		}
		cpuID, err := strconv.ParseInt(cpus[1], 10, 32)
		if err != nil {
			return vcpuInfo, errors.Wrapf(err, "Invalid fc thread info: %v", comm)
		}
		vcpuInfo.vcpus[int(cpuID)] = child.PID
	}

	return vcpuInfo, nil
}

func (fc *firecracker) cleanup(ctx context.Context) error {
	fc.cleanupJail(ctx)
	return nil
}

func (fc *firecracker) getPids() []int {
	return []int{fc.info.PID}
}

func (fc *firecracker) getVirtioFsPid() *int {
	return nil
}

func (fc *firecracker) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	return errors.New("firecracker is not supported by VM cache")
}

func (fc *firecracker) toGrpc(ctx context.Context) ([]byte, error) {
	return nil, errors.New("firecracker is not supported by VM cache")
}

func (fc *firecracker) save() (s persistapi.HypervisorState) {
	s.Pid = fc.info.PID
	s.Type = string(FirecrackerHypervisor)
	return
}

func (fc *firecracker) load(s persistapi.HypervisorState) {
	fc.info.PID = s.Pid
}

func (fc *firecracker) check() error {
	if err := syscall.Kill(fc.info.PID, syscall.Signal(0)); err != nil {
		return errors.Wrapf(err, "failed to ping fc process")
	}

	return nil
}

func (fc *firecracker) generateSocket(id string) (interface{}, error) {
	fc.Logger().Debug("Using hybrid-vsock endpoint")
	udsPath := filepath.Join(fc.jailerRoot, defaultHybridVSocketName)

	return types.HybridVSock{
		UdsPath: udsPath,
		Port:    uint32(vSockPort),
	}, nil
}

func (fc *firecracker) isRateLimiterBuiltin() bool {
	return true
}

// In firecracker, it accepts the size of rate limiter in scaling factors of 2^10(1024)
// But in kata-defined rate limiter, for better Human-readability, we prefer scaling factors of 10^3(1000).
// func revertByte reverts num from scaling factors of 1000 to 1024, e.g. 10000000(10MB) to 10485760.
func revertBytes(num uint64) uint64 {
	a := num / 1000
	b := num % 1000
	if a == 0 {
		return num
	}
	return 1024*revertBytes(a) + b
}

func (fc *firecracker) setSandbox(sandbox *Sandbox) {
}
