// Copyright (c) 2018 Huawei Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func TestSpecCliAction(t *testing.T) {
	assert := assert.New(t)

	actionFunc, ok := specCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	flagSet := flag.NewFlagSet("flag", flag.ContinueOnError)
	ctx := createCLIContext(flagSet)
	defer os.Remove(specConfig)
	err := actionFunc(ctx)
	assert.NoError(err)

	pattern := "gid=5"
	patternRootless := "uidMappings"
	err = grep(pattern, specConfig)
	assert.NoError(err)
	err = grep(patternRootless, specConfig)
	assert.Error(err)
}
