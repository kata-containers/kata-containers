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

	"github.com/Sirupsen/logrus"
	"github.com/urfave/cli"
)

const (
	// Character used (after an optional filename) before a heading ID.
	anchorPrefix = "#"

	// Character used to signify an "absolute link path" which should
	// expand to the value of the document root.
	absoluteLinkPrefix = "/"
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

func handleDoc(c *cli.Context) error {
	handleLogging(c)

	if c.NArg() == 0 {
		return errNeedFile
	}

	fileName := c.Args().First()
	if fileName == "" {
		return errNeedFile
	}

	createTOC := c.Bool("create-toc")
	singleDocOnly := c.Bool("single-doc-only")

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
		doc.Logger.Debug("Not checking referenced files as user request")
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
	app.Action = handleDoc
	app.Flags = []cli.Flag{
		cli.BoolFlag{
			Name:  "create-toc, t",
			Usage: "display a markdown Table of Contents for the document",
		},
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

	return app.Run(os.Args)
}

func main() {
	err := realMain()
	if err != nil {
		logger.Fatalf("%v", err)
	}
}
