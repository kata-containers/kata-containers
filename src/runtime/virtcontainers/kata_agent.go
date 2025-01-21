// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	b64 "encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/docker/go-units"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/drivers"
	volume "github.com/kata-containers/kata-containers/src/runtime/pkg/direct-volume"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/uuid"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	kataclient "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/client"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"

	ctrAnnotations "github.com/containerd/containerd/pkg/cri/annotations"
	podmanAnnotations "github.com/containers/podman/v4/pkg/annotations"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/opencontainers/selinux/go-selinux"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	grpcStatus "google.golang.org/grpc/status"
	"google.golang.org/protobuf/encoding/protojson"
	"google.golang.org/protobuf/proto"
)

// kataAgentTracingTags defines tags for the trace span
var kataAgentTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "agent",
}

const (
	// KataEphemeralDevType creates a tmpfs backed volume for sharing files between containers.
	KataEphemeralDevType = "ephemeral"

	// KataLocalDevType creates a local directory inside the VM for sharing files between
	// containers.
	KataLocalDevType = "local"

	// Allocating an FSGroup that owns the pod's volumes
	fsGid = "fsgid"

	// path to vfio devices
	vfioPath = "/dev/vfio/"

	VirtualVolumePrefix = "io.katacontainers.volume="

	// enable debug console
	kernelParamDebugConsole           = "agent.debug_console"
	kernelParamDebugConsoleVPort      = "agent.debug_console_vport"
	kernelParamDebugConsoleVPortValue = "1026"

	// Default SELinux type applied to the container process inside guest
	defaultSeLinuxContainerType = "container_t"
)

type customRequestTimeoutKeyType struct{}

var (
	checkRequestTimeout              = 30 * time.Second
	createContainerRequestTimeout    = 60 * time.Second
	defaultRequestTimeout            = 60 * time.Second
	remoteRequestTimeout             = 300 * time.Second
	customRequestTimeoutKey          = customRequestTimeoutKeyType(struct{}{})
	errorMissingOCISpec              = errors.New("Missing OCI specification")
	defaultKataHostSharedDir         = "/run/kata-containers/shared/sandboxes/"
	defaultKataGuestSharedDir        = "/run/kata-containers/shared/containers/"
	defaultKataGuestNydusRootDir     = "/run/kata-containers/shared/"
	defaultKataGuestVirtualVolumedir = "/run/kata-containers/virtual-volumes/"
	mountGuestTag                    = "kataShared"
	defaultKataGuestSandboxDir       = "/run/kata-containers/sandbox/"
	type9pFs                         = "9p"
	typeVirtioFS                     = "virtiofs"
	typeOverlayFS                    = "overlay"
	kata9pDevType                    = "9p"
	kataMmioBlkDevType               = "mmioblk"
	kataBlkDevType                   = "blk"
	kataBlkCCWDevType                = "blk-ccw"
	kataSCSIDevType                  = "scsi"
	kataNvdimmDevType                = "nvdimm"
	kataVirtioFSDevType              = "virtio-fs"
	kataOverlayDevType               = "overlayfs"
	kataWatchableBindDevType         = "watchable-bind"
	kataVfioPciDevType               = "vfio-pci"     // VFIO PCI device to used as VFIO in the container
	kataVfioPciGuestKernelDevType    = "vfio-pci-gk"  // VFIO PCI device for consumption by the guest kernel
	kataVfioApDevType                = "vfio-ap"      // VFIO AP device for hot-plugging
	kataVfioApColdDevType            = "vfio-ap-cold" // VFIO AP device for cold-plugging
	sharedDir9pOptions               = []string{"trans=virtio,version=9p2000.L,cache=mmap", "nodev"}
	sharedDirVirtioFSOptions         = []string{}
	sharedDirVirtioFSDaxOptions      = "dax"
	shmDir                           = "shm"
	kataEphemeralDevType             = "ephemeral"
	defaultEphemeralPath             = filepath.Join(defaultKataGuestSandboxDir, kataEphemeralDevType)
	grpcMaxDataSize                  = int64(1024 * 1024)
	localDirOptions                  = []string{"mode=0777"}
	maxHostnameLen                   = 64
	GuestDNSFile                     = "/etc/resolv.conf"
)

const (
	grpcCheckRequest                          = "grpc.CheckRequest"
	grpcExecProcessRequest                    = "grpc.ExecProcessRequest"
	grpcCreateSandboxRequest                  = "grpc.CreateSandboxRequest"
	grpcDestroySandboxRequest                 = "grpc.DestroySandboxRequest"
	grpcCreateContainerRequest                = "grpc.CreateContainerRequest"
	grpcStartContainerRequest                 = "grpc.StartContainerRequest"
	grpcRemoveContainerRequest                = "grpc.RemoveContainerRequest"
	grpcSignalProcessRequest                  = "grpc.SignalProcessRequest"
	grpcUpdateRoutesRequest                   = "grpc.UpdateRoutesRequest"
	grpcUpdateInterfaceRequest                = "grpc.UpdateInterfaceRequest"
	grpcUpdateEphemeralMountsRequest          = "grpc.UpdateEphemeralMountsRequest"
	grpcRemoveStaleVirtiofsShareMountsRequest = "grpc.RemoveStaleVirtiofsShareMountsRequest"
	grpcListInterfacesRequest                 = "grpc.ListInterfacesRequest"
	grpcListRoutesRequest                     = "grpc.ListRoutesRequest"
	grpcAddARPNeighborsRequest                = "grpc.AddARPNeighborsRequest"
	grpcOnlineCPUMemRequest                   = "grpc.OnlineCPUMemRequest"
	grpcUpdateContainerRequest                = "grpc.UpdateContainerRequest"
	grpcWaitProcessRequest                    = "grpc.WaitProcessRequest"
	grpcTtyWinResizeRequest                   = "grpc.TtyWinResizeRequest"
	grpcWriteStreamRequest                    = "grpc.WriteStreamRequest"
	grpcCloseStdinRequest                     = "grpc.CloseStdinRequest"
	grpcStatsContainerRequest                 = "grpc.StatsContainerRequest"
	grpcPauseContainerRequest                 = "grpc.PauseContainerRequest"
	grpcResumeContainerRequest                = "grpc.ResumeContainerRequest"
	grpcReseedRandomDevRequest                = "grpc.ReseedRandomDevRequest"
	grpcGuestDetailsRequest                   = "grpc.GuestDetailsRequest"
	grpcMemHotplugByProbeRequest              = "grpc.MemHotplugByProbeRequest"
	grpcCopyFileRequest                       = "grpc.CopyFileRequest"
	grpcSetGuestDateTimeRequest               = "grpc.SetGuestDateTimeRequest"
	grpcGetOOMEventRequest                    = "grpc.GetOOMEventRequest"
	grpcGetMetricsRequest                     = "grpc.GetMetricsRequest"
	grpcAddSwapRequest                        = "grpc.AddSwapRequest"
	grpcVolumeStatsRequest                    = "grpc.VolumeStatsRequest"
	grpcResizeVolumeRequest                   = "grpc.ResizeVolumeRequest"
	grpcGetIPTablesRequest                    = "grpc.GetIPTablesRequest"
	grpcSetIPTablesRequest                    = "grpc.SetIPTablesRequest"
	grpcSetPolicyRequest                      = "grpc.SetPolicyRequest"
)

// newKataAgent returns an agent from an agent type.
func newKataAgent() agent {
	return &kataAgent{}
}

// The function is declared this way for mocking in unit tests
var kataHostSharedDir = func() string {
	if rootless.IsRootless() {
		// filepath.Join removes trailing slashes, but it is necessary for mounting
		return filepath.Join(rootless.GetRootlessDir(), defaultKataHostSharedDir) + "/"
	}
	return defaultKataHostSharedDir
}

func getPagesizeFromOpt(fsOpts []string) string {
	// example options array: "rw", "relatime", "seclabel", "pagesize=2M"
	for _, opt := range fsOpts {
		if strings.HasPrefix(opt, "pagesize=") {
			return strings.TrimPrefix(opt, "pagesize=")
		}
	}
	return ""
}

func getFSGroupChangePolicy(policy volume.FSGroupChangePolicy) pbTypes.FSGroupChangePolicy {
	switch policy {
	case volume.FSGroupChangeOnRootMismatch:
		return pbTypes.FSGroupChangePolicy_OnRootMismatch
	default:
		return pbTypes.FSGroupChangePolicy_Always
	}
}

// Shared path handling:
// 1. create three directories for each sandbox:
// -. /run/kata-containers/shared/sandboxes/$sbx_id/mounts/, a directory to hold all host/guest shared mounts
// -. /run/kata-containers/shared/sandboxes/$sbx_id/shared/, a host/guest shared directory (9pfs/virtiofs source dir)
// -. /run/kata-containers/shared/sandboxes/$sbx_id/private/, a directory to hold all temporary private mounts when creating ro mounts
//
// 2. /run/kata-containers/shared/sandboxes/$sbx_id/mounts/ is bind mounted readonly to /run/kata-containers/shared/sandboxes/$sbx_id/shared/, so guest cannot modify it
//
// 3. host-guest shared files/directories are mounted one-level under /run/kata-containers/shared/sandboxes/$sbx_id/mounts/ and thus present to guest at one level under /run/kata-containers/shared/sandboxes/$sbx_id/shared/
func GetSharePath(id string) string {
	return filepath.Join(kataHostSharedDir(), id, "shared")
}

func getMountPath(id string) string {
	return filepath.Join(kataHostSharedDir(), id, "mounts")
}

func getPrivatePath(id string) string {
	return filepath.Join(kataHostSharedDir(), id, "private")
}

func getSandboxPath(id string) string {
	return filepath.Join(kataHostSharedDir(), id)
}

// Use in nydus case, guest shared dir is compatible with virtiofsd sharedir
// nydus images are presented in kataGuestNydusImageDir
//
// virtiofs mountpoint: "/run/kata-containers/shared/"
// kataGuestSharedDir: "/run/kata-containers/shared/containers"
// kataGuestNydusImageDir: "/run/kata-containers/shared/rafs"
var kataGuestNydusRootDir = func() string {
	if rootless.IsRootless() {
		// filepath.Join removes trailing slashes, but it is necessary for mounting
		return filepath.Join(rootless.GetRootlessDir(), defaultKataGuestNydusRootDir) + "/"
	}
	return defaultKataGuestNydusRootDir
}

var rafsMountPath = func(cid string) string {
	return filepath.Join("/", nydusRafs, cid, lowerDir)
}

var kataGuestNydusImageDir = func() string {
	if rootless.IsRootless() {
		// filepath.Join removes trailing slashes, but it is necessary for mounting
		return filepath.Join(rootless.GetRootlessDir(), defaultKataGuestNydusRootDir, nydusRafs) + "/"
	}
	return filepath.Join(defaultKataGuestNydusRootDir, nydusRafs) + "/"
}

// The function is declared this way for mocking in unit tests
var kataGuestSharedDir = func() string {
	if rootless.IsRootless() {
		// filepath.Join removes trailing slashes, but it is necessary for mounting
		return filepath.Join(rootless.GetRootlessDir(), defaultKataGuestSharedDir) + "/"
	}
	return defaultKataGuestSharedDir
}

// The function is declared this way for mocking in unit tests
var kataGuestSandboxDir = func() string {
	if rootless.IsRootless() {
		// filepath.Join removes trailing slashes, but it is necessary for mounting
		return filepath.Join(rootless.GetRootlessDir(), defaultKataGuestSandboxDir) + "/"
	}
	return defaultKataGuestSandboxDir
}

var kataGuestSandboxStorageDir = func() string {
	return filepath.Join(defaultKataGuestSandboxDir, "storage")
}

func ephemeralPath() string {
	if rootless.IsRootless() {
		return filepath.Join(kataGuestSandboxDir(), kataEphemeralDevType)
	}
	return defaultEphemeralPath
}

