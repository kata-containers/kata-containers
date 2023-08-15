// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package netmon

import (
	"context"
	"fmt"
	"runtime"
	"strconv"
	"strings"

	"log/syslog"
	"net"
	"os"

	"time"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	"github.com/sirupsen/logrus"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	lSyslog "github.com/sirupsen/logrus/hooks/syslog"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
	"golang.org/x/sys/unix"
)

const (
	netmonName = "kata-netmon"
	kataSuffix = "kata"
)

var (
	sandbox vc.VCSandbox

	// For simplicity the code will only focus on IPv4 addresses for now.
	netlinkFamily = netlink.FAMILY_ALL
)

type netmon struct {
	sandboxID string

	netIfaces map[int]pbTypes.Interface

	linkUpdateCh chan netlink.LinkUpdate
	linkDoneCh   chan struct{}

	rtUpdateCh chan netlink.RouteUpdate
	rtDoneCh   chan struct{}

	netHandler *netlink.Handle
}

var netmonLog = logrus.New()

// It is a network monitoring process that is intended to be started in the
// appropriate network namespace so that it can listen to any event related to
// link and routes. Whenever a new interface or route is created/updated, it is
// responsible for calling into the API to ask for the actual
// creation/update of the given interface or route.

func newNetmon(sandbox vc.VCSandbox) (*netmon, error) {
	handler, err := netlink.NewHandle(netlinkFamily)
	if err != nil {
		return nil, err
	}

	n := &netmon{
		sandboxID:    sandbox.ID(),
		netIfaces:    make(map[int]pbTypes.Interface),
		linkUpdateCh: make(chan netlink.LinkUpdate),
		linkDoneCh:   make(chan struct{}),
		rtUpdateCh:   make(chan netlink.RouteUpdate),
		rtDoneCh:     make(chan struct{}),
		netHandler:   handler,
	}
	return n, nil
}

func (n *netmon) cleanup() {
	n.netHandler.Close()
	close(n.linkDoneCh)
	close(n.rtDoneCh)
}

func (n *netmon) logger() *logrus.Entry {
	fields := logrus.Fields{
		"name":   netmonName,
		"pid":    os.Getpid(),
		"source": "netmon",
	}

	if n.sandboxID != "" {
		fields["sandbox"] = n.sandboxID
	}

	return netmonLog.WithFields(fields)
}

func (n *netmon) setupLogger() error {
	level := logrus.DebugLevel
	netmonLog.SetLevel(level)

	netmonLog.Formatter = &logrus.TextFormatter{TimestampFormat: time.RFC3339Nano}

	hook, err := lSyslog.NewSyslogHook("", "", syslog.LOG_INFO|syslog.LOG_USER, netmonName)
	if err != nil {
		return err
	}

	netmonLog.AddHook(hook)

	return nil
}

func (n *netmon) listenNetlinkEvents() error {
	if err := netlink.LinkSubscribe(n.linkUpdateCh, n.linkDoneCh); err != nil {
		return err
	}

	return netlink.RouteSubscribe(n.rtUpdateCh, n.rtDoneCh)
}

// convertInterface converts a link and its IP addresses as defined by netlink
// package, into the Interface structure format expected by kata-runtime to
// describe an interface and its associated IP addresses.
func convertInterface(linkAttrs *netlink.LinkAttrs, linkType string, addrs []netlink.Addr) pbTypes.Interface {
	if linkAttrs == nil {
		netmonLog.Warn("Link attributes are nil")
		return pbTypes.Interface{}
	}

	var ipAddrs []*pbTypes.IPAddress

	for _, addr := range addrs {
		if addr.IPNet == nil {
			continue
		}

		netMask, _ := addr.Mask.Size()

		ipAddr := &pbTypes.IPAddress{
			Family:  pbTypes.IPFamily_v4,
			Address: addr.IP.String(),
			Mask:    fmt.Sprintf("%d", netMask),
		}
		if addr.IP.To4() == nil {
			ipAddr.Family = pbTypes.IPFamily_v6
		}

		ipAddrs = append(ipAddrs, ipAddr)
	}

	iface := pbTypes.Interface{
		IPAddresses: ipAddrs,
		Device:      linkAttrs.Name,
		Name:        linkAttrs.Name,
		Mtu:         uint64(linkAttrs.MTU),
		HwAddr:      linkAttrs.HardwareAddr.String(),
		Type:        linkType,
	}

	netmonLog.WithField("interface", iface).Debug("Interface converted")

	return iface
}

