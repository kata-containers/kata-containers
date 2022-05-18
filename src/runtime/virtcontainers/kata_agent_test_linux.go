package virtcontainers

import (
	"bufio"
	"context"
	"io/ioutil"
	"runtime"

	"code.cloudfoundry.org/bytefmt"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/mock"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
)

func TestAgentCreateContainer(t *testing.T) {
	assert := assert.New(t)

	sandbox := &Sandbox{
		ctx: context.Background(),
		id:  "foobar",
		config: &SandboxConfig{
			ID:             "foobar",
			HypervisorType: MockHypervisor,
			HypervisorConfig: HypervisorConfig{
				KernelPath: "foo",
				ImagePath:  "bar",
			},
		},
		hypervisor: &mockHypervisor{},
	}

	fsShare, err := NewFilesystemShare(sandbox)
	assert.Nil(err)
	sandbox.fsShare = fsShare

	store, err := persist.GetDriver()
	assert.NoError(err)
	assert.NotNil(store)
	sandbox.store = store

	container := &Container{
		ctx:       sandbox.ctx,
		id:        "barfoo",
		sandboxID: "foobar",
		sandbox:   sandbox,
		state: types.ContainerState{
			Fstype: "xfs",
		},
		config: &ContainerConfig{
			CustomSpec:  &specs.Spec{},
			Annotations: map[string]string{},
		},
	}

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	dir := t.TempDir()

	err = k.configure(context.Background(), &mockHypervisor{}, sandbox.id, dir, KataAgentConfig{})
	assert.Nil(err)

	// We'll fail on container metadata file creation, but it helps increasing coverage...
	_, err = k.createContainer(context.Background(), sandbox, container)
	assert.Error(err)
}

func TestKataAgentConnect(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	err = k.connect(context.Background())
	assert.NoError(err)
	assert.NotNil(k.client)
}

func TestKataAgentDisconnect(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	assert.NoError(k.connect(context.Background()))
	assert.NoError(k.disconnect(context.Background()))
	assert.Nil(k.client)
}

var reqList = []interface{}{
	&pb.CreateSandboxRequest{},
	&pb.DestroySandboxRequest{},
	&pb.ExecProcessRequest{},
	&pb.CreateContainerRequest{},
	&pb.StartContainerRequest{},
	&pb.RemoveContainerRequest{},
	&pb.SignalProcessRequest{},
	&pb.CheckRequest{},
	&pb.WaitProcessRequest{},
	&pb.StatsContainerRequest{},
	&pb.SetGuestDateTimeRequest{},
}

func TestKataAgentSendReq(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	ctx := context.Background()

	for _, req := range reqList {
		_, err = k.sendReq(ctx, req)
		assert.Nil(err)
	}

	sandbox := &Sandbox{}
	container := &Container{}
	execid := "processFooBar"

	err = k.startContainer(ctx, sandbox, container)
	assert.Nil(err)

	err = k.signalProcess(ctx, container, execid, syscall.SIGKILL, true)
	assert.Nil(err)

	err = k.winsizeProcess(ctx, container, execid, 100, 200)
	assert.Nil(err)

	err = k.updateContainer(ctx, sandbox, Container{}, specs.LinuxResources{})
	assert.Nil(err)

	err = k.pauseContainer(ctx, sandbox, Container{})
	assert.Nil(err)

	err = k.resumeContainer(ctx, sandbox, Container{})
	assert.Nil(err)

	err = k.onlineCPUMem(ctx, 1, true)
	assert.Nil(err)

	_, err = k.statsContainer(ctx, sandbox, Container{})
	assert.Nil(err)

	err = k.check(ctx)
	assert.Nil(err)

	_, err = k.waitProcess(ctx, container, execid)
	assert.Nil(err)

	_, err = k.writeProcessStdin(ctx, container, execid, []byte{'c'})
	assert.Nil(err)

	err = k.closeProcessStdin(ctx, container, execid)
	assert.Nil(err)

	_, err = k.readProcessStdout(ctx, container, execid, []byte{})
	assert.Nil(err)

	_, err = k.readProcessStderr(ctx, container, execid, []byte{})
	assert.Nil(err)

	_, err = k.getOOMEvent(ctx)
	assert.Nil(err)
}

