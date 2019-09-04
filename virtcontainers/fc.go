// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	httptransport "github.com/go-openapi/runtime/client"
	"github.com/go-openapi/strfmt"
	persistapi "github.com/kata-containers/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/runtime/virtcontainers/pkg/firecracker/client"
	models "github.com/kata-containers/runtime/virtcontainers/pkg/firecracker/client/models"
	ops "github.com/kata-containers/runtime/virtcontainers/pkg/firecracker/client/operations"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/store"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

type vmmState uint8

const (
	notReady vmmState = iota
	apiReady
	vmReady
)

const (
	//fcTimeout is the maximum amount of time in seconds to wait for the VMM to respond
	fcTimeout = 10
	fcSocket  = "api.socket"
	//Name of the files within jailer root
	//Having predefined names helps with cleanup
	fcKernel             = "vmlinux"
	fcRootfs             = "rootfs"
	fcStopSandboxTimeout = 15
	// This indicates the number of block devices that can be attached to the
	// firecracker guest VM.
	// We attach a pool of placeholder drives before the guest has started, and then
	// patch the replace placeholder drives with drives with actual contents.
	fcDiskPoolSize = 8
)

var fcKernelParams = append(commonVirtioblkKernelRootParams, []Param{
	// The boot source is the first partition of the first block device added
	{"pci", "off"},
	{"reboot", "k"},
	{"panic", "1"},
	{"iommu", "off"},
	{"8250.nr_uarts", "0"},
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
	case apiReady:
		return "FC API ready"
	case vmReady:
		return "FC VM ready"
	}

	return ""
}

// FirecrackerInfo contains information related to the hypervisor that we
// want to store on disk
type FirecrackerInfo struct {
	PID int
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

// firecracker is an Hypervisor interface implementation for the firecracker hypervisor.
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

	store          *store.VCStore
	ctx            context.Context
	config         HypervisorConfig
	pendingDevices []firecrackerDevice // Devices to be added when the FC API is ready

	state  firecrackerState
	jailed bool //Set to true if jailer is enabled
}

type firecrackerDevice struct {
	dev     interface{}
	devType deviceType
}

// Logger returns a logrus logger appropriate for logging firecracker  messages
func (fc *firecracker) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "firecracker")
}

func (fc *firecracker) trace(name string) (opentracing.Span, context.Context) {
	if fc.ctx == nil {
		fc.Logger().WithField("type", "bug").Error("trace called before context set")
		fc.ctx = context.Background()
	}

	span, ctx := opentracing.StartSpanFromContext(fc.ctx, name)

	span.SetTag("subsystem", "hypervisor")
	span.SetTag("type", "firecracker")

	return span, ctx
}

// bindMount bind mounts a source in to a destination. This will
// do some bookkeeping:
// * evaluate all symlinks
// * ensure the source exists
// * recursively create the destination
func (fc *firecracker) bindMount(ctx context.Context, source, destination string, readonly bool) error {
	span, _ := trace(ctx, "bindMount")
	defer span.Finish()

	if source == "" {
		return fmt.Errorf("source must be specified")
	}
	if destination == "" {
		return fmt.Errorf("destination must be specified")
	}

	absSource, err := filepath.EvalSymlinks(source)
	if err != nil {
		return fmt.Errorf("Could not resolve symlink for source %v", source)
	}

	if err := ensureDestinationExists(absSource, destination); err != nil {
		return fmt.Errorf("Could not create destination mount point %v: %v", destination, err)
	}

	if err := syscall.Mount(absSource, destination, "bind", syscall.MS_BIND|syscall.MS_SLAVE, ""); err != nil {
		return fmt.Errorf("Could not bind mount %v to %v: %v", absSource, destination, err)
	}

	// For readonly bind mounts, we need to remount with the readonly flag.
	// This is needed as only very recent versions of libmount/util-linux support "bind,ro"
	if readonly {
		return syscall.Mount(absSource, destination, "bind", uintptr(syscall.MS_BIND|syscall.MS_SLAVE|syscall.MS_REMOUNT|syscall.MS_RDONLY), "")
	}

	return nil
}

