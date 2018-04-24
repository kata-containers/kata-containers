// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"encoding/hex"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

// vhostUserAttach handles the common logic among all of the vhost-user device's
// attach functions
func vhostUserAttach(device api.VhostUserDevice, devReceiver api.DeviceReceiver) error {
	// generate a unique ID to be used for hypervisor commandline fields
	randBytes, err := utils.GenerateRandomBytes(8)
	if err != nil {
		return err
	}
	id := hex.EncodeToString(randBytes)

	device.Attrs().ID = id

	return devReceiver.AddVhostUserDevice(device, device.Type())
}
