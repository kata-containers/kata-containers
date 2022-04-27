//
// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//--------------------------------------------------------------------
// Description: Tool to read Kata Containers logfmt-formatted [*]
//   log files, sort and display by time, showing the time difference
//   between each log record.
//
//   [*] - https://brandur.org/logfmt
//
//--------------------------------------------------------------------

package main

import (
	"errors"
	"fmt"
	"os"
	"sort"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

// if specified as a file, read from standard input
const stdinFile = "-"

var (
	// set by the build
	name    = ""
	version = ""
	commit  = ""

	// If true, do not unpack the agent log entries from their proxy log
	// entry wrapper.
	disableAgentUnpack = false

	// If true, error if the agent logs are not parseable.
	//
	// The default is to only warn in such circumstances as the kernel can
	// write to the console at any time. Since the agent also writes its
	// structured logs to the console this poses a problem: the log parser
	// will consider the agent log entry to be "corrupt" as it will also
	// contain unstructured kernel messages.
	strict = false

	// tag added to the LogEntry if the agent unpack fails when running in
	// non-strict mode.
	agentUnpackFailTag = fmt.Sprintf("%s-agent-unpack-failed", name)

	quiet = false

	outputFile = os.Stdout

	fileMode = os.FileMode(0600)

	logger *logrus.Entry
)

var notes = fmt.Sprintf(`

NOTES:

- If file is specified as %q, read from standard input.

- If run with '--debug', it is necessary to also specify '--output-file='
  to avoid invalidating the output.

`, stdinFile)

func init() {
	logger = logrus.WithFields(logrus.Fields{
		"name":    name,
		"source":  "log-parser",
		"version": version,
		"commit":  commit,
		"pid":     os.Getpid(),
	})

	logger.Logger.Formatter = &logrus.TextFormatter{
		TimestampFormat: time.RFC3339Nano,
	}

	// Write to stdout to avoid upsetting CI systems that consider stderr
	// writes as indicating an error.
	logger.Logger.Out = os.Stdout
}

func getLogFiles(c *cli.Context) (files []string, err error) {
	if c.NArg() == 0 {
		return []string{}, fmt.Errorf("need files")
	}

	for _, file := range c.Args() {
		var resolved string

		if file == stdinFile {
			// magic stdin file is handled by HexByteReader
			resolved = file
		} else {
			resolved, err = resolvePath(file)
			if err != nil {
				return []string{}, err
			}

			st, err := os.Stat(resolved)
			if err != nil {
				panic("BUG: resolvePath() should detect missing files")
			}

			if st.Size() == 0 {
				if c.GlobalBool("error-if-file-empty") {
					return []string{}, fmt.Errorf("file %q empty", file)
				}

				logger.Debugf("ignoring empty file %q\n", resolved)
				continue
			}
		}

		files = append(files, resolved)
	}

	if len(files) == 0 {
		msg := "no log records to process"

		if c.GlobalBool("error-if-no-records") {
			return []string{}, errors.New(msg)
		}

		logger.Debug(msg)
	}

	return files, nil
}

func handleLogFiles(c *cli.Context) (err error) {
	outputFilename := c.GlobalString("output-file")

	level := logrus.InfoLevel

	if c.GlobalBool("quiet") {
		level = logrus.ErrorLevel
	}

	if c.GlobalBool("debug") {
		if outputFilename == "" && !c.GlobalBool("check-only") {
			return fmt.Errorf("must specify '--output-file' with '--debug' to avoid invalidating output")
		}

		level = logrus.DebugLevel
	}

	logger.Logger.SetLevel(level)
	handlers := NewDisplayHandlers()

	availableFormats := handlers.Get()

	if c.GlobalBool("list-output-formats") {
		for _, format := range availableFormats {
			fmt.Fprintf(outputFile, "%s\n", format)
		}

		return nil
	}

	files, err := getLogFiles(c)
	if err != nil {
		return err
	}

	entries, err := parseLogFiles(files, c.GlobalBool("ignore-missing-fields"))
	if err != nil {
		return err
	}

	var formats []string
	file := outputFile

	var devNull *os.File

	// In check mode, don't write the output to the specified output file,
	// but *do* run all the display formatters on the data as they might
	// detect issues with the data that this program can't.
	if c.GlobalBool("check-only") {
		formats = availableFormats
		devNull, err = os.OpenFile(os.DevNull, os.O_WRONLY, fileMode)
		if err != nil {
			return nil
		}

		defer func() {
			err = devNull.Close()
		}()

		file = devNull
	} else {
		if outputFilename != "" {
			outputFile, err = os.OpenFile(outputFilename, os.O_CREATE|os.O_WRONLY, fileMode)
			if err != nil {
				return err
			}

			defer func() {
				err = outputFile.Close()
			}()

			file = outputFile
		}

		format := c.GlobalString("output-format")
		formats = append(formats, format)
	}

	return runHandlers(files, &entries, handlers, formats, file,
		c.GlobalBool("check-only"), c.GlobalBool("debug"))
}

func runHandlers(allFiles []string, entries *LogEntries, handlers *DisplayHandlers, formats []string,
	file *os.File, checkOnly, debug bool) error {
	for _, f := range formats {
		err := handlers.Handle(entries, f, file)
		if err != nil {
			if checkOnly {
				return fmt.Errorf("check failed for format %q: %v", f, err)
			}

			return err
		}
	}

	if debug {
		showSummary(entries, allFiles)
	}

	return nil
}

func showSummary(entries *LogEntries, files []string) {
	counts := make(map[string]uint64)

	for _, e := range entries.Entries {
		file := e.Filename

		count := counts[file]
		count++

		counts[file] = count
	}

	sort.Strings(files)

	recordCount := entries.Len()
	fileCount := len(files)

	recordCountStr := "s"
	if recordCount == 1 {
		recordCountStr = ""
	}

	fileCountStr := "s"
	if fileCount == 1 {
		fileCountStr = ""
	}

	logger.Debugf("parsed %d log record%s in %d file%s",
		recordCount,
		recordCountStr,
		fileCount,
		fileCountStr)

	for _, f := range files {
		logger.Debugf("%d records from file %q", counts[f], f)
	}
}

func main() {
	cli.VersionPrinter = func(c *cli.Context) {
		fmt.Fprintln(os.Stdout, c.App.Version)
	}

	cli.AppHelpTemplate = fmt.Sprintf(`%s%s`, cli.AppHelpTemplate,
		notes)
	app := cli.NewApp()
	app.Name = name
	app.Version = fmt.Sprintf("%s %s (commit %v)", name, version, commit)
	app.Description = "tool to collate logfmt-format log files"
	app.Usage = app.Description
	app.UsageText = fmt.Sprintf("%s [options] file ...", app.Name)
	app.Flags = []cli.Flag{
		cli.BoolFlag{
			Name:  "check-only",
			Usage: "check log files and only display output on error",
		},
		cli.BoolFlag{
			Name:  "debug",
			Usage: "display debug information (requires '--output-file')",
		},
		cli.BoolFlag{
			Name:  "error-if-file-empty",
			Usage: "error if any files are empty",
		},
		cli.BoolFlag{
			Name:  "error-if-no-records",
			Usage: "error if all logfiles are empty",
		},
		cli.BoolFlag{
			Name:  "ignore-missing-fields",
			Usage: "do not make an error for lines with no pid, source, name, or level",
		},
		cli.BoolFlag{
			Name:  "list-output-formats",
			Usage: "show available formatters",
		},
		cli.BoolFlag{
			Name:        "no-agent-unpack",
			Usage:       "do not unpack agent log entries",
			Destination: &disableAgentUnpack,
		},
		cli.BoolFlag{
			Name:        "quiet",
			Usage:       "suppress warning messages (ignored in debug mode)",
			Destination: &quiet,
		},
		cli.BoolFlag{
			Name:        "strict",
			Usage:       "do not tolerate misformed agent messages (generally caused by kernel writes to the console)",
			Destination: &strict,
		},
		cli.StringFlag{
			Name:  "output-format",
			Value: "text",
			Usage: "set the output format (see --list-output-formats)",
		},
		cli.StringFlag{
			Name:  "output-file",
			Usage: "write output to specified file",
		},
	}

	app.Action = handleLogFiles

	err := app.Run(os.Args)
	if err != nil {
		fmt.Fprintf(os.Stderr, "ERROR: %v: %v\n", name, err)
		os.Exit(1)
	}
}
