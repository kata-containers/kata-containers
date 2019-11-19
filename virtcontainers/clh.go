// Copyright (c) 2019 Ericsson Eurolab Deutschland GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"context"
	"fmt"
	"io/ioutil"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"

	persistapi "github.com/kata-containers/runtime/virtcontainers/persist/api"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/pkg/errors"
	log "github.com/sirupsen/logrus"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/store"
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
	clhTimeout            = 10
	clhSocket             = "clh.sock"
	clhAPISocket          = "clh-api.sock"
	clhLogFile            = "clh.log"
	virtioFsSocket        = "virtiofsd.sock"
	clhSerial             = "serial-tty.log"
	supportedMajorVersion = 0
	supportedMinorVersion = 3
	defaultClhPath        = "/usr/local/bin/cloud-hypervisor"
	virtioFsCacheAlways   = "always"
)

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
}

func (s *CloudHypervisorState) reset() {
	s.PID = 0
	s.VirtiofsdPID = 0
	s.state = clhNotReady
}

type cloudHypervisor struct {
	id         string
	state      CloudHypervisorState
	store      *store.VCStore
	config     HypervisorConfig
	ctx        context.Context
	socketPath string
	version    CloudHypervisorVersion
	cliBuilder *DefaultCLIBuilder
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

func (clh *cloudHypervisor) createSandbox(ctx context.Context, id string, networkNS NetworkNamespace, hypervisorConfig *HypervisorConfig, vcStore *store.VCStore) error {
	clh.ctx = ctx

	span, _ := clh.trace("createSandbox")
	defer span.Finish()

	err := hypervisorConfig.valid()
	if err != nil {
		return err
	}

	clh.id = id
	clh.store = vcStore
	clh.config = *hypervisorConfig
	clh.state.state = clhNotReady

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

	clh.cliBuilder = &DefaultCLIBuilder{}

	socketPath, err := clh.vsockSocketPath(id)
	if err != nil {
		clh.Logger().Info("Invalid socket path for cloud-hypervisor")
		return nil
	}
	clh.socketPath = socketPath

	clh.Logger().WithField("function", "createSandbox").Info("creating Sandbox")

	// No need to return an error from there since there might be nothing
	// to fetch if this is the first time the hypervisor is created.
	if err := clh.store.Load(store.Hypervisor, &clh.state); err != nil {
		clh.Logger().WithField("function", "createSandbox").WithError(err).Info("No info could be fetched")
	}

	// Set initial memomory size of the cloud hypervisor
	clh.cliBuilder.SetMemory(&CLIMemory{
		memorySize:  clh.config.MemorySize,
		backingFile: "/dev/shm",
	})
	// Set initial amount of cpu's for the cloud hypervisor
	clh.cliBuilder.SetCpus(&CLICpus{
		cpus: clh.config.NumVCPUs,
	})

	// Add the kernel path
	kernelPath, err := clh.config.KernelAssetPath()
	if err != nil {
		return err
	}
	clh.cliBuilder.SetKernel(&CLIKernel{
		path: kernelPath,
	})

	// First take the default parameters defined by this driver
	clh.cliBuilder.AddKernelParameters(clhKernelParams)

	// Followed by extra debug parameters if debug enabled in configuration file
	if clh.config.Debug {
		clh.cliBuilder.AddKernelParameters(clhDebugKernelParams)
	}

	// Followed by extra debug parameters defined in the configuration file
	clh.cliBuilder.AddKernelParameters(clh.config.KernelParams)

	// set random device generator to hypervisor
	clh.cliBuilder.SetRng(&CLIRng{
		src:   clh.config.EntropySource,
		iommu: false,
	})

	// Add the hybrid vsock device to hypervisor
	clh.cliBuilder.SetVsock(&CLIVsock{
		cid:        3,
		socketPath: clh.socketPath,
		iommu:      false,
	})

	// set the initial root/boot disk of hypervisor
	imagePath, err := clh.config.ImageAssetPath()
	if err != nil {
		return err
	}

	if imagePath != "" {
		clh.cliBuilder.SetDisk(&CLIDisk{
			path:  imagePath,
			iommu: false,
		})
	}

	// set the virtio-fs to the hypervisor
	vfsdSockPath, err := clh.virtioFsSocketPath(clh.id)
	if err != nil {
		return err
	}
	if clh.config.VirtioFSCache == virtioFsCacheAlways {
		clh.cliBuilder.SetFs(&CLIFs{
			tag:        "kataShared",
			socketPath: vfsdSockPath,
			queues:     1,
			queueSize:  512,
			dax:        true,
		})
	} else {
		clh.cliBuilder.SetFs(&CLIFs{
			tag:        "kataShared",
			socketPath: vfsdSockPath,
			queues:     1,
			queueSize:  512,
		})
	}

	// set the serial console to the cloud hypervisor
	if clh.config.Debug {
		serialPath, err := clh.serialPath(clh.id)
		if err != nil {
			return err
		}
		clh.cliBuilder.SetSerial(&CLISerialConsole{
			consoleType: cctFILE,
			filePath:    serialPath,
		})
		logFilePath, err := clh.logFilePath(clh.id)
		if err != nil {
			return err
		}
		clh.cliBuilder.SetLogFile(&CLILogFile{
			path: logFilePath,
		})
	}

	clh.cliBuilder.SetConsole(&CLIConsole{
		consoleType: cctOFF,
	})

	// Move the API endpoint socket location for the
	// by default enabled api endpoint
	apiSocketPath, err := clh.apiSocketPath(id)
	if err != nil {
		clh.Logger().Info("Invalid api socket path for cloud-hypervisor")
		return nil
	}
	clh.cliBuilder.SetAPISocket(&CLIAPISocket{
		socketPath: apiSocketPath,
	})

	return nil
}

func (clh *cloudHypervisor) startSandbox(timeout int) error {
	span, _ := clh.trace("startSandbox")
	defer span.Finish()

	clh.Logger().WithField("function", "startSandbox").Info("starting Sandbox")

	vmPath := filepath.Join(store.RunVMStoragePath(), clh.id)
	err := os.MkdirAll(vmPath, store.DirMode)
	if err != nil {
		return err
	}

	if clh.config.SharedFS == config.VirtioFS {
		clh.Logger().WithField("function", "startSandbox").Info("Starting virtiofsd")
		_, err = clh.setupVirtiofsd(timeout)
		if err != nil {
			return err
		}
		if err = clh.storeState(); err != nil {
			return err
		}
	} else {
		return errors.New("cloud-hypervisor only supports virtio based file sharing")
	}

	var strErr string
	strErr, pid, err := clh.LaunchClh()
	if err != nil {
		return fmt.Errorf("failed to launch cloud-hypervisor: %s, error messages from log: %s", err, strErr)
	}
	if err := clh.waitVMM(clhTimeout); err != nil {
		clh.Logger().WithField("error", err).Warn("cloud-hypervisor init failed")
		clh.shutdownVirtiofsd()
		return err
	}

	clh.state.PID = pid
	clh.state.state = clhReady
	clh.storeState()

	return nil
}

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

func (clh *cloudHypervisor) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, store *store.VCStore, j []byte) error {
	return errors.New("cloudHypervisor is not supported by VM cache")
}