// convertRoutes converts a list of routes as defined by netlink package,
// into a list of Route structure format expected by kata-runtime to
// describe a set of routes.
func convertRoutes(netRoutes []netlink.Route) []*pbTypes.Route {
	var routes []*pbTypes.Route

	for _, netRoute := range netRoutes {
		dst := ""

		if netRoute.Protocol == unix.RTPROT_KERNEL {
			continue
		}

		if netRoute.Dst != nil {
			dst = netRoute.Dst.String()
			if netRoute.Dst.IP.To4() != nil || netRoute.Dst.IP.To16() != nil {
				dst = netRoute.Dst.String()
			} else {
				netmonLog.WithField("destination", netRoute.Dst.IP.String()).Warn("Unexpected network address format")
			}
		}

		src := ""
		if netRoute.Src != nil {
			if netRoute.Src.To4() != nil || netRoute.Src.To16() != nil {
				src = netRoute.Src.String()
			} else {
				netmonLog.WithField("source", netRoute.Src.String()).Warn("Unexpected network address format")
			}
		}

		gw := ""
		if netRoute.Gw != nil {
			if netRoute.Gw.To4() != nil || netRoute.Gw.To16() != nil {
				gw = netRoute.Gw.String()
			} else {
				netmonLog.WithField("gateway", netRoute.Gw.String()).Warn("Unexpected network address format")
			}
		}

		dev := ""
		iface, err := net.InterfaceByIndex(netRoute.LinkIndex)
		if err == nil {
			dev = iface.Name
		}

		route := pbTypes.Route{
			Dest:    dst,
			Gateway: gw,
			Device:  dev,
			Source:  src,
			Scope:   uint32(netRoute.Scope),
			Family:  utils.ConvertAddressFamily((int32)(netRoute.Family)),
		}

		routes = append(routes, &route)
	}

	netmonLog.WithField("routes", routes).Debug("Routes converted")

	return routes
}

// scanNetwork lists all the interfaces it can find inside the current
// network namespace, and store them in-memory to keep track of them.
func (n *netmon) scanNetwork() error {
	links, err := n.netHandler.LinkList()
	if err != nil {
		return err
	}

	for _, link := range links {
		addrs, err := n.netHandler.AddrList(link, netlinkFamily)
		if err != nil {
			return err
		}

		linkAttrs := link.Attrs()
		if linkAttrs == nil {
			continue
		}

		iface := convertInterface(linkAttrs, link.Type(), addrs)
		n.netIfaces[linkAttrs.Index] = iface
	}

	n.logger().Debug("Network scanned")

	return nil
}

func (n *netmon) addInterface(ctx context.Context, iface pbTypes.Interface) error {
	_, err := sandbox.AddInterface(ctx, &iface)
	return err
}

func (n *netmon) removeInterface(ctx context.Context, iface pbTypes.Interface) error {
	_, err := sandbox.RemoveInterface(ctx, &iface)
	return err
}

func (n *netmon) updateRoutes(ctx context.Context) error {
	// Get all the routes.
	netlinkRoutes, err := n.netHandler.RouteList(nil, netlinkFamily)
	if err != nil {
		return err
	}

	// Translate them into Route structures.
	routes := convertRoutes(netlinkRoutes)

	// Update the routes through the Kata API
	_, err = sandbox.UpdateRoutes(ctx, routes)
	return err
}

func (n *netmon) handleRTMNewAddr(ctx context.Context, ev netlink.LinkUpdate) error {
	n.logger().Debug("Interface update not supported")
	return nil
}

func (n *netmon) handleRTMDelAddr(ctx context.Context, ev netlink.LinkUpdate) error {
	n.logger().Debug("Interface update not supported")
	return nil
}