// KataAgentConfig is a structure storing information needed
// to reach the Kata Containers agent.
type KataAgentConfig struct {
	KernelModules      []string
	ContainerPipeSize  uint32
	DialTimeout        uint32
	CdhApiTimeout      uint32
	LongLiveConn       bool
	Debug              bool
	Trace              bool
	EnableDebugConsole bool
	Policy             string
}

// KataAgentState is the structure describing the data stored from this
// agent implementation.
type KataAgentState struct {
	URL string
}

// nolint: govet
type kataAgent struct {
	ctx      context.Context
	vmSocket interface{}

	client *kataclient.AgentClient

	// lock protects the client pointer
	sync.Mutex

	state KataAgentState

	reqHandlers map[string]reqFunc
	kmodules    []string

	dialTimout uint32

	keepConn bool
	dead     bool
}

func (k *kataAgent) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "kata_agent")
}

func (k *kataAgent) longLiveConn() bool {
	return k.keepConn
}

// KataAgentKernelParams returns a list of Kata Agent specific kernel
// parameters.
func KataAgentKernelParams(config KataAgentConfig) []Param {
	var params []Param

	if config.Debug {
		params = append(params, Param{Key: "agent.log", Value: "debug"})
	}

	if config.Trace {
		params = append(params, Param{Key: "agent.trace", Value: "true"})
	}

	if config.ContainerPipeSize > 0 {
		containerPipeSize := strconv.FormatUint(uint64(config.ContainerPipeSize), 10)
		params = append(params, Param{Key: vcAnnotations.ContainerPipeSizeKernelParam, Value: containerPipeSize})
	}

	if config.EnableDebugConsole {
		params = append(params, Param{Key: kernelParamDebugConsole, Value: ""})
		params = append(params, Param{Key: kernelParamDebugConsoleVPort, Value: kernelParamDebugConsoleVPortValue})
	}

	if config.CdhApiTimeout > 0 {
		cdhApiTimeout := strconv.FormatUint(uint64(config.CdhApiTimeout), 10)
		params = append(params, Param{Key: vcAnnotations.CdhApiTimeoutKernelParam, Value: cdhApiTimeout})
	}

	return params
}

func (k *kataAgent) handleTraceSettings(config KataAgentConfig) bool {
	disableVMShutdown := false

	if config.Trace {
		// Agent tracing requires that the agent be able to shutdown
		// cleanly. This is the only scenario where the agent is
		// responsible for stopping the VM: normally this is handled
		// by the runtime.
		disableVMShutdown = true
	}

	return disableVMShutdown
}

func (k *kataAgent) init(ctx context.Context, sandbox *Sandbox, config KataAgentConfig) (disableVMShutdown bool, err error) {
	// Save
	k.ctx = sandbox.ctx

	span, _ := katatrace.Trace(ctx, k.Logger(), "init", kataAgentTracingTags)
	defer span.End()

	disableVMShutdown = k.handleTraceSettings(config)
	k.keepConn = config.LongLiveConn
	k.kmodules = config.KernelModules
	k.dialTimout = config.DialTimeout

	createContainerRequestTimeout = time.Duration(sandbox.config.CreateContainerTimeout) * time.Second
	k.Logger().WithFields(logrus.Fields{
		"createContainerRequestTimeout": fmt.Sprintf("%+v", createContainerRequestTimeout),
	}).Info("The createContainerRequestTimeout has been set ")

	return disableVMShutdown, nil
}

func (k *kataAgent) agentURL() (string, error) {
	switch s := k.vmSocket.(type) {
	case types.VSock:
		return s.String(), nil
	case types.HybridVSock:
		return s.String(), nil
	case types.RemoteSock:
		return s.String(), nil
	case types.MockHybridVSock:
		return s.String(), nil
	default:
		return "", fmt.Errorf("Invalid socket type")
	}
}

func (k *kataAgent) capabilities() types.Capabilities {
	var caps types.Capabilities

	// add all Capabilities supported by agent
	caps.SetBlockDeviceSupport()

	return caps
}

func (k *kataAgent) internalConfigure(ctx context.Context, h Hypervisor, id string, config KataAgentConfig) error {
	span, _ := katatrace.Trace(ctx, k.Logger(), "configure", kataAgentTracingTags)
	defer span.End()

	var err error
	if k.vmSocket, err = h.GenerateSocket(id); err != nil {
		return err
	}
	k.keepConn = config.LongLiveConn

	katatrace.AddTags(span, "socket", k.vmSocket)

	return nil
}

func (k *kataAgent) configure(ctx context.Context, h Hypervisor, id, sharePath string, config KataAgentConfig) error {
	span, ctx := katatrace.Trace(ctx, k.Logger(), "configure", kataAgentTracingTags)
	defer span.End()

	err := k.internalConfigure(ctx, h, id, config)
	if err != nil {
		return err
	}

	switch s := k.vmSocket.(type) {
	case types.VSock:
		if err = h.AddDevice(ctx, s, VSockPCIDev); err != nil {
			return err
		}
	case types.HybridVSock:
		err = h.AddDevice(ctx, s, HybridVirtioVsockDev)
		if err != nil {
			return err
		}
	case types.RemoteSock:
	case types.MockHybridVSock:
	default:
		return types.ErrInvalidConfigType
	}

	// Neither create shared directory nor add 9p device if hypervisor
	// doesn't support filesystem sharing.
	caps := h.Capabilities(ctx)
	if !caps.IsFsSharingSupported() {
		return nil
	}

	// Create shared directory and add the shared volume if filesystem sharing is supported.
	// This volume contains all bind mounted container bundles.
	sharedVolume := types.Volume{
		MountTag: mountGuestTag,
		HostPath: sharePath,
	}

	if err = os.MkdirAll(sharedVolume.HostPath, DirMode); err != nil {
		return err
	}

	return h.AddDevice(ctx, sharedVolume, FsDev)
}

func (k *kataAgent) configureFromGrpc(ctx context.Context, h Hypervisor, id string, config KataAgentConfig) error {
	return k.internalConfigure(ctx, h, id, config)
}

func (k *kataAgent) createSandbox(ctx context.Context, sandbox *Sandbox) error {
	span, ctx := katatrace.Trace(ctx, k.Logger(), "createSandbox", kataAgentTracingTags)
	defer span.End()

	return k.configure(ctx, sandbox.hypervisor, sandbox.id, GetSharePath(sandbox.id), sandbox.config.AgentConfig)
}

func cmdToKataProcess(cmd types.Cmd) (process *grpc.Process, err error) {
	var i uint64
	var extraGids []uint32

	// Number of bits used to store user+group values in
	// the gRPC "User" type.
	const grpcUserBits = 32

	// User can contain only the "uid" or it can contain "uid:gid".
	parsedUser := strings.Split(cmd.User, ":")
	if len(parsedUser) > 2 {
		return nil, fmt.Errorf("cmd.User %q format is wrong", cmd.User)
	}

	i, err = strconv.ParseUint(parsedUser[0], 10, grpcUserBits)
	if err != nil {
		return nil, err
	}

	uid := uint32(i)

	var gid uint32
	if len(parsedUser) > 1 {
		i, err = strconv.ParseUint(parsedUser[1], 10, grpcUserBits)
		if err != nil {
			return nil, err
		}

		gid = uint32(i)
	}

	if cmd.PrimaryGroup != "" {
		i, err = strconv.ParseUint(cmd.PrimaryGroup, 10, grpcUserBits)
		if err != nil {
			return nil, err
		}

		gid = uint32(i)
	}

	for _, g := range cmd.SupplementaryGroups {
		var extraGid uint64

		extraGid, err = strconv.ParseUint(g, 10, grpcUserBits)
		if err != nil {
			return nil, err
		}

		extraGids = append(extraGids, uint32(extraGid))
	}

	process = &grpc.Process{
		Terminal: cmd.Interactive,
		User: &grpc.User{
			UID:            uid,
			GID:            gid,
			AdditionalGids: extraGids,
		},
		Args: cmd.Args,
		Env:  cmdEnvsToStringSlice(cmd.Envs),
		Cwd:  cmd.WorkDir,
	}

	return process, nil
}

func cmdEnvsToStringSlice(ev []types.EnvVar) []string {
	var env []string

	for _, e := range ev {
		pair := []string{e.Var, e.Value}
		env = append(env, strings.Join(pair, "="))
	}

	return env
}

func (k *kataAgent) exec(ctx context.Context, sandbox *Sandbox, c Container, cmd types.Cmd) (*Process, error) {
	span, ctx := katatrace.Trace(ctx, k.Logger(), "exec", kataAgentTracingTags)
	defer span.End()

	var kataProcess *grpc.Process

	kataProcess, err := cmdToKataProcess(cmd)
	if err != nil {
		return nil, err
	}

	req := &grpc.ExecProcessRequest{
		ContainerId: c.id,
		ExecId:      uuid.Generate().String(),
		Process:     kataProcess,
	}

	if _, err := k.sendReq(ctx, req); err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return nil, status.Errorf(codes.DeadlineExceeded, "ExecProcessRequest timed out")
		}
		return nil, err
	}

	return buildProcessFromExecID(req.ExecId)
}

func (k *kataAgent) updateInterface(ctx context.Context, ifc *pbTypes.Interface) (*pbTypes.Interface, error) {
	// send update interface request
	ifcReq := &grpc.UpdateInterfaceRequest{
		Interface: ifc,
	}
	resultingInterface, err := k.sendReq(ctx, ifcReq)
	if err != nil {
		k.Logger().WithFields(logrus.Fields{
			"interface-requested": fmt.Sprintf("%+v", ifc),
			"resulting-interface": fmt.Sprintf("%+v", resultingInterface),
		}).WithError(err).Error("update interface request failed")
		if err.Error() == context.DeadlineExceeded.Error() {
			return nil, status.Errorf(codes.DeadlineExceeded, "UpdateInterfaceRequest timed out")
		}
	}
	if resultInterface, ok := resultingInterface.(*pbTypes.Interface); ok {
		return resultInterface, err
	}
	return nil, err
}

func (k *kataAgent) updateInterfaces(ctx context.Context, interfaces []*pbTypes.Interface) error {
	for _, ifc := range interfaces {
		if _, err := k.updateInterface(ctx, ifc); err != nil {
			return err
		}
	}
	return nil
}

func (k *kataAgent) updateRoutes(ctx context.Context, routes []*pbTypes.Route) ([]*pbTypes.Route, error) {
	if routes != nil {
		routesReq := &grpc.UpdateRoutesRequest{
			Routes: &grpc.Routes{
				Routes: routes,
			},
		}
		resultingRoutes, err := k.sendReq(ctx, routesReq)
		if err != nil {
			k.Logger().WithFields(logrus.Fields{
				"routes-requested": fmt.Sprintf("%+v", routes),
				"resulting-routes": fmt.Sprintf("%+v", resultingRoutes),
			}).WithError(err).Error("update routes request failed")
			if err.Error() == context.DeadlineExceeded.Error() {
				return nil, status.Errorf(codes.DeadlineExceeded, "UpdateRoutesRequest timed out")
			}
		}
		resultRoutes, ok := resultingRoutes.(*grpc.Routes)
		if ok && resultRoutes != nil {
			return resultRoutes.Routes, err
		}
		return nil, err
	}
	return nil, nil
}

func (k *kataAgent) updateEphemeralMounts(ctx context.Context, storages []*grpc.Storage) error {
	if storages != nil {
		storagesReq := &grpc.UpdateEphemeralMountsRequest{
			Storages: storages,
		}

		if _, err := k.sendReq(ctx, storagesReq); err != nil {
			k.Logger().WithError(err).Error("update mounts request failed")
			if err.Error() == context.DeadlineExceeded.Error() {
				return status.Errorf(codes.DeadlineExceeded, "UpdateEphemeralMountsRequest timed out")
			}
			return err
		}
		return nil
	}
	return nil
}

