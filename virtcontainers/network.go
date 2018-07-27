// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"math/rand"
	"net"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/safchain/ethtool"
	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
	"golang.org/x/sys/unix"

	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
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

	// NetXConnectInvalidModel is the last item to check valid values by IsValid()
	NetXConnectInvalidModel
)

//IsValid checks if a model is valid
func (n NetInterworkingModel) IsValid() bool {
	return 0 <= int(n) && int(n) < int(NetXConnectInvalidModel)
}

//SetModel change the model string value
func (n *NetInterworkingModel) SetModel(modelName string) error {
	switch modelName {
	case "default":
		*n = DefaultNetInterworkingModel
		return nil
	case "bridged":
		*n = NetXConnectBridgedModel
		return nil
	case "macvtap":
		*n = NetXConnectMacVtapModel
		return nil
	case "enlightened":
		*n = NetXConnectEnlightenedModel
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
	defaultQueues     = 8
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

// NetworkInterfacePair defines a pair between VM and virtual network interfaces.
type NetworkInterfacePair struct {
	ID        string
	Name      string
	VirtIface NetworkInterface
	TAPIface  NetworkInterface
	NetInterworkingModel
	VMFds    []*os.File
	VhostFds []*os.File
}

// NetworkConfig is the network configuration related to a network.
type NetworkConfig struct {
	NetNSPath         string
	NumInterfaces     int
	InterworkingModel NetInterworkingModel
}

// Endpoint represents a physical or virtual network interface.
type Endpoint interface {
	Properties() NetworkInfo
	Name() string
	HardwareAddr() string
	Type() EndpointType

	SetProperties(NetworkInfo)
	Attach(hypervisor) error
	Detach(netNsCreated bool, netNsPath string) error
}

// VirtualEndpoint gathers a network pair and its properties.
type VirtualEndpoint struct {
	NetPair            NetworkInterfacePair
	EndpointProperties NetworkInfo
	Physical           bool
	EndpointType       EndpointType
}

// PhysicalEndpoint gathers a physical network interface and its properties
type PhysicalEndpoint struct {
	IfaceName          string
	HardAddr           string
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
	BDF                string
	Driver             string
	VendorDeviceID     string
}

// VhostUserEndpoint represents a vhost-user socket based network interface
type VhostUserEndpoint struct {
	// Path to the vhost-user socket on the host system
	SocketPath string
	// MAC address of the interface
	HardAddr           string
	IfaceName          string
	EndpointProperties NetworkInfo
	EndpointType       EndpointType
}

// Properties returns properties for the veth interface in the network pair.
func (endpoint *VirtualEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the veth interface in the network pair.
func (endpoint *VirtualEndpoint) Name() string {
	return endpoint.NetPair.VirtIface.Name
}

// HardwareAddr returns the mac address that is assigned to the tap interface
// in th network pair.
func (endpoint *VirtualEndpoint) HardwareAddr() string {
	return endpoint.NetPair.TAPIface.HardAddr
}

// Type identifies the endpoint as a virtual endpoint.
func (endpoint *VirtualEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// SetProperties sets the properties for the endpoint.
func (endpoint *VirtualEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

func networkLogger() *logrus.Entry {
	return virtLog.WithField("subsystem", "network")
}

// Attach for virtual endpoint bridges the network pair and adds the
// tap interface of the network pair to the hypervisor.
func (endpoint *VirtualEndpoint) Attach(h hypervisor) error {
	networkLogger().Info("Attaching virtual endpoint")
	if err := xconnectVMNetwork(&(endpoint.NetPair), true); err != nil {
		networkLogger().WithError(err).Error("Error bridging virtual ep")
		return err
	}

	return h.addDevice(endpoint, netDev)
}

// Detach for the virtual endpoint tears down the tap and bridge
// created for the veth interface.
func (endpoint *VirtualEndpoint) Detach(netNsCreated bool, netNsPath string) error {
	// The network namespace would have been deleted at this point
	// if it has not been created by virtcontainers.
	if !netNsCreated {
		return nil
	}

	networkLogger().Info("Detaching virtual endpoint")

	return doNetNS(netNsPath, func(_ ns.NetNS) error {
		return xconnectVMNetwork(&(endpoint.NetPair), false)
	})
}

// Properties returns the properties of the interface.
func (endpoint *VhostUserEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// Name returns name of the interface.
func (endpoint *VhostUserEndpoint) Name() string {
	return endpoint.IfaceName
}

// HardwareAddr returns the mac address of the vhostuser network interface
func (endpoint *VhostUserEndpoint) HardwareAddr() string {
	return endpoint.HardAddr
}

// Type indentifies the endpoint as a vhostuser endpoint.
func (endpoint *VhostUserEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// SetProperties sets the properties of the endpoint.
func (endpoint *VhostUserEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// Attach for vhostuser endpoint
func (endpoint *VhostUserEndpoint) Attach(h hypervisor) error {
	networkLogger().Info("Attaching vhostuser based endpoint")

	// Generate a unique ID to be used for hypervisor commandline fields
	randBytes, err := utils.GenerateRandomBytes(8)
	if err != nil {
		return err
	}
	id := hex.EncodeToString(randBytes)

	d := &drivers.VhostUserNetDevice{
		MacAddress: endpoint.HardAddr,
	}
	d.SocketPath = endpoint.SocketPath
	d.ID = id

	return h.addDevice(d, vhostuserDev)
}

// Detach for vhostuser endpoint
func (endpoint *VhostUserEndpoint) Detach(netNsCreated bool, netNsPath string) error {
	networkLogger().Info("Detaching vhostuser based endpoint")
	return nil
}

// Create a vhostuser endpoint
func createVhostUserEndpoint(netInfo NetworkInfo, socket string) (*VhostUserEndpoint, error) {

	vhostUserEndpoint := &VhostUserEndpoint{
		SocketPath:   socket,
		HardAddr:     netInfo.Iface.HardwareAddr.String(),
		IfaceName:    netInfo.Iface.Name,
		EndpointType: VhostUserEndpointType,
	}
	return vhostUserEndpoint, nil
}

// Properties returns the properties of the physical interface.
func (endpoint *PhysicalEndpoint) Properties() NetworkInfo {
	return endpoint.EndpointProperties
}

// HardwareAddr returns the mac address of the physical network interface.
func (endpoint *PhysicalEndpoint) HardwareAddr() string {
	return endpoint.HardAddr
}

// Name returns name of the physical interface.
func (endpoint *PhysicalEndpoint) Name() string {
	return endpoint.IfaceName
}

// Type indentifies the endpoint as a physical endpoint.
func (endpoint *PhysicalEndpoint) Type() EndpointType {
	return endpoint.EndpointType
}

// SetProperties sets the properties of the physical endpoint.
func (endpoint *PhysicalEndpoint) SetProperties(properties NetworkInfo) {
	endpoint.EndpointProperties = properties
}

// Attach for physical endpoint binds the physical network interface to
// vfio-pci and adds device to the hypervisor with vfio-passthrough.
func (endpoint *PhysicalEndpoint) Attach(h hypervisor) error {
	networkLogger().Info("Attaching physical endpoint")

	// Unbind physical interface from host driver and bind to vfio
	// so that it can be passed to qemu.
	if err := bindNICToVFIO(endpoint); err != nil {
		return err
	}

	d := drivers.VFIODevice{
		BDF: endpoint.BDF,
	}

	return h.addDevice(d, vfioDev)
}

// Detach for physical endpoint unbinds the physical network interface from vfio-pci
// and binds it back to the saved host driver.
func (endpoint *PhysicalEndpoint) Detach(netNsCreated bool, netNsPath string) error {
	// Bind back the physical network interface to host.
	// We need to do this even if a new network namespace has not
	// been created by virtcontainers.
	networkLogger().Info("Detaching physical endpoint")

	// We do not need to enter the network namespace to bind back the
	// physical interface to host driver.
	return bindNICToHost(endpoint)
}

// EndpointType identifies the type of the network endpoint.
type EndpointType string

const (
	// PhysicalEndpointType is the physical network interface.
	PhysicalEndpointType EndpointType = "physical"

	// VirtualEndpointType is the virtual network interface.
	VirtualEndpointType EndpointType = "virtual"

	// VhostUserEndpointType is the vhostuser network interface.
	VhostUserEndpointType EndpointType = "vhost-user"
)

// Set sets an endpoint type based on the input string.
func (endpointType *EndpointType) Set(value string) error {
	switch value {
	case "physical":
		*endpointType = PhysicalEndpointType
		return nil
	case "virtual":
		*endpointType = VirtualEndpointType
		return nil
	case "vhost-user":
		*endpointType = VhostUserEndpointType
		return nil
	default:
		return fmt.Errorf("Unknown endpoint type %s", value)
	}
}

// String converts an endpoint type to a string.
func (endpointType *EndpointType) String() string {
	switch *endpointType {
	case PhysicalEndpointType:
		return string(PhysicalEndpointType)
	case VirtualEndpointType:
		return string(VirtualEndpointType)
	case VhostUserEndpointType:
		return string(VhostUserEndpointType)
	default:
		return ""
	}
}

// NetworkNamespace contains all data related to its network namespace.
type NetworkNamespace struct {
	NetNsPath    string
	NetNsCreated bool
	Endpoints    []Endpoint
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

	var endpoints []Endpoint

	for _, e := range typedEndpoints {
		switch e.Type {
		case PhysicalEndpointType:
			var endpoint PhysicalEndpoint
			err := json.Unmarshal(e.Data, &endpoint)
			if err != nil {
				return err
			}

			endpoints = append(endpoints, &endpoint)
			virtLog.Infof("Physical endpoint unmarshalled [%v]", endpoint)

		case VirtualEndpointType:
			var endpoint VirtualEndpoint
			err := json.Unmarshal(e.Data, &endpoint)
			if err != nil {
				return err
			}

			endpoints = append(endpoints, &endpoint)
			virtLog.Infof("Virtual endpoint unmarshalled [%v]", endpoint)

		case VhostUserEndpointType:
			var endpoint VhostUserEndpoint
			err := json.Unmarshal(e.Data, &endpoint)
			if err != nil {
				return err
			}

			endpoints = append(endpoints, &endpoint)
			virtLog.Infof("VhostUser endpoint unmarshalled [%v]", endpoint)

		default:
			virtLog.Errorf("Unknown endpoint type received %s\n", e.Type)
		}
	}

	(*n).Endpoints = endpoints
	return nil
}

// NetworkModel describes the type of network specification.
type NetworkModel string

const (
	// NoopNetworkModel is the No-Op network.
	NoopNetworkModel NetworkModel = "noop"

	// CNINetworkModel is the CNI network.
	CNINetworkModel NetworkModel = "CNI"

	// CNMNetworkModel is the CNM network.
	CNMNetworkModel NetworkModel = "CNM"
)

// Set sets a network type based on the input string.
func (networkType *NetworkModel) Set(value string) error {
	switch value {
	case "noop":
		*networkType = NoopNetworkModel
		return nil
	case "CNI":
		*networkType = CNINetworkModel
		return nil
	case "CNM":
		*networkType = CNMNetworkModel
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
	case CNINetworkModel:
		return string(CNINetworkModel)
	case CNMNetworkModel:
		return string(CNMNetworkModel)
	default:
		return ""
	}
}

// newNetwork returns a network from a network type.
func newNetwork(networkType NetworkModel) network {
	switch networkType {
	case NoopNetworkModel:
		return &noopNetwork{}
	case CNINetworkModel:
		return &cni{}
	case CNMNetworkModel:
		return &cnm{}
	default:
		return &noopNetwork{}
	}
}

func initNetworkCommon(config NetworkConfig) (string, bool, error) {
	if !config.InterworkingModel.IsValid() || config.InterworkingModel == NetXConnectDefaultModel {
		config.InterworkingModel = DefaultNetInterworkingModel
	}

	if config.NetNSPath == "" {
		path, err := createNetNS()
		if err != nil {
			return "", false, err
		}

		return path, true, nil
	}

	return config.NetNSPath, false, nil
}

func runNetworkCommon(networkNSPath string, cb func() error) error {
	if networkNSPath == "" {
		return fmt.Errorf("networkNSPath cannot be empty")
	}

	return doNetNS(networkNSPath, func(_ ns.NetNS) error {
		return cb()
	})
}

func addNetworkCommon(sandbox *Sandbox, networkNS *NetworkNamespace) error {
	err := doNetNS(networkNS.NetNsPath, func(_ ns.NetNS) error {
		for _, endpoint := range networkNS.Endpoints {
			if err := endpoint.Attach(sandbox.hypervisor); err != nil {
				return err
			}
		}

		return nil
	})

	return err
}

func removeNetworkCommon(networkNS NetworkNamespace, netNsCreated bool) error {
	for _, endpoint := range networkNS.Endpoints {
		// Detach for an endpoint should enter the network namespace
		// if required.
		if err := endpoint.Detach(netNsCreated, networkNS.NetNsPath); err != nil {
			return err
		}
	}

	return nil
}

func createLink(netHandle *netlink.Handle, name string, expectedLink netlink.Link) (netlink.Link, []*os.File, error) {
	var newLink netlink.Link
	var fds []*os.File

	switch expectedLink.Type() {
	case (&netlink.Bridge{}).Type():
		newLink = &netlink.Bridge{
			LinkAttrs:         netlink.LinkAttrs{Name: name},
			MulticastSnooping: expectedLink.(*netlink.Bridge).MulticastSnooping,
		}
	case (&netlink.Tuntap{}).Type():
		newLink = &netlink.Tuntap{
			LinkAttrs: netlink.LinkAttrs{Name: name},
			Mode:      netlink.TUNTAP_MODE_TAP,
			Queues:    defaultQueues,
			Flags:     netlink.TUNTAP_MULTI_QUEUE_DEFAULTS | netlink.TUNTAP_VNET_HDR,
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
	default:
		return nil, fmt.Errorf("Unsupported link type %s", expectedLink.Type())
	}

	return nil, fmt.Errorf("Incorrect link type %s, expecting %s", link.Type(), expectedLink.Type())
}

// The endpoint type should dictate how the connection needs to be made
func xconnectVMNetwork(netPair *NetworkInterfacePair, connect bool) error {
	if netPair.NetInterworkingModel == NetXConnectDefaultModel {
		netPair.NetInterworkingModel = DefaultNetInterworkingModel
	}
	switch netPair.NetInterworkingModel {
	case NetXConnectBridgedModel:
		netPair.NetInterworkingModel = NetXConnectBridgedModel
		if connect {
			return bridgeNetworkPair(netPair)
		}
		return unBridgeNetworkPair(*netPair)
	case NetXConnectMacVtapModel:
		netPair.NetInterworkingModel = NetXConnectMacVtapModel
		if connect {
			return tapNetworkPair(netPair)
		}
		return untapNetworkPair(*netPair)
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

func createMacVtap(netHandle *netlink.Handle, name string, link netlink.Link) (taplink netlink.Link, err error) {

	if !macvtapWorkaround {
		taplink, _, err = createLink(netHandle, name, link)
		return
	}

	r := rand.New(rand.NewSource(time.Now().UnixNano()))

	for i := 0; i < linkRetries; i++ {
		index := hostLinkOffset + (r.Int() & linkRange)
		link.Attrs().Index = index
		taplink, _, err = createLink(netHandle, name, link)
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

func tapNetworkPair(netPair *NetworkInterfacePair) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Delete()

	vethLink, err := getLinkByName(netHandle, netPair.VirtIface.Name, &netlink.Veth{})
	if err != nil {
		return fmt.Errorf("Could not get veth interface: %s: %s", netPair.VirtIface.Name, err)
	}
	vethLinkAttrs := vethLink.Attrs()

	// Attach the macvtap interface to the underlying container
	// interface. Also picks relevant attributes from the parent
	tapLink, err := createMacVtap(netHandle, netPair.TAPIface.Name,
		&netlink.Macvtap{
			Macvlan: netlink.Macvlan{
				LinkAttrs: netlink.LinkAttrs{
					TxQLen:      vethLinkAttrs.TxQLen,
					ParentIndex: vethLinkAttrs.Index,
				},
			},
		})

	if err != nil {
		return fmt.Errorf("Could not create TAP interface: %s", err)
	}

	// Save the veth MAC address to the TAP so that it can later be used
	// to build the hypervisor command line. This MAC address has to be
	// the one inside the VM in order to avoid any firewall issues. The
	// bridge created by the network plugin on the host actually expects
	// to see traffic from this MAC address and not another one.
	tapHardAddr := vethLinkAttrs.HardwareAddr
	netPair.TAPIface.HardAddr = vethLinkAttrs.HardwareAddr.String()

	if err := netHandle.LinkSetMTU(tapLink, vethLinkAttrs.MTU); err != nil {
		return fmt.Errorf("Could not set TAP MTU %d: %s", vethLinkAttrs.MTU, err)
	}

	hardAddr, err := net.ParseMAC(netPair.VirtIface.HardAddr)
	if err != nil {
		return err
	}
	if err := netHandle.LinkSetHardwareAddr(vethLink, hardAddr); err != nil {
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
	netPair.VirtIface.Addrs, err = netlink.AddrList(vethLink, netlink.FAMILY_V4)
	if err != nil {
		return fmt.Errorf("Unable to obtain veth IP addresses: %s", err)
	}

	if err := clearIPs(vethLink, netPair.VirtIface.Addrs); err != nil {
		return fmt.Errorf("Unable to clear veth IP addresses: %s", err)
	}

	if err := netHandle.LinkSetUp(vethLink); err != nil {
		return fmt.Errorf("Could not enable veth %s: %s", netPair.VirtIface.Name, err)
	}

	// Note: The underlying interfaces need to be up prior to fd creation.

	// Setup the multiqueue fds to be consumed by QEMU as macvtap cannot
	// be directly connected.
	// Ideally we want
	// netdev.FDs, err = createMacvtapFds(netdev.ID, int(config.SMP.CPUs))

	// We do not have global context here, hence a manifest constant
	// that matches our minimum vCPU configuration
	// Another option is to defer this to ciao qemu library which does have
	// global context but cannot handle errors when setting up the network
	netPair.VMFds, err = createMacvtapFds(tapLink.Attrs().Index, defaultQueues)
	if err != nil {
		return fmt.Errorf("Could not setup macvtap fds %s: %s", netPair.TAPIface, err)
	}

	vhostFds, err := createVhostFds(defaultQueues)
	if err != nil {
		return fmt.Errorf("Could not setup vhost fds %s : %s", netPair.VirtIface.Name, err)
	}
	netPair.VhostFds = vhostFds

	return nil
}

func bridgeNetworkPair(netPair *NetworkInterfacePair) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Delete()

	tapLink, fds, err := createLink(netHandle, netPair.TAPIface.Name, &netlink.Tuntap{})
	if err != nil {
		return fmt.Errorf("Could not create TAP interface: %s", err)
	}
	netPair.VMFds = fds

	vhostFds, err := createVhostFds(defaultQueues)
	if err != nil {
		return fmt.Errorf("Could not setup vhost fds %s : %s", netPair.VirtIface.Name, err)
	}
	netPair.VhostFds = vhostFds

	vethLink, err := getLinkByName(netHandle, netPair.VirtIface.Name, &netlink.Veth{})
	if err != nil {
		return fmt.Errorf("Could not get veth interface %s : %s", netPair.VirtIface.Name, err)
	}

	vethLinkAttrs := vethLink.Attrs()

	// Save the veth MAC address to the TAP so that it can later be used
	// to build the hypervisor command line. This MAC address has to be
	// the one inside the VM in order to avoid any firewall issues. The
	// bridge created by the network plugin on the host actually expects
	// to see traffic from this MAC address and not another one.
	netPair.TAPIface.HardAddr = vethLinkAttrs.HardwareAddr.String()

	if err := netHandle.LinkSetMTU(tapLink, vethLinkAttrs.MTU); err != nil {
		return fmt.Errorf("Could not set TAP MTU %d: %s", vethLinkAttrs.MTU, err)
	}

	hardAddr, err := net.ParseMAC(netPair.VirtIface.HardAddr)
	if err != nil {
		return err
	}
	if err := netHandle.LinkSetHardwareAddr(vethLink, hardAddr); err != nil {
		return fmt.Errorf("Could not set MAC address %s for veth interface %s: %s",
			netPair.VirtIface.HardAddr, netPair.VirtIface.Name, err)
	}

	mcastSnoop := false
	bridgeLink, _, err := createLink(netHandle, netPair.Name, &netlink.Bridge{MulticastSnooping: &mcastSnoop})
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

	if err := netHandle.LinkSetMaster(vethLink, bridgeLink.(*netlink.Bridge)); err != nil {
		return fmt.Errorf("Could not attach veth %s to the bridge %s: %s",
			netPair.VirtIface.Name, netPair.Name, err)
	}

	if err := netHandle.LinkSetUp(vethLink); err != nil {
		return fmt.Errorf("Could not enable veth %s: %s", netPair.VirtIface.Name, err)
	}

	if err := netHandle.LinkSetUp(bridgeLink); err != nil {
		return fmt.Errorf("Could not enable bridge %s: %s", netPair.Name, err)
	}

	return nil
}

func untapNetworkPair(netPair NetworkInterfacePair) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Delete()

	tapLink, err := getLinkByName(netHandle, netPair.TAPIface.Name, &netlink.Macvtap{})
	if err != nil {
		return fmt.Errorf("Could not get TAP interface %s: %s", netPair.TAPIface.Name, err)
	}

	if err := netHandle.LinkDel(tapLink); err != nil {
		return fmt.Errorf("Could not remove TAP %s: %s", netPair.TAPIface.Name, err)
	}

	vethLink, err := getLinkByName(netHandle, netPair.VirtIface.Name, &netlink.Veth{})
	if err != nil {
		// The veth pair is not totally managed by virtcontainers
		virtLog.Warnf("Could not get veth interface %s: %s", netPair.VirtIface.Name, err)
	} else {
		if err := netHandle.LinkSetDown(vethLink); err != nil {
			return fmt.Errorf("Could not disable veth %s: %s", netPair.VirtIface.Name, err)
		}
	}

	// Restore the IPs that were cleared
	err = setIPs(vethLink, netPair.VirtIface.Addrs)
	return err
}

func unBridgeNetworkPair(netPair NetworkInterfacePair) error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Delete()

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

	vethLink, err := getLinkByName(netHandle, netPair.VirtIface.Name, &netlink.Veth{})
	if err != nil {
		// The veth pair is not totally managed by virtcontainers
		virtLog.WithError(err).Warn("Could not get veth interface")
	} else {
		if err := netHandle.LinkSetDown(vethLink); err != nil {
			return fmt.Errorf("Could not disable veth %s: %s", netPair.VirtIface.Name, err)
		}

		if err := netHandle.LinkSetNoMaster(vethLink); err != nil {
			return fmt.Errorf("Could not detach veth %s: %s", netPair.VirtIface.Name, err)
		}

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

func createVirtualNetworkEndpoint(idx int, ifName string, interworkingModel NetInterworkingModel) (*VirtualEndpoint, error) {
	if idx < 0 {
		return &VirtualEndpoint{}, fmt.Errorf("invalid network endpoint index: %d", idx)
	}

	uniqueID := uuid.Generate().String()

	hardAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, byte(idx >> 8), byte(idx)}

	endpoint := &VirtualEndpoint{
		// TODO This is too specific. We may need to create multiple
		// end point types here and then decide how to connect them
		// at the time of hypervisor attach and not here
		NetPair: NetworkInterfacePair{
			ID:   uniqueID,
			Name: fmt.Sprintf("br%d", idx),
			VirtIface: NetworkInterface{
				Name:     fmt.Sprintf("eth%d", idx),
				HardAddr: hardAddr.String(),
			},
			TAPIface: NetworkInterface{
				Name: fmt.Sprintf("tap%d", idx),
			},
			NetInterworkingModel: interworkingModel,
		},
		EndpointType: VirtualEndpointType,
	}

	if ifName != "" {
		endpoint.NetPair.VirtIface.Name = ifName
	}

	return endpoint, nil
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
		var endpoint Endpoint

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

			// TODO: This is the incoming interface
			// based on the incoming interface we should create
			// an appropriate EndPoint based on interface type
			// This should be a switch

			// Check if interface is a physical interface. Do not create
			// tap interface/bridge if it is.
			isPhysical, err := isPhysicalIface(netInfo.Iface.Name)
			if err != nil {
				return err
			}

			if isPhysical {
				cnmLogger().WithField("interface", netInfo.Iface.Name).Info("Physical network interface found")
				endpoint, err = createPhysicalEndpoint(netInfo)
			} else {
				var socketPath string

				// Check if this is a dummy interface which has a vhost-user socket associated with it
				socketPath, err = vhostUserSocketPath(netInfo)
				if err != nil {
					return err
				}

				if socketPath != "" {
					cnmLogger().WithField("interface", netInfo.Iface.Name).Info("VhostUser network interface found")
					endpoint, err = createVhostUserEndpoint(netInfo, socketPath)
				} else {
					endpoint, err = createVirtualNetworkEndpoint(idx, netInfo.Iface.Name, config.InterworkingModel)
				}
			}

			return err
		}); err != nil {
			return []Endpoint{}, err
		}

		endpoint.SetProperties(netInfo)
		endpoints = append(endpoints, endpoint)

		idx++
	}

	return endpoints, nil
}

// isPhysicalIface checks if an interface is a physical device.
// We use ethtool here to not rely on device sysfs inside the network namespace.
func isPhysicalIface(ifaceName string) (bool, error) {
	if ifaceName == "lo" {
		return false, nil
	}

	ethHandle, err := ethtool.NewEthtool()
	if err != nil {
		return false, err
	}

	bus, err := ethHandle.BusInfo(ifaceName)
	if err != nil {
		return false, nil
	}

	// Check for a pci bus format
	tokens := strings.Split(bus, ":")
	if len(tokens) != 3 {
		return false, nil
	}

	return true, nil
}

var sysPCIDevicesPath = "/sys/bus/pci/devices"

func createPhysicalEndpoint(netInfo NetworkInfo) (*PhysicalEndpoint, error) {
	// Get ethtool handle to derive driver and bus
	ethHandle, err := ethtool.NewEthtool()
	if err != nil {
		return nil, err
	}

	// Get BDF
	bdf, err := ethHandle.BusInfo(netInfo.Iface.Name)
	if err != nil {
		return nil, err
	}

	// Get Driver
	driver, err := ethHandle.DriverName(netInfo.Iface.Name)
	if err != nil {
		return nil, err
	}

	// Get vendor and device id from pci space (sys/bus/pci/devices/$bdf)

	ifaceDevicePath := filepath.Join(sysPCIDevicesPath, bdf, "device")
	contents, err := ioutil.ReadFile(ifaceDevicePath)
	if err != nil {
		return nil, err
	}

	deviceID := strings.TrimSpace(string(contents))

	// Vendor id
	ifaceVendorPath := filepath.Join(sysPCIDevicesPath, bdf, "vendor")
	contents, err = ioutil.ReadFile(ifaceVendorPath)
	if err != nil {
		return nil, err
	}

	vendorID := strings.TrimSpace(string(contents))
	vendorDeviceID := fmt.Sprintf("%s %s", vendorID, deviceID)
	vendorDeviceID = strings.TrimSpace(vendorDeviceID)

	physicalEndpoint := &PhysicalEndpoint{
		IfaceName:      netInfo.Iface.Name,
		HardAddr:       netInfo.Iface.HardwareAddr.String(),
		VendorDeviceID: vendorDeviceID,
		EndpointType:   PhysicalEndpointType,
		Driver:         driver,
		BDF:            bdf,
	}

	return physicalEndpoint, nil
}

func bindNICToVFIO(endpoint *PhysicalEndpoint) error {
	return drivers.BindDevicetoVFIO(endpoint.BDF, endpoint.Driver, endpoint.VendorDeviceID)
}

func bindNICToHost(endpoint *PhysicalEndpoint) error {
	return drivers.BindDevicetoHost(endpoint.BDF, endpoint.Driver, endpoint.VendorDeviceID)
}

// Long term, this should be made more configurable.  For now matching path
// provided by CNM VPP and OVS-DPDK plugins, available at github.com/clearcontainers/vpp and
// github.com/clearcontainers/ovsdpdk.  The plugins create the socket on the host system
// using this path.
const hostSocketSearchPath = "/tmp/vhostuser_%s/vhu.sock"

// findVhostUserNetSocketPath checks if an interface is a dummy placeholder
// for a vhost-user socket, and if it is it returns the path to the socket
func findVhostUserNetSocketPath(netInfo NetworkInfo) (string, error) {
	if netInfo.Iface.Name == "lo" {
		return "", nil
	}

	// check for socket file existence at known location.
	for _, addr := range netInfo.Addrs {
		socketPath := fmt.Sprintf(hostSocketSearchPath, addr.IPNet.IP)
		if _, err := os.Stat(socketPath); err == nil {
			return socketPath, nil
		}
	}

	return "", nil
}

// vhostUserSocketPath returns the path of the socket discovered.  This discovery
// will vary depending on the type of vhost-user socket.
//  Today only VhostUserNetDevice is supported.
func vhostUserSocketPath(info interface{}) (string, error) {

	switch v := info.(type) {
	case NetworkInfo:
		return findVhostUserNetSocketPath(v)
	default:
		return "", nil
	}

}

// network is the virtcontainers network interface.
// Container network plugins are used to setup virtual network
// between VM netns and the host network physical interface.
type network interface {
	// init initializes the network, setting a new network namespace.
	init(config NetworkConfig) (string, bool, error)

	// run runs a callback function in a specified network namespace.
	run(networkNSPath string, cb func() error) error

	// add adds all needed interfaces inside the network namespace.
	add(sandbox *Sandbox, config NetworkConfig, netNsPath string, netNsCreated bool) (NetworkNamespace, error)

	// remove unbridges and deletes TAP interfaces. It also removes virtual network
	// interfaces and deletes the network namespace.
	remove(sandbox *Sandbox, networkNS NetworkNamespace, netNsCreated bool) error
}
