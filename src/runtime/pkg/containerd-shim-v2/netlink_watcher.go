//go:build linux

// Copyright (c) 2026 Naval Group
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"net"
	"strings"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
	"golang.org/x/sys/unix"

	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
)

const (
	// netlinkWatcherDebounce is the delay after the last netlink event
	// before we scan the namespace for changes. This lets the CNI plugin
	// finish setting up the interface (link + addresses) before we act.
	netlinkWatcherDebounce = 500 * time.Millisecond
)

// watchNetworkInterfaces subscribes to netlink link events inside the pod
// network namespace and hot-plugs / hot-unplugs network interfaces into
// the kata VM as they appear or disappear.
func watchNetworkInterfaces(ctx context.Context, s *service) {
	netNsPath := s.sandbox.GetNetNs()
	if netNsPath == "" {
		shimLog.Warn("no network namespace path, skipping netlink watcher")
		return
	}

	nsHandle, err := netns.GetFromPath(netNsPath)
	if err != nil {
		shimLog.WithError(err).WithField("netns", netNsPath).Error("failed to open network namespace for netlink watcher")
		return
	}
	defer nsHandle.Close()

	updates := make(chan netlink.LinkUpdate, 64)
	done := make(chan struct{})

	go func() {
		<-ctx.Done()
		close(done)
	}()

	if err := netlink.LinkSubscribeAt(nsHandle, updates, done); err != nil {
		shimLog.WithError(err).Error("failed to subscribe to netlink events")
		return
	}

	shimLog.WithField("netns", netNsPath).Warn("netlink watcher started")

	// knownIfaces tracks interfaces already present in the pod netns.
	// We track both MAC and name because DELLINK events for destroyed
	// veth peers may arrive with an empty MAC address.
	knownIfaces := buildKnownIfaces(ctx, s)

	var debounceTimer *time.Timer
	var debounceCh <-chan time.Time

	for {
		select {
		case <-ctx.Done():
			shimLog.Info("netlink watcher stopping (context cancelled)")
			if debounceTimer != nil {
				debounceTimer.Stop()
			}
			return

		case update, ok := <-updates:
			if !ok {
				shimLog.Info("netlink watcher stopping (channel closed)")
				if debounceTimer != nil {
					debounceTimer.Stop()
				}
				return
			}

			msgType := update.Header.Type
			linkName := update.Link.Attrs().Name
			linkMAC := update.Link.Attrs().HardwareAddr.String()

			shimLog.WithFields(logrus.Fields{
				"type":  nlMsgTypeName(msgType),
				"name":  linkName,
				"mac":   linkMAC,
				"flags": update.Link.Attrs().Flags,
			}).Debug("netlink event received")

			if msgType == unix.RTM_DELLINK {
				handleLinkRemoved(ctx, s, knownIfaces, linkMAC, linkName)
				continue
			}

			// For NEWLINK events, debounce to let the CNI plugin
			// finish configuring addresses.
			if debounceTimer == nil {
				debounceTimer = time.NewTimer(netlinkWatcherDebounce)
				debounceCh = debounceTimer.C
			} else {
				debounceTimer.Reset(netlinkWatcherDebounce)
			}

		case <-debounceCh:
			debounceTimer = nil
			debounceCh = nil
			handleNewLinks(ctx, s, knownIfaces, netNsPath)
		}
	}
}

// knownIfaceSet tracks interfaces by both MAC and name for reliable
// detection of additions and removals. DELLINK events for destroyed
// veth peers often arrive with an empty MAC, so name-based lookup
// is needed for removal.
type knownIfaceSet struct {
	byMAC  map[string]string // MAC → name
	byName map[string]string // name → MAC
	epMACs endpointMAC       // host name → endpoint/TAP MAC (for RemoveInterface)
}

func newKnownIfaceSet() *knownIfaceSet {
	return &knownIfaceSet{
		byMAC:  make(map[string]string),
		byName: make(map[string]string),
	}
}

// endpointMAC maps host-side name → endpoint (TAP) MAC for removal.
// RemoveInterface matches by endpoint.HardwareAddr() which is the TAP MAC,
// not the host-side veth MAC.
type endpointMAC map[string]string

func (k *knownIfaceSet) add(mac, name string) {
	if mac != "" {
		k.byMAC[mac] = name
	}
	if name != "" {
		k.byName[name] = mac
	}
}

// setEndpointMAC records the TAP/endpoint MAC for a host-side interface
// so that RemoveInterface can use the correct MAC for matching.
func (k *knownIfaceSet) setEndpointMAC(name, epMAC string) {
	if k.epMACs == nil {
		k.epMACs = make(endpointMAC)
	}
	k.epMACs[name] = epMAC
}

