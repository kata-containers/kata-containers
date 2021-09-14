// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"github.com/sirupsen/logrus"
	"io/ioutil"
)

var testCPUInfoTemplate = setTestCPUInfoTemplate()

func setTestCPUInfoTemplate() string {

	var kataLog *logrus.Entry
	content, err := ioutil.ReadFile("/proc/cpuinfo")

	if err != nil {
		kataLog.WithError(err).Error("failed to read file /proc/cpuinfo")
	}
	return string(content)
}
