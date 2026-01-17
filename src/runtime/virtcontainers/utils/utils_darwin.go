// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import "github.com/sirupsen/logrus"

func GetDevicePathAndFsTypeOptions(mountPoint string) (devicePath, fsType string, fsOptions []string, err error) {
	return
}

func waitForProcessCompletion(pid int, timeoutSecs uint, logger *logrus.Entry) bool {
	return waitProcessUsingWaitLoop(pid, timeoutSecs, logger)
}

func getHostNUMANodes() ([]int, error) {
	return nil, nil
}

func getHostNUMANodeCPUs(nodeId int) (string, error) {
	return "", nil
}
