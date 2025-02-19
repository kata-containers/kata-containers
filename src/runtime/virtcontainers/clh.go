//go:build linux

// Copyright (c) 2019 Ericsson Eurolab Deutschland GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/httputil"
	"os"
	"os/exec"
	"os/user"
	"path/filepath"
	"regexp"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/containerd/console"
	chclient "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/cloud-hypervisor/client"
	"github.com/opencontainers/selinux/go-selinux/label"
	"github.com/pkg/errors"
	log "github.com/sirupsen/logrus"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	hv "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	pkgUtils "github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// clhTracingTags defines tags for the trace span
var clhTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "hypervisor",
	"type":      "clh",
}

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
	clhTimeout                     = 10
	clhAPITimeout                  = 1
	clhAPITimeoutConfidentialGuest = 20
	// Minimum timout for calling CreateVM followed by BootVM. Executing these two APIs
	// might take longer than the value returned by getClhAPITimeout().
	clhCreateAndBootVMMinimumTimeout = 10
	// Timeout for hot-plug - hotplug devices can take more time, than usual API calls
	// Use longer time timeout for it.
	clhHotPlugAPITimeout                   = 5
	clhStopSandboxTimeout                  = 3
	clhStopSandboxTimeoutConfidentialGuest = 10
	clhSocket                              = "clh.sock"
	clhAPISocket                           = "clh-api.sock"
	virtioFsSocket                         = "virtiofsd.sock"
	defaultClhPath                         = "/usr/local/bin/cloud-hypervisor"
)

// Interface that hides the implementation of openAPI client
// If the client changes  its methods, this interface should do it as well,
// The main purpose is to hide the client in an interface to allow mock testing.
// This is an interface that has to match with OpenAPI CLH client
type clhClient interface {
	// Check for the REST API availability
	VmmPingGet(ctx context.Context) (chclient.VmmPingResponse, *http.Response, error)
	// Shut the VMM down
	ShutdownVMM(ctx context.Context) (*http.Response, error)
	// Create the VM
	CreateVM(ctx context.Context, vmConfig chclient.VmConfig) (*http.Response, error)
	// Dump the VM information
	// No lint: golint suggest to rename to VMInfoGet.
	VmInfoGet(ctx context.Context) (chclient.VmInfo, *http.Response, error) //nolint:golint
	// Boot the VM
	BootVM(ctx context.Context) (*http.Response, error)
	// Add/remove CPUs to/from the VM
	VmResizePut(ctx context.Context, vmResize chclient.VmResize) (*http.Response, error)
	// Add VFIO PCI device to the VM
	VmAddDevicePut(ctx context.Context, deviceConfig chclient.DeviceConfig) (chclient.PciDeviceInfo, *http.Response, error)
	// Add a new disk device to the VM
	VmAddDiskPut(ctx context.Context, diskConfig chclient.DiskConfig) (chclient.PciDeviceInfo, *http.Response, error)
	// Remove a device from the VM
	VmRemoveDevicePut(ctx context.Context, vmRemoveDevice chclient.VmRemoveDevice) (*http.Response, error)
}

type clhClientApi struct {
	ApiInternal *chclient.DefaultApiService
}

func (c *clhClientApi) VmmPingGet(ctx context.Context) (chclient.VmmPingResponse, *http.Response, error) {
	return c.ApiInternal.VmmPingGet(ctx).Execute()
}

func (c *clhClientApi) ShutdownVMM(ctx context.Context) (*http.Response, error) {
	return c.ApiInternal.ShutdownVMM(ctx).Execute()
}

func (c *clhClientApi) CreateVM(ctx context.Context, vmConfig chclient.VmConfig) (*http.Response, error) {
	return c.ApiInternal.CreateVM(ctx).VmConfig(vmConfig).Execute()
}

//nolint:golint
func (c *clhClientApi) VmInfoGet(ctx context.Context) (chclient.VmInfo, *http.Response, error) {
	return c.ApiInternal.VmInfoGet(ctx).Execute()
}

func (c *clhClientApi) BootVM(ctx context.Context) (*http.Response, error) {
	return c.ApiInternal.BootVM(ctx).Execute()
}

func (c *clhClientApi) VmResizePut(ctx context.Context, vmResize chclient.VmResize) (*http.Response, error) {
	return c.ApiInternal.VmResizePut(ctx).VmResize(vmResize).Execute()
}

func (c *clhClientApi) VmAddDevicePut(ctx context.Context, deviceConfig chclient.DeviceConfig) (chclient.PciDeviceInfo, *http.Response, error) {
	return c.ApiInternal.VmAddDevicePut(ctx).DeviceConfig(deviceConfig).Execute()
}

func (c *clhClientApi) VmAddDiskPut(ctx context.Context, diskConfig chclient.DiskConfig) (chclient.PciDeviceInfo, *http.Response, error) {
	return c.ApiInternal.VmAddDiskPut(ctx).DiskConfig(diskConfig).Execute()
}

func (c *clhClientApi) VmRemoveDevicePut(ctx context.Context, vmRemoveDevice chclient.VmRemoveDevice) (*http.Response, error) {
	return c.ApiInternal.VmRemoveDevicePut(ctx).VmRemoveDevice(vmRemoveDevice).Execute()
}

// This is done in order to be able to override such a function as part of
// our unit tests, as when testing bootVM we're on a mocked scenario already.
var vmAddNetPutRequest = func(clh *cloudHypervisor) error {
	if clh.netDevices == nil {
		clh.Logger().Info("No network device has been configured by the upper layer")
		return nil
	}

	addr, err := net.ResolveUnixAddr("unix", clh.state.apiSocket)
	if err != nil {
		return err
	}

	conn, err := net.DialUnix("unix", nil, addr)
	if err != nil {
		return err
	}
	defer conn.Close()

	for _, netDevice := range *clh.netDevices {
		clh.Logger().Infof("Adding the net device to the Cloud Hypervisor VM configuration: %+v", netDevice)

		netDeviceAsJson, err := json.Marshal(netDevice)
		if err != nil {
			return err
		}
		netDeviceAsIoReader := bytes.NewBuffer(netDeviceAsJson)

		req, err := http.NewRequest(http.MethodPut, "http://localhost/api/v1/vm.add-net", netDeviceAsIoReader)
		if err != nil {
			return err
		}

		req.Header.Set("Accept", "application/json")
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("Content-Length", strconv.Itoa(int(netDeviceAsIoReader.Len())))

		payload, err := httputil.DumpRequest(req, true)
		if err != nil {
			return err
		}

		files := clh.netDevicesFiles[*netDevice.Mac]
		var fds []int
		for _, f := range files {
			fds = append(fds, int(f.Fd()))
		}
		oob := syscall.UnixRights(fds...)
		payloadn, oobn, err := conn.WriteMsgUnix([]byte(payload), oob, nil)
		if err != nil {
			return err
		}
		if payloadn != len(payload) || oobn != len(oob) {
			return fmt.Errorf("Failed to send all the request to Cloud Hypervisor. %d bytes expect to send as payload, %d bytes expect to send as oob date,  but only %d sent as payload, and %d sent as oob", len(payload), len(oob), payloadn, oobn)
		}

		reader := bufio.NewReader(conn)
		resp, err := http.ReadResponse(reader, req)
		if err != nil {
			return err
		}

		respBody, err := io.ReadAll(resp.Body)
		if err != nil {
			return err
		}

		resp.Body.Close()
		resp.Body = io.NopCloser(bytes.NewBuffer(respBody))

		if resp.StatusCode != 200 && resp.StatusCode != 204 {
			clh.Logger().Errorf("vmAddNetPut failed with error '%d'. Response: %+v", resp.StatusCode, resp)
			return fmt.Errorf("Failed to add the network device '%+v' to Cloud Hypervisor: %v", netDevice, resp.StatusCode)
		}
	}

	return nil
}