func (k *kataAgent) addARPNeighbors(ctx context.Context, neighs []*pbTypes.ARPNeighbor) error {
	if neighs != nil {
		neighsReq := &grpc.AddARPNeighborsRequest{
			Neighbors: &grpc.ARPNeighbors{
				ARPNeighbors: neighs,
			},
		}
		_, err := k.sendReq(ctx, neighsReq)
		if err != nil {
			if grpcStatus.Convert(err).Code() == codes.Unimplemented {
				k.Logger().WithFields(logrus.Fields{
					"arpneighbors-requested": fmt.Sprintf("%+v", neighs),
				}).Warn("add ARP neighbors request failed due to old agent, please upgrade Kata Containers image version")
				return nil
			}
			if err.Error() == context.DeadlineExceeded.Error() {
				return status.Errorf(codes.DeadlineExceeded, "AddARPNeighborsRequest timed out")
			}
			k.Logger().WithFields(logrus.Fields{
				"arpneighbors-requested": fmt.Sprintf("%+v", neighs),
			}).WithError(err).Error("add ARP neighbors request failed")
		}
		return err
	}
	return nil
}

func (k *kataAgent) listInterfaces(ctx context.Context) ([]*pbTypes.Interface, error) {
	req := &grpc.ListInterfacesRequest{}
	resultingInterfaces, err := k.sendReq(ctx, req)
	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return nil, status.Errorf(codes.DeadlineExceeded, "ListInterfacesRequest timed out")
		}
		return nil, err
	}
	resultInterfaces, ok := resultingInterfaces.(*grpc.Interfaces)
	if !ok {
		return nil, fmt.Errorf("Unexpected type %T for interfaces", resultingInterfaces)
	}
	return resultInterfaces.Interfaces, nil
}

func (k *kataAgent) listRoutes(ctx context.Context) ([]*pbTypes.Route, error) {
	req := &grpc.ListRoutesRequest{}
	resultingRoutes, err := k.sendReq(ctx, req)
	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return nil, status.Errorf(codes.DeadlineExceeded, "ListRoutesRequest timed out")
		}
		return nil, err
	}
	resultRoutes, ok := resultingRoutes.(*grpc.Routes)
	if !ok {
		return nil, fmt.Errorf("Unexpected type %T for routes", resultingRoutes)
	}
	return resultRoutes.Routes, nil
}

func (k *kataAgent) getAgentURL() (string, error) {
	return k.agentURL()
}

func (k *kataAgent) setAgentURL() error {
	var err error
	if k.state.URL, err = k.agentURL(); err != nil {
		return err
	}

	return nil
}

func (k *kataAgent) reuseAgent(agent agent) error {
	a, ok := agent.(*kataAgent)
	if !ok {
		return fmt.Errorf("Bug: get a wrong type of agent")
	}

	k.installReqFunc(a.client)
	k.client = a.client
	return nil
}

func (k *kataAgent) getDNS(sandbox *Sandbox) ([]string, error) {
	ociSpec := sandbox.GetPatchedOCISpec()
	if ociSpec == nil {
		k.Logger().Debug("Sandbox OCI spec not found. Sandbox DNS will not be set.")
		return nil, nil
	}

	ociMounts := ociSpec.Mounts

	for _, m := range ociMounts {
		if m.Destination == GuestDNSFile {
			content, err := os.ReadFile(m.Source)
			if err != nil {
				return nil, fmt.Errorf("Could not read file %s: %s", m.Source, err)
			}
			dns := strings.Split(string(content), "\n")
			return dns, nil

		}
	}
	k.Logger().Debug("DNS file not present in ociMounts. Sandbox DNS will not be set.")
	return nil, nil
}

func (k *kataAgent) startSandbox(ctx context.Context, sandbox *Sandbox) error {
	span, ctx := katatrace.Trace(ctx, k.Logger(), "StartVM", kataAgentTracingTags)
	defer span.End()

	if err := k.setAgentURL(); err != nil {
		return err
	}

	hostname := sandbox.config.Hostname
	if len(hostname) > maxHostnameLen {
		hostname = hostname[:maxHostnameLen]
	}

	dns, err := k.getDNS(sandbox)
	if err != nil {
		return err
	}

	var kmodules []*grpc.KernelModule

	if sandbox.config.HypervisorType == RemoteHypervisor {
		ctx = context.WithValue(ctx, customRequestTimeoutKey, remoteRequestTimeout)
	}

	// Check grpc server is serving
	if err = k.check(ctx); err != nil {
		return err
	}

	// If a Policy has been specified, send it to the agent.
	if len(sandbox.config.AgentConfig.Policy) > 0 {
		if err := sandbox.agent.setPolicy(ctx, sandbox.config.AgentConfig.Policy); err != nil {
			return err
		}
	}

	if sandbox.config.HypervisorType != RemoteHypervisor {
		// Setup network interfaces and routes
		err = k.setupNetworks(ctx, sandbox, nil)
		if err != nil {
			return err
		}
		kmodules = setupKernelModules(k.kmodules)
	}

	storages := setupStorages(ctx, sandbox)

	req := &grpc.CreateSandboxRequest{
		Hostname:      hostname,
		Dns:           dns,
		Storages:      storages,
		SandboxPidns:  sandbox.sharePidNs,
		SandboxId:     sandbox.id,
		GuestHookPath: sandbox.config.HypervisorConfig.GuestHookPath,
		KernelModules: kmodules,
	}

	_, err = k.sendReq(ctx, req)
	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return status.Errorf(codes.DeadlineExceeded, "CreateSandboxRequest timed out")
		}
		return err
	}

	return nil
}

func setupKernelModules(kmodules []string) []*grpc.KernelModule {
	modules := []*grpc.KernelModule{}

	for _, m := range kmodules {
		l := strings.Fields(strings.TrimSpace(m))
		if len(l) == 0 {
			continue
		}

		module := &grpc.KernelModule{Name: l[0]}
		modules = append(modules, module)
		if len(l) == 1 {
			continue
		}

		module.Parameters = append(module.Parameters, l[1:]...)
	}

	return modules
}

func setupStorages(ctx context.Context, sandbox *Sandbox) []*grpc.Storage {
	storages := []*grpc.Storage{}
	caps := sandbox.hypervisor.Capabilities(ctx)

	// append 9p shared volume to storages only if filesystem sharing is supported
	if caps.IsFsSharingSupported() {
		// We mount the shared directory in a predefined location
		// in the guest.
		// This is where at least some of the host config files
		// (resolv.conf, etc...) and potentially all container
		// rootfs will reside.
		sharedFS := sandbox.config.HypervisorConfig.SharedFS
		if sharedFS == config.VirtioFS || sharedFS == config.VirtioFSNydus {
			// If virtio-fs uses either of the two cache options 'auto, always',
			// the guest directory can be mounted with option 'dax' allowing it to
			// directly map contents from the host. When set to 'never', the mount
			// options should not contain 'dax' lest the virtio-fs daemon crashing
			// with an invalid address reference.
			if sandbox.config.HypervisorConfig.VirtioFSCache != typeVirtioFSCacheModeNever {
				// If virtio_fs_cache_size = 0, dax should not be used.
				if sandbox.config.HypervisorConfig.VirtioFSCacheSize != 0 {
					sharedDirVirtioFSOptions = append(sharedDirVirtioFSOptions, sharedDirVirtioFSDaxOptions)
				}
			}
			mountPoint := kataGuestSharedDir()
			if sharedFS == config.VirtioFSNydus {
				mountPoint = kataGuestNydusRootDir()
			}
			sharedVolume := &grpc.Storage{
				Driver:     kataVirtioFSDevType,
				Source:     mountGuestTag,
				MountPoint: mountPoint,
				Fstype:     typeVirtioFS,
				Options:    sharedDirVirtioFSOptions,
			}

			storages = append(storages, sharedVolume)
		} else {
			sharedDir9pOptions = append(sharedDir9pOptions, fmt.Sprintf("msize=%d", sandbox.config.HypervisorConfig.Msize9p))

			sharedVolume := &grpc.Storage{
				Driver:     kata9pDevType,
				Source:     mountGuestTag,
				MountPoint: kataGuestSharedDir(),
				Fstype:     type9pFs,
				Options:    sharedDir9pOptions,
			}

			storages = append(storages, sharedVolume)
		}
	}

	if sandbox.shmSize > 0 {
		path := filepath.Join(kataGuestSandboxDir(), shmDir)
		shmSizeOption := fmt.Sprintf("size=%d", sandbox.shmSize)

		shmStorage := &grpc.Storage{
			Driver:     KataEphemeralDevType,
			MountPoint: path,
			Source:     "shm",
			Fstype:     "tmpfs",
			Options:    []string{"noexec", "nosuid", "nodev", "mode=1777", shmSizeOption},
		}

		storages = append(storages, shmStorage)
	}

	return storages
}

func (k *kataAgent) stopSandbox(ctx context.Context, sandbox *Sandbox) error {
	span, ctx := katatrace.Trace(ctx, k.Logger(), "stopSandbox", kataAgentTracingTags)
	defer span.End()

	req := &grpc.DestroySandboxRequest{}

	if _, err := k.sendReq(ctx, req); err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return status.Errorf(codes.DeadlineExceeded, "DestroySandboxRequest timed out")
		}
		return err
	}

	return nil
}

func (k *kataAgent) replaceOCIMountSource(spec *specs.Spec, guestMounts map[string]Mount) error {
	ociMounts := spec.Mounts

	for index, m := range ociMounts {
		if guestMount, ok := guestMounts[m.Destination]; ok {
			k.Logger().Debugf("Replacing OCI mount (%s) source %s with %s", m.Destination, m.Source, guestMount.Source)
			ociMounts[index].Source = guestMount.Source
		}
	}

	return nil
}

func (k *kataAgent) removeIgnoredOCIMount(spec *specs.Spec, ignoredMounts map[string]Mount) error {
	var mounts []specs.Mount

	for _, m := range spec.Mounts {
		if _, found := ignoredMounts[m.Source]; found {
			k.Logger().WithField("removed-mount", m.Source).Debug("Removing OCI mount")
		} else if HasOption(m.Options, vcAnnotations.IsFileSystemLayer) {
			k.Logger().WithField("removed-mount", m.Source).Debug("Removing layer")
		} else {
			mounts = append(mounts, m)
		}
	}

	// Replace the OCI mounts with the updated list.
	spec.Mounts = mounts

	return nil
}

