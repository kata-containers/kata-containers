// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"flag"
	"io/ioutil"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
)

const testDisabledAsNonRoot = "Test disabled as requires root privileges"

func TestFactoryCLIFunctionNoRuntimeConfig(t *testing.T) {
	assert := assert.New(t)

	ctx := createCLIContext(nil)
	ctx.App.Name = "foo"
	ctx.App.Metadata["foo"] = "bar"

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

	ctx := createCLIContext(set)
	ctx.App.Name = "foo"

	// No template
	ctx.App.Metadata["runtimeConfig"] = runtimeConfig

	fn, ok := initFactoryCommand.Action.(func(context *cli.Context) error)
	assert.True(ok)
	err = fn(ctx)
	assert.Nil(err)

	// With template
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	runtimeConfig.FactoryConfig.Template = true
	runtimeConfig.FactoryConfig.TemplatePath = "/run/vc/vm/template"
	runtimeConfig.HypervisorType = vc.MockHypervisor
	ctx.App.Metadata["runtimeConfig"] = runtimeConfig
	fn, ok = initFactoryCommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	// config mock agent
	stdCtx, err := cliContextToContext(ctx)
	if err != nil {
		stdCtx = context.Background()
	}
	stdCtx = vc.WithNewAgentFunc(stdCtx, vc.NewMockAgent)
	ctx.App.Metadata["context"] = stdCtx

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

	ctx := createCLIContext(set)
	ctx.App.Name = "foo"

	// No template
	ctx.App.Metadata["runtimeConfig"] = runtimeConfig
	fn, ok := destroyFactoryCommand.Action.(func(context *cli.Context) error)
	assert.True(ok)
	err = fn(ctx)
	assert.Nil(err)

	// With template
	runtimeConfig.FactoryConfig.Template = true
	runtimeConfig.HypervisorType = vc.MockHypervisor
	ctx.App.Metadata["runtimeConfig"] = runtimeConfig
	fn, ok = destroyFactoryCommand.Action.(func(context *cli.Context) error)
	assert.True(ok)
	err = fn(ctx)
	assert.Nil(err)
}

func TestFactoryCLIFunctionStatus(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	set := flag.NewFlagSet("", 0)

	set.String("console-socket", "", "")

	ctx := createCLIContext(set)
	ctx.App.Name = "foo"

	// No template
	ctx.App.Metadata["runtimeConfig"] = runtimeConfig

	fn, ok := statusFactoryCommand.Action.(func(context *cli.Context) error)
	assert.True(ok)
	err = fn(ctx)
	assert.Nil(err)

	// With template
	runtimeConfig.FactoryConfig.Template = true
	runtimeConfig.HypervisorType = vc.MockHypervisor
	ctx.App.Metadata["runtimeConfig"] = runtimeConfig
	err = fn(ctx)
	assert.Nil(err)
}
