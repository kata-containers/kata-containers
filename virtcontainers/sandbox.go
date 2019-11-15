// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"io"
	"math"
	"net"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"syscall"

	"github.com/containerd/cgroups"
	"github.com/containernetworking/plugins/pkg/ns"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"

	"github.com/kata-containers/agent/protocols/grpc"
	"github.com/kata-containers/runtime/pkg/rootless"
	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
	deviceManager "github.com/kata-containers/runtime/virtcontainers/device/manager"
	exp "github.com/kata-containers/runtime/virtcontainers/experimental"
	"github.com/kata-containers/runtime/virtcontainers/persist"
	persistapi "github.com/kata-containers/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/compatoci"
	vcTypes "github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/runtime/virtcontainers/store"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

const (
	// vmStartTimeout represents the time in seconds a sandbox can wait before
	// to consider the VM starting operation failed.
	vmStartTimeout = 10
)

// SandboxStatus describes a sandbox status.
type SandboxStatus struct {
	ID               string
	State            types.SandboxState
	Hypervisor       HypervisorType
	HypervisorConfig HypervisorConfig
	Agent            AgentType
	ContainersStatus []ContainerStatus

	// Annotations allow clients to store arbitrary values,
	// for example to add additional status values required
	// to support particular specifications.
	Annotations map[string]string
}

// SandboxStats describes a sandbox's stats
type SandboxStats struct {
	CgroupStats CgroupStats
	Cpus        int
}

// SandboxConfig is a Sandbox configuration.
type SandboxConfig struct {
	ID string

	Hostname string

	HypervisorType   HypervisorType
	HypervisorConfig HypervisorConfig

	AgentType   AgentType
	AgentConfig interface{}

	ProxyType   ProxyType
	ProxyConfig ProxyConfig

	ShimType   ShimType
	ShimConfig interface{}

	NetworkConfig NetworkConfig

	// Volumes is a list of shared volumes between the host and the Sandbox.
	Volumes []types.Volume

	// Containers describe the list of containers within a Sandbox.
	// This list can be empty and populated by adding containers
	// to the Sandbox a posteriori.
	//TODO: this should be a map to avoid duplicated containers
	Containers []ContainerConfig

	// Annotations keys must be unique strings and must be name-spaced
	// with e.g. reverse domain notation (org.clearlinux.key).
	Annotations map[string]string

	ShmSize uint64

	// SharePidNs sets all containers to share the same sandbox level pid namespace.
	SharePidNs bool

	// types.Stateful keeps sandbox resources in memory across APIs. Users will be responsible
	// for calling Release() to release the memory resources.
	Stateful bool

	// SystemdCgroup enables systemd cgroup support
	SystemdCgroup bool

	// SandboxCgroupOnly enables cgroup only at podlevel in the host
	SandboxCgroupOnly bool

	DisableGuestSeccomp bool

	// Experimental features enabled
	Experimental []exp.Feature
}

func (s *Sandbox) trace(name string) (opentracing.Span, context.Context) {
	if s.ctx == nil {
		s.Logger().WithField("type", "bug").Error("trace called before context set")
		s.ctx = context.Background()
	}

	span, ctx := opentracing.StartSpanFromContext(s.ctx, name)

	span.SetTag("subsystem", "sandbox")

	return span, ctx
}

func (s *Sandbox) startProxy() error {

	// If the proxy is KataBuiltInProxyType type, it needs to restart the proxy
	// to watch the guest console if it hadn't been watched.
	if s.agent == nil {
		return fmt.Errorf("sandbox %s missed agent pointer", s.ID())
	}

	return s.agent.startProxy(s)
}

// valid checks that the sandbox configuration is valid.
func (sandboxConfig *SandboxConfig) valid() bool {
	if sandboxConfig.ID == "" {
		return false
	}

	if _, err := newHypervisor(sandboxConfig.HypervisorType); err != nil {
		sandboxConfig.HypervisorType = QemuHypervisor
	}

	// validate experimental features
	for _, f := range sandboxConfig.Experimental {
		if exp.Get(f.Name) == nil {
			return false
		}
	}
	return true
}

// Sandbox is composed of a set of containers and a runtime environment.
// A Sandbox can be created, deleted, started, paused, stopped, listed, entered, and restored.
type Sandbox struct {
	id string

	sync.Mutex
	factory    Factory
	hypervisor hypervisor
	agent      agent
	store      *store.VCStore
	// store is used to replace VCStore step by step
	newStore persistapi.PersistDriver

	network Network
	monitor *monitor

	config *SandboxConfig

	devManager api.DeviceManager

	volumes []types.Volume

	containers map[string]*Container

	runPath    string
	configPath string

	state types.SandboxState

	networkNS NetworkNamespace

	annotationsLock *sync.RWMutex

	wg *sync.WaitGroup

	shmSize           uint64
	sharePidNs        bool
	stateful          bool
	seccompSupported  bool
	disableVMShutdown bool

	ctx context.Context
}

// ID returns the sandbox identifier string.
func (s *Sandbox) ID() string {
	return s.id
}

// Logger returns a logrus logger appropriate for logging Sandbox messages
func (s *Sandbox) Logger() *logrus.Entry {
	return virtLog.WithFields(logrus.Fields{
		"subsystem": "sandbox",
		"sandbox":   s.id,
	})
}

// Annotations returns any annotation that a user could have stored through the sandbox.
func (s *Sandbox) Annotations(key string) (string, error) {
	value, exist := s.config.Annotations[key]
	if !exist {
		return "", fmt.Errorf("Annotations key %s does not exist", key)
	}

	return value, nil
}

// SetAnnotations sets or adds an annotations
func (s *Sandbox) SetAnnotations(annotations map[string]string) error {
	s.annotationsLock.Lock()
	defer s.annotationsLock.Unlock()

	for k, v := range annotations {
		s.config.Annotations[k] = v
	}

	return s.store.Store(store.Configuration, *(s.config))
}

// GetAnnotations returns sandbox's annotations
func (s *Sandbox) GetAnnotations() map[string]string {
	s.annotationsLock.RLock()
	defer s.annotationsLock.RUnlock()

	return s.config.Annotations
}

// GetNetNs returns the network namespace of the current sandbox.
func (s *Sandbox) GetNetNs() string {
	return s.networkNS.NetNsPath
}

// GetAllContainers returns all containers.
func (s *Sandbox) GetAllContainers() []VCContainer {
	ifa := make([]VCContainer, len(s.containers))

	i := 0
	for _, v := range s.containers {
		ifa[i] = v
		i++
	}

	return ifa
}

// GetContainer returns the container named by the containerID.
func (s *Sandbox) GetContainer(containerID string) VCContainer {
	for id, c := range s.containers {
		if id == containerID {
			return c
		}
	}
	return nil
}

// Release closes the agent connection and removes sandbox from internal list.
func (s *Sandbox) Release() error {
	s.Logger().Info("release sandbox")
	globalSandboxList.removeSandbox(s.id)
	if s.monitor != nil {
		s.monitor.stop()
	}
	s.hypervisor.disconnect()
	return s.agent.disconnect()
}

func (s *Sandbox) releaseStatelessSandbox() error {
	if s.stateful {
		return nil
	}

	return s.Release()
}

// Status gets the status of the sandbox
// TODO: update container status properly, see kata-containers/runtime#253
func (s *Sandbox) Status() SandboxStatus {
	var contStatusList []ContainerStatus
	for _, c := range s.containers {
		rootfs := c.config.RootFs.Source
		if c.config.RootFs.Mounted {
			rootfs = c.config.RootFs.Target
		}

		contStatusList = append(contStatusList, ContainerStatus{
			ID:          c.id,
			State:       c.state,
			PID:         c.process.Pid,
			StartTime:   c.process.StartTime,
			RootFs:      rootfs,
			Annotations: c.config.Annotations,
		})
	}

	return SandboxStatus{
		ID:               s.id,
		State:            s.state,
		Hypervisor:       s.config.HypervisorType,
		HypervisorConfig: s.config.HypervisorConfig,
		Agent:            s.config.AgentType,
		ContainersStatus: contStatusList,
		Annotations:      s.config.Annotations,
	}
}