func (clh *cloudHypervisor) toGrpc() ([]byte, error) {
	return nil, errors.New("cloudHypervisor is not supported by VM cache")
}

func (clh *cloudHypervisor) save() (s persistapi.HypervisorState) {
	s.Pid = clh.state.PID
	s.Type = string(ClhHypervisor)
	return
}

func (clh *cloudHypervisor) load(s persistapi.HypervisorState) {
	clh.state.PID = s.Pid
	clh.state.VirtiofsdPID = s.VirtiofsdPid
}

func (clh *cloudHypervisor) check() error {
	return nil
}

func (clh *cloudHypervisor) getPids() []int {

	var pids []int
	pids = append(pids, clh.state.PID)

	return pids
}

//###########################################################################
//
// Local helper methods related to the hypervisor interface implementation
//
//###########################################################################

func (clh *cloudHypervisor) addDevice(devInfo interface{}, devType deviceType) error {
	span, _ := clh.trace("addDevice")
	defer span.Finish()

	var err error

	switch v := devInfo.(type) {
	case Endpoint:
		clh.Logger().WithField("function", "addDevice").Infof("Adding Endpoint of type %v", v)
		clh.cliBuilder.AddNet(CLINet{
			device: v.Name(),
			mac:    v.HardwareAddr(),
		})

	default:
		clh.Logger().WithField("function", "addDevice").Warnf("Add device of type %v is not supported.", v)
	}

	return err
}

