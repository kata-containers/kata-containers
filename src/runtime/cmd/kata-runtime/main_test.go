// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"context"
	"errors"
	"flag"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"regexp"
	"strings"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"

	"github.com/urfave/cli"
)

const (
	testDirMode     = os.FileMode(0750)
	testFileMode    = os.FileMode(0640)
	testExeFileMode = os.FileMode(0750)
)

var (
	// package variables set by calling TestMain()
	tc ktu.TestConstraint
)

// testingImpl is a concrete mock RVC implementation used for testing
var testingImpl = &vcmock.VCMock{}

func init() {
	if katautils.VERSION == "" {
		panic("ERROR: invalid build: version not set")
	}

	if katautils.COMMIT == "" {
		panic("ERROR: invalid build: commit not set")
	}

	if katautils.DEFAULTSYSCONFRUNTIMECONFIGURATION == "" {
		panic("ERROR: invalid build: defaultSysConfRuntimeConfiguration not set")
	}

	if katautils.DEFAULTRUNTIMECONFIGURATION == "" {
		panic("ERROR: invalid build: defaultRuntimeConfiguration not set")
	}

	fmt.Printf("INFO: running as actual user %v (effective %v), actual group %v (effective %v)\n",
		os.Getuid(), os.Geteuid(), os.Getgid(), os.Getegid())

	fmt.Printf("INFO: switching to fake virtcontainers implementation for testing\n")
	vci = testingImpl

	tc = ktu.NewTestConstraint(false)
}

// resetCLIGlobals undoes the effects of setCLIGlobals(), restoring the original values
func resetCLIGlobals() {
	cli.AppHelpTemplate = savedCLIAppHelpTemplate
	cli.VersionPrinter = savedCLIVersionPrinter
	cli.ErrWriter = savedCLIErrWriter
}

func runUnitTests(m *testing.M) {
	ret := m.Run()

	os.Exit(ret)
}

// TestMain is the common main function used by ALL the test functions
// for this package.
func TestMain(m *testing.M) {
	// If the test binary name is kata-runtime.coverage, we've are being asked to
	// run the coverage-instrumented kata-runtime.
	if path.Base(os.Args[0]) == katautils.NAME+".coverage" ||
		path.Base(os.Args[0]) == katautils.NAME {
		main()
		exitFunc(0)
	}

	runUnitTests(m)
}

func createEmptyFile(path string) (err error) {
	return os.WriteFile(path, []byte(""), testFileMode)
}

func grep(pattern, file string) error {
	if file == "" {
		return errors.New("need file")
	}

	bytes, err := os.ReadFile(file)
	if err != nil {
		return err
	}

	re := regexp.MustCompile(pattern)
	matches := re.FindAllStringSubmatch(string(bytes), -1)

	if matches == nil {
		return fmt.Errorf("pattern %q not found in file %q", pattern, file)
	}

	return nil
}

// newTestHypervisorConfig creaets a new virtcontainers
// HypervisorConfig, ensuring that the required resources are also
// created.
//
// Note: no parameter validation in case caller wishes to create an invalid
// object.
func newTestHypervisorConfig(dir string, create bool) (vc.HypervisorConfig, error) {
	kernelPath := path.Join(dir, "kernel")
	imagePath := path.Join(dir, "image")
	hypervisorPath := path.Join(dir, "hypervisor")

	if create {
		for _, file := range []string{kernelPath, imagePath, hypervisorPath} {
			err := createEmptyFile(file)
			if err != nil {
				return vc.HypervisorConfig{}, err
			}
		}
	}

	return vc.HypervisorConfig{
		KernelPath:            kernelPath,
		ImagePath:             imagePath,
		HypervisorPath:        hypervisorPath,
		HypervisorMachineType: "q35",
	}, nil
}