// Monitor returns a error channel for watcher to watch at
func (s *Sandbox) Monitor() (chan error, error) {
	if s.state.State != types.StateRunning {
		return nil, fmt.Errorf("Sandbox is not running")
	}

	s.Lock()
	if s.monitor == nil {
		s.monitor = newMonitor(s)
	}
	s.Unlock()

	return s.monitor.newWatcher()
}

// WaitProcess waits on a container process and return its exit code
func (s *Sandbox) WaitProcess(containerID, processID string) (int32, error) {
	if s.state.State != types.StateRunning {
		return 0, fmt.Errorf("Sandbox not running")
	}

	c, err := s.findContainer(containerID)
	if err != nil {
		return 0, err
	}

	return c.wait(processID)
}

// SignalProcess sends a signal to a process of a container when all is false.
// When all is true, it sends the signal to all processes of a container.
func (s *Sandbox) SignalProcess(containerID, processID string, signal syscall.Signal, all bool) error {
	if s.state.State != types.StateRunning {
		return fmt.Errorf("Sandbox not running")
	}

	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	return c.signalProcess(processID, signal, all)
}

// WinsizeProcess resizes the tty window of a process
func (s *Sandbox) WinsizeProcess(containerID, processID string, height, width uint32) error {
	if s.state.State != types.StateRunning {
		return fmt.Errorf("Sandbox not running")
	}

	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	return c.winsizeProcess(processID, height, width)
}

// IOStream returns stdin writer, stdout reader and stderr reader of a process
func (s *Sandbox) IOStream(containerID, processID string) (io.WriteCloser, io.Reader, io.Reader, error) {
	if s.state.State != types.StateRunning {
		return nil, nil, nil, fmt.Errorf("Sandbox not running")
	}

	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, nil, nil, err
	}

	return c.ioStream(processID)
}

func createAssets(ctx context.Context, sandboxConfig *SandboxConfig) error {
	span, _ := trace(ctx, "createAssets")
	defer span.Finish()

	kernel, err := types.NewAsset(sandboxConfig.Annotations, types.KernelAsset)
	if err != nil {
		return err
	}

	image, err := types.NewAsset(sandboxConfig.Annotations, types.ImageAsset)
	if err != nil {
		return err
	}

	initrd, err := types.NewAsset(sandboxConfig.Annotations, types.InitrdAsset)
	if err != nil {
		return err
	}

	if image != nil && initrd != nil {
		return fmt.Errorf("%s and %s cannot be both set", types.ImageAsset, types.InitrdAsset)
	}

	for _, a := range []*types.Asset{kernel, image, initrd} {
		if err := sandboxConfig.HypervisorConfig.addCustomAsset(a); err != nil {
			return err
		}
	}

	return nil
}

func (s *Sandbox) getAndStoreGuestDetails() error {
	guestDetailRes, err := s.agent.getGuestDetails(&grpc.GuestDetailsRequest{
		MemBlockSize:    true,
		MemHotplugProbe: true,
	})
	if err != nil {
		return err
	}

	if guestDetailRes != nil {
		s.state.GuestMemoryBlockSizeMB = uint32(guestDetailRes.MemBlockSizeBytes >> 20)
		if guestDetailRes.AgentDetails != nil {
			s.seccompSupported = guestDetailRes.AgentDetails.SupportsSeccomp
		}
		s.state.GuestMemoryHotplugProbe = guestDetailRes.SupportMemHotplugProbe

		if !s.supportNewStore() {
			if err = s.store.Store(store.State, s.state); err != nil {
				return err
			}
		}
	}

	return nil
}

// createSandbox creates a sandbox from a sandbox description, the containers list, the hypervisor
// and the agent passed through the Config structure.
// It will create and store the sandbox structure, and then ask the hypervisor
// to physically create that sandbox i.e. starts a VM for that sandbox to eventually
// be started.
func createSandbox(ctx context.Context, sandboxConfig SandboxConfig, factory Factory) (*Sandbox, error) {
	span, ctx := trace(ctx, "createSandbox")
	defer span.Finish()

	if err := createAssets(ctx, &sandboxConfig); err != nil {
		return nil, err
	}

	s, err := newSandbox(ctx, sandboxConfig, factory)
	if err != nil {
		return nil, err
	}

	if len(s.config.Experimental) != 0 {
		s.Logger().WithField("features", s.config.Experimental).Infof("Enable experimental features")
	}

	// Sandbox state has been loaded from storage.
	// If the Stae is not empty, this is a re-creation, i.e.
	// we don't need to talk to the guest's agent, but only
	// want to create the sandbox and its containers in memory.
	if s.state.State != "" {
		return s, nil
	}

	// Below code path is called only during create, because of earlier check.
	if err := s.agent.createSandbox(s); err != nil {
		return nil, err
	}

	// Set sandbox state
	if err := s.setSandboxState(types.StateReady); err != nil {
		return nil, err
	}

	return s, nil
}

func newSandbox(ctx context.Context, sandboxConfig SandboxConfig, factory Factory) (*Sandbox, error) {
	span, ctx := trace(ctx, "newSandbox")
	defer span.Finish()

	if !sandboxConfig.valid() {
		return nil, fmt.Errorf("Invalid sandbox configuration")
	}

	agent := newAgent(sandboxConfig.AgentType)

	hypervisor, err := newHypervisor(sandboxConfig.HypervisorType)
	if err != nil {
		return nil, err
	}

	s := &Sandbox{
		id:              sandboxConfig.ID,
		factory:         factory,
		hypervisor:      hypervisor,
		agent:           agent,
		config:          &sandboxConfig,
		volumes:         sandboxConfig.Volumes,
		containers:      map[string]*Container{},
		runPath:         store.SandboxRuntimeRootPath(sandboxConfig.ID),
		configPath:      store.SandboxConfigurationRootPath(sandboxConfig.ID),
		state:           types.SandboxState{},
		annotationsLock: &sync.RWMutex{},
		wg:              &sync.WaitGroup{},
		shmSize:         sandboxConfig.ShmSize,
		sharePidNs:      sandboxConfig.SharePidNs,
		stateful:        sandboxConfig.Stateful,
		networkNS:       NetworkNamespace{NetNsPath: sandboxConfig.NetworkConfig.NetNSPath},
		ctx:             ctx,
	}

	vcStore, err := store.NewVCSandboxStore(ctx, s.id)
	if err != nil {
		return nil, err
	}

	s.store = vcStore

	if s.newStore, err = persist.GetDriver("fs"); err != nil || s.newStore == nil {
		return nil, fmt.Errorf("failed to get fs persist driver")
	}

	if err = globalSandboxList.addSandbox(s); err != nil {
		return nil, err
	}

	defer func() {
		if err != nil {
			s.Logger().WithError(err).WithField("sandboxid", s.id).Error("Create new sandbox failed")
			globalSandboxList.removeSandbox(s.id)
			s.store.Delete()
		}
	}()

	if s.supportNewStore() {
		s.devManager = deviceManager.NewDeviceManager(sandboxConfig.HypervisorConfig.BlockDeviceDriver, nil)

		// Ignore the error. Restore can fail for a new sandbox
		s.Restore()

		// new store doesn't require hypervisor to be stored immediately
		if err = s.hypervisor.createSandbox(ctx, s.id, s.networkNS, &sandboxConfig.HypervisorConfig, nil, s.stateful); err != nil {
			return nil, err
		}
	} else {
		// Fetch sandbox network to be able to access it from the sandbox structure.
		var networkNS NetworkNamespace
		if err = s.store.Load(store.Network, &networkNS); err == nil {
			s.networkNS = networkNS
		}

		devices, err := s.store.LoadDevices()
		if err != nil {
			s.Logger().WithError(err).WithField("sandboxid", s.id).Warning("load sandbox devices failed")
		}
		s.devManager = deviceManager.NewDeviceManager(sandboxConfig.HypervisorConfig.BlockDeviceDriver, devices)

		// Load sandbox state. The hypervisor.createSandbox call, may need to access statei.
		state, err := s.store.LoadState()
		if err == nil {
			s.state = state
		}

		if err = s.hypervisor.createSandbox(ctx, s.id, s.networkNS, &sandboxConfig.HypervisorConfig, s.store, s.stateful); err != nil {
			return nil, err
		}
	}

	agentConfig, err := newAgentConfig(sandboxConfig.AgentType, sandboxConfig.AgentConfig)
	if err != nil {
		return nil, err
	}

	if s.disableVMShutdown, err = s.agent.init(ctx, s, agentConfig); err != nil {
		return nil, err
	}

	return s, nil
}

