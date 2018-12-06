// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"net"
	"net/url"
	"os"
	"path/filepath"
	"syscall"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"

	proxyClient "github.com/clearcontainers/proxy/client"
	"github.com/kata-containers/agent/protocols/grpc"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/pkg/hyperstart"
	ns "github.com/kata-containers/runtime/virtcontainers/pkg/nsenter"
	"github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/runtime/virtcontainers/utils"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"golang.org/x/net/context"
)

var defaultSockPathTemplates = []string{"%s/%s/hyper.sock", "%s/%s/tty.sock"}
var defaultChannelTemplate = "sh.hyper.channel.%d"
var defaultDeviceIDTemplate = "channel%d"
var defaultIDTemplate = "charch%d"
var defaultSharedDir = "/run/hyper/shared/sandboxes/"
var mountTag = "hyperShared"
var maxHostnameLen = 64

// HyperConfig is a structure storing information needed for
// hyperstart agent initialization.
type HyperConfig struct {
	SockCtlName string
	SockTtyName string
}

func (h *hyper) generateSockets(sandbox *Sandbox, c HyperConfig) {
	sandboxSocketPaths := []string{
		fmt.Sprintf(defaultSockPathTemplates[0], runStoragePath, sandbox.id),
		fmt.Sprintf(defaultSockPathTemplates[1], runStoragePath, sandbox.id),
	}

	if c.SockCtlName != "" {
		sandboxSocketPaths[0] = c.SockCtlName
	}

	if c.SockTtyName != "" {
		sandboxSocketPaths[1] = c.SockTtyName
	}

	for i := 0; i < len(sandboxSocketPaths); i++ {
		s := Socket{
			DeviceID: fmt.Sprintf(defaultDeviceIDTemplate, i),
			ID:       fmt.Sprintf(defaultIDTemplate, i),
			HostPath: sandboxSocketPaths[i],
			Name:     fmt.Sprintf(defaultChannelTemplate, i),
		}
		h.sockets = append(h.sockets, s)
	}
}

// HyperAgentState is the structure describing the data stored from this
// agent implementation.
type HyperAgentState struct {
	ProxyPid int
	URL      string
}

// hyper is the Agent interface implementation for hyperstart.
type hyper struct {
	sandbox *Sandbox
	shim    shim
	proxy   proxy
	client  *proxyClient.Client
	state   HyperAgentState

	sockets []Socket

	ctx context.Context
}

type hyperstartProxyCmd struct {
	cmd     string
	message interface{}
	token   string
}

// Logger returns a logrus logger appropriate for logging hyper messages
func (h *hyper) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "hyper")
}

func (h *hyper) buildHyperContainerProcess(cmd Cmd) (*hyperstart.Process, error) {
	var envVars []hyperstart.EnvironmentVar

	for _, e := range cmd.Envs {
		envVar := hyperstart.EnvironmentVar{
			Env:   e.Var,
			Value: e.Value,
		}

		envVars = append(envVars, envVar)
	}

	process := &hyperstart.Process{
		Terminal:         cmd.Interactive,
		Args:             cmd.Args,
		Envs:             envVars,
		Workdir:          cmd.WorkDir,
		User:             cmd.User,
		Group:            cmd.PrimaryGroup,
		AdditionalGroups: cmd.SupplementaryGroups,
		NoNewPrivileges:  cmd.NoNewPrivileges,
	}

	process.Capabilities = hyperstart.Capabilities{
		Bounding:    cmd.Capabilities.Bounding,
		Effective:   cmd.Capabilities.Effective,
		Inheritable: cmd.Capabilities.Inheritable,
		Permitted:   cmd.Capabilities.Permitted,
		Ambient:     cmd.Capabilities.Ambient,
	}

	return process, nil
}

