// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

/*
Program checkmetrics compares the results from a set of metrics
results, stored in JSON files, against a set of baseline metrics
'expectations', defined in a TOML file.

It returns non zero if any of the TOML metrics are not met.

It prints out a tabulated report summary at the end of the run.
*/

package main

import (
	"errors"
	"fmt"
	"os"
	"path"

	"github.com/olekukonko/tablewriter"
	log "github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

// name is the name of the program.
const name = "checkmetrics"

// usage is the usage of the program.
const usage = name + ` checks JSON metrics results against a TOML baseline`

var (
	// The TOML basefile
	ciBasefile *baseFile

	// If set then we show results as a relative percentage (to the baseline)
	showPercentage = false

	// System default path for baseline file
	// the value will be set by Makefile
	sysBaseFile string
)

// processMetricsBaseline locates the files matching each entry in the TOML
// baseline, loads and processes it, and checks if the metrics were in range.
// Finally it generates a summary report
func processMetricsBaseline(context *cli.Context) (err error) {
	var report [][]string // summary report table
	var passes int
	var fails int
	var summary []string

	log.Debug("processMetricsBaseline")

	// Process each Metrics TOML entry one at a time
	// FIXME - this is not structured to be testable - if you need to add a unit
	// test here - the *please* re-structure these funcs etc.
	for _, m := range ciBasefile.Metric {
		log.Debugf("Processing %s", m.Name)
		fullpath := path.Join(context.GlobalString("metricsdir"), m.Name)

		switch m.Type {
		case "":
			log.Debugf("No Type, default to JSON for [%s]", m.Name)
			fallthrough
		case "json":
			{
				var thisJSON jsonRecord
				log.Debug("Process a JSON")
				fullpath = fullpath + ".json"
				log.Debugf("Fullpath %s", fullpath)
				err = thisJSON.load(fullpath, &m)

				if err != nil {
					log.Warnf("[%s][%v]", fullpath, err)
					// Record that this one did not complete successfully
					fails++
					// Make some sort of note in the summary table that this failed
					summary = (&metricsCheck{}).genErrorLine(false, m.Name, "Failed to load JSON", fmt.Sprintf("%s", err))
					// Not a fatal error - continue to process any remaining files
					break
				}

				summary, err = (&metricsCheck{}).checkstats(m)
				if err != nil {
					log.Warnf("Check for [%s] failed [%v]", m.Name, err)
					log.Warnf(" with [%s]", summary)
					fails++
				} else {
					log.Debugf("Check for [%s] passed", m.Name)
					log.Debugf(" with [%s]", summary)
					passes++
				}
			}
		default:
			{
				log.Warnf("Unknown type [%s] for metric [%s]", m.Type, m.Name)
				summary = (&metricsCheck{}).genErrorLine(false, m.Name, "Unsupported Type", fmt.Sprint(m.Type))
				fails++
			}
		}

		report = append(report, summary)
		log.Debugf("Done %s", m.Name)
	}

	if fails != 0 {
		log.Warn("Overall we failed")
	}

	fmt.Printf("\n")

	// We need to find a better way here to report that some tests failed to even
	// get into the table - such as JSON file parse failures
	// Actually, now we report file failures into the report as well, we should not
	// see this - but, it is nice to leave as a sanity check.
	if len(report) < fails+passes {
		fmt.Printf("Warning: some tests (%d) failed to report\n", (fails+passes)-len(report))
	}

	// Note - not logging here - the summary goes to stdout
	fmt.Println("Report Summary:")

	table := tablewriter.NewWriter(os.Stdout)

	table.SetHeader((&metricsCheck{}).reportTitleSlice())
	for _, s := range report {
		table.Append(s)
	}
	table.Render()
	fmt.Printf("Fails: %d, Passes %d\n", fails, passes)

	// Did we see any failures during the run?
	if fails != 0 {
		err = errors.New("Failed")
	} else {
		err = nil
	}

	return
}

// checkmetrics main entry point.
// Do the command line processing, load the TOML file, and do the processing
// against the data files
func main() {
	app := cli.NewApp()
	app.Name = name
	app.Usage = usage

	app.Flags = []cli.Flag{
		cli.StringFlag{
			Name:  "basefile",
			Usage: "path to baseline TOML metrics file",
		},
		cli.BoolFlag{
			Name:  "debug",
			Usage: "enable debug output in the log",
		},
		cli.StringFlag{
			Name:  "log",
			Usage: "set the log file path",
		},
		cli.StringFlag{
			Name:  "metricsdir",
			Usage: "directory containing metrics results files",
		},
		cli.BoolFlag{
			Name:        "percentage",
			Usage:       "present results as percentage differences",
			Destination: &showPercentage,
		},
	}

	app.Before = func(context *cli.Context) error {
		var err error
		var baseFilePath string

		if path := context.GlobalString("log"); path != "" {
			f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND|os.O_SYNC, 0640)
			if err != nil {
				return err
			}
			log.SetOutput(f)
		}

		if context.GlobalBool("debug") {
			log.SetLevel(log.DebugLevel)
		}

		if context.GlobalString("metricsdir") == "" {
			log.Error("Must supply metricsdir argument")
			return errors.New("Must supply metricsdir argument")
		}

		baseFilePath = context.GlobalString("basefile")
		if baseFilePath == "" {
			baseFilePath = sysBaseFile
		}

		ciBasefile, err = newBasefile(baseFilePath)

		return err
	}

	app.Action = func(context *cli.Context) error {
		return processMetricsBaseline(context)
	}

	if err := app.Run(os.Args); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
