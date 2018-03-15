//
// Copyright (c) 2017 Intel Corporation
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
//

package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"regexp"
	"strings"
	"testing"

	"github.com/dlespiau/covertool/pkg/cover"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcMock"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

const (
	testDisabledNeedRoot    = "Test disabled as requires root user"
	testDisabledNeedNonRoot = "Test disabled as requires non-root user"
	testDirMode             = os.FileMode(0750)
	testFileMode            = os.FileMode(0640)
	testExeFileMode         = os.FileMode(0750)

	// small docker image used to create root filesystems from
	testDockerImage = "busybox"

	testPodID       = "99999999-9999-9999-99999999999999999"
	testContainerID = "1"
	testBundle      = "bundle"
)

var (
	// package variables set by calling TestMain()
	testDir       = ""
	testBundleDir = ""
)

// testingImpl is a concrete mock RVC implementation used for testing
var testingImpl = &vcMock.VCMock{}

func init() {
	if version == "" {
		panic("ERROR: invalid build: version not set")
	}

	if commit == "" {
		panic("ERROR: invalid build: commit not set")
	}

	if defaultSysConfRuntimeConfiguration == "" {
		panic("ERROR: invalid build: defaultSysConfRuntimeConfiguration not set")
	}

	if defaultRuntimeConfiguration == "" {
		panic("ERROR: invalid build: defaultRuntimeConfiguration not set")
	}

	fmt.Printf("INFO: running as actual user %v (effective %v), actual group %v (effective %v)\n",
		os.Getuid(), os.Geteuid(), os.Getgid(), os.Getegid())

	fmt.Printf("INFO: switching to fake virtcontainers implementation for testing\n")
	vci = testingImpl

	var err error

	fmt.Printf("INFO: creating test directory\n")
	testDir, err = ioutil.TempDir("", fmt.Sprintf("%s-", name))
	if err != nil {
		panic(fmt.Sprintf("ERROR: failed to create test directory: %v", err))
	}

	fmt.Printf("INFO: test directory is %v\n", testDir)

	fmt.Printf("INFO: ensuring docker is running\n")
	output, err := runCommandFull([]string{"docker", "version"}, true)
	if err != nil {
		panic(fmt.Sprintf("ERROR: docker daemon is not installed, not running, or not accessible to current user: %v (error %v)",
			output, err))
	}

	// Do this now to avoid hitting the test timeout value due to
	// slow network response.
	fmt.Printf("INFO: ensuring required docker image (%v) is available\n", testDockerImage)

	// Only hit the network if the image doesn't exist locally
	_, err = runCommand([]string{"docker", "image", "inspect", testDockerImage})
	if err == nil {
		fmt.Printf("INFO: docker image %v already exists locally\n", testDockerImage)
	} else {
		_, err = runCommand([]string{"docker", "pull", testDockerImage})
		if err != nil {
			panic(err)
		}
	}

	testBundleDir = filepath.Join(testDir, testBundle)
	err = os.MkdirAll(testBundleDir, testDirMode)
	if err != nil {
		panic(fmt.Sprintf("ERROR: failed to create bundle directory %v: %v", testBundleDir, err))
	}

	fmt.Printf("INFO: creating OCI bundle in %v for tests to use\n", testBundleDir)
	err = realMakeOCIBundle(testBundleDir)
	if err != nil {
		panic(fmt.Sprintf("ERROR: failed to create OCI bundle: %v", err))
	}
}

// resetCLIGlobals undoes the effects of setCLIGlobals(), restoring the original values
func resetCLIGlobals() {
	cli.AppHelpTemplate = savedCLIAppHelpTemplate
	cli.VersionPrinter = savedCLIVersionPrinter
	cli.ErrWriter = savedCLIErrWriter
}

func runUnitTests(m *testing.M) {
	ret := m.Run()

	os.RemoveAll(testDir)

	os.Exit(ret)
}

// Read fail that should contain a CompatOCISpec and
// return its JSON representation on success
func readOCIConfigJSON(configFile string) (string, error) {
	bundlePath := filepath.Dir(configFile)
	ociSpec, err := oci.ParseConfigJSON(bundlePath)
	if err != nil {
		return "", nil
	}
	ociSpecJSON, err := json.Marshal(ociSpec)
	if err != nil {
		return "", err
	}
	return string(ociSpecJSON), err
}