// Cloud hypervisor state
type CloudHypervisorState struct {
	apiSocket         string
	PID               int
	VirtiofsDaemonPid int
	state             clhState
}

func (s *CloudHypervisorState) reset() {
	s.PID = 0
	s.VirtiofsDaemonPid = 0
	s.state = clhNotReady
}

type cloudHypervisor struct {
	console         console.Console
	virtiofsDaemon  VirtiofsDaemon
	APIClient       clhClient
	ctx             context.Context
	id              string
	netDevices      *[]chclient.NetConfig
	devicesIds      map[string]string
	netDevicesFiles map[string][]*os.File
	vmconfig        chclient.VmConfig
	state           CloudHypervisorState
	config          HypervisorConfig
	stopped         int32
	mu              sync.Mutex
}

var clhKernelParams = []Param{
	{"panic", "1"},         // upon kernel panic wait 1 second before reboot
	{"no_timer_check", ""}, // do not Check broken timer IRQ resources
	{"noreplace-smp", ""},  // do not replace SMP instructions
}

var clhDebugKernelParams = []Param{
	{"console", "ttyS0,115200n8"}, // enable serial console
}

var clhArmDebugKernelParams = []Param{
	{"console", "ttyAMA0,115200n8"}, // enable serial console
}

var clhDebugConfidentialGuestKernelParams = []Param{
	{"console", "hvc0"}, // enable HVC console
}

var clhDebugKernelParamsCommon = []Param{
	{"systemd.log_target", "console"}, // send loggng to the console
}

//###########################################################
//
// hypervisor interface implementation for cloud-hypervisor
//
//###########################################################

func (clh *cloudHypervisor) getClhAPITimeout() time.Duration {
	// Increase the APITimeout when dealing with a Confidential Guest.
	// The value has been chosen based on tests using `ctr`, and hopefully
	// this change can be dropped in further steps of the development.
	if clh.config.ConfidentialGuest {
		return clhAPITimeoutConfidentialGuest
	}

	return clhAPITimeout
}

func (clh *cloudHypervisor) getClhStopSandboxTimeout() time.Duration {
	// Increase the StopSandboxTimeout when dealing with a Confidential Guest.
	// The value has been chosen based on tests using `ctr`, and hopefully
	// this change can be dropped in further steps of the development.
	if clh.config.ConfidentialGuest {
		return clhStopSandboxTimeoutConfidentialGuest
	}

	return clhStopSandboxTimeout
}

func (clh *cloudHypervisor) setConfig(config *HypervisorConfig) error {
	clh.config = *config

	return nil
}

func (clh *cloudHypervisor) createVirtiofsDaemon(sharedPath string) (VirtiofsDaemon, error) {
	virtiofsdSocketPath, err := clh.virtioFsSocketPath(clh.id)
	if err != nil {
		return nil, err
	}

	if clh.config.SharedFS == config.VirtioFSNydus {
		apiSockPath, err := clh.nydusdAPISocketPath(clh.id)
		if err != nil {
			clh.Logger().WithError(err).Error("Invalid api socket path for nydusd")
			return nil, err
		}
		nd := &nydusd{
			path:        clh.config.VirtioFSDaemon,
			sockPath:    virtiofsdSocketPath,
			apiSockPath: apiSockPath,
			sourcePath:  sharedPath,
			debug:       clh.config.Debug,
			extraArgs:   clh.config.VirtioFSExtraArgs,
			startFn:     startInShimNS,
		}
		nd.setupShareDirFn = nd.setupPassthroughFS
		return nd, nil
	}

	// default: use virtiofsd
	return &virtiofsd{
		path:       clh.config.VirtioFSDaemon,
		sourcePath: sharedPath,
		socketPath: virtiofsdSocketPath,
		extraArgs:  clh.config.VirtioFSExtraArgs,
		cache:      clh.config.VirtioFSCache,
	}, nil
}

func (clh *cloudHypervisor) setupVirtiofsDaemon(ctx context.Context) error {
	if clh.config.SharedFS == config.NoSharedFS {
		return nil
	}

	if clh.config.SharedFS == config.Virtio9P {
		return errors.New("cloud-hypervisor only supports virtio based file sharing")
	}

	// virtioFS or virtioFsNydus
	clh.Logger().WithField("function", "setupVirtiofsDaemon").Info("Starting virtiofsDaemon")

	if clh.virtiofsDaemon == nil {
		return errors.New("Missing virtiofsDaemon configuration")
	}

	pid, err := clh.virtiofsDaemon.Start(ctx, func() {
		clh.StopVM(ctx, false)
	})
	if err != nil {
		return err
	}
	clh.state.VirtiofsDaemonPid = pid

	return nil
}

func (clh *cloudHypervisor) stopVirtiofsDaemon(ctx context.Context) (err error) {
	if clh.state.VirtiofsDaemonPid == 0 {
		clh.Logger().Warn("The virtiofsd had stopped")
		return nil
	}

	err = clh.virtiofsDaemon.Stop(ctx)
	if err != nil {
		return err
	}

	clh.state.VirtiofsDaemonPid = 0

	return nil
}

func (clh *cloudHypervisor) loadVirtiofsDaemon(sharedPath string) (VirtiofsDaemon, error) {
	virtiofsdSocketPath, err := clh.virtioFsSocketPath(clh.id)
	if err != nil {
		return nil, err
	}

	return &virtiofsd{
		PID:        clh.state.VirtiofsDaemonPid,
		sourcePath: sharedPath,
		socketPath: virtiofsdSocketPath,
	}, nil
}

func (clh *cloudHypervisor) nydusdAPISocketPath(id string) (string, error) {
	return utils.BuildSocketPath(clh.config.VMStorePath, id, nydusdAPISock)
}

func (clh *cloudHypervisor) enableProtection() error {
	protection, err := availableGuestProtection()
	if err != nil {
		return err
	}

	switch protection {
	case tdxProtection:
		firmwarePath, err := clh.config.FirmwareAssetPath()
		if err != nil {
			return err
		}

		if firmwarePath == "" {
			return errors.New("Firmware path is not specified")
		}

		clh.vmconfig.Payload.SetFirmware(firmwarePath)

		if clh.vmconfig.Platform == nil {
			clh.vmconfig.Platform = chclient.NewPlatformConfig()
		}
		clh.vmconfig.Platform.SetTdx(true)

		return nil

	case sevProtection:
		return errors.New("SEV protection is not supported by Cloud Hypervisor")
	case snpProtection:
		return errors.New("SEV-SNP protection is not supported by Cloud Hypervisor")

	default:
		return errors.New("This system doesn't support Confidential Computing (Guest Protection)")
	}
}

