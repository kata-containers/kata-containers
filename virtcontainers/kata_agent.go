//
// Copyright (c) 2017 Intel Corporation
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
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"

	kataclient "github.com/kata-containers/agent/protocols/client"
	"github.com/kata-containers/agent/protocols/grpc"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	ns "github.com/kata-containers/runtime/virtcontainers/pkg/nsenter"
	"github.com/kata-containers/runtime/virtcontainers/pkg/uuid"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

var (
	defaultKataSockPathTemplate = "%s/%s/kata.sock"
	defaultKataChannel          = "agent.channel.0"
	defaultKataDeviceID         = "channel0"
	defaultKataID               = "charch0"
	errorMissingProxy           = errors.New("Missing proxy pointer")
	errorMissingOCISpec         = errors.New("Missing OCI specification")
	kataHostSharedDir           = "/run/kata-containers/shared/pods/"
	kataGuestSharedDir          = "/run/kata-containers/shared/containers/"
	mountGuest9pTag             = "kataShared"
	type9pFs                    = "9p"
	devPath                     = "/dev"
	vsockSocketScheme           = "vsock"
	kata9pDevType               = "9p"
	kataBlkDevType              = "blk"
	kataSCSIDevType             = "scsi"
	sharedDir9pOptions          = []string{"trans=virtio,version=9p2000.L", "nodev"}
)

// KataAgentConfig is a structure storing information needed
// to reach the Kata Containers agent.
type KataAgentConfig struct {
	GRPCSocket string
}

type kataVSOCK struct {
	contextID uint32
	port      uint32
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
	shim   shim
	proxy  proxy
	client *kataclient.AgentClient
	state  KataAgentState

	vmSocket interface{}
}

func (k *kataAgent) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "kata_agent")
}

func parseVSOCKAddr(sock string) (uint32, uint32, error) {
	sp := strings.Split(sock, ":")
	if len(sp) != 3 {
		return 0, 0, fmt.Errorf("Invalid vsock address: %s", sock)
	}
	if sp[0] != vsockSocketScheme {
		return 0, 0, fmt.Errorf("Invalid vsock URL scheme: %s", sp[0])
	}

	cid, err := strconv.ParseUint(sp[1], 10, 32)
	if err != nil {
		return 0, 0, fmt.Errorf("Invalid vsock cid: %s", sp[1])
	}
	port, err := strconv.ParseUint(sp[2], 10, 32)
	if err != nil {
		return 0, 0, fmt.Errorf("Invalid vsock port: %s", sp[2])
	}

	return uint32(cid), uint32(port), nil
}

func (k *kataAgent) generateVMSocket(pod Pod, c KataAgentConfig) error {
	cid, port, err := parseVSOCKAddr(c.GRPCSocket)
	if err != nil {
		// We need to generate a host UNIX socket path for the emulated serial port.
		k.vmSocket = Socket{
			DeviceID: defaultKataDeviceID,
			ID:       defaultKataID,
			HostPath: fmt.Sprintf(defaultKataSockPathTemplate, runStoragePath, pod.id),
			Name:     defaultKataChannel,
		}
	} else {
		// We want to go through VSOCK. The VM VSOCK endpoint will be our gRPC.
		k.vmSocket = kataVSOCK{
			contextID: cid,
			port:      port,
		}
	}

	return nil
}

func (k *kataAgent) init(pod *Pod, config interface{}) (err error) {
	switch c := config.(type) {
	case KataAgentConfig:
		if err := k.generateVMSocket(*pod, c); err != nil {
			return err
		}
	default:
		return fmt.Errorf("Invalid config type")
	}

	k.proxy, err = newProxy(pod.config.ProxyType)
	if err != nil {
		return err
	}

	k.shim, err = newShim(pod.config.ShimType)
	if err != nil {
		return err
	}

	// Fetch agent runtime info.
	if err := pod.storage.fetchAgentState(pod.id, &k.state); err != nil {
		k.Logger().Debug("Could not retrieve anything from storage")
	}

	return nil
}

func (k *kataAgent) agentURL() (string, error) {
	switch s := k.vmSocket.(type) {
	case Socket:
		return s.HostPath, nil
	case kataVSOCK:
		return s.String(), nil
	default:
		return "", fmt.Errorf("Invalid socket type")
	}
}

