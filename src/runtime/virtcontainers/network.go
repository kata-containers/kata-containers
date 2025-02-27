// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	cryptoRand "crypto/rand"
	"fmt"
	"net"
	"os"

	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"
	"go.opentelemetry.io/otel/trace"
	"golang.org/x/sys/unix"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/uuid"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// networkTracingTags defines tags for the trace span
var networkTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "network",
}

func networkLogger() *logrus.Entry {
	return virtLog.WithField("subsystem", "network")
}

var networkTrace = getNetworkTrace("")

func getNetworkTrace(networkType EndpointType) func(ctx context.Context, name string, endpoint interface{}) (trace.Span, context.Context) {
	return func(ctx context.Context, name string, endpoint interface{}) (trace.Span, context.Context) {
		span, ctx := katatrace.Trace(ctx, networkLogger(), name, networkTracingTags)
		if networkType != "" {
			katatrace.AddTags(span, "type", string(networkType))
		}
		if endpoint != nil {
			katatrace.AddTags(span, "endpoint", endpoint)
		}
		return span, ctx
	}
}

func closeSpan(span trace.Span, err error) {
	if err != nil {
		katatrace.AddTags(span, "error", err.Error())
	}
	span.End()
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

// TuntapInterface defines a tap interface
type TuntapInterface struct {
	Name     string
	TAPIface NetworkInterface
}

// NetworkInterfacePair defines a pair between VM and virtual network interfaces.
type NetworkInterfacePair struct {
	TapInterface
	VirtIface NetworkInterface
	NetInterworkingModel
}

// NetlinkIface describes fully a network interface.
type NetlinkIface struct {
	netlink.LinkAttrs
	Type string
}

// NetworkInfo gathers all information related to a network interface.
// It can be used to store the description of the underlying network.
type NetworkInfo struct {
	Iface     NetlinkIface
	DNS       DNSInfo
	Link      netlink.Link
	Addrs     []netlink.Addr
	Routes    []netlink.Route
	Neighbors []netlink.Neigh
}

// NetInterworkingModel defines the network model connecting
// the network interface to the virtual machine.
type NetInterworkingModel int

const (
	// NetXConnectDefaultModel Ask to use DefaultNetInterworkingModel
	NetXConnectDefaultModel NetInterworkingModel = iota

	// NetXConnectMacVtapModel can be used when the Container network
	// interface can be bridged using macvtap
	NetXConnectMacVtapModel

	// NetXConnectTCFilterModel redirects traffic from the network interface
	// provided by the network plugin to a tap interface.
	// This works for ipvlan and macvlan as well.
	NetXConnectTCFilterModel

	// NetXConnectNoneModel can be used when the VM is in the host network namespace
	NetXConnectNoneModel

	// NetXConnectInvalidModel is the last item to Check valid values by IsValid()
	NetXConnectInvalidModel
)

// IsValid checks if a model is valid
func (n NetInterworkingModel) IsValid() bool {
	return 0 <= int(n) && int(n) < int(NetXConnectInvalidModel)
}

const (
	defaultNetModelStr = "default"

	macvtapNetModelStr = "macvtap"

	tcFilterNetModelStr = "tcfilter"

	noneNetModelStr = "none"
)

// GetModel returns the string value of a NetInterworkingModel
func (n *NetInterworkingModel) GetModel() string {
	switch *n {
	case DefaultNetInterworkingModel:
		return defaultNetModelStr
	case NetXConnectMacVtapModel:
		return macvtapNetModelStr
	case NetXConnectTCFilterModel:
		return tcFilterNetModelStr
	case NetXConnectNoneModel:
		return noneNetModelStr
	}
	return "unknown"
}

// SetModel change the model string value
func (n *NetInterworkingModel) SetModel(modelName string) error {
	switch modelName {
	case defaultNetModelStr:
		*n = DefaultNetInterworkingModel
		return nil
	case macvtapNetModelStr:
		*n = NetXConnectMacVtapModel
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
var DefaultNetInterworkingModel = NetXConnectTCFilterModel

// DNSInfo describes the DNS setup related to a network interface.
type DNSInfo struct {
	Servers  []string
	Domain   string
	Searches []string
	Options  []string
}

// NetworkConfig is the network configuration related to a network.
type NetworkConfig struct {
	NetworkID         string
	InterworkingModel NetInterworkingModel
	NetworkCreated    bool
	DisableNewNetwork bool
	// if DAN config exists, use it to config network
	DanConfigPath string
}

type Network interface {
	// AddEndpoint adds endpoints to a sandbox's network.
	// If the NetworkInfo slice is empty, implementations are expected to scan
	// the sandbox's network for all existing endpoints.
	AddEndpoints(context.Context, *Sandbox, []NetworkInfo, bool) ([]Endpoint, error)

	// RemoveEndpoints removes endpoints from the sandbox's network.
	// If the the endpoint slice is empty, all endpoints will be removed.
	// If the network has been created by virtcontainers, Remove also deletes
	// the network.
	RemoveEndpoints(context.Context, *Sandbox, []Endpoint, bool) error

	// Run runs a callback in a sandbox's network.
	Run(context.Context, func() error) error

	// NetworkID returns a network unique identifier,
	// like e,g. a networking namespace on Linux hosts.
	NetworkID() string

	// NetworkCreated returns true if the network has been created
	// by virtcontainers.
	NetworkCreated() bool

	// Endpoints returns the list of networking endpoints attached to
	// the host network.
	Endpoints() []Endpoint

	// SetEndpoints sets a sandbox's network endpoints.
	SetEndpoints([]Endpoint)

	// GetEndpoints number of sandbox's network endpoints.
	GetEndpointsNum() (int, error)
}

func generateVCNetworkStructures(ctx context.Context, endpoints []Endpoint) ([]*pbTypes.Interface, []*pbTypes.Route, []*pbTypes.ARPNeighbor, error) {

	span, _ := networkTrace(ctx, "generateVCNetworkStructures", nil)
	defer span.End()

	var routes []*pbTypes.Route
	var ifaces []*pbTypes.Interface
	var neighs []*pbTypes.ARPNeighbor

	for _, endpoint := range endpoints {
		var ipAddresses []*pbTypes.IPAddress
		for _, addr := range endpoint.Properties().Addrs {
			// Skip localhost interface
			if addr.IP.IsLoopback() {
				continue
			}

			netMask, _ := addr.Mask.Size()
			ipAddress := pbTypes.IPAddress{
				Family:  pbTypes.IPFamily_v4,
				Address: addr.IP.String(),
				Mask:    fmt.Sprintf("%d", netMask),
			}

			if addr.IP.To4() == nil {
				ipAddress.Family = pbTypes.IPFamily_v6
			}
			ipAddresses = append(ipAddresses, &ipAddress)
		}
		noarp := endpoint.Properties().Iface.RawFlags & unix.IFF_NOARP
		ifc := pbTypes.Interface{
			IPAddresses: ipAddresses,
			Device:      endpoint.Name(),
			Name:        endpoint.Name(),
			Mtu:         uint64(endpoint.Properties().Iface.MTU),
			Type:        string(endpoint.Type()),
			RawFlags:    noarp,
			HwAddr:      endpoint.HardwareAddr(),
			PciPath:     endpoint.PciPath().String(),
		}

		ifaces = append(ifaces, &ifc)

		for _, route := range endpoint.Properties().Routes {
			var r pbTypes.Route

			if !validGuestRoute(route) {
				continue
			}

			if route.Dst != nil {
				r.Dest = route.Dst.String()
			}

			if route.Gw != nil {
				gateway := route.Gw.String()
				r.Gateway = gateway
			}

			if route.Src != nil {
				r.Source = route.Src.String()
			}

			r.Device = endpoint.Name()
			r.Scope = uint32(route.Scope)
			r.Family = utils.ConvertAddressFamily((int32)(route.Family))
			routes = append(routes, &r)
		}

		for _, neigh := range endpoint.Properties().Neighbors {
			var n pbTypes.ARPNeighbor

			if !validGuestNeighbor(neigh) {
				continue
			}

			n.Device = endpoint.Name()
			n.State = int32(neigh.State)
			n.Flags = int32(neigh.Flags)

			if neigh.HardwareAddr != nil {
				n.Lladdr = neigh.HardwareAddr.String()
			}

			n.ToIPAddress = &pbTypes.IPAddress{
				Family:  pbTypes.IPFamily_v4,
				Address: neigh.IP.String(),
			}
			if neigh.IP.To4() == nil {
				n.ToIPAddress.Family = pbTypes.IPFamily_v6
			}

			neighs = append(neighs, &n)
		}
	}

	return ifaces, routes, neighs, nil
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

	if ifName != "" {
		netPair.VirtIface.Name = ifName
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
