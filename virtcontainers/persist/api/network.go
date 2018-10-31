// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

// ============= sandbox level resources =============

// NetworkEndpoint contains network interface information
type NetworkEndpoint struct {
	Type string

	// ID used to pass the netdev option to qemu
	ID string

	// Name of the interface
	Name string

	// Index of interface
	Index int
}

// NetworkInfo contains network information of sandbox
type NetworkInfo struct {
	NetNsPath         string
	NetmonPID         int
	NetNsCreated      bool
	InterworkingModel string
	Endpoints         []NetworkEndpoint
}
