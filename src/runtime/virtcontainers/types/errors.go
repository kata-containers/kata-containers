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
	ErrNeedSandbox       = errors.New("sandbox must be specified")
	ErrNeedSandboxID     = errors.New("sandbox ID cannot be empty")
	ErrNeedContainerID   = errors.New("container ID cannot be empty")
	ErrNeedState         = errors.New("state cannot be empty")
	ErrNoSuchContainer   = errors.New("container does not exist")
	ErrInvalidConfigType = errors.New("invalid config type")
)
