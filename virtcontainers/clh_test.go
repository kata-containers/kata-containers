// Copyright (c) 2019 Ericsson Eurolab Deutschland G.m.b.H.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/store"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

//
// Cli helper functions
//

func getCliOption(args []string, key string) (string, error) {
	for i, p := range args {
		if p == key && i < (len(args)-1) {
			return args[i+1], nil
		}
	}
	return "", errors.New("Key not found")
}

func newClhConfig() HypervisorConfig {
	return HypervisorConfig{
		KernelPath:        testClhKernelPath,
		ImagePath:         testClhImagePath,
		HypervisorPath:    testClhPath,
		NumVCPUs:          defaultVCPUs,
		BlockDeviceDriver: config.VirtioBlock,
		MemorySize:        defaultMemSzMiB,
		DefaultBridges:    defaultBridges,
		DefaultMaxVCPUs:   MaxClhVCPUs(),
		// Adding this here, as hypervisorconfig.valid()
		// forcefully adds it even when 9pfs is not supported
		Msize9p:       defaultMsize9p,
		VirtioFSCache: virtioFsCacheAlways,
	}
}

//
// --cmdline <cmdline> Kernel command line
//
func TestClhCliKernelParameters(t *testing.T) {
	assert := assert.New(t)

	expectedOut := "--cmdline foo=foo bar=bar"
	params := []Param{
		{
			Key:   "foo",
			Value: "foo",
		},
		{
			Key:   "bar",
			Value: "bar",
		},
	}

	builder := &DefaultCLIBuilder{}
	builder.AddKernelParameters(params)
	director := &CommandLineDirector{}

	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)

	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), expectedOut)
}

