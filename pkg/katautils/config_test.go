// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"bytes"
	"fmt"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"reflect"
	goruntime "runtime"
	"strings"
	"syscall"
	"testing"

	"github.com/kata-containers/runtime/pkg/katatestutils"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/runtime/virtcontainers/utils"
	"github.com/stretchr/testify/assert"
)

var (
	hypervisorDebug = false
	proxyDebug      = false
	runtimeDebug    = false
	runtimeTrace    = false
	shimDebug       = false
	netmonDebug     = false
	agentDebug      = false
	agentTrace      = false
)

type testRuntimeConfig struct {
	RuntimeConfig     oci.RuntimeConfig
	RuntimeConfigFile string
	ConfigPath        string
	ConfigPathLink    string
	LogDir            string
	LogPath           string
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
	netmonPath := path.Join(dir, "netmon")
	logDir := path.Join(dir, "logs")
	logPath := path.Join(logDir, "runtime.log")
	machineType := "machineType"
	disableBlockDevice := true
	blockDeviceDriver := "virtio-scsi"
	enableIOThreads := true
	hotplugVFIOOnRootBus := true
	disableNewNetNs := false
	sharedFS := "virtio-9p"

	configFileOptions := katatestutils.RuntimeConfigOptions{
		Hypervisor:           "qemu",
		HypervisorPath:       hypervisorPath,
		KernelPath:           kernelPath,
		ImagePath:            imagePath,
		KernelParams:         kernelParams,
		MachineType:          machineType,
		ShimPath:             shimPath,
		ProxyPath:            proxyPath,
		NetmonPath:           netmonPath,
		LogPath:              logPath,
		DefaultGuestHookPath: defaultGuestHookPath,
		DisableBlock:         disableBlockDevice,
		BlockDeviceDriver:    blockDeviceDriver,
		EnableIOThreads:      enableIOThreads,
		HotplugVFIOOnRootBus: hotplugVFIOOnRootBus,
		DisableNewNetNs:      disableNewNetNs,
		DefaultVCPUCount:     defaultVCPUCount,
		DefaultMaxVCPUCount:  defaultMaxVCPUCount,
		DefaultMemSize:       defaultMemSize,
		DefaultMsize9p:       defaultMsize9p,
		HypervisorDebug:      hypervisorDebug,
		RuntimeDebug:         runtimeDebug,
		RuntimeTrace:         runtimeTrace,
		ProxyDebug:           proxyDebug,
		ShimDebug:            shimDebug,
		NetmonDebug:          netmonDebug,
		AgentDebug:           agentDebug,
		AgentTrace:           agentTrace,
		SharedFS:             sharedFS,
	}

	runtimeConfigFileData := katatestutils.MakeRuntimeConfigFileData(configFileOptions)

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
		// create the resource (which must be >0 bytes)
		err := WriteFile(file, "foo", testFileMode)
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
		NumVCPUs:              defaultVCPUCount,
		DefaultMaxVCPUs:       uint32(goruntime.NumCPU()),
		MemorySize:            defaultMemSize,
		DisableBlockDeviceUse: disableBlockDevice,
		BlockDeviceDriver:     defaultBlockDeviceDriver,
		DefaultBridges:        defaultBridgesCount,
		Mlock:                 !defaultEnableSwap,
		EnableIOThreads:       enableIOThreads,
		HotplugVFIOOnRootBus:  hotplugVFIOOnRootBus,
		Msize9p:               defaultMsize9p,
		MemSlots:              defaultMemSlots,
		EntropySource:         defaultEntropySource,
		GuestHookPath:         defaultGuestHookPath,
		SharedFS:              sharedFS,
		VirtioFSDaemon:        "/path/to/virtiofsd",
	}

	agentConfig := vc.KataAgentConfig{}

	proxyConfig := vc.ProxyConfig{
		Path: proxyPath,
	}

	shimConfig := vc.ShimConfig{
		Path: shimPath,
	}

	netmonConfig := vc.NetmonConfig{
		Path:   netmonPath,
		Debug:  false,
		Enable: false,
	}

	factoryConfig := oci.FactoryConfig{
		TemplatePath:    defaultTemplatePath,
		VMCacheEndpoint: defaultVMCacheEndpoint,
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

		NetmonConfig:    netmonConfig,
		DisableNewNetNs: disableNewNetNs,

		FactoryConfig: factoryConfig,
	}

	err = SetKernelParams(&runtimeConfig)
	if err != nil {
		return config, err
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

				resolvedConfigPath, config, err := LoadConfiguration(file, ignoreLogging, false)
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

			text, err := GetFileContents(config.ConfigPath)
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
	hypervisorPath := path.Join(dir, "hypervisor")
	defaultHypervisorPath = hypervisorPath
	netmonPath := path.Join(dir, "netmon")

	imagePath := path.Join(dir, "image.img")
	initrdPath := path.Join(dir, "initrd.img")

	kernelPath := path.Join(dir, "kernel")

	savedDefaultImagePath := defaultImagePath
	savedDefaultInitrdPath := defaultInitrdPath
	savedDefaultHypervisorPath := defaultHypervisorPath
	savedDefaultKernelPath := defaultKernelPath

	defer func() {
		defaultImagePath = savedDefaultImagePath
		defaultInitrdPath = savedDefaultInitrdPath
		defaultHypervisorPath = savedDefaultHypervisorPath
		defaultKernelPath = savedDefaultKernelPath
	}()

	// Temporarily change the defaults to avoid this test using the real
	// resource files that might be installed on the system!
	defaultImagePath = imagePath
	defaultInitrdPath = initrdPath
	defaultHypervisorPath = hypervisorPath
	defaultKernelPath = kernelPath

	for _, file := range []string{defaultImagePath, defaultInitrdPath, defaultHypervisorPath, defaultKernelPath} {
		err = WriteFile(file, "foo", testFileMode)
		if err != nil {
			t.Fatal(err)
		}
	}

	runtimeMinimalConfig := `
	# Runtime configuration file

	[proxy.kata]
	path = "` + proxyPath + `"

	[shim.kata]
	path = "` + shimPath + `"

	[agent.kata]

	[netmon]
	path = "` + netmonPath + `"
`

	configPath := path.Join(dir, "runtime.toml")
	err = createConfig(configPath, runtimeMinimalConfig)
	if err != nil {
		t.Fatal(err)
	}

	_, config, err := LoadConfiguration(configPath, false, false)
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

	err = createEmptyFile(hypervisorPath)
	if err != nil {
		t.Error(err)
	}

	err = createEmptyFile(netmonPath)
	if err != nil {
		t.Error(err)
	}

	_, config, err = LoadConfiguration(configPath, false, false)
	if err != nil {
		t.Fatal(err)
	}

	expectedHypervisorConfig := vc.HypervisorConfig{
		HypervisorPath:        defaultHypervisorPath,
		KernelPath:            defaultKernelPath,
		ImagePath:             defaultImagePath,
		InitrdPath:            defaultInitrdPath,
		HypervisorMachineType: defaultMachineType,
		NumVCPUs:              defaultVCPUCount,
		DefaultMaxVCPUs:       defaultMaxVCPUCount,
		MemorySize:            defaultMemSize,
		DisableBlockDeviceUse: defaultDisableBlockDeviceUse,
		DefaultBridges:        defaultBridgesCount,
		Mlock:                 !defaultEnableSwap,
		BlockDeviceDriver:     defaultBlockDeviceDriver,
		Msize9p:               defaultMsize9p,
		GuestHookPath:         defaultGuestHookPath,
	}

	expectedAgentConfig := vc.KataAgentConfig{}

	expectedProxyConfig := vc.ProxyConfig{
		Path: proxyPath,
	}

	expectedShimConfig := vc.ShimConfig{
		Path: shimPath,
	}

	expectedNetmonConfig := vc.NetmonConfig{
		Path:   netmonPath,
		Debug:  false,
		Enable: false,
	}

	expectedFactoryConfig := oci.FactoryConfig{
		TemplatePath:    defaultTemplatePath,
		VMCacheEndpoint: defaultVMCacheEndpoint,
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

		NetmonConfig: expectedNetmonConfig,

		FactoryConfig: expectedFactoryConfig,
	}
	err = SetKernelParams(&expectedConfig)
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(config, expectedConfig) == false {
		t.Fatalf("Got %+v\n expecting %+v", config, expectedConfig)
	}
}

