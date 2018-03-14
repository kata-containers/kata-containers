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
	"fmt"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"reflect"
	goruntime "runtime"
	"strconv"
	"strings"
	"syscall"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/stretchr/testify/assert"
)

type testRuntimeConfig struct {
	RuntimeConfig     oci.RuntimeConfig
	RuntimeConfigFile string
	ConfigPath        string
	ConfigPathLink    string
	LogDir            string
	LogPath           string
}

func makeRuntimeConfigFileData(hypervisor, hypervisorPath, kernelPath, imagePath, kernelParams, machineType, shimPath, proxyPath, logPath string, disableBlock bool, blockDeviceDriver string) string {
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
	default_memory = ` + strconv.FormatUint(uint64(defaultMemSize), 10) + `
	disable_block_device_use =  ` + strconv.FormatBool(disableBlock) + `

	[proxy.kata]
	path = "` + proxyPath + `"

	[shim.kata]
	path = "` + shimPath + `"

	[agent.kata]

        [runtime]
	`
}

func createConfig(configPath string, fileData string) error {

	err := ioutil.WriteFile(configPath, []byte(fileData), testFileMode)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Unable to create config file %s %v\n", configPath, err)
		return err
	}

	return nil
}

// createAllRuntimeConfigFiles creates all files necessary to call
// loadConfiguration().
func createAllRuntimeConfigFiles(dir, hypervisor string) (config testRuntimeConfig, err error) {
	if dir == "" {
		return config, fmt.Errorf("BUG: need directory")
	}

	if hypervisor == "" {
		return config, fmt.Errorf("BUG: need hypervisor")
	}

	hypervisorPath := path.Join(dir, "hypervisor")
	kernelPath := path.Join(dir, "kernel")
	kernelParams := "foo=bar xyz"
	imagePath := path.Join(dir, "image")
	shimPath := path.Join(dir, "shim")
	proxyPath := path.Join(dir, "proxy")
	logDir := path.Join(dir, "logs")
	logPath := path.Join(logDir, "runtime.log")
	machineType := "machineType"
	disableBlockDevice := true
	blockDeviceDriver := "virtio-scsi"

	runtimeConfigFileData := makeRuntimeConfigFileData(hypervisor, hypervisorPath, kernelPath, imagePath, kernelParams, machineType, shimPath, proxyPath, logPath, disableBlockDevice, blockDeviceDriver)

	configPath := path.Join(dir, "runtime.toml")
	err = createConfig(configPath, runtimeConfigFileData)
	if err != nil {
		return config, err
	}

	configPathLink := path.Join(filepath.Dir(configPath), "link-to-configuration.toml")

	// create a link to the config file
	err = syscall.Symlink(configPath, configPathLink)
	if err != nil {
		return config, err
	}

	files := []string{hypervisorPath, kernelPath, imagePath, shimPath, proxyPath}

	for _, file := range files {
		// create the resource
		err = createEmptyFile(file)
		if err != nil {
			return config, err
		}
	}

	hypervisorConfig := vc.HypervisorConfig{
		HypervisorPath:        hypervisorPath,
		KernelPath:            kernelPath,
		ImagePath:             imagePath,
		KernelParams:          vc.DeserializeParams(strings.Fields(kernelParams)),
		HypervisorMachineType: machineType,
		DefaultVCPUs:          defaultVCPUCount,
		DefaultMemSz:          defaultMemSize,
		DisableBlockDeviceUse: disableBlockDevice,
		BlockDeviceDriver:     defaultBlockDeviceDriver,
		DefaultBridges:        defaultBridgesCount,
		Mlock:                 !defaultEnableSwap,
	}

	agentConfig := vc.KataAgentConfig{}

	proxyConfig := vc.ProxyConfig{
		Path: proxyPath,
	}

	shimConfig := vc.ShimConfig{
		Path: shimPath,
	}

	runtimeConfig := oci.RuntimeConfig{
		HypervisorType:   defaultHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   defaultAgent,
		AgentConfig: agentConfig,

		ProxyType:   defaultProxy,
		ProxyConfig: proxyConfig,

		ShimType:   defaultShim,
		ShimConfig: shimConfig,

		VMConfig: vc.Resources{
			Memory: uint(defaultMemSize),
		},
	}

	config = testRuntimeConfig{
		RuntimeConfig:     runtimeConfig,
		RuntimeConfigFile: configPath,
		ConfigPath:        configPath,
		ConfigPathLink:    configPathLink,
		LogDir:            logDir,
		LogPath:           logPath,
	}

	return config, nil
}

// testLoadConfiguration accepts an optional function that can be used
// to modify the test: if a function is specified, it indicates if the
// subsequent call to loadConfiguration() is expected to fail by
// returning a bool. If the function itself fails, that is considered an
// error.
func testLoadConfiguration(t *testing.T, dir string,
	fn func(config testRuntimeConfig, configFile string, ignoreLogging bool) (bool, error)) {
	subDir := path.Join(dir, "test")

	for _, hypervisor := range []string{"qemu"} {
	Loop:
		for _, ignoreLogging := range []bool{true, false} {
			err := os.RemoveAll(subDir)
			assert.NoError(t, err)

			err = os.MkdirAll(subDir, testDirMode)
			assert.NoError(t, err)

			testConfig, err := createAllRuntimeConfigFiles(subDir, hypervisor)
			assert.NoError(t, err)

			configFiles := []string{testConfig.ConfigPath, testConfig.ConfigPathLink, ""}

			// override
			defaultRuntimeConfiguration = testConfig.ConfigPath
			defaultSysConfRuntimeConfiguration = ""

			for _, file := range configFiles {
				var err error
				expectFail := false

				if fn != nil {
					expectFail, err = fn(testConfig, file, ignoreLogging)
					assert.NoError(t, err)
				}

				resolvedConfigPath, config, err := loadConfiguration(file, ignoreLogging)
				if expectFail {
					assert.Error(t, err)

					// no point proceeding in the error scenario.
					break Loop
				} else {
					assert.NoError(t, err)
				}

				if file == "" {
					assert.Equal(t, defaultRuntimeConfiguration, resolvedConfigPath)
				} else {
					assert.Equal(t, testConfig.ConfigPath, resolvedConfigPath)
				}

				assert.Equal(t, defaultRuntimeConfiguration, resolvedConfigPath)
				result := reflect.DeepEqual(config, testConfig.RuntimeConfig)
				if !result {
					t.Fatalf("Expected\n%+v\nGot\n%+v", config, testConfig.RuntimeConfig)
				}
				assert.True(t, result)

				err = os.RemoveAll(testConfig.LogDir)
				assert.NoError(t, err)
			}
		}
	}
}

func TestConfigLoadConfiguration(t *testing.T) {
	tmpdir, err := ioutil.TempDir(testDir, "load-config-")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	testLoadConfiguration(t, tmpdir, nil)
}

func TestConfigLoadConfigurationFailBrokenSymLink(t *testing.T) {
	tmpdir, err := ioutil.TempDir(testDir, "runtime-config-")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	testLoadConfiguration(t, tmpdir,
		func(config testRuntimeConfig, configFile string, ignoreLogging bool) (bool, error) {
			expectFail := false

			if configFile == config.ConfigPathLink {
				// break the symbolic link
				err = os.Remove(config.ConfigPathLink)
				if err != nil {
					return expectFail, err
				}

				expectFail = true
			}

			return expectFail, nil
		})
}

func TestConfigLoadConfigurationFailSymLinkLoop(t *testing.T) {
	tmpdir, err := ioutil.TempDir(testDir, "runtime-config-")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	testLoadConfiguration(t, tmpdir,
		func(config testRuntimeConfig, configFile string, ignoreLogging bool) (bool, error) {
			expectFail := false

			if configFile == config.ConfigPathLink {
				// remove the config file
				err = os.Remove(config.ConfigPath)
				if err != nil {
					return expectFail, err
				}

				// now, create a sym-link loop
				err := os.Symlink(config.ConfigPathLink, config.ConfigPath)
				if err != nil {
					return expectFail, err
				}

				expectFail = true
			}

			return expectFail, nil
		})
}

func TestConfigLoadConfigurationFailMissingHypervisor(t *testing.T) {
	tmpdir, err := ioutil.TempDir(testDir, "runtime-config-")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	testLoadConfiguration(t, tmpdir,
		func(config testRuntimeConfig, configFile string, ignoreLogging bool) (bool, error) {
			expectFail := true

			err = os.Remove(config.RuntimeConfig.HypervisorConfig.HypervisorPath)
			if err != nil {
				return expectFail, err
			}

			return expectFail, nil
		})
}

func TestConfigLoadConfigurationFailMissingImage(t *testing.T) {
	tmpdir, err := ioutil.TempDir(testDir, "runtime-config-")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	testLoadConfiguration(t, tmpdir,
		func(config testRuntimeConfig, configFile string, ignoreLogging bool) (bool, error) {
			expectFail := true

			err = os.Remove(config.RuntimeConfig.HypervisorConfig.ImagePath)
			if err != nil {
				return expectFail, err
			}

			return expectFail, nil
		})
}

func TestConfigLoadConfigurationFailMissingKernel(t *testing.T) {
	tmpdir, err := ioutil.TempDir(testDir, "runtime-config-")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	testLoadConfiguration(t, tmpdir,
		func(config testRuntimeConfig, configFile string, ignoreLogging bool) (bool, error) {
			expectFail := true

			err = os.Remove(config.RuntimeConfig.HypervisorConfig.KernelPath)
			if err != nil {
				return expectFail, err
			}

			return expectFail, nil
		})
}

func TestConfigLoadConfigurationFailMissingShim(t *testing.T) {
	tmpdir, err := ioutil.TempDir(testDir, "runtime-config-")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	testLoadConfiguration(t, tmpdir,
		func(config testRuntimeConfig, configFile string, ignoreLogging bool) (bool, error) {
			expectFail := true

			shimConfig, ok := config.RuntimeConfig.ShimConfig.(vc.ShimConfig)
			if !ok {
				return expectFail, fmt.Errorf("cannot determine shim config")
			}
			err = os.Remove(shimConfig.Path)
			if err != nil {
				return expectFail, err
			}

			return expectFail, nil
		})
}

func TestConfigLoadConfigurationFailUnreadableConfig(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip(testDisabledNeedNonRoot)
	}

	tmpdir, err := ioutil.TempDir(testDir, "runtime-config-")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	testLoadConfiguration(t, tmpdir,
		func(config testRuntimeConfig, configFile string, ignoreLogging bool) (bool, error) {
			expectFail := true

			// make file unreadable by non-root user
			err = os.Chmod(config.ConfigPath, 0000)
			if err != nil {
				return expectFail, err
			}

			return expectFail, nil
		})
}

func TestConfigLoadConfigurationFailTOMLConfigFileInvalidContents(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip(testDisabledNeedNonRoot)
	}

	tmpdir, err := ioutil.TempDir(testDir, "runtime-config-")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	testLoadConfiguration(t, tmpdir,
		func(config testRuntimeConfig, configFile string, ignoreLogging bool) (bool, error) {
			expectFail := true

			err := createFile(config.ConfigPath,
				`<?xml version="1.0"?>
			<foo>I am not TOML! ;-)</foo>
			<bar>I am invalid XML!`)

			if err != nil {
				return expectFail, err
			}

			return expectFail, nil
		})
}

func TestConfigLoadConfigurationFailTOMLConfigFileDuplicatedData(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip(testDisabledNeedNonRoot)
	}

	tmpdir, err := ioutil.TempDir(testDir, "runtime-config-")
	assert.NoError(t, err)
	defer os.RemoveAll(tmpdir)

	testLoadConfiguration(t, tmpdir,
		func(config testRuntimeConfig, configFile string, ignoreLogging bool) (bool, error) {
			expectFail := true

			text, err := getFileContents(config.ConfigPath)
			if err != nil {
				return expectFail, err
			}

			// create a config file containing two sets of
			// data.
			err = createFile(config.ConfigPath, fmt.Sprintf("%s\n%s\n", text, text))
			if err != nil {
				return expectFail, err
			}

			return expectFail, nil
		})
}

func TestMinimalRuntimeConfig(t *testing.T) {
	dir, err := ioutil.TempDir(testDir, "minimal-runtime-config-")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	shimPath := path.Join(dir, "shim")
	proxyPath := path.Join(dir, "proxy")

	runtimeMinimalConfig := `
	# Runtime configuration file

	[proxy.kata]
	path = "` + proxyPath + `"

	[shim.kata]
	path = "` + shimPath + `"

	[agent.kata]
