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
	"regexp"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	aTypes "github.com/kata-containers/agent/pkg/types"
	kataclient "github.com/kata-containers/agent/protocols/client"
	"github.com/kata-containers/agent/protocols/grpc"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	ns "github.com/kata-containers/runtime/virtcontainers/pkg/nsenter"
	vcTypes "github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/runtime/virtcontainers/pkg/uuid"
	"github.com/kata-containers/runtime/virtcontainers/store"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/kata-containers/runtime/virtcontainers/utils"
	opentracing "github.com/opentracing/opentracing-go"

	"github.com/gogo/protobuf/proto"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"
	"golang.org/x/net/context"
	"golang.org/x/sys/unix"
	golangGrpc "google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

const (
	// KataEphemeralDevType creates a tmpfs backed volume for sharing files between containers.
	KataEphemeralDevType = "ephemeral"

	// KataLocalDevType creates a local directory inside the VM for sharing files between
	// containers.
	KataLocalDevType = "local"
)

var (
	checkRequestTimeout   = 30 * time.Second
	defaultKataSocketName = "kata.sock"
	defaultKataChannel    = "agent.channel.0"
	defaultKataDeviceID   = "channel0"
	defaultKataID         = "charch0"
	errorMissingProxy     = errors.New("Missing proxy pointer")
	errorMissingOCISpec   = errors.New("Missing OCI specification")
	kataHostSharedDir     = "/run/kata-containers/shared/sandboxes/"
	kataGuestSharedDir    = "/run/kata-containers/shared/containers/"
	mountGuest9pTag       = "kataShared"
	kataGuestSandboxDir   = "/run/kata-containers/sandbox/"
	type9pFs              = "9p"
	typeVirtioFS          = "virtio_fs"
	vsockSocketScheme     = "vsock"
	// port numbers below 1024 are called privileged ports. Only a process with
	// CAP_NET_BIND_SERVICE capability may bind to these port numbers.
	vSockPort                = 1024
	kata9pDevType            = "9p"
	kataMmioBlkDevType       = "mmioblk"
	kataBlkDevType           = "blk"
	kataSCSIDevType          = "scsi"
	kataNvdimmDevType        = "nvdimm"
	kataVirtioFSDevType      = "virtio-fs"
	sharedDir9pOptions       = []string{"trans=virtio,version=9p2000.L,cache=mmap", "nodev"}
	sharedDirVirtioFSOptions = []string{"default_permissions,allow_other,rootmode=040000,user_id=0,group_id=0,dax,tag=" + mountGuest9pTag, "nodev"}
	shmDir                   = "shm"
	kataEphemeralDevType     = "ephemeral"
	ephemeralPath            = filepath.Join(kataGuestSandboxDir, kataEphemeralDevType)
	grpcMaxDataSize          = int64(1024 * 1024)
	localDirOptions          = []string{"mode=0777"}
	maxHostnameLen           = 64
)

const (
	agentTraceModeDynamic  = "dynamic"
	agentTraceModeStatic   = "static"
	agentTraceTypeIsolated = "isolated"
	agentTraceTypeCollated = "collated"

	defaultAgentTraceMode = agentTraceModeDynamic
	defaultAgentTraceType = agentTraceTypeIsolated
)

// KataAgentConfig is a structure storing information needed
// to reach the Kata Containers agent.
type KataAgentConfig struct {
	LongLiveConn bool
	UseVSock     bool
	Debug        bool
	Trace        bool
	TraceMode    string
	TraceType    string
}

type kataVSOCK struct {
	contextID uint64
	port      uint32
	vhostFd   *os.File
}

func (s *kataVSOCK) String() string {
	return fmt.Sprintf("%s://%d:%d", vsockSocketScheme, s.contextID, s.port)
}

// KataAgentState is the structure describing the data stored from this
// agent implementation.
type KataAgentState struct {
	ProxyPid int
	URL      string
}

type kataAgent struct {
	shim  shim
	proxy proxy

	// lock protects the client pointer
	sync.Mutex
	client *kataclient.AgentClient

	reqHandlers    map[string]reqFunc
	state          KataAgentState
	keepConn       bool
	proxyBuiltIn   bool
	dynamicTracing bool

	vmSocket interface{}
	ctx      context.Context
}

func (k *kataAgent) trace(name string) (opentracing.Span, context.Context) {
	if k.ctx == nil {
		k.Logger().WithField("type", "bug").Error("trace called before context set")
		k.ctx = context.Background()
	}

	span, ctx := opentracing.StartSpanFromContext(k.ctx, name)

	span.SetTag("subsystem", "agent")
	span.SetTag("type", "kata")

	return span, ctx
}

func (k *kataAgent) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "kata_agent")
}

func (k *kataAgent) getVMPath(id string) string {
	return filepath.Join(store.RunVMStoragePath, id)
}

func (k *kataAgent) getSharePath(id string) string {
	return filepath.Join(kataHostSharedDir, id)
}

