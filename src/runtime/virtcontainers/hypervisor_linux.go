// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

func generateVMSocket(id string, vmStogarePath string) (interface{}, error) {
	vhostFd, contextID, err := utils.FindContextID()
	if err != nil {
		return nil, err
	}

	return types.VSock{
		VhostFd:   vhostFd,
		ContextID: contextID,
		Port:      uint32(vSockPort),
	}, nil
}

// NewHypervisor returns an hypervisor from a hypervisor type.
func NewHypervisor(hType HypervisorType) (Hypervisor, error) {
	switch hType {
	case QemuHypervisor:
		return &qemu{}, nil
	case FirecrackerHypervisor:
		return &firecracker{}, nil
	case ClhHypervisor:
		return &cloudHypervisor{}, nil
	case StratovirtHypervisor:
		return &stratovirt{}, nil
	case DragonballHypervisor:
		return &mockHypervisor{}, nil
	case RemoteHypervisor:
		return &remoteHypervisor{}, nil
	case MockHypervisor:
		return &mockHypervisor{}, nil
	default:
		return nil, fmt.Errorf("Unknown hypervisor type %s", hType)
	}
}