// For firecracker this call only sets the internal structure up.
// The sandbox will be created and started through startSandbox().
func (fc *firecracker) createSandbox(ctx context.Context, id string, networkNS NetworkNamespace, hypervisorConfig *HypervisorConfig, vcStore *store.VCStore) error {
	fc.ctx = ctx

	span, _ := fc.trace("createSandbox")
	defer span.Finish()

	//TODO: check validity of the hypervisor config provided
	//https://github.com/kata-containers/runtime/issues/1065
	fc.id = id
	fc.store = vcStore
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
	//store.ConfigStoragePath cannot be used as we need exec perms
	fc.chrootBaseDir = filepath.Join("/var/lib/", store.StoragePathSuffix)

	fc.vmPath = filepath.Join(fc.chrootBaseDir, hypervisorName, fc.id)
	fc.jailerRoot = filepath.Join(fc.vmPath, "root") // auto created by jailer
	fc.socketPath = filepath.Join(fc.jailerRoot, fcSocket)

	// So we need to repopulate this at startSandbox where it is valid
	fc.netNSPath = networkNS.NetNsPath

	// Till we create lower privileged kata user run as root
	// https://github.com/kata-containers/runtime/issues/1869
	fc.uid = "0"
	fc.gid = "0"

	// No need to return an error from there since there might be nothing
	// to fetch if this is the first time the hypervisor is created.
	if fc.store != nil {
		if err := fc.store.Load(store.Hypervisor, &fc.info); err != nil {
			fc.Logger().WithField("function", "init").WithError(err).Info("No info could be fetched")
		}
	}

	return nil
}

func (fc *firecracker) newFireClient() *client.Firecracker {
	span, _ := fc.trace("newFireClient")
	defer span.Finish()
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
	transport.Transport = socketTransport
	httpClient.SetTransport(transport)

	return httpClient
}

func (fc *firecracker) vmRunning() bool {
	resp, err := fc.client().Operations.DescribeInstance(nil)
	if err != nil {
		return false
	}

	// Be explicit
	switch *resp.Payload.State {
	case models.InstanceInfoStateStarting:
		// Unsure what we should do here
		fc.Logger().WithField("unexpected-state", models.InstanceInfoStateStarting).Debug("vmRunning")
		return false
	case models.InstanceInfoStateRunning:
		return true
	case models.InstanceInfoStateUninitialized, models.InstanceInfoStateHalting, models.InstanceInfoStateHalted:
		return false
	default:
		return false
	}
}

// waitVMM will wait for timeout seconds for the VMM to be up and running.
// This does not mean that the VM is up and running. It only indicates that the VMM is up and
// running and able to handle commands to setup and launch a VM
func (fc *firecracker) waitVMM(timeout int) error {
	span, _ := fc.trace("waitVMM")
	defer span.Finish()

	if timeout < 0 {
		return fmt.Errorf("Invalid timeout %ds", timeout)
	}

	timeStart := time.Now()
	for {
		_, err := fc.client().Operations.DescribeInstance(nil)
		if err == nil {
			return nil
		}

		if int(time.Since(timeStart).Seconds()) > timeout {
			return fmt.Errorf("Failed to connect to firecrackerinstance (timeout %ds): %v", timeout, err)
		}

		time.Sleep(time.Duration(10) * time.Millisecond)
	}
}

