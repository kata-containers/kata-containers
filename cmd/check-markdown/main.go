//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"fmt"
	"os"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

type DataToShow int

const (
	// Character used (after an optional filename) before a heading ID.
	anchorPrefix = "#"

	// Character used to signify an "absolute link path" which should
	// expand to the value of the document root.
	absoluteLinkPrefix = "/"

	showLinks    DataToShow = iota
	showHeadings DataToShow = iota

	textFormat          = "text"
	tsvFormat           = "tsv"
	defaultOutputFormat = textFormat
	defaultSeparator    = "\t"
)

var (
	// set by the build
	name    = ""
	version = ""
	commit  = ""

	strict = false

	// list entry character to use when generating TOCs
	listPrefix = "*"

	logger *logrus.Entry

	errNeedFile = errors.New("need markdown file")
)

// Black Friday sometimes chokes on markdown (I know!!), so record how many
// extra headings we found.
var extraHeadings int

// Root directory used to handle "absolute link paths" that start with a slash
// to denote the "top directory", like this:
//
// [Foo](/absolute-link.md)
var docRoot string

var notes = fmt.Sprintf(`

NOTES:

- The document root is used to handle markdown references that begin with %q,
  denoting that the path that follows is an "absolute path" from the specified
  document root path.

- The order the document nodes are parsed internally is not known to
  this program. This means that if multiple errors exist in the document,
  running this tool multiple times will error one *one* of the errors, but not
  necessarily the same one as last time.

LIMITATIONS:

- The default document root only works if this tool is run from the top-level
  of a repository.

`, absoluteLinkPrefix)

var formatFlag = cli.StringFlag{
	Name:  "format",
	Usage: "display in specified format ('help' to show all)",
	Value: defaultOutputFormat,
}

var separatorFlag = cli.StringFlag{
	Name:  "separator",
	Usage: fmt.Sprintf("use the specified separator character (%s format only)", tsvFormat),
	Value: defaultSeparator,
}

var noHeaderFlag = cli.BoolFlag{
	Name:  "no-header",
	Usage: "disable display of header (if format supports one)",
}

func init() {
	logger = logrus.WithFields(logrus.Fields{
		"name":    name,
		"source":  "check-markdown",
		"version": version,
		"commit":  commit,
		"pid":     os.Getpid(),
	})

	logger.Logger.Formatter = &logrus.TextFormatter{
		TimestampFormat: time.RFC3339Nano,
		//DisableColors:   true,
	}

	// Write to stdout to avoid upsetting CI systems that consider stderr
	// writes as indicating an error.
	logger.Logger.Out = os.Stdout
}

func handleLogging(c *cli.Context) {
	logLevel := logrus.InfoLevel

	if c.GlobalBool("debug") {
		logLevel = logrus.DebugLevel
	}

	logger.Logger.SetLevel(logLevel)
}

func handleDoc(c *cli.Context, createTOC bool) error {
	handleLogging(c)

	if c.NArg() == 0 {
		return errNeedFile
	}

	fileName := c.Args().First()
	if fileName == "" {
		return errNeedFile
	}

	singleDocOnly := c.GlobalBool("single-doc-only")

	doc := newDoc(fileName, logger)
	doc.ShowTOC = createTOC

	if createTOC {
		// Only makes sense to generate a single TOC!
		singleDocOnly = true
	}

	// Parse the main document first
	err := doc.parse()
	if err != nil {
		return err
	}

	if singleDocOnly && len(docs) > 1 {
		doc.Logger.Debug("Not checking referenced files at user request")
		return nil
	}

	// Now handle all other docs that the main doc references.
	// This requires care to avoid recursion.
	for {
		count := len(docs)
		parsed := 0
		for _, doc := range docs {
			if doc.Parsed {
				// Document has already been handled
				parsed++
				continue
			}

			if err := doc.parse(); err != nil {
				return err
			}
		}

		if parsed == count {
			break
		}
	}

	err = handleIntraDocLinks()
	if err != nil {
		return err
	}

	if !createTOC {
		doc.Logger.Info("Checked file")
		doc.showStats()
	}

	count := len(docs)

	if count > 1 {
		// Update to ignore main document
		count--

		doc.Logger.WithField("reference-document-count", count).Info("Checked referenced files")

		for _, d := range docs {
			if d.Name == doc.Name {
				// Ignore main document
				continue
			}

			fmt.Printf("\t%q\n", d.Name)
		}
	}

	// Highlight blackfriday deficiencies
	if !doc.ShowTOC && extraHeadings > 0 {
		doc.Logger.WithField("extra-heading-count", extraHeadings).Debug("Found extra headings")
	}

	return nil
}

// commonListHandler is used to handle all list operations.
func commonListHandler(context *cli.Context, what DataToShow) error {
	handleLogging(context)

	handlers := NewDisplayHandlers(context.String("separator"), context.Bool("no-header"))

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
		return errNeedFile
	}

	file := context.Args().Get(0)

	return show(file, logger, handler, what)
}

func realMain() error {
	cwd, err := os.Getwd()
	if err != nil {
		return err
	}

	docRoot = cwd

	cli.VersionPrinter = func(c *cli.Context) {
		fmt.Fprintln(os.Stdout, c.App.Version)
	}

	cli.AppHelpTemplate = fmt.Sprintf(`%s%s`, cli.AppHelpTemplate, notes)

	app := cli.NewApp()
	app.Name = name
	app.Version = fmt.Sprintf("%s %s (commit %v)", name, version, commit)
	app.Description = "Tool to check GitHub-Flavoured Markdown (GFM) format documents"
	app.Usage = app.Description
	app.UsageText = fmt.Sprintf("%s [options] file ...", app.Name)
	app.Flags = []cli.Flag{
		cli.BoolFlag{
			Name:  "debug, d",
			Usage: "display debug information",
		},
		cli.StringFlag{
			Name:  "doc-root, r",
			Usage: "specify document root",
			Value: docRoot,
		},
		cli.BoolFlag{
			Name:  "single-doc-only, o",
			Usage: "only check primary (specified) document",
		},
		cli.BoolFlag{
			Name:  "strict, s",
			Usage: "enable strict mode",
		},
	}

	app.Commands = []cli.Command{
		{
			Name:        "check",
			Usage:       "perform tests on the specified document",
			Description: "Exit code denotes success",
			Action: func(c *cli.Context) error {
				return handleDoc(c, false)
			},
		},
		{
			Name:  "toc",
			Usage: "display a markdown Table of Contents",
			Action: func(c *cli.Context) error {
				return handleDoc(c, true)
			},
		},
		{
			Name:  "list",
			Usage: "display particular parts of the document",
			Subcommands: []cli.Command{
				{
					Name:  "headings",
					Usage: "display headings",
					Flags: []cli.Flag{
						formatFlag,
						noHeaderFlag,
						separatorFlag,
					},
					Action: func(c *cli.Context) error {
						return commonListHandler(c, showHeadings)
					},
				},
				{
					Name:  "links",
					Usage: "display links",
					Flags: []cli.Flag{
						formatFlag,
						noHeaderFlag,
						separatorFlag,
					},
					Action: func(c *cli.Context) error {
						return commonListHandler(c, showLinks)
					},
				},
			},
		},
	}

	return app.Run(os.Args)
}

func main() {
	err := realMain()
	if err != nil {
		logger.Fatalf("%v", err)
	}
}
