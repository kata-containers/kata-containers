package virtcontainers

import (
	"golang.org/x/sys/unix"
	"syscall"

	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
)

func TestCreateEmptySandbox(t *testing.T) {
	_, err := testCreateSandbox(t, testSandboxID, MockHypervisor, HypervisorConfig{}, NetworkConfig{}, nil, nil)
	assert.Error(t, err)
	defer cleanUp()
}

func TestCreateEmptyHypervisorSandbox(t *testing.T) {
	_, err := testCreateSandbox(t, testSandboxID, QemuHypervisor, HypervisorConfig{}, NetworkConfig{}, nil, nil)
	assert.Error(t, err)
	defer cleanUp()
}
func TestCreateMockSandbox(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)
	_, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, nil, nil)
	assert.NoError(t, err)
	defer cleanUp()
}

func TestSandboxCreationFromConfigRollbackFromCreateSandbox(t *testing.T) {
	defer cleanUp()
	assert := assert.New(t)
	ctx := context.Background()
	hConf := newHypervisorConfig(nil, nil)
	sConf := SandboxConfig{
		ID:               testSandboxID,
		HypervisorType:   QemuHypervisor,
		HypervisorConfig: hConf,
		NetworkConfig:    NetworkConfig{},
		Volumes:          nil,
		Containers:       nil,
	}

	// Ensure hypervisor doesn't exist
	assert.NoError(os.Remove(hConf.HypervisorPath))

	_, err := createSandboxFromConfig(ctx, sConf, nil)
	// Fail at createSandbox: QEMU path does not exist, it is expected. Then rollback is called
	assert.Error(err)

	// Check dirs
	err = checkSandboxRemains()
	assert.NoError(err)
}

func TestSandboxUpdateResources(t *testing.T) {
	contConfig1 := newTestContainerConfigNoop("cont-00001")
	contConfig2 := newTestContainerConfigNoop("cont-00002")
	hConfig := newHypervisorConfig(nil, nil)

	defer cleanUp()
	// create a sandbox
	s, err := testCreateSandbox(t,
		testSandboxID,
		MockHypervisor,
		hConfig,
		NetworkConfig{},
		[]ContainerConfig{contConfig1, contConfig2},
		nil)

	assert.NoError(t, err)
	err = s.updateResources(context.Background())
	assert.NoError(t, err)

	containerMemLimit := int64(1000)
	containerCPUPeriod := uint64(1000)
	containerCPUQouta := int64(5)
	for _, c := range s.config.Containers {
		c.Resources.Memory = &specs.LinuxMemory{
			Limit: new(int64),
		}
		c.Resources.CPU = &specs.LinuxCPU{
			Period: new(uint64),
			Quota:  new(int64),
		}
		c.Resources.Memory.Limit = &containerMemLimit
		c.Resources.CPU.Period = &containerCPUPeriod
		c.Resources.CPU.Quota = &containerCPUQouta
	}
	err = s.updateResources(context.Background())
	assert.NoError(t, err)
}

func TestDeleteStoreWhenNewContainerFail(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)
	p, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	contID := "999"
	contConfig := newTestContainerConfigNoop(contID)
	contConfig.DeviceInfos = []config.DeviceInfo{
		{
			ContainerPath: "",
			DevType:       "",
		},
	}
	_, err = newContainer(context.Background(), p, &contConfig)
	assert.NotNil(t, err, "New container with invalid device info should fail")
	storePath := filepath.Join(p.store.RunStoragePath(), testSandboxID, contID)
	_, err = os.Stat(storePath)
	assert.NotNil(t, err, "Should delete configuration root after failed to create a container")
}

