// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	kataclient "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/client"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	vccgroups "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/cgroups"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/uuid"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/label"
	otelTrace "go.opentelemetry.io/otel/trace"

	"github.com/gogo/protobuf/proto"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"golang.org/x/net/context"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

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

	sandboxMountsDir = "sandbox-mounts"

	// enable debug console
	kernelParamDebugConsole           = "agent.debug_console"
	kernelParamDebugConsoleVPort      = "agent.debug_console_vport"
	kernelParamDebugConsoleVPortValue = "1026"
)

var (
	checkRequestTimeout         = 30 * time.Second
	defaultRequestTimeout       = 60 * time.Second
	errorMissingOCISpec         = errors.New("Missing OCI specification")
	defaultKataHostSharedDir    = "/run/kata-containers/shared/sandboxes/"
	defaultKataGuestSharedDir   = "/run/kata-containers/shared/containers/"
	mountGuestTag               = "kataShared"
	defaultKataGuestSandboxDir  = "/run/kata-containers/sandbox/"
	type9pFs                    = "9p"
	typeVirtioFS                = "virtiofs"
	typeVirtioFSNoCache         = "none"
	kata9pDevType               = "9p"
	kataMmioBlkDevType          = "mmioblk"
	kataBlkDevType              = "blk"
	kataBlkCCWDevType           = "blk-ccw"
	kataSCSIDevType             = "scsi"
	kataNvdimmDevType           = "nvdimm"
	kataVirtioFSDevType         = "virtio-fs"
	kataWatchableBindDevType    = "watchable-bind"
	sharedDir9pOptions          = []string{"trans=virtio,version=9p2000.L,cache=mmap", "nodev"}
	sharedDirVirtioFSOptions    = []string{}
	sharedDirVirtioFSDaxOptions = "dax"
	shmDir                      = "shm"
	kataEphemeralDevType        = "ephemeral"
	defaultEphemeralPath        = filepath.Join(defaultKataGuestSandboxDir, kataEphemeralDevType)
	grpcMaxDataSize             = int64(1024 * 1024)
	localDirOptions             = []string{"mode=0777"}
	maxHostnameLen              = 64
	GuestDNSFile                = "/etc/resolv.conf"
)

const (
	agentTraceModeDynamic  = "dynamic"
	agentTraceModeStatic   = "static"
	agentTraceTypeIsolated = "isolated"
	agentTraceTypeCollated = "collated"

	defaultAgentTraceMode = agentTraceModeDynamic
	defaultAgentTraceType = agentTraceTypeIsolated
)

const (
	grpcCheckRequest             = "grpc.CheckRequest"
	grpcExecProcessRequest       = "grpc.ExecProcessRequest"
	grpcCreateSandboxRequest     = "grpc.CreateSandboxRequest"
	grpcDestroySandboxRequest    = "grpc.DestroySandboxRequest"
	grpcCreateContainerRequest   = "grpc.CreateContainerRequest"
	grpcStartContainerRequest    = "grpc.StartContainerRequest"
	grpcRemoveContainerRequest   = "grpc.RemoveContainerRequest"
	grpcSignalProcessRequest     = "grpc.SignalProcessRequest"
	grpcUpdateRoutesRequest      = "grpc.UpdateRoutesRequest"
	grpcUpdateInterfaceRequest   = "grpc.UpdateInterfaceRequest"
	grpcListInterfacesRequest    = "grpc.ListInterfacesRequest"
	grpcListRoutesRequest        = "grpc.ListRoutesRequest"
	grpcAddARPNeighborsRequest   = "grpc.AddARPNeighborsRequest"
	grpcOnlineCPUMemRequest      = "grpc.OnlineCPUMemRequest"
	grpcUpdateContainerRequest   = "grpc.UpdateContainerRequest"
	grpcWaitProcessRequest       = "grpc.WaitProcessRequest"
	grpcTtyWinResizeRequest      = "grpc.TtyWinResizeRequest"
	grpcWriteStreamRequest       = "grpc.WriteStreamRequest"
	grpcCloseStdinRequest        = "grpc.CloseStdinRequest"
	grpcStatsContainerRequest    = "grpc.StatsContainerRequest"
	grpcPauseContainerRequest    = "grpc.PauseContainerRequest"
	grpcResumeContainerRequest   = "grpc.ResumeContainerRequest"
	grpcReseedRandomDevRequest   = "grpc.ReseedRandomDevRequest"
	grpcGuestDetailsRequest      = "grpc.GuestDetailsRequest"
	grpcMemHotplugByProbeRequest = "grpc.MemHotplugByProbeRequest"
	grpcCopyFileRequest          = "grpc.CopyFileRequest"
	grpcSetGuestDateTimeRequest  = "grpc.SetGuestDateTimeRequest"
	grpcStartTracingRequest      = "grpc.StartTracingRequest"
	grpcStopTracingRequest       = "grpc.StopTracingRequest"
	grpcGetOOMEventRequest       = "grpc.GetOOMEventRequest"
	grpcGetMetricsRequest        = "grpc.GetMetricsRequest"
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

// Shared path handling:
// 1. create three directories for each sandbox:
// -. /run/kata-containers/shared/sandboxes/$sbx_id/mounts/, a directory to hold all host/guest shared mounts
// -. /run/kata-containers/shared/sandboxes/$sbx_id/shared/, a host/guest shared directory (9pfs/virtiofs source dir)
// -. /run/kata-containers/shared/sandboxes/$sbx_id/private/, a directory to hold all temporary private mounts when creating ro mounts
//
// 2. /run/kata-containers/shared/sandboxes/$sbx_id/mounts/ is bind mounted readonly to /run/kata-containers/shared/sandboxes/$sbx_id/shared/, so guest cannot modify it
//
// 3. host-guest shared files/directories are mounted one-level under /run/kata-containers/shared/sandboxes/$sbx_id/mounts/ and thus present to guest at one level under /run/kata-containers/shared/sandboxes/$sbx_id/shared/
func getSharePath(id string) string {
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
	LongLiveConn       bool
	Debug              bool
	Trace              bool
	EnableDebugConsole bool
	ContainerPipeSize  uint32
	TraceMode          string
	TraceType          string
	DialTimeout        uint32
	KernelModules      []string
}

// KataAgentState is the structure describing the data stored from this
// agent implementation.
type KataAgentState struct {
	URL string
}

type kataAgent struct {
	// lock protects the client pointer
	sync.Mutex
	client *kataclient.AgentClient

	reqHandlers    map[string]reqFunc
	state          KataAgentState
	keepConn       bool
	dynamicTracing bool
	dead           bool
	dialTimout     uint32
	kmodules       []string

	vmSocket interface{}
	ctx      context.Context
}

func (k *kataAgent) trace(parent context.Context, name string) (otelTrace.Span, context.Context) {
	if parent == nil {
		k.Logger().WithField("type", "bug").Error("trace called before context set")
		parent = context.Background()
	}

	tracer := otel.Tracer("kata")
	ctx, span := tracer.Start(parent, name, otelTrace.WithAttributes(label.String("source", "runtime"), label.String("package", "virtcontainers"), label.String("subsystem", "agent")))

	return span, ctx
}

func (k *kataAgent) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "kata_agent")
}

