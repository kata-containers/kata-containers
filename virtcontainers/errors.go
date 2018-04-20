// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"errors"
)

// common error objects used for argument checking
var (
	errNeedSandbox     = errors.New("Sandbox must be specified")
	errNeedSandboxID   = errors.New("Sandbox ID cannot be empty")
	errNeedContainerID = errors.New("Container ID cannot be empty")
	errNeedFile        = errors.New("File cannot be empty")
	errNeedState       = errors.New("State cannot be empty")
	errInvalidResource = errors.New("Invalid sandbox resource")
	errNoSuchContainer = errors.New("Container does not exist")
)