func (h *hyper) processHyperRoute(route netlink.Route, deviceName string) *hyperstart.Route {
	gateway := route.Gw.String()
	if gateway == "<nil>" {
		gateway = ""
	} else if route.Gw.To4() == nil { // Skip IPv6 as it is not supported by hyperstart agent
		h.Logger().WithFields(logrus.Fields{
			"unsupported-route-type": "ipv6",
			"gateway":                gateway,
		}).Warn("unsupported route")
		return nil
	}

	var destination string
	if route.Dst == nil {
		destination = ""
	} else {
		destination = route.Dst.String()
		if destination == defaultRouteDest {
			destination = defaultRouteLabel
		}

		// Skip IPv6 because not supported by hyperstart
		if route.Dst.IP.To4() == nil {
			h.Logger().WithFields(logrus.Fields{
				"unsupported-route-type": "ipv6",
				"destination":            destination,
			}).Warn("unsupported route")
			return nil
		}
	}

	return &hyperstart.Route{
		Dest:    destination,
		Gateway: gateway,
		Device:  deviceName,
	}
}

func (h *hyper) buildNetworkInterfacesAndRoutes(sandbox *Sandbox) ([]hyperstart.NetworkIface, []hyperstart.Route, error) {
	if sandbox.networkNS.NetNsPath == "" {
		return []hyperstart.NetworkIface{}, []hyperstart.Route{}, nil
	}

	var ifaces []hyperstart.NetworkIface
	var routes []hyperstart.Route
	for _, endpoint := range sandbox.networkNS.Endpoints {
		var ipAddresses []hyperstart.IPAddress
		for _, addr := range endpoint.Properties().Addrs {
			// Skip IPv6 because not supported by hyperstart.
			// Skip localhost interface.
			if addr.IP.To4() == nil || addr.IP.IsLoopback() {
				continue
			}

			netMask, _ := addr.Mask.Size()

			ipAddress := hyperstart.IPAddress{
				IPAddress: addr.IP.String(),
				NetMask:   fmt.Sprintf("%d", netMask),
			}

			ipAddresses = append(ipAddresses, ipAddress)
		}

		iface := hyperstart.NetworkIface{
			NewDevice:   endpoint.Name(),
			IPAddresses: ipAddresses,
			MTU:         endpoint.Properties().Iface.MTU,
			MACAddr:     endpoint.HardwareAddr(),
		}

		ifaces = append(ifaces, iface)

		for _, r := range endpoint.Properties().Routes {
			route := h.processHyperRoute(r, endpoint.Name())
			if route == nil {
				continue
			}

			routes = append(routes, *route)
		}
	}

	return ifaces, routes, nil
}

func fsMapFromMounts(mounts []Mount) []*hyperstart.FsmapDescriptor {
	var fsmap []*hyperstart.FsmapDescriptor

	for _, m := range mounts {
		fsmapDesc := &hyperstart.FsmapDescriptor{
			Source:       m.Source,
			Path:         m.Destination,
			ReadOnly:     m.ReadOnly,
			DockerVolume: false,
		}

		fsmap = append(fsmap, fsmapDesc)
	}

	return fsmap
}

func fsMapFromDevices(c *Container) ([]*hyperstart.FsmapDescriptor, error) {
	var fsmap []*hyperstart.FsmapDescriptor
	for _, dev := range c.devices {
		device := c.sandbox.devManager.GetDeviceByID(dev.ID)
		if device == nil {
			return nil, fmt.Errorf("can't find device: %#v", dev)
		}

		d, ok := device.GetDeviceInfo().(*config.BlockDrive)
		if !ok || d == nil {
			return nil, fmt.Errorf("can't retrieve block device information")
		}

		fsmapDesc := &hyperstart.FsmapDescriptor{
			Source:       d.VirtPath,
			Path:         dev.ContainerPath,
			AbsolutePath: true,
			DockerVolume: false,
			SCSIAddr:     d.SCSIAddr,
		}
		fsmap = append(fsmap, fsmapDesc)
	}
	return fsmap, nil
}