func (s *Sandbox) storeSandboxDevices() error {
	return s.store.StoreDevices(s.devManager.GetAllDevices())
}

// storeSandbox stores a sandbox config.
func (s *Sandbox) storeSandbox() error {
	span, _ := s.trace("storeSandbox")
	defer span.Finish()

	err := s.store.Store(store.Configuration, *(s.config))
	if err != nil {
		return err
	}

	for _, container := range s.containers {
		err = container.store.Store(store.Configuration, *(container.config))
		if err != nil {
			return err
		}
	}

	if s.supportNewStore() {
		// flush data to storage
		if err := s.Save(); err != nil {
			return err
		}
	}

	return nil
}

func rLockSandbox(ctx context.Context, sandboxID string) (string, error) {
	store, err := store.NewVCSandboxStore(ctx, sandboxID)
	if err != nil {
		return "", err
	}

	return store.RLock()
}

func rwLockSandbox(ctx context.Context, sandboxID string) (string, error) {
	store, err := store.NewVCSandboxStore(ctx, sandboxID)
	if err != nil {
		return "", err
	}

	return store.Lock()
}

func unlockSandbox(ctx context.Context, sandboxID, token string) error {
	// If the store no longer exists, we won't be able to unlock.
	// Creating a new store for locking an item that does not even exist
	// does not make sense.
	if !store.VCSandboxStoreExists(ctx, sandboxID) {
		return nil
	}

	store, err := store.NewVCSandboxStore(ctx, sandboxID)
	if err != nil {
		return err
	}

	return store.Unlock(token)
}

// fetchSandbox fetches a sandbox config from a sandbox ID and returns a sandbox.
func fetchSandbox(ctx context.Context, sandboxID string) (sandbox *Sandbox, err error) {
	virtLog.Info("fetch sandbox")
	if sandboxID == "" {
		return nil, vcTypes.ErrNeedSandboxID
	}

	sandbox, err = globalSandboxList.lookupSandbox(sandboxID)
	if sandbox != nil && err == nil {
		return sandbox, err
	}

	// We're bootstrapping
	vcStore, err := store.NewVCSandboxStore(ctx, sandboxID)
	if err != nil {
		return nil, err
	}

	var config SandboxConfig
	if err := vcStore.Load(store.Configuration, &config); err != nil {
		return nil, err
	}

	// fetchSandbox is not suppose to create new sandbox VM.
	sandbox, err = createSandbox(ctx, config, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create sandbox with config %+v: %v", config, err)
	}

	// This sandbox already exists, we don't need to recreate the containers in the guest.
	// We only need to fetch the containers from storage and create the container structs.
	if err := sandbox.fetchContainers(); err != nil {
		return nil, err
	}

	return sandbox, nil
}

// findContainer returns a container from the containers list held by the
// sandbox structure, based on a container ID.
func (s *Sandbox) findContainer(containerID string) (*Container, error) {
	if s == nil {
		return nil, vcTypes.ErrNeedSandbox
	}

	if containerID == "" {
		return nil, vcTypes.ErrNeedContainerID
	}

	for id, c := range s.containers {
		if containerID == id {
			return c, nil
		}
	}

	return nil, errors.Wrapf(vcTypes.ErrNoSuchContainer, "Could not find the container %q from the sandbox %q containers list",
		containerID, s.id)
}

// removeContainer removes a container from the containers list held by the
// sandbox structure, based on a container ID.
func (s *Sandbox) removeContainer(containerID string) error {
	if s == nil {
		return vcTypes.ErrNeedSandbox
	}

	if containerID == "" {
		return vcTypes.ErrNeedContainerID
	}

	if _, ok := s.containers[containerID]; !ok {
		return errors.Wrapf(vcTypes.ErrNoSuchContainer, "Could not remove the container %q from the sandbox %q containers list",
			containerID, s.id)
	}

	delete(s.containers, containerID)

	return nil
}

// Delete deletes an already created sandbox.
// The VM in which the sandbox is running will be shut down.
func (s *Sandbox) Delete() error {
	if s.state.State != types.StateReady &&
		s.state.State != types.StatePaused &&
		s.state.State != types.StateStopped {
		return fmt.Errorf("Sandbox not ready, paused or stopped, impossible to delete")
	}

	for _, c := range s.containers {
		if err := c.delete(); err != nil {
			return err
		}
	}

	if !rootless.IsRootless() {
		if err := s.cgroupsDelete(); err != nil {
			return err
		}
	}

	globalSandboxList.removeSandbox(s.id)

	if s.monitor != nil {
		s.monitor.stop()
	}

	if err := s.hypervisor.cleanup(); err != nil {
		s.Logger().WithError(err).Error("failed to cleanup hypervisor")
	}

	s.agent.cleanup(s)

	return s.store.Delete()
}

func (s *Sandbox) startNetworkMonitor() error {
	span, _ := s.trace("startNetworkMonitor")
	defer span.Finish()

	binPath, err := os.Executable()
	if err != nil {
		return err
	}

	logLevel := "info"
	if s.config.NetworkConfig.NetmonConfig.Debug {
		logLevel = "debug"
	}

	params := netmonParams{
		netmonPath: s.config.NetworkConfig.NetmonConfig.Path,
		debug:      s.config.NetworkConfig.NetmonConfig.Debug,
		logLevel:   logLevel,
		runtime:    binPath,
		sandboxID:  s.id,
	}

	return s.network.Run(s.networkNS.NetNsPath, func() error {
		pid, err := startNetmon(params)
		if err != nil {
			return err
		}

		s.networkNS.NetmonPID = pid

		return nil
	})
}

func (s *Sandbox) createNetwork() error {
	if s.config.NetworkConfig.DisableNewNetNs ||
		s.config.NetworkConfig.NetNSPath == "" {
		return nil
	}

	span, _ := s.trace("createNetwork")
	defer span.Finish()

	s.networkNS = NetworkNamespace{
		NetNsPath:    s.config.NetworkConfig.NetNSPath,
		NetNsCreated: s.config.NetworkConfig.NetNsCreated,
	}

	// In case there is a factory, network interfaces are hotplugged
	// after vm is started.
	if s.factory == nil {
		// Add the network
		endpoints, err := s.network.Add(s.ctx, &s.config.NetworkConfig, s.hypervisor, false)
		if err != nil {
			return err
		}

		s.networkNS.Endpoints = endpoints

		if s.config.NetworkConfig.NetmonConfig.Enable {
			if err := s.startNetworkMonitor(); err != nil {
				return err
			}
		}
	}

	// Store the network
	if !s.supportNewStore() {
		return s.store.Store(store.Network, s.networkNS)
	}
	return nil
}

func (s *Sandbox) postCreatedNetwork() error {

	return s.network.PostAdd(s.ctx, &s.networkNS, s.factory != nil)
}