func (k *kataAgent) longLiveConn() bool {
	return k.keepConn
}

// KataAgentSetDefaultTraceConfigOptions validates agent trace options and
// sets defaults.
func KataAgentSetDefaultTraceConfigOptions(config *KataAgentConfig) error {
	if !config.Trace {
		return nil
	}

	switch config.TraceMode {
	case agentTraceModeDynamic:
	case agentTraceModeStatic:
	case "":
		config.TraceMode = defaultAgentTraceMode
	default:
		return fmt.Errorf("invalid kata agent trace mode: %q (need %q or %q)", config.TraceMode, agentTraceModeDynamic, agentTraceModeStatic)
	}

	switch config.TraceType {
	case agentTraceTypeIsolated:
	case agentTraceTypeCollated:
	case "":
		config.TraceType = defaultAgentTraceType
	default:
		return fmt.Errorf("invalid kata agent trace type: %q (need %q or %q)", config.TraceType, agentTraceTypeIsolated, agentTraceTypeCollated)
	}

	return nil
}

// KataAgentKernelParams returns a list of Kata Agent specific kernel
// parameters.
func KataAgentKernelParams(config KataAgentConfig) []Param {
	var params []Param

	if config.Debug {
		params = append(params, Param{Key: "agent.log", Value: "debug"})
	}

	if config.Trace && config.TraceMode == agentTraceModeStatic {
		params = append(params, Param{Key: "agent.trace", Value: config.TraceType})
	}

	if config.ContainerPipeSize > 0 {
		containerPipeSize := strconv.FormatUint(uint64(config.ContainerPipeSize), 10)
		params = append(params, Param{Key: vcAnnotations.ContainerPipeSizeKernelParam, Value: containerPipeSize})
	}

	if config.EnableDebugConsole {
		params = append(params, Param{Key: kernelParamDebugConsole, Value: ""})
		params = append(params, Param{Key: kernelParamDebugConsoleVPort, Value: kernelParamDebugConsoleVPortValue})
	}

	return params
}

func (k *kataAgent) handleTraceSettings(config KataAgentConfig) bool {
	if !config.Trace {
		return false
	}

	disableVMShutdown := false

	switch config.TraceMode {
	case agentTraceModeStatic:
		disableVMShutdown = true
	case agentTraceModeDynamic:
		k.dynamicTracing = true
	}

	return disableVMShutdown
}

func (k *kataAgent) init(ctx context.Context, sandbox *Sandbox, config KataAgentConfig) (disableVMShutdown bool, err error) {
	// save
	k.ctx = sandbox.ctx

	span, _ := k.trace(ctx, "init")
	defer span.End()

	disableVMShutdown = k.handleTraceSettings(config)
	k.keepConn = config.LongLiveConn
	k.kmodules = config.KernelModules
	k.dialTimout = config.DialTimeout

	return disableVMShutdown, nil
}

