// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/signal"
	goruntime "runtime"
	"strings"
	"syscall"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/signals"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	exp "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/experimental"
	vf "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory"
	tl "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory/template"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

// specConfig is the name of the file holding the containers configuration
const specConfig = "config.json"

// arch is the architecture for the running program
const arch = goruntime.GOARCH

var usage = fmt.Sprintf(`%s runtime

%s is a command line program for running applications packaged
according to the Open Container Initiative (OCI).`, name, name)

var notes = fmt.Sprintf(`
NOTES:

- Commands starting "%s-" and options starting "--%s-" are `+project+` extensions.

URL:

  The canonical URL for this project is: %s

`, projectPrefix, projectPrefix, projectURL)

// kataLog is the logger used to record all messages
var kataLog *logrus.Entry

// originalLoggerLevel is the default log level. It is used to revert the
// current log level back to its original value if debug output is not
// required. We set the default to 'Warn' for the runtime.
var originalLoggerLevel = logrus.WarnLevel

var debug = false

// concrete virtcontainer implementation
var virtcontainersImpl = &vc.VCImpl{}

// vci is used to access a particular virtcontainers implementation.
// Normally, it refers to the official package, but is re-assigned in
// the tests to allow virtcontainers to be mocked.
var vci vc.VC = virtcontainersImpl

// defaultOutputFile is the default output file to write the gathered
// information to.
var defaultOutputFile = os.Stdout

// defaultErrorFile is the default output file to write error
// messages to.
var defaultErrorFile = os.Stderr

// runtimeFlags is the list of supported global command-line flags
var runtimeFlags = []cli.Flag{
	cli.StringFlag{
		Name:  "config, kata-config",
		Usage: project + " config file path",
	},
	cli.StringFlag{
		Name:  "log",
		Value: "/dev/null",
		Usage: "set the log file path where internal debug information is written",
	},
	cli.StringFlag{
		Name:  "log-format",
		Value: "text",
		Usage: "set the format used by logs ('text' (default), or 'json')",
	},
	cli.StringFlag{
		Name:  "root",
		Value: defaultRootDirectory,
		Usage: "root directory for storage of container state (this should be located in tmpfs)",
	},
	cli.StringFlag{
		Name:  "rootless",
		Value: "auto",
		Usage: "ignore cgroup permission errors ('true', 'false', or 'auto')",
	},
	cli.BoolFlag{
		Name:  "show-default-config-paths, kata-show-default-config-paths",
		Usage: "show config file paths that will be checked for (in order)",
	},
	cli.BoolFlag{
		Name:  "systemd-cgroup",
		Usage: "enable systemd cgroup support, expects cgroupsPath to be of form \"slice:prefix:name\" for e.g. \"system.slice:runc:434234\"",
	},
}

// runtimeCommands is the list of supported command-line (sub-)
// commands.
var runtimeCommands = []cli.Command{
	versionCLICommand,

	// Kata Containers specific extensions
	kataCheckCLICommand,
	kataEnvCLICommand,
	kataExecCLICommand,
	kataMetricsCLICommand,
	factoryCLICommand,
}

// runtimeBeforeSubcommands is the function to run before command-line
// parsing occurs.
var runtimeBeforeSubcommands = beforeSubcommands

// runtimeCommandNotFound is the function to handle an invalid sub-command.
var runtimeCommandNotFound = commandNotFound

// runtimeVersion is the function that returns the full version
// string describing the runtime.
var runtimeVersion = makeVersionString

// saved default cli package values (for testing).
var savedCLIAppHelpTemplate = cli.AppHelpTemplate
var savedCLIVersionPrinter = cli.VersionPrinter
var savedCLIErrWriter = cli.ErrWriter

func init() {
	kataLog = logrus.WithFields(logrus.Fields{
		"name":   name,
		"source": "runtime",
		"arch":   arch,
		"pid":    os.Getpid(),
	})

	// Save the original log level and then set to debug level to ensure
	// that any problems detected before the config file is parsed are
	// logged. This is required since the config file determines the true
	// log level for the runtime: once parsed the log level is set
	// appropriately but for issues between now and completion of the
	// config file parsing, it is prudent to operate in verbose mode.
	originalLoggerLevel = kataLog.Logger.Level
	kataLog.Logger.Level = logrus.DebugLevel
}