func (s *Sandbox) removeNetwork() error {
	span, _ := s.trace("removeNetwork")
	defer span.Finish()

	if s.config.NetworkConfig.NetmonConfig.Enable {
		if err := stopNetmon(s.networkNS.NetmonPID); err != nil {
			return err
		}
	}

	return s.network.Remove(s.ctx, &s.networkNS, s.hypervisor)
}

func (s *Sandbox) generateNetInfo(inf *vcTypes.Interface) (NetworkInfo, error) {
	hw, err := net.ParseMAC(inf.HwAddr)
	if err != nil {
		return NetworkInfo{}, err
	}

	var addrs []netlink.Addr
	for _, addr := range inf.IPAddresses {
		netlinkAddrStr := fmt.Sprintf("%s/%s", addr.Address, addr.Mask)
		netlinkAddr, err := netlink.ParseAddr(netlinkAddrStr)
		if err != nil {
			return NetworkInfo{}, fmt.Errorf("could not parse %q: %v", netlinkAddrStr, err)
		}

		addrs = append(addrs, *netlinkAddr)
	}

	return NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: netlink.LinkAttrs{
				Name:         inf.Name,
				HardwareAddr: hw,
				MTU:          int(inf.Mtu),
			},
			Type: inf.LinkType,
		},
		Addrs: addrs,
	}, nil
}

// AddInterface adds new nic to the sandbox.
func (s *Sandbox) AddInterface(inf *vcTypes.Interface) (*vcTypes.Interface, error) {
	netInfo, err := s.generateNetInfo(inf)
	if err != nil {
		return nil, err
	}

	endpoint, err := createEndpoint(netInfo, len(s.networkNS.Endpoints), s.config.NetworkConfig.InterworkingModel, nil)
	if err != nil {
		return nil, err
	}

	endpoint.SetProperties(netInfo)
	if err := doNetNS(s.networkNS.NetNsPath, func(_ ns.NetNS) error {
		s.Logger().WithField("endpoint-type", endpoint.Type()).Info("Hot attaching endpoint")
		return endpoint.HotAttach(s.hypervisor)
	}); err != nil {
		return nil, err
	}

	// Update the sandbox storage
	s.networkNS.Endpoints = append(s.networkNS.Endpoints, endpoint)
	if s.supportNewStore() {
		if err := s.Save(); err != nil {
			return nil, err
		}
	} else {
		if err := s.store.Store(store.Network, s.networkNS); err != nil {
			return nil, err
		}
	}

	// Add network for vm
	inf.PciAddr = endpoint.PciAddr()
	return s.agent.updateInterface(inf)
}

// RemoveInterface removes a nic of the sandbox.
func (s *Sandbox) RemoveInterface(inf *vcTypes.Interface) (*vcTypes.Interface, error) {
	for i, endpoint := range s.networkNS.Endpoints {
		if endpoint.HardwareAddr() == inf.HwAddr {
			s.Logger().WithField("endpoint-type", endpoint.Type()).Info("Hot detaching endpoint")
			if err := endpoint.HotDetach(s.hypervisor, s.networkNS.NetNsCreated, s.networkNS.NetNsPath); err != nil {
				return inf, err
			}
			s.networkNS.Endpoints = append(s.networkNS.Endpoints[:i], s.networkNS.Endpoints[i+1:]...)

			if s.supportNewStore() {
				if err := s.Save(); err != nil {
					return inf, err
				}
			} else {
				if err := s.store.Store(store.Network, s.networkNS); err != nil {
					return inf, err
				}
			}

			break
		}
	}
	return nil, nil
}

// ListInterfaces lists all nics and their configurations in the sandbox.
func (s *Sandbox) ListInterfaces() ([]*vcTypes.Interface, error) {
	return s.agent.listInterfaces()
}

// UpdateRoutes updates the sandbox route table (e.g. for portmapping support).
func (s *Sandbox) UpdateRoutes(routes []*vcTypes.Route) ([]*vcTypes.Route, error) {
	return s.agent.updateRoutes(routes)
}

// ListRoutes lists all routes and their configurations in the sandbox.
func (s *Sandbox) ListRoutes() ([]*vcTypes.Route, error) {
	return s.agent.listRoutes()
}

// startVM starts the VM.
func (s *Sandbox) startVM() (err error) {
	span, ctx := s.trace("startVM")
	defer span.Finish()

	s.Logger().Info("Starting VM")

	if err := s.network.Run(s.networkNS.NetNsPath, func() error {
		if s.factory != nil {
			vm, err := s.factory.GetVM(ctx, VMConfig{
				HypervisorType:   s.config.HypervisorType,
				HypervisorConfig: s.config.HypervisorConfig,
				AgentType:        s.config.AgentType,
				AgentConfig:      s.config.AgentConfig,
				ProxyType:        s.config.ProxyType,
				ProxyConfig:      s.config.ProxyConfig,
			})
			if err != nil {
				return err
			}

			return vm.assignSandbox(s)
		}

		return s.hypervisor.startSandbox(vmStartTimeout)
	}); err != nil {
		return err
	}

	defer func() {
		if err != nil {
			s.hypervisor.stopSandbox()
		}
	}()

	// In case of vm factory, network interfaces are hotplugged
	// after vm is started.
	if s.factory != nil {
		endpoints, err := s.network.Add(s.ctx, &s.config.NetworkConfig, s.hypervisor, true)
		if err != nil {
			return err
		}

		s.networkNS.Endpoints = endpoints

		if s.config.NetworkConfig.NetmonConfig.Enable {
			if err := s.startNetworkMonitor(); err != nil {
				return err
			}
		}

		if !s.supportNewStore() {
			if err := s.store.Store(store.Network, s.networkNS); err != nil {
				return err
			}
		}
	}

	s.Logger().Info("VM started")

	// Once the hypervisor is done starting the sandbox,
	// we want to guarantee that it is manageable.
	// For that we need to ask the agent to start the
	// sandbox inside the VM.
	if err := s.agent.startSandbox(s); err != nil {
		return err
	}

	s.Logger().Info("Agent started in the sandbox")

	return nil
}

// stopVM: stop the sandbox's VM
func (s *Sandbox) stopVM() error {
	span, _ := s.trace("stopVM")
	defer span.Finish()

	s.Logger().Info("Stopping sandbox in the VM")
	if err := s.agent.stopSandbox(s); err != nil {
		s.Logger().WithError(err).WithField("sandboxid", s.id).Warning("Agent did not stop sandbox")
	}

	if s.disableVMShutdown {
		// Do not kill the VM - allow the agent to shut it down
		// (only used to support static agent tracing).
		s.Logger().Info("Not stopping VM")
		return nil
	}

	s.Logger().Info("Stopping VM")
	return s.hypervisor.stopSandbox()
}

func (s *Sandbox) addContainer(c *Container) error {
	if _, ok := s.containers[c.id]; ok {
		return fmt.Errorf("Duplicated container: %s", c.id)
	}
	s.containers[c.id] = c

	return nil
}

// newContainers creates new containers structure and
// adds them to the sandbox. It does not create the containers
// in the guest. This should only be used when fetching a
// sandbox that already exists.
func (s *Sandbox) fetchContainers() error {
	for i, contConfig := range s.config.Containers {
		// Add spec from bundle path
		spec, err := compatoci.GetContainerSpec(contConfig.Annotations)
		if err != nil {
			return err
		}
		contConfig.Spec = &spec
		s.config.Containers[i] = contConfig

		c, err := newContainer(s, &s.config.Containers[i])
		if err != nil {
			return err
		}

		if err := s.addContainer(c); err != nil {
			return err
		}
	}

	return nil
}