func (k *kataAgent) agentURL() (string, error) {
	switch s := k.vmSocket.(type) {
	case types.VSock:
		return s.String(), nil
	case types.HybridVSock:
		return s.String(), nil
	case types.MockHybridVSock:
		return s.String(), nil
	default:
		return "", fmt.Errorf("Invalid socket type")
	}
}

func (k *kataAgent) capabilities() types.Capabilities {
	var caps types.Capabilities

	// add all capabilities supported by agent
	caps.SetBlockDeviceSupport()

	return caps
}

func (k *kataAgent) internalConfigure(h hypervisor, id string, config KataAgentConfig) error {
	var err error
	if k.vmSocket, err = h.generateSocket(id); err != nil {
		return err
	}
	k.keepConn = config.LongLiveConn

	return nil
}

func (k *kataAgent) setupSandboxBindMounts(sandbox *Sandbox) (err error) {
	if len(sandbox.config.SandboxBindMounts) == 0 {
		return nil
	}

	// Create subdirectory in host shared path for sandbox mounts
	sandboxMountDir := filepath.Join(getMountPath(sandbox.id), sandboxMountsDir)
	sandboxShareDir := filepath.Join(getSharePath(sandbox.id), sandboxMountsDir)
	if err := os.MkdirAll(sandboxMountDir, DirMode); err != nil {
		return fmt.Errorf("Creating sandbox shared mount directory: %v: %w", sandboxMountDir, err)
	}
	var mountedList []string
	defer func() {
		if err != nil {
			for _, mnt := range mountedList {
				if derr := syscall.Unmount(mnt, syscall.MNT_DETACH|UmountNoFollow); derr != nil {
					k.Logger().WithError(derr).Errorf("cleanup: couldn't unmount %s", mnt)
				}
			}
			if derr := os.RemoveAll(sandboxMountDir); derr != nil {
				k.Logger().WithError(derr).Errorf("cleanup: failed to remove %s", sandboxMountDir)
			}

		}
	}()

	for _, m := range sandbox.config.SandboxBindMounts {
		mountDest := filepath.Join(sandboxMountDir, filepath.Base(m))
		// bind-mount each sandbox mount that's defined into the sandbox mounts dir
		if err := bindMount(context.Background(), m, mountDest, true, "private"); err != nil {
			return fmt.Errorf("Mounting sandbox directory: %v to %v: %w", m, mountDest, err)
		}
		mountedList = append(mountedList, mountDest)

		mountDest = filepath.Join(sandboxShareDir, filepath.Base(m))
		if err := remountRo(context.Background(), mountDest); err != nil {
			return fmt.Errorf("remount sandbox directory: %v to %v: %w", m, mountDest, err)
		}

	}

	return nil
}

func (k *kataAgent) cleanupSandboxBindMounts(sandbox *Sandbox) error {
	if sandbox.config == nil || len(sandbox.config.SandboxBindMounts) == 0 {
		return nil
	}

	var retErr error
	bindmountShareDir := filepath.Join(getMountPath(sandbox.id), sandboxMountsDir)
	for _, m := range sandbox.config.SandboxBindMounts {
		mountPath := filepath.Join(bindmountShareDir, filepath.Base(m))
		if err := syscall.Unmount(mountPath, syscall.MNT_DETACH|UmountNoFollow); err != nil {
			if retErr == nil {
				retErr = err
			}
			k.Logger().WithError(err).Errorf("Failed to unmount sandbox bindmount: %v", mountPath)
		}
	}
	if err := os.RemoveAll(bindmountShareDir); err != nil {
		if retErr == nil {
			retErr = err
		}
		k.Logger().WithError(err).Errorf("Failed to remove sandbox bindmount directory: %s", bindmountShareDir)
	}

	return retErr
}

func (k *kataAgent) configure(ctx context.Context, h hypervisor, id, sharePath string, config KataAgentConfig) error {
	err := k.internalConfigure(h, id, config)
	if err != nil {
		return err
	}

	switch s := k.vmSocket.(type) {
	case types.VSock:
		if err = h.addDevice(ctx, s, vSockPCIDev); err != nil {
			return err
		}
	case types.HybridVSock:
		err = h.addDevice(ctx, s, hybridVirtioVsockDev)
		if err != nil {
			return err
		}
	case types.MockHybridVSock:
	default:
		return vcTypes.ErrInvalidConfigType
	}

	// Neither create shared directory nor add 9p device if hypervisor
	// doesn't support filesystem sharing.
	caps := h.capabilities(ctx)
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

	return h.addDevice(ctx, sharedVolume, fsDev)
}

func (k *kataAgent) configureFromGrpc(h hypervisor, id string, config KataAgentConfig) error {
	return k.internalConfigure(h, id, config)
}