// For cloudHypervisor this call only sets the internal structure up.
// The VM will be created and started through StartVM().
func (clh *cloudHypervisor) CreateVM(ctx context.Context, id string, network Network, hypervisorConfig *HypervisorConfig) error {
	clh.ctx = ctx

	span, newCtx := katatrace.Trace(clh.ctx, clh.Logger(), "CreateVM", clhTracingTags, map[string]string{"sandbox_id": clh.id})
	clh.ctx = newCtx
	defer span.End()

	if err := clh.setConfig(hypervisorConfig); err != nil {
		return err
	}

	clh.id = id
	clh.state.state = clhNotReady
	clh.devicesIds = make(map[string]string)
	clh.netDevicesFiles = make(map[string][]*os.File)

	clh.Logger().WithField("function", "CreateVM").Info("creating Sandbox")

	if clh.state.PID > 0 {
		clh.Logger().WithField("function", "CreateVM").Info("Sandbox already exist, loading from state")

		virtiofsDaemon, err := clh.loadVirtiofsDaemon(hypervisorConfig.SharedFS)
		if err != nil {
			return err
		}
		clh.virtiofsDaemon = virtiofsDaemon

		return nil
	}

	// No need to return an error from there since there might be nothing
	// to fetch if this is the first time the hypervisor is created.
	clh.Logger().WithField("function", "CreateVM").Info("Sandbox not found creating")

	// Create the VM config via the constructor to ensure default values are properly assigned
	clh.vmconfig = *chclient.NewVmConfig(*chclient.NewPayloadConfig())

	// Make sure the kernel path is valid
	kernelPath, err := clh.config.KernelAssetPath()
	if err != nil {
		return err
	}
	clh.vmconfig.Payload.SetKernel(kernelPath)

	clh.vmconfig.Platform = chclient.NewPlatformConfig()
	platform := clh.vmconfig.Platform
	platform.SetNumPciSegments(2)
	if clh.config.IOMMU {
		platform.SetIommuSegments([]int32{0})
	}

	if clh.config.ConfidentialGuest {
		if err := clh.enableProtection(); err != nil {
			return err
		}
	}

	// Create the VM memory config via the constructor to ensure default values are properly assigned
	clh.vmconfig.Memory = chclient.NewMemoryConfig(int64((utils.MemUnit(clh.config.MemorySize) * utils.MiB).ToBytes()))
	// Memory config shared is to be enabled when using vhost_user backends, ex. virtio-fs
	// or when using HugePages.
	// If such features are disabled, turn off shared memory config.
	if clh.config.SharedFS == config.NoSharedFS && !clh.config.HugePages {
		clh.vmconfig.Memory.Shared = func(b bool) *bool { return &b }(false)
	} else {
		clh.vmconfig.Memory.Shared = func(b bool) *bool { return &b }(true)
	}
	// Enable hugepages if needed
	clh.vmconfig.Memory.Hugepages = func(b bool) *bool { return &b }(clh.config.HugePages)
	if !clh.config.ConfidentialGuest {
		hotplugSize := clh.config.DefaultMaxMemorySize
		// OpenAPI only supports int64 values
		clh.vmconfig.Memory.HotplugSize = func(i int64) *int64 { return &i }(int64((utils.MemUnit(hotplugSize) * utils.MiB).ToBytes()))
	}
	// Set initial amount of cpu's for the virtual machine
	clh.vmconfig.Cpus = chclient.NewCpusConfig(int32(clh.config.NumVCPUs()), int32(clh.config.DefaultMaxVCPUs))

	params, err := GetKernelRootParams(hypervisorConfig.RootfsType, clh.config.ConfidentialGuest, !clh.config.ConfidentialGuest)
	if err != nil {
		return err
	}
	params = append(params, clhKernelParams...)

	// Followed by extra debug parameters if debug enabled in configuration file
	if clh.config.Debug {
		if clh.config.ConfidentialGuest {
			params = append(params, clhDebugConfidentialGuestKernelParams...)
		} else if runtime.GOARCH == "arm64" {
			params = append(params, clhArmDebugKernelParams...)
		} else {
			params = append(params, clhDebugKernelParams...)
		}
		params = append(params, clhDebugKernelParamsCommon...)
	} else {
		// start the guest kernel with 'quiet' in non-debug mode
		params = append(params, Param{"quiet", ""})
	}
	if clh.config.IOMMU {
		params = append(params, Param{"iommu", "pt"})
	}

	// Followed by extra kernel parameters defined in the configuration file
	params = append(params, clh.config.KernelParams...)

	clh.vmconfig.Payload.SetCmdline(kernelParamsToString(params))

	// set random device generator to hypervisor
	clh.vmconfig.Rng = chclient.NewRngConfig(clh.config.EntropySource)
	clh.vmconfig.Rng.SetIommu(clh.config.IOMMU)

	// set the initial root/boot disk of hypervisor
	assetPath, assetType, err := clh.config.ImageOrInitrdAssetPath()
	if err != nil {
		return err
	}

	if assetType == types.ImageAsset {
		if clh.config.ConfidentialGuest {
			disk := chclient.NewDiskConfig(assetPath)
			disk.SetReadonly(true)

			diskRateLimiterConfig := clh.getDiskRateLimiterConfig()
			if diskRateLimiterConfig != nil {
				disk.SetRateLimiterConfig(*diskRateLimiterConfig)
			}

			if clh.vmconfig.Disks != nil {
				*clh.vmconfig.Disks = append(*clh.vmconfig.Disks, *disk)
			} else {
				clh.vmconfig.Disks = &[]chclient.DiskConfig{*disk}
			}
		} else {
			pmem := chclient.NewPmemConfig(assetPath)
			*pmem.DiscardWrites = true
			pmem.SetIommu(clh.config.IOMMU)

			if clh.vmconfig.Pmem != nil {
				*clh.vmconfig.Pmem = append(*clh.vmconfig.Pmem, *pmem)
			} else {
				clh.vmconfig.Pmem = &[]chclient.PmemConfig{*pmem}
			}
		}
	} else {
		// assetType == types.InitrdAsset
		clh.vmconfig.Payload.SetInitramfs(assetPath)
	}

	if clh.config.ConfidentialGuest {
		// Use HVC as the guest console only in debug mode, only
		// for Confidential Guests
		if clh.config.Debug {
			clh.vmconfig.Console = chclient.NewConsoleConfig(cctTTY)
		} else {
			clh.vmconfig.Console = chclient.NewConsoleConfig(cctOFF)
		}

		clh.vmconfig.Serial = chclient.NewConsoleConfig(cctOFF)
	} else {
		// Use serial port as the guest console only in debug mode,
		// so that we can gather early OS booting log
		if clh.config.Debug {
			clh.vmconfig.Serial = chclient.NewConsoleConfig(cctTTY)
		} else {
			clh.vmconfig.Serial = chclient.NewConsoleConfig(cctOFF)
		}

		clh.vmconfig.Console = chclient.NewConsoleConfig(cctOFF)
	}
	clh.vmconfig.Console.SetIommu(clh.config.IOMMU)

	cpu_topology := chclient.NewCpuTopology()
	cpu_topology.ThreadsPerCore = func(i int32) *int32 { return &i }(1)
	cpu_topology.CoresPerDie = func(i int32) *int32 { return &i }(int32(clh.config.DefaultMaxVCPUs))
	cpu_topology.DiesPerPackage = func(i int32) *int32 { return &i }(1)
	cpu_topology.Packages = func(i int32) *int32 { return &i }(1)
	clh.vmconfig.Cpus.Topology = cpu_topology

	// Overwrite the default value of HTTP API socket path for cloud hypervisor
	apiSocketPath, err := clh.apiSocketPath(id)
	if err != nil {
		clh.Logger().WithError(err).Info("Invalid api socket path for cloud-hypervisor")
		return err
	}
	clh.state.apiSocket = apiSocketPath

	cfg := chclient.NewConfiguration()
	cfg.HTTPClient = &http.Client{
		Transport: &http.Transport{
			DialContext: func(ctx context.Context, network, path string) (net.Conn, error) {
				addr, err := net.ResolveUnixAddr("unix", clh.state.apiSocket)
				if err != nil {
					return nil, err
				}

				return net.DialUnix("unix", nil, addr)
			},
		},
	}

	clh.APIClient = &clhClientApi{
		ApiInternal: chclient.NewAPIClient(cfg).DefaultApi,
	}

	clh.virtiofsDaemon, err = clh.createVirtiofsDaemon(filepath.Join(GetSharePath(clh.id)))
	if err != nil {
		return err
	}

	if clh.config.SGXEPCSize > 0 {
		epcSection := chclient.NewSgxEpcConfig("kata-epc", clh.config.SGXEPCSize)
		epcSection.Prefault = func(b bool) *bool { return &b }(true)

		if clh.vmconfig.SgxEpc != nil {
			*clh.vmconfig.SgxEpc = append(*clh.vmconfig.SgxEpc, *epcSection)
		} else {
			clh.vmconfig.SgxEpc = &[]chclient.SgxEpcConfig{*epcSection}
		}

	}

	return nil
}