// init is the agent initialization implementation for hyperstart.
func (h *hyper) init(ctx context.Context, sandbox *Sandbox, config interface{}) (err error) {
	// save
	h.ctx = ctx

	switch c := config.(type) {
	case HyperConfig:
		// Create agent sockets from paths provided through
		// configuration, or generate them from scratch.
		h.generateSockets(sandbox, c)

		h.sandbox = sandbox
	default:
		return fmt.Errorf("Invalid config type")
	}

	h.proxy, err = newProxy(sandbox.config.ProxyType)
	if err != nil {
		return err
	}

	h.shim, err = newShim(sandbox.config.ShimType)
	if err != nil {
		return err
	}

	// Fetch agent runtime info.
	if err := sandbox.storage.fetchAgentState(sandbox.id, &h.state); err != nil {
		h.Logger().Debug("Could not retrieve anything from storage")
	}

	return nil
}

func (h *hyper) getVMPath(id string) string {
	return filepath.Join(runStoragePath, id)
}

func (h *hyper) getSharePath(id string) string {
	return filepath.Join(defaultSharedDir, id)
}

func (h *hyper) configure(hv hypervisor, id, sharePath string, builtin bool, config interface{}) error {
	for _, socket := range h.sockets {
		err := hv.addDevice(socket, serialPortDev)
		if err != nil {
			return err
		}
	}

	// Adding the hyper shared volume.
	// This volume contains all bind mounted container bundles.
	sharedVolume := Volume{
		MountTag: mountTag,
		HostPath: sharePath,
	}

	if err := os.MkdirAll(sharedVolume.HostPath, dirMode); err != nil {
		return err
	}

	return hv.addDevice(sharedVolume, fsDev)
}

func (h *hyper) createSandbox(sandbox *Sandbox) (err error) {
	return h.configure(sandbox.hypervisor, "", h.getSharePath(sandbox.id), false, nil)
}

func (h *hyper) capabilities() capabilities {
	var caps capabilities

	// add all capabilities supported by agent
	caps.setBlockDeviceSupport()

	return caps
}

