// Copyright (c) 2024 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

type DanConfig struct {
	Netns   *string     `json:"netns"`
	Devices []DanDevice `json:"devices"`
}

type DanDevice struct {
	Name        string      `json:"name"`
	GuestMac    string      `json:"guest_mac"`
	Device      Device      `json:"device"`
	NetworkInfo NetworkInfo `json:"network_info"`
}

// DanDeviceType identifies the type of the network interface.
type DanDeviceType string

const (
	VfioDanDeviceType DanDeviceType = "vfio"
)

type Device struct {
	Type        DanDeviceType `json:"type"`
	Path        string        `json:"path,omitempty"`
	PciDeviceID string        `json:"pci_device_id,omitempty"`
	TapName     string        `json:"tap_name,omitempty"`
	QueueNum    int           `json:"queue_num,omitempty"`
	QueueSize   int           `json:"queue_size,omitempty"`
}

type NetworkInfo struct {
	Interface Interface     `json:"interface,omitempty"`
	Routes    []Route       `json:"routes,omitempty"`
	Neighbors []ARPNeighbor `json:"neighbors,omitempty"`
}

type Interface struct {
	IPAddresses []string `json:"ip_addresses"`
	MTU         uint64   `json:"mtu"`
	NType       string   `json:"ntype,omitempty"`
	Flags       uint32   `json:"flags,omitempty"`
}

type Route struct {
	Dest    string `json:"dest,omitempty"`
	Gateway string `json:"gateway"`
	Source  string `json:"source,omitempty"`
	Scope   uint32 `json:"scope,omitempty"`
}

type ARPNeighbor struct {
	IPAddress    string `json:"ip_address"`
	HardwareAddr string `json:"hardware_addr"`
	State        uint32 `json:"state,omitempty"`
	Flags        uint32 `json:"flags,omitempty"`
}
