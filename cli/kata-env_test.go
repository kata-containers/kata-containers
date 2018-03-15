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

package main

import (
	"bytes"
	"fmt"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	goruntime "runtime"
	"strings"
	"testing"

	"github.com/BurntSushi/toml"
	vc "github.com/kata-containers/runtime/virtcontainers"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/urfave/cli"

	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/stretchr/testify/assert"
)

const testProxyURL = "file:///proxyURL"
const testProxyVersion = "proxy version 0.1"
const testShimVersion = "shim version 0.1"
const testHypervisorVersion = "QEMU emulator version 2.7.0+git.741f430a96-6.1, Copyright (c) 2003-2016 Fabrice Bellard and the QEMU Project developers"

// makeVersionBinary creates a shell script with the specified file
// name. When run as "file --version", it will display the specified
// version to stdout and exit successfully.
func makeVersionBinary(file, version string) error {
	err := createFile(file,
		fmt.Sprintf(`#!/bin/sh
	[ "$1" = "--version" ] && echo "%s"`, version))
	if err != nil {
		return err
	}

	err = os.Chmod(file, testExeFileMode)
	if err != nil {
		return err
	}

	return nil
}

func makeRuntimeConfig(prefixDir string) (configFile string, config oci.RuntimeConfig, err error) {
	const logPath = "/log/path"
	hypervisorPath := filepath.Join(prefixDir, "hypervisor")
	kernelPath := filepath.Join(prefixDir, "kernel")
	imagePath := filepath.Join(prefixDir, "image")
	kernelParams := "foo=bar xyz"
	machineType := "machineType"
	shimPath := filepath.Join(prefixDir, "shim")
	proxyPath := filepath.Join(prefixDir, "proxy")
	disableBlock := true
	blockStorageDriver := "virtio-scsi"

	// override
	defaultProxyPath = proxyPath

	filesToCreate := []string{
		hypervisorPath,
		kernelPath,
		imagePath,
	}

	for _, file := range filesToCreate {
		err := createEmptyFile(file)
		if err != nil {
			return "", oci.RuntimeConfig{}, err
		}
	}

	err = makeVersionBinary(shimPath, testShimVersion)
	if err != nil {
		return "", oci.RuntimeConfig{}, err
	}

	err = makeVersionBinary(proxyPath, testProxyVersion)
	if err != nil {
		return "", oci.RuntimeConfig{}, err
	}

	err = makeVersionBinary(hypervisorPath, testHypervisorVersion)
	if err != nil {
		return "", oci.RuntimeConfig{}, err
	}

	runtimeConfig := makeRuntimeConfigFileData(
		"qemu",
		hypervisorPath,
		kernelPath,
		imagePath,
		kernelParams,
		machineType,
		shimPath,
		testProxyURL,
		logPath,
		disableBlock,
		blockStorageDriver)

	configFile = path.Join(prefixDir, "runtime.toml")
	err = createConfig(configFile, runtimeConfig)
	if err != nil {
		return "", oci.RuntimeConfig{}, err
	}

	_, config, err = loadConfiguration(configFile, true)
	if err != nil {
		return "", oci.RuntimeConfig{}, err
	}

	return configFile, config, nil
}

func getExpectedProxyDetails(config oci.RuntimeConfig) (ProxyInfo, error) {
	return ProxyInfo{
		Type:    string(config.ProxyType),
		Version: testProxyVersion,
		Path:    config.ProxyConfig.Path,
		Debug:   config.ProxyConfig.Debug,
	}, nil
}

func getExpectedShimDetails(config oci.RuntimeConfig) (ShimInfo, error) {
	shimConfig, ok := config.ShimConfig.(vc.ShimConfig)
	if !ok {
		return ShimInfo{}, fmt.Errorf("failed to get shim config")
	}

	shimPath := shimConfig.Path

	return ShimInfo{
		Type:    string(config.ShimType),
		Version: testShimVersion,
		Path:    shimPath,
	}, nil
}

func getExpectedAgentDetails(config oci.RuntimeConfig) (AgentInfo, error) {
	return AgentInfo{
		Type:    string(config.AgentType),
		Version: unknown,
	}, nil
}