// getEndpointMAC returns the TAP/endpoint MAC for a host-side interface.
func (k *knownIfaceSet) getEndpointMAC(name string) string {
	if k.epMACs == nil {
		return ""
	}
	return k.epMACs[name]
}

func (k *knownIfaceSet) remove(mac, name string) {
	delete(k.byMAC, mac)
	delete(k.byName, name)
}

func (k *knownIfaceSet) hasMACOrName(mac, name string) bool {
	if mac != "" {
		if _, ok := k.byMAC[mac]; ok {
			return true
		}
	}
	if name != "" {
		if _, ok := k.byName[name]; ok {
			return true
		}
	}
	return false
}

// lookupMAC returns the MAC for a known interface, looking up by MAC first
// then falling back to name. This handles DELLINK events with empty MACs.
func (k *knownIfaceSet) lookupMAC(mac, name string) string {
	if mac != "" {
		if _, ok := k.byMAC[mac]; ok {
			return mac
		}
	}
	if name != "" {
		if m, ok := k.byName[name]; ok {
			return m
		}
	}
	return ""
}

// buildKnownIfaces scans the pod network namespace to find all interfaces
// already present at startup. These are cold-plugged interfaces that
// should NOT be hot-plugged again.
func buildKnownIfaces(ctx context.Context, s *service) *knownIfaceSet {
	known := newKnownIfaceSet()

	netNsPath := s.sandbox.GetNetNs()
	if netNsPath == "" {
		return known
	}

	nsHandle, err := netns.GetFromPath(netNsPath)
	if err != nil {
		shimLog.WithError(err).Warn("failed to open netns to build known interfaces")
		return known
	}
	defer nsHandle.Close()

	nlHandle, err := netlink.NewHandleAt(nsHandle)
	if err != nil {
		shimLog.WithError(err).Warn("failed to create netlink handle for known interfaces")
		return known
	}
	defer nlHandle.Close()

	links, err := nlHandle.LinkList()
	if err != nil {
		shimLog.WithError(err).Warn("failed to list links for known interfaces")
		return known
	}

	for _, link := range links {
		mac := link.Attrs().HardwareAddr.String()
		name := link.Attrs().Name
		known.add(mac, name)
	}

	shimLog.WithField("known", len(known.byName)).Debug("netlink watcher initialized known interfaces from netns scan")
	return known
}

// handleNewLinks scans the pod network namespace for interfaces that are
// not yet known and hot-plugs them into the VM.
func handleNewLinks(ctx context.Context, s *service, knownIfaces *knownIfaceSet, netNsPath string) {
	nsHandle, err := netns.GetFromPath(netNsPath)
	if err != nil {
		shimLog.WithError(err).Error("failed to open netns for link scan")
		return
	}
	defer nsHandle.Close()

	nlHandle, err := netlink.NewHandleAt(nsHandle)
	if err != nil {
		shimLog.WithError(err).Error("failed to create netlink handle for link scan")
		return
	}
	defer nlHandle.Close()

	links, err := nlHandle.LinkList()
	if err != nil {
		shimLog.WithError(err).Error("failed to list links in netns")
		return
	}

	for _, link := range links {
		attrs := link.Attrs()

		// Skip loopback
		if attrs.Flags&net.FlagLoopback != 0 {
			continue
		}

		// Skip TAP devices created by kata itself (e.g. tap0_kata, tap1_kata)
		// and bridge devices, to avoid recursively hot-plugging infrastructure
		// interfaces that kata creates during AddInterface.
		if isInfraInterface(link) {
			continue
		}

		mac := attrs.HardwareAddr.String()
		if mac == "" {
			continue
		}

		if knownIfaces.hasMACOrName(mac, attrs.Name) {
			continue
		}

		// Check that the interface has at least one address configured,
		// otherwise the CNI plugin hasn't finished setting it up.
		// Note: we only subscribe to link events, not address events
		// (RTM_NEWADDR). In practice CNI plugins assign addresses before
		// or atomically with setting the link UP, so the debounced scan
		// sees them. If a CNI assigns addresses much later without a
		// NEWLINK, the interface would be missed until the next event.
		addrs, err := nlHandle.AddrList(link, netlink.FAMILY_ALL)
		if err != nil {
			shimLog.WithError(err).WithField("link", attrs.Name).Warn("failed to list addresses")
			continue
		}
		if len(addrs) == 0 {
			shimLog.WithField("link", attrs.Name).Debug("skipping interface with no addresses")
			continue
		}

		inf := linkToInterface(link, addrs)

		shimLog.WithFields(logrus.Fields{
			"name":  inf.Name,
			"mac":   inf.HwAddr,
			"addrs": len(addrs),
		}).Warn("hot-plugging new network interface into VM")

		func() {
			defer func() {
				if r := recover(); r != nil {
					shimLog.WithField("panic", r).Error("panic during hot-plug AddInterface")
				}
			}()
			result, err := s.sandbox.AddInterface(ctx, inf)
			if err != nil {
				shimLog.WithError(err).WithField("interface", inf.Name).Error("failed to hot-plug interface")
				return
			}
			knownIfaces.add(mac, attrs.Name)
			// Track the endpoint MAC (TAP MAC) returned by AddInterface
			// so we can pass it to RemoveInterface later. RemoveInterface
			// matches by endpoint.HardwareAddr() which is the TAP MAC.
			if result != nil && result.HwAddr != "" {
				knownIfaces.setEndpointMAC(attrs.Name, result.HwAddr)
			}
			shimLog.WithField("interface", inf.Name).Warn("network interface hot-plugged successfully")
		}()
	}
}