// TestMain is the common main function used by ALL the test functions
// for this package.
func TestMain(m *testing.M) {
	// Parse the command line using the stdlib flag package so the flags defined
	// in the testing package get populated.
	cover.ParseAndStripTestFlags()

	// Make sure we have the opportunity to flush the coverage report to disk when
	// terminating the process.
	atexit(cover.FlushProfiles)

	// If the test binary name is kata-runtime.coverage, we've are being asked to
	// run the coverage-instrumented kata-runtime.
	if path.Base(os.Args[0]) == name+".coverage" ||
		path.Base(os.Args[0]) == name {
		main()
		exit(0)
	}

	runUnitTests(m)
}

func createEmptyFile(path string) (err error) {
	return ioutil.WriteFile(path, []byte(""), testFileMode)
}

func grep(pattern, file string) error {
	if file == "" {
		return errors.New("need file")
	}

	bytes, err := ioutil.ReadFile(file)
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
		HypervisorMachineType: "pc-lite",
	}, nil
}

// newTestRuntimeConfig creates a new RuntimeConfig
func newTestRuntimeConfig(dir, consolePath string, create bool) (oci.RuntimeConfig, error) {
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
		AgentType:        vc.KataContainersAgent,
		ProxyType:        vc.CCProxyType,
		ShimType:         vc.CCShimType,
		Console:          consolePath,
	}, nil
}

// createOCIConfig creates an OCI configuration (spec) file in
// the bundle directory specified (which must exist).
func createOCIConfig(bundleDir string) error {
	if bundleDir == "" {
		return errors.New("BUG: Need bundle directory")
	}

	if !fileExists(bundleDir) {
		return fmt.Errorf("BUG: Bundle directory %s does not exist", bundleDir)
	}

	var configCmd string

	// Search for a suitable version of runc to use to generate
	// the OCI config file.
	for _, cmd := range []string{"docker-runc", "runc"} {
		fullPath, err := exec.LookPath(cmd)
		if err == nil {
			configCmd = fullPath
			break
		}
	}

	if configCmd == "" {
		return errors.New("Cannot find command to generate OCI config file")
	}

	_, err := runCommand([]string{configCmd, "spec", "--bundle", bundleDir})
	if err != nil {
		return err
	}

	specFile := filepath.Join(bundleDir, specConfig)
	if !fileExists(specFile) {
		return fmt.Errorf("generated OCI config file does not exist: %v", specFile)
	}

	return nil
}

// createRootfs creates a minimal root filesystem below the specified
// directory.
func createRootfs(dir string) error {
	err := os.MkdirAll(dir, testDirMode)
	if err != nil {
		return err
	}

	container, err := runCommand([]string{"docker", "create", testDockerImage})
	if err != nil {
		return err
	}

	cmd1 := exec.Command("docker", "export", container)
	cmd2 := exec.Command("tar", "-C", dir, "-xvf", "-")

	cmd1Stdout, err := cmd1.StdoutPipe()
	if err != nil {
		return err
	}

	cmd2.Stdin = cmd1Stdout

	err = cmd2.Start()
	if err != nil {
		return err
	}

	err = cmd1.Run()
	if err != nil {
		return err
	}

	err = cmd2.Wait()
	if err != nil {
		return err
	}

	// Clean up
	_, err = runCommand([]string{"docker", "rm", container})
	if err != nil {
		return err
	}

	return nil
}

// realMakeOCIBundle will create an OCI bundle (including the "config.json"
// config file) in the directory specified (which must already exist).
//
// XXX: Note that tests should *NOT* call this function - they should
// XXX: instead call makeOCIBundle().
func realMakeOCIBundle(bundleDir string) error {
	if bundleDir == "" {
		return errors.New("BUG: Need bundle directory")
	}

	if !fileExists(bundleDir) {
		return fmt.Errorf("BUG: Bundle directory %v does not exist", bundleDir)
	}

	err := createOCIConfig(bundleDir)
	if err != nil {
		return err
	}

	// Note the unusual parameter (a directory, not the config
	// file to parse!)
	spec, err := oci.ParseConfigJSON(bundleDir)
	if err != nil {
		return err
	}

	// Determine the rootfs directory name the OCI config refers to
	ociRootPath := spec.Root.Path

	rootfsDir := filepath.Join(bundleDir, ociRootPath)

	if strings.HasPrefix(ociRootPath, "/") {
		return fmt.Errorf("Cannot handle absolute rootfs as bundle must be unique to each test")
	}

	err = createRootfs(rootfsDir)
	if err != nil {
		return err
	}

	return nil
}