func (k *kataAgent) setupSharedPath(ctx context.Context, sandbox *Sandbox) (err error) {
	// create shared path structure
	sharePath := getSharePath(sandbox.id)
	mountPath := getMountPath(sandbox.id)
	if err := os.MkdirAll(sharePath, DirMode); err != nil {
		return err
	}
	if err := os.MkdirAll(mountPath, DirMode); err != nil {
		return err
	}

	// slave mount so that future mountpoints under mountPath are shown in sharePath as well
	if err := bindMount(ctx, mountPath, sharePath, true, "slave"); err != nil {
		return err
	}
	defer func() {
		if err != nil {
			if umountErr := syscall.Unmount(sharePath, syscall.MNT_DETACH|UmountNoFollow); umountErr != nil {
				k.Logger().WithError(umountErr).Errorf("failed to unmount vm share path %s", sharePath)
			}
		}
	}()

	// Setup sandbox bindmounts, if specified:
	if err = k.setupSandboxBindMounts(sandbox); err != nil {
		return err
	}

	return nil
}

func (k *kataAgent) createSandbox(ctx context.Context, sandbox *Sandbox) error {
	span, ctx := k.trace(ctx, "createSandbox")
	defer span.End()

	if err := k.setupSharedPath(ctx, sandbox); err != nil {
		return err
	}
	return k.configure(ctx, sandbox.hypervisor, sandbox.id, getSharePath(sandbox.id), sandbox.config.AgentConfig)
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
		User: grpc.User{
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
	span, ctx := k.trace(ctx, "exec")
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
		}
		resultRoutes, ok := resultingRoutes.(*grpc.Routes)
		if ok && resultRoutes != nil {
			return resultRoutes.Routes, err
		}
		return nil, err
	}
	return nil, nil
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
			content, err := ioutil.ReadFile(m.Source)
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
	span, ctx := k.trace(ctx, "startSandbox")
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

	// check grpc server is serving
	if err = k.check(ctx); err != nil {
		return err
	}

	// Setup network interfaces and routes
	interfaces, routes, neighs, err := generateVCNetworkStructures(ctx, sandbox.networkNS)
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

	storages := setupStorages(ctx, sandbox)

	kmodules := setupKernelModules(k.kmodules)

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
		return err
	}

	if k.dynamicTracing {
		_, err = k.sendReq(ctx, &grpc.StartTracingRequest{})
		if err != nil {
			return err
		}
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
	caps := sandbox.hypervisor.capabilities(ctx)

	// append 9p shared volume to storages only if filesystem sharing is supported
	if caps.IsFsSharingSupported() {
		// We mount the shared directory in a predefined location
		// in the guest.
		// This is where at least some of the host config files
		// (resolv.conf, etc...) and potentially all container
		// rootfs will reside.
		if sandbox.config.HypervisorConfig.SharedFS == config.VirtioFS {
			// If virtio-fs uses either of the two cache options 'auto, always',
			// the guest directory can be mounted with option 'dax' allowing it to
			// directly map contents from the host. When set to 'none', the mount
			// options should not contain 'dax' lest the virtio-fs daemon crashing
			// with an invalid address reference.
			if sandbox.config.HypervisorConfig.VirtioFSCache != typeVirtioFSNoCache {
				// If virtio_fs_cache_size = 0, dax should not be used.
				if sandbox.config.HypervisorConfig.VirtioFSCacheSize != 0 {
					sharedDirVirtioFSOptions = append(sharedDirVirtioFSOptions, sharedDirVirtioFSDaxOptions)
				}
			}
			sharedVolume := &grpc.Storage{
				Driver:     kataVirtioFSDevType,
				Source:     mountGuestTag,
				MountPoint: kataGuestSharedDir(),
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
	span, ctx := k.trace(ctx, "stopSandbox")
	defer span.End()

	req := &grpc.DestroySandboxRequest{}

	if _, err := k.sendReq(ctx, req); err != nil {
		return err
	}

	if k.dynamicTracing {
		_, err := k.sendReq(ctx, &grpc.StopTracingRequest{})
		if err != nil {
			return err
		}
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
		} else {
			mounts = append(mounts, m)
		}
	}

	// Replace the OCI mounts with the updated list.
	spec.Mounts = mounts

	return nil
}

func (k *kataAgent) replaceOCIMountsForStorages(spec *specs.Spec, volumeStorages []*grpc.Storage) error {
	ociMounts := spec.Mounts
	var index int
	var m specs.Mount

	for i, v := range volumeStorages {
		for index, m = range ociMounts {
			if m.Destination != v.MountPoint {
				continue
			}

			// Create a temporary location to mount the Storage. Mounting to the correct location
			// will be handled by the OCI mount structure.
			filename := fmt.Sprintf("%s-%s", uuid.Generate().String(), filepath.Base(m.Destination))
			path := filepath.Join(kataGuestSandboxStorageDir(), filename)

			k.Logger().Debugf("Replacing OCI mount source (%s) with %s", m.Source, path)
			ociMounts[index].Source = path
			volumeStorages[i].MountPoint = path

			break
		}
		if index == len(ociMounts) {
			return fmt.Errorf("OCI mount not found for block volume %s", v.MountPoint)
		}
	}
	return nil
}

func (k *kataAgent) constraintGRPCSpec(grpcSpec *grpc.Spec, passSeccomp bool) {
	// Disable Hooks since they have been handled on the host and there is
	// no reason to send them to the agent. It would make no sense to try
	// to apply them on the guest.
	grpcSpec.Hooks = nil

	// Pass seccomp only if disable_guest_seccomp is set to false in
	// configuration.toml and guest image is seccomp capable.
	if !passSeccomp {
		grpcSpec.Linux.Seccomp = nil
	}

	// Disable SELinux inside of the virtual machine, the label will apply
	// to the KVM process
	if grpcSpec.Process.SelinuxLabel != "" {
		k.Logger().Info("SELinux label from config will be applied to the hypervisor process, not the VM workload")
		grpcSpec.Process.SelinuxLabel = ""
	}

	// By now only CPU constraints are supported
	// Issue: https://github.com/kata-containers/runtime/issues/158
	// Issue: https://github.com/kata-containers/runtime/issues/204
	grpcSpec.Linux.Resources.Devices = nil
	grpcSpec.Linux.Resources.Pids = nil
	grpcSpec.Linux.Resources.BlockIO = nil
	grpcSpec.Linux.Resources.HugepageLimits = nil
	grpcSpec.Linux.Resources.Network = nil
	if grpcSpec.Linux.Resources.CPU != nil {
		grpcSpec.Linux.Resources.CPU.Cpus = ""
		grpcSpec.Linux.Resources.CPU.Mems = ""
	}

	// There are three main reasons to do not apply systemd cgroups in the VM
	// - Initrd image doesn't have systemd.
	// - Nobody will be able to modify the resources of a specific container by using systemctl set-property.
	// - docker is not running in the VM.
	if vccgroups.IsSystemdCgroup(grpcSpec.Linux.CgroupsPath) {
		// Convert systemd cgroup to cgroupfs
		slice := strings.Split(grpcSpec.Linux.CgroupsPath, ":")
		// 0 - slice: system.slice
		// 1 - prefix: docker
		// 2 - name: abc123
		grpcSpec.Linux.CgroupsPath = filepath.Join("/", slice[1], slice[2])
	}

	// Disable network namespace since it is already handled on the host by
	// virtcontainers. The network is a complex part which cannot be simply
	// passed to the agent.
	// Every other namespaces's paths have to be emptied. This way, there
	// is no confusion from the agent, trying to find an existing namespace
	// on the guest.
	var tmpNamespaces []grpc.LinuxNamespace
	for _, ns := range grpcSpec.Linux.Namespaces {
		switch ns.Type {
		case specs.CgroupNamespace:
		case specs.NetworkNamespace:
		default:
			ns.Path = ""
			tmpNamespaces = append(tmpNamespaces, ns)
		}
	}
	grpcSpec.Linux.Namespaces = tmpNamespaces

	// VFIO char device shouldn't not appear in the guest,
	// the device driver should handle it and determinate its group.
	var linuxDevices []grpc.LinuxDevice
	for _, dev := range grpcSpec.Linux.Devices {
		if dev.Type == "c" && strings.HasPrefix(dev.Path, vfioPath) {
			k.Logger().WithField("vfio-dev", dev.Path).Debug("removing vfio device from grpcSpec")
			continue
		}
		linuxDevices = append(linuxDevices, dev)
	}
	grpcSpec.Linux.Devices = linuxDevices
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

func (k *kataAgent) appendBlockDevice(dev ContainerDevice, c *Container) *grpc.Device {
	device := c.sandbox.devManager.GetDeviceByID(dev.ID)

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

func (k *kataAgent) appendVhostUserBlkDevice(dev ContainerDevice, c *Container) *grpc.Device {
	device := c.sandbox.devManager.GetDeviceByID(dev.ID)

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

func (k *kataAgent) appendDevices(deviceList []*grpc.Device, c *Container) []*grpc.Device {
	var kataDevice *grpc.Device

	for _, dev := range c.devices {
		device := c.sandbox.devManager.GetDeviceByID(dev.ID)
		if device == nil {
			k.Logger().WithField("device", dev.ID).Error("failed to find device by id")
			return nil
		}

		switch device.DeviceType() {
		case config.DeviceBlock:
			kataDevice = k.appendBlockDevice(dev, c)
		case config.VhostUserBlk:
			kataDevice = k.appendVhostUserBlkDevice(dev, c)
		}

		if kataDevice == nil {
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

		if err2 := bindUnmountContainerRootfs(ctx, getMountPath(c.sandbox.id), c.id); err2 != nil {
			k.Logger().WithError(err2).Error("rollback failed bindUnmountContainerRootfs()")
		}
	}
}

func (k *kataAgent) buildContainerRootfs(ctx context.Context, sandbox *Sandbox, c *Container, rootPathParent string) (*grpc.Storage, error) {
	if c.state.Fstype != "" && c.state.BlockDeviceID != "" {
		// The rootfs storage volume represents the container rootfs
		// mount point inside the guest.
		// It can be a block based device (when using block based container
		// overlay on the host) mount or a 9pfs one (for all other overlay
		// implementations).
		rootfs := &grpc.Storage{}

		// This is a block based device rootfs.
		device := sandbox.devManager.GetDeviceByID(c.state.BlockDeviceID)
		if device == nil {
			k.Logger().WithField("device", c.state.BlockDeviceID).Error("failed to find device by id")
			return nil, fmt.Errorf("failed to find device by id %q", c.state.BlockDeviceID)
		}

		blockDrive, ok := device.GetDeviceInfo().(*config.BlockDrive)
		if !ok || blockDrive == nil {
			k.Logger().Error("malformed block drive")
			return nil, fmt.Errorf("malformed block drive")
		}
		switch {
		case sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioMmio:
			rootfs.Driver = kataMmioBlkDevType
			rootfs.Source = blockDrive.VirtPath
		case sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioBlockCCW:
			rootfs.Driver = kataBlkCCWDevType
			rootfs.Source = blockDrive.DevNo
		case sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioBlock:
			rootfs.Driver = kataBlkDevType
			rootfs.Source = blockDrive.PCIPath.String()
		case sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioSCSI:
			rootfs.Driver = kataSCSIDevType
			rootfs.Source = blockDrive.SCSIAddr
		default:
			return nil, fmt.Errorf("Unknown block device driver: %s", sandbox.config.HypervisorConfig.BlockDeviceDriver)
		}

		rootfs.MountPoint = rootPathParent
		rootfs.Fstype = c.state.Fstype

		if c.state.Fstype == "xfs" {
			rootfs.Options = []string{"nouuid"}
		}

		// Ensure container mount destination exists
		// TODO: remove dependency on shared fs path. shared fs is just one kind of storage source.
		// we should not always use shared fs path for all kinds of storage. Instead, all storage
		// should be bind mounted to a tmpfs path for containers to use.
		if err := os.MkdirAll(filepath.Join(getMountPath(c.sandbox.id), c.id, c.rootfsSuffix), DirMode); err != nil {
			return nil, err
		}
		return rootfs, nil
	}

	// This is not a block based device rootfs. We are going to bind mount it into the shared drive
	// between the host and the guest.
	// With virtiofs/9pfs we don't need to ask the agent to mount the rootfs as the shared directory
	// (kataGuestSharedDir) is already mounted in the guest. We only need to mount the rootfs from
	// the host and it will show up in the guest.
	if err := bindMountContainerRootfs(ctx, getMountPath(sandbox.id), c.id, c.rootFs.Target, false); err != nil {
		return nil, err
	}

	return nil, nil
}

func (k *kataAgent) createContainer(ctx context.Context, sandbox *Sandbox, c *Container) (p *Process, err error) {
	span, ctx := k.trace(ctx, "createContainer")
	defer span.End()

	var ctrStorages []*grpc.Storage
	var ctrDevices []*grpc.Device
	var rootfs *grpc.Storage

	// This is the guest absolute root path for that container.
	rootPathParent := filepath.Join(kataGuestSharedDir(), c.id)
	rootPath := filepath.Join(rootPathParent, c.rootfsSuffix)

	// In case the container creation fails, the following defer statement
	// takes care of rolling back actions previously performed.
	defer func() {
		if err != nil {
			k.Logger().WithError(err).Error("createContainer failed")
			k.rollbackFailingContainerCreation(ctx, c)
		}
	}()

	// setup rootfs -- if its block based, we'll receive a non-nil storage object representing
	// the block device for the rootfs, which us utilized for mounting in the guest. This'll be handled
	// already for non-block based rootfs
	if rootfs, err = k.buildContainerRootfs(ctx, sandbox, c, rootPathParent); err != nil {
		return nil, err
	}

	if rootfs != nil {
		// Add rootfs to the list of container storage.
		// We only need to do this for block based rootfs, as we
		// want the agent to mount it into the right location
		// (kataGuestSharedDir/ctrID/
		ctrStorages = append(ctrStorages, rootfs)
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

	// Handle all the volumes that are block device files.
	// Note this call modifies the list of container devices to make sure
	// all hotplugged devices are unplugged, so this needs be done
	// after devices passed with --device are handled.
	volumeStorages, err := k.handleBlockVolumes(c)
	if err != nil {
		return nil, err
	}

	if err := k.replaceOCIMountsForStorages(ociSpec, volumeStorages); err != nil {
		return nil, err
	}

	ctrStorages = append(ctrStorages, volumeStorages...)

	grpcSpec, err := grpc.OCItoGRPC(ociSpec)
	if err != nil {
		return nil, err
	}

	// We need to give the OCI spec our absolute rootfs path in the guest.
	grpcSpec.Root.Path = rootPath

	sharedPidNs := k.handlePidNamespace(grpcSpec, sandbox)

	passSeccomp := !sandbox.config.DisableGuestSeccomp && sandbox.seccompSupported

	// We need to constraint the spec to make sure we're not passing
	// irrelevant information to the agent.
	k.constraintGRPCSpec(grpcSpec, passSeccomp)

	req := &grpc.CreateContainerRequest{
		ContainerId:  c.id,
		ExecId:       c.id,
		Storages:     ctrStorages,
		Devices:      ctrDevices,
		OCI:          grpcSpec,
		SandboxPidns: sharedPidNs,
	}

	if _, err = k.sendReq(ctx, req); err != nil {
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

// handleDeviceBlockVolume handles volume that is block device file
// and DeviceBlock type.
func (k *kataAgent) handleDeviceBlockVolume(c *Container, m Mount, device api.Device) (*grpc.Storage, error) {
	vol := &grpc.Storage{}

	blockDrive, ok := device.GetDeviceInfo().(*config.BlockDrive)
	if !ok || blockDrive == nil {
		k.Logger().Error("malformed block drive")
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

	vol.MountPoint = m.Destination

	// If no explicit FS Type or Options are being set, then let's use what is provided for the particular mount:
	if vol.Fstype == "" {
		vol.Fstype = m.Type
	}
	if len(vol.Options) == 0 {
		vol.Options = m.Options
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

	return vol, nil
}

// handleBlockVolumes handles volumes that are block devices files
// by passing the block devices as Storage to the agent.
func (k *kataAgent) handleBlockVolumes(c *Container) ([]*grpc.Storage, error) {

	var volumeStorages []*grpc.Storage

	for _, m := range c.mounts {
		id := m.BlockDeviceID

		if len(id) == 0 {
			continue
		}

		// Add the block device to the list of container devices, to make sure the
		// device is detached with detachDevices() for a container.
		c.devices = append(c.devices, ContainerDevice{ID: id, ContainerPath: m.Destination})

		var vol *grpc.Storage

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
			k.Logger().Error("Unknown device type")
			continue
		}

		if vol == nil || err != nil {
			return nil, err
		}

		volumeStorages = append(volumeStorages, vol)
	}

	return volumeStorages, nil
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
	span, ctx := k.trace(ctx, "startContainer")
	defer span.End()

	req := &grpc.StartContainerRequest{
		ContainerId: c.id,
	}

	_, err := k.sendReq(ctx, req)
	return err
}

func (k *kataAgent) stopContainer(ctx context.Context, sandbox *Sandbox, c Container) error {
	span, ctx := k.trace(ctx, "stopContainer")
	defer span.End()

	_, err := k.sendReq(ctx, &grpc.RemoveContainerRequest{ContainerId: c.id})
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
	return err
}

func (k *kataAgent) pauseContainer(ctx context.Context, sandbox *Sandbox, c Container) error {
	req := &grpc.PauseContainerRequest{
		ContainerId: c.id,
	}

	_, err := k.sendReq(ctx, req)
	return err
}

func (k *kataAgent) resumeContainer(ctx context.Context, sandbox *Sandbox, c Container) error {
	req := &grpc.ResumeContainerRequest{
		ContainerId: c.id,
	}

	_, err := k.sendReq(ctx, req)
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
	return err
}

func (k *kataAgent) onlineCPUMem(ctx context.Context, cpus uint32, cpuOnly bool) error {
	req := &grpc.OnlineCPUMemRequest{
		Wait:    false,
		NbCpus:  cpus,
		CpuOnly: cpuOnly,
	}

	_, err := k.sendReq(ctx, req)
	return err
}

func (k *kataAgent) statsContainer(ctx context.Context, sandbox *Sandbox, c Container) (*ContainerStats, error) {
	req := &grpc.StatsContainerRequest{
		ContainerId: c.id,
	}

	returnStats, err := k.sendReq(ctx, req)

	if err != nil {
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

	span, _ := k.trace(ctx, "connect")
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
	span, _ := k.trace(ctx, "disconnect")
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
		err = fmt.Errorf("Failed to check if grpc server is working: %s", err)
	}
	return err
}

func (k *kataAgent) waitProcess(ctx context.Context, c *Container, processID string) (int32, error) {
	span, ctx := k.trace(ctx, "waitProcess")
	defer span.End()

	resp, err := k.sendReq(ctx, &grpc.WaitProcessRequest{
		ContainerId: c.id,
		ExecId:      processID,
	})
	if err != nil {
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
		return 0, err
	}

	return int(resp.(*grpc.WriteStreamResponse).Len), nil
}

func (k *kataAgent) closeProcessStdin(ctx context.Context, c *Container, ProcessID string) error {
	_, err := k.sendReq(ctx, &grpc.CloseStdinRequest{
		ContainerId: c.id,
		ExecId:      ProcessID,
	})

	return err
}

func (k *kataAgent) reseedRNG(ctx context.Context, data []byte) error {
	_, err := k.sendReq(ctx, &grpc.ReseedRandomDevRequest{
		Data: data,
	})

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
	k.reqHandlers[grpcStartTracingRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.StartTracing(ctx, req.(*grpc.StartTracingRequest))
	}
	k.reqHandlers[grpcStopTracingRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.StopTracing(ctx, req.(*grpc.StopTracingRequest))
	}
	k.reqHandlers[grpcGetOOMEventRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.GetOOMEvent(ctx, req.(*grpc.GetOOMEventRequest))
	}
	k.reqHandlers[grpcGetMetricsRequest] = func(ctx context.Context, req interface{}) (interface{}, error) {
		return k.client.AgentServiceClient.GetMetrics(ctx, req.(*grpc.GetMetricsRequest))
	}
}

func (k *kataAgent) getReqContext(reqName string) (ctx context.Context, cancel context.CancelFunc) {
	ctx = context.Background()
	switch reqName {
	case grpcWaitProcessRequest, grpcGetOOMEventRequest:
		// Wait and GetOOMEvent have no timeout
	case grpcCheckRequest:
		ctx, cancel = context.WithTimeout(ctx, checkRequestTimeout)
	default:
		ctx, cancel = context.WithTimeout(ctx, defaultRequestTimeout)
	}

	return ctx, cancel
}

func (k *kataAgent) sendReq(spanCtx context.Context, request interface{}) (interface{}, error) {
	start := time.Now()

	if err := k.connect(spanCtx); err != nil {
		return nil, err
	}
	if !k.keepConn {
		defer k.disconnect(spanCtx)
	}

	msgName := proto.MessageName(request.(proto.Message))
	handler := k.reqHandlers[msgName]
	if msgName == "" || handler == nil {
		return nil, errors.New("Invalid request type")
	}
	message := request.(proto.Message)
	ctx, cancel := k.getReqContext(msgName)
	if cancel != nil {
		defer cancel()
	}
	k.Logger().WithField("name", msgName).WithField("req", message.String()).Trace("sending request")

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
		return nil, err
	}

	return resp.(*grpc.GuestDetailsResponse), nil
}

func (k *kataAgent) setGuestDateTime(ctx context.Context, tv time.Time) error {
	_, err := k.sendReq(ctx, &grpc.SetGuestDateTimeRequest{
		Sec:  tv.Unix(),
		Usec: int64(tv.Nanosecond() / 1e3),
	})

	return err
}

func (k *kataAgent) copyFile(ctx context.Context, src, dst string) error {
	var st unix.Stat_t

	err := unix.Stat(src, &st)
	if err != nil {
		return fmt.Errorf("Could not get file %s information: %v", src, err)
	}

	b, err := ioutil.ReadFile(src)
	if err != nil {
		return fmt.Errorf("Could not read file %s: %v", src, err)
	}

	fileSize := int64(len(b))

	k.Logger().WithFields(logrus.Fields{
		"source": src,
		"dest":   dst,
	}).Debugf("Copying file from host to guest")

	cpReq := &grpc.CopyFileRequest{
		Path:     dst,
		DirMode:  uint32(DirMode),
		FileMode: st.Mode,
		FileSize: fileSize,
		Uid:      int32(st.Uid),
		Gid:      int32(st.Gid),
	}

	// Handle the special case where the file is empty
	if fileSize == 0 {
		_, err = k.sendReq(ctx, cpReq)
		return err
	}

	// Copy file by parts if it's needed
	remainingBytes := fileSize
	offset := int64(0)
	for remainingBytes > 0 {
		bytesToCopy := int64(len(b))
		if bytesToCopy > grpcMaxDataSize {
			bytesToCopy = grpcMaxDataSize
		}

		cpReq.Data = b[:bytesToCopy]
		cpReq.Offset = offset

		if _, err = k.sendReq(ctx, cpReq); err != nil {
			return fmt.Errorf("Could not send CopyFile request: %v", err)
		}

		b = b[bytesToCopy:]
		remainingBytes -= bytesToCopy
		offset += grpcMaxDataSize
	}

	return nil
}

func (k *kataAgent) markDead(ctx context.Context) {
	k.Logger().Infof("mark agent dead")
	k.dead = true
	k.disconnect(ctx)
}

func (k *kataAgent) cleanup(ctx context.Context, s *Sandbox) {
	if err := k.cleanupSandboxBindMounts(s); err != nil {
		k.Logger().WithError(err).Errorf("failed to cleanup sandbox bindmounts")
	}

	// Unmount shared path
	path := getSharePath(s.id)
	k.Logger().WithField("path", path).Infof("cleanup agent")
	if err := syscall.Unmount(path, syscall.MNT_DETACH|UmountNoFollow); err != nil {
		k.Logger().WithError(err).Errorf("failed to unmount vm share path %s", path)
	}

	// Unmount mount path
	path = getMountPath(s.id)
	if err := bindUnmountAllRootfs(ctx, path, s); err != nil {
		k.Logger().WithError(err).Errorf("failed to unmount vm mount path %s", path)
	}
	if err := os.RemoveAll(getSandboxPath(s.id)); err != nil {
		k.Logger().WithError(err).Errorf("failed to cleanup vm path %s", getSandboxPath(s.id))
	}
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
		return nil, err
	}

	return resp.(*grpc.Metrics), nil
}