`

	configPath := path.Join(dir, "runtime.toml")
	err = createConfig(configPath, runtimeMinimalConfig)
	if err != nil {
		t.Fatal(err)
	}

	_, config, err := loadConfiguration(configPath, false)
	if err == nil {
		t.Fatalf("Expected loadConfiguration to fail as shim path does not exist: %+v", config)
	}

	err = createEmptyFile(shimPath)
	if err != nil {
		t.Error(err)
	}

	err = createEmptyFile(proxyPath)
	if err != nil {
		t.Error(err)
	}

	_, config, err = loadConfiguration(configPath, false)
	if err != nil {
		t.Fatal(err)
	}

	expectedHypervisorConfig := vc.HypervisorConfig{
		HypervisorPath:        defaultHypervisorPath,
		KernelPath:            defaultKernelPath,
		ImagePath:             defaultImagePath,
		HypervisorMachineType: defaultMachineType,
		DefaultVCPUs:          defaultVCPUCount,
		DefaultMemSz:          defaultMemSize,
		DisableBlockDeviceUse: defaultDisableBlockDeviceUse,
		DefaultBridges:        defaultBridgesCount,
		Mlock:                 !defaultEnableSwap,
		BlockDeviceDriver:     defaultBlockDeviceDriver,
	}

	expectedAgentConfig := vc.KataAgentConfig{}

	expectedProxyConfig := vc.ProxyConfig{
		Path: proxyPath,
	}

	expectedShimConfig := vc.ShimConfig{
		Path: shimPath,
	}

	expectedConfig := oci.RuntimeConfig{
		HypervisorType:   defaultHypervisor,
		HypervisorConfig: expectedHypervisorConfig,

		AgentType:   defaultAgent,
		AgentConfig: expectedAgentConfig,

		ProxyType:   defaultProxy,
		ProxyConfig: expectedProxyConfig,

		ShimType:   defaultShim,
		ShimConfig: expectedShimConfig,
	}

	if reflect.DeepEqual(config, expectedConfig) == false {
		t.Fatalf("Got %+v\n expecting %+v", config, expectedConfig)
	}

	if err := os.Remove(configPath); err != nil {
		t.Fatal(err)
	}
}

func TestNewQemuHypervisorConfig(t *testing.T) {
	dir, err := ioutil.TempDir(testDir, "hypervisor-config-")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	hypervisorPath := path.Join(dir, "hypervisor")
	kernelPath := path.Join(dir, "kernel")
	imagePath := path.Join(dir, "image")
	machineType := "machineType"
	disableBlock := true

	hypervisor := hypervisor{
		Path:                  hypervisorPath,
		Kernel:                kernelPath,
		Image:                 imagePath,
		MachineType:           machineType,
		DisableBlockDeviceUse: disableBlock,
	}

	files := []string{hypervisorPath, kernelPath, imagePath}
	filesLen := len(files)

	for i, file := range files {
		_, err := newQemuHypervisorConfig(hypervisor)
		if err == nil {
			t.Fatalf("Expected newQemuHypervisorConfig to fail as not all paths exist (not created %v)",
				strings.Join(files[i:filesLen], ","))
		}

		// create the resource
		err = createEmptyFile(file)
		if err != nil {
			t.Error(err)
		}
	}

	// all paths exist now
	config, err := newQemuHypervisorConfig(hypervisor)
	if err != nil {
		t.Fatal(err)
	}

	if config.HypervisorPath != hypervisor.Path {
		t.Errorf("Expected hypervisor path %v, got %v", hypervisor.Path, config.HypervisorPath)
	}

	if config.KernelPath != hypervisor.Kernel {
		t.Errorf("Expected kernel path %v, got %v", hypervisor.Kernel, config.KernelPath)
	}

	if config.ImagePath != hypervisor.Image {
		t.Errorf("Expected image path %v, got %v", hypervisor.Image, config.ImagePath)
	}

	if config.DisableBlockDeviceUse != disableBlock {
		t.Errorf("Expected value for disable block usage %v, got %v", disableBlock, config.DisableBlockDeviceUse)
	}

}

func TestNewShimConfig(t *testing.T) {
	dir, err := ioutil.TempDir(testDir, "shim-config-")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	shimPath := path.Join(dir, "shim")

	shim := shim{
		Path: shimPath,
	}

	_, err = newShimConfig(shim)
	if err == nil {
		t.Fatalf("Expected newShimConfig to fail as no paths exist")
	}

	err = createEmptyFile(shimPath)
	if err != nil {
		t.Error(err)
	}

	shConfig, err := newShimConfig(shim)
	if err != nil {
		t.Fatalf("newShimConfig failed unexpectedly: %v", err)
	}

	if shConfig.Path != shimPath {
		t.Errorf("Expected shim path %v, got %v", shimPath, shConfig.Path)
	}
}

func TestHypervisorDefaults(t *testing.T) {
	assert := assert.New(t)

	h := hypervisor{}

	assert.Equal(h.machineType(), defaultMachineType, "default hypervisor machine type wrong")
	assert.Equal(h.defaultVCPUs(), defaultVCPUCount, "default vCPU number is wrong")
	assert.Equal(h.defaultMemSz(), defaultMemSize, "default memory size is wrong")

	machineType := "foo"
	h.MachineType = machineType
	assert.Equal(h.machineType(), machineType, "custom hypervisor machine type wrong")

	// auto inferring
	h.DefaultVCPUs = -1
	assert.Equal(h.defaultVCPUs(), uint32(goruntime.NumCPU()), "default vCPU number is wrong")

	h.DefaultVCPUs = 2
	assert.Equal(h.defaultVCPUs(), uint32(2), "default vCPU number is wrong")

	numCPUs := goruntime.NumCPU()
	h.DefaultVCPUs = int32(numCPUs) + 1
	assert.Equal(h.defaultVCPUs(), uint32(numCPUs), "default vCPU number is wrong")

	h.DefaultMemSz = 1024
	assert.Equal(h.defaultMemSz(), uint32(1024), "default memory size is wrong")
}

func TestHypervisorDefaultsHypervisor(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	testHypervisorPath := filepath.Join(tmpdir, "hypervisor")
	testHypervisorLinkPath := filepath.Join(tmpdir, "hypervisor-link")

	err = createEmptyFile(testHypervisorPath)
	assert.NoError(err)

	err = syscall.Symlink(testHypervisorPath, testHypervisorLinkPath)
	assert.NoError(err)

	savedHypervisorPath := defaultHypervisorPath

	defer func() {
		defaultHypervisorPath = savedHypervisorPath
	}()

	defaultHypervisorPath = testHypervisorPath
	h := hypervisor{}
	p, err := h.path()
	assert.NoError(err)
	assert.Equal(p, defaultHypervisorPath, "default hypervisor path wrong")

	// test path resolution
	defaultHypervisorPath = testHypervisorLinkPath
	h = hypervisor{}
	p, err = h.path()
	assert.NoError(err)
	assert.Equal(p, testHypervisorPath)
}

func TestHypervisorDefaultsKernel(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	testKernelPath := filepath.Join(tmpdir, "kernel")
	testKernelLinkPath := filepath.Join(tmpdir, "kernel-link")

	err = createEmptyFile(testKernelPath)
	assert.NoError(err)

	err = syscall.Symlink(testKernelPath, testKernelLinkPath)
	assert.NoError(err)

	savedKernelPath := defaultKernelPath

	defer func() {
		defaultKernelPath = savedKernelPath
	}()

	defaultKernelPath = testKernelPath

	h := hypervisor{}
	p, err := h.kernel()
	assert.NoError(err)
	assert.Equal(p, defaultKernelPath, "default Kernel path wrong")

	// test path resolution
	defaultKernelPath = testKernelLinkPath
	h = hypervisor{}
	p, err = h.kernel()
	assert.NoError(err)
	assert.Equal(p, testKernelPath)

	assert.Equal(h.kernelParams(), defaultKernelParams, "default hypervisor image wrong")
	kernelParams := "foo=bar xyz"
	h.KernelParams = kernelParams
	assert.Equal(h.kernelParams(), kernelParams, "custom hypervisor kernel parameterms wrong")
}

func TestHypervisorDefaultsImage(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	testImagePath := filepath.Join(tmpdir, "image")
	testImageLinkPath := filepath.Join(tmpdir, "image-link")

	err = createEmptyFile(testImagePath)
	assert.NoError(err)

	err = syscall.Symlink(testImagePath, testImageLinkPath)
	assert.NoError(err)

	savedImagePath := defaultImagePath

	defer func() {
		defaultImagePath = savedImagePath
	}()

	defaultImagePath = testImagePath
	h := hypervisor{}
	p, err := h.image()
	assert.NoError(err)
	assert.Equal(p, defaultImagePath, "default Image path wrong")

	// test path resolution
	defaultImagePath = testImageLinkPath
	h = hypervisor{}
	p, err = h.image()
	assert.NoError(err)
	assert.Equal(p, testImagePath)
}

func TestProxyDefaults(t *testing.T) {
	p := proxy{}

	assert.Equal(t, p.path(), defaultProxyPath, "default proxy path wrong")

	path := "/foo/bar/baz/proxy"
	p.Path = path
	assert.Equal(t, p.path(), path, "custom proxy path wrong")
}

func TestShimDefaults(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	testShimPath := filepath.Join(tmpdir, "shim")
	testShimLinkPath := filepath.Join(tmpdir, "shim-link")

	err = createEmptyFile(testShimPath)
	assert.NoError(err)

	err = syscall.Symlink(testShimPath, testShimLinkPath)
	assert.NoError(err)

	savedShimPath := defaultShimPath

	defer func() {
		defaultShimPath = savedShimPath
	}()

	defaultShimPath = testShimPath
	s := shim{}
	p, err := s.path()
	assert.NoError(err)
	assert.Equal(p, defaultShimPath, "default shim path wrong")

	// test path resolution
	defaultShimPath = testShimLinkPath
	s = shim{}
	p, err = s.path()
	assert.NoError(err)
	assert.Equal(p, testShimPath)

	assert.False(s.debug())
	s.Debug = true
	assert.True(s.debug())
}

func TestGetDefaultConfigFilePaths(t *testing.T) {
	assert := assert.New(t)

	results := getDefaultConfigFilePaths()
	// There should be atleast two config file locations
	assert.True(len(results) >= 2)

	for _, f := range results {
		// Paths cannot be empty
		assert.NotNil(f)
	}
}

func TestGetDefaultConfigFile(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	hypervisor := "qemu"
	confDir := filepath.Join(tmpdir, "conf")
	sysConfDir := filepath.Join(tmpdir, "sysconf")

	for _, dir := range []string{confDir, sysConfDir} {
		err = os.MkdirAll(dir, testDirMode)
		assert.NoError(err)
	}

	confDirConfig, err := createAllRuntimeConfigFiles(confDir, hypervisor)
	assert.NoError(err)

	sysConfDirConfig, err := createAllRuntimeConfigFiles(sysConfDir, hypervisor)
	assert.NoError(err)

	savedConf := defaultRuntimeConfiguration
	savedSysConf := defaultSysConfRuntimeConfiguration

	defaultRuntimeConfiguration = confDirConfig.ConfigPath
	defaultSysConfRuntimeConfiguration = sysConfDirConfig.ConfigPath

	defer func() {
		defaultRuntimeConfiguration = savedConf
		defaultSysConfRuntimeConfiguration = savedSysConf

	}()

	got, err := getDefaultConfigFile()
	assert.NoError(err)
	// defaultSysConfRuntimeConfiguration has priority over defaultRuntimeConfiguration
	assert.Equal(got, defaultSysConfRuntimeConfiguration)

	// force defaultRuntimeConfiguration to be returned
	os.Remove(defaultSysConfRuntimeConfiguration)

	got, err = getDefaultConfigFile()
	assert.NoError(err)
	assert.Equal(got, defaultRuntimeConfiguration)

	// force error
	os.Remove(defaultRuntimeConfiguration)

	_, err = getDefaultConfigFile()
	assert.Error(err)
}

func TestDefaultBridges(t *testing.T) {
	assert := assert.New(t)

	h := hypervisor{DefaultBridges: 0}

	bridges := h.defaultBridges()
	assert.Equal(defaultBridgesCount, bridges)

	h.DefaultBridges = maxPCIBridges + 1
	bridges = h.defaultBridges()
	assert.Equal(maxPCIBridges, bridges)

	h.DefaultBridges = maxPCIBridges
	bridges = h.defaultBridges()
	assert.Equal(maxPCIBridges, bridges)
}

func TestDefaultFirmware(t *testing.T) {
	assert := assert.New(t)

	// save default firmware path
	oldDefaultFirmwarePath := defaultFirmwarePath

	f, err := ioutil.TempFile(os.TempDir(), "qboot.bin")
	assert.NoError(err)
	assert.NoError(f.Close())
	defer os.RemoveAll(f.Name())

	h := hypervisor{}
	defaultFirmwarePath = ""
	p, err := h.firmware()
	assert.NoError(err)
	assert.Empty(p)

	defaultFirmwarePath = f.Name()
	p, err = h.firmware()
	assert.NoError(err)
	assert.NotEmpty(p)

	// restore default firmware path
	defaultFirmwarePath = oldDefaultFirmwarePath
}

func TestDefaultMachineAccelerators(t *testing.T) {
	assert := assert.New(t)
	machineAccelerators := "abc,123,rgb"
	h := hypervisor{MachineAccelerators: machineAccelerators}
	assert.Equal(machineAccelerators, h.machineAccelerators())

	machineAccelerators = ""
	h.MachineAccelerators = machineAccelerators
	assert.Equal(machineAccelerators, h.machineAccelerators())

	machineAccelerators = "abc"
	h.MachineAccelerators = machineAccelerators
	assert.Equal(machineAccelerators, h.machineAccelerators())

	machineAccelerators = "abc,123"
	h.MachineAccelerators = "abc,,123"
	assert.Equal(machineAccelerators, h.machineAccelerators())

	machineAccelerators = "abc,123"
	h.MachineAccelerators = ",,abc,,123,,,"
	assert.Equal(machineAccelerators, h.machineAccelerators())

	machineAccelerators = "abc,123"
	h.MachineAccelerators = "abc,,123,,,"
	assert.Equal(machineAccelerators, h.machineAccelerators())

	machineAccelerators = "abc"
	h.MachineAccelerators = ",,abc,"
	assert.Equal(machineAccelerators, h.machineAccelerators())

	machineAccelerators = "abc"
	h.MachineAccelerators = ", , abc , ,"
	assert.Equal(machineAccelerators, h.machineAccelerators())

	machineAccelerators = "abc"
	h.MachineAccelerators = " abc "
	assert.Equal(machineAccelerators, h.machineAccelerators())

	machineAccelerators = "abc,123"
	h.MachineAccelerators = ", abc , 123 ,"
	assert.Equal(machineAccelerators, h.machineAccelerators())

	machineAccelerators = "abc,123"
	h.MachineAccelerators = ",, abc ,,, 123 ,,"
	assert.Equal(machineAccelerators, h.machineAccelerators())
}

func TestUpdateRuntimeConfiguration(t *testing.T) {
	assert := assert.New(t)

	assert.NotEqual(defaultAgent, vc.HyperstartAgent)

	config := oci.RuntimeConfig{}

	tomlConf := tomlConfig{
		Agent: map[string]agent{
			// force a non-default value
			kataAgentTableType: {},
		},
	}

	assert.NotEqual(config.AgentType, vc.AgentType(kataAgentTableType))
	assert.NotEqual(config.AgentConfig, vc.KataAgentConfig{})

	err := updateRuntimeConfig("", tomlConf, &config)
	assert.NoError(err)

	assert.Equal(config.AgentType, vc.AgentType(kataAgentTableType))
	assert.Equal(config.AgentConfig, vc.KataAgentConfig{})
}

func TestUpdateRuntimeConfigurationVMConfig(t *testing.T) {
	assert := assert.New(t)

	vcpus := uint(2)
	mem := uint(2048)

	config := oci.RuntimeConfig{}
	expectedVMConfig := vc.Resources{
		Memory: mem,
	}

	tomlConf := tomlConfig{
		Hypervisor: map[string]hypervisor{
			qemuHypervisorTableType: {
				DefaultVCPUs: int32(vcpus),
				DefaultMemSz: uint32(mem),
				Path:         "/",
				Kernel:       "/",
				Image:        "/",
				Firmware:     "/",
			},
		},
	}

	err := updateRuntimeConfig("", tomlConf, &config)
	assert.NoError(err)

	assert.Equal(expectedVMConfig, config.VMConfig)
}
