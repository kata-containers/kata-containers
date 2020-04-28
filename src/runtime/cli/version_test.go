// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"io/ioutil"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func TestVersion(t *testing.T) {
	const testAppName = "foo"
	const testAppVersion = "0.1.0"

	resetCLIGlobals()

	savedRuntimeVersionFunc := runtimeVersion

	defer func() {
		runtimeVersion = savedRuntimeVersionFunc
	}()

	runtimeVersion := func() string {
		return testAppVersion
	}

	ctx := createCLIContext(nil)
	ctx.App.Name = testAppName
	ctx.App.Version = runtimeVersion()

	fn, ok := versionCLICommand.Action.(func(context *cli.Context) error)
	assert.True(t, ok)

	tmpfile, err := ioutil.TempFile("", "")
	assert.NoError(t, err)
	defer os.Remove(tmpfile.Name())

	ctx.App.Writer = tmpfile

	err = fn(ctx)
	assert.NoError(t, err)

	pattern := fmt.Sprintf("%s.*version.*%s", testAppName, testAppVersion)
	err = grep(pattern, tmpfile.Name())
	assert.NoError(t, err)
}
