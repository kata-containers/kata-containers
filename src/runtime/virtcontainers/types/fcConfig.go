// Copyright (c) 2019 ARM Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/firecracker/client/models"
)

type FcConfig struct {
	BootSource *models.BootSource `json:"boot-source"`

	MachineConfig *models.MachineConfiguration `json:"machine-config"`

	Vsock *models.Vsock `json:"vsock,omitempty"`

	Logger *models.Logger `json:"logger,omitempty"`

	Metrics *models.Metrics `json:"metrics,omitempty"`

	Drives []*models.Drive `json:"drives,omitempty"`

	NetworkInterfaces []*models.NetworkInterface `json:"network-interfaces,omitempty"`
}
