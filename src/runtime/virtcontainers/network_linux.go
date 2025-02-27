// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"encoding/json"
	"fmt"
	"math/rand"
	"net"
	"os"
	"os/exec"
	"regexp"
	"runtime"
	"sort"
	"strconv"
	"time"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
	otelTrace "go.opentelemetry.io/otel/trace"
	"golang.org/x/sys/unix"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	vctypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// Introduces constants related to networking
const (
	defaultFilePerms = 0600
	defaultQlen      = 1500
)

// LinuxNetwork represents a sandbox networking setup.
type LinuxNetwork struct {
	netNSPath         string
	eps               []Endpoint
	interworkingModel NetInterworkingModel
	netNSCreated      bool
	danConfigPath     string
}

// NewNetwork creates a new Linux Network from a NetworkConfig.
// The constructor is overloaded as it can be called with 0 or 1
// argument. The former is used to create empty networks, mostly
// for unit testing. Passing more than one NetworkConfig pointer
// will make the constructor fail.
func NewNetwork(configs ...*NetworkConfig) (Network, error) {
	if len(configs) > 1 {
		return nil, fmt.Errorf("Too many network configurations")
	}

	// Empty constructor
	if len(configs) == 0 {
		return &LinuxNetwork{}, nil
	}

	config := configs[0]
	if config == nil {
		return nil, fmt.Errorf("Missing network configuration")
	}

	return &LinuxNetwork{
		config.NetworkID,
		[]Endpoint{},
		config.InterworkingModel,
		config.NetworkCreated,
		config.DanConfigPath,
	}, nil
}

func LoadNetwork(netInfo persistapi.NetworkInfo) Network {
	network := LinuxNetwork{
		netNSPath:    netInfo.NetworkID,
		netNSCreated: netInfo.NetworkCreated,
	}

	for _, e := range netInfo.Endpoints {
		var ep Endpoint
		switch EndpointType(e.Type) {
		case PhysicalEndpointType:
			ep = &PhysicalEndpoint{}
		case VethEndpointType:
			ep = &VethEndpoint{}
		case VhostUserEndpointType:
			ep = &VhostUserEndpoint{}
		case MacvlanEndpointType:
			ep = &MacvlanEndpoint{}
		case MacvtapEndpointType:
			ep = &MacvtapEndpoint{}
		case TapEndpointType:
			ep = &TapEndpoint{}
		case IPVlanEndpointType:
			ep = &IPVlanEndpoint{}
		default:
			networkLogger().WithField("endpoint-type", e.Type).Error("unknown endpoint type")
			continue
		}
		ep.load(e)
		network.eps = append(network.eps, ep)
	}

	return &network
}

func (n *LinuxNetwork) trace(ctx context.Context, name string) (otelTrace.Span, context.Context) {
	return networkTrace(ctx, name, nil)
}

func (n *LinuxNetwork) addSingleEndpoint(ctx context.Context, s *Sandbox, netInfo NetworkInfo, hotplug bool) (Endpoint, error) {
	var endpoint Endpoint
	// TODO: This is the incoming interface
	// based on the incoming interface we should create
	// an appropriate EndPoint based on interface type
	// This should be a switch

	// Check if interface is a physical interface. Do not create
	// tap interface/bridge if it is.
	isPhysical, err := isPhysicalIface(netInfo.Iface.Name)
	if err != nil {
		return nil, err
	}

	if isPhysical {
		if s.config.HypervisorConfig.ColdPlugVFIO == config.NoPort {
			// When `cold_plug_vfio` is set to "no-port", the PhysicalEndpoint's VFIO device cannot be attached to the guest VM.
			// Fail early to prevent the VF interface from being unbound and rebound to the VFIO driver.
			return nil, fmt.Errorf("unable to add PhysicalEndpoint %s because cold_plug_vfio is disabled", netInfo.Iface.Name)
		}
		networkLogger().WithField("interface", netInfo.Iface.Name).Info("Physical network interface found")
		endpoint, err = createPhysicalEndpoint(netInfo)
	} else {
		var socketPath string
		idx := len(n.eps)

		// Avoid endpoint naming conflicts
		// When creating a new endpoint, we check existing endpoint names and automatically adjust the naming of the new endpoint to ensure uniqueness.
		lastIdx := -1
		if len(n.eps) > 0 {
			lastEndpoint := n.eps[len(n.eps)-1]
			re := regexp.MustCompile("[0-9]+")
			matchStr := re.FindString(lastEndpoint.Name())
			n, err := strconv.ParseInt(matchStr, 10, 64)
			if err != nil {
				return nil, err
			}
			lastIdx = int(n)
		}
		if idx <= lastIdx {
			idx = lastIdx + 1
		}
		// Check if this is a dummy interface which has a vhost-user socket associated with it
		socketPath, err = vhostUserSocketPath(netInfo)
		if err != nil {
			return nil, err
		}

		if socketPath != "" {
			networkLogger().WithField("interface", netInfo.Iface.Name).Info("VhostUser network interface found")
			endpoint, err = createVhostUserEndpoint(netInfo, socketPath)
		} else if netInfo.Iface.Type == "macvlan" {
			networkLogger().Infof("macvlan interface found")
			endpoint, err = createMacvlanNetworkEndpoint(idx, netInfo.Iface.Name, n.interworkingModel)
		} else if netInfo.Iface.Type == "macvtap" {
			networkLogger().Infof("macvtap interface found")
			endpoint, err = createMacvtapNetworkEndpoint(netInfo)
		} else if netInfo.Iface.Type == "tap" {
			networkLogger().Info("tap interface found")
			endpoint, err = createTapNetworkEndpoint(idx, netInfo.Iface.Name)
		} else if netInfo.Iface.Type == "tuntap" {
			if netInfo.Link != nil {
				switch netInfo.Link.(*netlink.Tuntap).Mode {
				case 0:
					// mount /sys/class/net to get links
					return nil, fmt.Errorf("Network device mode not determined correctly. Mount sysfs in caller")
				case 1:
					return nil, fmt.Errorf("tun networking device not yet supported")
				case 2:
					networkLogger().Info("tuntap tap interface found")
					endpoint, err = createTuntapNetworkEndpoint(idx, netInfo.Iface.Name, netInfo.Iface.HardwareAddr, n.interworkingModel)
				default:
					return nil, fmt.Errorf("tuntap network %v mode unsupported", netInfo.Link.(*netlink.Tuntap).Mode)
				}
			}
		} else if netInfo.Iface.Type == "veth" {
			networkLogger().Info("veth interface found")
			endpoint, err = createVethNetworkEndpoint(idx, netInfo.Iface.Name, n.interworkingModel)
		} else if netInfo.Iface.Type == "ipvlan" {
			networkLogger().Info("ipvlan interface found")
			endpoint, err = createIPVlanNetworkEndpoint(idx, netInfo.Iface.Name)
		} else {
			return nil, fmt.Errorf("Unsupported network interface: %s", netInfo.Iface.Type)
		}
	}

	if err != nil {
		return nil, err
	}

	endpoint.SetProperties(netInfo)

	networkLogger().WithField("endpoint-type", endpoint.Type()).WithField("hotplug", hotplug).Info("Attaching endpoint")
	if hotplug {
		if err := endpoint.HotAttach(ctx, s); err != nil {
			return nil, err
		}
	} else {
		if err := endpoint.Attach(ctx, s); err != nil {
			return nil, err
		}
	}

	if !s.hypervisor.IsRateLimiterBuiltin() {
		rxRateLimiterMaxRate := s.hypervisor.HypervisorConfig().RxRateLimiterMaxRate
		if rxRateLimiterMaxRate > 0 {
			networkLogger().Info("Add Rx Rate Limiter")
			if err := addRxRateLimiter(endpoint, rxRateLimiterMaxRate); err != nil {
				return nil, err
			}
		}
		txRateLimiterMaxRate := s.hypervisor.HypervisorConfig().TxRateLimiterMaxRate
		if txRateLimiterMaxRate > 0 {
			networkLogger().Info("Add Tx Rate Limiter")
			if err := addTxRateLimiter(endpoint, txRateLimiterMaxRate); err != nil {
				return nil, err
			}
		}
	}

	n.eps = append(n.eps, endpoint)

	return endpoint, nil
}