//
// Control (virtio) console: <console>  off|null|tty|file=/path/to/a/file,iommu=on|off [default: tty]
//
func TestClhCliConsole(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultFilePath := "/a/b/c"

	builder.SetConsole(&CLIConsole{
		consoleType: cctOFF,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--console off")

	builder.SetConsole(&CLIConsole{
		consoleType: cctNULL,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--console null")

	builder.SetConsole(&CLIConsole{
		consoleType: cctTTY,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--console tty")

	builder.SetConsole(&CLIConsole{
		consoleType: cctFILE,
		filePath:    defaultFilePath,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--console file="+defaultFilePath+",iommu=off")

	builder.SetConsole(&CLIConsole{
		consoleType: cctFILE,
		filePath:    defaultFilePath,
		iommu:       true,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--console file="+defaultFilePath+",iommu=on")

}

//
// Control serial port: off|null|tty|file=/path/to/a/file [default: tty]
//
func TestClhCliSerial(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultFilePath := "/a/b/c"

	builder.SetSerial(&CLISerialConsole{
		consoleType: cctOFF,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--serial off")

	builder.SetSerial(&CLISerialConsole{
		consoleType: cctNULL,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--serial null")

	builder.SetSerial(&CLISerialConsole{
		consoleType: cctTTY,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--serial tty")

	builder.SetSerial(&CLISerialConsole{
		consoleType: cctFILE,
		filePath:    defaultFilePath,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--serial file="+defaultFilePath)

}

//
//  --api-socket <api-socket> HTTP API socket path (UNIX domain socket). [default: /run/cloud-hypervisor.23605
//
func TestClhCliApiSocket(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultFilePath := "/a/b/c"

	builder.SetAPISocket(&CLIAPISocket{
		socketPath: defaultFilePath,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--api-socket "+defaultFilePath)
}

//
//  --cpus <cpus> Number of virtual CPUs [default: 1]
//
func TestClhCliCpus(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultCPUs := "4"

	builder.SetCpus(&CLICpus{
		cpus: 4,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--cpus "+defaultCPUs)
}

//
// --disk <disk> Disk parameters "path=<disk_image_path>,iommu=on|off"
//
func TestClhCliDisk(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultDiskPath := "/a/b/c.img"

	builder.SetDisk(&CLIDisk{
		path: defaultDiskPath,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--disk path="+defaultDiskPath+",iommu=off")

	builder.SetDisk(&CLIDisk{
		path:  defaultDiskPath,
		iommu: true,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--disk path="+defaultDiskPath+",iommu=on")
}

//
//  --fs <fs> virtio-fs parameters
//            "tag=<tag_name>,sock=<socket_path>,num_queues=<number_of_queues>,queue_size=<size_of_each_queue>,dax=on|off,cache_size=<DAX
//            cache size: default 8Gib>"
//
func TestClhCliFs(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultFsPath := "/a/b/c"
	defaultFsQs := uint32(2)
	defaultFsQss := uint32(1024)
	defaultFsTag := "myTag"
	defaultFsCacheSize := "1Gib"

	builder.SetFs(&CLIFs{
		tag:        defaultFsTag,
		socketPath: defaultFsPath,
		queues:     defaultFsQs,
		queueSize:  defaultFsQss,
		dax:        false,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")),
		"--fs tag="+defaultFsTag+
			",sock="+defaultFsPath+
			",num_queues="+strconv.FormatUint(uint64(defaultFsQs), 10)+
			",queue_size="+strconv.FormatUint(uint64(defaultFsQss), 10))

	builder.SetFs(&CLIFs{
		tag:        defaultFsTag,
		socketPath: defaultFsPath,
		dax:        true,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")),
		"--fs tag="+defaultFsTag+
			",sock="+defaultFsPath+
			",dax=on")

	builder.SetFs(&CLIFs{
		tag:        defaultFsTag,
		socketPath: defaultFsPath,
		dax:        true,
		cacheSize:  defaultFsCacheSize,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")),
		"--fs tag="+defaultFsTag+
			",sock="+defaultFsPath+
			",dax=on,cache_size="+defaultFsCacheSize)

}

//
// --disk <disk> Disk parameters "path=<disk_image_path>,iommu=on|off"
//
func TestClhCliKernel(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultKernel := "/a/b/vmlinuz"

	builder.SetKernel(&CLIKernel{
		path: defaultKernel,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--kernel "+defaultKernel)

}

//
// --log-file <log-file> Log file. Standard error is used if not specified
//
func TestClhCliLogFile(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultPath := "/a/b/clh.log"

	builder.SetLogFile(&CLILogFile{
		path: defaultPath,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--log-file "+defaultPath)

}

//
// --memory <memory> Memory parameters "size=<guest_memory_size>,file=<backing_file_path>"
//
func TestClhCliMemory(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultMemory := uint32(1024)
	defaultFile := "/a/b.shm"

	builder.SetMemory(&CLIMemory{
		memorySize: defaultMemory,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--memory size="+strconv.FormatUint(uint64(defaultMemory), 10)+"M")

	builder.SetMemory(&CLIMemory{
		memorySize:  defaultMemory,
		backingFile: defaultFile,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")),
		"--memory size="+strconv.FormatUint(uint64(defaultMemory), 10)+"M"+
			",file="+defaultFile)

}

//
// --net <net> Network parameters
//       "tap=<if_name>,ip=<ip_addr>,mask=<net_mask>,mac=<mac_addr>,iommu=on|off"
//
func TestClhCliNetwork(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultIfName := "tap1"
	defaultIP := "1.2.3.4"
	defaultNetMask := "255.255.255.0"
	defaultMac := "00:11:22:33:44:55"

	builder.AddNet(CLINet{
		mac: defaultMac,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--net tap=tap1,mac="+defaultMac)

	builder = &DefaultCLIBuilder{}
	builder.AddNet(CLINet{
		device: defaultIfName,
		mac:    defaultMac,
	})

	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--net tap="+defaultIfName+",mac="+defaultMac)

	builder = &DefaultCLIBuilder{}
	builder.AddNet(CLINet{
		device: defaultIfName,
		mac:    defaultMac,
		ip:     defaultIP,
		mask:   defaultNetMask,
	})

	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--net tap="+defaultIfName+
		",ip="+defaultIP+
		",mask="+defaultNetMask+
		",mac="+defaultMac)

	builder.AddNet(CLINet{
		mac:  defaultMac,
		ip:   defaultIP,
		mask: defaultNetMask,
	})

	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")),
		"--net tap="+defaultIfName+
			",ip="+defaultIP+
			",mask="+defaultNetMask+
			",mac="+defaultMac+
			",tap=tap2"+
			",ip="+defaultIP+
			",mask="+defaultNetMask+
			",mac="+defaultMac)
}

//
// --rng <rng>  Random number generator parameters "src=<entropy_source_path>,iommu=on|off"
//
func TestClhCliRng(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultSource := "/a/b/c.random"

	builder.SetRng(&CLIRng{
		src: defaultSource,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--rng src="+defaultSource+",iommu=off")

	builder.SetRng(&CLIRng{
		src:   defaultSource,
		iommu: true,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")), "--rng src="+defaultSource+",iommu=on")
}

//
// --vsock <vsock> Virtio VSOCK parameters "cid=<context_id>,sock=<socket_path>,iommu=on|off"
//
func TestClhCliVsock(t *testing.T) {
	assert := assert.New(t)
	builder := &DefaultCLIBuilder{}
	director := &CommandLineDirector{}
	defaultSocket := "/a/b/c.sock"
	defaultCid := uint32(12345)

	builder.SetVsock(&CLIVsock{
		cid:        defaultCid,
		socketPath: defaultSocket,
	})
	cli, err := director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")),
		"--vsock cid="+strconv.FormatUint(uint64(defaultCid), 10)+
			",sock="+defaultSocket+
			",iommu=off")

	builder.SetVsock(&CLIVsock{
		cid:        defaultCid,
		socketPath: defaultSocket,
		iommu:      true,
	})
	cli, err = director.Build(builder)
	assert.NotNil(cli.args)
	assert.Nil(err)
	assert.Equal(strings.TrimSpace(strings.Join(cli.args, " ")),
		"--vsock cid="+strconv.FormatUint(uint64(defaultCid), 10)+
			",sock="+defaultSocket+
			",iommu=on")

}

func TestClhCreateSandbox(t *testing.T) {
	clhConfig := newClhConfig()
	clh := &cloudHypervisor{
		config: clhConfig,
	}
	assert := assert.New(t)

	sandbox := &Sandbox{
		ctx: context.Background(),
		id:  "testSandbox",
		config: &SandboxConfig{
			HypervisorConfig: clhConfig,
		},
	}

	vcStore, err := store.NewVCSandboxStore(sandbox.ctx, sandbox.id)
	assert.NoError(err)

	sandbox.store = vcStore

	// Create the hypervisor fake binary
	testClhPath := filepath.Join(testDir, testHypervisor)
	_, err = os.Create(testClhPath)
	assert.NoError(err)

	// Create parent dir path for hypervisor.json
	parentDir := store.SandboxConfigurationRootPath(sandbox.id)
	assert.NoError(os.MkdirAll(parentDir, store.DirMode))

	err = clh.createSandbox(context.Background(), sandbox.id, NetworkNamespace{}, &sandbox.config.HypervisorConfig, sandbox.store, false)
	assert.NoError(err)
	assert.NoError(os.RemoveAll(parentDir))
	assert.Exactly(clhConfig, clh.config)
}

func TestClhAddDeviceNet(t *testing.T) {
	defaultEndpointName := "tap1"
	defaultMac := "55:44:33:22:11:00"
	assert := assert.New(t)
	clh := &cloudHypervisor{
		ctx:        context.Background(),
		cliBuilder: &DefaultCLIBuilder{},
	}

	tep, _ := createTapNetworkEndpoint(0, defaultEndpointName)
	tep.TapInterface.TAPIface.HardAddr = defaultMac

	err := clh.addDevice(Endpoint(tep), netDev)
	assert.NoError(err)

	director := &CommandLineDirector{}

	cli, err := director.Build(clh.cliBuilder)
	assert.NoError(err)
	assert.NotNil(cli)

	netarg, err := getCliOption(cli.args, "--net")
	assert.NoError(err)
	assert.Equal(netarg, "tap="+defaultEndpointName+",mac="+defaultMac)

}

func TestClhAddDeviceVSock(t *testing.T) {
	defaultCid := uint64(12345)
	defaultUdsPath := "/a/b/c"
	defaultPort := uint32(1024)
	assert := assert.New(t)
	clh := &cloudHypervisor{
		ctx:        context.Background(),
		cliBuilder: &DefaultCLIBuilder{},
	}

	vsock := types.HybridVSock{
		UdsPath:   defaultUdsPath,
		ContextID: defaultCid,
		Port:      defaultPort,
	}

	err := clh.addDevice(vsock, netDev)
	assert.NoError(err)

	director := &CommandLineDirector{}

	cli, err := director.Build(clh.cliBuilder)
	assert.NoError(err)
	assert.NotNil(cli)

	netarg, err := getCliOption(cli.args, "--vsock")
	assert.NoError(err)
	assert.Equal(netarg, "cid="+strconv.FormatUint(defaultCid, 10)+",sock="+defaultUdsPath+",iommu=off")

}

func TestClhCapabilities(t *testing.T) {
	assert := assert.New(t)
	clh := &cloudHypervisor{
		ctx:        context.Background(),
		cliBuilder: &DefaultCLIBuilder{},
	}

	caps := clh.capabilities()

	assert.False(caps.IsBlockDeviceHotplugSupported())
	assert.True(caps.IsFsSharingSupported())
	assert.False(caps.IsMultiQueueSupported())
}

func TestClhGenerateSocket(t *testing.T) {
	defaultID := "123-456-99"
	defaultPort := uint32(1024)

	assert := assert.New(t)
	clh := &cloudHypervisor{
		ctx:        context.Background(),
		cliBuilder: &DefaultCLIBuilder{},
	}

	rtnval, err := clh.generateSocket(defaultID, true)
	assert.NoError(err)
	assert.NotNil(rtnval)

	if socket, ok := rtnval.(types.HybridVSock); ok {
		assert.Equal(socket.UdsPath, "/run/vc/vm/"+defaultID+"/"+clhSocket)
		assert.Equal(socket.Port, defaultPort)
		assert.NotEqual(socket.ContextID, 0, "ContextID 0 is reserved for the hypervisor communication")
		assert.NotEqual(socket.ContextID, 1, "ContextID 1 is reserved")
		assert.NotEqual(socket.ContextID, 2, "ContextID 2 is reserved for the host communication")
		assert.NotEqual(socket.ContextID, 0xffffffff, "ContextID 0xffffffff is reserved")
	} else {
		t.Fail()
	}

}

func TestClhReset(t *testing.T) {
	assert := assert.New(t)
	clh := &cloudHypervisor{
		ctx:        context.Background(),
		cliBuilder: &DefaultCLIBuilder{},
	}

	clh.state.PID = 10
	clh.state.VirtiofsdPID = 11
	clh.state.state = clhReady

	clh.reset()

	assert.Equal(clh.state.PID, 0)
	assert.Equal(clh.state.VirtiofsdPID, 0)
	assert.Equal(clh.state.state, clhNotReady)

}

func TestClhVirtiofsdArgs(t *testing.T) {
	assert := assert.New(t)

	defaultSocketPath := "/a/b/c/doit.sock"

	clhConfig := newClhConfig()
	clh := &cloudHypervisor{
		ctx:        context.Background(),
		cliBuilder: &DefaultCLIBuilder{},
		config:     clhConfig,
	}

	args, err := clh.virtiofsdArgs(defaultSocketPath)
	assert.NoError(err)
	assert.Equal(strings.Join(args, " "), "-f -o vhost_user_socket="+defaultSocketPath+" -o source=/run/kata-containers/shared/sandboxes -o cache="+virtioFsCacheAlways)

}

func TestClhPath(t *testing.T) {
	assert := assert.New(t)

	clhConfig := newClhConfig()
	clh := &cloudHypervisor{
		ctx:        context.Background(),
		cliBuilder: &DefaultCLIBuilder{},
		config:     clhConfig,
	}

	defaultPath, _ := clh.config.HypervisorAssetPath()

	path, err := clh.clhPath()
	assert.NoError(err)
	assert.Equal(path, defaultPath)

}
