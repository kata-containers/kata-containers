// Copyright (c) 2019 Ericsson Eurolab Deutschland GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bytes"
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

	persistapi "github.com/kata-containers/runtime/virtcontainers/persist/api"
	chclient "github.com/kata-containers/runtime/virtcontainers/pkg/cloud-hypervisor/client"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/pkg/errors"
	log "github.com/sirupsen/logrus"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/kata-containers/runtime/virtcontainers/utils"
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
	clhTimeout            = 10
	clhAPITimeout         = 1
	clhStopSandboxTimeout = 3
	clhSocket             = "clh.sock"
	clhAPISocket          = "clh-api.sock"
	clhLogFile            = "clh.log"
	virtioFsSocket        = "virtiofsd.sock"
	clhSerial             = "serial-tty.log"
	supportedMajorVersion = 0
	supportedMinorVersion = 3
	defaultClhPath        = "/usr/local/bin/cloud-hypervisor"
	virtioFsCacheAlways   = "always"
	maxClhVcpus           = uint32(64)
)

// Interface that hides the implementation of openAPI client
// If the client changes  its methods, this interface should do it as well,
// The main purpose is to hide the client in an interface to allow mock testing.
// This is an interface that has to match with OpenAPI CLH client
type clhClient interface {
	VmmPingGet(ctx context.Context) (chclient.VmmPingResponse, *http.Response, error)
	ShutdownVMM(ctx context.Context) (*http.Response, error)
	CreateVM(ctx context.Context, vmConfig chclient.VmConfig) (*http.Response, error)
	// No lint: golint suggest to rename to VMInfoGet.
	VmInfoGet(ctx context.Context) (chclient.VmInfo, *http.Response, error) //nolint:golint
	BootVM(ctx context.Context) (*http.Response, error)
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
	cmdOutput bytes.Buffer
	virtiofsd Virtiofsd
	store     persistapi.PersistDriver
}

var clhKernelParams = []Param{

	{"root", "/dev/vda1"},
	{"panic", "1"},         // upon kernel panic wait 1 second before reboot
	{"no_timer_check", ""}, // do not check broken timer IRQ resources
	{"noreplace-smp", ""},  // do not replace SMP instructions
	{"agent.log_vport", fmt.Sprintf("%d", vSockLogsPort)}, // tell the agent where to send the logs
}

var clhDebugKernelParams = []Param{

	{"console", "ttyS0,115200n8"},     // enable serial console
	{"systemd.log_level", "debug"},    // enable systemd debug output
	{"systemd.log_target", "console"}, // send loggng to the console
	{"initcall_debug", "1"},           // print init call timing information to the console
}

//###########################################################
//
// hypervisor interface implementation for cloud-hypervisor
//
//###########################################################

// For cloudHypervisor this call only sets the internal structure up.
// The VM will be created and started through startSandbox().
func (clh *cloudHypervisor) createSandbox(ctx context.Context, id string, networkNS NetworkNamespace, hypervisorConfig *HypervisorConfig, stateful bool) error {
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

		if clh.version.Major < supportedMajorVersion && clh.version.Minor < supportedMinorVersion {
			errorMessage := fmt.Sprintf("Unsupported version: cloud-hypervisor %d.%d not supported by this driver version (%d.%d)",
				clh.version.Major,
				clh.version.Minor,
				supportedMajorVersion,
				supportedMinorVersion)
			return errors.New(errorMessage)
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
			sourcePath: filepath.Join(kataHostSharedDir(), clh.id),
			debug:      clh.config.Debug,
			socketPath: virtiofsdSocketPath,
		}
		return nil
	}

	// No need to return an error from there since there might be nothing
	// to fetch if this is the first time the hypervisor is created.
	clh.Logger().WithField("function", "createSandbox").WithError(err).Info("Sandbox not found creating ")

	// Set initial memomory size of the virtual machine
	clh.vmconfig.Memory.Size = int64(clh.config.MemorySize) << utils.MibToBytesShift
	clh.vmconfig.Memory.File = "/dev/shm"
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

	disk := chclient.DiskConfig{
		Path: imagePath,
	}
	clh.vmconfig.Disks = append(clh.vmconfig.Disks, disk)

	// set the serial console to the cloud hypervisor
	if clh.config.Debug {
		serialPath, err := clh.serialPath(clh.id)
		if err != nil {
			return err
		}
		clh.vmconfig.Serial = chclient.ConsoleConfig{
			Mode: cctFILE,
			File: serialPath,
		}

	} else {
		clh.vmconfig.Serial = chclient.ConsoleConfig{
			Mode: cctNULL,
		}
	}

	clh.vmconfig.Console = chclient.ConsoleConfig{
		Mode: cctOFF,
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
		sourcePath: filepath.Join(kataHostSharedDir(), clh.id),
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

	var strErr string
	strErr, pid, err := clh.LaunchClh()
	if err != nil {
		return fmt.Errorf("failed to launch cloud-hypervisor: %s, error messages from log: %s", err, strErr)
	}
	clh.state.PID = pid

	if err := clh.waitVMM(clhTimeout); err != nil {
		clh.Logger().WithField("error", err).WithField("output", clh.cmdOutput.String()).Warn("cloud-hypervisor init failed")
		if shutdownErr := clh.virtiofsd.Stop(); shutdownErr != nil {
			clh.Logger().WithField("error", shutdownErr).Warn("error shutting down Virtiofsd")
		}
		return err
	}

	if err := clh.bootVM(ctx); err != nil {
		return err
	}

	clh.state.state = clhReady
	return nil
}

