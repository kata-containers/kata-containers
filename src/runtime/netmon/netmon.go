// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io/ioutil"
	"log/syslog"
	"net"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/signals"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"

	"github.com/sirupsen/logrus"
	lSyslog "github.com/sirupsen/logrus/hooks/syslog"
	"github.com/vishvananda/netlink"
	"golang.org/x/sys/unix"
)

const (
	netmonName = "kata-netmon"

	kataCmd              = "kata-network"
	kataCLIAddIfaceCmd   = "add-iface"
	kataCLIDelIfaceCmd   = "del-iface"
	kataCLIUpdtRoutesCmd = "update-routes"

	kataSuffix = "kata"

	// sharedFile is the name of the file that will be used to share
	// the data between this process and the kata-runtime process
	// responsible for updating the network.
	sharedFile      = "shared.json"
	storageFilePerm = os.FileMode(0640)
	storageDirPerm  = os.FileMode(0750)
)

var (
	// version is the netmon version. This variable is populated at build time.
	version = "unknown"

	// For simplicity the code will only focus on IPv4 addresses for now.
	netlinkFamily = netlink.FAMILY_ALL

	storageParentPath = "/var/run/kata-containers/netmon/sbs"
)

type netmonParams struct {
	sandboxID   string
	runtimePath string
	debug       bool
	logLevel    string
}

type netmon struct {
	netmonParams

	storagePath string
	sharedFile  string

	netIfaces map[int]pbTypes.Interface

	linkUpdateCh chan netlink.LinkUpdate
	linkDoneCh   chan struct{}

	rtUpdateCh chan netlink.RouteUpdate
	rtDoneCh   chan struct{}

	netHandler *netlink.Handle
}

var netmonLog = logrus.New()

func printVersion() {
	fmt.Printf("%s version %s\n", netmonName, version)
}

const componentDescription = `is a network monitoring process that is intended to be started in the
appropriate network namespace so that it can listen to any event related to
link and routes. Whenever a new interface or route is created/updated, it is
responsible for calling into the kata-runtime CLI to ask for the actual
creation/update of the given interface or route.
`

func printComponentDescription() {
	fmt.Printf("\n%s %s\n", netmonName, componentDescription)
}

func parseOptions() netmonParams {
	var version, help bool

	params := netmonParams{}

	flag.BoolVar(&help, "h", false, "describe component usage")
	flag.BoolVar(&help, "help", false, "")
	flag.BoolVar(&params.debug, "d", false, "enable debug mode")
	flag.BoolVar(&version, "v", false, "display program version and exit")
	flag.BoolVar(&version, "version", false, "")
	flag.StringVar(&params.sandboxID, "s", "", "sandbox id (required)")
	flag.StringVar(&params.runtimePath, "r", "", "runtime path (required)")
	flag.StringVar(&params.logLevel, "log", "warn",
		"log messages above specified level: debug, warn, error, fatal or panic")

	flag.Parse()

	if help {
		printComponentDescription()
		flag.PrintDefaults()
		os.Exit(0)
	}

	if version {
		printVersion()
		os.Exit(0)
	}

	if params.sandboxID == "" {
		fmt.Fprintf(os.Stderr, "Error: sandbox id is empty, one must be provided\n")
		flag.PrintDefaults()
		os.Exit(1)
	}

	if params.runtimePath == "" {
		fmt.Fprintf(os.Stderr, "Error: runtime path is empty, one must be provided\n")
		flag.PrintDefaults()
		os.Exit(1)
	}

	return params
}

func newNetmon(params netmonParams) (*netmon, error) {
	handler, err := netlink.NewHandle(netlinkFamily)
	if err != nil {
		return nil, err
	}

	n := &netmon{
		netmonParams: params,
		storagePath:  filepath.Join(storageParentPath, params.sandboxID),
		sharedFile:   filepath.Join(storageParentPath, params.sandboxID, sharedFile),
		netIfaces:    make(map[int]pbTypes.Interface),
		linkUpdateCh: make(chan netlink.LinkUpdate),
		linkDoneCh:   make(chan struct{}),
		rtUpdateCh:   make(chan netlink.RouteUpdate),
		rtDoneCh:     make(chan struct{}),
		netHandler:   handler,
	}

	if err := os.MkdirAll(n.storagePath, storageDirPerm); err != nil {
		return nil, err
	}

	return n, nil
}

func (n *netmon) cleanup() {
	os.RemoveAll(n.storagePath)
	n.netHandler.Delete()
	close(n.linkDoneCh)
	close(n.rtDoneCh)
}

