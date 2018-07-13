// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"io/ioutil"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"

	vc "github.com/kata-containers/runtime/virtcontainers"
)

func TestFactoryCLIFunctionNoRuntimeConfig(t *testing.T) {
	assert := assert.New(t)

	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = "foo"
	ctx.App.Metadata = map[string]interface{}{
		"foo": "bar",
	}

	fn, ok := initFactoryCommand.Action.(func(context *cli.Context) error)
	assert.True(ok)
	err := fn(ctx)
	// no runtime config in the Metadata
	assert.Error(err)

	fn, ok = destroyFactoryCommand.Action.(func(context *cli.Context) error)
	assert.True(ok)
	err = fn(ctx)
	// no runtime config in the Metadata
	assert.Error(err)
}

func TestFactoryCLIFunctionInit(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	set := flag.NewFlagSet("", 0)

	set.String("console-socket", "", "")

	app := cli.NewApp()
	ctx := cli.NewContext(app, set, nil)
	app.Name = "foo"

	// No template
	ctx.App.Metadata = map[string]interface{}{
		"runtimeConfig": runtimeConfig,
	}
	fn, ok := initFactoryCommand.Action.(func(context *cli.Context) error)
	assert.True(ok)
	err = fn(ctx)
	assert.Nil(err)

	// With template
	runtimeConfig.FactoryConfig.Template = true
	runtimeConfig.HypervisorType = vc.MockHypervisor
	runtimeConfig.AgentType = vc.NoopAgentType
	ctx.App.Metadata["runtimeConfig"] = runtimeConfig
	fn, ok = initFactoryCommand.Action.(func(context *cli.Context) error)
	assert.True(ok)
	err = fn(ctx)
	assert.Nil(err)
}

func TestFactoryCLIFunctionDestroy(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	set := flag.NewFlagSet("", 0)

	set.String("console-socket", "", "")

	app := cli.NewApp()
	ctx := cli.NewContext(app, set, nil)
	app.Name = "foo"

	// No template
	ctx.App.Metadata = map[string]interface{}{
		"runtimeConfig": runtimeConfig,
	}
	fn, ok := destroyFactoryCommand.Action.(func(context *cli.Context) error)
	assert.True(ok)
	err = fn(ctx)
	assert.Nil(err)

	// With template
	runtimeConfig.FactoryConfig.Template = true
	runtimeConfig.HypervisorType = vc.MockHypervisor
	runtimeConfig.AgentType = vc.NoopAgentType
	ctx.App.Metadata["runtimeConfig"] = runtimeConfig
	fn, ok = destroyFactoryCommand.Action.(func(context *cli.Context) error)
	assert.True(ok)
	err = fn(ctx)
	assert.Nil(err)
}