func (k *kataAgent) constrainGRPCSpec(grpcSpec *grpc.Spec, passSeccomp bool, disableGuestSeLinux bool, guestSeLinuxLabel string, stripVfio bool) error {
	// Disable Hooks since they have been handled on the host and there is
	// no reason to send them to the agent. It would make no sense to try
	// to apply them on the guest.
	grpcSpec.Hooks = nil

	// Pass seccomp only if disable_guest_seccomp is set to false in
	// configuration.toml and guest image is seccomp capable.
	if !passSeccomp {
		grpcSpec.Linux.Seccomp = nil
	}

	// Pass SELinux label for the container process to the agent.
	if grpcSpec.Process.SelinuxLabel != "" {
		if !disableGuestSeLinux {
			k.Logger().Info("SELinux label will be applied to the container process inside guest")

			var label string
			if guestSeLinuxLabel != "" {
				label = guestSeLinuxLabel
			} else {
				label = grpcSpec.Process.SelinuxLabel
			}

			processContext, err := selinux.NewContext(label)
			if err != nil {
				return err
			}

			// Change the type from KVM to container because the type passed from the high-level
			// runtime is for KVM process.
			if guestSeLinuxLabel == "" {
				processContext["type"] = defaultSeLinuxContainerType
			}
			grpcSpec.Process.SelinuxLabel = processContext.Get()
		} else {
			k.Logger().Info("Empty SELinux label for the process and the mount because guest SELinux is disabled")
			grpcSpec.Process.SelinuxLabel = ""
			grpcSpec.Linux.MountLabel = ""
		}
	}

	// By now only CPU constraints are supported
	// Issue: https://github.com/kata-containers/runtime/issues/158
	// Issue: https://github.com/kata-containers/runtime/issues/204
	grpcSpec.Linux.Resources.Devices = nil
	grpcSpec.Linux.Resources.Pids = nil
	grpcSpec.Linux.Resources.BlockIO = nil
	grpcSpec.Linux.Resources.Network = nil
	if grpcSpec.Linux.Resources.CPU != nil {
		grpcSpec.Linux.Resources.CPU.Cpus = ""
		grpcSpec.Linux.Resources.CPU.Mems = ""
	}

	// We need agent systemd cgroup now.
	// There are three main reasons to do not apply systemd cgroups in the VM
	// - Initrd image doesn't have systemd.
	// - Nobody will be able to modify the resources of a specific container by using systemctl set-property.
	// - docker is not running in the VM.
	// if resCtrl.IsSystemdCgroup(grpcSpec.Linux.CgroupsPath) {
	// 	// Convert systemd cgroup to cgroupfs
	// 	slice := strings.Split(grpcSpec.Linux.CgroupsPath, ":")
	// 	// 0 - slice: system.slice
	// 	// 1 - prefix: docker
	// 	// 2 - name: abc123
	// 	grpcSpec.Linux.CgroupsPath = filepath.Join("/", slice[1], slice[2])
	// }

	// Disable network namespace since it is already handled on the host by
	// virtcontainers. The network is a complex part which cannot be simply
	// passed to the agent.
	// Every other namespaces's paths have to be emptied. This way, there
	// is no confusion from the agent, trying to find an existing namespace
	// on the guest.
	var tmpNamespaces []*grpc.LinuxNamespace
	for _, ns := range grpcSpec.Linux.Namespaces {
		switch ns.Type {
		case string(specs.CgroupNamespace):
		case string(specs.NetworkNamespace):
		default:
			ns.Path = ""
			tmpNamespaces = append(tmpNamespaces, ns)
		}
	}
	grpcSpec.Linux.Namespaces = tmpNamespaces

	if stripVfio {
		// VFIO char device shouldn't appear in the guest
		// (because the VM device driver will do something
		// with it rather than just presenting it to the
		// container unmodified)
		var linuxDevices []*grpc.LinuxDevice
		for _, dev := range grpcSpec.Linux.Devices {
			if dev.Type == "c" && strings.HasPrefix(dev.Path, vfioPath) {
				k.Logger().WithField("vfio-dev", dev.Path).Debug("removing vfio device from grpcSpec")
				continue
			}
			linuxDevices = append(linuxDevices, dev)
		}
		grpcSpec.Linux.Devices = linuxDevices
	}

	return nil
}

func (k *kataAgent) handleShm(mounts []specs.Mount, sandbox *Sandbox) {
	for idx, mnt := range mounts {
		if mnt.Destination != "/dev/shm" {
			continue
		}

		// If /dev/shm for a container is backed by an ephemeral volume, skip
		// bind-mounting it to the sandbox shm.
		// A later call to handleEphemeralStorage should take care of setting up /dev/shm correctly.
		if mnt.Type == KataEphemeralDevType {
			continue
		}

		// A container shm mount is shared with sandbox shm mount.
		if sandbox.shmSize > 0 {
			mounts[idx].Type = "bind"
			mounts[idx].Options = []string{"rbind"}
			mounts[idx].Source = filepath.Join(kataGuestSandboxDir(), shmDir)
			k.Logger().WithField("shm-size", sandbox.shmSize).Info("Using sandbox shm")
		} else {
			// This should typically not happen, as a sandbox shm mount is always set up by the
			// upper stack.
			sizeOption := fmt.Sprintf("size=%d", DefaultShmSize)
			mounts[idx].Type = "tmpfs"
			mounts[idx].Source = "shm"
			mounts[idx].Options = []string{"noexec", "nosuid", "nodev", "mode=1777", sizeOption}
			k.Logger().WithField("shm-size", sizeOption).Info("Setting up a separate shm for container")
		}
	}
}

func (k *kataAgent) appendBlockDevice(dev ContainerDevice, device api.Device, c *Container) *grpc.Device {
	d, ok := device.GetDeviceInfo().(*config.BlockDrive)
	if !ok || d == nil {
		k.Logger().WithField("device", device).Error("malformed block drive")
		return nil
	}

	if d.Pmem {
		// block drive is a persistent memory device that
		// was passed as volume (-v) not as device (--device).
		// It shouldn't be visible in the container
		return nil
	}

	kataDevice := &grpc.Device{
		ContainerPath: dev.ContainerPath,
	}

	switch c.sandbox.config.HypervisorConfig.BlockDeviceDriver {
	case config.VirtioMmio:
		kataDevice.Type = kataMmioBlkDevType
		kataDevice.Id = d.VirtPath
		kataDevice.VmPath = d.VirtPath
	case config.VirtioBlockCCW:
		kataDevice.Type = kataBlkCCWDevType
		kataDevice.Id = d.DevNo
	case config.VirtioBlock:
		kataDevice.Type = kataBlkDevType
		kataDevice.Id = d.PCIPath.String()
		kataDevice.VmPath = d.VirtPath
	case config.VirtioSCSI:
		kataDevice.Type = kataSCSIDevType
		kataDevice.Id = d.SCSIAddr
	case config.Nvdimm:
		kataDevice.Type = kataNvdimmDevType
		kataDevice.VmPath = fmt.Sprintf("/dev/pmem%s", d.NvdimmID)
	}

	return kataDevice
}

func (k *kataAgent) appendVhostUserBlkDevice(dev ContainerDevice, device api.Device, c *Container) *grpc.Device {
	d, ok := device.GetDeviceInfo().(*config.VhostUserDeviceAttrs)
	if !ok || d == nil {
		k.Logger().WithField("device", device).Error("malformed vhost-user-blk drive")
		return nil
	}

	kataDevice := &grpc.Device{
		ContainerPath: dev.ContainerPath,
		Type:          kataBlkDevType,
		Id:            d.PCIPath.String(),
	}

	return kataDevice
}

func (k *kataAgent) appendVfioDevice(dev ContainerDevice, device api.Device, c *Container) *grpc.Device {
	devList, ok := device.GetDeviceInfo().([]*config.VFIODev)
	if !ok || devList == nil {
		k.Logger().WithField("device", device).Error("malformed vfio device")
		return nil
	}

	groupNum := filepath.Base(dev.ContainerPath)

	// For VFIO-PCI, each /dev/vfio/NN device represents a VFIO group,
	// which could include several PCI devices. So we give group
	// information in the main structure, then list each individual PCI
	// device in the Options array.
	//
	// Each option is formatted as "DDDD:BB:DD.F=<pcipath>"
	// DDDD:BB:DD.F is the device's PCI address on the
	// *host*. <pcipath> is the device's PCI path in the guest
	// (see qomGetPciPath() for details).
	//
	// For VFIO-AP, one VFIO group could include several queue devices. They are
	// identified by APQNs (Adjunct Processor Queue Numbers), which do not differ
	// between host and guest. They are passed as options so they can be awaited
	// by the agent.
	kataDevice := &grpc.Device{
		ContainerPath: dev.ContainerPath,
		Type:          kataVfioPciDevType,
		Id:            groupNum,
		Options:       make([]string, len(devList)),
	}

	// We always pass the device information to the agent, since
	// it needs that to wait for them to be ready.  But depending
	// on the vfio_mode, we need to use a different device type so
	// the agent can handle it properly
	if c.sandbox.config.VfioMode == config.VFIOModeGuestKernel {
		kataDevice.Type = kataVfioPciGuestKernelDevType
	}
	for i, dev := range devList {
		if dev.Type == config.VFIOAPDeviceMediatedType {
			kataDevice.Type = kataVfioApDevType
			coldPlugVFIO := (c.sandbox.config.HypervisorConfig.ColdPlugVFIO != config.NoPort)
			if coldPlugVFIO && c.sandbox.config.VfioMode == config.VFIOModeVFIO {
				// A new device type is required for cold-plugging VFIO-AP.
				// The VM guest should handle this differently from hot-plugging VFIO-AP
				// (e.g., wait_for_ap_device).
				// Note that a device already exists for cold-plugging VFIO-AP
				// at the time the device type is checked.
				kataDevice.Type = kataVfioApColdDevType
			}
			kataDevice.Options = dev.APDevices
		} else {

			devBDF := drivers.GetBDF(dev.BDF)
			kataDevice.Options[i] = fmt.Sprintf("0000:%s=%s", devBDF, dev.GuestPciPath)
		}

	}

	return kataDevice
}

func (k *kataAgent) appendDevices(deviceList []*grpc.Device, c *Container) []*grpc.Device {
	for _, dev := range c.devices {
		device := c.sandbox.devManager.GetDeviceByID(dev.ID)
		if device == nil {
			k.Logger().WithField("device", dev.ID).Error("failed to find device by id")
			return nil
		}

		if strings.HasPrefix(dev.ContainerPath, defaultKataGuestVirtualVolumedir) {
			continue
		}

		var kataDevice *grpc.Device

		switch device.DeviceType() {
		case config.DeviceBlock:
			kataDevice = k.appendBlockDevice(dev, device, c)
		case config.VhostUserBlk:
			kataDevice = k.appendVhostUserBlkDevice(dev, device, c)
		case config.DeviceVFIO:
			kataDevice = k.appendVfioDevice(dev, device, c)
		}

		if kataDevice == nil || kataDevice.Type == "" {
			continue
		}

		deviceList = append(deviceList, kataDevice)
	}

	return deviceList
}

// rollbackFailingContainerCreation rolls back important steps that might have
// been performed before the container creation failed.
// - Unmount container volumes.
// - Unmount container rootfs.
func (k *kataAgent) rollbackFailingContainerCreation(ctx context.Context, c *Container) {
	if c != nil {
		if err2 := c.unmountHostMounts(ctx); err2 != nil {
			k.Logger().WithError(err2).Error("rollback failed unmountHostMounts()")
		}

		if err2 := c.sandbox.fsShare.UnshareRootFilesystem(ctx, c); err2 != nil {
			k.Logger().WithError(err2).Error("rollback failed UnshareRootfs()")
		}
	}
}

func (k *kataAgent) setupNetworks(ctx context.Context, sandbox *Sandbox, c *Container) error {
	if sandbox.network.NetworkID() == "" {
		return nil
	}

	var err error
	var endpoints []Endpoint
	if c == nil || c.id == sandbox.id {
		// TODO: VFIO network deivce has not been hotplugged when creating the Sandbox,
		// so need to skip VFIO endpoint here.
		// After KEP #4113(https://github.com/kubernetes/enhancements/pull/4113)
		// is implemented, the VFIO network devices will be attached before container
		// creation, so no need to skip them here anymore.
		for _, ep := range sandbox.network.Endpoints() {
			if ep.Type() != VfioEndpointType {
				endpoints = append(endpoints, ep)
			}
		}
	} else if !sandbox.hotplugNetworkConfigApplied {
		// Apply VFIO network devices' configuration after they are hot-plugged.
		for _, ep := range sandbox.network.Endpoints() {
			if ep.Type() == VfioEndpointType {
				hostBDF := ep.(*VfioEndpoint).HostBDF
				pciPath := sandbox.GetVfioDeviceGuestPciPath(hostBDF)
				if pciPath.IsNil() {
					return fmt.Errorf("PCI path for VFIO interface '%s' not found", ep.Name())
				}
				ep.SetPciPath(pciPath)
				endpoints = append(endpoints, ep)
			}
		}

		defer func() {
			if err == nil {
				sandbox.hotplugNetworkConfigApplied = true
			}
		}()
	}

	if len(endpoints) == 0 {
		return nil
	}

	interfaces, routes, neighs, err := generateVCNetworkStructures(ctx, endpoints)
	if err != nil {
		return err
	}

	if err = k.updateInterfaces(ctx, interfaces); err != nil {
		return err
	}
	if _, err = k.updateRoutes(ctx, routes); err != nil {
		return err
	}
	if err = k.addARPNeighbors(ctx, neighs); err != nil {
		return err
	}

	return nil
}