func (n *netmon) handleRTMNewLink(ctx context.Context, ev netlink.LinkUpdate) error {
	// NEWLINK might be a lot of different things. We're interested in
	// adding the interface (both to our list and by calling into the
	// network API) only if this has the flags UP and RUNNING, meaning
	// we don't expect any further change on the interface, and that we
	// are ready to add it.

	linkAttrs := ev.Link.Attrs()
	if linkAttrs == nil {
		n.logger().Warn("The link attributes are nil")
		return nil
	}

	// First, ignore if the interface name contains "kata". This way we
	// are preventing from adding interfaces created by Kata Containers.
	if strings.HasSuffix(linkAttrs.Name, kataSuffix) {
		n.logger().Debugf("Ignore the interface %s because found %q",
			linkAttrs.Name, kataSuffix)
		return nil
	}

	// Check if the interface exist in the internal list.
	if _, exist := n.netIfaces[int(ev.Index)]; exist {
		n.logger().Debugf("Ignoring interface %s because already exist",
			linkAttrs.Name)
		return nil
	}

	// Now, check if the interface has been enabled to UP and RUNNING.
	if (ev.Flags&unix.IFF_UP) != unix.IFF_UP ||
		(ev.Flags&unix.IFF_RUNNING) != unix.IFF_RUNNING {
		n.logger().Debugf("Ignore the interface %s because not UP and RUNNING",
			linkAttrs.Name)
		return nil
	}

	// Get the list of IP addresses associated with this interface.
	addrs, err := n.netHandler.AddrList(ev.Link, netlinkFamily)
	if err != nil {
		return err
	}
	// Convert the interfaces in the appropriate structure format.
	iface := convertInterface(linkAttrs, ev.Link.Type(), addrs)

	//Add the interface through the Kata CLI.
	if err := n.addInterface(ctx, iface); err != nil {
		return err
	}

	// Add the interface to the internal list.
	n.netIfaces[linkAttrs.Index] = iface

	// Complete by updating the routes.
	return n.updateRoutes(ctx)
}

func (n *netmon) handleRTMDelLink(ctx context.Context, ev netlink.LinkUpdate) error {
	// It can only delete if identical interface is found in the internal
	// list of interfaces. Otherwise, the deletion will be ignored.
	linkAttrs := ev.Link.Attrs()
	if linkAttrs == nil {
		n.logger().Warn("Link attributes are nil")
		return nil
	}

	// First, ignore if the interface name contains "kata". This way we
	// are preventing from deleting interfaces created by Kata Containers.
	if strings.Contains(linkAttrs.Name, kataSuffix) {
		n.logger().Debugf("Ignore the interface %s because found %q",
			linkAttrs.Name, kataSuffix)
		return nil
	}

	// Check if the interface exist in the internal list.
	iface, exist := n.netIfaces[int(ev.Index)]
	if !exist {
		n.logger().Debugf("Ignoring interface %s because not found",
			linkAttrs.Name)
		return nil
	}

	if err := n.removeInterface(ctx, iface); err != nil {
		return err
	}

	// Delete the interface from the internal list.
	delete(n.netIfaces, linkAttrs.Index)

	// Complete by updating the routes.
	return n.updateRoutes(ctx)
}

func (n *netmon) handleRTMNewRoute(ctx context.Context, ev netlink.RouteUpdate) error {
	// Add the route through updateRoutes(), only if the route refer to an
	// interface that already exists in the internal list of interfaces.
	if _, exist := n.netIfaces[ev.Route.LinkIndex]; !exist {
		n.logger().Debugf("Ignoring route %+v since interface %d not found",
			ev.Route, ev.Route.LinkIndex)
		return nil
	}

	return n.updateRoutes(ctx)
}

func (n *netmon) handleRTMDelRoute(ctx context.Context, ev netlink.RouteUpdate) error {
	// Remove the route through updateRoutes(), only if the route refer to
	// an interface that already exists in the internal list of interfaces.
	return n.updateRoutes(ctx)
}