func (clh *cloudHypervisor) Logger() *log.Entry {
	return virtLog.WithField("subsystem", "cloudHypervisor")
}

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

	defer func() {
		if err != nil {
			clh.Logger().Info("Terminate Cloud Hypervisor failed")
		} else {
			clh.Logger().Info("Cloud Hypervisor stopped")
			clh.reset()
			clh.Logger().Debug("removing virtiofsd and vm sockets")
			path, err := clh.virtioFsSocketPath(clh.id)
			if err == nil {
				rerr := os.Remove(path)
				if rerr != nil {
					clh.Logger().WithField("path", path).Warn("removing virtiofsd socket failed")
				}
			}
			path, err = clh.vsockSocketPath(clh.id)
			if err == nil {
				rerr := os.Remove(path)
				if rerr != nil {
					clh.Logger().WithField("path", path).Warn("removing vm socket failed")
				}
			}
		}
	}()

	pid := clh.state.PID
	if pid == 0 {
		clh.Logger().WithField("PID", pid).Info("Skipping kill cloud hypervisor. invalid pid")
		return nil
	}
	clh.Logger().WithField("PID", pid).Info("Stopping Cloud Hypervisor")

	// Send a SIGTERM to the VM process to try to stop it properly
	if err = syscall.Kill(pid, syscall.SIGTERM); err != nil {
		if err == syscall.ESRCH {
			return nil
		}
		return err
	}

	// Wait for the VM process to terminate
	tInit := time.Now()
	for {
		if err = syscall.Kill(pid, syscall.Signal(0)); err != nil {
			return nil
		}

		if time.Since(tInit).Seconds() >= fcStopSandboxTimeout {
			clh.Logger().Warnf("VM still running after waiting %ds", fcStopSandboxTimeout)
			break
		}

		// Let's avoid to run a too busy loop
		time.Sleep(time.Duration(50) * time.Millisecond)
	}

	// Let's try with a hammer now, a SIGKILL should get rid of the
	// VM process.
	return syscall.Kill(pid, syscall.SIGKILL)
}

func (clh *cloudHypervisor) reset() {
	clh.state.reset()
	clh.storeState()
}

func (clh *cloudHypervisor) generateSocket(id string, useVsock bool) (interface{}, error) {
	if !useVsock {
		return nil, fmt.Errorf("Can't generate socket path for cloud-hypervisor: vsocks is disabled")
	}

	udsPath, err := clh.vsockSocketPath(id)
	if err != nil {
		clh.Logger().Info("Can't generate socket path for cloud-hypervisor")
		return types.HybridVSock{}, err
	}
	clh.Logger().WithField("function", "generateSocket").Infof("Using hybrid vsock %s:%d", udsPath, vSockPort)
	clh.socketPath = udsPath
	return types.HybridVSock{
		UdsPath: udsPath,
		Port:    uint32(vSockPort),
	}, nil
}