// StartVM will start the VMM and boot the virtual machine for the given sandbox.
func (clh *cloudHypervisor) StartVM(ctx context.Context, timeout int) error {
	span, ctx := katatrace.Trace(ctx, clh.Logger(), "StartVM", clhTracingTags, map[string]string{"sandbox_id": clh.id})
	defer span.End()

	clh.Logger().WithField("function", "StartVM").Info("starting Sandbox")

	vmPath := filepath.Join(clh.config.VMStorePath, clh.id)
	err := utils.MkdirAllWithInheritedOwner(vmPath, DirMode)
	if err != nil {
		return err
	}

	// This needs to be done as late as possible, just before launching
	// virtiofsd are executed by kata-runtime after this call, run with
	// the SELinux label. If these processes require privileged, we do
	// notwant to run them under confinement.
	if !clh.config.DisableSeLinux {

		if err := label.SetProcessLabel(clh.config.SELinuxProcessLabel); err != nil {
			return err
		}
		defer label.SetProcessLabel("")
	}

	err = clh.setupVirtiofsDaemon(ctx)
	if err != nil {
		return err
	}
	defer func() {
		if err != nil {
			if shutdownErr := clh.stopVirtiofsDaemon(ctx); shutdownErr != nil {
				clh.Logger().WithError(shutdownErr).Warn("error shutting down VirtiofsDaemon")
			}
		}
	}()

	err = clh.launchClh()
	if err != nil {
		return fmt.Errorf("failed to launch cloud-hypervisor: %q", err)
	}

	bootTimeout := clh.getClhAPITimeout()
	if bootTimeout < clhCreateAndBootVMMinimumTimeout {
		bootTimeout = clhCreateAndBootVMMinimumTimeout
	}
	ctx, cancel := context.WithTimeout(ctx, bootTimeout*time.Second)
	defer cancel()

	if err := clh.bootVM(ctx); err != nil {
		return err
	}

	clh.state.state = clhReady
	return nil
}

// GetVMConsole builds the path of the console where we can read logs coming
// from the sandbox.
func (clh *cloudHypervisor) GetVMConsole(ctx context.Context, id string) (string, string, error) {
	clh.Logger().WithField("function", "GetVMConsole").WithField("id", id).Info("Get Sandbox Console")
	master, slave, err := console.NewPty()
	if err != nil {
		clh.Logger().WithError(err).Error("Error create pseudo tty")
		return consoleProtoPty, "", err
	}
	clh.console = master

	return consoleProtoPty, slave, nil
}

func (clh *cloudHypervisor) Disconnect(ctx context.Context) {
	clh.Logger().WithField("function", "Disconnect").Info("Disconnecting Sandbox Console")
}

func (clh *cloudHypervisor) GetThreadIDs(ctx context.Context) (VcpuThreadIDs, error) {

	clh.Logger().WithField("function", "GetThreadIDs").Info("get thread ID's")

	var vcpuInfo VcpuThreadIDs

	vcpuInfo.vcpus = make(map[int]int)

	getVcpus := func(pid int) (map[int]int, error) {
		vcpus := make(map[int]int)

		dir := fmt.Sprintf("/proc/%d/task", pid)
		files, err := os.ReadDir(dir)
		if err != nil {
			return vcpus, err
		}

		pattern, err := regexp.Compile(`^vcpu\d+$`)
		if err != nil {
			return vcpus, err
		}
		for _, file := range files {
			comm, err := os.ReadFile(fmt.Sprintf("%s/%s/comm", dir, file.Name()))
			if err != nil {
				return vcpus, err
			}
			pName := strings.TrimSpace(string(comm))
			if !pattern.MatchString(pName) {
				continue
			}

			cpuID := strings.TrimPrefix(pName, "vcpu")
			threadID := file.Name()

			k, err := strconv.Atoi(cpuID)
			if err != nil {
				return vcpus, err
			}
			v, err := strconv.Atoi(threadID)
			if err != nil {
				return vcpus, err
			}
			vcpus[k] = v
		}
		return vcpus, nil
	}

	if clh.state.PID == 0 {
		return vcpuInfo, nil
	}

	vcpus, err := getVcpus(clh.state.PID)
	if err != nil {
		return vcpuInfo, err
	}
	vcpuInfo.vcpus = vcpus

	return vcpuInfo, nil
}

func clhDriveIndexToID(i int) string {
	return "clh_drive_" + strconv.Itoa(i)
}

// Various cloud-hypervisor APIs report a PCI address in "BB:DD.F"
// form within the PciDeviceInfo struct.  This is a broken API,
// because there's no way clh can reliably know the guest side bdf for
// a device, since the bus number depends on how the guest firmware
// and/or kernel enumerates it.  They get away with it only because
// they don't use bridges, and so the bus is always 0.  Under that
// assumption convert a clh PciDeviceInfo into a PCI path
func clhPciInfoToPath(pciInfo chclient.PciDeviceInfo) (types.PciPath, error) {
	tokens := strings.Split(pciInfo.Bdf, ":")
	if len(tokens) != 3 || tokens[0] != "0000" || tokens[1] != "00" {
		return types.PciPath{}, fmt.Errorf("Unexpected PCI address %q from clh hotplug", pciInfo.Bdf)
	}

	tokens = strings.Split(tokens[2], ".")
	if len(tokens) != 2 || tokens[1] != "0" || len(tokens[0]) != 2 {
		return types.PciPath{}, fmt.Errorf("Unexpected PCI address %q from clh hotplug", pciInfo.Bdf)
	}

	return types.PciPathFromString(tokens[0])
}