func (n *LinuxNetwork) removeSingleEndpoint(ctx context.Context, s *Sandbox, endpoint Endpoint, hotplug bool) error {
	var idx int = len(n.eps)
	for i, val := range n.eps {
		if val.HardwareAddr() == endpoint.HardwareAddr() {
			idx = i
			break
		}
	}
	if idx == len(n.eps) {
		return fmt.Errorf("Endpoint not found")
	}

	if endpoint.GetRxRateLimiter() {
		networkLogger().WithField("endpoint-type", endpoint.Type()).Info("Deleting rx rate limiter")
		// Deleting rx rate limiter should enter the network namespace.
		if err := removeRxRateLimiter(endpoint, n.netNSPath); err != nil {
			return err
		}
	}

	if endpoint.GetTxRateLimiter() {
		networkLogger().WithField("endpoint-type", endpoint.Type()).Info("Deleting tx rate limiter")
		// Deleting tx rate limiter should enter the network namespace.
		if err := removeTxRateLimiter(endpoint, n.netNSPath); err != nil {
			return err
		}
	}

	// Detach for an endpoint should enter the network namespace
	// if required.
	networkLogger().WithField("endpoint-type", endpoint.Type()).Info("Detaching endpoint")
	if hotplug && s != nil {
		if err := endpoint.HotDetach(ctx, s, n.netNSCreated, n.netNSPath); err != nil {
			return err
		}
	} else {
		if err := endpoint.Detach(ctx, n.netNSCreated, n.netNSPath); err != nil {
			return err
		}
	}

	n.eps = append(n.eps[:idx], n.eps[idx+1:]...)

	return nil
}

func (n *LinuxNetwork) endpointAlreadyAdded(netInfo *NetworkInfo) bool {
	for _, ep := range n.eps {
		// Existing endpoint
		if ep.Name() == netInfo.Iface.Name {
			return true
		}
		pair := ep.NetworkPair()
		// Existing virtual endpoints
		if pair != nil && (pair.TapInterface.Name == netInfo.Iface.Name || pair.TapInterface.TAPIface.Name == netInfo.Iface.Name || pair.VirtIface.Name == netInfo.Iface.Name) {
			return true
		}
	}

	return false
}

func (n *LinuxNetwork) GetEndpointsNum() (int, error) {
	netnsHandle, err := netns.GetFromPath(n.netNSPath)
	if err != nil {
		return 0, err
	}
	defer netnsHandle.Close()

	netlinkHandle, err := netlink.NewHandleAt(netnsHandle)
	if err != nil {
		return 0, err
	}
	defer netlinkHandle.Close()

	linkList, err := netlinkHandle.LinkList()
	if err != nil {
		return 0, err
	}

	return len(linkList), nil
}

// Scan the networking namespace through netlink and then:
// 1. Create the endpoints for the relevant interfaces found there.
// 2. Attach them to the VM.
func (n *LinuxNetwork) addAllEndpoints(ctx context.Context, s *Sandbox, hotplug bool) error {
	netnsHandle, err := netns.GetFromPath(n.netNSPath)
	if err != nil {
		return err
	}
	defer netnsHandle.Close()

	netlinkHandle, err := netlink.NewHandleAt(netnsHandle)
	if err != nil {
		return err
	}
	defer netlinkHandle.Close()

	linkList, err := netlinkHandle.LinkList()
	if err != nil {
		return err
	}

	for _, link := range linkList {
		netInfo, err := networkInfoFromLink(netlinkHandle, link)
		if err != nil {
			return err
		}

		// Ignore unconfigured network interfaces. These are
		// either base tunnel devices that are not namespaced
		// like gre0, gretap0, sit0, ipip0, tunl0 or incorrectly
		// setup interfaces.
		if len(netInfo.Addrs) == 0 {
			continue
		}

		// Skip any loopback interfaces:
		if (netInfo.Iface.Flags & net.FlagLoopback) != 0 {
			continue
		}

		// Skip any interfaces that are already added
		if n.endpointAlreadyAdded(&netInfo) {
			networkLogger().WithField("endpoint", netInfo.Iface.Name).Info("already added")
			continue
		}

		if err := doNetNS(n.netNSPath, func(_ ns.NetNS) error {
			_, err = n.addSingleEndpoint(ctx, s, netInfo, hotplug)
			return err
		}); err != nil {
			return err
		}

	}

	sort.Slice(n.eps, func(i, j int) bool {
		return n.eps[i].Name() < n.eps[j].Name()
	})

	networkLogger().WithField("endpoints", n.eps).Info("endpoints found after scan")

	return nil
}

