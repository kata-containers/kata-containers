//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"net"
	"reflect"
	"sync"

	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"
)

// Network fully describes a sandbox network with its interfaces, routes and dns
// related information.
type network struct {
	ifacesLock sync.Mutex
	ifaces     []*pb.Interface

	routesLock sync.Mutex
	routes     []*pb.Route

	dnsLock sync.Mutex
	dns     []string
}

////////////////
// Interfaces //
////////////////

func linkByHwAddr(netHandle *netlink.Handle, hwAddr string) (netlink.Link, error) {
	links, err := netHandle.LinkList()
	if err != nil {
		return nil, err
	}

	for _, link := range links {
		lAttrs := link.Attrs()
		if lAttrs == nil {
			continue
		}

		if lAttrs.HardwareAddr.String() == hwAddr {
			return link, nil
		}
	}

	return nil, fmt.Errorf("Could not find the link corresponding to HwAddr %q", hwAddr)
}

func updateInterfaceAddrs(netHandle *netlink.Handle, link netlink.Link, addrs []*pb.IPAddress, add bool) error {
	for _, addr := range addrs {
		netlinkAddrStr := fmt.Sprintf("%s/%s", addr.Address, addr.Mask)

		netlinkAddr, err := netlink.ParseAddr(netlinkAddrStr)
		if err != nil {
			return fmt.Errorf("Could not parse %q: %v", netlinkAddrStr, err)
		}

		if add {
			if err := netHandle.AddrAdd(link, netlinkAddr); err != nil {
				return fmt.Errorf("Could not add %s to interface %v: %v",
					netlinkAddrStr, link, err)
			}
		} else {
			if err := netHandle.AddrDel(link, netlinkAddr); err != nil {
				return fmt.Errorf("Could not remove %s from interface %v: %v",
					netlinkAddrStr, link, err)
			}
		}
	}

	return nil
}

func updateInterfaceName(netHandle *netlink.Handle, link netlink.Link, name string) error {
	return netHandle.LinkSetName(link, name)
}

func updateInterfaceMTU(netHandle *netlink.Handle, link netlink.Link, mtu int) error {
	return netHandle.LinkSetMTU(link, mtu)
}

func updateLink(netHandle *netlink.Handle, link netlink.Link, iface *pb.Interface, actionType pb.UpdateType) error {
	switch actionType {
	case pb.UpdateType_AddIP:
		return updateInterfaceAddrs(netHandle, link, iface.IpAddresses, true)
	case pb.UpdateType_RemoveIP:
		return updateInterfaceAddrs(netHandle, link, iface.IpAddresses, false)
	case pb.UpdateType_Name:
		return updateInterfaceName(netHandle, link, iface.Name)
	case pb.UpdateType_MTU:
		return updateInterfaceMTU(netHandle, link, int(iface.Mtu))
	default:
		return fmt.Errorf("Unknown UpdateType %v", actionType)
	}
}

func (s *sandbox) addInterface(netHandle *netlink.Handle, iface *pb.Interface) (err error) {
	s.network.ifacesLock.Lock()
	defer s.network.ifacesLock.Unlock()

	if netHandle == nil {
		netHandle, err = netlink.NewHandle()
		if err != nil {
			return err
		}
		defer netHandle.Delete()
	}

	if iface == nil {
		return fmt.Errorf("Provided interface is nil")
	}

	hwAddr, err := net.ParseMAC(iface.HwAddr)
	if err != nil {
		return err
	}

	link := &netlink.Device{
		LinkAttrs: netlink.LinkAttrs{
			MTU:          int(iface.Mtu),
			TxQLen:       -1,
			Name:         iface.Name,
			HardwareAddr: hwAddr,
		},
	}

	// Create the link.
	if err := netHandle.LinkAdd(link); err != nil {
		return err
	}

	// Set the link up.
	if err := netHandle.LinkSetUp(link); err != nil {
		return err
	}

	// Update sandbox interface list.
	s.network.ifaces = append(s.network.ifaces, iface)

	return nil
}

func (s *sandbox) removeInterface(netHandle *netlink.Handle, ifaceName string) (err error) {
	s.network.ifacesLock.Lock()
	defer s.network.ifacesLock.Unlock()

	if netHandle == nil {
		netHandle, err = netlink.NewHandle()
		if err != nil {
			return err
		}
		defer netHandle.Delete()
	}

	// Find the interface by name.
	link, err := netHandle.LinkByName(ifaceName)
	if err != nil {
		return err
	}

	// Set the link down.
	if err := netHandle.LinkSetDown(link); err != nil {
		return err
	}

	// Delete the link.
	if err := netHandle.LinkDel(link); err != nil {
		return err
	}

	// Update sandbox interface list.
	for idx, iface := range s.network.ifaces {
		if iface.Name == ifaceName {
			s.network.ifaces = append(s.network.ifaces[:idx], s.network.ifaces[idx+1:]...)
			break
		}
	}

	return nil
}