func (clh *cloudHypervisor) hotplugAddBlockDevice(drive *config.BlockDrive) error {
	if drive.Swap {
		return fmt.Errorf("cloudHypervisor doesn't support swap")
	}

	if clh.config.BlockDeviceDriver != config.VirtioBlock {
		return fmt.Errorf("incorrect hypervisor configuration on 'block_device_driver':"+
			" using '%v' but only support '%v'", clh.config.BlockDeviceDriver, config.VirtioBlock)
	}

	var err error

	cl := clh.client()
	ctx, cancel := context.WithTimeout(context.Background(), clhHotPlugAPITimeout*time.Second)
	defer cancel()

	driveID := clhDriveIndexToID(drive.Index)

	if drive.Pmem {
		return fmt.Errorf("pmem device hotplug not supported")
	}

	// Create the clh disk config via the constructor to ensure default values are properly assigned
	clhDisk := *chclient.NewDiskConfig(drive.File)
	clhDisk.Readonly = &drive.ReadOnly
	clhDisk.VhostUser = func(b bool) *bool { return &b }(false)
	if clh.config.BlockDeviceCacheSet {
		clhDisk.Direct = &clh.config.BlockDeviceCacheDirect
	}

	queues := int32(clh.config.NumVCPUs())
	queueSize := int32(1024)
	clhDisk.NumQueues = &queues
	clhDisk.QueueSize = &queueSize
	clhDisk.SetIommu(clh.config.IOMMU)

	diskRateLimiterConfig := clh.getDiskRateLimiterConfig()
	if diskRateLimiterConfig != nil {
		clhDisk.SetRateLimiterConfig(*diskRateLimiterConfig)
	}

	pciInfo, _, err := cl.VmAddDiskPut(ctx, clhDisk)

	if err != nil {
		return fmt.Errorf("failed to hotplug block device %+v %s", drive, openAPIClientError(err))
	}

	clh.devicesIds[driveID] = pciInfo.GetId()
	drive.PCIPath, err = clhPciInfoToPath(pciInfo)

	return err
}

func (clh *cloudHypervisor) hotPlugVFIODevice(device *config.VFIODev) error {
	cl := clh.client()
	ctx, cancel := context.WithTimeout(context.Background(), clhHotPlugAPITimeout*time.Second)
	defer cancel()

	// Create the clh device config via the constructor to ensure default values are properly assigned
	clhDevice := *chclient.NewDeviceConfig(device.SysfsDev)
	clhDevice.SetIommu(clh.config.IOMMU)
	pciInfo, _, err := cl.VmAddDevicePut(ctx, clhDevice)
	if err != nil {
		return fmt.Errorf("Failed to hotplug device %+v %s", device, openAPIClientError(err))
	}
	clh.devicesIds[device.ID] = pciInfo.GetId()

	// clh doesn't use bridges, so the PCI path is simply the slot
	// number of the device.  This will break if clh starts using
	// bridges (including PCI-E root ports), but so will the clh
	// API, since there's no way it can reliably predict a guest
	// Bdf when bridges are present.
	tokens := strings.Split(pciInfo.Bdf, ":")
	if len(tokens) != 3 || tokens[0] != "0000" || tokens[1] != "00" {
		return fmt.Errorf("Unexpected PCI address %q from clh hotplug", pciInfo.Bdf)
	}

	tokens = strings.Split(tokens[2], ".")
	if len(tokens) != 2 || tokens[1] != "0" || len(tokens[0]) != 2 {
		return fmt.Errorf("Unexpected PCI address %q from clh hotplug", pciInfo.Bdf)
	}

	if device.Type == config.VFIOAPDeviceMediatedType {
		return fmt.Errorf("VFIO device %+v is not PCI, only PCI is supported in Cloud Hypervisor", device)
	}

	device.GuestPciPath, err = types.PciPathFromString(tokens[0])

	return err
}

func (clh *cloudHypervisor) hotplugAddNetDevice(e Endpoint) error {
	err := clh.addNet(e)
	if err != nil {
		return err
	}

	return clh.vmAddNetPut()
}

func (clh *cloudHypervisor) HotplugAddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	span, _ := katatrace.Trace(ctx, clh.Logger(), "HotplugAddDevice", clhTracingTags, map[string]string{"sandbox_id": clh.id})
	defer span.End()

	switch devType {
	case BlockDev:
		drive := devInfo.(*config.BlockDrive)
		return nil, clh.hotplugAddBlockDevice(drive)
	case VfioDev:
		device := devInfo.(*config.VFIODev)
		return nil, clh.hotPlugVFIODevice(device)
	case NetDev:
		device := devInfo.(Endpoint)
		return nil, clh.hotplugAddNetDevice(device)
	default:
		return nil, fmt.Errorf("cannot hotplug device: unsupported device type '%v'", devType)
	}

}

func (clh *cloudHypervisor) HotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	span, _ := katatrace.Trace(ctx, clh.Logger(), "HotplugRemoveDevice", clhTracingTags, map[string]string{"sandbox_id": clh.id})
	defer span.End()

	var deviceID string

	switch devType {
	case BlockDev:
		deviceID = clhDriveIndexToID(devInfo.(*config.BlockDrive).Index)
	case VfioDev:
		deviceID = devInfo.(*config.VFIODev).ID
	default:
		clh.Logger().WithFields(log.Fields{"devInfo": devInfo,
			"deviceType": devType}).Error("HotplugRemoveDevice: unsupported device")
		return nil, fmt.Errorf("Could not hot remove device: unsupported device: %v, type: %v",
			devInfo, devType)
	}

	cl := clh.client()
	ctx, cancel := context.WithTimeout(context.Background(), clhHotPlugAPITimeout*time.Second)
	defer cancel()

	originalDeviceID := clh.devicesIds[deviceID]
	remove := *chclient.NewVmRemoveDevice()
	remove.Id = &originalDeviceID
	_, err := cl.VmRemoveDevicePut(ctx, remove)
	if err != nil {
		err = fmt.Errorf("failed to hotplug remove (unplug) device %+v: %s", devInfo, openAPIClientError(err))
	}

	delete(clh.devicesIds, deviceID)
	return nil, err
}

func (clh *cloudHypervisor) HypervisorConfig() HypervisorConfig {
	return clh.config
}

