// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"strings"
	"testing"
)

func testNsEnterFormatArgs(t *testing.T, args []string, expected string) {
	nsenter := &nsenter{}

	cmd, err := nsenter.formatArgs(args)
	if err != nil {
		t.Fatal(err)
	}

	if strings.Join(cmd, " ") != expected {
		t.Fatal()
	}
}

func TestNsEnterFormatArgsHello(t *testing.T) {
	expectedCmd := "nsenter --target -1 --mount --uts --ipc --net --pid echo hello"

	args := []string{"echo", "hello"}

	testNsEnterFormatArgs(t, args, expectedCmd)
}