func getExpectedHostDetails(tmpdir string) (HostInfo, error) {
	type filesToCreate struct {
		file     string
		contents string
	}

	const expectedKernelVersion = "99.1"
	const expectedArch = goruntime.GOARCH

	expectedDistro := DistroInfo{
		Name:    "Foo",
		Version: "42",
	}

	expectedCPU := CPUInfo{
		Vendor: "moi",
		Model:  "awesome XI",
	}

	expectedHostDetails := HostInfo{
		Kernel:             expectedKernelVersion,
		Architecture:       expectedArch,
		Distro:             expectedDistro,
		CPU:                expectedCPU,
		VMContainerCapable: false,
	}

	testProcCPUInfo := filepath.Join(tmpdir, "cpuinfo")
	testOSRelease := filepath.Join(tmpdir, "os-release")

	// XXX: This file is *NOT* created by this function on purpose
	// (to ensure the only file checked by the tests is
	// testOSRelease). osReleaseClr handling is tested in
	// utils_test.go.
	testOSReleaseClr := filepath.Join(tmpdir, "os-release-clr")

	testProcVersion := filepath.Join(tmpdir, "proc-version")

	// override
	procVersion = testProcVersion
	osRelease = testOSRelease
	osReleaseClr = testOSReleaseClr
	procCPUInfo = testProcCPUInfo

	procVersionContents := fmt.Sprintf("Linux version %s a b c",
		expectedKernelVersion)

	osReleaseContents := fmt.Sprintf(`
NAME="%s"
VERSION_ID="%s"
`, expectedDistro.Name, expectedDistro.Version)

	procCPUInfoContents := fmt.Sprintf(`
vendor_id	: %s
model name	: %s
`, expectedCPU.Vendor, expectedCPU.Model)

	data := []filesToCreate{
		{procVersion, procVersionContents},
		{osRelease, osReleaseContents},
		{procCPUInfo, procCPUInfoContents},
	}

	for _, d := range data {
		err := createFile(d.file, d.contents)
		if err != nil {
			return HostInfo{}, err
		}
	}

	return expectedHostDetails, nil
}

func getExpectedHypervisor(config oci.RuntimeConfig) HypervisorInfo {
	return HypervisorInfo{
		Version:           testHypervisorVersion,
		Path:              config.HypervisorConfig.HypervisorPath,
		MachineType:       config.HypervisorConfig.HypervisorMachineType,
		BlockDeviceDriver: config.HypervisorConfig.BlockDeviceDriver,
	}
}

func getExpectedImage(config oci.RuntimeConfig) ImageInfo {
	return ImageInfo{
		Path: config.HypervisorConfig.ImagePath,
	}
}

func getExpectedKernel(config oci.RuntimeConfig) KernelInfo {
	return KernelInfo{
		Path:       config.HypervisorConfig.KernelPath,
		Parameters: strings.Join(vc.SerializeParams(config.HypervisorConfig.KernelParams, "="), " "),
	}
}

func getExpectedRuntimeDetails(configFile string) RuntimeInfo {
	return RuntimeInfo{
		Version: RuntimeVersionInfo{
			Semver: version,
			Commit: commit,
			OCI:    specs.Version,
		},
		Config: RuntimeConfigInfo{
			Path: configFile,
		},
	}
}

func getExpectedSettings(config oci.RuntimeConfig, tmpdir, configFile string) (EnvInfo, error) {
	meta := getExpectedMetaInfo()

	runtime := getExpectedRuntimeDetails(configFile)

	proxy, err := getExpectedProxyDetails(config)
	if err != nil {
		return EnvInfo{}, err
	}

	shim, err := getExpectedShimDetails(config)
	if err != nil {
		return EnvInfo{}, err
	}

	agent, err := getExpectedAgentDetails(config)
	if err != nil {
		return EnvInfo{}, err
	}

	host, err := getExpectedHostDetails(tmpdir)
	if err != nil {
		return EnvInfo{}, err
	}

	hypervisor := getExpectedHypervisor(config)
	kernel := getExpectedKernel(config)
	image := getExpectedImage(config)

	env := EnvInfo{
		Meta:       meta,
		Runtime:    runtime,
		Hypervisor: hypervisor,
		Image:      image,
		Kernel:     kernel,
		Proxy:      proxy,
		Shim:       shim,
		Agent:      agent,
		Host:       host,
	}

	return env, nil
}

