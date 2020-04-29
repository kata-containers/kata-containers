//
// Copyright (c) 2018-2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"net"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync"
	"syscall"

	"golang.org/x/sys/unix"

	"github.com/kata-containers/agent/pkg/types"
	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

var (
	errNoHandle             = grpcStatus.Errorf(codes.InvalidArgument, "Need network handle")
	errNoIF                 = grpcStatus.Errorf(codes.InvalidArgument, "Need network interface")
	errNoLink               = grpcStatus.Errorf(codes.InvalidArgument, "Need network link")
	errNoMAC                = grpcStatus.Errorf(codes.InvalidArgument, "Need hardware address")
	errNoRoutes             = grpcStatus.Errorf(codes.InvalidArgument, "Need network routes")
	guestDNSFile            = "/etc/resolv.conf"
	kataGuestSandboxDNSFile = "/run/kata-containers/sandbox/resolv.conf"
)

const (
	// ipvlan plugin adds a route of the format "default dev eth0 scope link"
	// Here since source, dest and gateway are empty, netlink will complain.
	// Set the gateway address explcitly to handle this, setting this address
	// is equivalent to the above route.

	defaultV4RouteIP = "0.0.0.0"

	// Use the below address for ipv6 gateway once ipv6 support is added
	// defaultV6RouteIP = "::"
)

// Network fully describes a sandbox network with its interfaces, routes and dns
// related information.
type network struct {
	ifacesLock sync.Mutex
	ifaces     map[string]*types.Interface

	routesLock sync.Mutex
	routes     []types.Route

	dns []string
}

////////////////
// Interfaces //
////////////////

func linkByHwAddr(netHandle *netlink.Handle, hwAddr string) (netlink.Link, error) {
	if netHandle == nil {
		return nil, errNoHandle
	}

	if hwAddr == "" {
		return nil, errNoMAC
	}

	links, err := netHandle.LinkList()
	if err != nil {
		return nil, err
	}

	for _, link := range links {
		if link == nil {
			continue
		}

		lAttrs := link.Attrs()
		if lAttrs == nil {
			continue
		}

		if lAttrs.HardwareAddr == nil {
			continue
		}

		if lAttrs.HardwareAddr.String() == hwAddr {
			return link, nil
		}
	}

	return nil, grpcStatus.Errorf(codes.NotFound, "Could not find the link corresponding to HwAddr %q", hwAddr)
}

func updateLink(netHandle *netlink.Handle, link netlink.Link, iface *types.Interface) error {
	if netHandle == nil {
		return errNoHandle
	}

	if link == nil {
		return errNoLink
	}

	if iface == nil {
		return errNoIF
	}

	// As a first step, clear out any existing addresses associated with the link:
	linkIPs, err := netlink.AddrList(link, netlink.FAMILY_ALL)
	if err != nil {
		return grpcStatus.Errorf(codes.Internal, "Could not check initial addresses for the link: %v", err)
	}
	for _, linkIP := range linkIPs {
		if err := netlink.AddrDel(link, &linkIP); err != nil {
			return grpcStatus.Errorf(codes.Internal, "Could not delete existing addresses: %v", err)
		}
	}

	// Set desired IP addresses:
	for _, addr := range iface.IPAddresses {
		netlinkAddrStr := fmt.Sprintf("%s/%s", addr.Address, addr.Mask)
		netlinkAddr, err := netlink.ParseAddr(netlinkAddrStr)

		// With ipv6 addresses, there is a brief period during which the address is marked as "tentative"
		// making it unavailable. A process called duplicate address detection(DAD) is performed during this period.
		// Disble DAD so that networking is available once the container is up. The assumption is
		// that it is the reponsibility of the upper stack to make sure the addresses assigned to containers
		// do not conflict. A similar operation is performed by libnetwork:
		// https://github.com/moby/moby/issues/18871

		if addr.GetFamily() == types.IPFamily_v6 {
			netlinkAddr.Flags = netlinkAddr.Flags | syscall.IFA_F_NODAD
		}

		if err != nil {
			return grpcStatus.Errorf(codes.Internal, "Could not parse %q: %v", netlinkAddrStr, err)
		}

		if err := netHandle.AddrAdd(link, netlinkAddr); err != nil {
			return grpcStatus.Errorf(codes.Internal, "Could not add %s to interface %v: %v",
				netlinkAddrStr, link, err)
		}
	}

	// set the interface name:
	if err := netHandle.LinkSetName(link, iface.Name); err != nil {
		return grpcStatus.Errorf(codes.Internal, "Could not set name %s for interface %v: %v", iface.Name, link, err)
	}

	// set the interface MTU:
	if err := netHandle.LinkSetMTU(link, int(iface.Mtu)); err != nil {
		return grpcStatus.Errorf(codes.Internal, "Could not set MTU %d for interface %v: %v", iface.Mtu, link, err)
	}

	if iface.RawFlags&unix.IFF_NOARP == uint32(unix.IFF_NOARP) {
		agentLog.WithField("link", link).Info("Set NOARP")
		if err := netHandle.LinkSetARPOff(link); err != nil {
			return grpcStatus.Errorf(codes.Internal, "Could not set NOARP %d for interface %v: %v",
				iface.RawFlags, link, err)
		}
	}

	return nil
}