// handleLinkRemoved processes a RTM_DELLINK event by removing the
// corresponding interface from the VM if it was known.
func handleLinkRemoved(ctx context.Context, s *service, knownIfaces *knownIfaceSet, mac, name string) {
	// Look up the MAC by name if the DELLINK event has an empty MAC
	// (common for destroyed veth peers).
	resolvedMAC := knownIfaces.lookupMAC(mac, name)
	if resolvedMAC == "" {
		return
	}

	// Use the endpoint/TAP MAC for RemoveInterface, since
	// RemoveInterface matches by endpoint.HardwareAddr() which
	// is the TAP MAC, not the host-side veth MAC.
	removeMAC := knownIfaces.getEndpointMAC(name)
	if removeMAC == "" {
		removeMAC = resolvedMAC
	}

	shimLog.WithFields(logrus.Fields{
		"name":      name,
		"mac":       resolvedMAC,
		"removeMAC": removeMAC,
	}).Warn("hot-unplugging network interface from VM")

	inf := &pbTypes.Interface{
		Name:   name,
		HwAddr: removeMAC,
	}

	if _, err := s.sandbox.RemoveInterface(ctx, inf); err != nil {
		shimLog.WithError(err).WithField("interface", name).Error("failed to hot-unplug interface")
		return
	}

	knownIfaces.remove(resolvedMAC, name)
	shimLog.WithField("interface", name).Warn("network interface hot-unplugged successfully")
}

// linkToInterface converts a netlink.Link and its addresses to a
// pbTypes.Interface suitable for Sandbox.AddInterface().
func linkToInterface(link netlink.Link, addrs []netlink.Addr) *pbTypes.Interface {
	attrs := link.Attrs()
	inf := &pbTypes.Interface{
		Device: attrs.Name,
		Name:   attrs.Name,
		HwAddr: attrs.HardwareAddr.String(),
		Mtu:    uint64(attrs.MTU),
		Type:   link.Type(),
	}

	for _, addr := range addrs {
		ipAddr := &pbTypes.IPAddress{
			Address: addr.IP.String(),
			Mask:    addrMask(addr),
		}
		if addr.IP.To4() != nil {
			ipAddr.Family = pbTypes.IPFamily_v4
		} else {
			ipAddr.Family = pbTypes.IPFamily_v6
		}
		inf.IPAddresses = append(inf.IPAddresses, ipAddr)
	}

	return inf
}

// addrMask returns the network mask as a string (e.g. "24" or "64").
func addrMask(addr netlink.Addr) string {
	ones, _ := addr.Mask.Size()
	return fmt.Sprintf("%d", ones)
}

// nlMsgTypeName returns a human-readable name for common netlink message types.
func nlMsgTypeName(t uint16) string {
	switch t {
	case unix.RTM_NEWLINK:
		return "RTM_NEWLINK"
	case unix.RTM_DELLINK:
		return "RTM_DELLINK"
	default:
		return fmt.Sprintf("unknown(%d)", t)
	}
}

// isInfraInterface returns true for interfaces that are part of kata's
// internal networking plumbing (TAP devices, bridges, TC filter helpers)
// and should not be hot-plugged into the VM.
func isInfraInterface(link netlink.Link) bool {
	name := link.Attrs().Name
	linkType := link.Type()

	// TAP devices created by kata: tap0_kata, tap1_kata, etc.
	if strings.HasSuffix(name, "_kata") {
		return true
	}

	// Any tuntap or tun device
	if linkType == "tuntap" || linkType == "tun" {
		return true
	}

	// Bridge devices (e.g. br0_kata)
	if linkType == "bridge" {
		return true
	}

	return false
}