func getExpectedMetaInfo() MetaInfo {
	return MetaInfo{
		Version: formatVersion,
	}
}

func TestEnvGetMetaInfo(t *testing.T) {
	expectedMeta := getExpectedMetaInfo()

	meta := getMetaInfo()

	assert.Equal(t, expectedMeta, meta)
}

func TestEnvGetHostInfo(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	expectedHostDetails, err := getExpectedHostDetails(tmpdir)
	assert.NoError(t, err)

	host, err := getHostInfo()
	assert.NoError(t, err)

	assert.Equal(t, expectedHostDetails, host)
}

func TestEnvGetHostInfoNoProcCPUInfo(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	_, err = getExpectedHostDetails(tmpdir)
	assert.NoError(t, err)

	err = os.Remove(procCPUInfo)
	assert.NoError(t, err)

	_, err = getHostInfo()
	assert.Error(t, err)
}

func TestEnvGetHostInfoNoOSRelease(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	_, err = getExpectedHostDetails(tmpdir)
	assert.NoError(t, err)

	err = os.Remove(osRelease)
	assert.NoError(t, err)

	_, err = getHostInfo()
	assert.Error(t, err)
}

func TestEnvGetHostInfoNoProcVersion(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	_, err = getExpectedHostDetails(tmpdir)
	assert.NoError(t, err)

	err = os.Remove(procVersion)
	assert.NoError(t, err)

	_, err = getHostInfo()
	assert.Error(t, err)
}

func TestEnvGetEnvInfo(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	expectedEnv, err := getExpectedSettings(config, tmpdir, configFile)
	assert.NoError(t, err)

	env, err := getEnvInfo(configFile, config)
	assert.NoError(t, err)

	assert.Equal(t, expectedEnv, env)
}

func TestEnvGetEnvInfoNoHypervisorVersion(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(err)

	expectedEnv, err := getExpectedSettings(config, tmpdir, configFile)
	assert.NoError(err)

	err = os.Remove(config.HypervisorConfig.HypervisorPath)
	assert.NoError(err)

	expectedEnv.Hypervisor.Version = unknown

	env, err := getEnvInfo(configFile, config)
	assert.NoError(err)

	assert.Equal(expectedEnv, env)
}

func TestEnvGetEnvInfoShimError(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(err)

	config.ShimConfig = "invalid shim config"

	_, err = getEnvInfo(configFile, config)
	assert.Error(err)
}

func TestEnvGetEnvInfoAgentError(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(err)

	config.AgentConfig = "invalid agent config"

	_, err = getEnvInfo(configFile, config)
	assert.Error(err)
}

func TestEnvGetEnvInfoNoOSRelease(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	_, err = getExpectedSettings(config, tmpdir, configFile)
	assert.NoError(t, err)

	err = os.Remove(osRelease)
	assert.NoError(t, err)

	_, err = getEnvInfo(configFile, config)
	assert.Error(t, err)
}

func TestEnvGetEnvInfoNoProcCPUInfo(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	_, err = getExpectedSettings(config, tmpdir, configFile)
	assert.NoError(t, err)

	err = os.Remove(procCPUInfo)
	assert.NoError(t, err)

	_, err = getEnvInfo(configFile, config)
	assert.Error(t, err)
}

func TestEnvGetEnvInfoNoProcVersion(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	_, err = getExpectedSettings(config, tmpdir, configFile)
	assert.NoError(t, err)

	err = os.Remove(procVersion)
	assert.NoError(t, err)

	_, err = getEnvInfo(configFile, config)
	assert.Error(t, err)
}

func TestEnvGetRuntimeInfo(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	expectedRuntime := getExpectedRuntimeDetails(configFile)

	runtime := getRuntimeInfo(configFile, config)

	assert.Equal(t, expectedRuntime, runtime)
}