// Create an OCI bundle in the specified directory.
//
// Note that the directory will be created, but it's parent is expected to exist.
//
// This function works by copying the already-created test bundle. Ideally,
// the bundle would be recreated for each test, but createRootfs() uses
// docker which on some systems is too slow, resulting in the tests timing
// out.
func makeOCIBundle(bundleDir string) error {
	from := testBundleDir
	to := bundleDir

	// only the basename of bundleDir needs to exist as bundleDir
	// will get created by cp(1).
	base := filepath.Dir(bundleDir)

	for _, dir := range []string{from, base} {
		if !fileExists(dir) {
			return fmt.Errorf("BUG: directory %v should exist", dir)
		}
	}

	output, err := runCommandFull([]string{"cp", "-a", from, to}, true)
	if err != nil {
		return fmt.Errorf("failed to copy test OCI bundle from %v to %v: %v (output: %v)", from, to, err, output)
	}

	return nil
}

// readOCIConfig returns an OCI spec.
func readOCIConfigFile(configPath string) (oci.CompatOCISpec, error) {
	if configPath == "" {
		return oci.CompatOCISpec{}, errors.New("BUG: need config file path")
	}

	data, err := ioutil.ReadFile(configPath)
	if err != nil {
		return oci.CompatOCISpec{}, err
	}

	var ociSpec oci.CompatOCISpec
	if err := json.Unmarshal(data, &ociSpec); err != nil {
		return oci.CompatOCISpec{}, err
	}

	return ociSpec, nil
}

func writeOCIConfigFile(spec oci.CompatOCISpec, configPath string) error {
	if configPath == "" {
		return errors.New("BUG: need config file path")
	}

	bytes, err := json.MarshalIndent(spec, "", "\t")
	if err != nil {
		return err
	}

	return ioutil.WriteFile(configPath, bytes, testFileMode)
}

func newSingleContainerPodStatusList(podID, containerID string, podState, containerState vc.State, annotations map[string]string) []vc.PodStatus {
	return []vc.PodStatus{
		{
			ID:    podID,
			State: podState,
			ContainersStatus: []vc.ContainerStatus{
				{
					ID:          containerID,
					State:       containerState,
					Annotations: annotations,
				},
			},
		},
	}
}

func execCLICommandFunc(assertHandler *assert.Assertions, cliCommand cli.Command, set *flag.FlagSet, expectedErr bool) {
	app := cli.NewApp()
	ctx := cli.NewContext(app, set, nil)
	app.Name = "foo"

	fn, ok := cliCommand.Action.(func(context *cli.Context) error)
	assertHandler.True(ok)

	err := fn(ctx)

	if expectedErr {
		assertHandler.Error(err)
	} else {
		assertHandler.Nil(err)
	}
}

func TestMakeOCIBundle(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundleDir := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundleDir)
	assert.NoError(err)

	specFile := filepath.Join(bundleDir, specConfig)
	assert.True(fileExists(specFile))
}

func TestCreateOCIConfig(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundleDir := filepath.Join(tmpdir, "bundle")

	err = createOCIConfig(bundleDir)
	// ENOENT
	assert.Error(err)

	err = os.MkdirAll(bundleDir, testDirMode)
	assert.NoError(err)

	err = createOCIConfig(bundleDir)
	assert.NoError(err)

	specFile := filepath.Join(bundleDir, specConfig)
	assert.True(fileExists(specFile))
}

func TestCreateRootfs(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	rootfsDir := filepath.Join(tmpdir, "rootfs")
	assert.False(fileExists(rootfsDir))

	err = createRootfs(rootfsDir)
	assert.NoError(err)

	// non-comprehensive list of expected directories
	expectedDirs := []string{"bin", "dev", "etc", "usr", "var"}

	assert.True(fileExists(rootfsDir))

	for _, dir := range expectedDirs {
		dirPath := filepath.Join(rootfsDir, dir)
		assert.True(fileExists(dirPath))
	}
}

func TestMainUserWantsUsage(t *testing.T) {
	assert := assert.New(t)
	app := cli.NewApp()

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

		ctx := cli.NewContext(app, set, nil)
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
	app := cli.NewApp()

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
		{[]string{"kata-check"}, false},
	}

	for i, d := range data {
		set := flag.NewFlagSet("", 0)
		set.Parse(d.arguments)

		ctx := cli.NewContext(app, set, nil)
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

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	logFile := filepath.Join(tmpdir, "log")

	// create the file as the wrong type to force a failure
	err = os.MkdirAll(logFile, testDirMode)
	assert.NoError(err)

	app := cli.NewApp()

	set := flag.NewFlagSet("", 0)
	set.String("log", logFile, "")
	set.Parse([]string{"create"})

	ctx := cli.NewContext(app, set, nil)

	err = beforeSubcommands(ctx)
	assert.Error(err)
}