// getSandboxConsole builds the path of the console where we can read
// logs coming from the sandbox.
func (clh *cloudHypervisor) getSandboxConsole(id string) (string, error) {
	clh.Logger().WithField("function", "getSandboxConsole").WithField("id", id).Info("Get Sandbox Console")
	return "", nil
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

func (clh *cloudHypervisor) hotplugAddDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	clh.Logger().WithField("function", "hotplugAddDevice").Warn("hotplug add device not supported")
	return nil, nil
}

func (clh *cloudHypervisor) hotplugRemoveDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	clh.Logger().WithField("function", "hotplugRemoveDevice").Warn("hotplug remove device not supported")
	return nil, nil
}

func (clh *cloudHypervisor) hypervisorConfig() HypervisorConfig {
	return clh.config
}

func (clh *cloudHypervisor) resizeMemory(reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, memoryDevice, error) {
	clh.Logger().WithField("function", "resizeMemory").Warn("not supported")
	return 0, memoryDevice{}, nil
}

func (clh *cloudHypervisor) resizeVCPUs(reqVCPUs uint32) (currentVCPUs uint32, newVCPUs uint32, err error) {
	clh.Logger().WithField("function", "resizeVCPUs").Warn("not supported")
	return 0, 0, nil
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
	return
}

func (clh *cloudHypervisor) load(s persistapi.HypervisorState) {
	clh.state.PID = s.Pid
	clh.state.VirtiofsdPID = s.VirtiofsdPid
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

func (clh *cloudHypervisor) generateSocket(id string, useVsock bool) (interface{}, error) {
	if !useVsock {
		return nil, fmt.Errorf("Can't generate hybrid vsocket for cloud-hypervisor: vsocks is disabled")
	}

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

func (clh *cloudHypervisor) serialPath(id string) (string, error) {
	return utils.BuildSocketPath(clh.store.RunVMStoragePath(), id, clhSerial)
}

func (clh *cloudHypervisor) apiSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(clh.store.RunVMStoragePath(), id, clhAPISocket)
}

func (clh *cloudHypervisor) logFilePath(id string) (string, error) {
	return utils.BuildSocketPath(clh.store.RunVMStoragePath(), id, clhLogFile)
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

	major, err := strconv.ParseUint(versionSplit[0], 10, 64)
	if err != nil {
		return err

	}
	minor, err := strconv.ParseUint(versionSplit[1], 10, 64)
	if err != nil {
		return err

	}
	revision, err := strconv.ParseUint(versionSplit[2], 10, 64)
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

func (clh *cloudHypervisor) LaunchClh() (string, int, error) {

	errStr := ""

	clhPath, err := clh.clhPath()
	if err != nil {
		return "", -1, err
	}

	args := []string{cscAPIsocket, clh.state.apiSocket}
	if clh.config.Debug {

		logfile, err := clh.logFilePath(clh.id)
		if err != nil {
			return "", -1, err
		}
		args = append(args, cscLogFile)
		args = append(args, logfile)
	}

	clh.Logger().WithField("path", clhPath).Info()
	clh.Logger().WithField("args", strings.Join(args, " ")).Info()

	cmd := exec.Command(clhPath, args...)
	cmd.Stdout = &clh.cmdOutput
	cmd.Stderr = &clh.cmdOutput

	if clh.config.Debug {
		cmd.Env = os.Environ()
		cmd.Env = append(cmd.Env, "RUST_BACKTRACE=full")
	}

	if err := utils.StartCmd(cmd); err != nil {
		fmt.Println("Error starting cloudHypervisor", err)
		if cmd.Process != nil {
			cmd.Process.Kill()
		}
		return errStr, 0, err
	}

	return errStr, cmd.Process.Pid, nil
}

// MaxClhVCPUs returns the maximum number of vCPUs supported
func MaxClhVCPUs() uint32 {
	return maxClhVcpus
}

//###########################################################################
//
// Cloud-hypervisor CLI builder
//
//###########################################################################

const (
	cctOFF  string = "Off"
	cctFILE string = "File"
	cctNULL string = "Null"
)

const (
	cscAPIsocket string = "--api-socket"
	cscLogFile   string = "--log-file"
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

	info, _, err := cl.VmInfoGet(ctx)

	if err != nil {
		return openAPIClientError(err)
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

	info, _, err = cl.VmInfoGet(ctx)

	if err != nil {
		return openAPIClientError(err)
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

	clh.vmconfig.Vsock = append(clh.vmconfig.Vsock, chclient.VsockConfig{Cid: cid, Sock: path})
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
				Sock:      vfsdSockPath,
			},
		}
	} else {
		clh.vmconfig.Fs = []chclient.FsConfig{
			{
				Tag:  volume.MountTag,
				Sock: vfsdSockPath,
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
