//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"io/ioutil"
	"strings"

	"github.com/docker/docker/pkg/parsers"
	"github.com/sirupsen/logrus"
)

// set function in variable to overwrite for testing.
var getCpusetGuest = func() (string, error) {
	cpusetGuestByte, err := ioutil.ReadFile("/sys/fs/cgroup/cpuset/cpuset.cpus")
	if err != nil {
		return "", err
	}

	return strings.TrimSpace(string(cpusetGuestByte)), nil
}

// Return the best match for cpuset list in the guest.
// The runtime caller may apply cpuset for specific CPUs in the host.
// The CPUs may not exist on the guest as they are hotplugged based
// on cpu and qouta.
// This function return a working cpuset to apply on the guest.
func getAvailableCpusetList(cpusetReq string) (string, error) {

	cpusetGuest, err := getCpusetGuest()
	if err != nil {
		return "", err
	}

	cpusetListReq, err := parsers.ParseUintList(cpusetReq)
	if err != nil {
		return "", err
	}

	cpusetGuestList, err := parsers.ParseUintList(cpusetGuest)
	if err != nil {
		return "", err
	}

	for k := range cpusetListReq {
		if !cpusetGuestList[k] {
			agentLog.WithFields(logrus.Fields{
				"cpuset":     cpusetReq,
				"cpu":        k,
				"guest-cpus": cpusetGuest,
			}).Warnf("cpu is not in guest cpu list, using guest cpus")
			return cpusetGuest, nil
		}
	}

	// All the cpus are valid keep the same cpuset string
	agentLog.WithFields(logrus.Fields{
		"cpuset": cpusetReq,
	}).Debugf("the requested cpuset is valid, using it")
	return cpusetReq, nil
}