// newTestRuntimeConfig creates a new RuntimeConfig
func newTestRuntimeConfig(dir string, create bool) (oci.RuntimeConfig, error) {
	if dir == "" {
		return oci.RuntimeConfig{}, errors.New("BUG: need directory")
	}

	hypervisorConfig, err := newTestHypervisorConfig(dir, create)
	if err != nil {
		return oci.RuntimeConfig{}, err
	}

	return oci.RuntimeConfig{
		HypervisorType:   vc.QemuHypervisor,
		HypervisorConfig: hypervisorConfig,
	}, nil
}

func createCLIContextWithApp(flagSet *flag.FlagSet, app *cli.App) *cli.Context {
	ctx := cli.NewContext(app, flagSet, nil)

	// create the map if required
	if ctx.App.Metadata == nil {
		ctx.App.Metadata = map[string]interface{}{}
	}

	// add standard entries
	ctx.App.Metadata["context"] = context.Background()

	return ctx
}

func createCLIContext(flagset *flag.FlagSet) *cli.Context {
	return createCLIContextWithApp(flagset, cli.NewApp())
}

func TestMainUserWantsUsage(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		arguments  []string
		expectTrue bool
	}

	data := []testData{
		{[]string{}, true},
		{[]string{"help"}, true},
		{[]string{"version"}, true},
		{[]string{"sub-command", "-h"}, true},
		{[]string{"sub-command", "--help"}, true},

		{[]string{""}, false},
		{[]string{"sub-command", "--foo"}, false},
		{[]string{"kata-check"}, false},
		{[]string{"haaaalp"}, false},
		{[]string{"wibble"}, false},
		{[]string{"versioned"}, false},
	}

	for i, d := range data {
		set := flag.NewFlagSet("", 0)
		set.Parse(d.arguments)

		ctx := createCLIContext(set)
		result := userWantsUsage(ctx)

		if d.expectTrue {
			assert.True(result, "test %d (%+v)", i, d)
		} else {
			assert.False(result, "test %d (%+v)", i, d)
		}
	}
}

func TestMainBeforeSubCommands(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		arguments   []string
		expectError bool
	}

	data := []testData{
		{[]string{}, false},
		{[]string{"help"}, false},
		{[]string{"version"}, false},
		{[]string{"sub-command", "-h"}, false},
		{[]string{"sub-command", "--help"}, false},
	}

	for i, d := range data {
		set := flag.NewFlagSet("", 0)
		set.Parse(d.arguments)

		ctx := createCLIContext(set)
		err := beforeSubcommands(ctx)

		if d.expectError {
			assert.Errorf(err, "test %d (%+v)", i, d)
		} else {
			assert.NoError(err, "test %d (%+v)", i, d)
		}
	}
}

func TestMainBeforeSubCommandsInvalidLogFile(t *testing.T) {
	assert := assert.New(t)

	tmpdir := t.TempDir()

	logFile := filepath.Join(tmpdir, "log")

	// create the file as the wrong type to force a failure
	err := os.MkdirAll(logFile, testDirMode)
	assert.NoError(err)

	set := flag.NewFlagSet("", 0)
	set.String("log", logFile, "")
	set.Parse([]string{"create"})

	ctx := createCLIContext(set)

	err = beforeSubcommands(ctx)
	assert.Error(err)
}

func TestMainBeforeSubCommandsInvalidLogFormat(t *testing.T) {
	assert := assert.New(t)

	tmpdir := t.TempDir()

	logFile := filepath.Join(tmpdir, "log")

	set := flag.NewFlagSet("", 0)
	set.Bool("debug", true, "")
	set.String("log", logFile, "")
	set.String("log-format", "captain-barnacles", "")
	set.Parse([]string{"create"})

	logOut := kataLog.Logger.Out
	kataLog.Logger.Out = nil

	defer func() {
		kataLog.Logger.Out = logOut
	}()

	ctx := createCLIContext(set)

	err := beforeSubcommands(ctx)
	assert.Error(err)
	assert.NotNil(kataLog.Logger.Out)
}

