// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"fmt"

	"github.com/sirupsen/logrus"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
)

func deviceLogger() *logrus.Entry {
	return api.DeviceLogger()
}

// FIXME: this is duplicate code from virtcontainers/hypervisor.go
const maxDevIDSize = 31

// FIXME: this is duplicate code from virtcontainers/hypervisor.go
//Generic function for creating a named-id for passing on the hypervisor commandline
func makeNameID(namedType string, id string) string {
	nameID := fmt.Sprintf("%s-%s", namedType, id)
	if len(nameID) > maxDevIDSize {
		nameID = nameID[:maxDevIDSize]
	}

	return nameID
}