func (k *kataAgent) generateVMSocket(id string, c KataAgentConfig) error {
	if c.UseVSock {
		// We want to go through VSOCK. The VM VSOCK endpoint will be our gRPC.
		k.Logger().Debug("agent: Using vsock VM socket endpoint")
		// We dont know yet the context ID - set empty vsock configuration
		k.vmSocket = kataVSOCK{}
	} else {
		k.Logger().Debug("agent: Using unix socket form VM socket endpoint")
		// We need to generate a host UNIX socket path for the emulated serial port.
		kataSock, err := utils.BuildSocketPath(k.getVMPath(id), defaultKataSocketName)
		if err != nil {
			return err
		}

		k.vmSocket = types.Socket{
			DeviceID: defaultKataDeviceID,
			ID:       defaultKataID,
			HostPath: kataSock,
			Name:     defaultKataChannel,
		}
	}

	return nil
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

func (k *kataAgent) init(ctx context.Context, sandbox *Sandbox, config interface{}) (disableVMShutdown bool, err error) {
	// save
	k.ctx = sandbox.ctx

	span, _ := k.trace("init")
	defer span.Finish()

	switch c := config.(type) {
	case KataAgentConfig:
		if err := k.generateVMSocket(sandbox.id, c); err != nil {
			return false, err
		}

		disableVMShutdown = k.handleTraceSettings(c)
		k.keepConn = c.LongLiveConn
	default:
		return false, vcTypes.ErrInvalidConfigType
	}

	k.proxy, err = newProxy(sandbox.config.ProxyType)
	if err != nil {
		return false, err
	}

	k.shim, err = newShim(sandbox.config.ShimType)
	if err != nil {
		return false, err
	}

	k.proxyBuiltIn = isProxyBuiltIn(sandbox.config.ProxyType)

	// Fetch agent runtime info.
	if err := sandbox.store.Load(store.Agent, &k.state); err != nil {
		k.Logger().Debug("Could not retrieve anything from storage")
	}

	return disableVMShutdown, nil
}

func (k *kataAgent) agentURL() (string, error) {
	switch s := k.vmSocket.(type) {
	case types.Socket:
		return s.HostPath, nil
	case kataVSOCK:
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

func (k *kataAgent) internalConfigure(h hypervisor, id, sharePath string, builtin bool, config interface{}) error {
	if config != nil {
		switch c := config.(type) {
		case KataAgentConfig:
			if err := k.generateVMSocket(id, c); err != nil {
				return err
			}
			k.keepConn = c.LongLiveConn
		default:
			return vcTypes.ErrInvalidConfigType
		}
	}

	if builtin {
		k.proxyBuiltIn = true
	}

	return nil
}

func (k *kataAgent) configure(h hypervisor, id, sharePath string, builtin bool, config interface{}) error {
	err := k.internalConfigure(h, id, sharePath, builtin, config)
	if err != nil {
		return err
	}

	switch s := k.vmSocket.(type) {
	case types.Socket:
		err = h.addDevice(s, serialPortDev)
		if err != nil {
			return err
		}
	case kataVSOCK:
		s.vhostFd, s.contextID, err = utils.FindContextID()
		if err != nil {
			return err
		}
		s.port = uint32(vSockPort)
		if err = h.addDevice(s, vSockPCIDev); err != nil {
			return err
		}
		k.vmSocket = s
	default:
		return vcTypes.ErrInvalidConfigType
	}

	// Neither create shared directory nor add 9p device if hypervisor
	// doesn't support filesystem sharing.
	caps := h.capabilities()
	if !caps.IsFsSharingSupported() {
		return nil
	}

	// Create shared directory and add the shared volume if filesystem sharing is supported.
	// This volume contains all bind mounted container bundles.
	sharedVolume := types.Volume{
		MountTag: mountGuest9pTag,
		HostPath: sharePath,
	}

	if err = os.MkdirAll(sharedVolume.HostPath, store.DirMode); err != nil {
		return err
	}

	return h.addDevice(sharedVolume, fsDev)
}

func (k *kataAgent) configureFromGrpc(id string, builtin bool, config interface{}) error {
	return k.internalConfigure(nil, id, "", builtin, config)
}

func (k *kataAgent) createSandbox(sandbox *Sandbox) error {
	span, _ := k.trace("createSandbox")
	defer span.Finish()

	return k.configure(sandbox.hypervisor, sandbox.id, k.getSharePath(sandbox.id), k.proxyBuiltIn, nil)
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

func (k *kataAgent) exec(sandbox *Sandbox, c Container, cmd types.Cmd) (*Process, error) {
	span, _ := k.trace("exec")
	defer span.Finish()

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

	if _, err := k.sendReq(req); err != nil {
		return nil, err
	}

	enterNSList := []ns.Namespace{
		{
			PID:  c.process.Pid,
			Type: ns.NSTypeNet,
		},
		{
			PID:  c.process.Pid,
			Type: ns.NSTypePID,
		},
	}

	return prepareAndStartShim(sandbox, k.shim, c.id, req.ExecId,
		k.state.URL, "", cmd, []ns.NSType{}, enterNSList)
}

func (k *kataAgent) updateInterface(ifc *vcTypes.Interface) (*vcTypes.Interface, error) {
	// send update interface request
	ifcReq := &grpc.UpdateInterfaceRequest{
		Interface: k.convertToKataAgentInterface(ifc),
	}
	resultingInterface, err := k.sendReq(ifcReq)
	if err != nil {
		k.Logger().WithFields(logrus.Fields{
			"interface-requested": fmt.Sprintf("%+v", ifc),
			"resulting-interface": fmt.Sprintf("%+v", resultingInterface),
		}).WithError(err).Error("update interface request failed")
	}
	if resultInterface, ok := resultingInterface.(*vcTypes.Interface); ok {
		return resultInterface, err
	}
	return nil, err
}

func (k *kataAgent) updateInterfaces(interfaces []*vcTypes.Interface) error {
	for _, ifc := range interfaces {
		if _, err := k.updateInterface(ifc); err != nil {
			return err
		}
	}
	return nil
}

func (k *kataAgent) updateRoutes(routes []*vcTypes.Route) ([]*vcTypes.Route, error) {
	if routes != nil {
		routesReq := &grpc.UpdateRoutesRequest{
			Routes: &grpc.Routes{
				Routes: k.convertToKataAgentRoutes(routes),
			},
		}
		resultingRoutes, err := k.sendReq(routesReq)
		if err != nil {
			k.Logger().WithFields(logrus.Fields{
				"routes-requested": fmt.Sprintf("%+v", routes),
				"resulting-routes": fmt.Sprintf("%+v", resultingRoutes),
			}).WithError(err).Error("update routes request failed")
		}
		resultRoutes, ok := resultingRoutes.(*grpc.Routes)
		if ok && resultRoutes != nil {
			return k.convertToRoutes(resultRoutes.Routes), err
		}
		return nil, err
	}
	return nil, nil
}

func (k *kataAgent) listInterfaces() ([]*vcTypes.Interface, error) {
	req := &grpc.ListInterfacesRequest{}
	resultingInterfaces, err := k.sendReq(req)
	if err != nil {
		return nil, err
	}
	resultInterfaces, ok := resultingInterfaces.(*grpc.Interfaces)
	if ok {
		return k.convertToInterfaces(resultInterfaces.Interfaces), err
	}
	return nil, err
}

func (k *kataAgent) listRoutes() ([]*vcTypes.Route, error) {
	req := &grpc.ListRoutesRequest{}
	resultingRoutes, err := k.sendReq(req)
	if err != nil {
		return nil, err
	}
	resultRoutes, ok := resultingRoutes.(*grpc.Routes)
	if ok {
		return k.convertToRoutes(resultRoutes.Routes), err
	}
	return nil, err
}

func (k *kataAgent) startProxy(sandbox *Sandbox) error {
	span, _ := k.trace("startProxy")
	defer span.Finish()

	var err error

	if k.proxy == nil {
		return errorMissingProxy
	}

	if k.proxy.consoleWatched() {
		return nil
	}

	if k.state.URL != "" {
		k.Logger().WithFields(logrus.Fields{
			"sandbox":   sandbox.id,
			"proxy-pid": k.state.ProxyPid,
			"proxy-url": k.state.URL,
		}).Infof("proxy already started")
		return nil
	}

	// Get agent socket path to provide it to the proxy.
	agentURL, err := k.agentURL()
	if err != nil {
		return err
	}

	consoleURL, err := sandbox.hypervisor.getSandboxConsole(sandbox.id)
	if err != nil {
		return err
	}

	proxyParams := proxyParams{
		id:         sandbox.id,
		path:       sandbox.config.ProxyConfig.Path,
		agentURL:   agentURL,
		consoleURL: consoleURL,
		logger:     k.Logger().WithField("sandbox", sandbox.id),
		debug:      sandbox.config.ProxyConfig.Debug,
	}

	// Start the proxy here
	pid, uri, err := k.proxy.start(proxyParams)
	if err != nil {
		return err
	}

	// If error occurs after kata-proxy process start,
	// then rollback to kill kata-proxy process
	defer func() {
		if err != nil {
			k.proxy.stop(pid)
		}
	}()

	// Fill agent state with proxy information, and store them.
	if err = k.setProxy(sandbox, k.proxy, pid, uri); err != nil {
		return err
	}

	k.Logger().WithFields(logrus.Fields{
		"sandbox":   sandbox.id,
		"proxy-pid": pid,
		"proxy-url": uri,
	}).Info("proxy started")

	return nil
}

func (k *kataAgent) getAgentURL() (string, error) {
	return k.agentURL()
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

func (k *kataAgent) setProxy(sandbox *Sandbox, proxy proxy, pid int, url string) error {
	if url == "" {
		var err error
		if url, err = k.agentURL(); err != nil {
			return err
		}
	}

	// Are we setting the same proxy again?
	if k.proxy != nil && k.state.URL != "" && k.state.URL != url {
		k.proxy.stop(k.state.ProxyPid)
	}

	k.proxy = proxy
	k.state.ProxyPid = pid
	k.state.URL = url
	if sandbox != nil {
		if err := sandbox.store.Store(store.Agent, k.state); err != nil {
			return err
		}
	}

	return nil
}

func (k *kataAgent) setProxyFromGrpc(proxy proxy, pid int, url string) {
	k.proxy = proxy
	k.state.ProxyPid = pid
	k.state.URL = url
}

func (k *kataAgent) startSandbox(sandbox *Sandbox) error {
	span, _ := k.trace("startSandbox")
	defer span.Finish()

	err := k.startProxy(sandbox)
	if err != nil {
		return err
	}

	defer func() {
		if err != nil {
			k.proxy.stop(k.state.ProxyPid)
		}
	}()

	hostname := sandbox.config.Hostname
	if len(hostname) > maxHostnameLen {
		hostname = hostname[:maxHostnameLen]
	}

	// check grpc server is serving
	if err = k.check(); err != nil {
		return err
	}

	//
	// Setup network interfaces and routes
	//
	interfaces, routes, err := generateInterfacesAndRoutes(sandbox.networkNS)
	if err != nil {
		return err
	}
	if err = k.updateInterfaces(interfaces); err != nil {
		return err
	}
	if _, err = k.updateRoutes(routes); err != nil {
		return err
	}

	storages := []*grpc.Storage{}
	caps := sandbox.hypervisor.capabilities()

	// append 9p shared volume to storages only if filesystem sharing is supported
	if caps.IsFsSharingSupported() {
		// We mount the shared directory in a predefined location
		// in the guest.
		// This is where at least some of the host config files
		// (resolv.conf, etc...) and potentially all container
		// rootfs will reside.
		if sandbox.config.HypervisorConfig.SharedFS == config.VirtioFS {
			sharedVolume := &grpc.Storage{
				Driver:     kataVirtioFSDevType,
				Source:     "none",
				MountPoint: kataGuestSharedDir,
				Fstype:     typeVirtioFS,
				Options:    sharedDirVirtioFSOptions,
			}

			storages = append(storages, sharedVolume)
		} else {
			sharedDir9pOptions = append(sharedDir9pOptions, fmt.Sprintf("msize=%d", sandbox.config.HypervisorConfig.Msize9p))

			sharedVolume := &grpc.Storage{
				Driver:     kata9pDevType,
				Source:     mountGuest9pTag,
				MountPoint: kataGuestSharedDir,
				Fstype:     type9pFs,
				Options:    sharedDir9pOptions,
			}

			storages = append(storages, sharedVolume)
		}
	}

	if sandbox.shmSize > 0 {
		path := filepath.Join(kataGuestSandboxDir, shmDir)
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

	req := &grpc.CreateSandboxRequest{
		Hostname:      hostname,
		Storages:      storages,
		SandboxPidns:  sandbox.sharePidNs,
		SandboxId:     sandbox.id,
		GuestHookPath: sandbox.config.HypervisorConfig.GuestHookPath,
	}

	_, err = k.sendReq(req)
	if err != nil {
		return err
	}

	if k.dynamicTracing {
		_, err = k.sendReq(&grpc.StartTracingRequest{})
		if err != nil {
			return err
		}
	}

	return nil
}

func (k *kataAgent) stopSandbox(sandbox *Sandbox) error {
	span, _ := k.trace("stopSandbox")
	defer span.Finish()

	if k.proxy == nil {
		return errorMissingProxy
	}

	req := &grpc.DestroySandboxRequest{}

	if _, err := k.sendReq(req); err != nil {
		return err
	}

	if k.dynamicTracing {
		_, err := k.sendReq(&grpc.StopTracingRequest{})
		if err != nil {
			return err
		}
	}

	if err := k.proxy.stop(k.state.ProxyPid); err != nil {
		return err
	}

	// clean up agent state
	k.state.ProxyPid = -1
	k.state.URL = ""
	if err := sandbox.store.Store(store.Agent, k.state); err != nil {
		// ignore error
		k.Logger().WithError(err).WithField("sandbox", sandbox.id).Error("failed to clean up agent state")
	}

	return nil
}

func (k *kataAgent) replaceOCIMountSource(spec *specs.Spec, guestMounts []Mount) error {
	ociMounts := spec.Mounts

	for index, m := range ociMounts {
		for _, guestMount := range guestMounts {
			if guestMount.Destination != m.Destination {
				continue
			}

			k.Logger().Debugf("Replacing OCI mount (%s) source %s with %s", m.Destination, m.Source, guestMount.Source)
			ociMounts[index].Source = guestMount.Source
		}
	}

	return nil
}

func (k *kataAgent) removeIgnoredOCIMount(spec *specs.Spec, ignoredMounts []Mount) error {
	var mounts []specs.Mount

	for _, m := range spec.Mounts {
		found := false
		for _, ignoredMount := range ignoredMounts {
			if ignoredMount.Source == m.Source {
				k.Logger().WithField("removed-mount", m.Source).Debug("Removing OCI mount")
				found = true
				break
			}
		}

		if !found {
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
			path := filepath.Join(kataGuestSharedDir, filename)

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

func constraintGRPCSpec(grpcSpec *grpc.Spec, systemdCgroup bool, passSeccomp bool) {
	// Disable Hooks since they have been handled on the host and there is
	// no reason to send them to the agent. It would make no sense to try
	// to apply them on the guest.
	grpcSpec.Hooks = nil

	// Pass seccomp only if disable_guest_seccomp is set to false in
	// configuration.toml and guest image is seccomp capable.
	if !passSeccomp {
		grpcSpec.Linux.Seccomp = nil
	}

	// By now only CPU constraints are supported
	// Issue: https://github.com/kata-containers/runtime/issues/158
	// Issue: https://github.com/kata-containers/runtime/issues/204
	grpcSpec.Linux.Resources.Devices = nil
	grpcSpec.Linux.Resources.Pids = nil
	grpcSpec.Linux.Resources.BlockIO = nil
	grpcSpec.Linux.Resources.HugepageLimits = nil
	grpcSpec.Linux.Resources.Network = nil

	// There are three main reasons to do not apply systemd cgroups in the VM
	// - Initrd image doesn't have systemd.
	// - Nobody will be able to modify the resources of a specific container by using systemctl set-property.
	// - docker is not running in the VM.
	if systemdCgroup {
		// Convert systemd cgroup to cgroupfs
		// systemd cgroup path: slice:prefix:name
		re := regexp.MustCompile(`([[:alnum:]]|.)+:([[:alnum:]]|.)+:([[:alnum:]]|.)+`)
		systemdCgroupPath := re.FindString(grpcSpec.Linux.CgroupsPath)
		if systemdCgroupPath != "" {
			slice := strings.Split(systemdCgroupPath, ":")
			// 0 - slice: system.slice
			// 1 - prefix: docker
			// 2 - name: abc123
			grpcSpec.Linux.CgroupsPath = filepath.Join("/", slice[1], slice[2])
		}
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
}

func (k *kataAgent) handleShm(grpcSpec *grpc.Spec, sandbox *Sandbox) {
	for idx, mnt := range grpcSpec.Mounts {
		if mnt.Destination != "/dev/shm" {
			continue
		}

		if sandbox.shmSize > 0 {
			grpcSpec.Mounts[idx].Type = "bind"
			grpcSpec.Mounts[idx].Options = []string{"rbind"}
			grpcSpec.Mounts[idx].Source = filepath.Join(kataGuestSandboxDir, shmDir)
			k.Logger().WithField("shm-size", sandbox.shmSize).Info("Using sandbox shm")
		} else {
			sizeOption := fmt.Sprintf("size=%d", DefaultShmSize)
			grpcSpec.Mounts[idx].Type = "tmpfs"
			grpcSpec.Mounts[idx].Source = "shm"
			grpcSpec.Mounts[idx].Options = []string{"noexec", "nosuid", "nodev", "mode=1777", sizeOption}
			k.Logger().WithField("shm-size", sizeOption).Info("Setting up a separate shm for container")
		}
	}
}

func (k *kataAgent) appendDevices(deviceList []*grpc.Device, c *Container) []*grpc.Device {
	for _, dev := range c.devices {
		device := c.sandbox.devManager.GetDeviceByID(dev.ID)
		if device == nil {
			k.Logger().WithField("device", dev.ID).Error("failed to find device by id")
			return nil
		}

		if device.DeviceType() != config.DeviceBlock {
			continue
		}

		d, ok := device.GetDeviceInfo().(*config.BlockDrive)
		if !ok || d == nil {
			k.Logger().WithField("device", device).Error("malformed block drive")
			continue
		}

		kataDevice := &grpc.Device{
			ContainerPath: dev.ContainerPath,
		}

		switch c.sandbox.config.HypervisorConfig.BlockDeviceDriver {
		case config.VirtioMmio:
			kataDevice.Type = kataMmioBlkDevType
			kataDevice.Id = d.VirtPath
			kataDevice.VmPath = d.VirtPath
		case config.VirtioBlock:
			kataDevice.Type = kataBlkDevType
			kataDevice.Id = d.PCIAddr
		case config.VirtioSCSI:
			kataDevice.Type = kataSCSIDevType
			kataDevice.Id = d.SCSIAddr
		case config.Nvdimm:
			kataDevice.Type = kataNvdimmDevType
			kataDevice.VmPath = fmt.Sprintf("/dev/pmem%s", d.NvdimmID)
		}

		deviceList = append(deviceList, kataDevice)
	}

	return deviceList
}

// rollbackFailingContainerCreation rolls back important steps that might have
// been performed before the container creation failed.
// - Unmount container volumes.
// - Unmount container rootfs.
func (k *kataAgent) rollbackFailingContainerCreation(c *Container) {
	if c != nil {
		if err2 := c.unmountHostMounts(); err2 != nil {
			k.Logger().WithError(err2).Error("rollback failed unmountHostMounts()")
		}

		if err2 := bindUnmountContainerRootfs(k.ctx, kataHostSharedDir, c.sandbox.id, c.id); err2 != nil {
			k.Logger().WithError(err2).Error("rollback failed bindUnmountContainerRootfs()")
		}
	}
}

func (k *kataAgent) buildContainerRootfs(sandbox *Sandbox, c *Container, rootPathParent string) (*grpc.Storage, error) {
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

		if sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioMmio {
			rootfs.Driver = kataMmioBlkDevType
			rootfs.Source = blockDrive.VirtPath
		} else if sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioBlock {
			rootfs.Driver = kataBlkDevType
			rootfs.Source = blockDrive.PCIAddr
		} else {
			rootfs.Driver = kataSCSIDevType
			rootfs.Source = blockDrive.SCSIAddr
		}
		rootfs.MountPoint = rootPathParent
		rootfs.Fstype = c.state.Fstype

		if c.state.Fstype == "xfs" {
			rootfs.Options = []string{"nouuid"}
		}

		return rootfs, nil
	}

	// This is not a block based device rootfs.
	// We are going to bind mount it into the 9pfs
	// shared drive between the host and the guest.
	// With 9pfs we don't need to ask the agent to
	// mount the rootfs as the shared directory
	// (kataGuestSharedDir) is already mounted in the
	// guest. We only need to mount the rootfs from
	// the host and it will show up in the guest.
	if err := bindMountContainerRootfs(k.ctx, kataHostSharedDir, sandbox.id, c.id, c.rootFs.Target, false); err != nil {
		return nil, err
	}

	return nil, nil
}

func (k *kataAgent) createContainer(sandbox *Sandbox, c *Container) (p *Process, err error) {
	span, _ := k.trace("createContainer")
	defer span.Finish()

	ociSpecJSON, ok := c.config.Annotations[vcAnnotations.ConfigJSONKey]
	if !ok {
		return nil, errorMissingOCISpec
	}

	var ctrStorages []*grpc.Storage
	var ctrDevices []*grpc.Device
	var rootfs *grpc.Storage

	// This is the guest absolute root path for that container.
	rootPathParent := filepath.Join(kataGuestSharedDir, c.id)
	rootPath := filepath.Join(rootPathParent, c.rootfsSuffix)

	// In case the container creation fails, the following defer statement
	// takes care of rolling back actions previously performed.
	defer func() {
		if err != nil {
			k.rollbackFailingContainerCreation(c)
		}
	}()

	if rootfs, err = k.buildContainerRootfs(sandbox, c, rootPathParent); err != nil {
		return nil, err
	} else if rootfs != nil {
		// Add rootfs to the list of container storage.
		// We only need to do this for block based rootfs, as we
		// want the agent to mount it into the right location
		// (kataGuestSharedDir/ctrID/
		ctrStorages = append(ctrStorages, rootfs)
	}

	ociSpec := &specs.Spec{}
	if err = json.Unmarshal([]byte(ociSpecJSON), ociSpec); err != nil {
		return nil, err
	}

	// Handle container mounts
	newMounts, ignoredMounts, err := c.mountSharedDirMounts(kataHostSharedDir, kataGuestSharedDir)
	if err != nil {
		return nil, err
	}

	epheStorages := k.handleEphemeralStorage(ociSpec.Mounts)
	ctrStorages = append(ctrStorages, epheStorages...)

	localStorages := k.handleLocalStorage(ociSpec.Mounts, sandbox.id)
	ctrStorages = append(ctrStorages, localStorages...)

	// We replace all OCI mount sources that match our container mount
	// with the right source path (The guest one).
	if err = k.replaceOCIMountSource(ociSpec, newMounts); err != nil {
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
	volumeStorages := k.handleBlockVolumes(c)
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
	constraintGRPCSpec(grpcSpec, sandbox.config.SystemdCgroup, passSeccomp)

	k.handleShm(grpcSpec, sandbox)

	req := &grpc.CreateContainerRequest{
		ContainerId:  c.id,
		ExecId:       c.id,
		Storages:     ctrStorages,
		Devices:      ctrDevices,
		OCI:          grpcSpec,
		SandboxPidns: sharedPidNs,
	}

	if _, err = k.sendReq(req); err != nil {
		return nil, err
	}

	createNSList := []ns.NSType{ns.NSTypePID}

	enterNSList := []ns.Namespace{}
	if sandbox.networkNS.NetNsPath != "" {
		enterNSList = append(enterNSList, ns.Namespace{
			Path: sandbox.networkNS.NetNsPath,
			Type: ns.NSTypeNet,
		})
	}

	// Ask to the shim to print the agent logs, if it's the process who monitors the sandbox and use_vsock is true (no proxy)
	var consoleURL string
	if sandbox.config.HypervisorConfig.UseVSock && c.GetAnnotations()[vcAnnotations.ContainerTypeKey] == string(PodSandbox) {
		consoleURL, err = sandbox.hypervisor.getSandboxConsole(sandbox.id)
		if err != nil {
			return nil, err
		}
	}

	return prepareAndStartShim(sandbox, k.shim, c.id, req.ExecId,
		k.state.URL, consoleURL, c.config.Cmd, createNSList, enterNSList)
}

// handleEphemeralStorage handles ephemeral storages by
// creating a Storage from corresponding source of the mount point
func (k *kataAgent) handleEphemeralStorage(mounts []specs.Mount) []*grpc.Storage {
	var epheStorages []*grpc.Storage
	for idx, mnt := range mounts {
		if mnt.Type == KataEphemeralDevType {
			// Set the mount source path to a path that resides inside the VM
			mounts[idx].Source = filepath.Join(ephemeralPath, filepath.Base(mnt.Source))
			// Set the mount type to "bind"
			mounts[idx].Type = "bind"

			// Create a storage struct so that kata agent is able to create
			// tmpfs backed volume inside the VM
			epheStorage := &grpc.Storage{
				Driver:     KataEphemeralDevType,
				Source:     "tmpfs",
				Fstype:     "tmpfs",
				MountPoint: mounts[idx].Source,
			}
			epheStorages = append(epheStorages, epheStorage)
		}
	}
	return epheStorages
}

// handleLocalStorage handles local storage within the VM
// by creating a directory in the VM from the source of the mount point.
func (k *kataAgent) handleLocalStorage(mounts []specs.Mount, sandboxID string) []*grpc.Storage {
	var localStorages []*grpc.Storage
	for idx, mnt := range mounts {
		if mnt.Type == KataLocalDevType {
			// Set the mount source path to a the desired directory point in the VM.
			// In this case it is located in the sandbox directory.
			// We rely on the fact that the first container in the VM has the same ID as the sandbox ID.
			// In Kubernetes, this is usually the pause container and we depend on it existing for
			// local directories to work.
			mounts[idx].Source = filepath.Join(kataGuestSharedDir, sandboxID, KataLocalDevType, filepath.Base(mnt.Source))

			// Create a storage struct so that the kata agent is able to create the
			// directory inside the VM.
			localStorage := &grpc.Storage{
				Driver:     KataLocalDevType,
				Source:     KataLocalDevType,
				Fstype:     KataLocalDevType,
				MountPoint: mounts[idx].Source,
				Options:    localDirOptions,
			}
			localStorages = append(localStorages, localStorage)
		}
	}
	return localStorages
}

// handleBlockVolumes handles volumes that are block devices files
// by passing the block devices as Storage to the agent.
func (k *kataAgent) handleBlockVolumes(c *Container) []*grpc.Storage {

	var volumeStorages []*grpc.Storage

	for _, m := range c.mounts {
		id := m.BlockDeviceID

		if len(id) == 0 {
			continue
		}

		// Add the block device to the list of container devices, to make sure the
		// device is detached with detachDevices() for a container.
		c.devices = append(c.devices, ContainerDevice{ID: id, ContainerPath: m.Destination})
		if err := c.storeDevices(); err != nil {
			k.Logger().WithField("device", id).WithError(err).Error("store device failed")
			return nil
		}

		vol := &grpc.Storage{}

		device := c.sandbox.devManager.GetDeviceByID(id)
		if device == nil {
			k.Logger().WithField("device", id).Error("failed to find device by id")
			return nil
		}
		blockDrive, ok := device.GetDeviceInfo().(*config.BlockDrive)
		if !ok || blockDrive == nil {
			k.Logger().Error("malformed block drive")
			continue
		}
		if c.sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioBlock {
			vol.Driver = kataBlkDevType
			vol.Source = blockDrive.PCIAddr
		} else if c.sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioMmio {
			vol.Driver = kataMmioBlkDevType
			vol.Source = blockDrive.VirtPath
		} else {
			vol.Driver = kataSCSIDevType
			vol.Source = blockDrive.SCSIAddr
		}

		vol.MountPoint = m.Destination
		vol.Fstype = "bind"
		vol.Options = []string{"bind"}

		volumeStorages = append(volumeStorages, vol)
	}

	return volumeStorages
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

func (k *kataAgent) startContainer(sandbox *Sandbox, c *Container) error {
	span, _ := k.trace("startContainer")
	defer span.Finish()

	req := &grpc.StartContainerRequest{
		ContainerId: c.id,
	}

	_, err := k.sendReq(req)
	return err
}

func (k *kataAgent) stopContainer(sandbox *Sandbox, c Container) error {
	span, _ := k.trace("stopContainer")
	defer span.Finish()

	req := &grpc.RemoveContainerRequest{
		ContainerId: c.id,
	}

	if _, err := k.sendReq(req); err != nil {
		return err
	}

	if err := c.unmountHostMounts(); err != nil {
		return err
	}

	return bindUnmountContainerRootfs(k.ctx, kataHostSharedDir, sandbox.id, c.id)
}

func (k *kataAgent) signalProcess(c *Container, processID string, signal syscall.Signal, all bool) error {
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

	_, err := k.sendReq(req)
	return err
}

func (k *kataAgent) winsizeProcess(c *Container, processID string, height, width uint32) error {
	req := &grpc.TtyWinResizeRequest{
		ContainerId: c.id,
		ExecId:      processID,
		Row:         height,
		Column:      width,
	}

	_, err := k.sendReq(req)
	return err
}

func (k *kataAgent) processListContainer(sandbox *Sandbox, c Container, options ProcessListOptions) (ProcessList, error) {
	req := &grpc.ListProcessesRequest{
		ContainerId: c.id,
		Format:      options.Format,
		Args:        options.Args,
	}

	resp, err := k.sendReq(req)
	if err != nil {
		return nil, err
	}

	processList, ok := resp.(*grpc.ListProcessesResponse)
	if !ok {
		return nil, fmt.Errorf("Bad list processes response")
	}

	return processList.ProcessList, nil
}

func (k *kataAgent) updateContainer(sandbox *Sandbox, c Container, resources specs.LinuxResources) error {
	grpcResources, err := grpc.ResourcesOCItoGRPC(&resources)
	if err != nil {
		return err
	}

	req := &grpc.UpdateContainerRequest{
		ContainerId: c.id,
		Resources:   grpcResources,
	}

	_, err = k.sendReq(req)
	return err
}

func (k *kataAgent) pauseContainer(sandbox *Sandbox, c Container) error {
	req := &grpc.PauseContainerRequest{
		ContainerId: c.id,
	}

	_, err := k.sendReq(req)
	return err
}

func (k *kataAgent) resumeContainer(sandbox *Sandbox, c Container) error {
	req := &grpc.ResumeContainerRequest{
		ContainerId: c.id,
	}

	_, err := k.sendReq(req)
	return err
}

func (k *kataAgent) memHotplugByProbe(addr uint64, sizeMB uint32, memorySectionSizeMB uint32) error {
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

	_, err := k.sendReq(req)
	return err
}

func (k *kataAgent) onlineCPUMem(cpus uint32, cpuOnly bool) error {
	req := &grpc.OnlineCPUMemRequest{
		Wait:    false,
		NbCpus:  cpus,
		CpuOnly: cpuOnly,
	}

	_, err := k.sendReq(req)
	return err
}

func (k *kataAgent) statsContainer(sandbox *Sandbox, c Container) (*ContainerStats, error) {
	req := &grpc.StatsContainerRequest{
		ContainerId: c.id,
	}

	returnStats, err := k.sendReq(req)

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

func (k *kataAgent) connect() error {
	// lockless quick pass
	if k.client != nil {
		return nil
	}

	span, _ := k.trace("connect")
	defer span.Finish()

	// This is for the first connection only, to prevent race
	k.Lock()
	defer k.Unlock()
	if k.client != nil {
		return nil
	}

	k.Logger().WithField("url", k.state.URL).Info("New client")
	client, err := kataclient.NewAgentClient(k.ctx, k.state.URL, k.proxyBuiltIn)
	if err != nil {
		return err
	}

	k.installReqFunc(client)
	k.client = client

	return nil
}

func (k *kataAgent) disconnect() error {
	span, _ := k.trace("disconnect")
	defer span.Finish()

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
func (k *kataAgent) check() error {
	span, _ := k.trace("check")
	defer span.Finish()

	_, err := k.sendReq(&grpc.CheckRequest{})
	if err != nil {
		err = fmt.Errorf("Failed to check if grpc server is working: %s", err)
	}
	return err
}

func (k *kataAgent) waitProcess(c *Container, processID string) (int32, error) {
	span, _ := k.trace("waitProcess")
	defer span.Finish()

	resp, err := k.sendReq(&grpc.WaitProcessRequest{
		ContainerId: c.id,
		ExecId:      processID,
	})
	if err != nil {
		return 0, err
	}

	return resp.(*grpc.WaitProcessResponse).Status, nil
}

func (k *kataAgent) writeProcessStdin(c *Container, ProcessID string, data []byte) (int, error) {
	resp, err := k.sendReq(&grpc.WriteStreamRequest{
		ContainerId: c.id,
		ExecId:      ProcessID,
		Data:        data,
	})

	if err != nil {
		return 0, err
	}

	return int(resp.(*grpc.WriteStreamResponse).Len), nil
}

func (k *kataAgent) closeProcessStdin(c *Container, ProcessID string) error {
	_, err := k.sendReq(&grpc.CloseStdinRequest{
		ContainerId: c.id,
		ExecId:      ProcessID,
	})

	return err
}

func (k *kataAgent) reseedRNG(data []byte) error {
	_, err := k.sendReq(&grpc.ReseedRandomDevRequest{
		Data: data,
	})

	return err
}

type reqFunc func(context.Context, interface{}, ...golangGrpc.CallOption) (interface{}, error)

func (k *kataAgent) installReqFunc(c *kataclient.AgentClient) {
	k.reqHandlers = make(map[string]reqFunc)
	k.reqHandlers["grpc.CheckRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		ctx, cancel := context.WithTimeout(ctx, checkRequestTimeout)
		defer cancel()
		return k.client.Check(ctx, req.(*grpc.CheckRequest), opts...)
	}
	k.reqHandlers["grpc.ExecProcessRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.ExecProcess(ctx, req.(*grpc.ExecProcessRequest), opts...)
	}
	k.reqHandlers["grpc.CreateSandboxRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.CreateSandbox(ctx, req.(*grpc.CreateSandboxRequest), opts...)
	}
	k.reqHandlers["grpc.DestroySandboxRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.DestroySandbox(ctx, req.(*grpc.DestroySandboxRequest), opts...)
	}
	k.reqHandlers["grpc.CreateContainerRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.CreateContainer(ctx, req.(*grpc.CreateContainerRequest), opts...)
	}
	k.reqHandlers["grpc.StartContainerRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.StartContainer(ctx, req.(*grpc.StartContainerRequest), opts...)
	}
	k.reqHandlers["grpc.RemoveContainerRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.RemoveContainer(ctx, req.(*grpc.RemoveContainerRequest), opts...)
	}
	k.reqHandlers["grpc.SignalProcessRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.SignalProcess(ctx, req.(*grpc.SignalProcessRequest), opts...)
	}
	k.reqHandlers["grpc.UpdateRoutesRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.UpdateRoutes(ctx, req.(*grpc.UpdateRoutesRequest), opts...)
	}
	k.reqHandlers["grpc.UpdateInterfaceRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.UpdateInterface(ctx, req.(*grpc.UpdateInterfaceRequest), opts...)
	}
	k.reqHandlers["grpc.ListInterfacesRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.ListInterfaces(ctx, req.(*grpc.ListInterfacesRequest), opts...)
	}
	k.reqHandlers["grpc.ListRoutesRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.ListRoutes(ctx, req.(*grpc.ListRoutesRequest), opts...)
	}
	k.reqHandlers["grpc.OnlineCPUMemRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.OnlineCPUMem(ctx, req.(*grpc.OnlineCPUMemRequest), opts...)
	}
	k.reqHandlers["grpc.ListProcessesRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.ListProcesses(ctx, req.(*grpc.ListProcessesRequest), opts...)
	}
	k.reqHandlers["grpc.UpdateContainerRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.UpdateContainer(ctx, req.(*grpc.UpdateContainerRequest), opts...)
	}
	k.reqHandlers["grpc.WaitProcessRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.WaitProcess(ctx, req.(*grpc.WaitProcessRequest), opts...)
	}
	k.reqHandlers["grpc.TtyWinResizeRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.TtyWinResize(ctx, req.(*grpc.TtyWinResizeRequest), opts...)
	}
	k.reqHandlers["grpc.WriteStreamRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.WriteStdin(ctx, req.(*grpc.WriteStreamRequest), opts...)
	}
	k.reqHandlers["grpc.CloseStdinRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.CloseStdin(ctx, req.(*grpc.CloseStdinRequest), opts...)
	}
	k.reqHandlers["grpc.StatsContainerRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.StatsContainer(ctx, req.(*grpc.StatsContainerRequest), opts...)
	}
	k.reqHandlers["grpc.PauseContainerRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.PauseContainer(ctx, req.(*grpc.PauseContainerRequest), opts...)
	}
	k.reqHandlers["grpc.ResumeContainerRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.ResumeContainer(ctx, req.(*grpc.ResumeContainerRequest), opts...)
	}
	k.reqHandlers["grpc.ReseedRandomDevRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.ReseedRandomDev(ctx, req.(*grpc.ReseedRandomDevRequest), opts...)
	}
	k.reqHandlers["grpc.GuestDetailsRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.GetGuestDetails(ctx, req.(*grpc.GuestDetailsRequest), opts...)
	}
	k.reqHandlers["grpc.MemHotplugByProbeRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.MemHotplugByProbe(ctx, req.(*grpc.MemHotplugByProbeRequest), opts...)
	}
	k.reqHandlers["grpc.CopyFileRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.CopyFile(ctx, req.(*grpc.CopyFileRequest), opts...)
	}
	k.reqHandlers["grpc.SetGuestDateTimeRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.SetGuestDateTime(ctx, req.(*grpc.SetGuestDateTimeRequest), opts...)
	}
	k.reqHandlers["grpc.StartTracingRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.StartTracing(ctx, req.(*grpc.StartTracingRequest), opts...)
	}
	k.reqHandlers["grpc.StopTracingRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		return k.client.StopTracing(ctx, req.(*grpc.StopTracingRequest), opts...)
	}
}

func (k *kataAgent) sendReq(request interface{}) (interface{}, error) {
	span, _ := k.trace("sendReq")
	span.SetTag("request", request)
	defer span.Finish()

	if k.state.ProxyPid > 0 {
		// check that proxy is running before talk with it avoiding long timeouts
		if err := syscall.Kill(k.state.ProxyPid, syscall.Signal(0)); err != nil {
			return nil, fmt.Errorf("Proxy is not running: %v", err)
		}
	}

	if err := k.connect(); err != nil {
		return nil, err
	}
	if !k.keepConn {
		defer k.disconnect()
	}

	msgName := proto.MessageName(request.(proto.Message))
	handler := k.reqHandlers[msgName]
	if msgName == "" || handler == nil {
		return nil, errors.New("Invalid request type")
	}
	message := request.(proto.Message)
	k.Logger().WithField("name", msgName).WithField("req", message.String()).Debug("sending request")

	return handler(k.ctx, request)
}

// readStdout and readStderr are special that we cannot differentiate them with the request types...
func (k *kataAgent) readProcessStdout(c *Container, processID string, data []byte) (int, error) {
	if err := k.connect(); err != nil {
		return 0, err
	}
	if !k.keepConn {
		defer k.disconnect()
	}

	return k.readProcessStream(c.id, processID, data, k.client.ReadStdout)
}

// readStdout and readStderr are special that we cannot differentiate them with the request types...
func (k *kataAgent) readProcessStderr(c *Container, processID string, data []byte) (int, error) {
	if err := k.connect(); err != nil {
		return 0, err
	}
	if !k.keepConn {
		defer k.disconnect()
	}

	return k.readProcessStream(c.id, processID, data, k.client.ReadStderr)
}

type readFn func(context.Context, *grpc.ReadStreamRequest, ...golangGrpc.CallOption) (*grpc.ReadStreamResponse, error)

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

func (k *kataAgent) getGuestDetails(req *grpc.GuestDetailsRequest) (*grpc.GuestDetailsResponse, error) {
	resp, err := k.sendReq(req)
	if err != nil {
		return nil, err
	}

	return resp.(*grpc.GuestDetailsResponse), nil
}

func (k *kataAgent) setGuestDateTime(tv time.Time) error {
	_, err := k.sendReq(&grpc.SetGuestDateTimeRequest{
		Sec:  tv.Unix(),
		Usec: int64(tv.Nanosecond() / 1e3),
	})

	return err
}

func (k *kataAgent) convertToKataAgentIPFamily(ipFamily int) aTypes.IPFamily {
	switch ipFamily {
	case netlink.FAMILY_V4:
		return aTypes.IPFamily_v4
	case netlink.FAMILY_V6:
		return aTypes.IPFamily_v6
	}

	return aTypes.IPFamily_v4
}

func (k *kataAgent) convertToIPFamily(ipFamily aTypes.IPFamily) int {
	switch ipFamily {
	case aTypes.IPFamily_v4:
		return netlink.FAMILY_V4
	case aTypes.IPFamily_v6:
		return netlink.FAMILY_V6
	}

	return netlink.FAMILY_V4
}

func (k *kataAgent) convertToKataAgentIPAddresses(ipAddrs []*vcTypes.IPAddress) (aIPAddrs []*aTypes.IPAddress) {
	for _, ipAddr := range ipAddrs {
		if ipAddr == nil {
			continue
		}

		aIPAddr := &aTypes.IPAddress{
			Family:  k.convertToKataAgentIPFamily(ipAddr.Family),
			Address: ipAddr.Address,
			Mask:    ipAddr.Mask,
		}

		aIPAddrs = append(aIPAddrs, aIPAddr)
	}

	return aIPAddrs
}

func (k *kataAgent) convertToIPAddresses(aIPAddrs []*aTypes.IPAddress) (ipAddrs []*vcTypes.IPAddress) {
	for _, aIPAddr := range aIPAddrs {
		if aIPAddr == nil {
			continue
		}

		ipAddr := &vcTypes.IPAddress{
			Family:  k.convertToIPFamily(aIPAddr.Family),
			Address: aIPAddr.Address,
			Mask:    aIPAddr.Mask,
		}

		ipAddrs = append(ipAddrs, ipAddr)
	}

	return ipAddrs
}

func (k *kataAgent) convertToKataAgentInterface(iface *vcTypes.Interface) *aTypes.Interface {
	if iface == nil {
		return nil
	}

	return &aTypes.Interface{
		Device:      iface.Device,
		Name:        iface.Name,
		IPAddresses: k.convertToKataAgentIPAddresses(iface.IPAddresses),
		Mtu:         iface.Mtu,
		RawFlags:    iface.RawFlags,
		HwAddr:      iface.HwAddr,
		PciAddr:     iface.PciAddr,
	}
}

func (k *kataAgent) convertToInterfaces(aIfaces []*aTypes.Interface) (ifaces []*vcTypes.Interface) {
	for _, aIface := range aIfaces {
		if aIface == nil {
			continue
		}

		iface := &vcTypes.Interface{
			Device:      aIface.Device,
			Name:        aIface.Name,
			IPAddresses: k.convertToIPAddresses(aIface.IPAddresses),
			Mtu:         aIface.Mtu,
			HwAddr:      aIface.HwAddr,
			PciAddr:     aIface.PciAddr,
		}

		ifaces = append(ifaces, iface)
	}

	return ifaces
}

func (k *kataAgent) convertToKataAgentRoutes(routes []*vcTypes.Route) (aRoutes []*aTypes.Route) {
	for _, route := range routes {
		if route == nil {
			continue
		}

		aRoute := &aTypes.Route{
			Dest:    route.Dest,
			Gateway: route.Gateway,
			Device:  route.Device,
			Source:  route.Source,
			Scope:   route.Scope,
		}

		aRoutes = append(aRoutes, aRoute)
	}

	return aRoutes
}

func (k *kataAgent) convertToRoutes(aRoutes []*aTypes.Route) (routes []*vcTypes.Route) {
	for _, aRoute := range aRoutes {
		if aRoute == nil {
			continue
		}

		route := &vcTypes.Route{
			Dest:    aRoute.Dest,
			Gateway: aRoute.Gateway,
			Device:  aRoute.Device,
			Source:  aRoute.Source,
			Scope:   aRoute.Scope,
		}

		routes = append(routes, route)
	}

	return routes
}

func (k *kataAgent) copyFile(src, dst string) error {
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
		DirMode:  uint32(store.DirMode),
		FileMode: st.Mode,
		FileSize: fileSize,
		Uid:      int32(st.Uid),
		Gid:      int32(st.Gid),
	}

	// Handle the special case where the file is empty
	if fileSize == 0 {
		_, err = k.sendReq(cpReq)
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

		if _, err = k.sendReq(cpReq); err != nil {
			return fmt.Errorf("Could not send CopyFile request: %v", err)
		}

		b = b[bytesToCopy:]
		remainingBytes -= bytesToCopy
		offset += grpcMaxDataSize
	}

	return nil
}

func (k *kataAgent) cleanup(id string) {
	path := k.getSharePath(id)
	k.Logger().WithField("path", path).Infof("cleanup agent")
	if err := os.RemoveAll(path); err != nil {
		k.Logger().WithError(err).Errorf("failed to cleanup vm share path %s", path)
	}
}