// exec is the agent command execution implementation for hyperstart.
func (h *hyper) exec(sandbox *Sandbox, c Container, cmd Cmd) (*Process, error) {
	token, err := h.attach()
	if err != nil {
		return nil, err
	}

	hyperProcess, err := h.buildHyperContainerProcess(cmd)
	if err != nil {
		return nil, err
	}

	execCommand := hyperstart.ExecCommand{
		Container: c.id,
		Process:   *hyperProcess,
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

	process, err := prepareAndStartShim(sandbox, h.shim, c.id,
		token, h.state.URL, cmd, []ns.NSType{}, enterNSList)
	if err != nil {
		return nil, err
	}

	proxyCmd := hyperstartProxyCmd{
		cmd:     hyperstart.ExecCmd,
		message: execCommand,
		token:   process.Token,
	}

	if _, err := h.sendCmd(proxyCmd); err != nil {
		return nil, err
	}

	return process, nil
}

func (h *hyper) startProxy(sandbox *Sandbox) error {
	if h.proxy.consoleWatched() {
		return nil
	}

	if h.state.URL != "" {
		h.Logger().WithFields(logrus.Fields{
			"sandbox":   sandbox.id,
			"proxy-pid": h.state.ProxyPid,
			"proxy-url": h.state.URL,
		}).Infof("proxy already started")
		return nil
	}

	// Start the proxy here
	pid, uri, err := h.proxy.start(proxyParams{
		id:     sandbox.id,
		path:   sandbox.config.ProxyConfig.Path,
		debug:  sandbox.config.ProxyConfig.Debug,
		logger: h.Logger(),
	})
	if err != nil {
		return err
	}

	defer func() {
		if err != nil {
			h.proxy.stop(pid)
		}
	}()

	// Fill agent state with proxy information, and store them.
	if err = h.setProxy(sandbox, h.proxy, pid, uri); err != nil {
		return err
	}

	h.Logger().WithField("proxy-pid", pid).Info("proxy started")

	return nil
}

// startSandbox is the agent Sandbox starting implementation for hyperstart.
func (h *hyper) startSandbox(sandbox *Sandbox) error {

	err := h.startProxy(sandbox)
	if err != nil {
		return err
	}

	if err := h.register(); err != nil {
		return err
	}

	ifaces, routes, err := h.buildNetworkInterfacesAndRoutes(sandbox)
	if err != nil {
		return err
	}

	hostname := sandbox.config.Hostname
	if len(hostname) > maxHostnameLen {
		hostname = hostname[:maxHostnameLen]
	}

	hyperSandbox := hyperstart.Sandbox{
		Hostname:   hostname,
		Containers: []hyperstart.Container{},
		Interfaces: ifaces,
		Routes:     routes,
		ShareDir:   mountTag,
	}

	proxyCmd := hyperstartProxyCmd{
		cmd:     hyperstart.StartSandbox,
		message: hyperSandbox,
	}

	_, err = h.sendCmd(proxyCmd)
	return err
}

// stopSandbox is the agent Sandbox stopping implementation for hyperstart.
func (h *hyper) stopSandbox(sandbox *Sandbox) error {
	proxyCmd := hyperstartProxyCmd{
		cmd:     hyperstart.DestroySandbox,
		message: nil,
	}

	if _, err := h.sendCmd(proxyCmd); err != nil {
		return err
	}

	if err := h.unregister(); err != nil {
		return err
	}

	if err := h.proxy.stop(h.state.ProxyPid); err != nil {
		return err
	}

	h.state.ProxyPid = -1
	h.state.URL = ""
	if err := sandbox.storage.storeAgentState(sandbox.id, h.state); err != nil {
		// ignore error
		h.Logger().WithError(err).WithField("sandbox", sandbox.id).Error("failed to clean up agent state")
	}

	return nil
}

// handleBlockVolumes handles volumes that are block device files, by
// appending the block device to the list of devices associated with the
// container.
func (h *hyper) handleBlockVolumes(c *Container) {
	for _, m := range c.mounts {
		if len(m.BlockDeviceID) > 0 {
			c.devices = append(c.devices, ContainerDevice{ID: m.BlockDeviceID})
		}
	}
}

func (h *hyper) startOneContainer(sandbox *Sandbox, c *Container) error {
	process, err := h.buildHyperContainerProcess(c.config.Cmd)
	if err != nil {
		return err
	}

	container := hyperstart.Container{
		ID:      c.id,
		Image:   c.id,
		Rootfs:  rootfsDir,
		Process: process,
	}

	container.SystemMountsInfo.BindMountDev = c.systemMountsInfo.BindMountDev

	if c.state.Fstype != "" {
		// Pass a drive name only in case of block driver
		if sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioBlock ||
			sandbox.config.HypervisorConfig.BlockDeviceDriver == config.VirtioMmio {
			driveName, err := utils.GetVirtDriveName(c.state.BlockIndex)
			if err != nil {
				return err
			}
			container.Image = driveName
		} else {
			scsiAddr, err := utils.GetSCSIAddress(c.state.BlockIndex)
			if err != nil {
				return err
			}
			container.SCSIAddr = scsiAddr
		}

		container.Fstype = c.state.Fstype
	} else {

		if err := bindMountContainerRootfs(c.ctx, defaultSharedDir, sandbox.id, c.id, c.rootFs, false); err != nil {
			bindUnmountAllRootfs(c.ctx, defaultSharedDir, sandbox)
			return err
		}
	}

	//TODO : Enter mount namespace

	// Handle container mounts
	newMounts, err := c.mountSharedDirMounts(defaultSharedDir, "")
	if err != nil {
		bindUnmountAllRootfs(c.ctx, defaultSharedDir, sandbox)
		return err
	}

	fsmap := fsMapFromMounts(newMounts)

	h.handleBlockVolumes(c)

	// Append container mounts for block devices passed with --device.
	fsmapDev, err := fsMapFromDevices(c)
	if err != nil {
		return err
	}
	fsmap = append(fsmap, fsmapDev...)

	// Assign fsmap for hyperstart to mount these at the correct location within the container
	container.Fsmap = fsmap

	proxyCmd := hyperstartProxyCmd{
		cmd:     hyperstart.NewContainer,
		message: container,
		token:   c.process.Token,
	}

	if _, err := h.sendCmd(proxyCmd); err != nil {
		return err
	}

	return nil
}

// createContainer is the agent Container creation implementation for hyperstart.
func (h *hyper) createContainer(sandbox *Sandbox, c *Container) (*Process, error) {
	token, err := h.attach()
	if err != nil {
		return nil, err
	}

	createNSList := []ns.NSType{ns.NSTypePID}

	enterNSList := []ns.Namespace{
		{
			Path: sandbox.networkNS.NetNsPath,
			Type: ns.NSTypeNet,
		},
	}

	return prepareAndStartShim(sandbox, h.shim, c.id, token,
		h.state.URL, c.config.Cmd, createNSList, enterNSList)
}

// startContainer is the agent Container starting implementation for hyperstart.
func (h *hyper) startContainer(sandbox *Sandbox, c *Container) error {
	return h.startOneContainer(sandbox, c)
}

// stopContainer is the agent Container stopping implementation for hyperstart.
func (h *hyper) stopContainer(sandbox *Sandbox, c Container) error {
	// Nothing to be done in case the container has not been started.
	if c.state.State == StateReady {
		return nil
	}

	return h.stopOneContainer(sandbox.id, c)
}

func (h *hyper) stopOneContainer(sandboxID string, c Container) error {
	removeCommand := hyperstart.RemoveCommand{
		Container: c.id,
	}

	proxyCmd := hyperstartProxyCmd{
		cmd:     hyperstart.RemoveContainer,
		message: removeCommand,
	}

	if _, err := h.sendCmd(proxyCmd); err != nil {
		return err
	}

	if err := c.unmountHostMounts(); err != nil {
		return err
	}

	if c.state.Fstype == "" {
		if err := bindUnmountContainerRootfs(c.ctx, defaultSharedDir, sandboxID, c.id); err != nil {
			return err
		}
	}

	return nil
}

// signalProcess is the agent process signal implementation for hyperstart.
func (h *hyper) signalProcess(c *Container, processID string, signal syscall.Signal, all bool) error {
	// Send the signal to the shim directly in case the container has not
	// been started yet.
	if c.state.State == StateReady {
		return signalShim(c.process.Pid, signal)
	}

	return h.killOneContainer(c.id, signal, all)
}

func (h *hyper) killOneContainer(cID string, signal syscall.Signal, all bool) error {
	killCmd := hyperstart.KillCommand{
		Container:    cID,
		Signal:       signal,
		AllProcesses: all,
	}

	proxyCmd := hyperstartProxyCmd{
		cmd:     hyperstart.KillContainer,
		message: killCmd,
	}

	if _, err := h.sendCmd(proxyCmd); err != nil {
		return err
	}

	return nil
}

func (h *hyper) processListContainer(sandbox *Sandbox, c Container, options ProcessListOptions) (ProcessList, error) {
	return h.processListOneContainer(sandbox.id, c.id, options)
}

// statsContainer is the hyperstart agent Container stats implementation. It does nothing.
func (h *hyper) statsContainer(sandbox *Sandbox, c Container) (*ContainerStats, error) {
	return &ContainerStats{}, nil
}

func (h *hyper) updateContainer(sandbox *Sandbox, c Container, resources specs.LinuxResources) error {
	// hyperstart-agent does not support update
	return nil
}

func (h *hyper) processListOneContainer(sandboxID, cID string, options ProcessListOptions) (ProcessList, error) {
	psCmd := hyperstart.PsCommand{
		Container: cID,
		Format:    options.Format,
		PsArgs:    options.Args,
	}

	proxyCmd := hyperstartProxyCmd{
		cmd:     hyperstart.PsContainer,
		message: psCmd,
	}

	response, err := h.sendCmd(proxyCmd)
	if err != nil {
		return nil, err
	}

	msg, ok := response.([]byte)
	if !ok {
		return nil, fmt.Errorf("failed to get response message from container %s sandbox %s", cID, sandboxID)
	}

	return msg, nil
}

// connectProxyRetry repeatedly tries to connect to the proxy on the specified
// address until a timeout state is reached, when it will fail.
func (h *hyper) connectProxyRetry(scheme, address string) (conn net.Conn, err error) {
	attempt := 1

	timeoutSecs := waitForProxyTimeoutSecs * time.Second

	startTime := time.Now()
	lastLogTime := startTime

	for {
		conn, err = net.Dial(scheme, address)
		if err == nil {
			// If the initial connection was unsuccessful,
			// ensure a log message is generated when successfully
			// connected.
			if attempt > 1 {
				h.Logger().WithField("attempt", fmt.Sprintf("%d", attempt)).Info("Connected to proxy")
			}

			return conn, nil
		}

		attempt++

		now := time.Now()

		delta := now.Sub(startTime)
		remaining := timeoutSecs - delta

		if remaining <= 0 {
			return nil, fmt.Errorf("failed to connect to proxy after %v: %v", timeoutSecs, err)
		}

		logDelta := now.Sub(lastLogTime)
		logDeltaSecs := logDelta / time.Second

		if logDeltaSecs >= 1 {
			h.Logger().WithError(err).WithFields(logrus.Fields{
				"attempt":             fmt.Sprintf("%d", attempt),
				"proxy-network":       scheme,
				"proxy-address":       address,
				"remaining-time-secs": fmt.Sprintf("%2.2f", remaining.Seconds()),
			}).Warning("Retrying proxy connection")

			lastLogTime = now
		}

		time.Sleep(time.Duration(100) * time.Millisecond)
	}
}

func (h *hyper) connect() error {
	if h.client != nil {
		return nil
	}

	u, err := url.Parse(h.state.URL)
	if err != nil {
		return err
	}

	if u.Scheme == "" {
		return fmt.Errorf("URL scheme cannot be empty")
	}

	address := u.Host
	if address == "" {
		if u.Path == "" {
			return fmt.Errorf("URL host and path cannot be empty")
		}

		address = u.Path
	}

	conn, err := h.connectProxyRetry(u.Scheme, address)
	if err != nil {
		return err
	}

	h.client = proxyClient.NewClient(conn)

	return nil
}

func (h *hyper) disconnect() error {
	if h.client != nil {
		h.client.Close()
		h.client = nil
	}

	return nil
}

func (h *hyper) register() error {
	if err := h.connect(); err != nil {
		return err
	}
	defer h.disconnect()

	console, err := h.sandbox.hypervisor.getSandboxConsole(h.sandbox.id)
	if err != nil {
		return err
	}

	registerVMOptions := &proxyClient.RegisterVMOptions{
		Console:      console,
		NumIOStreams: 0,
	}

	_, err = h.client.RegisterVM(h.sandbox.id, h.sockets[0].HostPath,
		h.sockets[1].HostPath, registerVMOptions)
	return err
}

func (h *hyper) unregister() error {
	if err := h.connect(); err != nil {
		return err
	}
	defer h.disconnect()

	h.client.UnregisterVM(h.sandbox.id)

	return nil
}

func (h *hyper) attach() (string, error) {
	if err := h.connect(); err != nil {
		return "", err
	}
	defer h.disconnect()

	numTokens := 1
	attachVMOptions := &proxyClient.AttachVMOptions{
		NumIOStreams: numTokens,
	}

	attachVMReturn, err := h.client.AttachVM(h.sandbox.id, attachVMOptions)
	if err != nil {
		return "", err
	}

	if len(attachVMReturn.IO.Tokens) != numTokens {
		return "", fmt.Errorf("%d tokens retrieved out of %d expected",
			len(attachVMReturn.IO.Tokens), numTokens)
	}

	return attachVMReturn.IO.Tokens[0], nil
}

func (h *hyper) sendCmd(proxyCmd hyperstartProxyCmd) (interface{}, error) {
	if err := h.connect(); err != nil {
		return nil, err
	}
	defer h.disconnect()

	attachVMOptions := &proxyClient.AttachVMOptions{
		NumIOStreams: 0,
	}

	if _, err := h.client.AttachVM(h.sandbox.id, attachVMOptions); err != nil {
		return nil, err
	}

	var tokens []string
	if proxyCmd.token != "" {
		tokens = append(tokens, proxyCmd.token)
	}

	return h.client.HyperWithTokens(proxyCmd.cmd, tokens, proxyCmd.message)
}

func (h *hyper) onlineCPUMem(cpus uint32, cpuOnly bool) error {
	// hyperstart-agent uses udev to online CPUs automatically
	return nil
}

func (h *hyper) updateInterface(inf *types.Interface) (*types.Interface, error) {
	// hyperstart-agent does not support update interface
	return nil, nil
}

func (h *hyper) listInterfaces() ([]*types.Interface, error) {
	// hyperstart-agent does not support list interfaces
	return nil, nil
}

func (h *hyper) updateRoutes(routes []*types.Route) ([]*types.Route, error) {
	// hyperstart-agent does not support update routes
	return nil, nil
}

func (h *hyper) listRoutes() ([]*types.Route, error) {
	// hyperstart-agent does not support list routes
	return nil, nil
}

func (h *hyper) check() error {
	// hyperstart-agent does not support check
	return nil
}

func (h *hyper) waitProcess(c *Container, processID string) (int32, error) {
	// hyperstart-agent does not support wait process
	return 0, nil
}

func (h *hyper) winsizeProcess(c *Container, processID string, height, width uint32) error {
	// hyperstart-agent does not support winsize process
	return nil
}

func (h *hyper) writeProcessStdin(c *Container, ProcessID string, data []byte) (int, error) {
	// hyperstart-agent does not support stdin write request
	return 0, nil
}

func (h *hyper) closeProcessStdin(c *Container, ProcessID string) error {
	// hyperstart-agent does not support stdin close request
	return nil
}

func (h *hyper) readProcessStdout(c *Container, processID string, data []byte) (int, error) {
	// hyperstart-agent does not support stdout read request
	return 0, nil
}

func (h *hyper) readProcessStderr(c *Container, processID string, data []byte) (int, error) {
	// hyperstart-agent does not support stderr read request
	return 0, nil
}

func (h *hyper) pauseContainer(sandbox *Sandbox, c Container) error {
	// hyperstart-agent does not support pause container
	return nil
}

func (h *hyper) resumeContainer(sandbox *Sandbox, c Container) error {
	// hyperstart-agent does not support resume container
	return nil
}

func (h *hyper) reseedRNG(data []byte) error {
	// hyperstart-agent does not support reseeding
	return nil
}

func (h *hyper) getAgentURL() (string, error) {
	// hyperstart-agent does not support getting agent url
	return "", nil
}

func (h *hyper) reuseAgent(agent agent) error {
	a, ok := agent.(*hyper)
	if !ok {
		return fmt.Errorf("Bug: get a wrong type of agent")
	}

	h.client = a.client

	return nil
}

func (h *hyper) setProxy(sandbox *Sandbox, proxy proxy, pid int, url string) error {
	if url == "" {
		return fmt.Errorf("invalid empty proxy url")
	}

	if h.state.URL != "" && h.state.URL != url {
		h.proxy.stop(h.state.ProxyPid)
	}

	h.proxy = proxy
	h.state.ProxyPid = pid
	h.state.URL = url
	if sandbox != nil {
		if err := sandbox.storage.storeAgentState(sandbox.id, h.state); err != nil {
			return err
		}
	}

	return nil
}

func (h *hyper) getGuestDetails(*grpc.GuestDetailsRequest) (*grpc.GuestDetailsResponse, error) {
	// hyperstart-agent does not support getGuestDetails
	return nil, nil
}

func (h *hyper) setGuestDateTime(time.Time) error {
	// hyperstart-agent does not support setGuestDateTime
	return nil
}