func (fc *firecracker) fcInit(timeout int) error {
	span, _ := fc.trace("fcInit")
	defer span.Finish()

	if fc.config.JailerPath != "" {
		fc.jailed = true
	}

	// Fetch sandbox network to be able to access it from the sandbox structure.
	var networkNS NetworkNamespace
	if fc.store != nil {
		if err := fc.store.Load(store.Network, &networkNS); err == nil {
			if networkNS.NetNsPath == "" {
				fc.Logger().WithField("NETWORK NAMESPACE NULL", networkNS).Warn()
			}
			fc.netNSPath = networkNS.NetNsPath
		}
	}

	err := os.MkdirAll(fc.jailerRoot, store.DirMode)
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

	var args []string
	var cmd *exec.Cmd

	//https://github.com/firecracker-microvm/firecracker/blob/master/docs/jailer.md#jailer-usage
	//--seccomp-level specifies whether seccomp filters should be installed and how restrictive they should be. Possible values are:
	//0 : disabled.
	//1 : basic filtering. This prohibits syscalls not whitelisted by Firecracker.
	//2 (default): advanced filtering. This adds further checks on some of the parameters of the allowed syscalls.
	if fc.jailed {
		args = []string{
			"--id", fc.id,
			"--node", "0", //FIXME: Comprehend NUMA topology or explicit ignore
			"--seccomp-level", "2",
			"--exec-file", fc.config.HypervisorPath,
			"--uid", "0", //https://github.com/kata-containers/runtime/issues/1869
			"--gid", "0",
			"--chroot-base-dir", fc.chrootBaseDir,
			"--daemonize",
		}
		if fc.netNSPath != "" {
			args = append(args, "--netns", fc.netNSPath)
		}
		cmd = exec.Command(fc.config.JailerPath, args...)
	} else {
		args = []string{"--api-sock", fc.socketPath}
		cmd = exec.Command(fc.config.HypervisorPath, args...)

	}

	fc.Logger().WithField("hypervisor args", args).Debug()
	fc.Logger().WithField("hypervisor cmd", cmd).Debug()
	if err := cmd.Start(); err != nil {
		fc.Logger().WithField("Error starting firecracker", err).Debug()
		return err
	}

	fc.info.PID = cmd.Process.Pid
	fc.firecrackerd = cmd
	fc.connection = fc.newFireClient()

	if err := fc.waitVMM(timeout); err != nil {
		fc.Logger().WithField("fcInit failed:", err).Debug()
		return err
	}

	fc.state.set(apiReady)

	// Store VMM information
	if fc.store != nil {
		return fc.store.Store(store.Hypervisor, fc.info)
	}
	return nil
}

func (fc *firecracker) fcEnd() (err error) {
	span, _ := fc.trace("fcEnd")
	defer span.Finish()

	fc.Logger().Info("Stopping firecracker VM")

	defer func() {
		if err != nil {
			fc.Logger().Info("fcEnd failed")
		} else {
			fc.Logger().Info("Firecracker VM stopped")
		}
	}()

	pid := fc.info.PID

	// Check if VM process is running, in case it is not, let's
	// return from here.
	if err = syscall.Kill(pid, syscall.Signal(0)); err != nil {
		return nil
	}

	// Send a SIGTERM to the VM process to try to stop it properly
	if err = syscall.Kill(pid, syscall.SIGTERM); err != nil {
		return err
	}

	// Wait for the VM process to terminate
	tInit := time.Now()
	for {
		if err = syscall.Kill(pid, syscall.Signal(0)); err != nil {
			return nil
		}

		if time.Since(tInit).Seconds() >= fcStopSandboxTimeout {
			fc.Logger().Warnf("VM still running after waiting %ds", fcStopSandboxTimeout)
			break
		}

		// Let's avoid to run a too busy loop
		time.Sleep(time.Duration(50) * time.Millisecond)
	}

	// Let's try with a hammer now, a SIGKILL should get rid of the
	// VM process.
	return syscall.Kill(pid, syscall.SIGKILL)
}

func (fc *firecracker) client() *client.Firecracker {
	span, _ := fc.trace("client")
	defer span.Finish()

	if fc.connection == nil {
		fc.connection = fc.newFireClient()
	}

	return fc.connection
}