func (clh *cloudHypervisor) ResizeMemory(ctx context.Context, reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, MemoryDevice, error) {

	// TODO: Add support for virtio-mem

	if probe {
		return 0, MemoryDevice{}, errors.New("probe memory is not supported for cloud-hypervisor")
	}

	if reqMemMB == 0 {
		// This is a corner case if requested to resize to 0 means something went really wrong.
		return 0, MemoryDevice{}, errors.New("Can not resize memory to 0")
	}

	info, err := clh.vmInfo()
	if err != nil {
		return 0, MemoryDevice{}, err
	}

	// HotplugSize can be nil in cases where Hotplug is not supported, as Cloud Hypervisor API
	// does *not* allow us to set 0 as the HotplugSize.
	maxHotplugSize := 0 * utils.Byte
	if info.Config.Memory.HotplugSize != nil {
		maxHotplugSize = utils.MemUnit(*info.Config.Memory.HotplugSize) * utils.Byte
	}

	if reqMemMB > uint32(maxHotplugSize.ToMiB()) {
		reqMemMB = uint32(maxHotplugSize.ToMiB())
	}

	currentMem := utils.MemUnit(info.Config.Memory.Size) * utils.Byte
	newMem := utils.MemUnit(reqMemMB) * utils.MiB

	// Early Check to verify if boot memory is the same as requested
	if currentMem == newMem {
		clh.Logger().WithField("memory", reqMemMB).Debugf("VM already has requested memory")
		return uint32(currentMem.ToMiB()), MemoryDevice{}, nil
	}

	if currentMem > newMem {
		clh.Logger().Warn("Remove memory is not supported, nothing to do")
		return uint32(currentMem.ToMiB()), MemoryDevice{}, nil
	}

	blockSize := utils.MemUnit(memoryBlockSizeMB) * utils.MiB
	hotplugSize := (newMem - currentMem).AlignMem(blockSize)

	// Update memory request to increase memory aligned block
	alignedRequest := currentMem + hotplugSize
	if newMem != alignedRequest {
		clh.Logger().WithFields(log.Fields{"request": newMem, "aligned-request": alignedRequest}).Debug("aligning VM memory request")
		newMem = alignedRequest
	}

	// Post-alignment checks
	if currentMem == newMem {
		clh.Logger().WithFields(log.Fields{"current-memory": currentMem, "new-memory": newMem}).Debug("VM already has requested memory(after alignment)")
		return uint32(currentMem.ToMiB()), MemoryDevice{}, nil
	}
	// Check for aligned memory exceeding max hotplug size
	if newMem > (utils.MemUnit(uint32(maxHotplugSize.ToMiB())) * utils.MiB) {
		newMem = utils.MemUnit(uint32(maxHotplugSize.ToMiB())) * utils.MiB
	}

	cl := clh.client()
	ctx, cancelResize := context.WithTimeout(ctx, clh.getClhAPITimeout()*time.Second)
	defer cancelResize()

	resize := *chclient.NewVmResize()
	// OpenApi does not support uint64, convert to int64
	resize.DesiredRam = func(i int64) *int64 { return &i }(int64(newMem.ToBytes()))
	clh.Logger().WithFields(log.Fields{"current-memory": currentMem, "new-memory": newMem}).Debug("updating VM memory")
	if _, err = cl.VmResizePut(ctx, resize); err != nil {
		clh.Logger().WithError(err).WithFields(log.Fields{"current-memory": currentMem, "new-memory": newMem}).Warnf("failed to update memory %s", openAPIClientError(err))
		err = fmt.Errorf("Failed to resize memory from %d to %d: %s", currentMem, newMem, openAPIClientError(err))
		return uint32(currentMem.ToMiB()), MemoryDevice{}, openAPIClientError(err)
	}

	return uint32(newMem.ToMiB()), MemoryDevice{SizeMB: int(hotplugSize.ToMiB())}, nil
}

func (clh *cloudHypervisor) ResizeVCPUs(ctx context.Context, reqVCPUs uint32) (currentVCPUs uint32, newVCPUs uint32, err error) {
	cl := clh.client()

	// Retrieve the number of current vCPUs via HTTP API
	info, err := clh.vmInfo()
	if err != nil {
		clh.Logger().WithField("function", "ResizeVCPUs").WithError(err).Info("[clh] vmInfo failed")
		return 0, 0, openAPIClientError(err)
	}

	currentVCPUs = uint32(info.Config.Cpus.BootVcpus)
	newVCPUs = currentVCPUs

	// Sanity Check
	if reqVCPUs == 0 {
		clh.Logger().WithField("function", "ResizeVCPUs").Debugf("Cannot resize vCPU to 0")
		return currentVCPUs, newVCPUs, fmt.Errorf("Cannot resize vCPU to 0")
	}
	if reqVCPUs > uint32(info.Config.Cpus.MaxVcpus) {
		clh.Logger().WithFields(log.Fields{
			"function":    "ResizeVCPUs",
			"reqVCPUs":    reqVCPUs,
			"clhMaxVCPUs": info.Config.Cpus.MaxVcpus,
		}).Warn("exceeding the 'clhMaxVCPUs' (resizing to 'clhMaxVCPUs')")

		reqVCPUs = uint32(info.Config.Cpus.MaxVcpus)
	}

	// Resize (hot-plug) vCPUs via HTTP API
	ctx, cancel := context.WithTimeout(ctx, clh.getClhAPITimeout()*time.Second)
	defer cancel()
	resize := *chclient.NewVmResize()
	resize.DesiredVcpus = func(i int32) *int32 { return &i }(int32(reqVCPUs))
	if _, err = cl.VmResizePut(ctx, resize); err != nil {
		return currentVCPUs, newVCPUs, errors.Wrap(err, "[clh] VmResizePut failed")
	}

	newVCPUs = reqVCPUs

	return currentVCPUs, newVCPUs, nil
}

func (clh *cloudHypervisor) Cleanup(ctx context.Context) error {
	clh.Logger().WithField("function", "Cleanup").Info("Cleanup")
	return nil
}

func (clh *cloudHypervisor) PauseVM(ctx context.Context) error {
	clh.Logger().WithField("function", "PauseVM").Info("Pause Sandbox")
	return nil
}

func (clh *cloudHypervisor) SaveVM() error {
	clh.Logger().WithField("function", "saveSandboxC").Info("Save Sandbox")
	return nil
}

func (clh *cloudHypervisor) ResumeVM(ctx context.Context) error {
	clh.Logger().WithField("function", "ResumeVM").Info("Resume Sandbox")
	return nil
}

// StopVM will stop the Sandbox's VM.
func (clh *cloudHypervisor) StopVM(ctx context.Context, waitOnly bool) (err error) {
	clh.mu.Lock()
	defer func() {
		if err == nil {
			atomic.StoreInt32(&clh.stopped, 1)
		}
		clh.mu.Unlock()
	}()
	span, _ := katatrace.Trace(ctx, clh.Logger(), "StopVM", clhTracingTags, map[string]string{"sandbox_id": clh.id})
	defer span.End()
	clh.Logger().WithField("function", "StopVM").Info("Stop Sandbox")
	if atomic.LoadInt32(&clh.stopped) != 0 {
		clh.Logger().Info("Already stopped")
		return nil
	}

	return clh.terminate(ctx, waitOnly)
}

func (clh *cloudHypervisor) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	return errors.New("cloudHypervisor is not supported by VM cache")
}

func (clh *cloudHypervisor) toGrpc(ctx context.Context) ([]byte, error) {
	return nil, errors.New("cloudHypervisor is not supported by VM cache")
}

func (clh *cloudHypervisor) Save() (s hv.HypervisorState) {
	s.Pid = clh.state.PID
	s.Type = string(ClhHypervisor)
	s.VirtiofsDaemonPid = clh.state.VirtiofsDaemonPid
	s.APISocket = clh.state.apiSocket
	return
}

func (clh *cloudHypervisor) Load(s hv.HypervisorState) {
	clh.state.PID = s.Pid
	clh.state.VirtiofsDaemonPid = s.VirtiofsDaemonPid
	clh.state.apiSocket = s.APISocket
}

// Check is the implementation of Check from the Hypervisor interface.
// Check if the VMM API is working.

func (clh *cloudHypervisor) Check() error {
	// Use a long timeout to check if the VMM is running:
	// Check is used by the monitor thread(a background thread). If the
	// monitor thread calls Check() during the Container boot, it will take
	// longer than usual specially if there is a hot-plug request in progress.
	running, err := clh.isClhRunning(10)
	if !running {
		return fmt.Errorf("clh is not running: %s", err)
	}
	return err
}

func (clh *cloudHypervisor) GetPids() []int {
	return []int{clh.state.PID}
}

func (clh *cloudHypervisor) GetVirtioFsPid() *int {
	return &clh.state.VirtiofsDaemonPid
}