func (clh *cloudHypervisor) setupVirtiofsd(timeout int) (remain int, err error) {

	sockPath, perr := clh.virtioFsSocketPath(clh.id)
	if perr != nil {
		return 0, perr
	}

	theArgs, err := clh.virtiofsdArgs(sockPath)
	if err != nil {
		return 0, err
	}

	clh.Logger().WithField("path", clh.config.VirtioFSDaemon).Info()
	clh.Logger().WithField("args", strings.Join(theArgs, " ")).Info()

	cmd := exec.Command(clh.config.VirtioFSDaemon, theArgs...)
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return 0, err
	}

	if err = cmd.Start(); err != nil {
		return 0, err
	}
	defer func() {
		if err != nil {
			clh.state.VirtiofsdPID = 0
			cmd.Process.Kill()
		} else {
			clh.state.VirtiofsdPID = cmd.Process.Pid

		}
		clh.storeState()
	}()

	// Wait for socket to become available
	sockReady := make(chan error, 1)
	timeStart := time.Now()
	go func() {
		scanner := bufio.NewScanner(stderr)
		var sent bool
		for scanner.Scan() {
			if clh.config.Debug {
				clh.Logger().WithField("source", "virtiofsd").Debug(scanner.Text())
			}
			if !sent && strings.Contains(scanner.Text(), "Waiting for vhost-user socket connection...") {
				sockReady <- nil
				sent = true
			}
		}
		if !sent {
			if err := scanner.Err(); err != nil {
				sockReady <- err
			} else {
				sockReady <- fmt.Errorf("virtiofsd did not announce socket connection")
			}
		}
		clh.Logger().Info("virtiofsd quits")
		// Wait to release resources of virtiofsd process
		cmd.Process.Wait()

	}()

	return clh.waitVirtiofsd(timeStart, timeout, sockReady,
		fmt.Sprintf("virtiofsd (pid=%d) socket %s", cmd.Process.Pid, sockPath))
}

func (clh *cloudHypervisor) waitVirtiofsd(start time.Time, timeout int, ready chan error, errMsg string) (int, error) {
	var err error

	timeoutDuration := time.Duration(timeout) * time.Second
	select {
	case err = <-ready:
	case <-time.After(timeoutDuration):
		err = fmt.Errorf("timed out waiting for %s", errMsg)
	}
	if err != nil {
		return 0, err
	}

	// Now reduce timeout by the elapsed time
	elapsed := time.Since(start)
	if elapsed < timeoutDuration {
		timeout = timeout - int(elapsed.Seconds())
	} else {
		timeout = 0
	}
	return timeout, nil
}

func (clh *cloudHypervisor) virtiofsdArgs(sockPath string) ([]string, error) {

	sourcePath := filepath.Join(kataHostSharedDir(), clh.id)
	if _, err := os.Stat(sourcePath); os.IsNotExist(err) {
		os.MkdirAll(sourcePath, os.ModePerm)
	}

	args := []string{
		"-f",
		"-o", "vhost_user_socket=" + sockPath,
		"-o", "source=" + sourcePath,
		"-o", "cache=" + clh.config.VirtioFSCache}

	if len(clh.config.VirtioFSExtraArgs) != 0 {
		args = append(args, clh.config.VirtioFSExtraArgs...)
	}
	return args, nil
}

func (clh *cloudHypervisor) shutdownVirtiofsd() (err error) {

	err = syscall.Kill(-clh.state.VirtiofsdPID, syscall.SIGKILL)

	if err != nil {
		clh.state.VirtiofsdPID = 0
		clh.storeState()
	}
	return err

}

func (clh *cloudHypervisor) virtioFsSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(store.RunVMStoragePath(), id, virtioFsSocket)
}

func (clh *cloudHypervisor) vsockSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(store.RunVMStoragePath(), id, clhSocket)
}

func (clh *cloudHypervisor) serialPath(id string) (string, error) {
	return utils.BuildSocketPath(store.RunVMStoragePath(), id, clhSerial)
}

func (clh *cloudHypervisor) apiSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(store.RunVMStoragePath(), id, clhAPISocket)
}

func (clh *cloudHypervisor) logFilePath(id string) (string, error) {
	return utils.BuildSocketPath(store.RunVMStoragePath(), id, clhLogFile)
}

func (clh *cloudHypervisor) storeState() error {
	if clh.store != nil {
		if err := clh.store.Store(store.Hypervisor, clh.state); err != nil {
			return err
		}
	}
	return nil
}

