// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package rootless

import (
	"bufio"
	"context"
	"io"
	"os"
	"strconv"
	"strings"
	"sync"

	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
)

var (
	// initRootless states whether the isRootless variable
	// has been set yet
	initRootless bool

	// isRootless states whether execution is rootless or not
	isRootless bool

	// lock for the initRootless and isRootless variables
	rLock sync.Mutex

	// XDG_RUNTIME_DIR defines the base directory relative to
	// which user-specific non-essential runtime files are stored.
	rootlessDir = os.Getenv("XDG_RUNTIME_DIR")

	// uidMapPath defines the location of the uid_map file to
	// determine whether a user is root or not
	uidMapPath = "/proc/self/uid_map"

	rootlessLog = logrus.WithFields(logrus.Fields{
		"source": "rootless",
	})
)

// SetLogger sets up a logger for the rootless pkg
func SetLogger(ctx context.Context, logger *logrus.Entry) {
	fields := rootlessLog.Data
	rootlessLog = logger.WithFields(fields)
}

// setRootless reads a uid_map file, compares the UID of the
// user inside the container vs on the host. If the host UID
// is not root, but the container ID is, it can be determined
// the user is running rootlessly.
func setRootless() error {
	initRootless = true
	file, err := os.Open(uidMapPath)
	if err != nil {
		return err
	}
	defer file.Close()

	buf := bufio.NewReader(file)
	for {
		line, _, err := buf.ReadLine()
		if err != nil {
			if err == io.EOF {
				return nil
			}
			return err
		}
		if line == nil {
			return nil
		}

		var parseError = errors.Errorf("Failed to parse uid map file %s", uidMapPath)
		// if the container id (id[0]) is 0 (root inside the container)
		// has a mapping to the host id (id[1]) that is not root, then
		// it can be determined that the host user is running rootless
		ids := strings.Fields(string(line))
		// do some sanity checks
		if len(ids) != 3 {
			return parseError
		}
		userNSUid, err := strconv.ParseUint(ids[0], 10, 0)
		if err != nil {
			return parseError
		}
		hostUID, err := strconv.ParseUint(ids[1], 10, 0)
		if err != nil {
			return parseError
		}
		rangeUID, err := strconv.ParseUint(ids[2], 10, 0)
		if err != nil || rangeUID == 0 {
			return parseError
		}

		if userNSUid == 0 && hostUID != 0 {
			rootlessLog.Info("Running as rootless")
			isRootless = true
			return nil
		}
	}
}

// IsRootless states whether kata is being ran with root or not
func IsRootless() bool {
	rLock.Lock()
	if !initRootless {
		err := setRootless()
		if err != nil {
			rootlessLog.WithError(err).Error("Unable to determine if running rootless")
		}
	}
	rLock.Unlock()
	return isRootless
}

// GetRootlessDir returns the path to the location for rootless
// container and sandbox storage
func GetRootlessDir() string {
	return rootlessDir
}