func (clh *cloudHypervisor) AddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) error {
	span, _ := katatrace.Trace(ctx, clh.Logger(), "AddDevice", clhTracingTags, map[string]string{"sandbox_id": clh.id})
	defer span.End()

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
		clh.Logger().WithField("function", "AddDevice").Warnf("Add device of type %v is not supported.", v)
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
	return hvLogger.WithField("subsystem", "cloudHypervisor")
}

// Adds all capabilities supported by cloudHypervisor implementation of hypervisor interface
func (clh *cloudHypervisor) Capabilities(ctx context.Context) types.Capabilities {
	span, _ := katatrace.Trace(ctx, clh.Logger(), "Capabilities", clhTracingTags, map[string]string{"sandbox_id": clh.id})
	defer span.End()

	clh.Logger().WithField("function", "Capabilities").Info("get Capabilities")
	var caps types.Capabilities
	if clh.config.SharedFS != config.NoSharedFS {
		caps.SetFsSharingSupport()
	}
	caps.SetBlockDeviceHotplugSupport()
	caps.SetNetworkDeviceHotplugSupported()
	return caps
}

func (clh *cloudHypervisor) terminate(ctx context.Context, waitOnly bool) (err error) {
	span, _ := katatrace.Trace(ctx, clh.Logger(), "terminate", clhTracingTags, map[string]string{"sandbox_id": clh.id})
	defer span.End()

	pid := clh.state.PID
	pidRunning := true
	if pid == 0 {
		pidRunning = false
	}

	defer func() {
		clh.Logger().Debug("Cleanup VM")
		if err1 := clh.cleanupVM(true); err1 != nil {
			clh.Logger().WithError(err1).Error("failed to cleanupVM")
		}
	}()

	clh.Logger().Debug("Stopping Cloud Hypervisor")

	if pidRunning && !waitOnly {
		clhRunning, _ := clh.isClhRunning(uint(clh.getClhStopSandboxTimeout()))
		if clhRunning {
			ctx, cancel := context.WithTimeout(context.Background(), clh.getClhStopSandboxTimeout()*time.Second)
			defer cancel()
			if _, err = clh.client().ShutdownVMM(ctx); err != nil {
				return err
			}
		}
	}

	if err = utils.WaitLocalProcess(pid, uint(clh.getClhStopSandboxTimeout()), syscall.Signal(0), clh.Logger()); err != nil {
		return err
	}

	clh.Logger().Debug("stop virtiofsDaemon")

	if err = clh.stopVirtiofsDaemon(ctx); err != nil {
		clh.Logger().WithError(err).Error("failed to stop virtiofsDaemon")
	}

	return
}

func (clh *cloudHypervisor) reset() {
	clh.state.reset()
}

func (clh *cloudHypervisor) GenerateSocket(id string) (interface{}, error) {
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
	return utils.BuildSocketPath(clh.config.VMStorePath, id, virtioFsSocket)
}

func (clh *cloudHypervisor) vsockSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(clh.config.VMStorePath, id, clhSocket)
}

func (clh *cloudHypervisor) apiSocketPath(id string) (string, error) {
	return utils.BuildSocketPath(clh.config.VMStorePath, id, clhAPISocket)
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

	return p, err
}

func (clh *cloudHypervisor) launchClh() error {

	clh.state.PID = -1

	clhPath, err := clh.clhPath()
	if err != nil {
		return err
	}

	args := []string{cscAPIsocket, clh.state.apiSocket}
	if clh.config.Debug && clh.config.HypervisorLoglevel > 0 {
		// Cloud hypervisor log levels
		// 'v' occurrences increase the level
		//0 =>  Warn
		//1 =>  Info
		//2 =>  Debug
		//3+ => Trace
		// Use Info, the CI runs with debug enabled
		// a high level of logging increases the boot time
		// and in a nested environment this could increase
		// the chances to fail because agent is not
		// ready on time.
		//
		// Note that for debugging CLH boot failures, the Info level
		// should be sufficient: Debug level generates so many
		// messages it floods the output stream to the extent that it
		// is almost impossible to view the guest kernel and userland
		// output. For further details, see the discussion on:
		//
		//   https://github.com/kata-containers/kata-containers/pull/2751
		verbosityString := fmt.Sprintf("-%s", strings.Repeat("v", int(clh.config.HypervisorLoglevel)))
		args = append(args, verbosityString)
	}

	// Enable the `seccomp` feature from Cloud Hypervisor by default
	// Disable it only when requested by users for debugging purposes
	if clh.config.DisableSeccomp {
		args = append(args, "--seccomp", "false")
	}

	clh.Logger().WithField("path", clhPath).Info()
	clh.Logger().WithField("args", strings.Join(args, " ")).Info()

	cmdHypervisor := exec.Command(clhPath, args...)
	if clh.config.Debug {
		cmdHypervisor.Env = os.Environ()
		cmdHypervisor.Env = append(cmdHypervisor.Env, "RUST_BACKTRACE=full")
		if clh.console != nil {
			cmdHypervisor.Stderr = clh.console
			cmdHypervisor.Stdout = clh.console
		}
	}
	cmdHypervisor.Stderr = cmdHypervisor.Stdout

	attr := syscall.SysProcAttr{}
	attr.Credential = &syscall.Credential{
		Uid:    clh.config.Uid,
		Gid:    clh.config.Gid,
		Groups: clh.config.Groups,
	}
	cmdHypervisor.SysProcAttr = &attr

	err = utils.StartCmd(cmdHypervisor)
	if err != nil {
		return err
	}

	clh.state.PID = cmdHypervisor.Process.Pid

	if err := clh.waitVMM(clhTimeout); err != nil {
		clh.Logger().WithError(err).Warn("cloud-hypervisor init failed")
		return err
	}

	return nil
}

//###########################################################################
//
// Cloud-hypervisor CLI builder
//
//###########################################################################

const (
	cctOFF string = "Off"
	cctTTY string = "Tty"
)

