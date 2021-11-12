// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"errors"
)

// common error objects used for argument checking
var (
	ErrNeedSandbox       = errors.New("Sandbox must be specified")
	ErrNeedSandboxID     = errors.New("Sandbox ID cannot be empty")
	ErrNeedContainerID   = errors.New("Container ID cannot be empty")
	ErrNeedState         = errors.New("State cannot be empty")
	ErrNoSuchContainer   = errors.New("Container does not exist")
	ErrInvalidConfigType = errors.New("Invalid config type")
)