func (s *sandbox) removeInterface(netHandle *netlink.Handle, iface *types.Interface) (resultingIfc *types.Interface, err error) {
	if iface == nil {
		return nil, errNoIF
	}

	s.network.ifacesLock.Lock()
	defer s.network.ifacesLock.Unlock()

	if netHandle == nil {
		netHandle, err = netlink.NewHandle(unix.NETLINK_ROUTE)
		if err != nil {
			return nil, err
		}
		defer netHandle.Delete()
	}

	// Find the interface by hardware address.
	link, err := linkByHwAddr(netHandle, iface.HwAddr)
	if err != nil {
		return nil, grpcStatus.Errorf(codes.Internal, "removeInterface: %v", err)
	}

	// Set the link down.
	if err := netHandle.LinkSetDown(link); err != nil {
		return iface, err
	}

	// Delete the link.
	if err := netHandle.LinkDel(link); err != nil {
		return iface, err
	}

	// Update sandbox interface list.
	delete(s.network.ifaces, iface.Name)

	return nil, nil
}

// updateInterface will update an existing interface with the values provided in the types.Interface.  It will identify the
// existing interface via MAC address and will return the state of the interface once the function completes as well an any
// errors observed.
func (s *sandbox) updateInterface(netHandle *netlink.Handle, iface *types.Interface) (resultingIfc *types.Interface, err error) {
	if iface == nil {
		return nil, errNoIF
	}

	s.network.ifacesLock.Lock()
	defer s.network.ifacesLock.Unlock()

	if netHandle == nil {
		netHandle, err = netlink.NewHandle(unix.NETLINK_ROUTE)
		if err != nil {
			return nil, err
		}
		defer netHandle.Delete()
	}

	fieldLogger := agentLog.WithFields(logrus.Fields{
		"mac-address":    iface.HwAddr,
		"interface-name": iface.Device,
		"pci-address":    iface.PciAddr,
	})

	// If the PCI address of the network device is provided, wait/check for the device
	// to be available first
	if iface.PciAddr != "" {
		// iface.PciAddr is in the format bridgeAddr/deviceAddr eg. 05/06
		_, err := getPCIDeviceName(s, iface.PciAddr)
		if err != nil {
			return nil, err
		}
	}

	var link netlink.Link
	if iface.HwAddr != "" {
		fieldLogger.Info("Getting interface from MAC address")

		// Find the interface link from its hardware address.
		link, err = linkByHwAddr(netHandle, iface.HwAddr)
		if err != nil {
			return nil, grpcStatus.Errorf(codes.Internal, "updateInterface: %v", err)
		}
	} else {
		return nil, grpcStatus.Errorf(codes.InvalidArgument, "Interface HwAddr empty")
	}

	// Use defer function to create and return the interface's state in
	// gRPC agent protocol format in the event that an error is observed
	defer func() {
		if err != nil {
			resultingIfc, _ = getInterface(netHandle, link)
		} else {
			resultingIfc = iface
		}
		//put the link back into the up state
		retErr := netHandle.LinkSetUp(link)

		//if we failed to setup the link but already are returning
		//with an error, return the original error
		if err == nil {
			err = retErr
		}
	}()

	fieldLogger.WithField("link", fmt.Sprintf("%+v", link)).Info("Link found")

	lAttrs := link.Attrs()
	if lAttrs != nil && (lAttrs.Flags&net.FlagUp) == net.FlagUp {
		// The link is up, makes sure we get it down before
		// doing any modification.
		if err = netHandle.LinkSetDown(link); err != nil {
			return
		}
	}

	err = updateLink(netHandle, link, iface)

	return

}

