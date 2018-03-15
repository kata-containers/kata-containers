//
// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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

	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = testAppName
	app.Version = runtimeVersion()

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