func (s *sandbox) updateInterface(netHandle *netlink.Handle, iface *pb.Interface, actionType pb.UpdateType) (err error) {
	s.network.ifacesLock.Lock()
	defer s.network.ifacesLock.Unlock()

	if netHandle == nil {
		netHandle, err = netlink.NewHandle()
		if err != nil {
			return err
		}
		defer netHandle.Delete()
	}

	if iface == nil {
		return fmt.Errorf("Provided interface is nil")
	}

	fieldLogger := agentLog.WithFields(logrus.Fields{
		"mac-address":    iface.HwAddr,
		"interface-name": iface.Device,
	})

	var link netlink.Link
	if iface.HwAddr != "" {
		fieldLogger.Info("Getting interface from MAC address")

		// Find the interface link from its hardware address.
		link, err = linkByHwAddr(netHandle, iface.HwAddr)
		if err != nil {
			return err
		}
	} else if iface.Device != "" {
		fieldLogger.Info("Getting interface from name")

		// Find the interface link from its name.
		link, err = netHandle.LinkByName(iface.Device)
		if err != nil {
			return err
		}
	} else {
		return fmt.Errorf("Interface HwAddr and Name are both empty")
	}

	fieldLogger.WithField("link", fmt.Sprintf("%+v", link)).Infof("Link found")

	lAttrs := link.Attrs()
	if lAttrs != nil && (lAttrs.Flags&net.FlagUp) == net.FlagUp {
		// The link is up, makes sure we get it down before
		// doing any modification.
		if err := netHandle.LinkSetDown(link); err != nil {
			return err
		}
	}

	if err := updateLink(netHandle, link, iface, actionType); err != nil {
		return err
	}

	return netHandle.LinkSetUp(link)
}

////////////
// Routes //
////////////

func (s *sandbox) addRoute(netHandle *netlink.Handle, route *pb.Route) error {
	return s.updateRoute(netHandle, route, true)
}

func (s *sandbox) removeRoute(netHandle *netlink.Handle, route *pb.Route) error {
	return s.updateRoute(netHandle, route, false)
}

func (s *sandbox) updateRoute(netHandle *netlink.Handle, route *pb.Route, add bool) (err error) {
	s.network.routesLock.Lock()
	defer s.network.routesLock.Unlock()

	if netHandle == nil {
		netHandle, err = netlink.NewHandle()
		if err != nil {
			return err
		}
		defer netHandle.Delete()
	}

	if route == nil {
		return fmt.Errorf("Provided route is nil")
	}

	// Find link index from route's device name.
	link, err := netHandle.LinkByName(route.Device)
	if err != nil {
		return fmt.Errorf("Could not find link from device %s: %v", route.Device, err)
	}

	linkAttrs := link.Attrs()
	if linkAttrs == nil {
		return fmt.Errorf("Could not get link's attributes for device %s", route.Device)
	}

	var dst *net.IPNet
	if route.Dest != "default" {
		_, dst, err = net.ParseCIDR(route.Dest)
		if err != nil {
			return fmt.Errorf("Could not parse route destination %s: %v", route.Dest, err)
		}
	}

	netRoute := &netlink.Route{
		LinkIndex: linkAttrs.Index,
		Dst:       dst,
		Gw:        net.ParseIP(route.Gateway),
	}

	if add {
		if err := netHandle.RouteAdd(netRoute); err != nil {
			return fmt.Errorf("Could not add route dest(%s)/gw(%s)/dev(%s): %v",
				route.Dest, route.Gateway, route.Device, err)
		}

		// Add route to sandbox route list.
		s.network.routes = append(s.network.routes, route)
	} else {
		if err := netHandle.RouteDel(netRoute); err != nil {
			return fmt.Errorf("Could not remove route dest(%s)/gw(%s)/dev(%s): %v",
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

func setupDNS(dns []string) error {
	return nil
}

func removeDNS(dns []string) error {
	return nil
}

////////////
// Global //
////////////

// Remove everything related to network.
func (s *sandbox) removeNetwork() error {
	netHandle, err := netlink.NewHandle()
	if err != nil {
		return err
	}
	defer netHandle.Delete()

	for _, route := range s.network.routes {
		if err := s.removeRoute(netHandle, route); err != nil {
			return fmt.Errorf("Could not remove network route %v: %v",
				route, err)
		}
	}

	for _, iface := range s.network.ifaces {
		if err := s.removeInterface(netHandle, iface.Name); err != nil {
			return fmt.Errorf("Could not remove network interface %v: %v",
				iface, err)
		}
	}

	if err := removeDNS(s.network.dns); err != nil {
		return fmt.Errorf("Could not remove network DNS: %v", err)
	}

	return nil
}