func convertDanDeviceToNetworkInfo(device *vctypes.DanDevice) (*NetworkInfo, error) {
	var netInfo NetworkInfo
	var err error
	netInfo.Iface.Name = device.Name
	if netInfo.Iface.HardwareAddr, err = net.ParseMAC(device.GuestMac); err != nil {
		return nil, fmt.Errorf("bad mac address in DAN config: %v", err)
	}
	netInfo.Iface.MTU = int(device.NetworkInfo.Interface.MTU)
	netInfo.Iface.Flags = net.Flags(device.NetworkInfo.Interface.Flags)

	for _, addr := range device.NetworkInfo.Interface.IPAddresses {
		a, err := netlink.ParseAddr(addr)
		if err != nil {
			return nil, fmt.Errorf("bad IP address in DAN config: %v", err)
		}

		netInfo.Addrs = append(netInfo.Addrs, *a)
	}

	for _, route := range device.NetworkInfo.Routes {
		var r netlink.Route
		if len(route.Dest) > 0 {
			if _, r.Dst, err = net.ParseCIDR(route.Dest); err != nil {
				return nil, fmt.Errorf("bad route dest in DAN config: %v", err)
			}
		}
		r.Src = net.ParseIP(route.Source)
		r.Gw = net.ParseIP(route.Gateway)
		r.Scope = netlink.Scope(route.Scope)
		if len(r.Gw.To4()) == net.IPv4len {
			r.Family = unix.AF_INET
		} else {
			r.Family = unix.AF_INET6
		}

		netInfo.Routes = append(netInfo.Routes, r)
	}

	for _, neigh := range device.NetworkInfo.Neighbors {
		var n netlink.Neigh
		n.State = int(neigh.State)
		n.Flags = int(neigh.Flags)
		if n.HardwareAddr, err = net.ParseMAC(neigh.HardwareAddr); err != nil {
			return nil, fmt.Errorf("bad neighbor hardware address in DAN config: %v", err)
		}

		n.IP = net.ParseIP(neigh.IPAddress)
		netInfo.Neighbors = append(netInfo.Neighbors, n)
	}

	return &netInfo, nil
}

// Load network config in DAN config
// Create the endpoints for the interfaces in Dan.
func (n *LinuxNetwork) addDanEndpoints() error {
	if len(n.eps) > 0 {
		// only load DAN config once
		return nil
	}

	jsonData, err := os.ReadFile(n.danConfigPath)
	if err != nil {
		return fmt.Errorf("fail to load DAN config file: %v", err)
	}

	var config vctypes.DanConfig
	err = json.Unmarshal([]byte(jsonData), &config)
	if err != nil {
		return fmt.Errorf("fail to unmarshal DAN config: %v", err)
	}

	for _, device := range config.Devices {
		var endpoint Endpoint
		networkLogger().WithField("interface", device.Name).Info("DAN interface found")

		netInfo, err := convertDanDeviceToNetworkInfo(&device)
		if err != nil {
			return err
		}

		switch device.Device.Type {
		case vctypes.VfioDanDeviceType:
			endpoint, err = createVfioEndpoint(device.Device.PciDeviceID, netInfo)
			if err != nil {
				return err
			}
		default:
			return fmt.Errorf("unknown DAN device type: '%s'", device.Device.Type)
		}

		n.eps = append(n.eps, endpoint)
	}

	sort.Slice(n.eps, func(i, j int) bool {
		return n.eps[i].Name() < n.eps[j].Name()
	})

	return nil
}

// Run runs a callback in the specified network namespace.
func (n *LinuxNetwork) Run(ctx context.Context, cb func() error) error {
	span, _ := n.trace(ctx, "Run")
	defer span.End()

	return doNetNS(n.netNSPath, func(_ ns.NetNS) error {
		return cb()
	})
}

// Add adds all needed interfaces inside the network namespace.
func (n *LinuxNetwork) AddEndpoints(ctx context.Context, s *Sandbox, endpointsInfo []NetworkInfo, hotplug bool) ([]Endpoint, error) {
	span, ctx := n.trace(ctx, "AddEndpoints")
	katatrace.AddTags(span, "type", n.interworkingModel.GetModel())
	defer span.End()

	if endpointsInfo == nil {
		// If a sandbox has a DAN configuration, it takes priority and will be used exclusively.
		if n.danConfigPath != "" {
			if err := n.addDanEndpoints(); err != nil {
				return nil, err
			}
		} else {
			if err := n.addAllEndpoints(ctx, s, hotplug); err != nil {
				return nil, err
			}
		}
	} else {
		for _, ep := range endpointsInfo {
			if err := doNetNS(n.netNSPath, func(_ ns.NetNS) error {
				if _, err := n.addSingleEndpoint(ctx, s, ep, hotplug); err != nil {
					n.eps = nil
					return err
				}

				return nil
			}); err != nil {
				return nil, err
			}
		}
	}

	katatrace.AddTags(span, "endpoints", n.eps, "hotplug", hotplug)
	networkLogger().Debug("Endpoints added")

	return n.eps, nil
}

// Remove network endpoints in the network namespace. It also deletes the network
// namespace in case the namespace has been created by us.
func (n *LinuxNetwork) RemoveEndpoints(ctx context.Context, s *Sandbox, endpoints []Endpoint, hotplug bool) error {
	span, ctx := n.trace(ctx, "RemoveEndpoints")
	defer span.End()

	eps := n.eps
	if endpoints != nil {
		eps = endpoints
	}

	for _, ep := range eps {
		if endpoints != nil {
			new_ep, _ := findEndpoint(ep, n.eps)
			if new_ep == nil {
				continue
			}
		}

		if err := n.removeSingleEndpoint(ctx, s, ep, hotplug); err != nil {
			// Log the error instead of returning right away
			// Proceed to remove the next endpoint so as to clean the network setup as
			// much as possible.
			// This is crucial for physical endpoints as we want to bind back the physical
			// interface to its original host driver.
			networkLogger().Warnf("Error removing endpoint %v : %v", ep.Name(), err)
		}
	}

	networkLogger().Debug("Endpoints removed")

	if n.netNSCreated && endpoints == nil {
		networkLogger().Infof("Network namespace %q deleted", n.netNSPath)
		return deleteNetNS(n.netNSPath)
	}

	return nil
}

// Network getters
func (n *LinuxNetwork) NetworkID() string {
	return n.netNSPath
}

func (n *LinuxNetwork) NetworkCreated() bool {
	return n.netNSCreated
}

func (n *LinuxNetwork) Endpoints() []Endpoint {
	return n.eps
}

func (n *LinuxNetwork) SetEndpoints(endpoints []Endpoint) {
	n.eps = endpoints
}