func (k *kataAgent) createContainer(ctx context.Context, sandbox *Sandbox, c *Container) (p *Process, err error) {
	span, ctx := katatrace.Trace(ctx, k.Logger(), "createContainer", kataAgentTracingTags)
	defer span.End()
	var ctrStorages []*grpc.Storage
	var ctrDevices []*grpc.Device
	var sharedRootfs *SharedFile

	// In case the container creation fails, the following defer statement
	// takes care of rolling back actions previously performed.
	defer func() {
		if err != nil {
			k.Logger().WithError(err).Error("createContainer failed")
			k.rollbackFailingContainerCreation(ctx, c)
		}
	}()

	// Share the container rootfs -- if its block based, we'll receive a non-nil storage object representing
	// the block device for the rootfs, which us utilized for mounting in the guest. This'll be handled
	// already for non-block based rootfs
	if sharedRootfs, err = sandbox.fsShare.ShareRootFilesystem(ctx, c); err != nil {
		return nil, err
	}

	if sharedRootfs.containerStorages != nil {
		// Add rootfs to the list of container storage.
		ctrStorages = append(ctrStorages, sharedRootfs.containerStorages...)
	}

	if sharedRootfs.volumeStorages != nil {
		// Add volumeStorages to the list of container storage.
		// We only need to do this for KataVirtualVolume based rootfs, as we
		// want the agent to mount it into the right location

		ctrStorages = append(ctrStorages, sharedRootfs.volumeStorages...)
	}

	ociSpec := c.GetPatchedOCISpec()
	if ociSpec == nil {
		return nil, errorMissingOCISpec
	}

	// Handle container mounts
	sharedDirMounts := make(map[string]Mount)
	ignoredMounts := make(map[string]Mount)

	shareStorages, err := c.mountSharedDirMounts(ctx, sharedDirMounts, ignoredMounts)
	if err != nil {
		return nil, err
	}
	ctrStorages = append(ctrStorages, shareStorages...)

	k.handleShm(ociSpec.Mounts, sandbox)

	epheStorages, err := k.handleEphemeralStorage(ociSpec.Mounts)
	if err != nil {
		return nil, err
	}

	ctrStorages = append(ctrStorages, epheStorages...)

	k.Logger().WithField("ociSpec Hugepage Resources", ociSpec.Linux.Resources.HugepageLimits).Debug("ociSpec HugepageLimit")
	hugepages, err := k.handleHugepages(ociSpec.Mounts, ociSpec.Linux.Resources.HugepageLimits)
	if err != nil {
		return nil, err
	}
	ctrStorages = append(ctrStorages, hugepages...)

	localStorages, err := k.handleLocalStorage(ociSpec.Mounts, sandbox.id, c.rootfsSuffix)
	if err != nil {
		return nil, err
	}

	ctrStorages = append(ctrStorages, localStorages...)

	// We replace all OCI mount sources that match our container mount
	// with the right source path (The guest one).
	if err = k.replaceOCIMountSource(ociSpec, sharedDirMounts); err != nil {
		return nil, err
	}

	// Remove all mounts that should be ignored from the spec
	if err = k.removeIgnoredOCIMount(ociSpec, ignoredMounts); err != nil {
		return nil, err
	}

	// Append container devices for block devices passed with --device.
	ctrDevices = k.appendDevices(ctrDevices, c)

	// Block based volumes will require some adjustments in the OCI spec, and creation of
	// storage objects to pass to the agent.
	layerStorages, volumeStorages, err := k.handleBlkOCIMounts(c, ociSpec)
	if err != nil {
		return nil, err
	}

	ctrStorages = append(ctrStorages, volumeStorages...)

	// Layer storage objects are prepended to the list so that they come _before_ the
	// rootfs because the rootfs depends on them (it's an overlay of the layers).
	ctrStorages = append(layerStorages, ctrStorages...)

	grpcSpec, err := grpc.OCItoGRPC(ociSpec)
	if err != nil {
		return nil, err
	}

	// We need to give the OCI spec our absolute rootfs path in the guest.
	grpcSpec.Root.Path = sharedRootfs.guestPath

	sharedPidNs := k.handlePidNamespace(grpcSpec, sandbox)

	if !sandbox.config.DisableGuestSeccomp && !sandbox.seccompSupported {
		return nil, fmt.Errorf("Seccomp profiles are passed to the virtual machine, but the Kata agent does not support seccomp")
	}

	passSeccomp := !sandbox.config.DisableGuestSeccomp && sandbox.seccompSupported

	// Currently, guest SELinux can be enabled only when SELinux is enabled on the host side.
	if !sandbox.config.HypervisorConfig.DisableGuestSeLinux && !selinux.GetEnabled() {
		return nil, fmt.Errorf("Guest SELinux is enabled, but SELinux is disabled on the host side")
	}
	if sandbox.config.HypervisorConfig.DisableGuestSeLinux && sandbox.config.GuestSeLinuxLabel != "" {
		return nil, fmt.Errorf("Custom SELinux security policy is provided, but guest SELinux is disabled")
	}

	// We need to constrain the spec to make sure we're not
	// passing irrelevant information to the agent.
	err = k.constrainGRPCSpec(grpcSpec, passSeccomp, sandbox.config.HypervisorConfig.DisableGuestSeLinux, sandbox.config.GuestSeLinuxLabel, sandbox.config.VfioMode == config.VFIOModeGuestKernel)
	if err != nil {
		return nil, err
	}

	req := &grpc.CreateContainerRequest{
		ContainerId:  c.id,
		ExecId:       c.id,
		Storages:     ctrStorages,
		Devices:      ctrDevices,
		OCI:          grpcSpec,
		SandboxPidns: sharedPidNs,
	}

	if _, err = k.sendReq(ctx, req); err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return nil, status.Errorf(codes.DeadlineExceeded, "CreateContainerRequest timed out")
		}
		return nil, err
	}

	if err = k.setupNetworks(ctx, sandbox, c); err != nil {
		return nil, err
	}

	return buildProcessFromExecID(req.ExecId)
}

func buildProcessFromExecID(token string) (*Process, error) {
	return &Process{
		Token:     token,
		StartTime: time.Now().UTC(),
		Pid:       -1,
	}, nil
}

// handleHugePages handles hugepages storage by
// creating a Storage from corresponding source of the mount point
func (k *kataAgent) handleHugepages(mounts []specs.Mount, hugepageLimits []specs.LinuxHugepageLimit) ([]*grpc.Storage, error) {
	//Map to hold the total memory of each type of hugepages
	optionsMap := make(map[int64]string)

	for _, hp := range hugepageLimits {
		if hp.Limit != 0 {
			k.Logger().WithFields(logrus.Fields{
				"Pagesize": hp.Pagesize,
				"Limit":    hp.Limit,
			}).Info("hugepage request")
			//example Pagesize 2MB, 1GB etc. The Limit are in Bytes
			pageSize, err := units.RAMInBytes(hp.Pagesize)
			if err != nil {
				k.Logger().Error("Unable to convert pagesize to bytes")
				return nil, err
			}
			totalHpSizeStr := strconv.FormatUint(hp.Limit, 10)
			optionsMap[pageSize] = totalHpSizeStr
		}
	}

	var hugepages []*grpc.Storage
	for idx, mnt := range mounts {
		if mnt.Type != KataLocalDevType {
			continue
		}
		//HugePages mount Type is Local
		if _, fsType, fsOptions, _ := utils.GetDevicePathAndFsTypeOptions(mnt.Source); fsType == "hugetlbfs" {
			k.Logger().WithField("fsOptions", fsOptions).Debug("hugepage mount options")
			//Find the pagesize from the mountpoint options
			pagesizeOpt := getPagesizeFromOpt(fsOptions)
			if pagesizeOpt == "" {
				return nil, fmt.Errorf("No pagesize option found in filesystem mount options")
			}
			pageSize, err := units.RAMInBytes(pagesizeOpt)
			if err != nil {
				k.Logger().Error("Unable to convert pagesize from fs mount options to bytes")
				return nil, err
			}
			//Create mount option string
			options := fmt.Sprintf("pagesize=%s,size=%s", strconv.FormatInt(pageSize, 10), optionsMap[pageSize])
			k.Logger().WithField("Hugepage options string", options).Debug("hugepage mount options")
			// Set the mount source path to a path that resides inside the VM
			mounts[idx].Source = filepath.Join(ephemeralPath(), filepath.Base(mnt.Source))
			// Set the mount type to "bind"
			mounts[idx].Type = "bind"

			// Create a storage struct so that kata agent is able to create
			// hugetlbfs backed volume inside the VM
			hugepage := &grpc.Storage{
				Driver:     KataEphemeralDevType,
				Source:     "nodev",
				Fstype:     "hugetlbfs",
				MountPoint: mounts[idx].Source,
				Options:    []string{options},
			}
			hugepages = append(hugepages, hugepage)
		}

	}
	return hugepages, nil
}

// handleEphemeralStorage handles ephemeral storages by
// creating a Storage from corresponding source of the mount point
func (k *kataAgent) handleEphemeralStorage(mounts []specs.Mount) ([]*grpc.Storage, error) {
	var epheStorages []*grpc.Storage
	for idx, mnt := range mounts {
		if mnt.Type == KataEphemeralDevType {
			origin_src := mounts[idx].Source
			stat := syscall.Stat_t{}
			err := syscall.Stat(origin_src, &stat)
			if err != nil {
				k.Logger().WithError(err).Errorf("failed to stat %s", origin_src)
				return nil, err
			}

			var dir_options []string

			// if volume's gid isn't root group(default group), this means there's
			// an specific fsGroup is set on this local volume, then it should pass
			// to guest.
			if stat.Gid != 0 {
				dir_options = append(dir_options, fmt.Sprintf("%s=%d", fsGid, stat.Gid))
			}

			// Set the mount source path to a path that resides inside the VM
			mounts[idx].Source = filepath.Join(ephemeralPath(), filepath.Base(mnt.Source))
			// Set the mount type to "bind"
			mounts[idx].Type = "bind"

			// Create a storage struct so that kata agent is able to create
			// tmpfs backed volume inside the VM
			epheStorage := &grpc.Storage{
				Driver:     KataEphemeralDevType,
				Source:     "tmpfs",
				Fstype:     "tmpfs",
				MountPoint: mounts[idx].Source,
				Options:    dir_options,
			}
			epheStorages = append(epheStorages, epheStorage)
		}
	}
	return epheStorages, nil
}

// handleLocalStorage handles local storage within the VM
// by creating a directory in the VM from the source of the mount point.
func (k *kataAgent) handleLocalStorage(mounts []specs.Mount, sandboxID string, rootfsSuffix string) ([]*grpc.Storage, error) {
	var localStorages []*grpc.Storage
	for idx, mnt := range mounts {
		if mnt.Type == KataLocalDevType {
			origin_src := mounts[idx].Source
			stat := syscall.Stat_t{}
			err := syscall.Stat(origin_src, &stat)
			if err != nil {
				k.Logger().WithError(err).Errorf("failed to stat %s", origin_src)
				return nil, err
			}

			dir_options := localDirOptions

			// if volume's gid isn't root group(default group), this means there's
			// an specific fsGroup is set on this local volume, then it should pass
			// to guest.
			if stat.Gid != 0 {
				dir_options = append(dir_options, fmt.Sprintf("%s=%d", fsGid, stat.Gid))
			}

			// Set the mount source path to a the desired directory point in the VM.
			// In this case it is located in the sandbox directory.
			// We rely on the fact that the first container in the VM has the same ID as the sandbox ID.
			// In Kubernetes, this is usually the pause container and we depend on it existing for
			// local directories to work.
			mounts[idx].Source = filepath.Join(kataGuestSharedDir(), sandboxID, rootfsSuffix, KataLocalDevType, filepath.Base(mnt.Source))

			// Create a storage struct so that the kata agent is able to create the
			// directory inside the VM.
			localStorage := &grpc.Storage{
				Driver:     KataLocalDevType,
				Source:     KataLocalDevType,
				Fstype:     KataLocalDevType,
				MountPoint: mounts[idx].Source,
				Options:    dir_options,
			}
			localStorages = append(localStorages, localStorage)
		}
	}
	return localStorages, nil
}