func TestMinimalRuntimeConfigWithVsock(t *testing.T) {
	dir, err := ioutil.TempDir(testDir, "minimal-runtime-config-")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	imagePath := path.Join(dir, "image.img")
	initrdPath := path.Join(dir, "initrd.img")
	proxyPath := path.Join(dir, "proxy")
	shimPath := path.Join(dir, "shim")
	hypervisorPath := path.Join(dir, "hypervisor")
	kernelPath := path.Join(dir, "kernel")

	savedDefaultImagePath := defaultImagePath
	savedDefaultInitrdPath := defaultInitrdPath
	savedDefaultHypervisorPath := defaultHypervisorPath
	savedDefaultKernelPath := defaultKernelPath

	defer func() {
		defaultImagePath = savedDefaultImagePath
		defaultInitrdPath = savedDefaultInitrdPath
		defaultHypervisorPath = savedDefaultHypervisorPath
		defaultKernelPath = savedDefaultKernelPath
	}()

	// Temporarily change the defaults to avoid this test using the real
	// resource files that might be installed on the system!
	defaultImagePath = imagePath
	defaultInitrdPath = initrdPath
	defaultHypervisorPath = hypervisorPath
	defaultKernelPath = kernelPath

	for _, file := range []string{proxyPath, shimPath, hypervisorPath, kernelPath, imagePath} {
		err = WriteFile(file, "foo", testFileMode)
		if err != nil {
			t.Fatal(err)
		}
	}

	// minimal config with vsock enabled
	runtimeMinimalConfig := `
	# Runtime configuration file
	[hypervisor.qemu]
	use_vsock = true
	image = "` + imagePath + `"

	[proxy.kata]
	path = "` + proxyPath + `"

	[shim.kata]
	path = "` + shimPath + `"

	[agent.kata]
`
	orgVHostVSockDevicePath := utils.VHostVSockDevicePath
	orgVSockDevicePath := utils.VSockDevicePath
	defer func() {
		utils.VHostVSockDevicePath = orgVHostVSockDevicePath
		utils.VSockDevicePath = orgVSockDevicePath
	}()
	utils.VHostVSockDevicePath = "/dev/null"
	utils.VSockDevicePath = "/dev/null"

	configPath := path.Join(dir, "runtime.toml")
	err = createConfig(configPath, runtimeMinimalConfig)
	if err != nil {
		t.Fatal(err)
	}

	_, config, err := LoadConfiguration(configPath, false, false)
	if err != nil {
		t.Fatal(err)
	}

	if config.ProxyType != vc.NoProxyType {
		t.Fatalf("Proxy type must be NoProxy, got %+v", config.ProxyType)
	}

	if !reflect.DeepEqual(config.ProxyConfig, vc.ProxyConfig{}) {
		t.Fatalf("Got %+v\n expecting %+v", config.ProxyConfig, vc.ProxyConfig{})
	}

	if config.HypervisorConfig.UseVSock != true {
		t.Fatalf("use_vsock must be true, got %v", config.HypervisorConfig.UseVSock)
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
	enableIOThreads := true
	hotplugVFIOOnRootBus := true
	orgVSockDevicePath := utils.VSockDevicePath
	orgVHostVSockDevicePath := utils.VHostVSockDevicePath
	defer func() {
		utils.VSockDevicePath = orgVSockDevicePath
		utils.VHostVSockDevicePath = orgVHostVSockDevicePath
	}()
	utils.VSockDevicePath = "/dev/abc/xyz"
	utils.VHostVSockDevicePath = "/dev/abc/xyz"

	hypervisor := hypervisor{
		Path:                  hypervisorPath,
		Kernel:                kernelPath,
		Image:                 imagePath,
		MachineType:           machineType,
		DisableBlockDeviceUse: disableBlock,
		EnableIOThreads:       enableIOThreads,
		HotplugVFIOOnRootBus:  hotplugVFIOOnRootBus,
		UseVSock:              true,
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

	// falling back to legacy serial port
	config, err := newQemuHypervisorConfig(hypervisor)
	if err != nil {
		t.Fatal(err)
	}

	utils.VSockDevicePath = "/dev/null"
	utils.VHostVSockDevicePath = "/dev/null"

	// all paths exist now
	config, err = newQemuHypervisorConfig(hypervisor)
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

	if config.EnableIOThreads != enableIOThreads {
		t.Errorf("Expected value for enable IOThreads  %v, got %v", enableIOThreads, config.EnableIOThreads)
	}

	if config.HotplugVFIOOnRootBus != hotplugVFIOOnRootBus {
		t.Errorf("Expected value for HotplugVFIOOnRootBus %v, got %v", hotplugVFIOOnRootBus, config.HotplugVFIOOnRootBus)
	}
}

func TestNewQemuHypervisorConfigImageAndInitrd(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	imagePath := filepath.Join(tmpdir, "image")
	initrdPath := filepath.Join(tmpdir, "initrd")
	hypervisorPath := path.Join(tmpdir, "hypervisor")
	kernelPath := path.Join(tmpdir, "kernel")

	for _, file := range []string{imagePath, initrdPath, hypervisorPath, kernelPath} {
		err = createEmptyFile(file)
		assert.NoError(err)
	}

	machineType := "machineType"
	disableBlock := true
	enableIOThreads := true
	hotplugVFIOOnRootBus := true

	hypervisor := hypervisor{
		Path:                  hypervisorPath,
		Kernel:                kernelPath,
		Image:                 imagePath,
		Initrd:                initrdPath,
		MachineType:           machineType,
		DisableBlockDeviceUse: disableBlock,
		EnableIOThreads:       enableIOThreads,
		HotplugVFIOOnRootBus:  hotplugVFIOOnRootBus,
	}

	_, err = newQemuHypervisorConfig(hypervisor)

	// specifying both an image+initrd is invalid
	assert.Error(err)
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

	numCPUs := goruntime.NumCPU()

	h := hypervisor{}

	assert.Equal(h.machineType(), defaultMachineType, "default hypervisor machine type wrong")
	assert.Equal(h.defaultVCPUs(), defaultVCPUCount, "default vCPU number is wrong")
	assert.Equal(h.defaultMaxVCPUs(), uint32(numCPUs), "default max vCPU number is wrong")
	assert.Equal(h.defaultMemSz(), defaultMemSize, "default memory size is wrong")

	machineType := "foo"
	h.MachineType = machineType
	assert.Equal(h.machineType(), machineType, "custom hypervisor machine type wrong")

	// auto inferring
	h.NumVCPUs = -1
	assert.Equal(h.defaultVCPUs(), uint32(numCPUs), "default vCPU number is wrong")

	h.NumVCPUs = 2
	assert.Equal(h.defaultVCPUs(), uint32(2), "default vCPU number is wrong")

	h.NumVCPUs = int32(numCPUs) + 1
	assert.Equal(h.defaultVCPUs(), uint32(numCPUs), "default vCPU number is wrong")

	h.DefaultMaxVCPUs = 2
	assert.Equal(h.defaultMaxVCPUs(), uint32(2), "default max vCPU number is wrong")

	h.DefaultMaxVCPUs = uint32(numCPUs) + 1
	assert.Equal(h.defaultMaxVCPUs(), uint32(numCPUs), "default max vCPU number is wrong")

	maxvcpus := vc.MaxQemuVCPUs()
	h.DefaultMaxVCPUs = maxvcpus + 1
	assert.Equal(h.defaultMaxVCPUs(), uint32(numCPUs), "default max vCPU number is wrong")

	h.MemorySize = 1024
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

// The default initrd path is not returned by h.initrd()
func TestHypervisorDefaultsInitrd(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	testInitrdPath := filepath.Join(tmpdir, "initrd")
	testInitrdLinkPath := filepath.Join(tmpdir, "initrd-link")

	err = createEmptyFile(testInitrdPath)
	assert.NoError(err)

	err = syscall.Symlink(testInitrdPath, testInitrdLinkPath)
	assert.NoError(err)

	savedInitrdPath := defaultInitrdPath

	defer func() {
		defaultInitrdPath = savedInitrdPath
	}()

	defaultInitrdPath = testInitrdPath
	h := hypervisor{}
	p, err := h.initrd()
	assert.Error(err)
	assert.Equal(p, "", "default Image path wrong")

	// test path resolution
	defaultInitrdPath = testInitrdLinkPath
	h = hypervisor{}
	p, err = h.initrd()
	assert.Error(err)
	assert.Equal(p, "")
}

// The default image path is not returned by h.image()
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
	assert.Error(err)
	assert.Equal(p, "", "default Image path wrong")

	// test path resolution
	defaultImagePath = testImageLinkPath
	h = hypervisor{}
	p, err = h.image()
	assert.Error(err)
	assert.Equal(p, "")
}

func TestHypervisorDefaultsGuestHookPath(t *testing.T) {
	assert := assert.New(t)

	h := hypervisor{}
	guestHookPath := h.guestHookPath()
	assert.Equal(guestHookPath, defaultGuestHookPath, "default guest hook path wrong")

	testGuestHookPath := "/test/guest/hook/path"
	h = hypervisor{
		GuestHookPath: testGuestHookPath,
	}
	guestHookPath = h.guestHookPath()
	assert.Equal(guestHookPath, testGuestHookPath, "custom guest hook path wrong")
}

func TestProxyDefaults(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	testProxyPath := filepath.Join(tmpdir, "proxy")
	testProxyLinkPath := filepath.Join(tmpdir, "proxy-link")

	err = createEmptyFile(testProxyPath)
	assert.NoError(err)

	err = syscall.Symlink(testProxyPath, testProxyLinkPath)
	assert.NoError(err)

	savedProxyPath := defaultProxyPath

	defer func() {
		defaultProxyPath = savedProxyPath
	}()

	defaultProxyPath = testProxyPath
	p := proxy{}
	path, err := p.path()
	assert.NoError(err)
	assert.Equal(path, defaultProxyPath, "default proxy path wrong")

	// test path resolution
	defaultProxyPath = testProxyLinkPath
	p = proxy{}
	path, err = p.path()
	assert.NoError(err)
	assert.Equal(path, testProxyPath)

	assert.False(p.debug())
	p.Debug = true
	assert.True(p.debug())
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

	assert.False(s.trace())
	s.Tracing = true
	assert.True(s.trace())
}

func TestAgentDefaults(t *testing.T) {
	assert := assert.New(t)

	a := agent{}

	assert.Equal(a.debug(), a.Debug)

	a.Debug = true
	assert.Equal(a.debug(), a.Debug)

	assert.Equal(a.trace(), a.Tracing)

	a.Tracing = true
	assert.Equal(a.trace(), a.Tracing)

	assert.Equal(a.traceMode(), a.TraceMode)
	assert.Equal(a.traceType(), a.TraceType)
}

func TestGetDefaultConfigFilePaths(t *testing.T) {
	assert := assert.New(t)

	results := GetDefaultConfigFilePaths()
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

	assert.Equal(defaultAgent, vc.KataContainersAgent)

	config := oci.RuntimeConfig{}

	tomlConf := tomlConfig{
		Agent: map[string]agent{
			// force a non-default value
			kataAgentTableType: {},
		},
	}

	assert.NotEqual(config.AgentType, vc.AgentType(kataAgentTableType))
	assert.NotEqual(config.AgentConfig, vc.KataAgentConfig{})

	err := updateRuntimeConfig("", tomlConf, &config, false)
	assert.NoError(err)

	assert.Equal(config.AgentType, vc.AgentType(kataAgentTableType))
	assert.Equal(config.AgentConfig, vc.KataAgentConfig{})
}

func TestUpdateRuntimeConfigurationVMConfig(t *testing.T) {
	assert := assert.New(t)

	vcpus := uint(2)
	mem := uint32(2048)

	config := oci.RuntimeConfig{}
	expectedVMConfig := mem

	tomlConf := tomlConfig{
		Hypervisor: map[string]hypervisor{
			qemuHypervisorTableType: {
				NumVCPUs:   int32(vcpus),
				MemorySize: mem,
				Path:       "/",
				Kernel:     "/",
				Image:      "/",
				Firmware:   "/",
			},
		},
	}

	err := updateRuntimeConfig("", tomlConf, &config, false)
	assert.NoError(err)

	assert.Equal(expectedVMConfig, config.HypervisorConfig.MemorySize)
}

func TestUpdateRuntimeConfigurationFactoryConfig(t *testing.T) {
	assert := assert.New(t)

	config := oci.RuntimeConfig{}
	expectedFactoryConfig := oci.FactoryConfig{
		Template:        true,
		TemplatePath:    defaultTemplatePath,
		VMCacheEndpoint: defaultVMCacheEndpoint,
	}

	tomlConf := tomlConfig{Factory: factory{Template: true}}

	err := updateRuntimeConfig("", tomlConf, &config, false)
	assert.NoError(err)

	assert.Equal(expectedFactoryConfig, config.FactoryConfig)
}

func TestUpdateRuntimeConfigurationInvalidKernelParams(t *testing.T) {
	assert := assert.New(t)

	assert.Equal(defaultAgent, vc.KataContainersAgent)

	config := oci.RuntimeConfig{}

	tomlConf := tomlConfig{}

	savedFunc := GetKernelParamsFunc
	defer func() {
		GetKernelParamsFunc = savedFunc
	}()

	GetKernelParamsFunc = func(needSystemd, trace bool) []vc.Param {
		return []vc.Param{
			{
				Key:   "",
				Value: "",
			},
		}
	}

	err := updateRuntimeConfig("", tomlConf, &config, false)
	assert.EqualError(err, "Empty kernel parameter")
}

func TestCheckHypervisorConfig(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir(testDir, "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	// Not created on purpose
	imageENOENT := filepath.Join(dir, "image-ENOENT.img")
	initrdENOENT := filepath.Join(dir, "initrd-ENOENT.img")

	imageEmpty := filepath.Join(dir, "image-empty.img")
	initrdEmpty := filepath.Join(dir, "initrd-empty.img")

	for _, file := range []string{imageEmpty, initrdEmpty} {
		err = createEmptyFile(file)
		assert.NoError(err)
	}

	image := filepath.Join(dir, "image.img")
	initrd := filepath.Join(dir, "initrd.img")

	mb := uint32(1024 * 1024)

	fileSizeMB := uint32(3)
	fileSizeBytes := fileSizeMB * mb

	fileData := strings.Repeat("X", int(fileSizeBytes))

	for _, file := range []string{image, initrd} {
		err = WriteFile(file, fileData, testFileMode)
		assert.NoError(err)
	}

	type testData struct {
		imagePath        string
		initrdPath       string
		memBytes         uint32
		expectError      bool
		expectLogWarning bool
	}

	// Note that checkHypervisorConfig() does not check to ensure an image
	// or an initrd has been specified - that's handled by a separate
	// function, hence no test for it here.

	data := []testData{
		{"", "", 0, true, false},

		{imageENOENT, "", 2, true, false},
		{"", initrdENOENT, 2, true, false},

		{imageEmpty, "", 2, true, false},
		{"", initrdEmpty, 2, true, false},

		{image, "", fileSizeMB + 2, false, false},
		{image, "", fileSizeMB + 1, false, false},
		{image, "", fileSizeMB + 0, false, true},
		{image, "", fileSizeMB - 1, false, true},
		{image, "", fileSizeMB - 2, false, true},

		{"", initrd, fileSizeMB + 2, false, false},
		{"", initrd, fileSizeMB + 1, false, false},
		{"", initrd, fileSizeMB + 0, true, false},
		{"", initrd, fileSizeMB - 1, true, false},
		{"", initrd, fileSizeMB - 2, true, false},
	}

	for i, d := range data {
		savedOut := kataUtilsLogger.Logger.Out

		// create buffer to save logger output
		logBuf := &bytes.Buffer{}

		// capture output to buffer
		kataUtilsLogger.Logger.Out = logBuf

		config := vc.HypervisorConfig{
			ImagePath:  d.imagePath,
			InitrdPath: d.initrdPath,
			MemorySize: d.memBytes,
		}

		err := checkHypervisorConfig(config)

		if d.expectError {
			assert.Error(err, "test %d (%+v)", i, d)
		} else {
			assert.NoError(err, "test %d (%+v)", i, d)
		}

		if d.expectLogWarning {
			assert.True(strings.Contains(logBuf.String(), "warning"))
		} else {
			assert.Empty(logBuf.String())
		}

		// reset logger
		kataUtilsLogger.Logger.Out = savedOut
	}
}

func TestCheckNetNsConfig(t *testing.T) {
	assert := assert.New(t)

	config := oci.RuntimeConfig{
		DisableNewNetNs: true,
		NetmonConfig: vc.NetmonConfig{
			Enable: true,
		},
	}
	err := checkNetNsConfig(config)
	assert.Error(err)

	config = oci.RuntimeConfig{
		DisableNewNetNs:   true,
		InterNetworkModel: vc.NetXConnectDefaultModel,
	}
	err = checkNetNsConfig(config)
	assert.Error(err)
}

func TestCheckFactoryConfig(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		factoryEnabled bool
		expectError    bool
		imagePath      string
		initrdPath     string
	}

	data := []testData{
		{false, false, "", ""},
		{false, false, "image", ""},
		{false, false, "", "initrd"},

		{true, false, "", "initrd"},
		{true, true, "image", ""},
	}

	for i, d := range data {
		config := oci.RuntimeConfig{
			HypervisorConfig: vc.HypervisorConfig{
				ImagePath:  d.imagePath,
				InitrdPath: d.initrdPath,
			},

			FactoryConfig: oci.FactoryConfig{
				Template: d.factoryEnabled,
			},
		}

		err := checkFactoryConfig(config)

		if d.expectError {
			assert.Error(err, "test %d (%+v)", i, d)
		} else {
			assert.NoError(err, "test %d (%+v)", i, d)
		}
	}
}

func TestCheckNetNsConfigShimTrace(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		networkModel vc.NetInterworkingModel
		disableNetNs bool
		shimTrace    bool
		expectError  bool
	}

	data := []testData{
		{vc.NetXConnectMacVtapModel, false, false, false},
		{vc.NetXConnectMacVtapModel, false, true, true},
		{vc.NetXConnectMacVtapModel, true, true, true},
		{vc.NetXConnectMacVtapModel, true, false, true},
		{vc.NetXConnectNoneModel, true, false, false},
		{vc.NetXConnectNoneModel, true, true, false},
	}

	for i, d := range data {
		config := oci.RuntimeConfig{
			DisableNewNetNs:   d.disableNetNs,
			InterNetworkModel: d.networkModel,
			ShimConfig: vc.ShimConfig{
				Trace: d.shimTrace,
			},
		}

		err := checkNetNsConfig(config)

		if d.expectError {
			assert.Error(err, "test %d (%+v)", i, d)
		} else {
			assert.NoError(err, "test %d (%+v)", i, d)
		}
	}
}