func TestMonitor(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	_, err = s.Monitor(context.Background())
	assert.NotNil(t, err, "Monitoring non-running container should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, err = s.Monitor(context.Background())
	assert.Nil(t, err, "Monitor sandbox failed: %v", err)

	_, err = s.Monitor(context.Background())
	assert.Nil(t, err, "Monitor sandbox again failed: %v", err)

	s.monitor.stop()
}

func TestWaitProcess(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	_, err = s.WaitProcess(context.Background(), contID, execID)
	assert.NotNil(t, err, "Wait process in stopped sandbox should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, err = s.WaitProcess(context.Background(), contID, execID)
	assert.NotNil(t, err, "Wait process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.WaitProcess(context.Background(), contID, execID)
	assert.Nil(t, err, "Wait process in ready container failed: %v", err)

	_, err = s.StartContainer(context.Background(), contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	_, err = s.WaitProcess(context.Background(), contID, execID)
	assert.Nil(t, err, "Wait process failed: %v", err)
}

func TestSignalProcess(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	err = s.SignalProcess(context.Background(), contID, execID, syscall.SIGKILL, true)
	assert.NotNil(t, err, "Wait process in stopped sandbox should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	err = s.SignalProcess(context.Background(), contID, execID, syscall.SIGKILL, false)
	assert.NotNil(t, err, "Wait process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	err = s.SignalProcess(context.Background(), contID, execID, syscall.SIGKILL, true)
	assert.Nil(t, err, "Wait process in ready container failed: %v", err)

	_, err = s.StartContainer(context.Background(), contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	err = s.SignalProcess(context.Background(), contID, execID, syscall.SIGKILL, false)
	assert.Nil(t, err, "Wait process failed: %v", err)
}

func TestWinsizeProcess(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	err = s.WinsizeProcess(context.Background(), contID, execID, 100, 200)
	assert.NotNil(t, err, "Winsize process in stopped sandbox should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	err = s.WinsizeProcess(context.Background(), contID, execID, 100, 200)
	assert.NotNil(t, err, "Winsize process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	err = s.WinsizeProcess(context.Background(), contID, execID, 100, 200)
	assert.Nil(t, err, "Winsize process in ready container failed: %v", err)

	_, err = s.StartContainer(context.Background(), contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	err = s.WinsizeProcess(context.Background(), contID, execID, 100, 200)
	assert.Nil(t, err, "Winsize process failed: %v", err)
}

func TestContainerProcessIOStream(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "foo"
	execID := "bar"
	_, _, _, err = s.IOStream(contID, execID)
	assert.NotNil(t, err, "Winsize process in stopped sandbox should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, _, _, err = s.IOStream(contID, execID)
	assert.NotNil(t, err, "Winsize process in non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, _, _, err = s.IOStream(contID, execID)
	assert.Nil(t, err, "Winsize process in ready container failed: %v", err)

	_, err = s.StartContainer(context.Background(), contID)
	assert.Nil(t, err, "Start container failed: %v", err)

	_, _, _, err = s.IOStream(contID, execID)
	assert.Nil(t, err, "Winsize process failed: %v", err)
}

func TestCreateContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	assert.Equal(t, len(s.config.Containers), 1, "Container config list length from sandbox structure should be 1")

	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.NotNil(t, err, "Should failed to create a duplicated container")
	assert.Equal(t, len(s.config.Containers), 1, "Container config list length from sandbox structure should be 1")
}

func TestDeleteContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	_, err = s.DeleteContainer(context.Background(), contID)
	assert.NotNil(t, err, "Deletng non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.DeleteContainer(context.Background(), contID)
	assert.Nil(t, err, "Failed to delete container %s in sandbox %s: %v", contID, s.ID(), err)
}

func TestStartContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	_, err = s.StartContainer(context.Background(), contID)
	assert.NotNil(t, err, "Starting non-existing container should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.StartContainer(context.Background(), contID)
	assert.Nil(t, err, "Start container failed: %v", err)
}

func TestStatusContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	_, err = s.StatusContainer(contID)
	assert.NotNil(t, err, "Status non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, err = s.StatusContainer(contID)
	assert.Nil(t, err, "Status container failed: %v", err)

	_, err = s.DeleteContainer(context.Background(), contID)
	assert.Nil(t, err, "Failed to delete container %s in sandbox %s: %v", contID, s.ID(), err)
}

func TestStatusSandbox(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	s.Status()
}

func TestEnterContainer(t *testing.T) {
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, newHypervisorConfig(nil, nil), NetworkConfig{}, nil, nil)
	assert.Nil(t, err, "VirtContainers should not allow empty sandboxes")
	defer cleanUp()

	contID := "999"
	cmd := types.Cmd{}
	_, _, err = s.EnterContainer(context.Background(), contID, cmd)
	assert.NotNil(t, err, "Entering non-existing container should fail")

	contConfig := newTestContainerConfigNoop(contID)
	_, err = s.CreateContainer(context.Background(), contConfig)
	assert.Nil(t, err, "Failed to create container %+v in sandbox %+v: %v", contConfig, s, err)

	_, _, err = s.EnterContainer(context.Background(), contID, cmd)
	assert.NotNil(t, err, "Entering non-running container should fail")

	err = s.Start(context.Background())
	assert.Nil(t, err, "Failed to start sandbox: %v", err)

	_, _, err = s.EnterContainer(context.Background(), contID, cmd)
	assert.Nil(t, err, "Enter container failed: %v", err)
}

func TestContainerStateSetFstype(t *testing.T) {
	var err error
	assert := assert.New(t)

	containers := []ContainerConfig{
		{
			ID:          "100",
			Annotations: containerAnnotations,
			CustomSpec:  newEmptySpec(),
		},
	}

	hConfig := newHypervisorConfig(nil, nil)
	sandbox, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, containers, nil)
	assert.Nil(err)
	defer cleanUp()

	c := sandbox.GetContainer("100")
	assert.NotNil(c)

	cImpl, ok := c.(*Container)
	assert.True(ok)

	state := types.ContainerState{
		State:  "ready",
		Fstype: "vfs",
	}

	cImpl.state = state

	newFstype := "ext4"
	err = cImpl.setStateFstype(newFstype)
	assert.NoError(err)
	assert.Equal(cImpl.state.Fstype, newFstype)
}
func TestSandboxGetContainer(t *testing.T) {
	assert := assert.New(t)

	emptySandbox := Sandbox{}
	_, err := emptySandbox.findContainer("")
	assert.Error(err)

	_, err = emptySandbox.findContainer("foo")
	assert.Error(err)

	hConfig := newHypervisorConfig(nil, nil)
	p, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, nil, nil)
	assert.NoError(err)
	defer cleanUp()

	contID := "999"
	contConfig := newTestContainerConfigNoop(contID)
	nc, err := newContainer(context.Background(), p, &contConfig)
	assert.NoError(err)

	err = p.addContainer(nc)
	assert.NoError(err)

	got := false
	for _, c := range p.GetAllContainers() {
		c2, err := p.findContainer(c.ID())
		assert.NoError(err)
		assert.Equal(c2.ID(), c.ID())

		if c2.ID() == contID {
			got = true
		}
	}

	assert.True(got)
}

func TestSandboxSetSandboxAndContainerState(t *testing.T) {
	contID := "505"
	contConfig := newTestContainerConfigNoop(contID)
	assert := assert.New(t)

	configDir, err := writeContainerConfig(t)
	assert.NoError(err)

	// set bundle path annotation, fetchSandbox need this annotation to get containers
	contConfig.Annotations[vcAnnotations.BundlePathKey] = configDir

	hConfig := newHypervisorConfig(nil, nil)

	// create a sandbox
	p, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, []ContainerConfig{contConfig}, nil)
	assert.NoError(err)
	defer cleanUp()

	l := len(p.GetAllContainers())
	assert.Equal(l, 1)

	initialSandboxState := types.SandboxState{
		State: types.StateReady,
	}

	// After a sandbox creation, a container has a READY state
	initialContainerState := types.ContainerState{
		State: types.StateReady,
	}

	c, err := p.findContainer(contID)
	assert.NoError(err)

	// Check initial sandbox and container states
	if err := testCheckInitSandboxAndContainerStates(p, initialSandboxState, c, initialContainerState); err != nil {
		t.Error(err)
	}

	// persist to disk
	err = p.storeSandbox(p.ctx)
	assert.NoError(err)

	newSandboxState := types.SandboxState{
		State: types.StateRunning,
	}

	if err := testForceSandboxStateChangeAndCheck(t, p, newSandboxState); err != nil {
		t.Error(err)
	}

	newContainerState := types.ContainerState{
		State: types.StateStopped,
	}

	if err := testForceContainerStateChangeAndCheck(t, p, c, newContainerState); err != nil {
		t.Error(err)
	}

	// force state to be read from disk
	p2, err := fetchSandbox(context.Background(), p.ID())
	assert.NoError(err)

	if err := testCheckSandboxOnDiskState(p2, newSandboxState); err != nil {
		t.Error(err)
	}

	c2, err := p2.findContainer(contID)
	assert.NoError(err)

	if err := testCheckContainerOnDiskState(c2, newContainerState); err != nil {
		t.Error(err)
	}

	// revert sandbox state to allow it to be deleted
	err = p.setSandboxState(initialSandboxState.State)
	assert.NoError(err)

	// clean up
	err = p.Delete(context.Background())
	assert.NoError(err)
}

func TestCreateSandboxEmptyID(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)
	_, err := testCreateSandbox(t, "", MockHypervisor, hConfig, NetworkConfig{}, nil, nil)
	assert.Error(t, err)
	defer cleanUp()
}
func TestSandboxAttachDevicesVhostUserBlk(t *testing.T) {
	rootEnabled := true
	tc := ktu.NewTestConstraint(false)
	if tc.NotValid(ktu.NeedRoot()) {
		rootEnabled = false
	}

	tmpDir := t.TempDir()
	os.RemoveAll(tmpDir)
	dm := manager.NewDeviceManager(config.VirtioSCSI, true, tmpDir, nil)

	vhostUserDevNodePath := filepath.Join(tmpDir, "/block/devices/")
	vhostUserSockPath := filepath.Join(tmpDir, "/block/sockets/")
	deviceNodePath := filepath.Join(vhostUserDevNodePath, "vhostblk0")
	deviceSockPath := filepath.Join(vhostUserSockPath, "vhostblk0")

	err := os.MkdirAll(vhostUserDevNodePath, dirMode)
	assert.Nil(t, err)
	err = os.MkdirAll(vhostUserSockPath, dirMode)
	assert.Nil(t, err)
	_, err = os.Create(deviceSockPath)
	assert.Nil(t, err)

	// mknod requires root privilege, call mock function for non-root to
	// get VhostUserBlk device type.
	if rootEnabled == true {
		err = unix.Mknod(deviceNodePath, unix.S_IFBLK, int(unix.Mkdev(config.VhostUserBlkMajor, 0)))
		assert.Nil(t, err)
	} else {
		savedFunc := config.GetVhostUserNodeStatFunc

		_, err = os.Create(deviceNodePath)
		assert.Nil(t, err)

		config.GetVhostUserNodeStatFunc = func(devNodePath string,
			devNodeStat *unix.Stat_t) error {
			if deviceNodePath != devNodePath {
				return fmt.Errorf("mock GetVhostUserNodeStatFunc error")
			}

			devNodeStat.Rdev = unix.Mkdev(config.VhostUserBlkMajor, 0)
			return nil
		}

		defer func() {
			config.GetVhostUserNodeStatFunc = savedFunc
		}()
	}

	path := "/dev/vda"
	deviceInfo := config.DeviceInfo{
		HostPath:      deviceNodePath,
		ContainerPath: path,
		DevType:       "b",
		Major:         config.VhostUserBlkMajor,
		Minor:         0,
	}

	device, err := dm.NewDevice(deviceInfo)
	assert.Nil(t, err)
	_, ok := device.(*drivers.VhostUserBlkDevice)
	assert.True(t, ok)

	c := &Container{
		id: "100",
		devices: []ContainerDevice{
			{
				ID:            device.DeviceID(),
				ContainerPath: path,
			},
		},
	}

	containers := map[string]*Container{}
	containers[c.id] = c

	sandbox := Sandbox{
		id:         "100",
		containers: containers,
		hypervisor: &mockHypervisor{},
		devManager: dm,
		ctx:        context.Background(),
		config:     &SandboxConfig{},
	}

	containers[c.id].sandbox = &sandbox

	err = containers[c.id].attachDevices(context.Background())
	assert.Nil(t, err, "Error while attaching vhost-user-blk devices %s", err)

	err = containers[c.id].detachDevices(context.Background())
	assert.Nil(t, err, "Error while detaching vhost-user-blk devices %s", err)
}
func TestSandboxHugepageLimit(t *testing.T) {
	contConfig1 := newTestContainerConfigNoop("cont-00001")
	contConfig2 := newTestContainerConfigNoop("cont-00002")
	limit := int64(4000)
	contConfig1.Resources.Memory = &specs.LinuxMemory{Limit: &limit}
	contConfig2.Resources.Memory = &specs.LinuxMemory{Limit: &limit}
	hConfig := newHypervisorConfig(nil, nil)

	defer cleanUp()
	// create a sandbox
	s, err := testCreateSandbox(t,
		testSandboxID,
		MockHypervisor,
		hConfig,
		NetworkConfig{},
		[]ContainerConfig{contConfig1, contConfig2},
		nil)

	assert.NoError(t, err)

	hugepageLimits := []specs.LinuxHugepageLimit{
		{
			Pagesize: "1GB",
			Limit:    322122547,
		},
		{
			Pagesize: "2MB",
			Limit:    134217728,
		},
	}

	for i := range s.config.Containers {
		s.config.Containers[i].Resources.HugepageLimits = hugepageLimits
	}
	err = s.updateResources(context.Background())
	assert.NoError(t, err)
}