func createLink(netHandle *netlink.Handle, name string, expectedLink netlink.Link, queues int) (netlink.Link, []*os.File, error) {
	var newLink netlink.Link
	var fds []*os.File

	switch expectedLink.Type() {
	case (&netlink.Tuntap{}).Type():
		flags := netlink.TUNTAP_VNET_HDR | netlink.TUNTAP_NO_PI
		if queues > 0 {
			flags |= netlink.TUNTAP_MULTI_QUEUE_DEFAULTS
		} else {
			// We need to enforce `queues = 1` here in case
			// multi-queue is *not* supported, the reason being
			// `linkModify()`, a method called by `LinkAdd()`, only
			// returning the file descriptor of the opened tuntap
			// device when the queues are set to *non zero*.
			//
			// Please, for more information, refer to:
			// https://github.com/kata-containers/kata-containers/blob/e6e5d2593ac319329269d7b58c30f99ba7b2bf5a/src/runtime/vendor/github.com/vishvananda/netlink/link_linux.go#L1164-L1316
			queues = 1
		}
		newLink = &netlink.Tuntap{
			LinkAttrs: netlink.LinkAttrs{Name: name},
			Mode:      netlink.TUNTAP_MODE_TAP,
			Queues:    queues,
			Flags:     flags,
		}
	case (&netlink.Macvtap{}).Type():
		qlen := expectedLink.Attrs().TxQLen
		if qlen <= 0 {
			qlen = defaultQlen
		}
		newLink = &netlink.Macvtap{
			Macvlan: netlink.Macvlan{
				Mode: netlink.MACVLAN_MODE_BRIDGE,
				LinkAttrs: netlink.LinkAttrs{
					Index:       expectedLink.Attrs().Index,
					Name:        name,
					TxQLen:      qlen,
					ParentIndex: expectedLink.Attrs().ParentIndex,
				},
			},
		}
	default:
		return nil, fds, fmt.Errorf("Unsupported link type %s", expectedLink.Type())
	}

	if err := netHandle.LinkAdd(newLink); err != nil {
		return nil, fds, fmt.Errorf("LinkAdd() failed for %s name %s: %s", expectedLink.Type(), name, err)
	}

	tuntapLink, ok := newLink.(*netlink.Tuntap)
	if ok {
		fds = tuntapLink.Fds
	}

	newLink, err := getLinkByName(netHandle, name, expectedLink)
	return newLink, fds, err
}

func getLinkForEndpoint(endpoint Endpoint, netHandle *netlink.Handle) (netlink.Link, error) {
	var link netlink.Link

	switch ep := endpoint.(type) {
	case *VethEndpoint:
		link = &netlink.Veth{}
	case *MacvlanEndpoint:
		link = &netlink.Macvlan{}
	case *IPVlanEndpoint:
		link = &netlink.IPVlan{}
	case *TuntapEndpoint:
		link = &netlink.Tuntap{}
	default:
		return nil, fmt.Errorf("Unexpected endpointType %s", ep.Type())
	}

	return getLinkByName(netHandle, endpoint.NetworkPair().VirtIface.Name, link)
}

func getLinkByName(netHandle *netlink.Handle, name string, expectedLink netlink.Link) (netlink.Link, error) {
	link, err := netHandle.LinkByName(name)
	if err != nil {
		return nil, fmt.Errorf("LinkByName() failed for %s name %s: %s", expectedLink.Type(), name, err)
	}

	switch expectedLink.Type() {
	case (&netlink.Tuntap{}).Type():
		if l, ok := link.(*netlink.Tuntap); ok {
			return l, nil
		}
	case (&netlink.Veth{}).Type():
		if l, ok := link.(*netlink.Veth); ok {
			return l, nil
		}
	case (&netlink.Macvtap{}).Type():
		if l, ok := link.(*netlink.Macvtap); ok {
			return l, nil
		}
	case (&netlink.Macvlan{}).Type():
		if l, ok := link.(*netlink.Macvlan); ok {
			return l, nil
		}
	case (&netlink.IPVlan{}).Type():
		if l, ok := link.(*netlink.IPVlan); ok {
			return l, nil
		}
	default:
		return nil, fmt.Errorf("Unsupported link type %s", expectedLink.Type())
	}

	return nil, fmt.Errorf("Incorrect link type %s, expecting %s", link.Type(), expectedLink.Type())
}

// The endpoint type should dictate how the connection needs to happen.
func xConnectVMNetwork(ctx context.Context, endpoint Endpoint, h Hypervisor) error {
	var err error

	span, ctx := networkTrace(ctx, "xConnectVMNetwork", endpoint)
	defer closeSpan(span, err)

	netPair := endpoint.NetworkPair()

	queues := 0
	caps := h.Capabilities(ctx)
	if caps.IsMultiQueueSupported() {
		queues = int(h.HypervisorConfig().NumVCPUs())
	}

	disableVhostNet := h.HypervisorConfig().DisableVhostNet

	if netPair.NetInterworkingModel == NetXConnectDefaultModel {
		netPair.NetInterworkingModel = DefaultNetInterworkingModel
	}

	switch netPair.NetInterworkingModel {
	case NetXConnectMacVtapModel:
		networkLogger().Info("connect macvtap to VM network")
		err = tapNetworkPair(ctx, endpoint, queues, disableVhostNet)
	case NetXConnectTCFilterModel:
		networkLogger().Info("connect TCFilter to VM network")
		err = setupTCFiltering(ctx, endpoint, queues, disableVhostNet)
	default:
		err = fmt.Errorf("Invalid internetworking model")
	}
	return err
}

// The endpoint type should dictate how the disconnection needs to happen.
func xDisconnectVMNetwork(ctx context.Context, endpoint Endpoint) error {
	var err error

	span, ctx := networkTrace(ctx, "xDisconnectVMNetwork", endpoint)
	defer closeSpan(span, err)

	netPair := endpoint.NetworkPair()

	if netPair.NetInterworkingModel == NetXConnectDefaultModel {
		netPair.NetInterworkingModel = DefaultNetInterworkingModel
	}

	switch netPair.NetInterworkingModel {
	case NetXConnectMacVtapModel:
		err = untapNetworkPair(ctx, endpoint)
	case NetXConnectTCFilterModel:
		err = removeTCFiltering(ctx, endpoint)
	default:
		err = fmt.Errorf("Invalid internetworking model")
	}
	return err
}

func createMacvtapFds(linkIndex int, queues int) ([]*os.File, error) {
	tapDev := fmt.Sprintf("/dev/tap%d", linkIndex)
	return createFds(tapDev, queues)
}

func createVhostFds(numFds int) ([]*os.File, error) {
	vhostDev := "/dev/vhost-net"
	return createFds(vhostDev, numFds)
}

func createFds(device string, numFds int) ([]*os.File, error) {
	fds := make([]*os.File, numFds)

	for i := 0; i < numFds; i++ {
		f, err := os.OpenFile(device, os.O_RDWR, defaultFilePerms)
		if err != nil {
			utils.CleanupFds(fds, i)
			return nil, err
		}
		fds[i] = f
	}
	return fds, nil
}

// There is a limitation in the linux kernel that prevents a macvtap/macvlan link
// from getting the correct link index when created in a network namespace
// https://github.com/clearcontainers/runtime/issues/708
//
// Till that bug is fixed we need to pick a random non conflicting index and try to
// create a link. If that fails, we need to try with another.
// All the kernel does not Check if the link id conflicts with a link id on the host
// hence we need to offset the link id to prevent any overlaps with the host index
//
// Here the kernel will ensure that there is no race condition