// setupSignalHandler sets up signal handling, starting a go routine to deal
// with signals as they arrive.
func setupSignalHandler(ctx context.Context) {
	signals.SetLogger(kataLog)

	sigCh := make(chan os.Signal, 8)

	for _, sig := range signals.HandledSignals() {
		signal.Notify(sigCh, sig)
	}

	go func() {
		for {
			sig := <-sigCh

			nativeSignal, ok := sig.(syscall.Signal)
			if !ok {
				err := errors.New("unknown signal")
				kataLog.WithError(err).WithField("signal", sig.String()).Error()
				continue
			}

			if signals.FatalSignal(nativeSignal) {
				kataLog.WithField("signal", sig).Error("received fatal signal")
			} else if debug && signals.NonFatalSignal(nativeSignal) {
				kataLog.WithField("signal", sig).Debug("handling signal")
				signals.Backtrace()
			}
		}
	}()
}

// setExternalLoggers registers the specified logger with the external
// packages which accept a logger to handle their own logging.
func setExternalLoggers(ctx context.Context, logger *logrus.Entry) {
	// Set virtcontainers logger.
	vci.SetLogger(ctx, logger)

	// Set vm factory logger.
	vf.SetLogger(ctx, logger)

	// Set vm factory template logger.
	tl.SetLogger(ctx, logger)

	// Set the OCI package logger.
	oci.SetLogger(ctx, logger)

	// Set the katautils package logger
	katautils.SetLogger(ctx, logger, originalLoggerLevel)

	// Set the rootless package logger
	rootless.SetLogger(ctx, logger)
}

// beforeSubcommands is the function to perform preliminary checks
// before command-line parsing occurs.
func beforeSubcommands(c *cli.Context) error {
	var configFile string
	var runtimeConfig oci.RuntimeConfig
	var err error

	katautils.SetConfigOptions(name, defaultRuntimeConfiguration, defaultSysConfRuntimeConfiguration)

	handleShowConfig(c)

	if userWantsUsage(c) {
		// No setup required if the user just
		// wants to see the usage statement.
		return nil
	}

	r, err := parseBoolOrAuto(c.GlobalString("rootless"))
	if err != nil {
		return err
	}
	// If flag is true/false, assign the rootless flag.
	// vc will not perform any auto-detection in that case.
	// In case flag is nil or auto, vc detects if the runtime is running as rootless.
	if r != nil {
		rootless.SetRootless(*r)
	}
	// Support --systed-cgroup
	// Issue: https://github.com/kata-containers/runtime/issues/2428

	ignoreConfigLogs := false

	subCmdIsCheckCmd := (c.NArg() >= 1 && ((c.Args()[0] == "kata-check") || (c.Args()[0] == "check")))
	if subCmdIsCheckCmd {
		// checkCmd will use the default logrus logger to stderr
		// raise the logger default level to warn
		kataLog.Logger.SetLevel(logrus.WarnLevel)
		// do not print configuration logs for checkCmd
		ignoreConfigLogs = true
	} else {
		if path := c.GlobalString("log"); path != "" {
			f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND|os.O_SYNC, 0640)
			if err != nil {
				return err
			}
			kataLog.Logger.Out = f
		}

		switch c.GlobalString("log-format") {
		case "text":
			// retain logrus's default.
		case "json":
			kataLog.Logger.Formatter = new(logrus.JSONFormatter)
		default:
			return fmt.Errorf("unknown log-format %q", c.GlobalString("log-format"))
		}

		// Add the name of the sub-command to each log entry for easier
		// debugging.
		cmdName := c.Args().First()
		if c.App.Command(cmdName) != nil {
			kataLog = kataLog.WithField("command", cmdName)
		}

		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}
		setExternalLoggers(ctx, kataLog)

		if c.NArg() == 1 && (c.Args()[0] == "kata-env" || c.Args()[0] == "env") {
			// simply report the logging setup
			ignoreConfigLogs = true
		}
	}

	configFile, runtimeConfig, err = katautils.LoadConfiguration(c.GlobalString("kata-config"), ignoreConfigLogs)
	if err != nil {
		fatal(err)
	}
	if !subCmdIsCheckCmd {
		debug = runtimeConfig.Debug
	}

	args := strings.Join(c.Args(), " ")

	fields := logrus.Fields{
		"version":   version,
		"commit":    commit,
		"arguments": `"` + args + `"`,
	}

	err = addExpFeatures(c, runtimeConfig)
	if err != nil {
		return err
	}

	kataLog.WithFields(fields).Info()

	// make the data accessible to the sub-commands.
	c.App.Metadata["runtimeConfig"] = runtimeConfig
	c.App.Metadata["configFile"] = configFile

	return nil
}

