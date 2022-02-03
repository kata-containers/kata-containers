// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Copyright 2015-2019 CNI authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package rootless

import (
	"context"
	"os"
	"sync"

	"github.com/opencontainers/runc/libcontainer/userns"
	"github.com/sirupsen/logrus"
)

var (
	// isRootless states whether execution is rootless or not
	// If nil, rootless is auto-detected
	isRootless *bool

	// lock for the initRootless and isRootless variables
	rLock sync.Mutex

	// XDG_RUNTIME_DIR defines the base directory relative to
	// which user-specific non-essential runtime files are stored.
	rootlessDir = os.Getenv("XDG_RUNTIME_DIR")

	rootlessLog = logrus.WithFields(logrus.Fields{
		"source": "rootless",
	})

	// IsRootless is declared this way for mocking in unit tests
	IsRootless = isRootlessFunc
)

func SetRootless(rootless bool) {
	isRootless = &rootless
}

// SetLogger sets up a logger for the rootless pkg
func SetLogger(ctx context.Context, logger *logrus.Entry) {
	fields := rootlessLog.Data
	rootlessLog = logger.WithFields(fields)
}

// isRootlessFunc states whether kata is being ran with root or not
func isRootlessFunc() bool {
	rLock.Lock()
	defer rLock.Unlock()
	// auto-detect if nil
	if isRootless == nil {
		SetRootless(true)
		// --rootless and --systemd-cgroup options must honoured
		// but with the current implementation this is not possible
		// https://github.com/kata-containers/runtime/issues/2412
		if os.Geteuid() != 0 {
			return true
		}
		if userns.RunningInUserNS() {
			return true
		}
		SetRootless(false)
	}
	return *isRootless
}

// GetRootlessDir returns the path to the location for rootless
// container and sandbox storage
func GetRootlessDir() string {
	return rootlessDir
}