// getInterface will retrieve interface details from the provided link
func getInterface(netHandle *netlink.Handle, link netlink.Link) (*types.Interface, error) {
	if netHandle == nil {
		return nil, errNoHandle
	}

	if link == nil {
		return nil, errNoLink
	}

	var ifc types.Interface
	linkAttrs := link.Attrs()
	ifc.Name = linkAttrs.Name
	ifc.Mtu = uint64(linkAttrs.MTU)
	ifc.HwAddr = linkAttrs.HardwareAddr.String()

	addrs, err := netHandle.AddrList(link, netlink.FAMILY_ALL)
	if err != nil {
		agentLog.WithError(err).Error("getInterface() failed")
		return nil, err
	}
	for _, addr := range addrs {
		netMask, _ := addr.Mask.Size()
		m := types.IPAddress{
			Address: addr.IP.String(),
			Mask:    fmt.Sprintf("%d", netMask),
		}
		if addr.IP.To4() != nil {
			m.Family = types.IPFamily_v4
		} else {
			m.Family = types.IPFamily_v6
		}
		ifc.IPAddresses = append(ifc.IPAddresses, &m)
	}

	return &ifc, nil
}

func (s *sandbox) listInterfaces(netHandle *netlink.Handle) (*pb.Interfaces, error) {
	var err error
	if netHandle == nil {
		netHandle, err = netlink.NewHandle(unix.NETLINK_ROUTE)
		if err != nil {
			return nil, err
		}
		defer netHandle.Delete()
	}

	links, err := netHandle.LinkList()
	if err != nil {
		return nil, err
	}

	var interfaces pb.Interfaces
	for _, link := range links {
		ifc, err := getInterface(netHandle, link)
		if err != nil {
			agentLog.WithField("link", link).WithError(err).Error("listInterfaces() failed")
			return &interfaces, err
		}
		interfaces.Interfaces = append(interfaces.Interfaces, ifc)
	}
	return &interfaces, nil
}

////////////
// Routes //
////////////

func (s *sandbox) deleteRoutes(netHandle *netlink.Handle) error {
	if netHandle == nil {
		return errNoHandle
	}

	initRouteList, err := netHandle.RouteList(nil, netlink.FAMILY_ALL)
	if err != nil {
		return err
	}
	for _, initRoute := range initRouteList {
		// don't delete routes associated with lo:
		link, _ := netHandle.LinkByIndex(initRoute.LinkIndex)
		if link.Attrs().Name == "lo" || link.Attrs().Name == "::1" {
			continue
		}

		// If route is installed by kernel, do not delete
		if initRoute.Protocol == unix.RTPROT_KERNEL {
			continue
		}

		err = netHandle.RouteDel(&initRoute)
		if err != nil {
			//If there was an error deleting some of the initial routes,
			//return the error and the current routes on the system via
			//the defer function
			return err
		}
	}

	return nil
}