const hostLinkOffset = 8192 // Host should not have more than 8k interfaces
const linkRange = 0xFFFF    // This will allow upto 2^16 containers
const linkRetries = 128     // The numbers of time we try to find a non conflicting index
const macvtapWorkaround = true

func createMacVtap(netHandle *netlink.Handle, name string, link netlink.Link, queues int) (taplink netlink.Link, err error) {

	if !macvtapWorkaround {
		taplink, _, err = createLink(netHandle, name, link, queues)
		return
	}

	r := rand.New(rand.NewSource(time.Now().UnixNano()))

	for i := 0; i < linkRetries; i++ {
		index := hostLinkOffset + (r.Int() & linkRange)
		link.Attrs().Index = index
		taplink, _, err = createLink(netHandle, name, link, queues)
		if err == nil {
			break
		}
	}

	return
}

func clearIPs(link netlink.Link, addrs []netlink.Addr) error {
	for _, addr := range addrs {
		if err := netlink.AddrDel(link, &addr); err != nil {
			return err
		}
	}
	return nil
}

func setIPs(link netlink.Link, addrs []netlink.Addr) error {
	for _, addr := range addrs {
		if err := netlink.AddrAdd(link, &addr); err != nil {
			return err
		}
	}
	return nil
}

func tapNetworkPair(ctx context.Context, endpoint Endpoint, queues int, disableVhostNet bool) error {
	span, _ := networkTrace(ctx, "tapNetworkPair", endpoint)
	defer span.End()

	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Close()

	netPair := endpoint.NetworkPair()

	link, err := getLinkForEndpoint(endpoint, netHandle)
	if err != nil {
		return err
	}

	attrs := link.Attrs()

	// Attach the macvtap interface to the underlying container
	// interface. Also picks relevant attributes from the parent
	tapLink, err := createMacVtap(netHandle, netPair.TAPIface.Name,
		&netlink.Macvtap{
			Macvlan: netlink.Macvlan{
				LinkAttrs: netlink.LinkAttrs{
					TxQLen:      attrs.TxQLen,
					ParentIndex: attrs.Index,
				},
			},
		}, queues)

	if err != nil {
		return fmt.Errorf("Could not create TAP interface: %s", err)
	}

	// Save the veth MAC address to the TAP so that it can later be used
	// to build the hypervisor command line. This MAC address has to be
	// the one inside the VM in order to avoid any firewall issues. The
	// bridge created by the network plugin on the host actually expects
	// to see traffic from this MAC address and not another one.
	tapHardAddr := attrs.HardwareAddr
	netPair.TAPIface.HardAddr = attrs.HardwareAddr.String()

	if err := netHandle.LinkSetMTU(tapLink, attrs.MTU); err != nil {
		return fmt.Errorf("Could not set TAP MTU %d: %s", attrs.MTU, err)
	}

	hardAddr, err := net.ParseMAC(netPair.VirtIface.HardAddr)
	if err != nil {
		return err
	}
	if err := netHandle.LinkSetHardwareAddr(link, hardAddr); err != nil {
		return fmt.Errorf("Could not set MAC address %s for veth interface %s: %s",
			netPair.VirtIface.HardAddr, netPair.VirtIface.Name, err)
	}

	if err := netHandle.LinkSetHardwareAddr(tapLink, tapHardAddr); err != nil {
		return fmt.Errorf("Could not set MAC address %s for TAP interface %s: %s",
			netPair.TAPIface.HardAddr, netPair.TAPIface.Name, err)
	}

	if err := netHandle.LinkSetUp(tapLink); err != nil {
		return fmt.Errorf("Could not enable TAP %s: %s", netPair.TAPIface.Name, err)
	}

	// Clear the IP addresses from the veth interface to prevent ARP conflict
	netPair.VirtIface.Addrs, err = netlink.AddrList(link, netlink.FAMILY_ALL)
	if err != nil {
		return fmt.Errorf("Unable to obtain veth IP addresses: %s", err)
	}

	if err := clearIPs(link, netPair.VirtIface.Addrs); err != nil {
		return fmt.Errorf("Unable to clear veth IP addresses: %s", err)
	}

	if err := netHandle.LinkSetUp(link); err != nil {
		return fmt.Errorf("Could not enable veth %s: %s", netPair.VirtIface.Name, err)
	}

	// Note: The underlying interfaces need to be up prior to fd creation.

	netPair.VMFds, err = createMacvtapFds(tapLink.Attrs().Index, queues)
	if err != nil {
		return fmt.Errorf("Could not setup macvtap fds %s: %s", netPair.TAPIface, err)
	}

	if !disableVhostNet {
		vhostFds, err := createVhostFds(queues)
		if err != nil {
			return fmt.Errorf("Could not setup vhost fds %s : %s", netPair.VirtIface.Name, err)
		}
		netPair.VhostFds = vhostFds
	}

	return nil
}

func setupTCFiltering(ctx context.Context, endpoint Endpoint, queues int, disableVhostNet bool) error {
	span, _ := networkTrace(ctx, "setupTCFiltering", endpoint)
	defer span.End()

	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Close()

	netPair := endpoint.NetworkPair()

	tapLink, fds, err := createLink(netHandle, netPair.TAPIface.Name, &netlink.Tuntap{}, queues)
	if err != nil {
		return fmt.Errorf("Could not create TAP interface: %s", err)
	}
	netPair.VMFds = fds

	if !disableVhostNet {
		vhostFds, err := createVhostFds(queues)
		if err != nil {
			return fmt.Errorf("Could not setup vhost fds %s : %s", netPair.VirtIface.Name, err)
		}
		netPair.VhostFds = vhostFds
	}

	var attrs *netlink.LinkAttrs
	var link netlink.Link

	link, err = getLinkForEndpoint(endpoint, netHandle)
	if err != nil {
		return err
	}

	attrs = link.Attrs()

	// Save the veth MAC address to the TAP so that it can later be used
	// to build the Hypervisor command line. This MAC address has to be
	// the one inside the VM in order to avoid any firewall issues. The
	// bridge created by the network plugin on the host actually expects
	// to see traffic from this MAC address and not another one.
	netPair.TAPIface.HardAddr = attrs.HardwareAddr.String()

	if err := netHandle.LinkSetMTU(tapLink, attrs.MTU); err != nil {
		return fmt.Errorf("Could not set TAP MTU %d: %s", attrs.MTU, err)
	}

	if err := netHandle.LinkSetUp(tapLink); err != nil {
		return fmt.Errorf("Could not enable TAP %s: %s", netPair.TAPIface.Name, err)
	}

	tapAttrs := tapLink.Attrs()

	if err := addQdiscIngress(tapAttrs.Index); err != nil {
		return err
	}

	if err := addQdiscIngress(attrs.Index); err != nil {
		return err
	}

	if err := addRedirectTCFilter(attrs.Index, tapAttrs.Index); err != nil {
		return err
	}

	if err := addRedirectTCFilter(tapAttrs.Index, attrs.Index); err != nil {
		return err
	}

	return nil
}

