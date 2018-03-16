//
// Copyright (c) 2016 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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

	proxyClient "github.com/clearcontainers/proxy/client"
	"github.com/kata-containers/runtime/virtcontainers/pkg/hyperstart"
	ns "github.com/kata-containers/runtime/virtcontainers/pkg/nsenter"
	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"
)

var defaultSockPathTemplates = []string{"%s/%s/hyper.sock", "%s/%s/tty.sock"}
var defaultChannelTemplate = "sh.hyper.channel.%d"
var defaultDeviceIDTemplate = "channel%d"
var defaultIDTemplate = "charch%d"
var defaultSharedDir = "/run/hyper/shared/pods/"
var mountTag = "hyperShared"
var maxHostnameLen = 64

const (
	unixSocket = "unix"
)

// HyperConfig is a structure storing information needed for
// hyperstart agent initialization.
type HyperConfig struct {
	SockCtlName string
	SockTtyName string
}

func (h *hyper) generateSockets(pod Pod, c HyperConfig) {
	podSocketPaths := []string{
		fmt.Sprintf(defaultSockPathTemplates[0], runStoragePath, pod.id),
		fmt.Sprintf(defaultSockPathTemplates[1], runStoragePath, pod.id),
	}

	if c.SockCtlName != "" {
		podSocketPaths[0] = c.SockCtlName
	}

	if c.SockTtyName != "" {
		podSocketPaths[1] = c.SockTtyName
	}

	for i := 0; i < len(podSocketPaths); i++ {
		s := Socket{
			DeviceID: fmt.Sprintf(defaultDeviceIDTemplate, i),
			ID:       fmt.Sprintf(defaultIDTemplate, i),
			HostPath: podSocketPaths[i],
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
	pod    Pod
	shim   shim
	proxy  proxy
	client *proxyClient.Client
	state  HyperAgentState

	sockets []Socket
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

func (h *hyper) buildNetworkInterfacesAndRoutes(pod Pod) ([]hyperstart.NetworkIface, []hyperstart.Route, error) {
	if pod.networkNS.NetNsPath == "" {
		return []hyperstart.NetworkIface{}, []hyperstart.Route{}, nil
	}

	var ifaces []hyperstart.NetworkIface
	var routes []hyperstart.Route
	for _, endpoint := range pod.networkNS.Endpoints {
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

// init is the agent initialization implementation for hyperstart.
func (h *hyper) init(pod *Pod, config interface{}) (err error) {
	switch c := config.(type) {
	case HyperConfig:
		// Create agent sockets from paths provided through
		// configuration, or generate them from scratch.
		h.generateSockets(*pod, c)

		h.pod = *pod
	default:
		return fmt.Errorf("Invalid config type")
	}

	h.proxy, err = newProxy(pod.config.ProxyType)
	if err != nil {
		return err
	}

	h.shim, err = newShim(pod.config.ShimType)
	if err != nil {
		return err
	}

	// Fetch agent runtime info.
	if err := pod.storage.fetchAgentState(pod.id, &h.state); err != nil {
		h.Logger().Debug("Could not retrieve anything from storage")
	}

	return nil
}

func (h *hyper) createPod(pod *Pod) (err error) {
	for _, socket := range h.sockets {
		err := pod.hypervisor.addDevice(socket, serialPortDev)
		if err != nil {
			return err
		}
	}

	// Adding the hyper shared volume.
	// This volume contains all bind mounted container bundles.
	sharedVolume := Volume{
		MountTag: mountTag,
		HostPath: filepath.Join(defaultSharedDir, pod.id),
	}

	if err := os.MkdirAll(sharedVolume.HostPath, dirMode); err != nil {
		return err
	}

	return pod.hypervisor.addDevice(sharedVolume, fsDev)
}

func (h *hyper) capabilities() capabilities {
	var caps capabilities

	// add all capabilities supported by agent
	caps.setBlockDeviceSupport()

	return caps
}

// exec is the agent command execution implementation for hyperstart.
func (h *hyper) exec(pod *Pod, c Container, cmd Cmd) (*Process, error) {
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

	process, err := prepareAndStartShim(pod, h.shim, c.id,
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

// startPod is the agent Pod starting implementation for hyperstart.
func (h *hyper) startPod(pod Pod) error {
	// Start the proxy here
	pid, uri, err := h.proxy.start(pod, proxyParams{})
	if err != nil {
		return err
	}

	// Fill agent state with proxy information, and store them.
	h.state.ProxyPid = pid
	h.state.URL = uri
	if err := pod.storage.storeAgentState(pod.id, h.state); err != nil {
		return err
	}

	h.Logger().WithField("proxy-pid", pid).Info("proxy started")

	if err := h.register(); err != nil {
		return err
	}

	ifaces, routes, err := h.buildNetworkInterfacesAndRoutes(pod)
	if err != nil {
		return err
	}

	hostname := pod.config.Hostname
	if len(hostname) > maxHostnameLen {
		hostname = hostname[:maxHostnameLen]
	}

	hyperPod := hyperstart.Pod{
		Hostname:   hostname,
		Containers: []hyperstart.Container{},
		Interfaces: ifaces,
		Routes:     routes,
		ShareDir:   mountTag,
	}

	proxyCmd := hyperstartProxyCmd{
		cmd:     hyperstart.StartPod,
		message: hyperPod,
	}

	_, err = h.sendCmd(proxyCmd)
	return err
}

// stopPod is the agent Pod stopping implementation for hyperstart.
func (h *hyper) stopPod(pod Pod) error {
	proxyCmd := hyperstartProxyCmd{
		cmd:     hyperstart.DestroyPod,
		message: nil,
	}

	if _, err := h.sendCmd(proxyCmd); err != nil {
		return err
	}

	if err := h.unregister(); err != nil {
		return err
	}

	return h.proxy.stop(pod, h.state.ProxyPid)
}

func (h *hyper) startOneContainer(pod Pod, c *Container) error {
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

	if c.config.Resources.CPUQuota != 0 && c.config.Resources.CPUPeriod != 0 {
		container.Constraints = hyperstart.Constraints{
			CPUQuota:  c.config.Resources.CPUQuota,
			CPUPeriod: c.config.Resources.CPUPeriod,
		}
	}

	if c.config.Resources.CPUShares != 0 {
		container.Constraints.CPUShares = c.config.Resources.CPUShares
	}

	container.SystemMountsInfo.BindMountDev = c.systemMountsInfo.BindMountDev

	if c.state.Fstype != "" {
		// Pass a drive name only in case of block driver
		if pod.config.HypervisorConfig.BlockDeviceDriver == VirtioBlock {
			driveName, err := getVirtDriveName(c.state.BlockIndex)
			if err != nil {
				return err
			}
			container.Image = driveName
		} else {
			scsiAddr, err := getSCSIAddress(c.state.BlockIndex)
			if err != nil {
				return err
			}
			container.SCSIAddr = scsiAddr
		}

		container.Fstype = c.state.Fstype
	} else {

		if err := bindMountContainerRootfs(defaultSharedDir, pod.id, c.id, c.rootFs, false); err != nil {
			bindUnmountAllRootfs(defaultSharedDir, pod)
			return err
		}
	}

	//TODO : Enter mount namespace

	// Handle container mounts
	newMounts, err := c.mountSharedDirMounts(defaultSharedDir, "")
	if err != nil {
		bindUnmountAllRootfs(defaultSharedDir, pod)
		return err
	}

	fsmap := fsMapFromMounts(newMounts)

	// Append container mounts for block devices passed with --device.
	for _, device := range c.devices {
		d, ok := device.(*BlockDevice)

		if ok {
			fsmapDesc := &hyperstart.FsmapDescriptor{
				Source:       d.VirtPath,
				Path:         d.DeviceInfo.ContainerPath,
				AbsolutePath: true,
				DockerVolume: false,
				SCSIAddr:     d.SCSIAddr,
			}
			fsmap = append(fsmap, fsmapDesc)
		}
	}

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
func (h *hyper) createContainer(pod *Pod, c *Container) (*Process, error) {
	token, err := h.attach()
	if err != nil {
		return nil, err
	}

	createNSList := []ns.NSType{ns.NSTypePID}

	enterNSList := []ns.Namespace{
		{
			Path: pod.networkNS.NetNsPath,
			Type: ns.NSTypeNet,
		},
	}

	return prepareAndStartShim(pod, h.shim, c.id, token,
		h.state.URL, c.config.Cmd, createNSList, enterNSList)
}

// startContainer is the agent Container starting implementation for hyperstart.
func (h *hyper) startContainer(pod Pod, c *Container) error {
	return h.startOneContainer(pod, c)
}

// stopContainer is the agent Container stopping implementation for hyperstart.
func (h *hyper) stopContainer(pod Pod, c Container) error {
	// Nothing to be done in case the container has not been started.
	if c.state.State == StateReady {
		return nil
	}

	return h.stopOneContainer(pod.id, c)
}

func (h *hyper) stopOneContainer(podID string, c Container) error {
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
		if err := bindUnmountContainerRootfs(defaultSharedDir, podID, c.id); err != nil {
			return err
		}
	}

	return nil
}

// killContainer is the agent process signal implementation for hyperstart.
func (h *hyper) killContainer(pod Pod, c Container, signal syscall.Signal, all bool) error {
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

func (h *hyper) processListContainer(pod Pod, c Container, options ProcessListOptions) (ProcessList, error) {
	return h.processListOneContainer(pod.id, c.id, options)
}

func (h *hyper) processListOneContainer(podID, cID string, options ProcessListOptions) (ProcessList, error) {
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
		return nil, fmt.Errorf("failed to get response message from container %s pod %s", cID, podID)
	}

	return msg, nil
}

// connectProxyRetry repeatedly tries to connect to the proxy on the specified
// address until a timeout state is reached, when it will fail.
func (h *hyper) connectProxyRetry(scheme, address string) (conn net.Conn, err error) {
	attempt := 1

	timeoutSecs := time.Duration(waitForProxyTimeoutSecs * time.Second)

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

func (h *hyper) disconnect() {
	if h.client == nil {
		return
	}

	h.client.Close()
	h.client = nil
}

func (h *hyper) register() error {
	if err := h.connect(); err != nil {
		return err
	}
	defer h.disconnect()

	registerVMOptions := &proxyClient.RegisterVMOptions{
		Console:      h.pod.hypervisor.getPodConsole(h.pod.id),
		NumIOStreams: 0,
	}

	_, err := h.client.RegisterVM(h.pod.id, h.sockets[0].HostPath,
		h.sockets[1].HostPath, registerVMOptions)
	return err
}

func (h *hyper) unregister() error {
	if err := h.connect(); err != nil {
		return err
	}
	defer h.disconnect()

	h.client.UnregisterVM(h.pod.id)

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

	attachVMReturn, err := h.client.AttachVM(h.pod.id, attachVMOptions)
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

	if _, err := h.client.AttachVM(h.pod.id, attachVMOptions); err != nil {
		return nil, err
	}

	var tokens []string
	if proxyCmd.token != "" {
		tokens = append(tokens, proxyCmd.token)
	}

	return h.client.HyperWithTokens(proxyCmd.cmd, tokens, proxyCmd.message)
}