func (clh *cloudHypervisor) waitVMM(timeout int) error {

	var err error
	timeoutDuration := time.Duration(timeout) * time.Second

	sockReady := make(chan error, 1)
	go func() {
		udsPath, err := clh.vsockSocketPath(clh.id)
		if err != nil {
			sockReady <- err
		}

		for {
			addr, err := net.ResolveUnixAddr("unix", udsPath)
			if err != nil {
				sockReady <- err
			}
			conn, err := net.DialUnix("unix", nil, addr)

			if err != nil {
				time.Sleep(50 * time.Millisecond)
			} else {
				conn.Close()
				sockReady <- nil

				break
			}
		}
	}()

	select {
	case err = <-sockReady:
	case <-time.After(timeoutDuration):
		err = fmt.Errorf("timed out waiting for cloud-hypervisor vsock")
	}

	time.Sleep(1000 * time.Millisecond)
	return err
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
	director := &CommandLineDirector{}

	cli, err := director.Build(clh.cliBuilder)
	if err != nil {
		return "", -1, err
	}

	clh.Logger().WithField("path", clhPath).Info()
	clh.Logger().WithField("args", strings.Join(cli.args, " ")).Info()

	cmd := exec.Command(clhPath, cli.args...)
	cmd.Stderr = ioutil.Discard

	if clh.config.Debug {
		cmd.Env = os.Environ()
		cmd.Env = append(cmd.Env, "RUST_BACKTRACE=FULL")
	}

	if err := cmd.Start(); err != nil {
		fmt.Println("Error starting cloudHypervisor", err)
		if cmd.Process != nil {
			cmd.Process.Kill()
		}
		return errStr, 0, err
	}

	return errStr, cmd.Process.Pid, nil
}

//###########################################################################
//
// Cloud-hypervisor CLI builder
//
//###########################################################################

const (
	cctOFF  string = "off"
	cctFILE string = "file"
)

const (
	cscApisocket string = "--api-socket"
	cscCmdline   string = "--cmdline"
	cscConsole   string = "--console"
	cscCpus      string = "--cpus"
	cscDisk      string = "--disk"
	cscFs        string = "--fs"
	cscKernel    string = "--kernel"
	cscLogFile   string = "--log-file"
	cscMemory    string = "--memory"
	cscNet       string = "--net"
	cscRng       string = "--rng"
	cscSerial    string = "--serial"
	cscVsock     string = "--vsock"
)

type CommandLineBuilder interface {
	AddKernelParameters(cmdline []Param)
	SetConsole(console *CLIConsole)
	SetCpus(cpus *CLICpus)
	SetDisk(disk *CLIDisk)
	SetFs(fs *CLIFs)
	SetKernel(kernel *CLIKernel)
	SetMemory(memory *CLIMemory)
	AddNet(net CLINet)
	SetRng(rng *CLIRng)
	SetSerial(serial *CLISerialConsole)
	SetVsock(vsock *CLIVsock)
	SetAPISocket(apiSocket *CLIAPISocket)
	SetLogFile(logFile *CLILogFile)
	GetCommandLine() (*CommandLine, error)
}

type CLIOption interface {
	Build(cmdline *CommandLine)
}

type CommandLine struct {
	args []string
}

//**********************************
// The (virtio) Console
//**********************************
type CLIConsole struct {
	consoleType string
	filePath    string
	iommu       bool
}

func (o *CLIConsole) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscConsole)

	consoleArg := ""
	if o.consoleType == cctFILE {
		consoleArg = o.consoleType + "=" + o.filePath
		if o.iommu {
			consoleArg += ",iommu=on"
		} else {
			consoleArg += ",iommu=off"
		}
	} else {
		consoleArg = o.consoleType
	}

	cmdline.args = append(cmdline.args, consoleArg)
}

//**********************************
// The serial port
//**********************************
type CLISerialConsole struct {
	consoleType string
	filePath    string
}

func (o *CLISerialConsole) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscSerial)
	if o.consoleType == cctFILE {
		cmdline.args = append(cmdline.args, o.consoleType+"="+o.filePath)
	} else {
		cmdline.args = append(cmdline.args, o.consoleType)
	}

}

//**********************************
// The API socket
//**********************************
type CLIAPISocket struct {
	socketPath string
}