func handleBlockVolume(c *Container, device api.Device) (*grpc.Storage, error) {
	vol := &grpc.Storage{}

	blockDrive, ok := device.GetDeviceInfo().(*config.BlockDrive)
	if !ok || blockDrive == nil {
		return nil, fmt.Errorf("malformed block drive")
	}
	switch {
	// pmem volumes case
	case blockDrive.Pmem:
		vol.Driver = kataNvdimmDevType
		vol.Source = fmt.Sprintf("/dev/pmem%s", blockDrive.NvdimmID)
		vol.Fstype = blockDrive.Format
		vol.Options = []string{"dax"}
	case c.sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioBlockCCW:
		vol.Driver = kataBlkCCWDevType
		vol.Source = blockDrive.DevNo
	case c.sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioBlock:
		vol.Driver = kataBlkDevType
		vol.Source = blockDrive.PCIPath.String()
	case c.sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioMmio:
		vol.Driver = kataMmioBlkDevType
		vol.Source = blockDrive.VirtPath
	case c.sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioSCSI:
		vol.Driver = kataSCSIDevType
		vol.Source = blockDrive.SCSIAddr
	default:
		return nil, fmt.Errorf("Unknown block device driver: %s", c.sandbox.config.HypervisorConfig.BlockDeviceDriver)
	}
	return vol, nil
}

// getContainerTypeforCRI get container type from different CRI annotations
func getContainerTypeforCRI(c *Container) (string, string) {

	// CRIContainerTypeKeyList lists all the CRI keys that could define
	// the container type from annotations in the config.json.
	CRIContainerTypeKeyList := []string{ctrAnnotations.ContainerType, podmanAnnotations.ContainerType}
	containerType := c.config.Annotations[vcAnnotations.ContainerTypeKey]
	for _, key := range CRIContainerTypeKeyList {
		_, ok := c.config.CustomSpec.Annotations[key]
		if ok {
			return containerType, key
		}
	}
	return "", ""
}

func handleImageGuestPullBlockVolume(c *Container, virtualVolumeInfo *types.KataVirtualVolume, vol *grpc.Storage) (*grpc.Storage, error) {
	container_annotations := c.GetAnnotations()
	containerType, criContainerType := getContainerTypeforCRI(c)

	var image_ref string
	if containerType == string(PodSandbox) {
		image_ref = "pause"
	} else {
		const kubernetesCRIImageName = "io.kubernetes.cri.image-name"
		const kubernetesCRIOImageName = "io.kubernetes.cri-o.ImageName"

		switch criContainerType {
		case ctrAnnotations.ContainerType:
			image_ref = container_annotations[kubernetesCRIImageName]
		case podmanAnnotations.ContainerType:
			image_ref = container_annotations[kubernetesCRIOImageName]
		default:
			// There are cases, like when using nerdctl, where the criContainerType
			// will never be set, leading to this code path.
			//
			// nerdctl also doesn't set any mechanism for automatically setting the
			// image, but as part of it's v2.0.0 release it allows the user to set
			// any kind of OCI annotation, which we can take advantage of and use.
			//
			// With this in mind, let's "fallback" to the default k8s cri image-name
			// annotation, as documented on our image-pull documentation.
			image_ref = container_annotations[kubernetesCRIImageName]
		}

		if image_ref == "" {
			return nil, fmt.Errorf("Failed to get image name from annotations")
		}
	}
	virtualVolumeInfo.Source = image_ref

	//merge virtualVolumeInfo.ImagePull.Metadata and container_annotations
	for k, v := range container_annotations {
		virtualVolumeInfo.ImagePull.Metadata[k] = v
	}

	no, err := json.Marshal(virtualVolumeInfo.ImagePull)
	if err != nil {
		return nil, err
	}
	vol.Driver = types.KataVirtualVolumeImageGuestPullType
	vol.DriverOptions = append(vol.DriverOptions, types.KataVirtualVolumeImageGuestPullType+"="+string(no))
	vol.Source = virtualVolumeInfo.Source
	vol.Fstype = typeOverlayFS
	return vol, nil
}

// handleVirtualVolumeStorageObject handles KataVirtualVolume that is block device file.
func handleVirtualVolumeStorageObject(c *Container, blockDeviceId string, virtVolume *types.KataVirtualVolume) (*grpc.Storage, error) {
	var vol *grpc.Storage
	if virtVolume.VolumeType == types.KataVirtualVolumeImageGuestPullType {
		var err error
		vol = &grpc.Storage{}
		vol, err = handleImageGuestPullBlockVolume(c, virtVolume, vol)
		if err != nil {
			return nil, err
		}
		vol.MountPoint = filepath.Join("/run/kata-containers/", c.id, c.rootfsSuffix)
	}
	return vol, nil
}

// handleDeviceBlockVolume handles volume that is block device file
// and DeviceBlock type.
func (k *kataAgent) handleDeviceBlockVolume(c *Container, m Mount, device api.Device) (*grpc.Storage, error) {
	vol, err := handleBlockVolume(c, device)
	if err != nil {
		return nil, err
	}

	vol.MountPoint = m.Destination

	// If no explicit FS Type or Options are being set, then let's use what is provided for the particular mount:
	if vol.Fstype == "" {
		vol.Fstype = m.Type
	}
	if len(vol.Options) == 0 {
		vol.Options = m.Options
	}
	if m.FSGroup != nil {
		vol.FsGroup = &grpc.FSGroup{
			GroupId:           uint32(*m.FSGroup),
			GroupChangePolicy: getFSGroupChangePolicy(m.FSGroupChangePolicy),
		}
	}

	return vol, nil
}

// handleVhostUserBlkVolume handles volume that is block device file
// and VhostUserBlk type.
func (k *kataAgent) handleVhostUserBlkVolume(c *Container, m Mount, device api.Device) (*grpc.Storage, error) {
	vol := &grpc.Storage{}

	d, ok := device.GetDeviceInfo().(*config.VhostUserDeviceAttrs)
	if !ok || d == nil {
		k.Logger().Error("malformed vhost-user blk drive")
		return nil, fmt.Errorf("malformed vhost-user blk drive")
	}

	vol.Driver = kataBlkDevType
	vol.Source = d.PCIPath.String()
	vol.Fstype = "bind"
	vol.Options = []string{"bind"}
	vol.MountPoint = m.Destination

	// Assign the type from the mount, if it's specified (e.g. direct assigned volume)
	if m.Type != "" {
		vol.Fstype = m.Type
		vol.Options = m.Options
	}

	return vol, nil
}

func (k *kataAgent) createBlkStorageObject(c *Container, m Mount) (*grpc.Storage, error) {
	var vol *grpc.Storage

	id := m.BlockDeviceID
	device := c.sandbox.devManager.GetDeviceByID(id)
	if device == nil {
		k.Logger().WithField("device", id).Error("failed to find device by id")
		return nil, fmt.Errorf("Failed to find device by id (id=%s)", id)
	}

	var err error
	switch device.DeviceType() {
	case config.DeviceBlock:
		vol, err = k.handleDeviceBlockVolume(c, m, device)
	case config.VhostUserBlk:
		vol, err = k.handleVhostUserBlkVolume(c, m, device)
	default:
		return nil, fmt.Errorf("Unknown device type")
	}

	return vol, err
}

// handleBlkOCIMounts will create a unique destination mountpoint in the guest for each volume in the
// given container and will update the OCI spec to utilize this mount point as the new source for the
// container volume. The container mount structure is updated to store the guest destination mountpoint.
func (k *kataAgent) handleBlkOCIMounts(c *Container, spec *specs.Spec) ([]*grpc.Storage, []*grpc.Storage, error) {

	var volumeStorages []*grpc.Storage
	var layerStorages []*grpc.Storage

	for i, m := range c.mounts {
		id := m.BlockDeviceID

		if len(id) == 0 {
			continue
		}

		// Add the block device to the list of container devices, to make sure the
		// device is detached with detachDevices() for a container.
		c.devices = append(c.devices, ContainerDevice{ID: id, ContainerPath: m.Destination})

		// Create Storage structure
		vol, err := k.createBlkStorageObject(c, m)
		if vol == nil || err != nil {
			return nil, nil, err
		}

		if HasOption(m.Options, vcAnnotations.IsFileSystemLayer) {
			layerStorages = append(layerStorages, vol)
			continue
		}

		// Each device will be mounted at a unique location within the VM only once. Mounting
		// to the container specific location is handled within the OCI spec. Let's ensure that
		// the storage mount point is unique for each device. This is then utilized as the source
		// in the OCI spec. If multiple containers mount the same block device, it's ref-counted inside
		// the guest by Kata agent.
		filename := b64.URLEncoding.EncodeToString([]byte(vol.Source))
		path := filepath.Join(kataGuestSandboxStorageDir(), filename)

		// Update applicable OCI mount source
		for idx, ociMount := range spec.Mounts {
			if ociMount.Destination != vol.MountPoint {
				continue
			}
			k.Logger().WithFields(logrus.Fields{
				"original-source": ociMount.Source,
				"new-source":      path,
			}).Debug("Replacing OCI mount source")
			spec.Mounts[idx].Source = path
			if HasOption(spec.Mounts[idx].Options, vcAnnotations.IsFileBlockDevice) {
				// The device is already mounted, just bind to path in container.
				spec.Mounts[idx].Options = []string{"bind"}
			}
			break
		}

		// Update storage mountpoint, and save guest device mount path to container mount struct:
		vol.MountPoint = path
		c.mounts[i].GuestDeviceMount = path

		volumeStorages = append(volumeStorages, vol)
	}

	return layerStorages, volumeStorages, nil
}

// handlePidNamespace checks if Pid namespace for a container needs to be shared with its sandbox
// pid namespace. This function also modifies the grpc spec to remove the pid namespace
// from the list of namespaces passed to the agent.
func (k *kataAgent) handlePidNamespace(grpcSpec *grpc.Spec, sandbox *Sandbox) bool {
	sharedPidNs := false
	pidIndex := -1

	for i, ns := range grpcSpec.Linux.Namespaces {
		if ns.Type != string(specs.PIDNamespace) {
			continue
		}

		pidIndex = i
		// host pidns path does not make sense in kata. Let's just align it with
		// sandbox namespace whenever it is set.
		if ns.Path != "" {
			sharedPidNs = true
		}
		break
	}

	// Remove pid namespace.
	if pidIndex >= 0 {
		grpcSpec.Linux.Namespaces = append(grpcSpec.Linux.Namespaces[:pidIndex], grpcSpec.Linux.Namespaces[pidIndex+1:]...)
	}

	return sharedPidNs
}

func (k *kataAgent) startContainer(ctx context.Context, sandbox *Sandbox, c *Container) error {
	span, ctx := katatrace.Trace(ctx, k.Logger(), "startContainer", kataAgentTracingTags)
	defer span.End()

	req := &grpc.StartContainerRequest{
		ContainerId: c.id,
	}

	_, err := k.sendReq(ctx, req)
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "StartContainerRequest timed out")
	}
	return err
}

func (k *kataAgent) stopContainer(ctx context.Context, sandbox *Sandbox, c Container) error {
	span, ctx := katatrace.Trace(ctx, k.Logger(), "stopContainer", kataAgentTracingTags)
	defer span.End()

	_, err := k.sendReq(ctx, &grpc.RemoveContainerRequest{ContainerId: c.id})
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "RemoveContainerRequest timed out")
	}
	return err
}

func (k *kataAgent) signalProcess(ctx context.Context, c *Container, processID string, signal syscall.Signal, all bool) error {
	execID := processID
	if all {
		// kata agent uses empty execId to signal all processes in a container
		execID = ""
	}
	req := &grpc.SignalProcessRequest{
		ContainerId: c.id,
		ExecId:      execID,
		Signal:      uint32(signal),
	}

	_, err := k.sendReq(ctx, req)
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "SignalProcessRequest timed out")
	}
	return err
}