func (n *netmon) handleLinkEvent(ctx context.Context, ev netlink.LinkUpdate) error {
	n.logger().Debug("handleLinkEvent: netlink event received")

	switch ev.Header.Type {
	case unix.NLMSG_DONE:
		n.logger().Debug("NLMSG_DONE")
		return nil
	case unix.NLMSG_ERROR:
		n.logger().Error("NLMSG_ERROR")
		return fmt.Errorf("error while listening on netlink socket")
	case unix.RTM_NEWADDR:
		n.logger().Debug("RTM_NEWADDR")
		return n.handleRTMNewAddr(ctx, ev)
	case unix.RTM_DELADDR:
		n.logger().Debug("RTM_DELADDR")
		return n.handleRTMDelAddr(ctx, ev)
	case unix.RTM_NEWLINK:
		n.logger().Debug("RTM_NEWLINK")
		return n.handleRTMNewLink(ctx, ev)
	case unix.RTM_DELLINK:
		n.logger().Debug("RTM_DELLINK")
		return n.handleRTMDelLink(ctx, ev)
	default:
		n.logger().Warnf("Unknown msg type %v", ev.Header.Type)
	}

	return nil
}

func (n *netmon) handleRouteEvent(ctx context.Context, ev netlink.RouteUpdate) error {
	n.logger().Debug("handleRouteEvent: netlink event received")

	switch ev.Type {
	case unix.RTM_NEWROUTE:
		n.logger().Debug("RTM_NEWROUTE")
		return n.handleRTMNewRoute(ctx, ev)
	case unix.RTM_DELROUTE:
		n.logger().Debug("RTM_DELROUTE")
		return n.handleRTMDelRoute(ctx, ev)
	default:
		n.logger().Warnf("Unknown msg type %v", ev.Type)
	}

	return nil
}

func (n *netmon) handleEvents(ctx context.Context) (err error) {
	for {
		select {
		case ev := <-n.linkUpdateCh:
			if err = n.handleLinkEvent(ctx, ev); err != nil {
				return err
			}
		case ev := <-n.rtUpdateCh:
			if err = n.handleRouteEvent(ctx, ev); err != nil {
				return err
			}
		}
	}
}

func StartNetMon(ctx context.Context, Sandbox vc.VCSandbox) error {
	sandbox = Sandbox
	hid, err := sandbox.GetHypervisorPid()
	if err != nil {
		netmonLog.WithError(err).Error("GetHypervisorPid()")
		return err
	}
	f, err := os.OpenFile(fmt.Sprintf("/proc/%s/ns/net", strconv.Itoa(hid)), os.O_RDONLY, 0)
	if err != nil {
		netmonLog.WithError(err).Error("error get sandbox net namespacea")
		return err
	}
	nsFD := f.Fd()
	runtime.LockOSThread()
	origns, err := netns.Get()
	if err != nil {
		netmonLog.WithError(err).Error("error get current net namespace")
		return err
	}
	if err = netns.Set(netns.NsHandle(nsFD)); err != nil {
		netmonLog.WithError(err).Error("error set current namespace")
		return err
	}

	// Create netmon handler.
	n, err := newNetmon(sandbox)

	if err != nil {
		netmonLog.WithError(err).Error("newNetmon()")
		return err
	}
	defer func() {
		n.cleanup()
		netns.Set(origns)
		origns.Close()
		runtime.UnlockOSThread()
		f.Close()
	}()

	// Init logger.
	if err := n.setupLogger(); err != nil {
		netmonLog.WithError(err).Error("setupLogger()")
		return err
	}

	// Scan the current interfaces.
	if err := n.scanNetwork(); err != nil {
		n.logger().WithError(err).Error("scanNetwork()")
		return err
	}

	//Subscribe to the link listener.
	if err := n.listenNetlinkEvents(); err != nil {
		n.logger().WithError(err).Error("listenNetlinkEvents()")
		return err
	}
	//Go into the main loop.
	if err := n.handleEvents(ctx); err != nil {
		n.logger().WithError(err).Error("handleEvents()")
		return err
	}
	return nil
}