func TestMainBeforeSubCommandsLoadConfigurationFail(t *testing.T) {
	assert := assert.New(t)

	tmpdir := t.TempDir()

	logFile := filepath.Join(tmpdir, "log")
	configFile := filepath.Join(tmpdir, "config")

	for _, logFormat := range []string{"json", "text"} {
		set := flag.NewFlagSet("", 0)
		set.Bool("debug", true, "")
		set.String("log", logFile, "")
		set.String("log-format", logFormat, "")
		set.String("kata-config", configFile, "")
		set.Parse([]string{"kata-env"})

		ctx := createCLIContext(set)

		savedExitFunc := exitFunc

		exitStatus := 0
		exitFunc = func(status int) { exitStatus = status }

		defer func() {
			exitFunc = savedExitFunc
		}()

		// calls fatal() so no return
		_ = beforeSubcommands(ctx)
		assert.NotEqual(exitStatus, 0)
	}
}

func TestMainBeforeSubCommandsShowCCConfigPaths(t *testing.T) {
	assert := assert.New(t)

	tmpdir := t.TempDir()

	set := flag.NewFlagSet("", 0)
	set.Bool("show-default-config-paths", true, "")

	ctx := createCLIContext(set)

	savedExitFunc := exitFunc

	exitStatus := 99
	exitFunc = func(status int) { exitStatus = status }

	defer func() {
		exitFunc = savedExitFunc
	}()

	savedOutputFile := defaultOutputFile

	defer func() {
		resetCLIGlobals()
		defaultOutputFile = savedOutputFile
	}()

	output := filepath.Join(tmpdir, "output")
	f, err := os.OpenFile(output, os.O_CREATE|os.O_WRONLY|os.O_SYNC, testFileMode)
	assert.NoError(err)
	defer f.Close()

	defaultOutputFile = f

	setCLIGlobals()

	_ = beforeSubcommands(ctx)
	assert.Equal(exitStatus, 0)

	text, err := katautils.GetFileContents(output)
	assert.NoError(err)

	lines := strings.Split(text, "\n")

	// Remove last line if empty
	length := len(lines)
	last := lines[length-1]
	if last == "" {
		lines = lines[:length-1]
	}

	assert.Equal(len(lines), 2)

	for i, line := range lines {
		switch i {
		case 0:
			assert.Equal(line, katautils.DEFAULTSYSCONFRUNTIMECONFIGURATION)
		case 1:
			assert.Equal(line, katautils.DEFAULTRUNTIMECONFIGURATION)
		}
	}
}

func TestMainFatal(t *testing.T) {
	assert := assert.New(t)

	tmpdir := t.TempDir()

	var exitStatus int
	savedExitFunc := exitFunc

	exitFunc = func(status int) { exitStatus = status }

	savedErrorFile := defaultErrorFile

	output := filepath.Join(tmpdir, "output")
	f, err := os.OpenFile(output, os.O_CREATE|os.O_WRONLY|os.O_SYNC, testFileMode)
	assert.NoError(err)
	defaultErrorFile = f

	defer func() {
		f.Close()
		defaultErrorFile = savedErrorFile
		exitFunc = savedExitFunc
	}()

	exitError := errors.New("hello world")

	fatal(exitError)
	assert.Equal(exitStatus, 1)

	text, err := katautils.GetFileContents(output)
	assert.NoError(err)

	trimmed := strings.TrimSpace(text)
	assert.Equal(exitError.Error(), trimmed)
}

