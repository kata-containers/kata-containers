// Copyright (c) 2014,2015,2016,2017 Docker, Inc.
// Copyright (c) 2018 Huawei Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"

	"github.com/kata-containers/runtime/pkg/katautils"
	"github.com/opencontainers/runc/libcontainer/specconv"
	"github.com/urfave/cli"
)

var specCLICommand = cli.Command{
	Name:      "spec",
	Usage:     "create a new specification file",
	ArgsUsage: "",
	Description: `The spec command creates the new specification file named "` + specConfig + `" for
the bundle.

The spec generated is just a starter file. Editing of the spec is required to
achieve desired results. For example, the newly generated spec includes an args
parameter that is initially set to call the "sh" command when the container is
started. Calling "sh" may work for an ubuntu container or busybox, but will not
work for containers that do not include the "sh" program.

EXAMPLE:
  To run docker's hello-world container one needs to set the args parameter
in the spec to call hello. This can be done using the sed command or a text
editor. The following commands create a bundle for hello-world, change the
default args parameter in the spec from "sh" to "/hello", then run the hello
command in a new hello-world container named container1:

    mkdir hello
    cd hello
    docker pull hello-world
    docker export $(docker create hello-world) > hello-world.tar
    mkdir rootfs
    tar -C rootfs -xf hello-world.tar
    kata-runtime spec
    sed -i 's;"sh";"/hello";' ` + specConfig + `
    kata-runtime run container1

In the run command above, "container1" is the name for the instance of the
container that you are starting. The name you provide for the container instance
must be unique on your host.

An alternative for generating a customized spec config is to use "oci-runtime-tool", the
sub-command "oci-runtime-tool generate" has lots of options that can be used to do any
customizations as you want, see runtime-tools (https://github.com/opencontainers/runtime-tools)
to get more information.

When starting a container through kata-runtime, kata-runtime needs root privilege. If not
already running as root, you can use sudo to give kata-runtime root privilege. For
example: "sudo kata-runtime start container1" will give kata-runtime root privilege to start the
container on your host.

Alternatively, you can start a rootless container, which has the ability to run
without root privileges. For this to work, the specification file needs to be
adjusted accordingly. You can pass the parameter --rootless to this command to
generate a proper rootless spec file.`,
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "bundle, b",
			Value: "",
			Usage: "path to the root of the bundle directory",
		},
	},
	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		span, _ := katautils.Trace(ctx, "spec")
		defer span.Finish()

		spec := specconv.Example()

		checkNoFile := func(name string) error {
			_, err := os.Stat(name)
			if err == nil {
				return fmt.Errorf("File %s exists. Remove it first", name)
			}
			if !os.IsNotExist(err) {
				return err
			}
			return nil
		}
		bundle := context.String("bundle")
		if bundle != "" {
			if err := os.Chdir(bundle); err != nil {
				return err
			}
		}
		if err := checkNoFile(specConfig); err != nil {
			return err
		}
		data, err := json.MarshalIndent(spec, "", "\t")
		if err != nil {
			return err
		}
		if err := ioutil.WriteFile(specConfig, data, 0640); err != nil {
			return err
		}
		return nil
	},
}