func TestMainBeforeSubCommandsInvalidLogFormat(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	logFile := filepath.Join(tmpdir, "log")

	app := cli.NewApp()

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

	ctx := cli.NewContext(app, set, nil)

	err = beforeSubcommands(ctx)
	assert.Error(err)
	assert.NotNil(kataLog.Logger.Out)
}

func TestMainBeforeSubCommandsLoadConfigurationFail(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	logFile := filepath.Join(tmpdir, "log")
	configFile := filepath.Join(tmpdir, "config")

	app := cli.NewApp()

	for _, logFormat := range []string{"json", "text"} {
		set := flag.NewFlagSet("", 0)
		set.Bool("debug", true, "")
		set.String("log", logFile, "")
		set.String("log-format", logFormat, "")
		set.String("kata-config", configFile, "")
		set.Parse([]string{"kata-env"})

		ctx := cli.NewContext(app, set, nil)

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

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	app := cli.NewApp()

	set := flag.NewFlagSet("", 0)
	set.Bool("kata-show-default-config-paths", true, "")

	ctx := cli.NewContext(app, set, nil)

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

	text, err := getFileContents(output)
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
			assert.Equal(line, defaultSysConfRuntimeConfiguration)
		case 1:
			assert.Equal(line, defaultRuntimeConfiguration)
		}
	}
}

func TestMainFatal(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

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

	text, err := getFileContents(output)
	assert.NoError(err)

	trimmed := strings.TrimSpace(text)
	assert.Equal(exitError.Error(), trimmed)
}

func testVersionString(assert *assert.Assertions, versionString, expectedVersion, expectedCommit, expectedOCIVersion string) {
	foundVersion := false
	foundCommit := false
	foundOCIVersion := false

	versionRE := regexp.MustCompile(fmt.Sprintf(`%s\s*:\s*%v`, name, expectedVersion))
	commitRE := regexp.MustCompile(fmt.Sprintf(`%s\s*:\s*%v`, "commit", expectedCommit))

	ociRE := regexp.MustCompile(fmt.Sprintf(`%s\s*:\s*%v`, "OCI specs", expectedOCIVersion))

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

	testVersionString(assert, v, version, commit, specs.Version)
}

func TestMainMakeVersionStringNoVersion(t *testing.T) {
	assert := assert.New(t)

	savedVersion := version
	version = ""

	defer func() {
		version = savedVersion
	}()

	v := makeVersionString()

	testVersionString(assert, v, unknown, commit, specs.Version)
}

func TestMainMakeVersionStringNoCommit(t *testing.T) {
	assert := assert.New(t)

	savedCommit := commit
	commit = ""

	defer func() {
		commit = savedCommit
	}()

	v := makeVersionString()

	testVersionString(assert, v, version, unknown, specs.Version)
}

func TestMainMakeVersionStringNoOCIVersion(t *testing.T) {
	assert := assert.New(t)

	savedVersion := specs.Version
	specs.Version = ""

	defer func() {
		specs.Version = savedVersion
	}()

	v := makeVersionString()

	testVersionString(assert, v, version, commit, unknown)
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

	args := []string{name}

	err = createRuntimeApp(args)
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
	_ = createRuntimeApp([]string{name, "i-am-an-invalid-sub-command"})

	assert.NotEqual(exitStatus, 0)
}

func TestMainCreateRuntime(t *testing.T) {
	assert := assert.New(t)

	const cmd = "foo"
	const msg = "moo FAILURE"

	resetCLIGlobals()

	exitStatus := 0

	savedOSArgs := os.Args
	savedExitFunc := exitFunc
	savedBefore := runtimeBeforeSubcommands
	savedCommands := runtimeCommands

	os.Args = []string{name, cmd}
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
	createRuntime()
	assert.NotEqual(exitStatus, 0)
}

func TestMainVersionPrinter(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

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

	err = createRuntimeApp([]string{name, "--version"})
	assert.NoError(err)

	err = grep(fmt.Sprintf(`%s\s*:\s*%s`, name, version), output)
	assert.NoError(err)
}

func TestMainFatalWriter(t *testing.T) {
	assert := assert.New(t)

	const cmd = "foo"
	const msg = "moo FAILURE"

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

	err := createRuntimeApp([]string{name, cmd})
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