func testVersionString(assert *assert.Assertions, versionString, expectedVersion, expectedCommit, expectedOCIVersion string) {
	foundVersion := false
	foundCommit := false
	foundOCIVersion := false

	versionRE := regexp.MustCompile(fmt.Sprintf(`%s\s*:\s*%v`, katautils.NAME, expectedVersion))
	commitRE := regexp.MustCompile(fmt.Sprintf(`%s\s*:\s*%v`, "commit", expectedCommit))

	ociRE := regexp.MustCompile(fmt.Sprintf(`%s\s*:\s*%v`, "OCI specs", regexp.QuoteMeta(expectedOCIVersion)))

	lines := strings.Split(versionString, "\n")
	assert.True(len(lines) > 0)

	for _, line := range lines {
		vMatches := versionRE.FindAllStringSubmatch(line, -1)
		if vMatches != nil {
			foundVersion = true
		}

		cMatches := commitRE.FindAllStringSubmatch(line, -1)
		if cMatches != nil {
			foundCommit = true
		}

		oMatches := ociRE.FindAllStringSubmatch(line, -1)
		if oMatches != nil {
			foundOCIVersion = true
		}
	}

	args := fmt.Sprintf("versionString: %q, expectedVersion: %q, expectedCommit: %v, expectedOCIVersion: %v\n",
		versionString, expectedVersion, expectedCommit, expectedOCIVersion)

	assert.True(foundVersion, args)
	assert.True(foundCommit, args)
	assert.True(foundOCIVersion, args)
}

func TestMainMakeVersionString(t *testing.T) {
	assert := assert.New(t)

	v := makeVersionString()

	testVersionString(assert, v, katautils.VERSION, katautils.COMMIT, specs.Version)
}

func TestMainMakeVersionStringNoVersion(t *testing.T) {
	assert := assert.New(t)

	savedVersion := katautils.VERSION
	katautils.VERSION = ""

	defer func() {
		katautils.VERSION = savedVersion
	}()

	v := makeVersionString()

	testVersionString(assert, v, unknown, katautils.COMMIT, specs.Version)
}

func TestMainMakeVersionStringNoCommit(t *testing.T) {
	assert := assert.New(t)

	savedCommit := katautils.COMMIT
	katautils.COMMIT = ""

	defer func() {
		katautils.COMMIT = savedCommit
	}()

	v := makeVersionString()

	testVersionString(assert, v, katautils.VERSION, unknown, specs.Version)
}

func TestMainMakeVersionStringNoOCIVersion(t *testing.T) {
	assert := assert.New(t)

	savedVersion := specs.Version
	specs.Version = ""

	defer func() {
		specs.Version = savedVersion
	}()

	v := makeVersionString()

	testVersionString(assert, v, katautils.VERSION, katautils.COMMIT, unknown)
}

func TestMainCreateRuntimeApp(t *testing.T) {
	assert := assert.New(t)

	savedBefore := runtimeBeforeSubcommands
	savedOutputFile := defaultOutputFile

	// disable
	runtimeBeforeSubcommands = nil

	devNull, err := os.OpenFile("/dev/null", os.O_RDWR, 0640)
	assert.NoError(err)
	defer devNull.Close()

	defaultOutputFile = devNull

	setCLIGlobals()

	defer func() {
		resetCLIGlobals()
		runtimeBeforeSubcommands = savedBefore
		defaultOutputFile = savedOutputFile
	}()

	args := []string{katautils.NAME}

	err = createRuntimeApp(context.Background(), args)
	assert.NoError(err, "%v", args)
}

func TestMainCreateRuntimeAppInvalidSubCommand(t *testing.T) {
	assert := assert.New(t)

	exitStatus := 0

	savedBefore := runtimeBeforeSubcommands
	savedExitFunc := exitFunc

	exitFunc = func(status int) { exitStatus = status }

	// disable
	runtimeBeforeSubcommands = nil

	defer func() {
		runtimeBeforeSubcommands = savedBefore
		exitFunc = savedExitFunc
	}()

	// calls fatal() so no return
	_ = createRuntimeApp(context.Background(), []string{katautils.NAME, "i-am-an-invalid-sub-command"})

	assert.NotEqual(exitStatus, 0)
}