// addQdiscIngress creates a new qdisc for network interface with the specified network index
// on "ingress". qdiscs normally don't work on ingress so this is really a special qdisc
// that you can consider an "alternate root" for inbound packets.
// Handle for ingress qdisc defaults to "ffff:"
//
// This is equivalent to calling `tc qdisc add dev eth0 ingress`
func addQdiscIngress(index int) error {
	qdisc := &netlink.Ingress{
		QdiscAttrs: netlink.QdiscAttrs{
			LinkIndex: index,
			Parent:    netlink.HANDLE_INGRESS,
		},
	}

	err := netlink.QdiscAdd(qdisc)
	if err != nil {
		return fmt.Errorf("Failed to add qdisc for network index %d : %s", index, err)
	}

	return nil
}

// addRedirectTCFilter adds a tc filter for device with index "sourceIndex".
// All traffic for interface with index "sourceIndex" is redirected to interface with
// index "destIndex"
//
// This is equivalent to calling:
// `tc filter add dev source parent ffff: protocol all u32 match u8 0 0 action mirred egress redirect dev dest`
func addRedirectTCFilter(sourceIndex, destIndex int) error {
	filter := &netlink.U32{
		FilterAttrs: netlink.FilterAttrs{
			LinkIndex: sourceIndex,
			Parent:    netlink.MakeHandle(0xffff, 0),
			Protocol:  unix.ETH_P_ALL,
		},
		Actions: []netlink.Action{
			&netlink.MirredAction{
				ActionAttrs: netlink.ActionAttrs{
					Action: netlink.TC_ACT_STOLEN,
				},
				MirredAction: netlink.TCA_EGRESS_REDIR,
				Ifindex:      destIndex,
			},
		},
	}

	if err := netlink.FilterAdd(filter); err != nil {
		return fmt.Errorf("Failed to add filter for index %d : %s", sourceIndex, err)
	}

	return nil
}

// removeRedirectTCFilter removes all tc u32 filters created on ingress qdisc for "link".
func removeRedirectTCFilter(link netlink.Link) error {
	if link == nil {
		return nil
	}

	// Handle 0xffff is used for ingress
	filters, err := netlink.FilterList(link, netlink.MakeHandle(0xffff, 0))
	if err != nil {
		return err
	}

	for _, f := range filters {
		u32, ok := f.(*netlink.U32)

		if !ok {
			continue
		}

		if err := netlink.FilterDel(u32); err != nil {
			return err
		}
	}
	return nil
}

// removeQdiscIngress removes the ingress qdisc previously created on "link".
func removeQdiscIngress(link netlink.Link) error {
	if link == nil {
		return nil
	}

	qdiscs, err := netlink.QdiscList(link)
	if err != nil {
		return err
	}

	for _, qdisc := range qdiscs {
		ingress, ok := qdisc.(*netlink.Ingress)
		if !ok {
			continue
		}

		if err := netlink.QdiscDel(ingress); err != nil {
			return err
		}
	}
	return nil
}

func untapNetworkPair(ctx context.Context, endpoint Endpoint) error {
	span, _ := networkTrace(ctx, "untapNetworkPair", endpoint)
	defer span.End()

	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Close()

	netPair := endpoint.NetworkPair()

	tapLink, err := getLinkByName(netHandle, netPair.TAPIface.Name, &netlink.Macvtap{})
	if err != nil {
		return fmt.Errorf("Could not get TAP interface %s: %s", netPair.TAPIface.Name, err)
	}

	if err := netHandle.LinkDel(tapLink); err != nil {
		return fmt.Errorf("Could not remove TAP %s: %s", netPair.TAPIface.Name, err)
	}

	link, err := getLinkForEndpoint(endpoint, netHandle)
	if err != nil {
		return err
	}

	hardAddr, err := net.ParseMAC(netPair.TAPIface.HardAddr)
	if err != nil {
		return err
	}
	if err := netHandle.LinkSetHardwareAddr(link, hardAddr); err != nil {
		return fmt.Errorf("Could not set MAC address %s for veth interface %s: %s",
			netPair.VirtIface.HardAddr, netPair.VirtIface.Name, err)
	}

	if err := netHandle.LinkSetDown(link); err != nil {
		return fmt.Errorf("Could not disable veth %s: %s", netPair.VirtIface.Name, err)
	}

	// Restore the IPs that were cleared
	err = setIPs(link, netPair.VirtIface.Addrs)
	return err
}

func removeTCFiltering(ctx context.Context, endpoint Endpoint) error {
	span, _ := networkTrace(ctx, "removeTCFiltering", endpoint)
	defer span.End()

	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Close()

	netPair := endpoint.NetworkPair()

	tapLink, err := getLinkByName(netHandle, netPair.TAPIface.Name, &netlink.Tuntap{})
	if err != nil {
		return fmt.Errorf("Could not get TAP interface: %s", err)
	}

	if err := netHandle.LinkSetDown(tapLink); err != nil {
		return fmt.Errorf("Could not disable TAP %s: %s", netPair.TAPIface.Name, err)
	}

	if err := netHandle.LinkDel(tapLink); err != nil {
		return fmt.Errorf("Could not remove TAP %s: %s", netPair.TAPIface.Name, err)
	}

	link, err := getLinkForEndpoint(endpoint, netHandle)
	if err != nil {
		return err
	}

	if err := removeRedirectTCFilter(link); err != nil {
		return err
	}

	if err := removeQdiscIngress(link); err != nil {
		return err
	}

	if err := netHandle.LinkSetDown(link); err != nil {
		return fmt.Errorf("Could not disable veth %s: %s", netPair.VirtIface.Name, err)
	}

	return nil
}

// doNetNS is free from any call to a go routine, and it calls
// into runtime.LockOSThread(), meaning it won't be executed in a
// different thread than the one expected by the caller.
func doNetNS(netNSPath string, cb func(ns.NetNS) error) error {
	// if netNSPath is empty, the callback function will be run in the current network namespace.
	// So skip the whole function, just call cb(). cb() needs a NetNS as arg but ignored, give it a fake one.
	if netNSPath == "" {
		var netNs ns.NetNS
		return cb(netNs)
	}

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	currentNS, err := ns.GetCurrentNS()
	if err != nil {
		return err
	}
	defer currentNS.Close()

	targetNS, err := ns.GetNS(netNSPath)
	if err != nil {
		return err
	}

	if err := targetNS.Set(); err != nil {
		return err
	}
	defer currentNS.Set()

	return cb(targetNS)
}