//updateRoutes will take requestedRoutes and create netlink routes, with a goal of creating a final
// state which matches the requested routes.  In doing this, preesxisting non-loopback routes will be
// removed from the network.  If an error occurs, this function returns the list of routes in
// gRPC-route format at the time of failure
func (s *sandbox) updateRoutes(netHandle *netlink.Handle, requestedRoutes *pb.Routes) (resultingRoutes *pb.Routes, err error) {
	if requestedRoutes == nil {
		return nil, errNoRoutes
	}

	if netHandle == nil {
		netHandle, err = netlink.NewHandle(unix.NETLINK_ROUTE)
		if err != nil {
			return nil, err
		}
		defer netHandle.Delete()
	}

	//If we are returning an error, return the current routes on the system
	defer func() {
		if err != nil {
			resultingRoutes, _ = getCurrentRoutes(netHandle)
		}
	}()

	//
	// First things first, let's blow away all the existing routes.  The updateRoutes function
	// is designed to be declarative, so we will attempt to create state matching what is
	// requested, and in the event that we fail to do so, will return the error and final state.
	//
	if err = s.deleteRoutes(netHandle); err != nil {
		return nil, err
	}

	//
	// Set each of the requested routes
	//
	// First make sure we set the interfaces initial routes, as otherwise we
	// won't be able to access the gateway
	for _, reqRoute := range requestedRoutes.Routes {
		if reqRoute.Gateway == "" {
			err = s.updateRoute(netHandle, reqRoute, true)
			if err != nil {
				agentLog.WithError(err).Error("update Route failed")
				//If there was an error setting the route, return the error
				//and the current routes on the system via the defer func
				return
			}

		}
	}
	// Take a second pass and apply the routes which include a gateway
	for _, reqRoute := range requestedRoutes.Routes {
		if reqRoute.Gateway != "" {
			err = s.updateRoute(netHandle, reqRoute, true)
			if err != nil {
				agentLog.WithError(err).Error("update Route failed")
				//If there was an error setting the route, return the
				//error and the current routes on the system via defer
				return
			}
		}
	}

	return requestedRoutes, err
}

func (s *sandbox) listRoutes(netHandle *netlink.Handle) (*pb.Routes, error) {
	return getCurrentRoutes(netHandle)
}

//getCurrentRoutes is a helper to gather existing routes in gRPC protocol format
func getCurrentRoutes(netHandle *netlink.Handle) (*pb.Routes, error) {
	var err error
	if netHandle == nil {
		netHandle, err = netlink.NewHandle(unix.NETLINK_ROUTE)
		if err != nil {
			return nil, err
		}
		defer netHandle.Delete()
	}

	var routes pb.Routes

	finalRouteList, err := netHandle.RouteList(nil, netlink.FAMILY_ALL)
	if err != nil {
		return &routes, err
	}

	for _, route := range finalRouteList {
		var r types.Route
		if route.Dst != nil {
			r.Dest = route.Dst.String()
		}

		if route.Gw != nil {
			r.Gateway = route.Gw.String()
		}

		if route.Src != nil {
			r.Source = route.Src.String()
		}

		r.Scope = uint32(route.Scope)

		link, err := netHandle.LinkByIndex(route.LinkIndex)
		if err != nil {
			return &routes, err
		}
		r.Device = link.Attrs().Name

		routes.Routes = append(routes.Routes, &r)
	}

	return &routes, nil
}

func (s *sandbox) processRoute(netHandle *netlink.Handle, route *types.Route) (*netlink.Route, error) {
	if route == nil {
		return nil, grpcStatus.Error(codes.InvalidArgument, "Provided route is nil")
	}

	// Find link index from route's device name.
	link, err := netHandle.LinkByName(route.Device)
	if err != nil {
		return nil, grpcStatus.Errorf(codes.Internal, "Could not find link from device %s: %v", route.Device, err)
	}

	linkAttrs := link.Attrs()
	if linkAttrs == nil {
		return nil, grpcStatus.Errorf(codes.Internal, "Could not get link's attributes for device %s", route.Device)
	}

	// We do not modify the gateway in the list of routes provided,
	// since we loop through the routes checking for the gateway again.
	gateway := route.Gateway
	if route.Dest == "" && route.Gateway == "" && route.Source == "" {
		gateway = defaultV4RouteIP
	}

	var dst *net.IPNet
	if route.Dest == "default" || route.Dest == "" {
		dst = nil
	} else {
		_, dst, err = net.ParseCIDR(route.Dest)
		if err != nil {
			return nil, grpcStatus.Errorf(codes.Internal, "Could not parse route destination %s: %v", route.Dest, err)
		}
	}

	netRoute := &netlink.Route{
		LinkIndex: linkAttrs.Index,
		Dst:       dst,
		Src:       net.ParseIP(route.Source),
		Gw:        net.ParseIP(gateway),
		Scope:     netlink.Scope(route.Scope),
	}

	return netRoute, nil
}

func checkDuplicateRoute(rt, netRoute *netlink.Route) bool {
	if rt.Dst == nil {
		return netRoute.Dst == nil && rt.Gw.Equal(netRoute.Gw)
	}

	return netRoute.Dst != nil && (rt.Dst.String() == netRoute.Dst.String()) && rt.Src.Equal(netRoute.Src)
}