func TestEnvGetProxyInfo(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	expectedProxy, err := getExpectedProxyDetails(config)
	assert.NoError(t, err)

	proxy, err := getProxyInfo(config)
	assert.NoError(t, err)

	assert.Equal(t, expectedProxy, proxy)
}

func TestEnvGetProxyInfoNoVersion(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	expectedProxy, err := getExpectedProxyDetails(config)
	assert.NoError(t, err)

	// remove the proxy ensuring its version cannot be queried
	err = os.Remove(defaultProxyPath)
	assert.NoError(t, err)

	expectedProxy.Version = unknown

	proxy, err := getProxyInfo(config)
	assert.NoError(t, err)

	assert.Equal(t, expectedProxy, proxy)
}

func TestEnvGetShimInfo(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	expectedShim, err := getExpectedShimDetails(config)
	assert.NoError(t, err)

	shim, err := getShimInfo(config)
	assert.NoError(t, err)

	assert.Equal(t, expectedShim, shim)
}

func TestEnvGetShimInfoNoVersion(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	expectedShim, err := getExpectedShimDetails(config)
	assert.NoError(t, err)

	shimPath := expectedShim.Path

	// ensure querying the shim version fails
	err = createFile(shimPath, `#!/bin/sh
	exit 1`)
	assert.NoError(t, err)

	expectedShim.Version = unknown

	shim, err := getShimInfo(config)
	assert.NoError(t, err)

	assert.Equal(t, expectedShim, shim)
}

func TestEnvGetShimInfoInvalidType(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	_, err = getExpectedShimDetails(config)
	assert.NoError(t, err)

	config.ShimConfig = "foo"
	_, err = getShimInfo(config)
	assert.Error(t, err)
}

func TestEnvGetAgentInfo(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	expectedAgent, err := getExpectedAgentDetails(config)
	assert.NoError(t, err)

	agent := getAgentInfo(config)
	assert.Equal(t, expectedAgent, agent)
}

func testEnvShowSettings(t *testing.T, tmpdir string, tmpfile *os.File) error {

	runtime := RuntimeInfo{}

	hypervisor := HypervisorInfo{
		Path:        "/resolved/hypervisor/path",
		MachineType: "hypervisor-machine-type",
	}

	image := ImageInfo{
		Path: "/resolved/image/path",
	}

	kernel := KernelInfo{
		Path:       "/kernel/path",
		Parameters: "foo=bar xyz",
	}

	proxy := ProxyInfo{
		Type:    "proxy-type",
		Version: "proxy-version",
		Path:    "file:///proxy-url",
		Debug:   false,
	}

	shim := ShimInfo{
		Type:    "shim-type",
		Version: "shim-version",
		Path:    "/resolved/shim/path",
	}

	agent := AgentInfo{
		Type:    "agent-type",
		Version: "agent-version",
	}

	expectedHostDetails, err := getExpectedHostDetails(tmpdir)
	assert.NoError(t, err)

	env := EnvInfo{
		Runtime:    runtime,
		Hypervisor: hypervisor,
		Image:      image,
		Kernel:     kernel,
		Proxy:      proxy,
		Shim:       shim,
		Agent:      agent,
		Host:       expectedHostDetails,
	}

	err = showSettings(env, tmpfile)
	if err != nil {
		return err
	}

	contents, err := getFileContents(tmpfile.Name())
	assert.NoError(t, err)

	buf := new(bytes.Buffer)
	encoder := toml.NewEncoder(buf)
	err = encoder.Encode(env)
	assert.NoError(t, err)

	expectedContents := buf.String()

	assert.Equal(t, expectedContents, contents)

	return nil
}

func TestEnvShowSettings(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	tmpfile, err := ioutil.TempFile("", "envShowSettings-")
	assert.NoError(t, err)
	defer os.Remove(tmpfile.Name())

	err = testEnvShowSettings(t, tmpdir, tmpfile)
	assert.NoError(t, err)
}

func TestEnvShowSettingsInvalidFile(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	tmpfile, err := ioutil.TempFile("", "envShowSettings-")
	assert.NoError(t, err)

	// close the file
	tmpfile.Close()

	err = testEnvShowSettings(t, tmpdir, tmpfile)
	assert.Error(t, err)
}

