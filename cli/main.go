// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017-2018 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package main

import (
	"fmt"
	"io"
	"os"
	"os/signal"
	goruntime "runtime"
	"strings"
	"syscall"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
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
// required.
var originalLoggerLevel logrus.Level

// if true, coredump when an internal error occurs or a fatal signal is received
var crashOnError = false

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
		Name:  configFilePathOption,
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
	cli.BoolFlag{
		Name:  showConfigPathsOption,
		Usage: "show config file paths that will be checked for (in order)",
	},
}

// runtimeCommands is the list of supported command-line (sub-)
// commands.
var runtimeCommands = []cli.Command{
	createCLICommand,
	deleteCLICommand,
	execCLICommand,
	killCLICommand,
	listCLICommand,
	pauseCLICommand,
	psCLICommand,
	resumeCLICommand,
	runCLICommand,
	startCLICommand,
	stateCLICommand,
	versionCLICommand,

	// Kata Containers specific extensions
	kataCheckCLICommand,
	kataEnvCLICommand,
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

func setupSignalHandler() {
	sigCh := make(chan os.Signal, 8)

	for _, sig := range fatalSignals() {
		signal.Notify(sigCh, sig)
	}

	go func() {
		sig := <-sigCh

		nativeSignal, ok := sig.(syscall.Signal)
		if ok {
			if fatalSignal(nativeSignal) {
				kataLog.WithField("signal", sig).Error("received fatal signal")
				die()
			}
		}
	}()
}

// beforeSubcommands is the function to perform preliminary checks
// before command-line parsing occurs.
func beforeSubcommands(context *cli.Context) error {
	if context.GlobalBool(showConfigPathsOption) {
		files := getDefaultConfigFilePaths()

		for _, file := range files {
			fmt.Fprintf(defaultOutputFile, "%s\n", file)
		}

		exit(0)
	}

	if userWantsUsage(context) || (context.NArg() == 1 && (context.Args()[0] == checkCmd)) {
		// No setup required if the user just
		// wants to see the usage statement or are
		// running a command that does not manipulate
		// containers.
		return nil
	}

	if path := context.GlobalString("log"); path != "" {
		f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND|os.O_SYNC, 0640)
		if err != nil {
			return err
		}
		kataLog.Logger.Out = f
	}

	switch context.GlobalString("log-format") {
	case "text":
		// retain logrus's default.
	case "json":
		kataLog.Logger.Formatter = new(logrus.JSONFormatter)
	default:
		return fmt.Errorf("unknown log-format %q", context.GlobalString("log-format"))
	}

	// Set virtcontainers logger.
	vci.SetLogger(kataLog)

	// Set the OCI package logger.
	oci.SetLogger(kataLog)

	ignoreLogging := false

	// Add the name of the sub-command to each log entry for easier
	// debugging.
	cmdName := context.Args().First()
	if context.App.Command(cmdName) != nil {
		kataLog = kataLog.WithField("command", cmdName)
	}

	if context.NArg() == 1 && context.Args()[0] == envCmd {
		// simply report the logging setup
		ignoreLogging = true
	}

	configFile, runtimeConfig, err := loadConfiguration(context.GlobalString(configFilePathOption), ignoreLogging)
	if err != nil {
		fatal(err)
	}

	args := strings.Join(context.Args(), " ")

	fields := logrus.Fields{
		"version":   version,
		"commit":    commit,
		"arguments": `"` + args + `"`,
	}

	kataLog.WithFields(fields).Info()

	// make the data accessible to the sub-commands.
	context.App.Metadata = map[string]interface{}{
		"runtimeConfig": runtimeConfig,
		"configFile":    configFile,
	}

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
func createRuntimeApp(args []string) error {
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

func createRuntime() {
	setupSignalHandler()

	setCLIGlobals()

	err := createRuntimeApp(os.Args)
	if err != nil {
		fatal(err)
	}
}

func main() {
	defer handlePanic()
	createRuntime()
}
