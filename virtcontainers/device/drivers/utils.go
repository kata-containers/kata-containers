// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"github.com/sirupsen/logrus"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
)

func deviceLogger() *logrus.Entry {
	return api.DeviceLogger()
}