func (fc *firecracker) fcJailResource(src, dst string) (string, error) {
	if src == "" || dst == "" {
		return "", fmt.Errorf("fcJailResource: invalid jail locations: src:%v, dst:%v",
			src, dst)
	}
	jailedLocation := filepath.Join(fc.jailerRoot, dst)
	if err := fc.bindMount(context.Background(), src, jailedLocation, false); err != nil {
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

func (fc *firecracker) fcSetBootSource(path, params string) error {
	span, _ := fc.trace("fcSetBootSource")
	defer span.Finish()
	fc.Logger().WithFields(logrus.Fields{"kernel-path": path,
		"kernel-params": params}).Debug("fcSetBootSource")

	kernelPath, err := fc.fcJailResource(path, fcKernel)
	if err != nil {
		return err
	}

	bootSrcParams := ops.NewPutGuestBootSourceParams()
	src := &models.BootSource{
		KernelImagePath: &kernelPath,
		BootArgs:        params,
	}
	bootSrcParams.SetBody(src)

	_, err = fc.client().Operations.PutGuestBootSource(bootSrcParams)
	return err
}

func (fc *firecracker) fcSetVMRootfs(path string) error {
	span, _ := fc.trace("fcSetVMRootfs")
	defer span.Finish()

	jailedRootfs, err := fc.fcJailResource(path, fcRootfs)
	if err != nil {
		return err
	}

	driveID := "rootfs"
	driveParams := ops.NewPutGuestDriveByIDParams()
	driveParams.SetDriveID(driveID)
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
	driveParams.SetBody(drive)
	_, err = fc.client().Operations.PutGuestDriveByID(driveParams)
	return err
}

func (fc *firecracker) fcSetVMBaseConfig(mem int64, vcpus int64, htEnabled bool) error {
	span, _ := fc.trace("fcSetVMBaseConfig")
	defer span.Finish()
	fc.Logger().WithFields(logrus.Fields{"mem": mem,
		"vcpus":     vcpus,
		"htEnabled": htEnabled}).Debug("fcSetVMBaseConfig")

	param := ops.NewPutMachineConfigurationParams()
	cfg := &models.MachineConfiguration{
		HtEnabled:  &htEnabled,
		MemSizeMib: &mem,
		VcpuCount:  &vcpus,
	}
	param.SetBody(cfg)
	_, err := fc.client().Operations.PutMachineConfiguration(param)
	return err
}

func (fc *firecracker) fcStartVM() error {
	fc.Logger().Info("start firecracker virtual machine")
	span, _ := fc.trace("fcStartVM")
	defer span.Finish()

	fc.Logger().Info("Starting VM")

	fc.connection = fc.newFireClient()

	actionParams := ops.NewCreateSyncActionParams()
	actionType := "InstanceStart"
	actionInfo := &models.InstanceActionInfo{
		ActionType: &actionType,
	}
	actionParams.SetInfo(actionInfo)
	_, err := fc.client().Operations.CreateSyncAction(actionParams)
	if err != nil {
		return err
	}

	fc.state.set(vmReady)
	return nil
}

// startSandbox will start the hypervisor for the given sandbox.
// In the context of firecracker, this will start the hypervisor,
// for configuration, but not yet start the actual virtual machine
func (fc *firecracker) startSandbox(timeout int) error {
	span, _ := fc.trace("startSandbox")
	defer span.Finish()

	err := fc.fcInit(fcTimeout)
	if err != nil {
		return err
	}

	defer func() {
		if err != nil {
			fc.fcEnd()
		}
	}()

	if err := fc.fcSetVMBaseConfig(int64(fc.config.MemorySize),
		int64(fc.config.NumVCPUs),
		false); err != nil {
		return err
	}

	kernelPath, err := fc.config.KernelAssetPath()
	if err != nil {
		return err
	}

	kernelParams := append(fc.config.KernelParams, fcKernelParams...)
	strParams := SerializeParams(kernelParams, "=")
	formattedParams := strings.Join(strParams, " ")
	fc.fcSetBootSource(kernelPath, formattedParams)

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

	fc.fcSetVMRootfs(image)
	fc.createDiskPool()

	for _, d := range fc.pendingDevices {
		if err = fc.addDevice(d.dev, d.devType); err != nil {
			return err
		}
	}

	if err := fc.fcStartVM(); err != nil {
		return err
	}

	return fc.waitVMM(timeout)
}

func fcDriveIndexToID(i int) string {
	return "drive_" + strconv.Itoa(i)
}

func (fc *firecracker) createDiskPool() error {
	span, _ := fc.trace("createDiskPool")
	defer span.Finish()

	for i := 0; i < fcDiskPoolSize; i++ {
		driveID := fcDriveIndexToID(i)
		driveParams := ops.NewPutGuestDriveByIDParams()
		driveParams.SetDriveID(driveID)
		isReadOnly := false
		isRootDevice := false

		// Create a temporary file as a placeholder backend for the drive
		//hostURL, err := fc.store.Raw("")
		hostURL, err := fc.store.Raw(driveID)
		if err != nil {
			return err
		}

		// We get a full URL from Raw(), we need to parse it.
		u, err := url.Parse(hostURL)
		if err != nil {
			return err
		}

		jailedDrive, err := fc.fcJailResource(u.Path, driveID)
		if err != nil {
			fc.Logger().WithField("createDiskPool failed", err).Error()
			return err
		}

		drive := &models.Drive{
			DriveID:      &driveID,
			IsReadOnly:   &isReadOnly,
			IsRootDevice: &isRootDevice,
			PathOnHost:   &jailedDrive,
		}
		driveParams.SetBody(drive)
		_, err = fc.client().Operations.PutGuestDriveByID(driveParams)
		if err != nil {
			return err
		}
	}

	return nil
}

func (fc *firecracker) umountResource(jailedPath string) {
	hostPath := filepath.Join(fc.jailerRoot, jailedPath)
	err := syscall.Unmount(hostPath, syscall.MNT_DETACH)
	if err != nil {
		fc.Logger().WithField("umountResource failed", err).Info()
	}
}

// cleanup all jail artifacts
func (fc *firecracker) cleanupJail() {
	span, _ := fc.trace("cleanupJail")
	defer span.Finish()

	fc.umountResource(fcKernel)
	fc.umountResource(fcRootfs)

	for i := 0; i < fcDiskPoolSize; i++ {
		fc.umountResource(fcDriveIndexToID(i))
	}

	//Run through the list second time as may have bindmounted
	//to the same location twice. In the future this needs to
	//be tracked so that we do not do this blindly
	for i := 0; i < fcDiskPoolSize; i++ {
		fc.umountResource(fcDriveIndexToID(i))
	}

	fc.Logger().WithField("cleaningJail", fc.vmPath).Info()
	if err := os.RemoveAll(fc.vmPath); err != nil {
		fc.Logger().WithField("cleanupJail failed", err).Error()
	}
}

// stopSandbox will stop the Sandbox's VM.
func (fc *firecracker) stopSandbox() (err error) {
	span, _ := fc.trace("stopSandbox")
	defer span.Finish()

	return fc.fcEnd()
}

func (fc *firecracker) pauseSandbox() error {
	return nil
}

func (fc *firecracker) saveSandbox() error {
	return nil
}

func (fc *firecracker) resumeSandbox() error {
	return nil
}

func (fc *firecracker) fcAddVsock(vs kataVSOCK) error {
	span, _ := fc.trace("fcAddVsock")
	defer span.Finish()

	vsockParams := ops.NewPutGuestVsockByIDParams()
	vsockID := "root"
	ctxID := int64(vs.contextID)
	udsPath := ""
	vsock := &models.Vsock{
		GuestCid: &ctxID,
		UdsPath: &udsPath,
		VsockID: &vsockID,
	}
	vsockParams.SetID(vsockID)
	vsockParams.SetBody(vsock)
	_, err := fc.client().Operations.PutGuestVsockByID(vsockParams)
	if err != nil {
		return err
	}
	//Still racy. There is no way to send an fd to the firecracker
	//REST API. We could release this just before we start the instance
	//but even that will not eliminate the race
	vs.vhostFd.Close()
	return nil
}

func (fc *firecracker) fcAddNetDevice(endpoint Endpoint) error {
	span, _ := fc.trace("fcAddNetDevice")
	defer span.Finish()

	cfg := ops.NewPutGuestNetworkInterfaceByIDParams()
	ifaceID := endpoint.Name()
	ifaceCfg := &models.NetworkInterface{
		AllowMmdsRequests: false,
		GuestMac:          endpoint.HardwareAddr(),
		IfaceID:           &ifaceID,
		HostDevName:       &endpoint.NetworkPair().TapInterface.TAPIface.Name,
	}
	cfg.SetBody(ifaceCfg)
	cfg.SetIfaceID(ifaceID)
	_, err := fc.client().Operations.PutGuestNetworkInterfaceByID(cfg)
	return err
}

func (fc *firecracker) fcAddBlockDrive(drive config.BlockDrive) error {
	span, _ := fc.trace("fcAddBlockDrive")
	defer span.Finish()

	driveID := drive.ID
	driveParams := ops.NewPutGuestDriveByIDParams()
	driveParams.SetDriveID(driveID)
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

	driveParams.SetBody(driveFc)
	_, err = fc.client().Operations.PutGuestDriveByID(driveParams)
	return err
}

// Firecracker supports replacing the host drive used once the VM has booted up
func (fc *firecracker) fcUpdateBlockDrive(drive config.BlockDrive) error {
	span, _ := fc.trace("fcUpdateBlockDrive")
	defer span.Finish()

	// Use the global block index as an index into the pool of the devices
	// created for firecracker.
	driveID := fcDriveIndexToID(drive.Index)
	driveParams := ops.NewPatchGuestDriveByIDParams()
	driveParams.SetDriveID(driveID)

	jailedDrive, err := fc.fcJailResource(drive.File, driveID)
	if err != nil {
		fc.Logger().WithField("fcUpdateBlockDrive failed", err).Error()
		return err
	}
	driveFc := &models.PartialDrive{
		DriveID:    &driveID,
		PathOnHost: &jailedDrive, //This is the only property that can be modified
	}

	driveParams.SetBody(driveFc)
	if _, err := fc.client().Operations.PatchGuestDriveByID(driveParams); err != nil {
		return err
	}

	// Rescan needs to used only if the VM is running
	if fc.vmRunning() {
		actionParams := ops.NewCreateSyncActionParams()
		actionType := "BlockDeviceRescan"
		actionInfo := &models.InstanceActionInfo{
			ActionType: &actionType,
			Payload:    driveID,
		}
		actionParams.SetInfo(actionInfo)
		_, err = fc.client().Operations.CreateSyncAction(actionParams)
		if err != nil {
			return err
		}
	}

	return nil
}

// addDevice will add extra devices to firecracker.  Limited to configure before the
// virtual machine starts.  Devices include drivers and network interfaces only.
func (fc *firecracker) addDevice(devInfo interface{}, devType deviceType) error {
	span, _ := fc.trace("addDevice")
	defer span.Finish()

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

	switch v := devInfo.(type) {
	case Endpoint:
		fc.Logger().WithField("device-type-endpoint", devInfo).Info("Adding device")
		return fc.fcAddNetDevice(v)
	case config.BlockDrive:
		fc.Logger().WithField("device-type-blockdrive", devInfo).Info("Adding device")
		return fc.fcAddBlockDrive(v)
	case kataVSOCK:
		fc.Logger().WithField("device-type-vsock", devInfo).Info("Adding device")
		return fc.fcAddVsock(v)
	default:
		fc.Logger().WithField("unknown-device-type", devInfo).Error("Adding device")
	}

	return nil
}

// hotplugAddDevice supported in Firecracker VMM
func (fc *firecracker) hotplugAddDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	span, _ := fc.trace("hotplugAddDevice")
	defer span.Finish()

	switch devType {
	case blockDev:
		//The drive placeholder has to exist prior to Update
		return nil, fc.fcUpdateBlockDrive(*devInfo.(*config.BlockDrive))
	default:
		fc.Logger().WithFields(logrus.Fields{"devInfo": devInfo,
			"deviceType": devType}).Warn("hotplugAddDevice: unsupported device")
		return nil, fmt.Errorf("hotplugAddDevice: unsupported device: devInfo:%v, deviceType%v",
			devInfo, devType)
	}
}

// hotplugRemoveDevice supported in Firecracker VMM, but no-op
func (fc *firecracker) hotplugRemoveDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	return nil, nil
}