// CreateContainer creates a new container in the sandbox
// This should be called only when the sandbox is already created.
// It will add new container config to sandbox.config.Containers
func (s *Sandbox) CreateContainer(contConfig ContainerConfig) (VCContainer, error) {
	storeAlreadyExists := store.VCContainerStoreExists(s.ctx, s.id, contConfig.ID)
	// Create the container.
	c, err := newContainer(s, &contConfig)
	if err != nil {
		return nil, err
	}

	// Update sandbox config.
	s.config.Containers = append(s.config.Containers, contConfig)

	defer func() {
		if err != nil {
			if len(s.config.Containers) > 0 {
				// delete container config
				s.config.Containers = s.config.Containers[:len(s.config.Containers)-1]
			}
			if !storeAlreadyExists {
				if delerr := c.store.Delete(); delerr != nil {
					c.Logger().WithError(delerr).WithField("cid", c.id).Error("delete store failed")
				}
			}
		}
	}()

	// Sandbox is reponsable to update VM resources needed by Containers
	err = s.updateResources()
	if err != nil {
		return nil, err
	}

	err = c.create()
	if err != nil {
		return nil, err
	}

	// Add the container to the containers list in the sandbox.
	if err = s.addContainer(c); err != nil {
		return nil, err
	}

	// Store it.
	err = c.storeContainer()
	if err != nil {
		return nil, err
	}

	if err = s.cgroupsUpdate(); err != nil {
		return nil, err
	}

	if err = s.storeSandbox(); err != nil {
		return nil, err
	}

	return c, nil
}

// StartContainer starts a container in the sandbox
func (s *Sandbox) StartContainer(containerID string) (VCContainer, error) {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	// Start it.
	err = c.start()
	if err != nil {
		return nil, err
	}

	if err = s.storeSandbox(); err != nil {
		return nil, err
	}

	s.Logger().Info("Container is started")
	//Fixme Container delete from sandbox, need to update resources

	return c, nil
}

// StopContainer stops a container in the sandbox
func (s *Sandbox) StopContainer(containerID string, force bool) (VCContainer, error) {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	// Stop it.
	if err := c.stop(force); err != nil {
		return nil, err
	}

	if err = s.storeSandbox(); err != nil {
		return nil, err
	}
	return c, nil
}

// KillContainer signals a container in the sandbox
func (s *Sandbox) KillContainer(containerID string, signal syscall.Signal, all bool) error {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	// Send a signal to the process.
	err = c.kill(signal, all)

	// SIGKILL should never fail otherwise it is
	// impossible to clean things up.
	if signal == syscall.SIGKILL {
		return nil
	}

	return err
}

// DeleteContainer deletes a container from the sandbox
func (s *Sandbox) DeleteContainer(containerID string) (VCContainer, error) {
	if containerID == "" {
		return nil, vcTypes.ErrNeedContainerID
	}

	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	// Delete it.
	err = c.delete()
	if err != nil {
		return nil, err
	}

	// Update sandbox config
	for idx, contConfig := range s.config.Containers {
		if contConfig.ID == containerID {
			s.config.Containers = append(s.config.Containers[:idx], s.config.Containers[idx+1:]...)
			break
		}
	}

	if err = s.storeSandbox(); err != nil {
		return nil, err
	}
	return c, nil
}

// ProcessListContainer lists every process running inside a specific
// container in the sandbox.
func (s *Sandbox) ProcessListContainer(containerID string, options ProcessListOptions) (ProcessList, error) {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	// Get the process list related to the container.
	return c.processList(options)
}

// StatusContainer gets the status of a container
// TODO: update container status properly, see kata-containers/runtime#253
func (s *Sandbox) StatusContainer(containerID string) (ContainerStatus, error) {
	if containerID == "" {
		return ContainerStatus{}, vcTypes.ErrNeedContainerID
	}

	for id, c := range s.containers {
		rootfs := c.config.RootFs.Source
		if c.config.RootFs.Mounted {
			rootfs = c.config.RootFs.Target
		}
		if id == containerID {
			return ContainerStatus{
				ID:          c.id,
				State:       c.state,
				PID:         c.process.Pid,
				StartTime:   c.process.StartTime,
				RootFs:      rootfs,
				Annotations: c.config.Annotations,
			}, nil
		}
	}

	return ContainerStatus{}, vcTypes.ErrNoSuchContainer
}

// EnterContainer is the virtcontainers container command execution entry point.
// EnterContainer enters an already running container and runs a given command.
func (s *Sandbox) EnterContainer(containerID string, cmd types.Cmd) (VCContainer, *Process, error) {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, nil, err
	}

	// Enter it.
	process, err := c.enter(cmd)
	if err != nil {
		return nil, nil, err
	}

	return c, process, nil
}

// UpdateContainer update a running container.
func (s *Sandbox) UpdateContainer(containerID string, resources specs.LinuxResources) error {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	err = c.update(resources)
	if err != nil {
		return err
	}

	if err := s.cgroupsUpdate(); err != nil {
		return err
	}

	if err := c.storeContainer(); err != nil {
		return err
	}
	if err = s.storeSandbox(); err != nil {
		return err
	}
	return nil
}

// StatsContainer return the stats of a running container
func (s *Sandbox) StatsContainer(containerID string) (ContainerStats, error) {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return ContainerStats{}, err
	}

	stats, err := c.stats()
	if err != nil {
		return ContainerStats{}, err
	}
	return *stats, nil
}

// Stats returns the stats of a running sandbox
func (s *Sandbox) Stats() (SandboxStats, error) {
	if s.state.CgroupPath == "" {
		return SandboxStats{}, fmt.Errorf("sandbox cgroup path is emtpy")
	}

	var path string
	var cgroupSubsystems cgroups.Hierarchy

	if s.config.SandboxCgroupOnly {
		cgroupSubsystems = cgroups.V1
		path = s.state.CgroupPath
	} else {
		cgroupSubsystems = V1NoConstraints
		path = cgroupNoConstraintsPath(s.state.CgroupPath)
	}

	cgroup, err := cgroupsLoadFunc(cgroupSubsystems, cgroups.StaticPath(path))
	if err != nil {
		return SandboxStats{}, fmt.Errorf("Could not load sandbox cgroup in %v: %v", s.state.CgroupPath, err)
	}

	metrics, err := cgroup.Stat(cgroups.ErrorHandler(cgroups.IgnoreNotExist))
	if err != nil {
		return SandboxStats{}, err
	}

	stats := SandboxStats{}

	stats.CgroupStats.CPUStats.CPUUsage.TotalUsage = metrics.CPU.Usage.Total
	stats.CgroupStats.MemoryStats.Usage.Usage = metrics.Memory.Usage.Usage
	tids, err := s.hypervisor.getThreadIDs()
	if err != nil {
		return stats, err
	}
	stats.Cpus = len(tids.vcpus)

	return stats, nil
}

// PauseContainer pauses a running container.
func (s *Sandbox) PauseContainer(containerID string) error {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	// Pause the container.
	if err := c.pause(); err != nil {
		return err
	}

	if err = s.storeSandbox(); err != nil {
		return err
	}
	return nil
}

// ResumeContainer resumes a paused container.
func (s *Sandbox) ResumeContainer(containerID string) error {
	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	// Resume the container.
	if err := c.resume(); err != nil {
		return err
	}

	if err = s.storeSandbox(); err != nil {
		return err
	}
	return nil
}

// createContainers registers all containers to the proxy, create the
// containers in the guest and starts one shim per container.
func (s *Sandbox) createContainers() error {
	span, _ := s.trace("createContainers")
	defer span.Finish()

	if err := s.updateResources(); err != nil {
		return err
	}

	for _, contConfig := range s.config.Containers {

		c, err := newContainer(s, &contConfig)
		if err != nil {
			return err
		}
		if err := c.create(); err != nil {
			return err
		}

		if err := s.addContainer(c); err != nil {
			return err
		}
	}

	if err := s.cgroupsUpdate(); err != nil {
		return err
	}
	if err := s.storeSandbox(); err != nil {
		return err
	}

	return nil
}

// Start starts a sandbox. The containers that are making the sandbox
// will be started.
func (s *Sandbox) Start() error {
	if err := s.state.ValidTransition(s.state.State, types.StateRunning); err != nil {
		return err
	}

	if err := s.setSandboxState(types.StateRunning); err != nil {
		return err
	}

	for _, c := range s.containers {
		if err := c.start(); err != nil {
			return err
		}
	}

	if err := s.storeSandbox(); err != nil {
		return err
	}

	s.Logger().Info("Sandbox is started")

	return nil
}