func (s *sandbox) updateRoute(netHandle *netlink.Handle, route *types.Route, add bool) (err error) {
	s.network.routesLock.Lock()
	defer s.network.routesLock.Unlock()

	if netHandle == nil {
		netHandle, err = netlink.NewHandle(unix.NETLINK_ROUTE)
		if err != nil {
			return err
		}
		defer netHandle.Delete()
	}

	netRoute, err := s.processRoute(netHandle, route)
	if err != nil {
		return err
	}

	if add {
		if err := netHandle.RouteAdd(netRoute); err != nil {
			// Doing a string comparison here for lack of error codes from upstream
			if strings.Contains(err.Error(), "file exists") {
				agentLog.Infof("Route exists, will try to delete duplicate route first")

				rts, _ := netHandle.RouteList(nil, netlink.FAMILY_ALL)
				for _, rt := range rts {
					if checkDuplicateRoute(&rt, netRoute) {
						// Delete route first
						if err := netHandle.RouteDel(&rt); err != nil {
							return grpcStatus.Errorf(codes.Internal, "Could not add route dest(%s)/gw(%s)/dev(%s), duplicate route exists, error deleting existing route: %v", route.Dest, route.Gateway, route.Device, err)
						}

						if err := netHandle.RouteAdd(netRoute); err != nil {
							break
						}

						s.network.routes = append(s.network.routes, *route)
						return nil
					}
				}
			}
			return grpcStatus.Errorf(codes.Internal, "Could not add/replace route dest(%s)/gw(%s)/dev(%s): %v",
				route.Dest, route.Gateway, route.Device, err)
		}
		// Add route to sandbox route list.
		s.network.routes = append(s.network.routes, *route)
	} else {
		if err := netHandle.RouteDel(netRoute); err != nil {
			return grpcStatus.Errorf(codes.Internal, "Could not remove route dest(%s)/gw(%s)/dev(%s): %v",
				route.Dest, route.Gateway, route.Device, err)
		}

		// Remove route from sandbox route list.
		for idx, sandboxRoute := range s.network.routes {
			if reflect.DeepEqual(sandboxRoute, route) {
				s.network.routes = append(s.network.routes[:idx], s.network.routes[idx+1:]...)
				break
			}
		}
	}

	return nil
}

/////////
// DNS //
/////////

func setupDNS(dns []string) (err error) {
	var file *os.File

	if len(dns) == 0 {
		agentLog.Debug("Did not set sandbox DNS as DNS not received as part of grpc request.")
		return nil
	}

	if err := os.MkdirAll(filepath.Dir(kataGuestSandboxDNSFile), 0700); err != nil {
		return err
	}
	if file, err = os.Create(kataGuestSandboxDNSFile); err != nil {
		return err
	}
	defer file.Close()

	for i, line := range dns {
		if i == (len(dns) - 1) {
			_, err = file.WriteString(strings.TrimSpace(line))
		} else {
			_, err = file.WriteString(strings.TrimSpace(line) + "\n")
		}

		if err != nil {
			return err
		}
	}

	return mount(kataGuestSandboxDNSFile, guestDNSFile, "bind", syscall.MS_BIND, "")
}

////////////
// Global //
////////////

// Remove everything related to network.
func (s *sandbox) removeNetwork() error {
	netHandle, err := netlink.NewHandle(unix.NETLINK_ROUTE)
	if err != nil {
		return err
	}
	defer netHandle.Delete()

	for _, iface := range s.network.ifaces {
		if _, err := s.removeInterface(netHandle, iface); err != nil {
			return grpcStatus.Errorf(codes.Internal, "Could not remove network interface %v: %v",
				iface, err)
		}
	}

	return nil
}

// Bring up localhost network interface.
func (s *sandbox) handleLocalhost() error {
	span, _ := s.trace("handleLocalhost")
	defer span.finish()

	// If not running as the init daemon, there is nothing to do as the
	// localhost interface will already exist.
	if os.Getpid() != 1 {
		return nil
	}

	lo, err := netlink.LinkByName("lo")
	if err != nil {
		return err
	}

	return netlink.LinkSetUp(lo)
}