func TestHandleHugepages(t *testing.T) {
	if os.Getuid() != 0 {
		t.Skip("Test disabled as requires root user")
	}

	assert := assert.New(t)

	dir := t.TempDir()

	k := kataAgent{}
	var formattedSizes []string
	var mounts []specs.Mount
	var hugepageLimits []specs.LinuxHugepageLimit

	// On s390x, hugepage sizes must be set at boot and cannot be created ad hoc. Use any that
	// are present (default is 1M, can only be changed on LPAR). See
	// https://www.ibm.com/docs/en/linuxonibm/pdf/lku5dd05.pdf, p. 345 for more information.
	if runtime.GOARCH == "s390x" {
		dirs, err := ioutil.ReadDir(sysHugepagesDir)
		assert.Nil(err)
		for _, dir := range dirs {
			formattedSizes = append(formattedSizes, strings.TrimPrefix(dir.Name(), "hugepages-"))
		}
	} else {
		formattedSizes = []string{"1G", "2M"}
	}

	for _, formattedSize := range formattedSizes {
		bytes, err := bytefmt.ToBytes(formattedSize)
		assert.Nil(err)
		hugepageLimits = append(hugepageLimits, specs.LinuxHugepageLimit{
			Pagesize: formattedSize,
			Limit:    1_000_000 * bytes,
		})

		target := path.Join(dir, fmt.Sprintf("hugepages-%s", formattedSize))
		err = os.MkdirAll(target, 0777)
		assert.NoError(err, "Unable to create dir %s", target)

		err = syscall.Mount("nodev", target, "hugetlbfs", uintptr(0), fmt.Sprintf("pagesize=%s", formattedSize))
		assert.NoError(err, "Unable to mount %s", target)

		defer syscall.Unmount(target, 0)
		defer os.RemoveAll(target)
		mount := specs.Mount{
			Type:   KataLocalDevType,
			Source: target,
		}
		mounts = append(mounts, mount)
	}

	hugepages, err := k.handleHugepages(mounts, hugepageLimits)

	assert.NoError(err, "Unable to handle hugepages %v", hugepageLimits)
	assert.NotNil(hugepages)
	assert.Equal(len(hugepages), len(formattedSizes))

}

func TestAgentNetworkOperation(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	_, err = k.updateInterface(k.ctx, nil)
	assert.Nil(err)

	_, err = k.listInterfaces(k.ctx)
	assert.Nil(err)

	_, err = k.updateRoutes(k.ctx, []*pbTypes.Route{})
	assert.Nil(err)

	_, err = k.listRoutes(k.ctx)
	assert.Nil(err)
}

func TestKataCopyFile(t *testing.T) {
	assert := assert.New(t)

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	k := &kataAgent{
		ctx: context.Background(),
		state: KataAgentState{
			URL: url,
		},
	}

	err = k.copyFile(context.Background(), "/abc/xyz/123", "/tmp")
	assert.Error(err)

	src, err := os.CreateTemp("", "src")
	assert.NoError(err)
	defer os.Remove(src.Name())

	data := []byte("abcdefghi123456789")
	_, err = src.Write(data)
	assert.NoError(err)
	assert.NoError(src.Close())

	dst, err := os.CreateTemp("", "dst")
	assert.NoError(err)
	assert.NoError(dst.Close())
	defer os.Remove(dst.Name())

	orgGrpcMaxDataSize := grpcMaxDataSize
	grpcMaxDataSize = 1
	defer func() {
		grpcMaxDataSize = orgGrpcMaxDataSize
	}()

	err = k.copyFile(context.Background(), src.Name(), dst.Name())
	assert.NoError(err)
}

func TestKataAgentDirs(t *testing.T) {
	assert := assert.New(t)

	uidmapFile, err := os.OpenFile("/proc/self/uid_map", os.O_RDONLY, 0)
	assert.NoError(err)

	line, err := bufio.NewReader(uidmapFile).ReadBytes('\n')
	assert.NoError(err)

	uidmap := strings.Fields(string(line))
	expectedRootless := (uidmap[0] == "0" && uidmap[1] != "0")
	assert.Equal(expectedRootless, rootless.IsRootless())
	if expectedRootless {
		assert.Equal(kataHostSharedDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataHostSharedDir)
		assert.Equal(kataGuestSharedDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataGuestSharedDir)
		assert.Equal(kataGuestSandboxDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataGuestSandboxDir)
		assert.Equal(ephemeralPath(), os.Getenv("XDG_RUNTIME_DIR")+defaultEphemeralPath)
		assert.Equal(kataGuestNydusRootDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataGuestNydusRootDir)
		assert.Equal(kataGuestNydusImageDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataGuestNydusRootDir+"images"+"/")
		assert.Equal(kataGuestSharedDir(), os.Getenv("XDG_RUNTIME_DIR")+defaultKataGuestNydusRootDir+"containers"+"/")
	} else {
		assert.Equal(kataHostSharedDir(), defaultKataHostSharedDir)
		assert.Equal(kataGuestSharedDir(), defaultKataGuestSharedDir)
		assert.Equal(kataGuestSandboxDir(), defaultKataGuestSandboxDir)
		assert.Equal(ephemeralPath(), defaultEphemeralPath)
		assert.Equal(kataGuestNydusRootDir(), defaultKataGuestNydusRootDir)
		assert.Equal(kataGuestNydusImageDir(), defaultKataGuestNydusRootDir+"rafs"+"/")
		assert.Equal(kataGuestSharedDir(), defaultKataGuestNydusRootDir+"containers"+"/")
	}

	cid := "123"
	expected := "/rafs/123/lowerdir"
	assert.Equal(rafsMountPath(cid), expected)
}