// getSandboxConsole builds the path of the console where we can read
// logs coming from the sandbox.
//
// we can get logs from firecracker itself; WIP on enabling.  Who needs
// logs when you're just hacking?
func (fc *firecracker) getSandboxConsole(id string) (string, error) {
	return "", nil
}

func (fc *firecracker) disconnect() {
	fc.state.set(notReady)
}

// Adds all capabilities supported by firecracker implementation of hypervisor interface
func (fc *firecracker) capabilities() types.Capabilities {
	span, _ := fc.trace("capabilities")
	defer span.Finish()
	var caps types.Capabilities
	caps.SetFsSharingUnsupported()
	caps.SetBlockDeviceHotplugSupport()

	return caps
}

func (fc *firecracker) hypervisorConfig() HypervisorConfig {
	return fc.config
}

func (fc *firecracker) resizeMemory(reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, memoryDevice, error) {
	return 0, memoryDevice{}, nil
}

func (fc *firecracker) resizeVCPUs(reqVCPUs uint32) (currentVCPUs uint32, newVCPUs uint32, err error) {
	return 0, 0, nil
}

// This is used to apply cgroup information on the host.
//
// As suggested by https://github.com/firecracker-microvm/firecracker/issues/718,
// let's use `ps -T -p <pid>` to get fc vcpu info.
func (fc *firecracker) getThreadIDs() (vcpuThreadIDs, error) {
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

func (fc *firecracker) cleanup() error {
	fc.cleanupJail()
	return nil
}

func (fc *firecracker) getPids() []int {
	return []int{fc.info.PID}
}

func (fc *firecracker) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, store *store.VCStore, j []byte) error {
	return errors.New("firecracker is not supported by VM cache")
}

func (fc *firecracker) toGrpc() ([]byte, error) {
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