// Stop stops a sandbox. The containers that are making the sandbox
// will be destroyed.
// When force is true, ignore guest related stop failures.
func (s *Sandbox) Stop(force bool) error {
	span, _ := s.trace("stop")
	defer span.Finish()

	if s.state.State == types.StateStopped {
		s.Logger().Info("sandbox already stopped")
		return nil
	}

	if err := s.state.ValidTransition(s.state.State, types.StateStopped); err != nil {
		return err
	}

	for _, c := range s.containers {
		if err := c.stop(force); err != nil {
			return err
		}
	}

	if err := s.stopVM(); err != nil && !force {
		return err
	}

	if err := s.setSandboxState(types.StateStopped); err != nil {
		return err
	}

	// Remove the network.
	if err := s.removeNetwork(); err != nil && !force {
		return err
	}

	if err := s.storeSandbox(); err != nil {
		return err
	}

	return nil
}

// Pause pauses the sandbox
func (s *Sandbox) Pause() error {
	if err := s.hypervisor.pauseSandbox(); err != nil {
		return err
	}

	//After the sandbox is paused, it's needed to stop its monitor,
	//Otherwise, its monitors will receive timeout errors if it is
	//paused for a long time, thus its monitor will not tell it's a
	//crash caused timeout or just a paused timeout.
	if s.monitor != nil {
		s.monitor.stop()
	}

	if err := s.pauseSetStates(); err != nil {
		return err
	}

	if err := s.storeSandbox(); err != nil {
		return err
	}

	return nil
}

// Resume resumes the sandbox
func (s *Sandbox) Resume() error {
	if err := s.hypervisor.resumeSandbox(); err != nil {
		return err
	}

	if err := s.resumeSetStates(); err != nil {
		return err
	}

	if err := s.storeSandbox(); err != nil {
		return err
	}

	return nil
}

// list lists all sandbox running on the host.
func (s *Sandbox) list() ([]Sandbox, error) {
	return nil, nil
}

// enter runs an executable within a sandbox.
func (s *Sandbox) enter(args []string) error {
	return nil
}

// setSandboxState sets both the in-memory and on-disk state of the
// sandbox.
func (s *Sandbox) setSandboxState(state types.StateString) error {
	if state == "" {
		return vcTypes.ErrNeedState
	}

	// update in-memory state
	s.state.State = state

	// update on-disk state
	if !s.supportNewStore() {
		return s.store.Store(store.State, s.state)
	}
	return nil
}

func (s *Sandbox) pauseSetStates() error {
	// XXX: When a sandbox is paused, all its containers are forcibly
	// paused too.
	if err := s.setContainersState(types.StatePaused); err != nil {
		return err
	}

	return s.setSandboxState(types.StatePaused)
}

func (s *Sandbox) resumeSetStates() error {
	// XXX: Resuming a paused sandbox puts all containers back into the
	// running state.
	if err := s.setContainersState(types.StateRunning); err != nil {
		return err
	}

	return s.setSandboxState(types.StateRunning)
}

// getAndSetSandboxBlockIndex retrieves sandbox block index and increments it for
// subsequent accesses. This index is used to maintain the index at which a
// block device is assigned to a container in the sandbox.
func (s *Sandbox) getAndSetSandboxBlockIndex() (int, error) {
	currentIndex := s.state.BlockIndex

	// Increment so that container gets incremented block index
	s.state.BlockIndex++

	if !s.supportNewStore() {
		// experimental runtime use "persist.json" which doesn't need "state.json" anymore
		// update on-disk state
		if err := s.store.Store(store.State, s.state); err != nil {
			return -1, err
		}
	}

	return currentIndex, nil
}

// decrementSandboxBlockIndex decrements the current sandbox block index.
// This is used to recover from failure while adding a block device.
func (s *Sandbox) decrementSandboxBlockIndex() error {
	s.state.BlockIndex--

	if !s.supportNewStore() {
		// experimental runtime use "persist.json" which doesn't need "state.json" anymore
		// update on-disk state
		if err := s.store.Store(store.State, s.state); err != nil {
			return err
		}
	}

	return nil
}

func (s *Sandbox) setContainersState(state types.StateString) error {
	if state == "" {
		return vcTypes.ErrNeedState
	}

	for _, c := range s.containers {
		if err := c.setContainerState(state); err != nil {
			return err
		}
	}

	return nil
}

// togglePauseSandbox pauses a sandbox if pause is set to true, else it resumes it.
func togglePauseSandbox(ctx context.Context, sandboxID string, pause bool) (*Sandbox, error) {
	span, ctx := trace(ctx, "togglePauseSandbox")
	defer span.Finish()

	if sandboxID == "" {
		return nil, vcTypes.ErrNeedSandbox
	}

	lockFile, err := rwLockSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(ctx, sandboxID, lockFile)

	// Fetch the sandbox from storage and create it.
	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	if pause {
		err = s.Pause()
	} else {
		err = s.Resume()
	}

	if err != nil {
		return nil, err
	}

	return s, nil
}

// HotplugAddDevice is used for add a device to sandbox
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) HotplugAddDevice(device api.Device, devType config.DeviceType) error {
	span, _ := s.trace("HotplugAddDevice")
	defer span.Finish()

	switch devType {
	case config.DeviceVFIO:
		vfioDevices, ok := device.GetDeviceInfo().([]*config.VFIODev)
		if !ok {
			return fmt.Errorf("device type mismatch, expect device type to be %s", devType)
		}

		// adding a group of VFIO devices
		for _, dev := range vfioDevices {
			if _, err := s.hypervisor.hotplugAddDevice(dev, vfioDev); err != nil {
				s.Logger().
					WithFields(logrus.Fields{
						"sandbox":         s.id,
						"vfio-device-ID":  dev.ID,
						"vfio-device-BDF": dev.BDF,
					}).WithError(err).Error("failed to hotplug VFIO device")
				return err
			}
		}
		return nil
	case config.DeviceBlock:
		blockDevice, ok := device.(*drivers.BlockDevice)
		if !ok {
			return fmt.Errorf("device type mismatch, expect device type to be %s", devType)
		}
		_, err := s.hypervisor.hotplugAddDevice(blockDevice.BlockDrive, blockDev)
		return err
	case config.DeviceGeneric:
		// TODO: what?
		return nil
	}
	return nil
}

// HotplugRemoveDevice is used for removing a device from sandbox
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) HotplugRemoveDevice(device api.Device, devType config.DeviceType) error {
	switch devType {
	case config.DeviceVFIO:
		vfioDevices, ok := device.GetDeviceInfo().([]*config.VFIODev)
		if !ok {
			return fmt.Errorf("device type mismatch, expect device type to be %s", devType)
		}

		// remove a group of VFIO devices
		for _, dev := range vfioDevices {
			if _, err := s.hypervisor.hotplugRemoveDevice(dev, vfioDev); err != nil {
				s.Logger().WithError(err).
					WithFields(logrus.Fields{
						"sandbox":         s.id,
						"vfio-device-ID":  dev.ID,
						"vfio-device-BDF": dev.BDF,
					}).Error("failed to hot unplug VFIO device")
				return err
			}
		}
		return nil
	case config.DeviceBlock:
		blockDrive, ok := device.GetDeviceInfo().(*config.BlockDrive)
		if !ok {
			return fmt.Errorf("device type mismatch, expect device type to be %s", devType)
		}
		_, err := s.hypervisor.hotplugRemoveDevice(blockDrive, blockDev)
		return err
	case config.DeviceGeneric:
		// TODO: what?
		return nil
	}
	return nil
}

// GetAndSetSandboxBlockIndex is used for getting and setting virtio-block indexes
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) GetAndSetSandboxBlockIndex() (int, error) {
	return s.getAndSetSandboxBlockIndex()
}