// setupSignalHandler sets up signal handling, starting a go routine to deal
// with signals as they arrive.
func (n *netmon) setupSignalHandler() {
	signals.SetLogger(n.logger())

	sigCh := make(chan os.Signal, 8)

	for _, sig := range signals.HandledSignals() {
		signal.Notify(sigCh, sig)
	}

	go func() {
		for {
			sig := <-sigCh

			nativeSignal, ok := sig.(syscall.Signal)
			if !ok {
				err := errors.New("unknown signal")
				netmonLog.WithError(err).WithField("signal", sig.String()).Error()
				continue
			}

			if signals.FatalSignal(nativeSignal) {
				netmonLog.WithField("signal", sig).Error("received fatal signal")
				signals.Die(nil)
			} else if n.debug && signals.NonFatalSignal(nativeSignal) {
				netmonLog.WithField("signal", sig).Debug("handling signal")
				signals.Backtrace()
			}
		}
	}()
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
	level, err := logrus.ParseLevel(n.logLevel)
	if err != nil {
		return err
	}

	netmonLog.SetLevel(level)

	netmonLog.Formatter = &logrus.TextFormatter{TimestampFormat: time.RFC3339Nano}

	hook, err := lSyslog.NewSyslogHook("", "", syslog.LOG_INFO|syslog.LOG_USER, netmonName)
	if err != nil {
		return err
	}

	netmonLog.AddHook(hook)

	announceFields := logrus.Fields{
		"runtime-path": n.runtimePath,
		"debug":        n.debug,
		"log-level":    n.logLevel,
	}

	n.logger().WithFields(announceFields).Info("announce")

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
			Address: addr.IP.String(),
			Mask:    fmt.Sprintf("%d", netMask),
		}

		if addr.IP.To4() != nil {
			ipAddr.Family = utils.ConvertNetlinkFamily(netlink.FAMILY_V4)
		} else {
			ipAddr.Family = utils.ConvertNetlinkFamily(netlink.FAMILY_V6)
		}

		ipAddrs = append(ipAddrs, ipAddr)
	}

	iface := pbTypes.Interface{
		Device:      linkAttrs.Name,
		Name:        linkAttrs.Name,
		IPAddresses: ipAddrs,
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
func convertRoutes(netRoutes []netlink.Route) []pbTypes.Route {
	var routes []pbTypes.Route

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
		}

		routes = append(routes, route)
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

func (n *netmon) storeDataToSend(data interface{}) error {
	// Marshal the data structure into a JSON bytes array.
	jsonArray, err := json.Marshal(data)
	if err != nil {
		return err
	}

	// Store the JSON bytes array at the specified path.
	return ioutil.WriteFile(n.sharedFile, jsonArray, storageFilePerm)
}

func (n *netmon) execKataCmd(subCmd string) error {
	execCmd := exec.Command(n.runtimePath, kataCmd, subCmd, n.sandboxID, n.sharedFile)

	n.logger().WithField("command", execCmd).Debug("Running runtime command")

	// Make use of Run() to ensure the kata-runtime process has correctly
	// terminated before to go further.
	if err := execCmd.Run(); err != nil {
		return err
	}

	// Remove the shared file after the command returned. At this point
	// we know the content of the file is not going to be used anymore,
	// and the file path can be reused for further commands.
	return os.Remove(n.sharedFile)
}

func (n *netmon) addInterfaceCLI(iface pbTypes.Interface) error {
	if err := n.storeDataToSend(iface); err != nil {
		return err
	}

	return n.execKataCmd(kataCLIAddIfaceCmd)
}

func (n *netmon) delInterfaceCLI(iface pbTypes.Interface) error {
	if err := n.storeDataToSend(iface); err != nil {
		return err
	}

	return n.execKataCmd(kataCLIDelIfaceCmd)
}

func (n *netmon) updateRoutesCLI(routes []pbTypes.Route) error {
	if err := n.storeDataToSend(routes); err != nil {
		return err
	}

	return n.execKataCmd(kataCLIUpdtRoutesCmd)
}

func (n *netmon) updateRoutes() error {
	// Get all the routes.
	netlinkRoutes, err := n.netHandler.RouteList(nil, netlinkFamily)
	if err != nil {
		return err
	}

	// Translate them into Route structures.
	routes := convertRoutes(netlinkRoutes)

	// Update the routes through the Kata CLI.
	return n.updateRoutesCLI(routes)
}

func (n *netmon) handleRTMNewAddr(ev netlink.LinkUpdate) error {
	n.logger().Debug("Interface update not supported")
	return nil
}

func (n *netmon) handleRTMDelAddr(ev netlink.LinkUpdate) error {
	n.logger().Debug("Interface update not supported")
	return nil
}

func (n *netmon) handleRTMNewLink(ev netlink.LinkUpdate) error {
	// NEWLINK might be a lot of different things. We're interested in
	// adding the interface (both to our list and by calling into the
	// Kata CLI API) only if this has the flags UP and RUNNING, meaning
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

	// Add the interface through the Kata CLI.
	if err := n.addInterfaceCLI(iface); err != nil {
		return err
	}

	// Add the interface to the internal list.
	n.netIfaces[linkAttrs.Index] = iface

	// Complete by updating the routes.
	return n.updateRoutes()
}

func (n *netmon) handleRTMDelLink(ev netlink.LinkUpdate) error {
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

	if err := n.delInterfaceCLI(iface); err != nil {
		return err
	}

	// Delete the interface from the internal list.
	delete(n.netIfaces, linkAttrs.Index)

	// Complete by updating the routes.
	return n.updateRoutes()
}

func (n *netmon) handleRTMNewRoute(ev netlink.RouteUpdate) error {
	// Add the route through updateRoutes(), only if the route refer to an
	// interface that already exists in the internal list of interfaces.
	if _, exist := n.netIfaces[ev.Route.LinkIndex]; !exist {
		n.logger().Debugf("Ignoring route %+v since interface %d not found",
			ev.Route, ev.Route.LinkIndex)
		return nil
	}

	return n.updateRoutes()
}

func (n *netmon) handleRTMDelRoute(ev netlink.RouteUpdate) error {
	// Remove the route through updateRoutes(), only if the route refer to
	// an interface that already exists in the internal list of interfaces.
	return n.updateRoutes()
}

func (n *netmon) handleLinkEvent(ev netlink.LinkUpdate) error {
	n.logger().Debug("handleLinkEvent: netlink event received")

	switch ev.Header.Type {
	case unix.NLMSG_DONE:
		n.logger().Debug("NLMSG_DONE")
		return nil
	case unix.NLMSG_ERROR:
		n.logger().Error("NLMSG_ERROR")
		return fmt.Errorf("Error while listening on netlink socket")
	case unix.RTM_NEWADDR:
		n.logger().Debug("RTM_NEWADDR")
		return n.handleRTMNewAddr(ev)
	case unix.RTM_DELADDR:
		n.logger().Debug("RTM_DELADDR")
		return n.handleRTMDelAddr(ev)
	case unix.RTM_NEWLINK:
		n.logger().Debug("RTM_NEWLINK")
		return n.handleRTMNewLink(ev)
	case unix.RTM_DELLINK:
		n.logger().Debug("RTM_DELLINK")
		return n.handleRTMDelLink(ev)
	default:
		n.logger().Warnf("Unknown msg type %v", ev.Header.Type)
	}

	return nil
}

func (n *netmon) handleRouteEvent(ev netlink.RouteUpdate) error {
	n.logger().Debug("handleRouteEvent: netlink event received")

	switch ev.Type {
	case unix.RTM_NEWROUTE:
		n.logger().Debug("RTM_NEWROUTE")
		return n.handleRTMNewRoute(ev)
	case unix.RTM_DELROUTE:
		n.logger().Debug("RTM_DELROUTE")
		return n.handleRTMDelRoute(ev)
	default:
		n.logger().Warnf("Unknown msg type %v", ev.Type)
	}

	return nil
}

func (n *netmon) handleEvents() (err error) {
	for {
		select {
		case ev := <-n.linkUpdateCh:
			if err = n.handleLinkEvent(ev); err != nil {
				return err
			}
		case ev := <-n.rtUpdateCh:
			if err = n.handleRouteEvent(ev); err != nil {
				return err
			}
		}
	}
}

func main() {
	// Parse parameters.
	params := parseOptions()

	// Create netmon handler.
	n, err := newNetmon(params)
	if err != nil {
		netmonLog.WithError(err).Fatal("newNetmon()")
		os.Exit(1)
	}
	defer n.cleanup()

	// Init logger.
	if err := n.setupLogger(); err != nil {
		netmonLog.WithError(err).Fatal("setupLogger()")
		os.Exit(1)
	}

	// Setup signal handlers
	n.setupSignalHandler()

	// Scan the current interfaces.
	if err := n.scanNetwork(); err != nil {
		n.logger().WithError(err).Fatal("scanNetwork()")
		os.Exit(1)
	}

	// Subscribe to the link listener.
	if err := n.listenNetlinkEvents(); err != nil {
		n.logger().WithError(err).Fatal("listenNetlinkEvents()")
		os.Exit(1)
	}

	// Go into the main loop.
	if err := n.handleEvents(); err != nil {
		n.logger().WithError(err).Fatal("handleEvents()")
		os.Exit(1)
	}
}