func (o *CLIAPISocket) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscApisocket)
	if o.socketPath != "" {
		cmdline.args = append(cmdline.args, o.socketPath)
	}
}

//**********************************
// The amount of memory in Mb
//**********************************
type CLIMemory struct {
	memorySize  uint32
	backingFile string
}

func (o *CLIMemory) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscMemory)
	if o.backingFile == "" {
		cmdline.args = append(cmdline.args, "size="+strconv.FormatUint(uint64(o.memorySize), 10)+"M")
	} else {
		cmdline.args = append(cmdline.args, "size="+strconv.FormatUint(uint64(o.memorySize), 10)+"M,file="+o.backingFile)
	}

}

//**********************************
// The number of CPU's
//**********************************
type CLICpus struct {
	cpus uint32
}

func (o *CLICpus) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscCpus)
	cmdline.args = append(cmdline.args, strconv.FormatUint(uint64(o.cpus), 10))

}

//**********************************
// The Path to the kernel image
//**********************************
type CLIKernel struct {
	path string
}

func (o *CLIKernel) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscKernel)
	cmdline.args = append(cmdline.args, o.path)

}

//****************************************
// The Path to the root (boot) disk image
//****************************************
type CLIDisk struct {
	path  string
	iommu bool
}

func (o *CLIDisk) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscDisk)
	if o.iommu {
		cmdline.args = append(cmdline.args, "path="+o.path+",iommu=on")
	} else {
		cmdline.args = append(cmdline.args, "path="+o.path+",iommu=off")
	}

}

//****************************************
// The random device
//****************************************
type CLIRng struct {
	src   string
	iommu bool
}

func (o *CLIRng) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscRng)
	if o.iommu {
		cmdline.args = append(cmdline.args, "src="+o.src+",iommu=on")
	} else {
		cmdline.args = append(cmdline.args, "src="+o.src+",iommu=off")
	}

}

//****************************************
// The VSock socket
//****************************************
type CLIVsock struct {
	socketPath string
	cid        uint32
	iommu      bool
}

func (o *CLIVsock) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscVsock)
	if o.iommu {
		cmdline.args = append(cmdline.args, "cid="+strconv.FormatUint(uint64(o.cid), 10)+",sock="+o.socketPath+",iommu=on")
	} else {
		cmdline.args = append(cmdline.args, "cid="+strconv.FormatUint(uint64(o.cid), 10)+",sock="+o.socketPath+",iommu=off")
	}
}

//****************************************
// The shard (virtio) file system
//****************************************
type CLIFs struct {
	tag        string
	socketPath string
	queues     uint32
	queueSize  uint32
	dax        bool
}

func (o *CLIFs) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscFs)

	fsarg := "tag=" + o.tag + ",sock=" + o.socketPath
	if o.dax {
		fsarg += ",dax=on"
	} else {
		fsarg += ",num_queues=" + strconv.FormatUint(uint64(o.queues), 10) + ",queue_size=" + strconv.FormatUint(uint64(o.queueSize), 10)
	}
	cmdline.args = append(cmdline.args, fsarg)
}

//****************************************
// The net (nic)
//****************************************
type CLINet struct {
	device string
	mac    string
	iommu  bool
}

type CLINets struct {
	networks []CLINet
}

func (o *CLINets) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscNet)

	networks := ""
	netIndex := 1
	for _, net := range o.networks {
		tapName := "tap" + strconv.FormatUint(uint64(netIndex), 10)
		netIndex++
		if net.iommu {
			networks += "tap=" + tapName + ",mac=" + net.mac + ",iommu=on"
		} else {
			networks += "tap=" + tapName + ",mac=" + net.mac
		}
	}
	cmdline.args = append(cmdline.args, networks)
}

//****************************************
// The log file
//****************************************
type CLILogFile struct {
	path string
}

func (o *CLILogFile) Build(cmdline *CommandLine) {

	if o.path != "" {
		cmdline.args = append(cmdline.args, cscLogFile)
		cmdline.args = append(cmdline.args, o.path)
	}
}

//****************************************
// The kernel command line
//****************************************
type CLICmdline struct {
	params []Param
}