func TestEnvHandleSettings(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	_, err = getExpectedSettings(config, tmpdir, configFile)
	assert.NoError(t, err)

	m := map[string]interface{}{
		"configFile":    configFile,
		"runtimeConfig": config,
	}

	tmpfile, err := ioutil.TempFile("", "")
	assert.NoError(t, err)
	defer os.Remove(tmpfile.Name())

	err = handleSettings(tmpfile, m)
	assert.NoError(t, err)

	var env EnvInfo

	_, err = toml.DecodeFile(tmpfile.Name(), &env)
	assert.NoError(t, err)
}

func TestEnvHandleSettingsInvalidShimConfig(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(err)

	_, err = getExpectedSettings(config, tmpdir, configFile)
	assert.NoError(err)

	config.ShimConfig = "invalid shim config"

	m := map[string]interface{}{
		"configFile":    configFile,
		"runtimeConfig": config,
	}

	tmpfile, err := ioutil.TempFile("", "")
	assert.NoError(err)
	defer os.Remove(tmpfile.Name())

	err = handleSettings(tmpfile, m)
	assert.Error(err)
}

func TestEnvHandleSettingsInvalidParams(t *testing.T) {
	err := handleSettings(nil, map[string]interface{}{})
	assert.Error(t, err)
}

func TestEnvHandleSettingsEmptyMap(t *testing.T) {
	err := handleSettings(os.Stdout, map[string]interface{}{})
	assert.Error(t, err)
}

func TestEnvHandleSettingsInvalidFile(t *testing.T) {
	m := map[string]interface{}{
		"configFile":    "foo",
		"runtimeConfig": oci.RuntimeConfig{},
	}

	err := handleSettings(nil, m)
	assert.Error(t, err)
}

func TestEnvHandleSettingsInvalidConfigFileType(t *testing.T) {
	m := map[string]interface{}{
		"configFile":    123,
		"runtimeConfig": oci.RuntimeConfig{},
	}

	err := handleSettings(os.Stderr, m)
	assert.Error(t, err)
}

func TestEnvHandleSettingsInvalidRuntimeConfigType(t *testing.T) {
	m := map[string]interface{}{
		"configFile":    "/some/where",
		"runtimeConfig": true,
	}

	err := handleSettings(os.Stderr, m)
	assert.Error(t, err)
}

func TestEnvCLIFunction(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	_, err = getExpectedSettings(config, tmpdir, configFile)
	assert.NoError(t, err)

	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = "foo"

	ctx.App.Metadata = map[string]interface{}{
		"configFile":    configFile,
		"runtimeConfig": config,
	}

	fn, ok := kataEnvCLICommand.Action.(func(context *cli.Context) error)
	assert.True(t, ok)

	devNull, err := os.OpenFile(os.DevNull, os.O_WRONLY, 0666)
	assert.NoError(t, err)

	// throw away output
	savedOutputFile := defaultOutputFile
	defaultOutputFile = devNull

	defer func() {
		defaultOutputFile = savedOutputFile
	}()

	err = fn(ctx)
	assert.NoError(t, err)
}

func TestEnvCLIFunctionFail(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	configFile, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	_, err = getExpectedSettings(config, tmpdir, configFile)
	assert.NoError(t, err)

	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = "foo"

	ctx.App.Metadata = map[string]interface{}{
		"configFile":    configFile,
		"runtimeConfig": config,
	}

	fn, ok := kataEnvCLICommand.Action.(func(context *cli.Context) error)
	assert.True(t, ok)

	savedOutputFile := defaultOutputFile
	// invalidate
	defaultOutputFile = nil

	defer func() {
		defaultOutputFile = savedOutputFile
	}()

	err = fn(ctx)
	assert.Error(t, err)
}

func TestGetHypervisorInfo(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(err)

	info := getHypervisorInfo(config)
	assert.Equal(info.Version, testHypervisorVersion)

	err = os.Remove(config.HypervisorConfig.HypervisorPath)
	assert.NoError(err)

	info = getHypervisorInfo(config)
	assert.Equal(info.Version, unknown)
}