// DecrementSandboxBlockIndex decrease block indexes
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) DecrementSandboxBlockIndex() error {
	return s.decrementSandboxBlockIndex()
}

// AppendDevice can only handle vhost user device currently, it adds a
// vhost user device to sandbox
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) AppendDevice(device api.Device) error {
	switch device.DeviceType() {
	case config.VhostUserSCSI, config.VhostUserNet, config.VhostUserBlk, config.VhostUserFS:
		return s.hypervisor.addDevice(device.GetDeviceInfo().(*config.VhostUserDeviceAttrs), vhostuserDev)
	}
	return fmt.Errorf("unsupported device type")
}

// AddDevice will add a device to sandbox
func (s *Sandbox) AddDevice(info config.DeviceInfo) (api.Device, error) {
	if s.devManager == nil {
		return nil, fmt.Errorf("device manager isn't initialized")
	}

	b, err := s.devManager.NewDevice(info)
	if err != nil {
		return nil, err
	}

	if err := s.devManager.AttachDevice(b.DeviceID(), s); err != nil {
		return nil, err
	}

	if !s.supportNewStore() {
		if err := s.storeSandboxDevices(); err != nil {
			return nil, err
		}
	}

	return b, nil
}

// updateResources will calculate the resources required for the virtual machine, and
// adjust the virtual machine sizing accordingly. For a given sandbox, it will calculate the
// number of vCPUs required based on the sum of container requests, plus default CPUs for the VM.
// Similar is done for memory. If changes in memory or CPU are made, the VM will be updated and
// the agent will online the applicable CPU and memory.
func (s *Sandbox) updateResources() error {
	if s == nil {
		return errors.New("sandbox is nil")
	}

	if s.config == nil {
		return fmt.Errorf("sandbox config is nil")
	}

	sandboxVCPUs := s.calculateSandboxCPUs()
	// Add default vcpus for sandbox
	sandboxVCPUs += s.hypervisor.hypervisorConfig().NumVCPUs

	sandboxMemoryByte := s.calculateSandboxMemory()
	// Add default / rsvd memory for sandbox.
	sandboxMemoryByte += int64(s.hypervisor.hypervisorConfig().MemorySize) << utils.MibToBytesShift

	// Update VCPUs
	s.Logger().WithField("cpus-sandbox", sandboxVCPUs).Debugf("Request to hypervisor to update vCPUs")
	oldCPUs, newCPUs, err := s.hypervisor.resizeVCPUs(sandboxVCPUs)
	if err != nil {
		return err
	}

	// If the CPUs were increased, ask agent to online them
	if oldCPUs < newCPUs {
		vcpusAdded := newCPUs - oldCPUs
		if err := s.agent.onlineCPUMem(vcpusAdded, true); err != nil {
			return err
		}
	}
	s.Logger().Debugf("Sandbox CPUs: %d", newCPUs)

	// Update Memory
	s.Logger().WithField("memory-sandbox-size-byte", sandboxMemoryByte).Debugf("Request to hypervisor to update memory")
	newMemory, updatedMemoryDevice, err := s.hypervisor.resizeMemory(uint32(sandboxMemoryByte>>utils.MibToBytesShift), s.state.GuestMemoryBlockSizeMB, s.state.GuestMemoryHotplugProbe)
	if err != nil {
		return err
	}
	s.Logger().Debugf("Sandbox memory size: %d MB", newMemory)
	if s.state.GuestMemoryHotplugProbe && updatedMemoryDevice.addr != 0 {
		// notify the guest kernel about memory hot-add event, before onlining them
		s.Logger().Debugf("notify guest kernel memory hot-add event via probe interface, memory device located at 0x%x", updatedMemoryDevice.addr)
		if err := s.agent.memHotplugByProbe(updatedMemoryDevice.addr, uint32(updatedMemoryDevice.sizeMB), s.state.GuestMemoryBlockSizeMB); err != nil {
			return err
		}
	}
	if err := s.agent.onlineCPUMem(0, false); err != nil {
		return err
	}
	return nil
}

func (s *Sandbox) calculateSandboxMemory() int64 {
	memorySandbox := int64(0)
	for _, c := range s.config.Containers {
		if m := c.Resources.Memory; m != nil && m.Limit != nil {
			memorySandbox += *m.Limit
		}
	}
	return memorySandbox
}

func (s *Sandbox) calculateSandboxCPUs() uint32 {
	mCPU := uint32(0)

	for _, c := range s.config.Containers {
		if cpu := c.Resources.CPU; cpu != nil {
			if cpu.Period != nil && cpu.Quota != nil {
				mCPU += utils.CalculateMilliCPUs(*cpu.Quota, *cpu.Period)
			}

		}
	}
	return utils.CalculateVCpusFromMilliCpus(mCPU)
}

// GetHypervisorType is used for getting Hypervisor name currently used.
// Sandbox implement DeviceReceiver interface from device/api/interface.go
func (s *Sandbox) GetHypervisorType() string {
	return string(s.config.HypervisorType)
}

// cgroupsUpdate will:
//  1) get the v1constraints cgroup associated with the stored cgroup path
//  2) (re-)add hypervisor vCPU threads to the appropriate cgroup
//  3) If we are managing sandbox cgroup, update the v1constraints cgroup size
func (s *Sandbox) cgroupsUpdate() error {

	// If Kata is configured for SandboxCgroupOnly, the VMM and its processes are already
	// in the Kata sandbox cgroup (inherited). No need to move threads/processes, and we should
	// rely on parent's cgroup CPU/memory values
	if s.config.SandboxCgroupOnly {
		return nil
	}

	if s.state.CgroupPath == "" {
		s.Logger().Warn("sandbox's cgroup won't be updated: cgroup path is empty")
		return nil
	}

	cgroup, err := cgroupsLoadFunc(V1Constraints, cgroups.StaticPath(s.state.CgroupPath))
	if err != nil {
		return fmt.Errorf("Could not load cgroup %v: %v", s.state.CgroupPath, err)
	}

	if err := s.constrainHypervisor(cgroup); err != nil {
		return err
	}

	if len(s.containers) <= 1 {
		// nothing to update
		return nil
	}

	resources, err := s.resources()
	if err != nil {
		return err
	}

	if err := cgroup.Update(&resources); err != nil {
		return fmt.Errorf("Could not update sandbox cgroup path='%v' error='%v'", s.state.CgroupPath, err)
	}

	return nil
}

// cgroupsDelete will move the running processes in the sandbox cgroup
// to the parent and then delete the sandbox cgroup
func (s *Sandbox) cgroupsDelete() error {
	s.Logger().Debug("Deleting sandbox cgroup")
	if s.state.CgroupPath == "" {
		s.Logger().Warnf("sandox cgroups path is empty")
		return nil
	}

	var path string
	var cgroupSubsystems cgroups.Hierarchy

	if s.config.SandboxCgroupOnly {
		cgroupSubsystems = cgroups.V1
		path = s.state.CgroupPath
		s.Logger().WithField("path", path).Debug("Deleting sandbox cgroups (all subsystems)")
	} else {
		cgroupSubsystems = V1NoConstraints
		path = cgroupNoConstraintsPath(s.state.CgroupPath)
		s.Logger().WithField("path", path).Debug("Deleting no constraints cgroup")
	}

	sandboxCgroups, err := cgroupsLoadFunc(cgroupSubsystems, cgroups.StaticPath(path))
	if err == cgroups.ErrCgroupDeleted {
		// cgroup already deleted
		s.Logger().Warnf("cgroup already deleted: '%s'", err)
		return nil
	}

	if err != nil {
		return fmt.Errorf("Could not load cgroups %v: %v", path, err)
	}

	// move running process here, that way cgroup can be removed
	parent, err := parentCgroup(cgroupSubsystems, path)
	if err != nil {
		// parent cgroup doesn't exist, that means there are no process running
		// and the no constraints cgroup was removed.
		s.Logger().WithError(err).Warn("Parent cgroup doesn't exist")
		return nil
	}

	if err := sandboxCgroups.MoveTo(parent); err != nil {
		// Don't fail, cgroup can be deleted
		s.Logger().WithError(err).Warnf("Could not move process from %s to parent cgroup", path)
	}

	return sandboxCgroups.Delete()
}