// handleShowConfig determines if the user wishes to see the configuration
// paths. If so, it will display them and then exit.
func handleShowConfig(context *cli.Context) {
	if context.GlobalBool("show-default-config-paths") {
		files := katautils.GetDefaultConfigFilePaths()

		for _, file := range files {
			fmt.Fprintf(defaultOutputFile, "%s\n", file)
		}

		exit(0)
	}
}

// add supported experimental features in context
func addExpFeatures(clictx *cli.Context, runtimeConfig oci.RuntimeConfig) error {
	ctx, err := cliContextToContext(clictx)
	if err != nil {
		return err
	}

	var exps []string
	for _, e := range runtimeConfig.Experimental {
		exps = append(exps, e.Name)
	}

	ctx = exp.ContextWithExp(ctx, exps)
	// Add experimental features to metadata and update the context
	clictx.App.Metadata["context"] = ctx
	return nil
}

// function called when an invalid command is specified which causes the
// runtime to error.
func commandNotFound(c *cli.Context, command string) {
	err := fmt.Errorf("Invalid command %q", command)
	fatal(err)
}

// makeVersionString returns a multi-line string describing the runtime
// version along with the version of the OCI specification it supports.
func makeVersionString() string {
	v := make([]string, 0, 3)

	versionStr := version
	if versionStr == "" {
		versionStr = unknown
	}

	v = append(v, name+"  : "+versionStr)

	commitStr := commit
	if commitStr == "" {
		commitStr = unknown
	}

	v = append(v, "   commit   : "+commitStr)

	specVersionStr := specs.Version
	if specVersionStr == "" {
		specVersionStr = unknown
	}

	v = append(v, "   OCI specs: "+specVersionStr)

	return strings.Join(v, "\n")
}

// setCLIGlobals modifies various cli package global variables
func setCLIGlobals() {
	cli.AppHelpTemplate = fmt.Sprintf(`%s%s`, cli.AppHelpTemplate, notes)

	// Override the default function to display version details to
	// ensure the "--version" option and "version" command are identical.
	cli.VersionPrinter = func(c *cli.Context) {
		fmt.Fprintln(defaultOutputFile, c.App.Version)
	}

	// If the command returns an error, cli takes upon itself to print
	// the error on cli.ErrWriter and exit.
	// Use our own writer here to ensure the log gets sent to the right
	// location.
	cli.ErrWriter = &fatalWriter{cli.ErrWriter}
}

// createRuntimeApp creates an application to process the command-line
// arguments and invoke the requested runtime command.
func createRuntimeApp(ctx context.Context, args []string) error {
	app := cli.NewApp()

	app.Name = name
	app.Writer = defaultOutputFile
	app.Usage = usage
	app.CommandNotFound = runtimeCommandNotFound
	app.Version = runtimeVersion()
	app.Flags = runtimeFlags
	app.Commands = runtimeCommands
	app.Before = runtimeBeforeSubcommands
	app.EnableBashCompletion = true

	// allow sub-commands to access context
	app.Metadata = map[string]interface{}{
		"context": ctx,
	}

	return app.Run(args)
}

// userWantsUsage determines if the user only wishes to see the usage
// statement.
func userWantsUsage(context *cli.Context) bool {
	if context.NArg() == 0 {
		return true
	}

	if context.NArg() == 1 && (context.Args()[0] == "help" || context.Args()[0] == "version") {
		return true
	}

	if context.NArg() >= 2 && (context.Args()[1] == "-h" || context.Args()[1] == "--help") {
		return true
	}

	return false
}

// fatal prints the error's details exits the program.
func fatal(err error) {
	kataLog.Error(err)
	fmt.Fprintln(defaultErrorFile, err)
	exit(1)
}

type fatalWriter struct {
	cliErrWriter io.Writer
}

func (f *fatalWriter) Write(p []byte) (n int, err error) {
	// Ensure error is logged before displaying to the user
	kataLog.Error(string(p))
	return f.cliErrWriter.Write(p)
}

func createRuntime(ctx context.Context) {
	setupSignalHandler(ctx)

	setCLIGlobals()

	err := createRuntimeApp(ctx, os.Args)
	if err != nil {
		fatal(err)
	}
}

// cliContextToContext extracts the generic context from the specified
// cli context.
func cliContextToContext(c *cli.Context) (context.Context, error) {
	if c == nil {
		return nil, errors.New("need cli.Context")
	}

	// extract the main context
	ctx, ok := c.App.Metadata["context"].(context.Context)
	if !ok {
		return nil, errors.New("invalid or missing context in metadata")
	}

	return ctx, nil
}

func main() {
	// create a new empty context
	ctx := context.Background()
	createRuntime(ctx)
}
