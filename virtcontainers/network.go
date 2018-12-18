// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	cryptoRand "crypto/rand"
	"encoding/json"
	"fmt"
	"math/rand"
	"net"
	"os"
	"runtime"
	"sort"
	"time"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
	"golang.org/x/sys/unix"

	"github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/runtime/virtcontainers/pkg/uuid"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

// NetInterworkingModel defines the network model connecting
// the network interface to the virtual machine.
type NetInterworkingModel int

const (
	// NetXConnectDefaultModel Ask to use DefaultNetInterworkingModel
	NetXConnectDefaultModel NetInterworkingModel = iota

	// NetXConnectBridgedModel uses a linux bridge to interconnect
	// the container interface to the VM. This is the
	// safe default that works for most cases except
	// macvlan and ipvlan
	NetXConnectBridgedModel

	// NetXConnectMacVtapModel can be used when the Container network
	// interface can be bridged using macvtap
	NetXConnectMacVtapModel

	// NetXConnectEnlightenedModel can be used when the Network plugins
	// are enlightened to create VM native interfaces
	// when requested by the runtime
	// This will be used for vethtap, macvtap, ipvtap
	NetXConnectEnlightenedModel

	// NetXConnectTCFilterModel redirects traffic from the network interface
	// provided by the network plugin to a tap interface.
	// This works for ipvlan and macvlan as well.
	NetXConnectTCFilterModel

	// NetXConnectNoneModel can be used when the VM is in the host network namespace
	NetXConnectNoneModel

	// NetXConnectInvalidModel is the last item to check valid values by IsValid()
	NetXConnectInvalidModel
)

//IsValid checks if a model is valid
func (n NetInterworkingModel) IsValid() bool {
	return 0 <= int(n) && int(n) < int(NetXConnectInvalidModel)
}

const (
	defaultNetModelStr = "default"

	bridgedNetModelStr = "bridged"

	macvtapNetModelStr = "macvtap"

	enlightenedNetModelStr = "enlightened"

	tcFilterNetModelStr = "tcfilter"

	noneNetModelStr = "none"
)

//SetModel change the model string value
func (n *NetInterworkingModel) SetModel(modelName string) error {
	switch modelName {
	case defaultNetModelStr:
		*n = DefaultNetInterworkingModel
		return nil
	case bridgedNetModelStr:
		*n = NetXConnectBridgedModel
		return nil
	case macvtapNetModelStr:
		*n = NetXConnectMacVtapModel
		return nil
	case enlightenedNetModelStr:
		*n = NetXConnectEnlightenedModel
		return nil
	case tcFilterNetModelStr:
		*n = NetXConnectTCFilterModel
		return nil
	case noneNetModelStr:
		*n = NetXConnectNoneModel
		return nil
	}
	return fmt.Errorf("Unknown type %s", modelName)
}

// DefaultNetInterworkingModel is a package level default
// that determines how the VM should be connected to the
// the container network interface
var DefaultNetInterworkingModel = NetXConnectMacVtapModel

// Introduces constants related to networking
const (
	defaultRouteDest  = "0.0.0.0/0"
	defaultRouteLabel = "default"
	defaultFilePerms  = 0600
	defaultQlen       = 1500
)

// DNSInfo describes the DNS setup related to a network interface.
type DNSInfo struct {
	Servers  []string
	Domain   string
	Searches []string
	Options  []string
}

// NetlinkIface describes fully a network interface.
type NetlinkIface struct {
	netlink.LinkAttrs
	Type string
}

// NetworkInfo gathers all information related to a network interface.
// It can be used to store the description of the underlying network.
type NetworkInfo struct {
	Iface  NetlinkIface
	Addrs  []netlink.Addr
	Routes []netlink.Route
	DNS    DNSInfo
}

// NetworkInterface defines a network interface.
type NetworkInterface struct {
	Name     string
	HardAddr string
	Addrs    []netlink.Addr
}

// TapInterface defines a tap interface
type TapInterface struct {
	ID       string
	Name     string
	TAPIface NetworkInterface
	VMFds    []*os.File
	VhostFds []*os.File
}