// constrainHypervisor will place the VMM and vCPU threads into cgroups.
func (s *Sandbox) constrainHypervisor(cgroup cgroups.Cgroup) error {
	// VMM threads are only placed into the constrained cgroup if SandboxCgroupOnly is being set.
	// This is the "correct" behavior, but if the parent cgroup isn't set up correctly to take
	// Kata/VMM into account, Kata may fail to boot due to being overconstrained.
	// If !SandboxCgroupOnly, place the VMM into an unconstrained cgroup, and the vCPU threads into constrained
	// cgroup
	if s.config.SandboxCgroupOnly {
		// Kata components were moved into the sandbox-cgroup already, so VMM
		// will already land there as well. No need to take action
		return nil
	}

	pids := s.hypervisor.getPids()
	if len(pids) == 0 || pids[0] == 0 {
		return fmt.Errorf("Invalid hypervisor PID: %+v", pids)
	}

	// VMM threads are only placed into the constrained cgroup if SandboxCgroupOnly is being set.
	// This is the "correct" behavior, but if the parent cgroup isn't set up correctly to take
	// Kata/VMM into account, Kata may fail to boot due to being overconstrained.
	// If !SandboxCgroupOnly, place the VMM into an unconstrained cgroup, and the vCPU threads into constrained
	// cgroup
	// Move the VMM into cgroups without constraints, those cgroups are not yet supported.
	resources := &specs.LinuxResources{}
	path := cgroupNoConstraintsPath(s.state.CgroupPath)
	vmmCgroup, err := cgroupsNewFunc(V1NoConstraints, cgroups.StaticPath(path), resources)
	if err != nil {
		return fmt.Errorf("Could not create cgroup %v: %v", path, err)
	}

	for _, pid := range pids {
		if pid <= 0 {
			s.Logger().Warnf("Invalid hypervisor pid: %d", pid)
			continue
		}

		if err := vmmCgroup.Add(cgroups.Process{Pid: pid}); err != nil {
			return fmt.Errorf("Could not add hypervisor PID %d to cgroup: %v", pid, err)
		}
	}

	// when new container joins, new CPU could be hotplugged, so we
	// have to query fresh vcpu info from hypervisor every time.
	tids, err := s.hypervisor.getThreadIDs()
	if err != nil {
		return fmt.Errorf("failed to get thread ids from hypervisor: %v", err)
	}
	if len(tids.vcpus) == 0 {
		// If there's no tid returned from the hypervisor, this is not
		// a bug. It simply means there is nothing to constrain, hence
		// let's return without any error from here.
		return nil
	}

	// Move vcpus (threads) into cgroups with constraints.
	// Move whole hypervisor process would be easier but the IO/network performance
	// would be over-constrained.
	for _, i := range tids.vcpus {
		// In contrast, AddTask will write thread id to `tasks`
		// After this, vcpu threads are in "vcpu" sub-cgroup, other threads in
		// qemu will be left in parent cgroup untouched.
		if err := cgroup.AddTask(cgroups.Process{
			Pid: i,
		}); err != nil {
			return err
		}
	}

	return nil
}

func (s *Sandbox) resources() (specs.LinuxResources, error) {
	resources := specs.LinuxResources{
		CPU: s.cpuResources(),
	}

	return resources, nil
}

func (s *Sandbox) cpuResources() *specs.LinuxCPU {
	// Use default period and quota if they are not specified.
	// Container will inherit the constraints from its parent.
	quota := int64(0)
	period := uint64(0)
	shares := uint64(0)
	realtimePeriod := uint64(0)
	realtimeRuntime := int64(0)

	cpu := &specs.LinuxCPU{
		Quota:           &quota,
		Period:          &period,
		Shares:          &shares,
		RealtimePeriod:  &realtimePeriod,
		RealtimeRuntime: &realtimeRuntime,
	}

	for _, c := range s.containers {
		ann := c.GetAnnotations()
		if ann[annotations.ContainerTypeKey] == string(PodSandbox) {
			// skip sandbox container
			continue
		}

		if c.config.Resources.CPU == nil {
			continue
		}

		if c.config.Resources.CPU.Shares != nil {
			shares = uint64(math.Max(float64(*c.config.Resources.CPU.Shares), float64(shares)))
		}

		if c.config.Resources.CPU.Quota != nil {
			quota += *c.config.Resources.CPU.Quota
		}

		if c.config.Resources.CPU.Period != nil {
			period = uint64(math.Max(float64(*c.config.Resources.CPU.Period), float64(period)))
		}

		if c.config.Resources.CPU.Cpus != "" {
			cpu.Cpus += c.config.Resources.CPU.Cpus + ","
		}

		if c.config.Resources.CPU.RealtimeRuntime != nil {
			realtimeRuntime += *c.config.Resources.CPU.RealtimeRuntime
		}

		if c.config.Resources.CPU.RealtimePeriod != nil {
			realtimePeriod += *c.config.Resources.CPU.RealtimePeriod
		}

		if c.config.Resources.CPU.Mems != "" {
			cpu.Mems += c.config.Resources.CPU.Mems + ","
		}
	}

	cpu.Cpus = strings.Trim(cpu.Cpus, " \n\t,")

	return validCPUResources(cpu)
}

// setupSandboxCgroup creates and joins sandbox cgroups for the sandbox config
func (s *Sandbox) setupSandboxCgroup() error {
	spec := s.GetOCISpec()

	if spec == nil {
		return errorMissingOCISpec
	}

	if spec.Linux == nil {
		// Cgroup path is optional, though expected. If not defined, skip the setup
		s.Logger().WithField("sandboxid", s.id).Warning("no cgroup path provided for pod sandbox, not creating sandbox cgroup")
		return nil
	}
	validContainerCgroup := utils.ValidCgroupPath(spec.Linux.CgroupsPath)

	// Create a Kata sandbox cgroup with the cgroup of the sandbox container as the parent
	s.state.CgroupPath = filepath.Join(filepath.Dir(validContainerCgroup), cgroupKataPrefix+"_"+s.id)
	cgroup, err := cgroupsNewFunc(cgroups.V1, cgroups.StaticPath(s.state.CgroupPath), &specs.LinuxResources{})
	if err != nil {
		return fmt.Errorf("Could not create sandbox cgroup in %v: %v", s.state.CgroupPath, err)

	}

	// Add the runtime to the Kata sandbox cgroup
	runtimePid := os.Getpid()
	if err := cgroup.Add(cgroups.Process{Pid: runtimePid}); err != nil {
		return fmt.Errorf("Could not add runtime PID %d to sandbox cgroup:  %v", runtimePid, err)
	}
	return nil
}

func (s *Sandbox) sandboxContConf() *ContainerConfig {
	var podSandboxConfig *ContainerConfig

	if s.config == nil {
		return nil
	}

	// get the container associated with the PodSandbox annotation. In Kubernetes, this
	// represents the pause container. In Docker, this is the container. We derive the
	// cgroup path from this container.
	for _, cConfig := range s.config.Containers {
		if cConfig.Annotations[annotations.ContainerTypeKey] == string(PodSandbox) {
			podSandboxConfig = &cConfig
			break
		}
	}

	if podSandboxConfig == nil {
		return nil
	}

	return podSandboxConfig
}

// GetOCISpec returns sandbox's OCI specification
func (s *Sandbox) GetOCISpec() *specs.Spec {
	conf := s.sandboxContConf()
	if conf == nil {
		return nil
	}

	// First container is sandbox container as default
	return conf.Spec
}
