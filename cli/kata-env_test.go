// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"encoding/json"
	"flag"
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
	vcUtils "github.com/kata-containers/runtime/virtcontainers/utils"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/urfave/cli"

	"github.com/kata-containers/runtime/pkg/katautils"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/stretchr/testify/assert"
	"strconv"
)

const testProxyVersion = "proxy version 0.1"
const testShimVersion = "shim version 0.1"
const testNetmonVersion = "netmon version 0.1"
const testHypervisorVersion = "QEMU emulator version 2.7.0+git.741f430a96-6.1, Copyright (c) 2003-2016 Fabrice Bellard and the QEMU Project developers"

const defaultVCPUCount uint32 = 1
const defaultMaxVCPUCount uint32 = 0
const defaultMemSize uint32 = 2048 // MiB
const defaultMsize9p uint32 = 8192
const defaultGuestHookPath string = ""

var (
	hypervisorDebug = false
	proxyDebug      = false
	runtimeDebug    = false
	shimDebug       = false
	netmonDebug     = false
)

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

func createConfig(configPath string, fileData string) error {

	err := ioutil.WriteFile(configPath, []byte(fileData), testFileMode)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Unable to create config file %s %v\n", configPath, err)
		return err
	}

	return nil
}

func makeRuntimeConfigFileData(hypervisor, hypervisorPath, kernelPath, imagePath, kernelParams, machineType, shimPath, proxyPath, netmonPath, logPath string, disableBlock bool, blockDeviceDriver string, enableIOThreads bool, hotplugVFIOOnRootBus, disableNewNetNs bool) string {
	return `
	# Runtime configuration file

	[hypervisor.` + hypervisor + `]
	path = "` + hypervisorPath + `"
	kernel = "` + kernelPath + `"
	block_device_driver =  "` + blockDeviceDriver + `"
	kernel_params = "` + kernelParams + `"
	image = "` + imagePath + `"
	machine_type = "` + machineType + `"
	default_vcpus = ` + strconv.FormatUint(uint64(defaultVCPUCount), 10) + `
	default_maxvcpus = ` + strconv.FormatUint(uint64(defaultMaxVCPUCount), 10) + `
	default_memory = ` + strconv.FormatUint(uint64(defaultMemSize), 10) + `
	disable_block_device_use =  ` + strconv.FormatBool(disableBlock) + `
	enable_iothreads =  ` + strconv.FormatBool(enableIOThreads) + `
	hotplug_vfio_on_root_bus =  ` + strconv.FormatBool(hotplugVFIOOnRootBus) + `
	msize_9p = ` + strconv.FormatUint(uint64(defaultMsize9p), 10) + `
	enable_debug = ` + strconv.FormatBool(hypervisorDebug) + `
	guest_hook_path = "` + defaultGuestHookPath + `"

	[proxy.kata]
	enable_debug = ` + strconv.FormatBool(proxyDebug) + `
	path = "` + proxyPath + `"

	[shim.kata]
	path = "` + shimPath + `"
	enable_debug = ` + strconv.FormatBool(shimDebug) + `

	[agent.kata]

	[netmon]
	path = "` + netmonPath + `"
	enable_debug = ` + strconv.FormatBool(netmonDebug) + `

        [runtime]
	enable_debug = ` + strconv.FormatBool(runtimeDebug) + `
	disable_new_netns= ` + strconv.FormatBool(disableNewNetNs)
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
	netmonPath := filepath.Join(prefixDir, "netmon")
	disableBlock := true
	blockStorageDriver := "virtio-scsi"
	enableIOThreads := true
	hotplugVFIOOnRootBus := true
	disableNewNetNs := false

	filesToCreate := []string{
		hypervisorPath,
		kernelPath,
		imagePath,
	}

	for _, file := range filesToCreate {
		// files must exist and be >0 bytes.
		err := katautils.WriteFile(file, "foo", testFileMode)
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

	err = makeVersionBinary(netmonPath, testNetmonVersion)
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
		proxyPath,
		netmonPath,
		logPath,
		disableBlock,
		blockStorageDriver,
		enableIOThreads,
		hotplugVFIOOnRootBus,
		disableNewNetNs,
	)

	configFile = path.Join(prefixDir, "runtime.toml")
	err = createConfig(configFile, runtimeConfig)
	if err != nil {
		return "", oci.RuntimeConfig{}, err
	}

	_, config, err = katautils.LoadConfiguration(configFile, true, false)
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

func getExpectedNetmonDetails(config oci.RuntimeConfig) (NetmonInfo, error) {
	return NetmonInfo{
		Version: testNetmonVersion,
		Path:    config.NetmonConfig.Path,
		Debug:   config.NetmonConfig.Debug,
		Enable:  config.NetmonConfig.Enable,
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
		Debug:   shimConfig.Debug,
	}, nil
}

func getExpectedAgentDetails(config oci.RuntimeConfig) (AgentInfo, error) {
	return AgentInfo{
		Type: string(config.AgentType),
	}, nil
}

func genericGetExpectedHostDetails(tmpdir string) (HostInfo, error) {
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
		SupportVSocks:      vcUtils.SupportsVsocks(),
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
%s	: %s
%s	: %s
`,
		archCPUVendorField,
		expectedCPU.Vendor,
		archCPUModelField,
		expectedCPU.Model)

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
		Msize9p:           config.HypervisorConfig.Msize9p,
		MemorySlots:       config.HypervisorConfig.MemSlots,
		Debug:             config.HypervisorConfig.Debug,
		EntropySource:     config.HypervisorConfig.EntropySource,
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

func getExpectedRuntimeDetails(config oci.RuntimeConfig, configFile string) RuntimeInfo {
	runtimePath, _ := os.Executable()

	return RuntimeInfo{
		Version: RuntimeVersionInfo{
			Semver: version,
			Commit: commit,
			OCI:    specs.Version,
		},
		Config: RuntimeConfigInfo{
			Path: configFile,
		},
		Path:            runtimePath,
		Debug:           config.Debug,
		DisableNewNetNs: config.DisableNewNetNs,
	}
}

func getExpectedSettings(config oci.RuntimeConfig, tmpdir, configFile string) (EnvInfo, error) {
	meta := getExpectedMetaInfo()

	runtime := getExpectedRuntimeDetails(config, configFile)

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

	netmon, err := getExpectedNetmonDetails(config)
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
		Netmon:     netmon,
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

	// Run test twice to ensure the individual component debug options are
	// tested.
	for _, debug := range []bool{false, true} {
		hypervisorDebug = debug
		proxyDebug = debug
		runtimeDebug = debug
		shimDebug = debug

		configFile, config, err := makeRuntimeConfig(tmpdir)
		assert.NoError(t, err)

		expectedEnv, err := getExpectedSettings(config, tmpdir, configFile)
		assert.NoError(t, err)

		env, err := getEnvInfo(configFile, config)
		assert.NoError(t, err)

		assert.Equal(t, expectedEnv, env)
	}
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

	expectedRuntime := getExpectedRuntimeDetails(config, configFile)

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
	err = os.Remove(config.ProxyConfig.Path)
	assert.NoError(t, err)

	expectedProxy.Version = unknown

	proxy, err := getProxyInfo(config)
	assert.NoError(t, err)

	assert.Equal(t, expectedProxy, proxy)
}

func TestEnvGetNetmonInfo(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	expectedNetmon, err := getExpectedNetmonDetails(config)
	assert.NoError(t, err)

	netmon, err := getNetmonInfo(config)
	assert.NoError(t, err)

	assert.Equal(t, expectedNetmon, netmon)
}

func TestEnvGetNetmonInfoNoVersion(t *testing.T) {
	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(t, err)

	expectedNetmon, err := getExpectedNetmonDetails(config)
	assert.NoError(t, err)

	// remove the netmon ensuring its version cannot be queried
	err = os.Remove(config.NetmonConfig.Path)
	assert.NoError(t, err)

	expectedNetmon.Version = unknown

	netmon, err := getNetmonInfo(config)
	assert.NoError(t, err)

	assert.Equal(t, expectedNetmon, netmon)
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

func testEnvShowTOMLSettings(t *testing.T, tmpdir string, tmpfile *os.File) error {

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
		Type: "agent-type",
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

	err = writeTOMLSettings(env, tmpfile)
	if err != nil {
		return err
	}

	contents, err := katautils.GetFileContents(tmpfile.Name())
	assert.NoError(t, err)

	buf := new(bytes.Buffer)
	encoder := toml.NewEncoder(buf)
	err = encoder.Encode(env)
	assert.NoError(t, err)

	expectedContents := buf.String()

	assert.Equal(t, expectedContents, contents)

	return nil
}

func testEnvShowJSONSettings(t *testing.T, tmpdir string, tmpfile *os.File) error {

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
		Type: "agent-type",
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

	err = writeJSONSettings(env, tmpfile)
	if err != nil {
		return err
	}

	contents, err := katautils.GetFileContents(tmpfile.Name())
	assert.NoError(t, err)

	buf := new(bytes.Buffer)
	encoder := json.NewEncoder(buf)
	// Ensure we have the same human readable layout
	encoder.SetIndent("", "  ")
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

	err = testEnvShowTOMLSettings(t, tmpdir, tmpfile)
	assert.NoError(t, err)

	// Reset the file to empty for next test
	tmpfile.Truncate(0)
	tmpfile.Seek(0, 0)
	err = testEnvShowJSONSettings(t, tmpdir, tmpfile)
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
	defer os.Remove(tmpfile.Name())

	// close the file
	tmpfile.Close()

	err = testEnvShowTOMLSettings(t, tmpdir, tmpfile)
	assert.Error(t, err)

	// Reset the file to empty for next test
	tmpfile.Truncate(0)
	tmpfile.Seek(0, 0)
	err = testEnvShowJSONSettings(t, tmpdir, tmpfile)
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

	set := flag.NewFlagSet("test", flag.ContinueOnError)
	ctx := createCLIContext(set)
	ctx.App.Name = "foo"
	ctx.App.Metadata["configFile"] = configFile
	ctx.App.Metadata["runtimeConfig"] = config

	tmpfile, err := ioutil.TempFile("", "")
	assert.NoError(t, err)
	defer os.Remove(tmpfile.Name())

	err = handleSettings(tmpfile, ctx)
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

	ctx := createCLIContext(nil)
	ctx.App.Name = "foo"
	ctx.App.Metadata["configFile"] = configFile
	ctx.App.Metadata["runtimeConfig"] = config

	tmpfile, err := ioutil.TempFile("", "")
	assert.NoError(err)
	defer os.Remove(tmpfile.Name())

	err = handleSettings(tmpfile, ctx)
	assert.Error(err)
}

func TestEnvHandleSettingsInvalidParams(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	configFile, _, err := makeRuntimeConfig(tmpdir)
	assert.NoError(err)

	ctx := createCLIContext(nil)
	ctx.App.Name = "foo"
	ctx.App.Metadata["configFile"] = configFile

	err = handleSettings(nil, ctx)
	assert.Error(err)
}

func TestEnvHandleSettingsEmptyMap(t *testing.T) {
	ctx := createCLIContext(nil)
	ctx.App.Name = "foo"
	ctx.App.Metadata = map[string]interface{}{}
	err := handleSettings(os.Stdout, ctx)
	assert.Error(t, err)
}

func TestEnvHandleSettingsInvalidFile(t *testing.T) {
	ctx := createCLIContext(nil)
	ctx.App.Name = "foo"
	ctx.App.Metadata["configFile"] = "foo"
	ctx.App.Metadata["runtimeConfig"] = oci.RuntimeConfig{}

	err := handleSettings(nil, ctx)
	assert.Error(t, err)
}

func TestEnvHandleSettingsInvalidConfigFileType(t *testing.T) {
	ctx := createCLIContext(nil)
	ctx.App.Name = "foo"
	ctx.App.Metadata["configFile"] = 123
	ctx.App.Metadata["runtimeConfig"] = oci.RuntimeConfig{}

	err := handleSettings(os.Stderr, ctx)
	assert.Error(t, err)
}

func TestEnvHandleSettingsInvalidRuntimeConfigType(t *testing.T) {
	ctx := createCLIContext(nil)
	ctx.App.Name = "foo"
	ctx.App.Metadata["configFile"] = "/some/where"
	ctx.App.Metadata["runtimeConfig"] = true

	err := handleSettings(os.Stderr, ctx)
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
	set := flag.NewFlagSet("test", flag.ContinueOnError)
	ctx := createCLIContextWithApp(set, app)
	app.Name = "foo"

	ctx.App.Metadata["configFile"] = configFile
	ctx.App.Metadata["runtimeConfig"] = config

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

	set.Bool("json", true, "")
	ctx = createCLIContextWithApp(set, app)

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

	ctx := createCLIContext(nil)
	ctx.App.Name = "foo"

	ctx.App.Metadata["configFile"] = configFile
	ctx.App.Metadata["runtimeConfig"] = config

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