// NetworkInterfacePair defines a pair between VM and virtual network interfaces.
type NetworkInterfacePair struct {
	TapInterface
	VirtIface NetworkInterface
	NetInterworkingModel
}

// NetworkConfig is the network configuration related to a network.
type NetworkConfig struct {
	NetNSPath         string
	NetNsCreated      bool
	DisableNewNetNs   bool
	NetmonConfig      NetmonConfig
	InterworkingModel NetInterworkingModel
}

func networkLogger() *logrus.Entry {
	return virtLog.WithField("subsystem", "network")
}

// NetworkNamespace contains all data related to its network namespace.
type NetworkNamespace struct {
	NetNsPath    string
	NetNsCreated bool
	Endpoints    []Endpoint
	NetmonPID    int
}

// TypedJSONEndpoint is used as an intermediate representation for
// marshalling and unmarshalling Endpoint objects.
type TypedJSONEndpoint struct {
	Type EndpointType
	Data json.RawMessage
}

// MarshalJSON is the custom NetworkNamespace JSON marshalling routine.
// This is needed to properly marshall Endpoints array.
func (n NetworkNamespace) MarshalJSON() ([]byte, error) {
	// We need a shadow structure in order to prevent json from
	// entering a recursive loop when only calling json.Marshal().
	type shadow struct {
		NetNsPath    string
		NetNsCreated bool
		Endpoints    []TypedJSONEndpoint
	}

	s := &shadow{
		NetNsPath:    n.NetNsPath,
		NetNsCreated: n.NetNsCreated,
	}

	var typedEndpoints []TypedJSONEndpoint
	for _, endpoint := range n.Endpoints {
		tempJSON, _ := json.Marshal(endpoint)

		t := TypedJSONEndpoint{
			Type: endpoint.Type(),
			Data: tempJSON,
		}

		typedEndpoints = append(typedEndpoints, t)
	}

	s.Endpoints = typedEndpoints

	b, err := json.Marshal(s)
	return b, err
}

func generateEndpoints(typedEndpoints []TypedJSONEndpoint) ([]Endpoint, error) {
	var endpoints []Endpoint

	for _, e := range typedEndpoints {
		switch e.Type {
		case PhysicalEndpointType:
			var endpoint PhysicalEndpoint
			err := json.Unmarshal(e.Data, &endpoint)
			if err != nil {
				return nil, err
			}

			endpoints = append(endpoints, &endpoint)
			networkLogger().WithFields(logrus.Fields{
				"endpoint":      endpoint,
				"endpoint-type": "physical",
			}).Info("endpoint unmarshalled")

		case VethEndpointType:
			var endpoint VethEndpoint
			err := json.Unmarshal(e.Data, &endpoint)
			if err != nil {
				return nil, err
			}

			endpoints = append(endpoints, &endpoint)
			networkLogger().WithFields(logrus.Fields{
				"endpoint":      endpoint,
				"endpoint-type": "virtual",
			}).Info("endpoint unmarshalled")

		case VhostUserEndpointType:
			var endpoint VhostUserEndpoint
			err := json.Unmarshal(e.Data, &endpoint)
			if err != nil {
				return nil, err
			}

			endpoints = append(endpoints, &endpoint)
			networkLogger().WithFields(logrus.Fields{
				"endpoint":      endpoint,
				"endpoint-type": "vhostuser",
			}).Info("endpoint unmarshalled")

		case BridgedMacvlanEndpointType:
			var endpoint BridgedMacvlanEndpoint
			err := json.Unmarshal(e.Data, &endpoint)
			if err != nil {
				return nil, err
			}

			endpoints = append(endpoints, &endpoint)
			networkLogger().WithFields(logrus.Fields{
				"endpoint":      endpoint,
				"endpoint-type": "macvlan",
			}).Info("endpoint unmarshalled")

		case MacvtapEndpointType:
			var endpoint MacvtapEndpoint
			err := json.Unmarshal(e.Data, &endpoint)
			if err != nil {
				return nil, err
			}

			endpoints = append(endpoints, &endpoint)
			networkLogger().WithFields(logrus.Fields{
				"endpoint":      endpoint,
				"endpoint-type": "macvtap",
			}).Info("endpoint unmarshalled")

		case TapEndpointType:
			var endpoint TapEndpoint
			err := json.Unmarshal(e.Data, &endpoint)
			if err != nil {
				return nil, err
			}

			endpoints = append(endpoints, &endpoint)
			networkLogger().WithFields(logrus.Fields{
				"endpoint":      endpoint,
				"endpoint-type": "tap",
			}).Info("endpoint unmarshalled")

		default:
			networkLogger().WithField("endpoint-type", e.Type).Error("Ignoring unknown endpoint type")
		}
	}
	return endpoints, nil
}

