// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Description: Program to check and summarise the Kata GitHub
//   labels YAML file.

package main

import (
	"errors"
	"fmt"
	"os"

	"github.com/urfave/cli"
)

type DataToShow int

const (
	showLabels     DataToShow = iota
	showCategories DataToShow = iota

	textFormat          = "text"
	defaultOutputFormat = textFormat
)

var errNeedYAMLFile = errors.New("need YAML file")

var (
	// set by the build
	name    = ""
	version = ""
	commit  = ""

	debug = false
)

var formatFlag = cli.StringFlag{
	Name:  "format",
	Usage: "display in specified format ('help' to show all)",
	Value: defaultOutputFormat,
}

func commonHandler(context *cli.Context, what DataToShow, withLabels bool) error {
	handlers := NewDisplayHandlers()

	format := context.String("format")
	if format == "help" {
		availableFormats := handlers.Get()

		for _, format := range availableFormats {
			fmt.Fprintf(outputFile, "%s\n", format)
		}

		return nil
	}

	handler := handlers.find(format)
	if handler == nil {
		return fmt.Errorf("no handler for format %q", format)
	}

	if context.NArg() == 0 {
		return errNeedYAMLFile
	}

	file := context.Args().Get(0)

	return show(file, handler, what, withLabels)
}

func main() {
	app := cli.NewApp()
	app.Description = "tool to manipulate Kata GitHub labels"
	app.Usage = app.Description
	app.Version = fmt.Sprintf("%s %s (commit %v)", name, version, commit)

	app.Flags = []cli.Flag{
		cli.BoolFlag{
			Name:        "debug, d",
			Usage:       "enable debug output",
			Destination: &debug,
		},
	}

	app.Commands = []cli.Command{
		{
			Name:        "check",
			Usage:       "Perform tests on the labels database",
			Description: "Exit code denotes success",
			Action: func(context *cli.Context) error {
				if context.NArg() == 0 {
					return errNeedYAMLFile
				}

				file := context.Args().Get(0)

				return checkYAML(file)
			},
		},
		{
			Name:  "show",
			Usage: "Display labels database details",
			Subcommands: []cli.Command{
				{
					Name:  "categories",
					Usage: "Display categories from labels database",
					Flags: []cli.Flag{
						formatFlag,
						cli.BoolFlag{
							Name:  "with-labels",
							Usage: "Add labels in each category to output",
						},
					},
					Action: func(context *cli.Context) error {
						withLabels := context.Bool("with-labels")
						return commonHandler(context, showCategories, withLabels)
					},
				},
				{
					Name:  "labels",
					Usage: "Display labels from labels database",
					Flags: []cli.Flag{
						formatFlag,
					},
					Action: func(context *cli.Context) error {
						withLabels := context.Bool("with-labels")
						return commonHandler(context, showLabels, withLabels)
					},
				},
			},
		},
		{
			Name:        "sort",
			Usage:       "Sort the specified YAML labels file and write to a new file",
			Description: "Can be used to keep the master labels file sorted",
			ArgsUsage:   "<input-file> <output-file>",
			Action: func(context *cli.Context) error {
				if context.NArg() != 2 {
					return errors.New("need two YAML files: <input-file> <output-file>")
				}

				from := context.Args().Get(0)
				to := context.Args().Get(1)
				return sortYAML(from, to)
			},
		},
	}

	err := app.Run(os.Args)
	if err != nil {
		fmt.Fprintf(os.Stderr, "ERROR: %v\n", err)
		os.Exit(1)
	}
}