// EnterNetNS is free from any call to a go routine, and it calls
// into runtime.LockOSThread(), meaning it won't be executed in a
// different thread than the one expected by the caller.
func EnterNetNS(networkID string, cb func() error) error {
	return doNetNS(networkID, func(nn ns.NetNS) error {
		return cb()
	})
}

func deleteNetNS(netNSPath string) error {
	n, err := ns.GetNS(netNSPath)
	if err != nil {
		return err
	}

	err = n.Close()
	if err != nil {
		return err
	}

	if err = unix.Unmount(netNSPath, unix.MNT_DETACH); err != nil {
		return fmt.Errorf("Failed to unmount namespace %s: %v", netNSPath, err)
	}
	if err := os.RemoveAll(netNSPath); err != nil {
		return fmt.Errorf("Failed to clean up namespace %s: %v", netNSPath, err)
	}

	return nil
}

func networkInfoFromLink(handle *netlink.Handle, link netlink.Link) (NetworkInfo, error) {
	addrs, err := handle.AddrList(link, netlink.FAMILY_ALL)
	if err != nil {
		return NetworkInfo{}, err
	}

	routes, err := handle.RouteList(link, netlink.FAMILY_ALL)
	if err != nil {
		return NetworkInfo{}, err
	}

	neighbors, err := handle.NeighList(link.Attrs().Index, netlink.FAMILY_ALL)
	if err != nil {
		return NetworkInfo{}, err
	}

	return NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: *(link.Attrs()),
			Type:      link.Type(),
		},
		Addrs:     addrs,
		Routes:    routes,
		Neighbors: neighbors,
		Link:      link,
	}, nil
}

// func addRxRateLmiter implements tc-based rx rate limiter to control network I/O inbound traffic
// on VM level for hypervisors which don't implement rate limiter in itself, like qemu, etc.
func addRxRateLimiter(endpoint Endpoint, maxRate uint64) error {
	var linkName string
	switch ep := endpoint.(type) {
	case *VethEndpoint, *IPVlanEndpoint, *TuntapEndpoint, *MacvlanEndpoint:
		netPair := endpoint.NetworkPair()
		linkName = netPair.TapInterface.TAPIface.Name
	case *MacvtapEndpoint, *TapEndpoint:
		linkName = endpoint.Name()
	default:
		return fmt.Errorf("Unsupported endpointType %s for adding rx rate limiter", ep.Type())
	}

	if err := endpoint.SetRxRateLimiter(); err != nil {
		return nil
	}

	link, err := netlink.LinkByName(linkName)
	if err != nil {
		return err
	}
	linkIndex := link.Attrs().Index

	return addHTBQdisc(linkIndex, maxRate)
}

// func addHTBQdisc uses HTB(Hierarchical Token Bucket) qdisc shaping schemes to control interface traffic.
// HTB (Hierarchical Token Bucket) shapes traffic based on the Token Bucket Filter algorithm.
// A fundamental part of the HTB qdisc is the borrowing mechanism. Children classes borrow tokens
// from their parents once they have exceeded rate. A child class will continue to attempt to borrow until
// it reaches ceil. See more details in https://tldp.org/HOWTO/Traffic-Control-HOWTO/classful-qdiscs.html.
//
//   - +-----+     +---------+     +-----------+      +-----------+
//   - |     |     |  qdisc  |     | class 1:1 |      | class 1:2 |
//   - | NIC |     |   htb   |     |   rate    |      |   rate    |
//   - |     | --> | def 1:2 | --> |   ceil    | -+-> |   ceil    |
//   - +-----+     +---------+     +-----------+  |   +-----------+
//   - |
//   - |   +-----------+
//   - |   | class 1:n |
//   - |   |   rate    |
//   - +-> |   ceil    |
//   - |   +-----------+
//
// Seeing from pic, after the routing decision, all packets will be sent to the interface root htb qdisc.
// This root qdisc has only one direct child class (with id 1:1) which shapes the overall maximum rate
// that will be sent through interface. Then, this class has at least one default child (1:2) meant to control all
// non-privileged traffic.
// e.g.
// if we try to set VM bandwidth with maximum 10Mbit/s, we should give
// classid 1:2 rate 10Mbit/s, ceil 10Mbit/s and classid 1:1 rate 10Mbit/s, ceil 10Mbit/s.
// To-do:
// Later, if we want to do limitation on some dedicated traffic(special process running in VM), we could create
// a separate class (1:n) with guarantee throughput.
func addHTBQdisc(linkIndex int, maxRate uint64) error {
	// we create a new htb root qdisc for network interface with the specified network index
	qdiscAttrs := netlink.QdiscAttrs{
		LinkIndex: linkIndex,
		Handle:    netlink.MakeHandle(1, 0),
		Parent:    netlink.HANDLE_ROOT,
	}
	qdisc := netlink.NewHtb(qdiscAttrs)
	// all non-privileged traffic go to classid 1:2.
	qdisc.Defcls = 2

	err := netlink.QdiscAdd(qdisc)
	if err != nil {
		return fmt.Errorf("Failed to add htb qdisc: %v", err)
	}

	// root htb qdisc has only one direct child class (with id 1:1) to control overall rate.
	classAttrs := netlink.ClassAttrs{
		LinkIndex: linkIndex,
		Parent:    netlink.MakeHandle(1, 0),
		Handle:    netlink.MakeHandle(1, 1),
	}
	htbClassAttrs := netlink.HtbClassAttrs{
		Rate: maxRate,
		Ceil: maxRate,
	}
	class := netlink.NewHtbClass(classAttrs, htbClassAttrs)
	if err := netlink.ClassAdd(class); err != nil {
		return fmt.Errorf("Failed to add htb classid 1:1 : %v", err)
	}

	// above class has at least one default child class(1:2) for all non-privileged traffic.
	classAttrs = netlink.ClassAttrs{
		LinkIndex: linkIndex,
		Parent:    netlink.MakeHandle(1, 1),
		Handle:    netlink.MakeHandle(1, 2),
	}
	htbClassAttrs = netlink.HtbClassAttrs{
		Rate: maxRate,
		Ceil: maxRate,
	}
	class = netlink.NewHtbClass(classAttrs, htbClassAttrs)
	if err := netlink.ClassAdd(class); err != nil {
		return fmt.Errorf("Failed to add htb class 1:2 : %v", err)
	}

	return nil
}