func (o *CLICmdline) Build(cmdline *CommandLine) {

	cmdline.args = append(cmdline.args, cscCmdline)

	var paramBuilder strings.Builder
	for _, p := range o.params {
		paramBuilder.WriteString(p.Key)
		if len(p.Value) > 0 {

			paramBuilder.WriteString("=")
			paramBuilder.WriteString(p.Value)
		}
		paramBuilder.WriteString(" ")
	}
	cmdline.args = append(cmdline.args, strings.TrimSpace(paramBuilder.String()))

}

//**********************************
// The Default Builder
//**********************************
type DefaultCLIBuilder struct {
	console   *CLIConsole
	serial    *CLISerialConsole
	apiSocket *CLIAPISocket
	cpus      *CLICpus
	memory    *CLIMemory
	kernel    *CLIKernel
	disk      *CLIDisk
	fs        *CLIFs
	rng       *CLIRng
	logFile   *CLILogFile
	vsock     *CLIVsock
	cmdline   *CLICmdline
	nets      *CLINets
}

func (d *DefaultCLIBuilder) AddKernelParameters(params []Param) {

	if d.cmdline == nil {
		d.cmdline = &CLICmdline{}
	}
	d.cmdline.params = append(d.cmdline.params, params...)
}

func (d *DefaultCLIBuilder) SetConsole(console *CLIConsole) {
	d.console = console
}

func (d *DefaultCLIBuilder) SetCpus(cpus *CLICpus) {
	d.cpus = cpus
}

func (d *DefaultCLIBuilder) SetDisk(disk *CLIDisk) {
	d.disk = disk
}

func (d *DefaultCLIBuilder) SetFs(fs *CLIFs) {
	d.fs = fs
}

func (d *DefaultCLIBuilder) SetKernel(kernel *CLIKernel) {
	d.kernel = kernel
}

func (d *DefaultCLIBuilder) SetMemory(memory *CLIMemory) {
	d.memory = memory
}

func (d *DefaultCLIBuilder) AddNet(net CLINet) {
	if d.nets == nil {
		d.nets = &CLINets{}
	}
	d.nets.networks = append(d.nets.networks, net)
}

func (d *DefaultCLIBuilder) SetRng(rng *CLIRng) {
	d.rng = rng
}

func (d *DefaultCLIBuilder) SetSerial(serial *CLISerialConsole) {
	d.serial = serial
}

func (d *DefaultCLIBuilder) SetVsock(vsock *CLIVsock) {
	d.vsock = vsock
}

func (d *DefaultCLIBuilder) SetAPISocket(apiSocket *CLIAPISocket) {
	d.apiSocket = apiSocket
}

func (d *DefaultCLIBuilder) SetLogFile(logFile *CLILogFile) {
	d.logFile = logFile
}

func (d *DefaultCLIBuilder) GetCommandLine() (*CommandLine, error) {

	cmdLine := &CommandLine{}

	if d.serial != nil {
		d.serial.Build(cmdLine)
	}

	if d.console != nil {
		d.console.Build(cmdLine)
	}

	if d.logFile != nil {
		d.logFile.Build(cmdLine)
	}

	if d.cpus != nil {
		d.cpus.Build(cmdLine)
	}
	if d.memory != nil {
		d.memory.Build(cmdLine)
	}
	if d.disk != nil {
		d.disk.Build(cmdLine)
	}
	if d.rng != nil {
		d.rng.Build(cmdLine)
	}
	if d.vsock != nil {
		d.vsock.Build(cmdLine)
	}
	if d.fs != nil {
		d.fs.Build(cmdLine)
	}
	if d.kernel != nil {
		d.kernel.Build(cmdLine)
	}
	if d.nets != nil {
		d.nets.Build(cmdLine)
	}
	if d.cmdline != nil {
		d.cmdline.Build(cmdLine)
	}

	return cmdLine, nil
}

type CommandLineDirector struct{}

func (s *CommandLineDirector) Build(builder CommandLineBuilder) (*CommandLine, error) {
	return builder.GetCommandLine()
}