func (k *kataAgent) winsizeProcess(ctx context.Context, c *Container, processID string, height, width uint32) error {
	req := &grpc.TtyWinResizeRequest{
		ContainerId: c.id,
		ExecId:      processID,
		Row:         height,
		Column:      width,
	}

	_, err := k.sendReq(ctx, req)
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "TtyWinResizeRequest timed out")
	}
	return err
}

func (k *kataAgent) updateContainer(ctx context.Context, sandbox *Sandbox, c Container, resources specs.LinuxResources) error {
	grpcResources, err := grpc.ResourcesOCItoGRPC(&resources)
	if err != nil {
		return err
	}

	req := &grpc.UpdateContainerRequest{
		ContainerId: c.id,
		Resources:   grpcResources,
	}

	_, err = k.sendReq(ctx, req)
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "UpdateContainerRequest timed out")
	}
	return err
}

func (k *kataAgent) pauseContainer(ctx context.Context, sandbox *Sandbox, c Container) error {
	req := &grpc.PauseContainerRequest{
		ContainerId: c.id,
	}

	_, err := k.sendReq(ctx, req)
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "PauseContainerRequest timed out")
	}
	return err
}

func (k *kataAgent) resumeContainer(ctx context.Context, sandbox *Sandbox, c Container) error {
	req := &grpc.ResumeContainerRequest{
		ContainerId: c.id,
	}

	_, err := k.sendReq(ctx, req)
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "ResumeContainerRequest timed out")
	}
	return err
}

func (k *kataAgent) memHotplugByProbe(ctx context.Context, addr uint64, sizeMB uint32, memorySectionSizeMB uint32) error {
	if memorySectionSizeMB == uint32(0) {
		return fmt.Errorf("memorySectionSizeMB couldn't be zero")
	}
	// hot-added memory device should be sliced into the size of memory section, which is the basic unit for
	// memory hotplug
	numSection := uint64(sizeMB / memorySectionSizeMB)
	var addrList []uint64
	index := uint64(0)
	for index < numSection {
		k.Logger().WithFields(logrus.Fields{
			"addr": fmt.Sprintf("0x%x", addr+(index*uint64(memorySectionSizeMB))<<20),
		}).Debugf("notify guest kernel the address of memory device")
		addrList = append(addrList, addr+(index*uint64(memorySectionSizeMB))<<20)
		index++
	}
	req := &grpc.MemHotplugByProbeRequest{
		MemHotplugProbeAddr: addrList,
	}

	_, err := k.sendReq(ctx, req)
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "MemHotplugByProbeRequest timed out")
	}
	return err
}

func (k *kataAgent) onlineCPUMem(ctx context.Context, cpus uint32, cpuOnly bool) error {
	req := &grpc.OnlineCPUMemRequest{
		Wait:    false,
		NbCpus:  cpus,
		CpuOnly: cpuOnly,
	}

	_, err := k.sendReq(ctx, req)
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "OnlineCPUMemRequest timed out")
	}
	return err
}

func (k *kataAgent) statsContainer(ctx context.Context, sandbox *Sandbox, c Container) (*ContainerStats, error) {
	req := &grpc.StatsContainerRequest{
		ContainerId: c.id,
	}

	returnStats, err := k.sendReq(ctx, req)

	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return nil, status.Errorf(codes.DeadlineExceeded, "StatsContainerRequest timed out")
		}
		return nil, err
	}

	stats, ok := returnStats.(*grpc.StatsContainerResponse)
	if !ok {
		return nil, fmt.Errorf("irregular response container stats")
	}

	data, err := json.Marshal(stats.CgroupStats)
	if err != nil {
		return nil, err
	}

	var cgroupStats CgroupStats
	err = json.Unmarshal(data, &cgroupStats)
	if err != nil {
		return nil, err
	}
	containerStats := &ContainerStats{
		CgroupStats: &cgroupStats,
	}
	return containerStats, nil
}

func (k *kataAgent) connect(ctx context.Context) error {
	if k.dead {
		return errors.New("Dead agent")
	}
	// lockless quick pass
	if k.client != nil {
		return nil
	}

	span, _ := katatrace.Trace(ctx, k.Logger(), "connect", kataAgentTracingTags)
	defer span.End()

	// This is for the first connection only, to prevent race
	k.Lock()
	defer k.Unlock()
	if k.client != nil {
		return nil
	}

	k.Logger().WithField("url", k.state.URL).Info("New client")
	client, err := kataclient.NewAgentClient(k.ctx, k.state.URL, k.dialTimout)
	if err != nil {
		k.dead = true
		return err
	}

	k.installReqFunc(client)
	k.client = client

	return nil
}

func (k *kataAgent) disconnect(ctx context.Context) error {
	span, _ := katatrace.Trace(ctx, k.Logger(), "Disconnect", kataAgentTracingTags)
	defer span.End()

	k.Lock()
	defer k.Unlock()

	if k.client == nil {
		return nil
	}

	if err := k.client.Close(); err != nil && grpcStatus.Convert(err).Code() != codes.Canceled {
		return err
	}

	k.client = nil
	k.reqHandlers = nil

	return nil
}

// check grpc server is serving
func (k *kataAgent) check(ctx context.Context) error {
	_, err := k.sendReq(ctx, &grpc.CheckRequest{})
	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return status.Errorf(codes.DeadlineExceeded, "CheckRequest timed out")
		}
		err = fmt.Errorf("Failed to Check if grpc server is working: %s", err)
	}
	return err
}

func (k *kataAgent) waitProcess(ctx context.Context, c *Container, processID string) (int32, error) {
	span, ctx := katatrace.Trace(ctx, k.Logger(), "waitProcess", kataAgentTracingTags)
	defer span.End()

	resp, err := k.sendReq(ctx, &grpc.WaitProcessRequest{
		ContainerId: c.id,
		ExecId:      processID,
	})
	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return 0, status.Errorf(codes.DeadlineExceeded, "WaitProcessRequest timed out")
		}
		return 0, err
	}

	return resp.(*grpc.WaitProcessResponse).Status, nil
}

func (k *kataAgent) writeProcessStdin(ctx context.Context, c *Container, ProcessID string, data []byte) (int, error) {
	resp, err := k.sendReq(ctx, &grpc.WriteStreamRequest{
		ContainerId: c.id,
		ExecId:      ProcessID,
		Data:        data,
	})

	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return 0, status.Errorf(codes.DeadlineExceeded, "WriteStreamRequest timed out")
		}
		return 0, err
	}

	return int(resp.(*grpc.WriteStreamResponse).Len), nil
}

func (k *kataAgent) closeProcessStdin(ctx context.Context, c *Container, ProcessID string) error {
	_, err := k.sendReq(ctx, &grpc.CloseStdinRequest{
		ContainerId: c.id,
		ExecId:      ProcessID,
	})
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "CloseStdinRequest timed out")
	}
	return err
}

func (k *kataAgent) reseedRNG(ctx context.Context, data []byte) error {
	_, err := k.sendReq(ctx, &grpc.ReseedRandomDevRequest{
		Data: data,
	})
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "ReseedRandomDevRequest timed out")
	}
	return err
}

func (k *kataAgent) removeStaleVirtiofsShareMounts(ctx context.Context) error {
	_, err := k.sendReq(ctx, &grpc.RemoveStaleVirtiofsShareMountsRequest{})
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "removeStaleVirtiofsShareMounts timed out")
	}
	return err
}

type reqFunc func(context.Context, interface{}) (interface{}, error)

func (k *kataAgent) installReqFunc(c *kataclient.AgentClient) {
	k.reqHandlers = make(map[string]reqFunc)
	k.reqHandlers[grpcCheckRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.HealthClient.Check(ctx, req.(*grpc.CheckRequest))
	}
	k.reqHandlers[grpcExecProcessRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.ExecProcess(ctx, req.(*grpc.ExecProcessRequest))
	}
	k.reqHandlers[grpcCreateSandboxRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.CreateSandbox(ctx, req.(*grpc.CreateSandboxRequest))
	}
	k.reqHandlers[grpcDestroySandboxRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.DestroySandbox(ctx, req.(*grpc.DestroySandboxRequest))
	}
	k.reqHandlers[grpcCreateContainerRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.CreateContainer(ctx, req.(*grpc.CreateContainerRequest))
	}
	k.reqHandlers[grpcStartContainerRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.StartContainer(ctx, req.(*grpc.StartContainerRequest))
	}
	k.reqHandlers[grpcRemoveContainerRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.RemoveContainer(ctx, req.(*grpc.RemoveContainerRequest))
	}
	k.reqHandlers[grpcSignalProcessRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.SignalProcess(ctx, req.(*grpc.SignalProcessRequest))
	}
	k.reqHandlers[grpcUpdateRoutesRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.UpdateRoutes(ctx, req.(*grpc.UpdateRoutesRequest))
	}
	k.reqHandlers[grpcUpdateInterfaceRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.UpdateInterface(ctx, req.(*grpc.UpdateInterfaceRequest))
	}
	k.reqHandlers[grpcUpdateEphemeralMountsRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.UpdateEphemeralMounts(ctx, req.(*grpc.UpdateEphemeralMountsRequest))
	}
	k.reqHandlers[grpcListInterfacesRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.ListInterfaces(ctx, req.(*grpc.ListInterfacesRequest))
	}
	k.reqHandlers[grpcListRoutesRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.ListRoutes(ctx, req.(*grpc.ListRoutesRequest))
	}
	k.reqHandlers[grpcAddARPNeighborsRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.AddARPNeighbors(ctx, req.(*grpc.AddARPNeighborsRequest))
	}
	k.reqHandlers[grpcOnlineCPUMemRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.OnlineCPUMem(ctx, req.(*grpc.OnlineCPUMemRequest))
	}
	k.reqHandlers[grpcUpdateContainerRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.UpdateContainer(ctx, req.(*grpc.UpdateContainerRequest))
	}
	k.reqHandlers[grpcWaitProcessRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.WaitProcess(ctx, req.(*grpc.WaitProcessRequest))
	}
	k.reqHandlers[grpcTtyWinResizeRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.TtyWinResize(ctx, req.(*grpc.TtyWinResizeRequest))
	}
	k.reqHandlers[grpcWriteStreamRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.WriteStdin(ctx, req.(*grpc.WriteStreamRequest))
	}
	k.reqHandlers[grpcCloseStdinRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.CloseStdin(ctx, req.(*grpc.CloseStdinRequest))
	}
	k.reqHandlers[grpcStatsContainerRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.StatsContainer(ctx, req.(*grpc.StatsContainerRequest))
	}
	k.reqHandlers[grpcPauseContainerRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.PauseContainer(ctx, req.(*grpc.PauseContainerRequest))
	}
	k.reqHandlers[grpcResumeContainerRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.ResumeContainer(ctx, req.(*grpc.ResumeContainerRequest))
	}
	k.reqHandlers[grpcReseedRandomDevRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.ReseedRandomDev(ctx, req.(*grpc.ReseedRandomDevRequest))
	}
	k.reqHandlers[grpcGuestDetailsRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.GetGuestDetails(ctx, req.(*grpc.GuestDetailsRequest))
	}
	k.reqHandlers[grpcMemHotplugByProbeRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.MemHotplugByProbe(ctx, req.(*grpc.MemHotplugByProbeRequest))
	}
	k.reqHandlers[grpcCopyFileRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.CopyFile(ctx, req.(*grpc.CopyFileRequest))
	}
	k.reqHandlers[grpcSetGuestDateTimeRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.SetGuestDateTime(ctx, req.(*grpc.SetGuestDateTimeRequest))
	}
	k.reqHandlers[grpcGetOOMEventRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.GetOOMEvent(ctx, req.(*grpc.GetOOMEventRequest))
	}
	k.reqHandlers[grpcGetMetricsRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.GetMetrics(ctx, req.(*grpc.GetMetricsRequest))
	}
	k.reqHandlers[grpcAddSwapRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.AddSwap(ctx, req.(*grpc.AddSwapRequest))
	}
	k.reqHandlers[grpcVolumeStatsRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.GetVolumeStats(ctx, req.(*grpc.VolumeStatsRequest))
	}
	k.reqHandlers[grpcResizeVolumeRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.ResizeVolume(ctx, req.(*grpc.ResizeVolumeRequest))
	}
	k.reqHandlers[grpcGetIPTablesRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.GetIPTables(ctx, req.(*grpc.GetIPTablesRequest))
	}
	k.reqHandlers[grpcSetIPTablesRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.SetIPTables(ctx, req.(*grpc.SetIPTablesRequest))
	}
	k.reqHandlers[grpcRemoveStaleVirtiofsShareMountsRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.RemoveStaleVirtiofsShareMounts(ctx, req.(*grpc.RemoveStaleVirtiofsShareMountsRequest))
	}
	k.reqHandlers[grpcSetPolicyRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.SetPolicy(ctx, req.(*grpc.SetPolicyRequest))
	}
}