const (
	cscAPIsocket string = "--api-socket"
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

// ****************************************
// API calls
// ****************************************
func (clh *cloudHypervisor) isClhRunning(timeout uint) (bool, error) {

	pid := clh.state.PID

	if atomic.LoadInt32(&clh.stopped) != 0 {
		return false, nil
	}

	timeStart := time.Now()
	cl := clh.client()
	for {
		waitedPid, err := syscall.Wait4(pid, nil, syscall.WNOHANG, nil)
		if waitedPid == pid && err == nil {
			return false, nil
		}

		err = syscall.Kill(pid, syscall.Signal(0))
		if err != nil {
			return false, nil
		}
		ctx, cancel := context.WithTimeout(context.Background(), clh.getClhAPITimeout()*time.Second)
		_, _, err = cl.VmmPingGet(ctx)
		cancel()
		if err == nil {
			return true, nil
		} else {
			clh.Logger().WithError(err).Warning("clh.VmmPingGet API call failed")
		}

		if time.Since(timeStart).Seconds() > float64(timeout) {
			return false, fmt.Errorf("Failed to connect to API (timeout %ds): %s", timeout, openAPIClientError(err))
		}

		time.Sleep(time.Duration(10) * time.Millisecond)
	}

}

func (clh *cloudHypervisor) client() clhClient {
	return clh.APIClient
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

func (clh *cloudHypervisor) vmAddNetPut() error {
	return vmAddNetPutRequest(clh)
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

	info, err := clh.vmInfo()
	if err != nil {
		return err
	}

	clh.Logger().Debugf("VM state after create: %#v", info)

	if info.State != clhStateCreated {
		return fmt.Errorf("VM state is not 'Created' after 'CreateVM'")
	}

	err = clh.vmAddNetPut()
	if err != nil {
		return err
	}

	clh.Logger().Debug("Booting VM")
	_, err = cl.BootVM(ctx)
	if err != nil {
		return openAPIClientError(err)
	}

	info, err = clh.vmInfo()
	if err != nil {
		return err
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

	clh.vmconfig.Vsock = chclient.NewVsockConfig(cid, path)
	clh.vmconfig.Vsock.SetIommu(clh.config.IOMMU)
}

func (clh *cloudHypervisor) getRateLimiterConfig(bwSize, bwOneTimeBurst, opsSize, opsOneTimeBurst int64) *chclient.RateLimiterConfig {
	if bwSize == 0 && opsSize == 0 {
		return nil
	}

	rateLimiterConfig := chclient.NewRateLimiterConfig()

	if bwSize != 0 {
		bwTokenBucket := chclient.NewTokenBucket(bwSize, int64(utils.DefaultRateLimiterRefillTimeMilliSecs))

		if bwOneTimeBurst != 0 {
			bwTokenBucket.SetOneTimeBurst(bwOneTimeBurst)
		}

		rateLimiterConfig.SetBandwidth(*bwTokenBucket)
	}

	if opsSize != 0 {
		opsTokenBucket := chclient.NewTokenBucket(opsSize, int64(utils.DefaultRateLimiterRefillTimeMilliSecs))

		if opsOneTimeBurst != 0 {
			opsTokenBucket.SetOneTimeBurst(opsOneTimeBurst)
		}

		rateLimiterConfig.SetOps(*opsTokenBucket)
	}

	return rateLimiterConfig
}

func (clh *cloudHypervisor) getNetRateLimiterConfig() *chclient.RateLimiterConfig {
	return clh.getRateLimiterConfig(
		int64(utils.RevertBytes(uint64(clh.config.NetRateLimiterBwMaxRate/8))),
		int64(utils.RevertBytes(uint64(clh.config.NetRateLimiterBwOneTimeBurst/8))),
		clh.config.NetRateLimiterOpsMaxRate,
		clh.config.NetRateLimiterOpsOneTimeBurst)
}

func (clh *cloudHypervisor) getDiskRateLimiterConfig() *chclient.RateLimiterConfig {
	return clh.getRateLimiterConfig(
		int64(utils.RevertBytes(uint64(clh.config.DiskRateLimiterBwMaxRate/8))),
		int64(utils.RevertBytes(uint64(clh.config.DiskRateLimiterBwOneTimeBurst/8))),
		clh.config.DiskRateLimiterOpsMaxRate,
		clh.config.DiskRateLimiterOpsOneTimeBurst)
}

func (clh *cloudHypervisor) addNet(e Endpoint) error {
	clh.Logger().WithField("endpoint", e).Debugf("Adding Endpoint of type %v", e.Type())

	mac := e.HardwareAddr()
	netPair := e.NetworkPair()
	if netPair == nil {
		return errors.New("net Pair to be added is nil, needed to get TAP file descriptors")
	}

	if len(netPair.TapInterface.VMFds) == 0 {
		return errors.New("The file descriptors for the network pair are not present")
	}
	clh.netDevicesFiles[mac] = netPair.TapInterface.VMFds

	netRateLimiterConfig := clh.getNetRateLimiterConfig()

	net := chclient.NewNetConfig()
	net.Mac = &mac
	if netRateLimiterConfig != nil {
		net.SetRateLimiterConfig(*netRateLimiterConfig)
	}
	net.SetIommu(clh.config.IOMMU)

	if clh.netDevices != nil {
		*clh.netDevices = append(*clh.netDevices, *net)
	} else {
		clh.netDevices = &[]chclient.NetConfig{*net}
	}

	clh.Logger().Infof("Storing the Cloud Hypervisor network configuration: %+v", net)

	return nil
}

// Add shared Volume using virtiofs
func (clh *cloudHypervisor) addVolume(volume types.Volume) error {
	if clh.config.SharedFS != config.VirtioFS && clh.config.SharedFS != config.VirtioFSNydus {
		return fmt.Errorf("shared fs method not supported %s", clh.config.SharedFS)
	}

	vfsdSockPath, err := clh.virtioFsSocketPath(clh.id)
	if err != nil {
		return err
	}

	// numQueues and queueSize are required, let's use the
	// default values defined by cloud-hypervisor
	numQueues := int32(1)
	queueSize := int32(1024)
	if clh.config.VirtioFSQueueSize != 0 {
		queueSize = int32(clh.config.VirtioFSQueueSize)
	}

	fs := chclient.NewFsConfig(volume.MountTag, vfsdSockPath, numQueues, queueSize)
	fs.SetPciSegment(1)
	clh.vmconfig.Fs = &[]chclient.FsConfig{*fs}

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
			clh.Logger().WithError(err).WithField("path", path).Warn("removing vm socket failed")
		}
	}

	// Cleanup vm path
	dir := filepath.Join(clh.config.VMStorePath, clh.id)

	// If it's a symlink, remove both dir and the target.
	link, err := filepath.EvalSymlinks(dir)
	if err != nil {
		clh.Logger().WithError(err).WithField("dir", dir).Warn("failed to resolve vm path")
	}

	clh.Logger().WithFields(log.Fields{
		"link": link,
		"dir":  dir,
	}).Infof("Cleanup vm path")

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
		dir = filepath.Join(clh.config.VMStorePath, clh.config.VMid)
		if err := os.RemoveAll(dir); err != nil {
			if !force {
				return err
			}
			clh.Logger().WithError(err).WithField("path", dir).Warnf("failed to remove vm path")
		}
	}
	if rootless.IsRootless() {
		if _, err := user.Lookup(clh.config.User); err != nil {
			clh.Logger().WithError(err).WithFields(
				log.Fields{
					"user": clh.config.User,
					"uid":  clh.config.Uid,
				}).Warn("failed to find the user, it might have been removed")
			return nil
		}

		if err := pkgUtils.RemoveVmmUser(clh.config.User); err != nil {
			clh.Logger().WithError(err).WithFields(
				log.Fields{
					"user": clh.config.User,
					"uid":  clh.config.Uid,
				}).Warn("failed to delete the user")
			return nil
		}
		clh.Logger().WithFields(
			log.Fields{
				"user": clh.config.User,
				"uid":  clh.config.Uid,
			}).Debug("successfully removed the non root user")
	}

	clh.reset()

	return nil
}

func (clh *cloudHypervisor) GetTotalMemoryMB(ctx context.Context) uint32 {
	vminfo, err := clh.vmInfo()
	if err != nil {
		clh.Logger().WithError(err).Error("failed to get vminfo")
		return 0
	}

	return uint32(vminfo.GetMemoryActualSize() >> utils.MibToBytesShift)
}

// vmInfo ask to hypervisor for current VM status
func (clh *cloudHypervisor) vmInfo() (chclient.VmInfo, error) {
	cl := clh.client()
	ctx, cancelInfo := context.WithTimeout(context.Background(), clh.getClhAPITimeout()*time.Second)
	defer cancelInfo()

	info, _, err := cl.VmInfoGet(ctx)
	if err != nil {
		clh.Logger().WithError(openAPIClientError(err)).Warn("VmInfoGet failed")
	}
	return info, openAPIClientError(err)
}

func (clh *cloudHypervisor) IsRateLimiterBuiltin() bool {
	return true
}
