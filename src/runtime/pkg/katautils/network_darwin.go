// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
)

func EnterNetNS(networkID string, cb func() error) error {
	return nil
}

func SetupNetworkNamespace(config *vc.NetworkConfig) error {
	return nil
}

func cleanupNetNS(netNSPath string) error {
	return nil
}