// UnmarshalJSON is the custom NetworkNamespace unmarshalling routine.
// This is needed for unmarshalling the Endpoints interfaces array.
func (n *NetworkNamespace) UnmarshalJSON(b []byte) error {
	var s struct {
		NetNsPath    string
		NetNsCreated bool
		Endpoints    json.RawMessage
	}

	if err := json.Unmarshal(b, &s); err != nil {
		return err
	}

	(*n).NetNsPath = s.NetNsPath
	(*n).NetNsCreated = s.NetNsCreated

	var typedEndpoints []TypedJSONEndpoint
	if err := json.Unmarshal([]byte(string(s.Endpoints)), &typedEndpoints); err != nil {
		return err
	}
	endpoints, err := generateEndpoints(typedEndpoints)
	if err != nil {
		return err
	}

	(*n).Endpoints = endpoints
	return nil
}

// NetworkModel describes the type of network specification.
type NetworkModel string

const (
	// NoopNetworkModel is the No-Op network.
	NoopNetworkModel NetworkModel = "noop"

	// DefaultNetworkModel is the default network.
	DefaultNetworkModel NetworkModel = "default"
)

// Set sets a network type based on the input string.
func (networkType *NetworkModel) Set(value string) error {
	switch value {
	case "noop":
		*networkType = NoopNetworkModel
		return nil
	case "default":
		*networkType = DefaultNetworkModel
		return nil
	default:
		return fmt.Errorf("Unknown network type %s", value)
	}
}

// String converts a network type to a string.
func (networkType *NetworkModel) String() string {
	switch *networkType {
	case NoopNetworkModel:
		return string(NoopNetworkModel)
	case DefaultNetworkModel:
		return string(DefaultNetworkModel)
	default:
		return ""
	}
}

// newNetwork returns a network from a network type.
func newNetwork(networkType NetworkModel) network {
	switch networkType {
	case NoopNetworkModel:
		return &noopNetwork{}
	case DefaultNetworkModel:
		return &defNetwork{}
	default:
		return &noopNetwork{}
	}
}