func (k *kataAgent) getReqContext(ctx context.Context, reqName string) (newCtx context.Context, cancel context.CancelFunc) {
	newCtx = ctx
	switch reqName {
	case grpcWaitProcessRequest, grpcGetOOMEventRequest:
		// Wait and GetOOMEvent have no timeout
	case grpcCheckRequest:
		newCtx, cancel = context.WithTimeout(ctx, checkRequestTimeout)
	case grpcCreateContainerRequest:
		newCtx, cancel = context.WithTimeout(ctx, createContainerRequestTimeout)
	default:
		var requestTimeout = defaultRequestTimeout

		if timeout, ok := ctx.Value(customRequestTimeoutKey).(time.Duration); ok {
			requestTimeout = timeout
		}
		newCtx, cancel = context.WithTimeout(ctx, requestTimeout)
	}

	return newCtx, cancel
}

func (k *kataAgent) sendReq(spanCtx context.Context, request interface{}) (interface{}, error) {
	start := time.Now()

	if err := k.connect(spanCtx); err != nil {
		return nil, err
	}
	if !k.keepConn {
		defer k.disconnect(spanCtx)
	}

	msgName := string(proto.MessageName(request.(proto.Message)))

	k.Lock()

	if k.reqHandlers == nil {
		k.Unlock()
		return nil, errors.New("Client has already disconnected")
	}

	handler := k.reqHandlers[msgName]
	if msgName == "" || handler == nil {
		k.Unlock()
		return nil, errors.New("Invalid request type")
	}

	k.Unlock()

	message := request.(proto.Message)
	ctx, cancel := k.getReqContext(spanCtx, msgName)
	if cancel != nil {
		defer cancel()
	}

	jsonStr, err := protojson.Marshal(message)
	if err != nil {
		return nil, err
	}
	k.Logger().WithField("name", msgName).WithField("req", string(jsonStr)).Trace("sending request")

	defer func() {
		agentRPCDurationsHistogram.WithLabelValues(msgName).Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()
	return handler(ctx, request)
}

// readStdout and readStderr are special that we cannot differentiate them with the request types...
func (k *kataAgent) readProcessStdout(ctx context.Context, c *Container, processID string, data []byte) (int, error) {
	if err := k.connect(ctx); err != nil {
		return 0, err
	}
	if !k.keepConn {
		defer k.disconnect(ctx)
	}

	return k.readProcessStream(c.id, processID, data, k.client.AgentServiceClient.ReadStdout)
}

// readStdout and readStderr are special that we cannot differentiate them with the request types...
func (k *kataAgent) readProcessStderr(ctx context.Context, c *Container, processID string, data []byte) (int, error) {
	if err := k.connect(ctx); err != nil {
		return 0, err
	}
	if !k.keepConn {
		defer k.disconnect(ctx)
	}

	return k.readProcessStream(c.id, processID, data, k.client.AgentServiceClient.ReadStderr)
}

type readFn func(context.Context, *grpc.ReadStreamRequest) (*grpc.ReadStreamResponse, error)

func (k *kataAgent) readProcessStream(containerID, processID string, data []byte, read readFn) (int, error) {
	resp, err := read(k.ctx, &grpc.ReadStreamRequest{
		ContainerId: containerID,
		ExecId:      processID,
		Len:         uint32(len(data))})
	if err == nil {
		copy(data, resp.Data)
		return len(resp.Data), nil
	}
	return 0, err
}

func (k *kataAgent) getGuestDetails(ctx context.Context, req *grpc.GuestDetailsRequest) (*grpc.GuestDetailsResponse, error) {
	resp, err := k.sendReq(ctx, req)
	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return nil, status.Errorf(codes.DeadlineExceeded, "GuestDetailsRequest request timed out")
		}
		return nil, err
	}

	return resp.(*grpc.GuestDetailsResponse), nil
}

func (k *kataAgent) setGuestDateTime(ctx context.Context, tv time.Time) error {
	_, err := k.sendReq(ctx, &grpc.SetGuestDateTimeRequest{
		Sec:  tv.Unix(),
		Usec: int64(tv.Nanosecond() / 1e3),
	})
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "SetGuestDateTimeRequest request timed out")
	}
	return err
}

func (k *kataAgent) copyFile(ctx context.Context, src, dst string) error {
	var st unix.Stat_t

	err := unix.Lstat(src, &st)
	if err != nil {
		return fmt.Errorf("Could not get file %s information: %v", src, err)
	}

	cpReq := &grpc.CopyFileRequest{
		Path:     dst,
		DirMode:  uint32(DirMode),
		FileMode: st.Mode,
		Uid:      int32(st.Uid),
		Gid:      int32(st.Gid),
	}

	var b []byte

	switch sflag := st.Mode & unix.S_IFMT; sflag {
	case unix.S_IFREG:
		var err error
		// TODO: Support incremental file copying instead of loading whole file into memory
		b, err = os.ReadFile(src)
		if err != nil {
			return fmt.Errorf("Could not read file %s: %v", src, err)
		}
		cpReq.FileSize = int64(len(b))

	case unix.S_IFDIR:

	case unix.S_IFLNK:
		symlink, err := os.Readlink(src)
		if err != nil {
			return fmt.Errorf("Could not read symlink %s: %v", src, err)
		}
		cpReq.Data = []byte(symlink)

	default:
		return fmt.Errorf("Unsupported file type: %o", sflag)
	}

	k.Logger().WithFields(logrus.Fields{
		"source": src,
		"dest":   dst,
	}).Debugf("Copying file from host to guest")

	// Handle the special case where the file is empty
	if cpReq.FileSize == 0 {
		_, err := k.sendReq(ctx, cpReq)
		if err != nil && err.Error() == context.DeadlineExceeded.Error() {
			return status.Errorf(codes.DeadlineExceeded, "CopyFileRequest timed out")
		}
		return err
	}

	// Copy file by parts if it's needed
	remainingBytes := cpReq.FileSize
	offset := int64(0)
	for remainingBytes > 0 {
		bytesToCopy := int64(len(b))
		if bytesToCopy > grpcMaxDataSize {
			bytesToCopy = grpcMaxDataSize
		}

		cpReq.Data = b[:bytesToCopy]
		cpReq.Offset = offset

		if _, err = k.sendReq(ctx, cpReq); err != nil {
			if err.Error() == context.DeadlineExceeded.Error() {
				return status.Errorf(codes.DeadlineExceeded, "CopyFileRequest timed out")
			}
			return fmt.Errorf("Could not send CopyFile request: %v", err)
		}

		b = b[bytesToCopy:]
		remainingBytes -= bytesToCopy
		offset += grpcMaxDataSize
	}

	return nil
}

func (k *kataAgent) addSwap(ctx context.Context, PCIPath types.PciPath) error {
	span, ctx := katatrace.Trace(ctx, k.Logger(), "addSwap", kataAgentTracingTags)
	defer span.End()

	_, err := k.sendReq(ctx, &grpc.AddSwapRequest{PCIPath: PCIPath.ToArray()})
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "AddSwapRequest timed out")
	}
	return err
}

func (k *kataAgent) markDead(ctx context.Context) {
	k.Logger().Infof("mark agent dead")
	k.dead = true
	k.disconnect(ctx)
}

func (k *kataAgent) cleanup(ctx context.Context) {
}

func (k *kataAgent) save() persistapi.AgentState {
	return persistapi.AgentState{
		URL: k.state.URL,
	}
}

func (k *kataAgent) load(s persistapi.AgentState) {
	k.state.URL = s.URL
}

func (k *kataAgent) getOOMEvent(ctx context.Context) (string, error) {
	req := &grpc.GetOOMEventRequest{}
	result, err := k.sendReq(ctx, req)
	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return "", status.Errorf(codes.DeadlineExceeded, "GetOOMEventRequest timed out")
		}
		return "", err
	}
	if oomEvent, ok := result.(*grpc.OOMEvent); ok {
		return oomEvent.ContainerId, nil
	}
	return "", err
}

func (k *kataAgent) getAgentMetrics(ctx context.Context, req *grpc.GetMetricsRequest) (*grpc.Metrics, error) {
	resp, err := k.sendReq(ctx, req)
	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return nil, status.Errorf(codes.DeadlineExceeded, "GetMetricsRequest timed out")
		}
		return nil, err
	}

	return resp.(*grpc.Metrics), nil
}

func (k *kataAgent) getIPTables(ctx context.Context, isIPv6 bool) ([]byte, error) {
	resp, err := k.sendReq(ctx, &grpc.GetIPTablesRequest{IsIpv6: isIPv6})
	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return nil, status.Errorf(codes.DeadlineExceeded, "GetIPTablesRequest timed out")
		}
		return nil, err
	}
	return resp.(*grpc.GetIPTablesResponse).Data, nil
}

func (k *kataAgent) setIPTables(ctx context.Context, isIPv6 bool, data []byte) error {
	_, err := k.sendReq(ctx, &grpc.SetIPTablesRequest{
		IsIpv6: isIPv6,
		Data:   data,
	})
	if err != nil {
		k.Logger().WithError(err).Errorf("setIPTables request to agent failed")
		if err.Error() == context.DeadlineExceeded.Error() {
			return status.Errorf(codes.DeadlineExceeded, "SetIPTablesRequest timed out")
		}
	}

	return err
}

func (k *kataAgent) getGuestVolumeStats(ctx context.Context, volumeGuestPath string) ([]byte, error) {
	result, err := k.sendReq(ctx, &grpc.VolumeStatsRequest{VolumeGuestPath: volumeGuestPath})
	if err != nil {
		if err.Error() == context.DeadlineExceeded.Error() {
			return nil, status.Errorf(codes.DeadlineExceeded, "VolumeStatsRequest timed out")
		}
		return nil, err
	}

	buf, err := json.Marshal(result.(*grpc.VolumeStatsResponse))
	if err != nil {
		return nil, err
	}

	return buf, nil
}

func (k *kataAgent) resizeGuestVolume(ctx context.Context, volumeGuestPath string, size uint64) error {
	_, err := k.sendReq(ctx, &grpc.ResizeVolumeRequest{VolumeGuestPath: volumeGuestPath, Size: size})
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "ResizeVolumeRequest timed out")
	}
	return err
}

func (k *kataAgent) setPolicy(ctx context.Context, policy string) error {
	_, err := k.sendReq(ctx, &grpc.SetPolicyRequest{Policy: policy})
	if err != nil && err.Error() == context.DeadlineExceeded.Error() {
		return status.Errorf(codes.DeadlineExceeded, "SetPolicyRequest timed out")
	}
	return err
}

// IsNydusRootFSType checks if the given mount type indicates Nydus is used.
// By default, Nydus will use "fuse.nydus-overlayfs" as the mount type, but
// we also accept binaries which have "nydus-overlayfs" prefix, so you can,
// for example, place a nydus-overlayfs-abcde binary in the PATH and use
// "fuse.nydus-overlayfs-abcde" as the mount type.
// Further, we allow passing the full path to a Nydus binary as the mount type,
// so "fuse./usr/local/bin/nydus-overlayfs" is also recognized.
func IsNydusRootFSType(s string) bool {
	if !strings.HasPrefix(s, "fuse.") {
		return false
	}
	s = strings.TrimPrefix(s, "fuse.")
	return strings.HasPrefix(path.Base(s), "nydus-overlayfs")
}
