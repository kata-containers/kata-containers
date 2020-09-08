// Copyright (c) 2020 ARM Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	models "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/firecracker/client/models"
	ops "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/firecracker/client/operations"
)

func init() {
	archFcPowerOffFunc = fcSendCtrlAltDel
}

// At boot time, the Linux driver for i8042 spends a few tens of
// milliseconds probing the device.
// This can be disabled by using the following kernel parameters.
var archRequiredFcKernelParams = []Param{
	{"i8042.noaux", ""},
	{"i8042.nomux", ""},
	{"i8042.nopnp", ""},
	{"i8042.dumbkbd", ""},
}

// Use SendCtrlAltDel API action to send CTRL+ALT+DEL to the VM.
// This can be used to trigger a graceful shutdown of the microVM,
// if the guest has support for i8042 and AT Keyboard.
func fcSendCtrlAltDel(fc *firecracker) error {
	span, _ := fc.trace("fcSendCtrlAltDel")
	defer span.Finish()

	fc.Logger().Info("Sending CTRL+ALT+DEL to the VM")

	actionType := "SendCtrlAltDel"
	actionParams := ops.NewCreateSyncActionParams()
	actionInfo := &models.InstanceActionInfo{
		ActionType: &actionType,
	}
	actionParams.SetInfo(actionInfo)
	if _, err := fc.client().Operations.CreateSyncAction(actionParams); err != nil {
		return err
	}

	return nil
}