func (k *kataAgent) capabilities() capabilities {
	var caps capabilities

	// add all capabilities supported by agent
	caps.setBlockDeviceSupport()

	return caps
}

func (k *kataAgent) createPod(pod *Pod) error {
	switch s := k.vmSocket.(type) {
	case Socket:
		err := pod.hypervisor.addDevice(s, serialPortDev)
		if err != nil {
			return err
		}
	case kataVSOCK:
		// TODO Add an hypervisor vsock
	default:
		return fmt.Errorf("Invalid config type")
	}

	// Adding the shared volume.
	// This volume contains all bind mounted container bundles.
	sharedVolume := Volume{
		MountTag: mountGuest9pTag,
		HostPath: filepath.Join(kataHostSharedDir, pod.id),
	}

	if err := os.MkdirAll(sharedVolume.HostPath, dirMode); err != nil {
		return err
	}

	return pod.hypervisor.addDevice(sharedVolume, fsDev)
}

func cmdToKataProcess(cmd Cmd) (process *grpc.Process, err error) {
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

func cmdEnvsToStringSlice(ev []EnvVar) []string {
	var env []string

	for _, e := range ev {
		pair := []string{e.Var, e.Value}
		env = append(env, strings.Join(pair, "="))
	}

	return env
}

func (k *kataAgent) exec(pod *Pod, c Container, cmd Cmd) (*Process, error) {
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

	return prepareAndStartShim(pod, k.shim, c.id, req.ExecId,
		k.state.URL, cmd, []ns.NSType{}, enterNSList)
}

func (k *kataAgent) generateInterfacesAndRoutes(networkNS NetworkNamespace) ([]*grpc.Interface, []*grpc.Route, error) {

	if networkNS.NetNsPath == "" {
		return nil, nil, nil
	}

	var routes []*grpc.Route
	var ifaces []*grpc.Interface

	for _, endpoint := range networkNS.Endpoints {

		var ipAddresses []*grpc.IPAddress
		for _, addr := range endpoint.Properties().Addrs {
			// Skip IPv6 because not supported
			if addr.IP.To4() == nil {
				// Skip IPv6 because not supported
				k.Logger().WithFields(logrus.Fields{
					"unsupported-address-type": "ipv6",
					"address":                  addr,
				}).Warn("unsupported address")
				continue
			}
			// Skip localhost interface
			if addr.IP.IsLoopback() {
				continue
			}
			netMask, _ := addr.Mask.Size()
			ipAddress := grpc.IPAddress{
				Family:  grpc.IPFamily_v4,
				Address: addr.IP.String(),
				Mask:    fmt.Sprintf("%d", netMask),
			}
			ipAddresses = append(ipAddresses, &ipAddress)
		}
		ifc := grpc.Interface{
			IPAddresses: ipAddresses,
			Device:      endpoint.Name(),
			Name:        endpoint.Name(),
			Mtu:         uint64(endpoint.Properties().Iface.MTU),
			HwAddr:      endpoint.HardwareAddr(),
		}

		ifaces = append(ifaces, &ifc)

		for _, route := range endpoint.Properties().Routes {
			var r grpc.Route

			if route.Dst != nil {
				r.Dest = route.Dst.String()

				if route.Dst.IP.To4() == nil {
					// Skip IPv6 because not supported
					k.Logger().WithFields(logrus.Fields{
						"unsupported-route-type": "ipv6",
						"destination":            r.Dest,
					}).Warn("unsupported route")
					continue
				}
			}

			if route.Gw != nil {
				gateway := route.Gw.String()

				if route.Gw.To4() == nil {
					// Skip IPv6 because is is not supported
					k.Logger().WithFields(logrus.Fields{
						"unsupported-route-type": "ipv6",
						"gateway":                gateway,
					}).Warn("unsupported route")
					continue
				}
				r.Gateway = gateway
			}

			if route.Src != nil {
				r.Source = route.Src.String()
			}

			r.Device = endpoint.Name()
			r.Scope = uint32(route.Scope)
			routes = append(routes, &r)

		}
	}
	return ifaces, routes, nil
}

func (k *kataAgent) startPod(pod Pod) error {
	if k.proxy == nil {
		return errorMissingProxy
	}

	// Get agent socket path to provide it to the proxy.
	agentURL, err := k.agentURL()
	if err != nil {
		return err
	}

	proxyParams := proxyParams{
		agentURL: agentURL,
	}

	// Start the proxy here
	pid, uri, err := k.proxy.start(pod, proxyParams)
	if err != nil {
		return err
	}

	// Fill agent state with proxy information, and store them.
	k.state.ProxyPid = pid
	k.state.URL = uri
	if err := pod.storage.storeAgentState(pod.id, k.state); err != nil {
		return err
	}

	k.Logger().WithField("proxy-pid", pid).Info("proxy started")

	hostname := pod.config.Hostname
	if len(hostname) > maxHostnameLen {
		hostname = hostname[:maxHostnameLen]
	}

	//
	// Setup network interfaces and routes
	//
	interfaces, routes, err := k.generateInterfacesAndRoutes(pod.networkNS)
	if err != nil {
		return err
	}
	for _, ifc := range interfaces {
		// send update interface request
		ifcReq := &grpc.UpdateInterfaceRequest{
			Interface: ifc,
		}
		resultingInterface, err := k.sendReq(ifcReq)
		if err != nil {
			k.Logger().WithFields(logrus.Fields{
				"interface-requested": fmt.Sprintf("%+v", ifc),
				"resulting-interface": fmt.Sprintf("%+v", resultingInterface),
			}).WithError(err).Error("update interface request failed")
			return err
		}
	}

	if routes != nil {
		routesReq := &grpc.UpdateRoutesRequest{
			Routes: &grpc.Routes{
				Routes: routes,
			},
		}

		resultingRoutes, err := k.sendReq(routesReq)
		if err != nil {
			k.Logger().WithFields(logrus.Fields{
				"routes-requested": fmt.Sprintf("%+v", routes),
				"resulting-routes": fmt.Sprintf("%+v", resultingRoutes),
			}).WithError(err).Error("update routes request failed")
			return err
		}
	}

	// We mount the shared directory in a predefined location
	// in the guest.
	// This is where at least some of the host config files
	// (resolv.conf, etc...) and potentially all container
	// rootfs will reside.
	sharedVolume := &grpc.Storage{
		Driver:     kata9pDevType,
		Source:     mountGuest9pTag,
		MountPoint: kataGuestSharedDir,
		Fstype:     type9pFs,
		Options:    sharedDir9pOptions,
	}

	req := &grpc.CreateSandboxRequest{
		Hostname:     hostname,
		Storages:     []*grpc.Storage{sharedVolume},
		SandboxPidns: false,
	}

	_, err = k.sendReq(req)
	return err
}

func (k *kataAgent) stopPod(pod Pod) error {
	if k.proxy == nil {
		return errorMissingProxy
	}

	req := &grpc.DestroySandboxRequest{}

	if _, err := k.sendReq(req); err != nil {
		return err
	}

	return k.proxy.stop(pod, k.state.ProxyPid)
}

func appendStorageFromMounts(storage []*grpc.Storage, mounts []*Mount) []*grpc.Storage {
	for _, m := range mounts {
		s := &grpc.Storage{
			Source:     m.Source,
			MountPoint: m.Destination,
			Fstype:     m.Type,
			Options:    m.Options,
		}

		storage = append(storage, s)
	}

	return storage
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

func constraintGRPCSpec(grpcSpec *grpc.Spec) {
	// Disable Hooks since they have been handled on the host and there is
	// no reason to send them to the agent. It would make no sense to try
	// to apply them on the guest.
	grpcSpec.Hooks = nil

	// Disable Seccomp since they cannot be handled properly by the agent
	// until we provide a guest image with libseccomp support. More details
	// here: https://github.com/kata-containers/agent/issues/104
	grpcSpec.Linux.Seccomp = nil

	// TODO: Remove this constraint as soon as the agent properly handles
	// resources provided through the specification.
	grpcSpec.Linux.Resources = nil

	// Disable network namespace since it is already handled on the host by
	// virtcontainers. The network is a complex part which cannot be simply
	// passed to the agent.
	// Every other namespaces's paths have to be emptied. This way, there
	// is no confusion from the agent, trying to find an existing namespace
	// on the guest.
	var tmpNamespaces []grpc.LinuxNamespace
	for _, ns := range grpcSpec.Linux.Namespaces {
		switch ns.Type {
		case specs.NetworkNamespace:
		default:
			ns.Path = ""
			tmpNamespaces = append(tmpNamespaces, ns)
		}
	}
	grpcSpec.Linux.Namespaces = tmpNamespaces

	// Handle /dev/shm mount
	for idx, mnt := range grpcSpec.Mounts {
		if mnt.Destination == "/dev/shm" {
			grpcSpec.Mounts[idx].Type = "tmpfs"
			grpcSpec.Mounts[idx].Source = "shm"
			grpcSpec.Mounts[idx].Options = []string{"noexec", "nosuid", "nodev", "mode=1777", "size=65536k"}

			break
		}
	}
}

func (k *kataAgent) appendDevices(deviceList []*grpc.Device, devices []Device) []*grpc.Device {
	for _, device := range devices {
		d, ok := device.(*BlockDevice)
		if !ok {
			continue
		}

		kataDevice := &grpc.Device{
			ContainerPath: d.DeviceInfo.ContainerPath,
		}

		if d.SCSIAddr == "" {
			kataDevice.Type = kataBlkDevType
			kataDevice.VmPath = d.VirtPath
		} else {
			kataDevice.Type = kataSCSIDevType
			kataDevice.Id = d.SCSIAddr
		}

		deviceList = append(deviceList, kataDevice)
	}

	return deviceList
}

func (k *kataAgent) createContainer(pod *Pod, c *Container) (*Process, error) {
	ociSpecJSON, ok := c.config.Annotations[vcAnnotations.ConfigJSONKey]
	if !ok {
		return nil, errorMissingOCISpec
	}

	var ctrStorages []*grpc.Storage
	var ctrDevices []*grpc.Device

	// The rootfs storage volume represents the container rootfs
	// mount point inside the guest.
	// It can be a block based device (when using block based container
	// overlay on the host) mount or a 9pfs one (for all other overlay
	// implementations).
	rootfs := &grpc.Storage{}

	// This is the guest absolute root path for that container.
	rootPathParent := filepath.Join(kataGuestSharedDir, c.id)
	rootPath := filepath.Join(rootPathParent, rootfsDir)

	if c.state.Fstype != "" {
		// This is a block based device rootfs.

		// Pass a drive name only in case of virtio-blk driver.
		// If virtio-scsi driver, the agent will be able to find the
		// device based on the provided address.
		if pod.config.HypervisorConfig.BlockDeviceDriver == VirtioBlock {
			// driveName is the predicted virtio-block guest name (the vd* in /dev/vd*).
			driveName, err := getVirtDriveName(c.state.BlockIndex)
			if err != nil {
				return nil, err
			}
			virtPath := filepath.Join(devPath, driveName)

			rootfs.Driver = kataBlkDevType
			rootfs.Source = virtPath
		} else {
			scsiAddr, err := getSCSIAddress(c.state.BlockIndex)
			if err != nil {
				return nil, err
			}

			rootfs.Driver = kataSCSIDevType
			rootfs.Source = scsiAddr
		}

		rootfs.MountPoint = rootPathParent
		rootfs.Fstype = c.state.Fstype

		if c.state.Fstype == "xfs" {
			rootfs.Options = []string{"nouuid"}
		}

		// Add rootfs to the list of container storage.
		// We only need to do this for block based rootfs, as we
		// want the agent to mount it into the right location
		// (kataGuestSharedDir/ctrID/
		ctrStorages = append(ctrStorages, rootfs)

	} else {
		// This is not a block based device rootfs.
		// We are going to bind mount it into the 9pfs
		// shared drive between the host and the guest.
		// With 9pfs we don't need to ask the agent to
		// mount the rootfs as the shared directory
		// (kataGuestSharedDir) is already mounted in the
		// guest. We only need to mount the rootfs from
		// the host and it will show up in the guest.
		if err := bindMountContainerRootfs(kataHostSharedDir, pod.id, c.id, c.rootFs, false); err != nil {
			bindUnmountAllRootfs(kataHostSharedDir, *pod)
			return nil, err
		}
	}

	ociSpec := &specs.Spec{}
	if err := json.Unmarshal([]byte(ociSpecJSON), ociSpec); err != nil {
		return nil, err
	}

	// Handle container mounts
	newMounts, err := c.mountSharedDirMounts(kataHostSharedDir, kataGuestSharedDir)
	if err != nil {
		bindUnmountAllRootfs(kataHostSharedDir, *pod)
		return nil, err
	}

	// We replace all OCI mount sources that match our container mount
	// with the right source path (The guest one).
	if err := k.replaceOCIMountSource(ociSpec, newMounts); err != nil {
		return nil, err
	}

	grpcSpec, err := grpc.OCItoGRPC(ociSpec)
	if err != nil {
		return nil, err
	}

	// We need to give the OCI spec our absolute rootfs path in the guest.
	grpcSpec.Root.Path = rootPath

	// We need to constraint the spec to make sure we're not passing
	// irrelevant information to the agent.
	constraintGRPCSpec(grpcSpec)

	// Append container devices for block devices passed with --device.
	ctrDevices = k.appendDevices(ctrDevices, c.devices)

	req := &grpc.CreateContainerRequest{
		ContainerId: c.id,
		ExecId:      c.id,
		Storages:    ctrStorages,
		Devices:     ctrDevices,
		OCI:         grpcSpec,
	}

	if _, err := k.sendReq(req); err != nil {
		return nil, err
	}

	createNSList := []ns.NSType{ns.NSTypePID}

	enterNSList := []ns.Namespace{
		{
			Path: pod.networkNS.NetNsPath,
			Type: ns.NSTypeNet,
		},
	}

	return prepareAndStartShim(pod, k.shim, c.id, req.ExecId,
		k.state.URL, c.config.Cmd, createNSList, enterNSList)
}

func (k *kataAgent) startContainer(pod Pod, c *Container) error {
	req := &grpc.StartContainerRequest{
		ContainerId: c.id,
	}

	_, err := k.sendReq(req)
	return err
}

func (k *kataAgent) stopContainer(pod Pod, c Container) error {
	req := &grpc.RemoveContainerRequest{
		ContainerId: c.id,
	}

	if _, err := k.sendReq(req); err != nil {
		return err
	}

	if err := c.unmountHostMounts(); err != nil {
		return err
	}

	if err := bindUnmountContainerRootfs(kataHostSharedDir, pod.id, c.id); err != nil {
		return err
	}

	return nil
}

func (k *kataAgent) killContainer(pod Pod, c Container, signal syscall.Signal, all bool) error {
	req := &grpc.SignalProcessRequest{
		ContainerId: c.id,
		ExecId:      c.process.Token,
		Signal:      uint32(signal),
	}

	_, err := k.sendReq(req)
	return err
}

func (k *kataAgent) processListContainer(pod Pod, c Container, options ProcessListOptions) (ProcessList, error) {
	return nil, nil
}

func (k *kataAgent) connect() error {
	if k.client != nil {
		return nil
	}

	client, err := kataclient.NewAgentClient(k.state.URL)
	if err != nil {
		return err
	}

	k.client = client

	return nil
}

func (k *kataAgent) disconnect() error {
	if k.client == nil {
		return nil
	}

	if err := k.client.Close(); err != nil {
		return err
	}

	k.client = nil

	return nil
}

func (k *kataAgent) sendReq(request interface{}) (interface{}, error) {
	if err := k.connect(); err != nil {
		return nil, err
	}
	defer k.disconnect()

	switch req := request.(type) {
	case *grpc.ExecProcessRequest:
		_, err := k.client.ExecProcess(context.Background(), req)
		return nil, err
	case *grpc.CreateSandboxRequest:
		_, err := k.client.CreateSandbox(context.Background(), req)
		return nil, err
	case *grpc.DestroySandboxRequest:
		_, err := k.client.DestroySandbox(context.Background(), req)
		return nil, err
	case *grpc.CreateContainerRequest:
		_, err := k.client.CreateContainer(context.Background(), req)
		return nil, err
	case *grpc.StartContainerRequest:
		_, err := k.client.StartContainer(context.Background(), req)
		return nil, err
	case *grpc.RemoveContainerRequest:
		_, err := k.client.RemoveContainer(context.Background(), req)
		return nil, err
	case *grpc.SignalProcessRequest:
		_, err := k.client.SignalProcess(context.Background(), req)
		return nil, err
	case *grpc.UpdateRoutesRequest:
		_, err := k.client.UpdateRoutes(context.Background(), req)
		return nil, err
	case *grpc.UpdateInterfaceRequest:
		ifc, err := k.client.UpdateInterface(context.Background(), req)
		return ifc, err
	default:
		return nil, fmt.Errorf("Unknown gRPC type %T", req)
	}
}