func createLink(netHandle *netlink.Handle, name string, expectedLink netlink.Link, queues int) (netlink.Link, []*os.File, error) {
	var newLink netlink.Link
	var fds []*os.File

	switch expectedLink.Type() {
	case (&netlink.Bridge{}).Type():
		newLink = &netlink.Bridge{
			LinkAttrs:         netlink.LinkAttrs{Name: name},
			MulticastSnooping: expectedLink.(*netlink.Bridge).MulticastSnooping,
		}
	case (&netlink.Tuntap{}).Type():
		flags := netlink.TUNTAP_VNET_HDR
		if queues > 0 {
			flags |= netlink.TUNTAP_MULTI_QUEUE_DEFAULTS
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
	case *BridgedMacvlanEndpoint:
		link = &netlink.Macvlan{}
	case *IPVlanEndpoint:
		link = &netlink.IPVlan{}
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
	case (&netlink.Bridge{}).Type():
		if l, ok := link.(*netlink.Bridge); ok {
			return l, nil
		}
	case (&netlink.Tuntap{}).Type():
		if l, ok := link.(*netlink.GenericLink); ok {
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
func xConnectVMNetwork(endpoint Endpoint, h hypervisor) error {
	netPair := endpoint.NetworkPair()

	queues := 0
	caps := h.capabilities()
	if caps.isMultiQueueSupported() {
		queues = int(h.hypervisorConfig().NumVCPUs)
	}

	disableVhostNet := h.hypervisorConfig().DisableVhostNet

	if netPair.NetInterworkingModel == NetXConnectDefaultModel {
		netPair.NetInterworkingModel = DefaultNetInterworkingModel
	}

	switch netPair.NetInterworkingModel {
	case NetXConnectBridgedModel:
		return bridgeNetworkPair(endpoint, queues, disableVhostNet)
	case NetXConnectMacVtapModel:
		return tapNetworkPair(endpoint, queues, disableVhostNet)
	case NetXConnectTCFilterModel:
		return setupTCFiltering(endpoint, queues, disableVhostNet)
	case NetXConnectEnlightenedModel:
		return fmt.Errorf("Unsupported networking model")
	default:
		return fmt.Errorf("Invalid internetworking model")
	}
}

// The endpoint type should dictate how the disconnection needs to happen.
func xDisconnectVMNetwork(endpoint Endpoint) error {
	netPair := endpoint.NetworkPair()

	if netPair.NetInterworkingModel == NetXConnectDefaultModel {
		netPair.NetInterworkingModel = DefaultNetInterworkingModel
	}

	switch netPair.NetInterworkingModel {
	case NetXConnectBridgedModel:
		return unBridgeNetworkPair(endpoint)
	case NetXConnectMacVtapModel:
		return untapNetworkPair(endpoint)
	case NetXConnectTCFilterModel:
		return removeTCFiltering(endpoint)
	case NetXConnectEnlightenedModel:
		return fmt.Errorf("Unsupported networking model")
	default:
		return fmt.Errorf("Invalid internetworking model")
	}
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
// All the kernel does not check if the link id conflicts with a link id on the host
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

func tapNetworkPair(endpoint Endpoint, queues int, disableVhostNet bool) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Delete()

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
		return fmt.Errorf("Could not set MAC address %s for veth interface %s: %s",
			netPair.VirtIface.HardAddr, netPair.VirtIface.Name, err)
	}

	if err := netHandle.LinkSetUp(tapLink); err != nil {
		return fmt.Errorf("Could not enable TAP %s: %s", netPair.TAPIface.Name, err)
	}

	// Clear the IP addresses from the veth interface to prevent ARP conflict
	netPair.VirtIface.Addrs, err = netlink.AddrList(link, netlink.FAMILY_V4)
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

func bridgeNetworkPair(endpoint Endpoint, queues int, disableVhostNet bool) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Delete()

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
	// to build the hypervisor command line. This MAC address has to be
	// the one inside the VM in order to avoid any firewall issues. The
	// bridge created by the network plugin on the host actually expects
	// to see traffic from this MAC address and not another one.
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

	mcastSnoop := false
	bridgeLink, _, err := createLink(netHandle, netPair.Name, &netlink.Bridge{MulticastSnooping: &mcastSnoop}, queues)
	if err != nil {
		return fmt.Errorf("Could not create bridge: %s", err)
	}

	if err := netHandle.LinkSetMaster(tapLink, bridgeLink.(*netlink.Bridge)); err != nil {
		return fmt.Errorf("Could not attach TAP %s to the bridge %s: %s",
			netPair.TAPIface.Name, netPair.Name, err)
	}

	if err := netHandle.LinkSetUp(tapLink); err != nil {
		return fmt.Errorf("Could not enable TAP %s: %s", netPair.TAPIface.Name, err)
	}

	if err := netHandle.LinkSetMaster(link, bridgeLink.(*netlink.Bridge)); err != nil {
		return fmt.Errorf("Could not attach veth %s to the bridge %s: %s",
			netPair.VirtIface.Name, netPair.Name, err)
	}

	if err := netHandle.LinkSetUp(link); err != nil {
		return fmt.Errorf("Could not enable veth %s: %s", netPair.VirtIface.Name, err)
	}

	if err := netHandle.LinkSetUp(bridgeLink); err != nil {
		return fmt.Errorf("Could not enable bridge %s: %s", netPair.Name, err)
	}

	return nil
}

func setupTCFiltering(endpoint Endpoint, queues int, disableVhostNet bool) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Delete()

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
	// to build the hypervisor command line. This MAC address has to be
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

// addQdiscIngress creates a new qdisc for nwtwork interface with the specified network index
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

func untapNetworkPair(endpoint Endpoint) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Delete()

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

func unBridgeNetworkPair(endpoint Endpoint) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Delete()

	netPair := endpoint.NetworkPair()

	tapLink, err := getLinkByName(netHandle, netPair.TAPIface.Name, &netlink.Tuntap{})
	if err != nil {
		return fmt.Errorf("Could not get TAP interface: %s", err)
	}

	bridgeLink, err := getLinkByName(netHandle, netPair.Name, &netlink.Bridge{})
	if err != nil {
		return fmt.Errorf("Could not get bridge interface: %s", err)
	}

	if err := netHandle.LinkSetDown(bridgeLink); err != nil {
		return fmt.Errorf("Could not disable bridge %s: %s", netPair.Name, err)
	}

	if err := netHandle.LinkSetDown(tapLink); err != nil {
		return fmt.Errorf("Could not disable TAP %s: %s", netPair.TAPIface.Name, err)
	}

	if err := netHandle.LinkSetNoMaster(tapLink); err != nil {
		return fmt.Errorf("Could not detach TAP %s: %s", netPair.TAPIface.Name, err)
	}

	if err := netHandle.LinkDel(bridgeLink); err != nil {
		return fmt.Errorf("Could not remove bridge %s: %s", netPair.Name, err)
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

	if err := netHandle.LinkSetNoMaster(link); err != nil {
		return fmt.Errorf("Could not detach veth %s: %s", netPair.VirtIface.Name, err)
	}

	return nil
}

func removeTCFiltering(endpoint Endpoint) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Delete()

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

func createNetNS() (string, error) {
	n, err := ns.NewNS()
	if err != nil {
		return "", err
	}

	return n.Path(), nil
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

func generateInterfacesAndRoutes(networkNS NetworkNamespace) ([]*types.Interface, []*types.Route, error) {

	if networkNS.NetNsPath == "" {
		return nil, nil, nil
	}

	var routes []*types.Route
	var ifaces []*types.Interface

	for _, endpoint := range networkNS.Endpoints {

		var ipAddresses []*types.IPAddress
		for _, addr := range endpoint.Properties().Addrs {
			// Skip IPv6 because not supported
			if addr.IP.To4() == nil {
				// Skip IPv6 because not supported
				networkLogger().WithFields(logrus.Fields{
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
			ipAddress := types.IPAddress{
				Family:  netlink.FAMILY_V4,
				Address: addr.IP.String(),
				Mask:    fmt.Sprintf("%d", netMask),
			}
			ipAddresses = append(ipAddresses, &ipAddress)
		}
		ifc := types.Interface{
			IPAddresses: ipAddresses,
			Device:      endpoint.Name(),
			Name:        endpoint.Name(),
			Mtu:         uint64(endpoint.Properties().Iface.MTU),
			HwAddr:      endpoint.HardwareAddr(),
			PciAddr:     endpoint.PciAddr(),
		}

		ifaces = append(ifaces, &ifc)

		for _, route := range endpoint.Properties().Routes {
			var r types.Route

			if route.Dst != nil {
				r.Dest = route.Dst.String()

				if route.Dst.IP.To4() == nil {
					// Skip IPv6 because not supported
					networkLogger().WithFields(logrus.Fields{
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
					networkLogger().WithFields(logrus.Fields{
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

func createNetworkInterfacePair(idx int, ifName string, interworkingModel NetInterworkingModel) (NetworkInterfacePair, error) {
	uniqueID := uuid.Generate().String()

	randomMacAddr, err := generateRandomPrivateMacAddr()
	if err != nil {
		return NetworkInterfacePair{}, fmt.Errorf("Could not generate random mac address: %s", err)
	}

	netPair := NetworkInterfacePair{
		TapInterface: TapInterface{
			ID:   uniqueID,
			Name: fmt.Sprintf("br%d_kata", idx),
			TAPIface: NetworkInterface{
				Name: fmt.Sprintf("tap%d_kata", idx),
			},
		},
		VirtIface: NetworkInterface{
			Name:     fmt.Sprintf("eth%d", idx),
			HardAddr: randomMacAddr,
		},
		NetInterworkingModel: interworkingModel,
	}

	return netPair, nil
}

func generateRandomPrivateMacAddr() (string, error) {
	buf := make([]byte, 6)
	_, err := cryptoRand.Read(buf)
	if err != nil {
		return "", err
	}

	// Set the local bit for local addresses
	// Addresses in this range are local mac addresses:
	// x2-xx-xx-xx-xx-xx , x6-xx-xx-xx-xx-xx , xA-xx-xx-xx-xx-xx , xE-xx-xx-xx-xx-xx
	buf[0] = (buf[0] | 2) & 0xfe

	hardAddr := net.HardwareAddr(buf)
	return hardAddr.String(), nil
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

	return NetworkInfo{
		Iface: NetlinkIface{
			LinkAttrs: *(link.Attrs()),
			Type:      link.Type(),
		},
		Addrs:  addrs,
		Routes: routes,
	}, nil
}

func createEndpointsFromScan(networkNSPath string, config NetworkConfig) ([]Endpoint, error) {
	var endpoints []Endpoint

	netnsHandle, err := netns.GetFromPath(networkNSPath)
	if err != nil {
		return []Endpoint{}, err
	}
	defer netnsHandle.Close()

	netlinkHandle, err := netlink.NewHandleAt(netnsHandle)
	if err != nil {
		return []Endpoint{}, err
	}
	defer netlinkHandle.Delete()

	linkList, err := netlinkHandle.LinkList()
	if err != nil {
		return []Endpoint{}, err
	}

	idx := 0
	for _, link := range linkList {
		var (
			endpoint  Endpoint
			errCreate error
		)

		netInfo, err := networkInfoFromLink(netlinkHandle, link)
		if err != nil {
			return []Endpoint{}, err
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

		if err := doNetNS(networkNSPath, func(_ ns.NetNS) error {
			endpoint, errCreate = createEndpoint(netInfo, idx, config.InterworkingModel)
			return errCreate
		}); err != nil {
			return []Endpoint{}, err
		}

		endpoint.SetProperties(netInfo)
		endpoints = append(endpoints, endpoint)

		idx++
	}

	sort.Slice(endpoints, func(i, j int) bool {
		return endpoints[i].Name() < endpoints[j].Name()
	})

	networkLogger().WithField("endpoints", endpoints).Info("Endpoints found after scan")

	return endpoints, nil
}

func createEndpoint(netInfo NetworkInfo, idx int, model NetInterworkingModel) (Endpoint, error) {
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
		networkLogger().WithField("interface", netInfo.Iface.Name).Info("Physical network interface found")
		endpoint, err = createPhysicalEndpoint(netInfo)
	} else {
		var socketPath string

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
			endpoint, err = createBridgedMacvlanNetworkEndpoint(idx, netInfo.Iface.Name, model)
		} else if netInfo.Iface.Type == "macvtap" {
			networkLogger().Infof("macvtap interface found")
			endpoint, err = createMacvtapNetworkEndpoint(netInfo)
		} else if netInfo.Iface.Type == "tap" {
			networkLogger().Info("tap interface found")
			endpoint, err = createTapNetworkEndpoint(idx, netInfo.Iface.Name)
		} else if netInfo.Iface.Type == "veth" {
			endpoint, err = createVethNetworkEndpoint(idx, netInfo.Iface.Name, model)
		} else if netInfo.Iface.Type == "ipvlan" {
			endpoint, err = createIPVlanNetworkEndpoint(idx, netInfo.Iface.Name)
		} else {
			return nil, fmt.Errorf("Unsupported network interface")
		}
	}

	return endpoint, err
}

// network is the virtcontainers network interface.
// Container network plugins are used to setup virtual network
// between VM netns and the host network physical interface.
type network interface {
	// run runs a callback function in a specified network namespace.
	run(networkNSPath string, cb func() error) error

	// add adds all needed interfaces inside the network namespace.
	add(sandbox *Sandbox, hotplug bool) error

	// remove unbridges and deletes TAP interfaces. It also removes virtual network
	// interfaces and deletes the network namespace.
	remove(sandbox *Sandbox, hotunplug bool) error
}