// The Intermediate Functional Block (ifb) pseudo network interface is an alternative
// to tc filters for handling ingress traffic,
// By redirecting interface ingress traffic to ifb and treat it as egress traffic there,
// we could do network shaping to interface inbound traffic.
func addIFBDevice() (int, error) {
	// Check whether host supports ifb
	if ok, err := utils.SupportsIfb(); !ok {
		return -1, err
	}

	netHandle, err := netlink.NewHandle()
	if err != nil {
		return -1, err
	}
	defer netHandle.Close()

	// There exists error when using netlink library to create ifb interface
	cmd := exec.Command("ip", "link", "add", "dev", "ifb0", "type", "ifb")
	if output, err := cmd.CombinedOutput(); err != nil {
		return -1, fmt.Errorf("Could not create link ifb0: %v, error %v", output, err)
	}

	ifbLink, err := netlink.LinkByName("ifb0")
	if err != nil {
		return -1, err
	}

	if err := netHandle.LinkSetUp(ifbLink); err != nil {
		return -1, fmt.Errorf("Could not enable link ifb0 %v", err)
	}

	return ifbLink.Attrs().Index, nil
}

// This is equivalent to calling:
// tc filter add dev source parent ffff: protocol all u32 match u8 0 0 action mirred egress redirect dev ifb
func addIFBRedirecting(sourceIndex int, ifbIndex int) error {
	if err := addQdiscIngress(sourceIndex); err != nil {
		return err
	}

	if err := addRedirectTCFilter(sourceIndex, ifbIndex); err != nil {
		return err
	}

	return nil
}

// addTxRateLimiter implements tx rate limiter to control network I/O outbound traffic
// on VM level for hypervisors which don't implement rate limiter in itself, like qemu, etc.
// We adopt different actions, based on different inter-networking models.
// For tcfilters as inter-networking model, we simply apply htb qdisc discipline to the virtual netpair.
// For other inter-networking models, such as macvtap, we resort to ifb, by redirecting endpoint ingress traffic
// to ifb egress, and then apply htb to ifb egress.
func addTxRateLimiter(endpoint Endpoint, maxRate uint64) error {
	var netPair *NetworkInterfacePair
	var linkName string
	switch ep := endpoint.(type) {
	case *VethEndpoint, *IPVlanEndpoint, *TuntapEndpoint, *MacvlanEndpoint:
		netPair = endpoint.NetworkPair()
		switch netPair.NetInterworkingModel {
		// For those endpoints we've already used tcfilter as their inter-networking model,
		// another ifb redirect will be redundant and confused.
		case NetXConnectTCFilterModel:
			linkName = netPair.VirtIface.Name
			link, err := netlink.LinkByName(linkName)
			if err != nil {
				return err
			}
			return addHTBQdisc(link.Attrs().Index, maxRate)
		case NetXConnectMacVtapModel, NetXConnectNoneModel:
			linkName = netPair.TapInterface.TAPIface.Name
		default:
			return fmt.Errorf("Unsupported inter-networking model %v for adding tx rate limiter", netPair.NetInterworkingModel)
		}

	case *MacvtapEndpoint, *TapEndpoint:
		linkName = endpoint.Name()
	default:
		return fmt.Errorf("Unsupported endpointType %s for adding tx rate limiter", ep.Type())
	}

	if err := endpoint.SetTxRateLimiter(); err != nil {
		return err
	}

	ifbIndex, err := addIFBDevice()
	if err != nil {
		return err
	}

	link, err := netlink.LinkByName(linkName)
	if err != nil {
		return err
	}

	if err := addIFBRedirecting(link.Attrs().Index, ifbIndex); err != nil {
		return err
	}

	return addHTBQdisc(ifbIndex, maxRate)
}

func removeHTBQdisc(linkName string) error {
	link, err := netlink.LinkByName(linkName)
	if err != nil {
		return fmt.Errorf("Get link %s by name failed: %v", linkName, err)
	}

	qdiscs, err := netlink.QdiscList(link)
	if err != nil {
		return err
	}

	for _, qdisc := range qdiscs {
		htb, ok := qdisc.(*netlink.Htb)
		if !ok {
			continue
		}

		if err := netlink.QdiscDel(htb); err != nil {
			return fmt.Errorf("Failed to delete htb qdisc on link %s: %v", linkName, err)
		}
	}

	return nil
}

func removeRxRateLimiter(endpoint Endpoint, networkNSPath string) error {
	var linkName string
	switch ep := endpoint.(type) {
	case *VethEndpoint, *IPVlanEndpoint, *TuntapEndpoint, *MacvlanEndpoint:
		netPair := endpoint.NetworkPair()
		linkName = netPair.TapInterface.TAPIface.Name
	case *MacvtapEndpoint, *TapEndpoint:
		linkName = endpoint.Name()
	default:
		return fmt.Errorf("Unsupported endpointType %s for removing rx rate limiter", ep.Type())
	}

	if err := doNetNS(networkNSPath, func(_ ns.NetNS) error {
		return removeHTBQdisc(linkName)
	}); err != nil {
		return err
	}

	return nil
}

func removeTxRateLimiter(endpoint Endpoint, networkNSPath string) error {
	var linkName string
	switch ep := endpoint.(type) {
	case *VethEndpoint, *IPVlanEndpoint, *TuntapEndpoint, *MacvlanEndpoint:
		netPair := endpoint.NetworkPair()
		switch netPair.NetInterworkingModel {
		case NetXConnectTCFilterModel:
			linkName = netPair.VirtIface.Name
			if err := doNetNS(networkNSPath, func(_ ns.NetNS) error {
				return removeHTBQdisc(linkName)
			}); err != nil {
				return err
			}
			return nil
		case NetXConnectMacVtapModel, NetXConnectNoneModel:
			linkName = netPair.TapInterface.TAPIface.Name
		}
	case *MacvtapEndpoint, *TapEndpoint:
		linkName = endpoint.Name()
	default:
		return fmt.Errorf("Unsupported endpointType %s for adding tx rate limiter", ep.Type())
	}

	if err := doNetNS(networkNSPath, func(_ ns.NetNS) error {
		link, err := netlink.LinkByName(linkName)
		if err != nil {
			return fmt.Errorf("Get link %s by name failed: %v", linkName, err)
		}

		if err := removeRedirectTCFilter(link); err != nil {
			return err
		}

		if err := removeQdiscIngress(link); err != nil {
			return err
		}

		netHandle, err := netlink.NewHandle()
		if err != nil {
			return err
		}
		defer netHandle.Close()

		// remove ifb interface
		ifbLink, err := netlink.LinkByName("ifb0")
		if err != nil {
			return fmt.Errorf("Get link %s by name failed: %v", linkName, err)
		}

		if err := netHandle.LinkSetDown(ifbLink); err != nil {
			return fmt.Errorf("Could not disable ifb interface: %v", err)
		}

		if err := netHandle.LinkDel(ifbLink); err != nil {
			return fmt.Errorf("Could not remove ifb interface: %v", err)
		}

		return nil
	}); err != nil {
		return err
	}

	return nil
}

func validGuestRoute(route netlink.Route) bool {
	return route.Protocol != unix.RTPROT_KERNEL
}

func validGuestNeighbor(neigh netlink.Neigh) bool {
	// We add only static ARP entries
	return neigh.State == netlink.NUD_PERMANENT
}