func TestMainCreateRuntime(t *testing.T) {
	assert := assert.New(t)

	const cmd = "foo"
	const msg = "moo message"

	resetCLIGlobals()

	exitStatus := 0

	savedOSArgs := os.Args
	savedExitFunc := exitFunc
	savedBefore := runtimeBeforeSubcommands
	savedCommands := runtimeCommands

	os.Args = []string{katautils.NAME, cmd}
	exitFunc = func(status int) { exitStatus = status }

	// disable
	runtimeBeforeSubcommands = nil

	// override sub-commands
	runtimeCommands = []cli.Command{
		{
			Name: cmd,
			Action: func(context *cli.Context) error {
				return errors.New(msg)
			},
		},
	}

	defer func() {
		os.Args = savedOSArgs
		exitFunc = savedExitFunc
		runtimeBeforeSubcommands = savedBefore
		runtimeCommands = savedCommands
	}()

	assert.Equal(exitStatus, 0)
	createRuntime(context.Background())
	assert.NotEqual(exitStatus, 0)
}

func TestMainVersionPrinter(t *testing.T) {
	assert := assert.New(t)

	tmpdir := t.TempDir()

	savedOutputFile := defaultOutputFile

	defer func() {
		resetCLIGlobals()
		defaultOutputFile = savedOutputFile
	}()

	output := filepath.Join(tmpdir, "output")
	f, err := os.OpenFile(output, os.O_CREATE|os.O_WRONLY|os.O_SYNC, testFileMode)
	assert.NoError(err)
	defer f.Close()

	defaultOutputFile = f

	setCLIGlobals()

	err = createRuntimeApp(context.Background(), []string{katautils.NAME, "--version"})
	assert.NoError(err)

	err = grep(fmt.Sprintf(`%s\s*:\s*%s`, katautils.NAME, katautils.VERSION), output)
	assert.NoError(err)
}

func TestMainFatalWriter(t *testing.T) {
	assert := assert.New(t)

	const cmd = "foo"
	const msg = "moo message"

	// create buffer to save logger output
	buf := &bytes.Buffer{}

	savedBefore := runtimeBeforeSubcommands
	savedLogOutput := kataLog.Logger.Out
	savedCLIExiter := cli.OsExiter
	savedCommands := runtimeCommands

	// disable
	runtimeBeforeSubcommands = nil

	// save all output
	kataLog.Logger.Out = buf

	cli.OsExiter = func(status int) {}

	// override sub-commands
	runtimeCommands = []cli.Command{
		{
			Name: cmd,
			Action: func(context *cli.Context) error {
				return cli.NewExitError(msg, 42)
			},
		},
	}

	defer func() {
		runtimeBeforeSubcommands = savedBefore
		kataLog.Logger.Out = savedLogOutput
		cli.OsExiter = savedCLIExiter
		runtimeCommands = savedCommands
	}()

	setCLIGlobals()

	err := createRuntimeApp(context.Background(), []string{katautils.NAME, cmd})
	assert.Error(err)

	re := regexp.MustCompile(
		fmt.Sprintf(`\blevel\b.*\berror\b.*\b%s\b`, msg))
	matches := re.FindAllStringSubmatch(buf.String(), -1)
	assert.NotEmpty(matches)
}

func TestMainSetCLIGlobals(t *testing.T) {
	assert := assert.New(t)

	defer resetCLIGlobals()

	cli.AppHelpTemplate = ""
	cli.VersionPrinter = nil
	cli.ErrWriter = nil

	setCLIGlobals()

	assert.NotEqual(cli.AppHelpTemplate, "")
	assert.NotNil(cli.VersionPrinter)
	assert.NotNil(cli.ErrWriter)
}

func TestMainResetCLIGlobals(t *testing.T) {
	assert := assert.New(t)

	assert.NotEqual(cli.AppHelpTemplate, "")
	assert.NotNil(savedCLIVersionPrinter)
	assert.NotNil(savedCLIErrWriter)

	cli.AppHelpTemplate = ""
	cli.VersionPrinter = nil
	cli.ErrWriter = nil

	resetCLIGlobals()

	assert.Equal(cli.AppHelpTemplate, savedCLIAppHelpTemplate)
	assert.NotNil(cli.VersionPrinter)
	assert.NotNil(savedCLIVersionPrinter)
}
