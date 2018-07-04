// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	kataclient "github.com/kata-containers/agent/protocols/client"
	"github.com/kata-containers/agent/protocols/grpc"
	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	ns "github.com/kata-containers/runtime/virtcontainers/pkg/nsenter"
	"github.com/kata-containers/runtime/virtcontainers/pkg/uuid"
	"github.com/kata-containers/runtime/virtcontainers/utils"

	"github.com/gogo/protobuf/proto"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"golang.org/x/net/context"
	golangGrpc "google.golang.org/grpc"
)

var (
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
	vsockSocketScheme     = "vsock"
	kata9pDevType         = "9p"
	kataBlkDevType        = "blk"
	kataSCSIDevType       = "scsi"
	sharedDir9pOptions    = []string{"trans=virtio,version=9p2000.L", "nodev"}
	shmDir                = "shm"
	kataEphemeralDevType  = "ephemeral"
)

// KataAgentConfig is a structure storing information needed
// to reach the Kata Containers agent.
type KataAgentConfig struct {
	GRPCSocket   string
	LongLiveConn bool
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
	shim  shim
	proxy proxy

	// lock protects the client pointer
	sync.Mutex
	client *kataclient.AgentClient

	reqHandlers  map[string]reqFunc
	state        KataAgentState
	keepConn     bool
	proxyBuiltIn bool

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

func (k *kataAgent) generateVMSocket(sandbox *Sandbox, c KataAgentConfig) error {
	cid, port, err := parseVSOCKAddr(c.GRPCSocket)
	if err != nil {
		// We need to generate a host UNIX socket path for the emulated serial port.
		kataSock, err := utils.BuildSocketPath(runStoragePath, sandbox.id, defaultKataSocketName)
		if err != nil {
			return err
		}

		k.vmSocket = Socket{
			DeviceID: defaultKataDeviceID,
			ID:       defaultKataID,
			HostPath: kataSock,
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

func (k *kataAgent) init(sandbox *Sandbox, config interface{}) (err error) {
	switch c := config.(type) {
	case KataAgentConfig:
		if err := k.generateVMSocket(sandbox, c); err != nil {
			return err
		}
		k.keepConn = c.LongLiveConn
	default:
		return fmt.Errorf("Invalid config type")
	}

	k.proxy, err = newProxy(sandbox.config.ProxyType)
	if err != nil {
		return err
	}

	k.shim, err = newShim(sandbox.config.ShimType)
	if err != nil {
		return err
	}

	k.proxyBuiltIn = isProxyBuiltIn(sandbox.config.ProxyType)

	// Fetch agent runtime info.
	if err := sandbox.storage.fetchAgentState(sandbox.id, &k.state); err != nil {
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

func (k *kataAgent) createSandbox(sandbox *Sandbox) error {
	switch s := k.vmSocket.(type) {
	case Socket:
		err := sandbox.hypervisor.addDevice(s, serialPortDev)
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
		HostPath: filepath.Join(kataHostSharedDir, sandbox.id),
	}

	if err := os.MkdirAll(sharedVolume.HostPath, dirMode); err != nil {
		return err
	}

	return sandbox.hypervisor.addDevice(sharedVolume, fsDev)
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

func (k *kataAgent) exec(sandbox *Sandbox, c Container, cmd Cmd) (*Process, error) {
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

func (k *kataAgent) startProxy(sandbox *Sandbox) error {
	if k.proxy == nil {
		return errorMissingProxy
	}

	if k.proxy.consoleWatched() {
		return nil
	}

	// Get agent socket path to provide it to the proxy.
	agentURL, err := k.agentURL()
	if err != nil {
		return err
	}

	proxyParams := proxyParams{
		agentURL: agentURL,
		logger:   k.Logger().WithField("sandbox-id", sandbox.id),
	}

	// Start the proxy here
	pid, uri, err := k.proxy.start(sandbox, proxyParams)
	if err != nil {
		return err
	}

	// Fill agent state with proxy information, and store them.
	k.state.ProxyPid = pid
	k.state.URL = uri
	if err := sandbox.storage.storeAgentState(sandbox.id, k.state); err != nil {
		return err
	}

	k.Logger().WithFields(logrus.Fields{
		"sandbox-id": sandbox.id,
		"proxy-pid":  pid,
		"proxy-url":  uri,
	}).Info("proxy started")

	return nil
}

func (k *kataAgent) startSandbox(sandbox *Sandbox) error {
	err := k.startProxy(sandbox)
	if err != nil {
		return err
	}

	hostname := sandbox.config.Hostname
	if len(hostname) > maxHostnameLen {
		hostname = hostname[:maxHostnameLen]
	}

	//
	// Setup network interfaces and routes
	//
	interfaces, routes, err := k.generateInterfacesAndRoutes(sandbox.networkNS)
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

	sharedDir9pOptions = append(sharedDir9pOptions, fmt.Sprintf("msize=%d", sandbox.config.HypervisorConfig.Msize9p))

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

	storages := []*grpc.Storage{sharedVolume}

	if sandbox.shmSize > 0 {
		path := filepath.Join(kataGuestSandboxDir, shmDir)
		shmSizeOption := fmt.Sprintf("size=%d", sandbox.shmSize)

		shmStorage := &grpc.Storage{
			Driver:     kataEphemeralDevType,
			MountPoint: path,
			Source:     "shm",
			Fstype:     "tmpfs",
			Options:    []string{"noexec", "nosuid", "nodev", "mode=1777", shmSizeOption},
		}

		storages = append(storages, shmStorage)
	}

	req := &grpc.CreateSandboxRequest{
		Hostname:     hostname,
		Storages:     storages,
		SandboxPidns: sandbox.sharePidNs,
	}

	_, err = k.sendReq(req)
	return err
}

func (k *kataAgent) stopSandbox(sandbox *Sandbox) error {
	if k.proxy == nil {
		return errorMissingProxy
	}

	req := &grpc.DestroySandboxRequest{}

	if _, err := k.sendReq(req); err != nil {
		return err
	}

	return k.proxy.stop(sandbox, k.state.ProxyPid)
}

func (k *kataAgent) cleanupSandbox(sandbox *Sandbox) error {
	return os.RemoveAll(filepath.Join(kataHostSharedDir, sandbox.id))
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

func constraintGRPCSpec(grpcSpec *grpc.Spec) {
	// Disable Hooks since they have been handled on the host and there is
	// no reason to send them to the agent. It would make no sense to try
	// to apply them on the guest.
	grpcSpec.Hooks = nil

	// Disable Seccomp since they cannot be handled properly by the agent
	// until we provide a guest image with libseccomp support. More details
	// here: https://github.com/kata-containers/agent/issues/104
	grpcSpec.Linux.Seccomp = nil

	// By now only CPU constraints are supported
	// Issue: https://github.com/kata-containers/runtime/issues/158
	// Issue: https://github.com/kata-containers/runtime/issues/204
	grpcSpec.Linux.Resources.Devices = nil
	grpcSpec.Linux.Resources.Memory = nil
	grpcSpec.Linux.Resources.Pids = nil
	grpcSpec.Linux.Resources.BlockIO = nil
	grpcSpec.Linux.Resources.HugepageLimits = nil
	grpcSpec.Linux.Resources.Network = nil

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

func (k *kataAgent) appendDevices(deviceList []*grpc.Device, devices []api.Device) []*grpc.Device {
	for _, device := range devices {
		d, ok := device.(*drivers.BlockDevice)
		if !ok {
			continue
		}

		kataDevice := &grpc.Device{
			ContainerPath: d.DeviceInfo.ContainerPath,
		}

		if d.SCSIAddr == "" {
			kataDevice.Type = kataBlkDevType
			kataDevice.Id = d.PCIAddr
		} else {
			kataDevice.Type = kataSCSIDevType
			kataDevice.Id = d.SCSIAddr
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

		if err2 := bindUnmountContainerRootfs(kataHostSharedDir, c.sandbox.id, c.id); err2 != nil {
			k.Logger().WithError(err2).Error("rollback failed bindUnmountContainerRootfs()")
		}
	}
}

func (k *kataAgent) createContainer(sandbox *Sandbox, c *Container) (p *Process, err error) {
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

	// In case the container creation fails, the following defer statement
	// takes care of rolling back actions previously performed.
	defer func() {
		if err != nil {
			k.rollbackFailingContainerCreation(c)
		}
	}()

	if c.state.Fstype != "" {
		// This is a block based device rootfs.

		// Pass a drive name only in case of virtio-blk driver.
		// If virtio-scsi driver, the agent will be able to find the
		// device based on the provided address.
		if sandbox.config.HypervisorConfig.BlockDeviceDriver == VirtioBlock {
			rootfs.Driver = kataBlkDevType
			rootfs.Source = c.state.RootfsPCIAddr
		} else {
			scsiAddr, err := utils.GetSCSIAddress(c.state.BlockIndex)
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
		if err = bindMountContainerRootfs(kataHostSharedDir, sandbox.id, c.id, c.rootFs, false); err != nil {
			return nil, err
		}
	}

	ociSpec := &specs.Spec{}
	if err = json.Unmarshal([]byte(ociSpecJSON), ociSpec); err != nil {
		return nil, err
	}

	// Handle container mounts
	newMounts, err := c.mountSharedDirMounts(kataHostSharedDir, kataGuestSharedDir)
	if err != nil {
		return nil, err
	}

	// We replace all OCI mount sources that match our container mount
	// with the right source path (The guest one).
	if err = k.replaceOCIMountSource(ociSpec, newMounts); err != nil {
		return nil, err
	}

	// Append container devices for block devices passed with --device.
	ctrDevices = k.appendDevices(ctrDevices, c.devices)

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

	sharedPidNs, err := k.handlePidNamespace(grpcSpec, sandbox)
	if err != nil {
		return nil, err
	}

	// We need to constraint the spec to make sure we're not passing
	// irrelevant information to the agent.
	constraintGRPCSpec(grpcSpec)

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

	enterNSList := []ns.Namespace{
		{
			Path: sandbox.networkNS.NetNsPath,
			Type: ns.NSTypeNet,
		},
	}

	return prepareAndStartShim(sandbox, k.shim, c.id, req.ExecId,
		k.state.URL, c.config.Cmd, createNSList, enterNSList)
}

// handleBlockVolumes handles volumes that are block devices files
// by passing the block devices as Storage to the agent.
func (k *kataAgent) handleBlockVolumes(c *Container) []*grpc.Storage {

	var volumeStorages []*grpc.Storage

	for _, m := range c.mounts {
		b := m.BlockDevice

		if b == nil {
			continue
		}

		// Add the block device to the list of container devices, to make sure the
		// device is detached with detachDevices() for a container.
		c.devices = append(c.devices, b)

		vol := &grpc.Storage{}

		if c.sandbox.config.HypervisorConfig.BlockDeviceDriver == VirtioBlock {
			vol.Driver = kataBlkDevType
			vol.Source = b.VirtPath
		} else {
			vol.Driver = kataSCSIDevType
			vol.Source = b.SCSIAddr
		}

		vol.MountPoint = b.DeviceInfo.ContainerPath
		vol.Fstype = "bind"
		vol.Options = []string{"bind"}

		volumeStorages = append(volumeStorages, vol)
	}

	return volumeStorages
}

// handlePidNamespace checks if Pid namespace for a container needs to be shared with its sandbox
// pid namespace. This function also modifies the grpc spec to remove the pid namespace
// from the list of namespaces passed to the agent.
func (k *kataAgent) handlePidNamespace(grpcSpec *grpc.Spec, sandbox *Sandbox) (bool, error) {
	sharedPidNs := false
	pidIndex := -1

	for i, ns := range grpcSpec.Linux.Namespaces {
		if ns.Type != string(specs.PIDNamespace) {
			continue
		}

		pidIndex = i

		if ns.Path == "" || sandbox.state.Pid == 0 {
			break
		}

		pidNsPath := fmt.Sprintf("/proc/%d/ns/pid", sandbox.state.Pid)

		//  Check if pid namespace path is the same as the sandbox
		if ns.Path == pidNsPath {
			sharedPidNs = true
		} else {
			ln, err := filepath.EvalSymlinks(ns.Path)
			if err != nil {
				return sharedPidNs, err
			}

			// We have arbitrary pid namespace path here.
			if ln != pidNsPath {
				return sharedPidNs, fmt.Errorf("Pid namespace path %s other than sandbox %s", ln, pidNsPath)
			}
			sharedPidNs = true
		}

		break
	}

	// Remove pid namespace.
	if pidIndex >= 0 {
		grpcSpec.Linux.Namespaces = append(grpcSpec.Linux.Namespaces[:pidIndex], grpcSpec.Linux.Namespaces[pidIndex+1:]...)
	}
	return sharedPidNs, nil
}

func (k *kataAgent) startContainer(sandbox *Sandbox, c *Container) error {
	req := &grpc.StartContainerRequest{
		ContainerId: c.id,
	}

	_, err := k.sendReq(req)
	return err
}

func (k *kataAgent) stopContainer(sandbox *Sandbox, c Container) error {
	req := &grpc.RemoveContainerRequest{
		ContainerId: c.id,
	}

	if _, err := k.sendReq(req); err != nil {
		return err
	}

	if err := c.unmountHostMounts(); err != nil {
		return err
	}

	if err := bindUnmountContainerRootfs(kataHostSharedDir, sandbox.id, c.id); err != nil {
		return err
	}

	// since rootfs is umounted it's safe to remove the dir now
	rootPathParent := filepath.Join(kataHostSharedDir, sandbox.id, c.id)

	return os.RemoveAll(rootPathParent)
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

func (k *kataAgent) onlineCPUMem(cpus uint32) error {
	req := &grpc.OnlineCPUMemRequest{
		Wait:   false,
		NbCpus: cpus,
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

	// This is for the first connection only, to prevent race
	k.Lock()
	defer k.Unlock()
	if k.client != nil {
		return nil
	}

	client, err := kataclient.NewAgentClient(k.state.URL, k.proxyBuiltIn)
	if err != nil {
		return err
	}

	k.installReqFunc(client)
	k.client = client

	return nil
}

func (k *kataAgent) disconnect() error {
	k.Lock()
	defer k.Unlock()

	if k.client == nil {
		return nil
	}

	if err := k.client.Close(); err != nil && err != golangGrpc.ErrClientConnClosing {
		return err
	}

	k.client = nil
	k.reqHandlers = nil

	return nil
}

func (k *kataAgent) check() error {
	_, err := k.sendReq(&grpc.CheckRequest{})
	return err
}

func (k *kataAgent) waitProcess(c *Container, processID string) (int32, error) {
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

type reqFunc func(context.Context, interface{}, ...golangGrpc.CallOption) (interface{}, error)

func (k *kataAgent) installReqFunc(c *kataclient.AgentClient) {
	k.reqHandlers = make(map[string]reqFunc)
	k.reqHandlers["grpc.CheckRequest"] = func(ctx context.Context, req interface{}, opts ...golangGrpc.CallOption) (interface{}, error) {
		ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
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
}

func (k *kataAgent) sendReq(request interface{}) (interface{}, error) {
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

	return handler(context.Background(), request)
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
	resp, err := read(context.Background(), &grpc.ReadStreamRequest{
		ContainerId: containerID,
		ExecId:      processID,
		Len:         uint32(len(data))})
	if err == nil {
		copy(data, resp.Data)
		return len(resp.Data), nil
	}

	return 0, err
}
