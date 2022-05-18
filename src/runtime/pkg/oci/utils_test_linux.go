package oci

import (
	"runtime"
	"strconv"

	"golang.org/x/sys/unix"
)

func TestGetShmSizeBindMounted(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip("Test disabled as requires root privileges")
	}

	dir := t.TempDir()

	shmPath := filepath.Join(dir, "shm")
	err := os.Mkdir(shmPath, 0700)
	assert.Nil(t, err)

	size := 8192
	if runtime.GOARCH == "ppc64le" {
		// PAGE_SIZE on ppc64le is 65536
		size = 65536
	}

	shmOptions := "mode=1777,size=" + strconv.Itoa(size)
	err = unix.Mount("shm", shmPath, "tmpfs", unix.MS_NOEXEC|unix.MS_NOSUID|unix.MS_NODEV, shmOptions)
	assert.Nil(t, err)

	defer unix.Unmount(shmPath, 0)

	containerConfig := vc.ContainerConfig{
		Mounts: []vc.Mount{
			{
				Source:      shmPath,
				Destination: "/dev/shm",
				Type:        "bind",
				Options:     nil,
			},
		},
	}

	shmSize, err := getShmSize(containerConfig)
	assert.Nil(t, err)
	assert.Equal(t, shmSize, uint64(size))
}

func TestAddProtectedHypervisorAnnotations(t *testing.T) {
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	ocispec := specs.Spec{
		Annotations: make(map[string]string),
	}

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
		Console:        consolePath,
	}
	ocispec.Annotations[vcAnnotations.KernelParams] = "vsyscall=emulate iommu=on"
	err := addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)
	assert.Exactly(vc.HypervisorConfig{}, config.HypervisorConfig)

	// Enable annotations
	runtimeConfig.HypervisorConfig.EnableAnnotations = []string{".*"}

	ocispec.Annotations[vcAnnotations.FileBackedMemRootDir] = "/dev/shm"
	ocispec.Annotations[vcAnnotations.VirtioFSDaemon] = "/bin/false"
	ocispec.Annotations[vcAnnotations.EntropySource] = "/dev/urandom"

	config.HypervisorConfig.FileBackedMemRootDir = "do-not-touch"
	config.HypervisorConfig.VirtioFSDaemon = "dangerous-daemon"
	config.HypervisorConfig.EntropySource = "truly-random"

	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)
	assert.Equal(config.HypervisorConfig.FileBackedMemRootDir, "do-not-touch")
	assert.Equal(config.HypervisorConfig.VirtioFSDaemon, "dangerous-daemon")
	assert.Equal(config.HypervisorConfig.EntropySource, "truly-random")

	// Now enable them and check again
	runtimeConfig.HypervisorConfig.FileBackedMemRootList = []string{"/dev/*m"}
	runtimeConfig.HypervisorConfig.VirtioFSDaemonList = []string{"/bin/*ls*"}
	runtimeConfig.HypervisorConfig.EntropySourceList = []string{"/dev/*random*"}
	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.NoError(err)
	assert.Equal(config.HypervisorConfig.FileBackedMemRootDir, "/dev/shm")
	assert.Equal(config.HypervisorConfig.VirtioFSDaemon, "/bin/false")
	assert.Equal(config.HypervisorConfig.EntropySource, "/dev/urandom")

	// In case an absurd large value is provided, the config value if not over-ridden
	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "655536"
	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "-1"
	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "-1"
	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMemory] = fmt.Sprintf("%d", vc.MinHypervisorMemory+1)
	assert.Error(err)
}

func TestAddHypervisorAnnotations(t *testing.T) {
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	ocispec := specs.Spec{
		Annotations: make(map[string]string),
	}

	expectedHyperConfig := vc.HypervisorConfig{
		KernelParams: []vc.Param{
			{
				Key:   "vsyscall",
				Value: "emulate",
			},
			{
				Key:   "iommu",
				Value: "on",
			},
		},
	}

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
		Console:        consolePath,
	}
	runtimeConfig.HypervisorConfig.EnableAnnotations = []string{".*"}
	runtimeConfig.HypervisorConfig.FileBackedMemRootList = []string{"/dev/shm*"}
	runtimeConfig.HypervisorConfig.VirtioFSDaemonList = []string{"/bin/*ls*"}

	ocispec.Annotations[vcAnnotations.KernelParams] = "vsyscall=emulate iommu=on"
	addHypervisorConfigOverrides(ocispec, &config, runtimeConfig)
	assert.Exactly(expectedHyperConfig, config.HypervisorConfig)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMemory] = "1024"
	ocispec.Annotations[vcAnnotations.MemSlots] = "20"
	ocispec.Annotations[vcAnnotations.MemOffset] = "512"
	ocispec.Annotations[vcAnnotations.VirtioMem] = "true"
	ocispec.Annotations[vcAnnotations.MemPrealloc] = "true"
	ocispec.Annotations[vcAnnotations.FileBackedMemRootDir] = "/dev/shm"
	ocispec.Annotations[vcAnnotations.HugePages] = "true"
	ocispec.Annotations[vcAnnotations.IOMMU] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceDriver] = "virtio-scsi"
	ocispec.Annotations[vcAnnotations.DisableBlockDeviceUse] = "true"
	ocispec.Annotations[vcAnnotations.EnableIOThreads] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceCacheSet] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceCacheDirect] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceCacheNoflush] = "true"
	ocispec.Annotations[vcAnnotations.SharedFS] = "virtio-fs"
	ocispec.Annotations[vcAnnotations.VirtioFSDaemon] = "/bin/false"
	ocispec.Annotations[vcAnnotations.VirtioFSCache] = "/home/cache"
	ocispec.Annotations[vcAnnotations.VirtioFSExtraArgs] = "[ \"arg0\", \"arg1\" ]"
	ocispec.Annotations[vcAnnotations.Msize9p] = "512"
	ocispec.Annotations[vcAnnotations.MachineType] = "q35"
	ocispec.Annotations[vcAnnotations.MachineAccelerators] = "nofw"
	ocispec.Annotations[vcAnnotations.CPUFeatures] = "pmu=off"
	ocispec.Annotations[vcAnnotations.DisableVhostNet] = "true"
	ocispec.Annotations[vcAnnotations.GuestHookPath] = "/usr/bin/"
	ocispec.Annotations[vcAnnotations.DisableImageNvdimm] = "true"
	ocispec.Annotations[vcAnnotations.HotplugVFIOOnRootBus] = "true"
	ocispec.Annotations[vcAnnotations.PCIeRootPort] = "2"
	ocispec.Annotations[vcAnnotations.IOMMUPlatform] = "true"
	ocispec.Annotations[vcAnnotations.SGXEPC] = "64Mi"
	ocispec.Annotations[vcAnnotations.UseLegacySerial] = "true"
	// 10Mbit
	ocispec.Annotations[vcAnnotations.RxRateLimiterMaxRate] = "10000000"
	ocispec.Annotations[vcAnnotations.TxRateLimiterMaxRate] = "10000000"

	addAnnotations(ocispec, &config, runtimeConfig)
	assert.Equal(config.HypervisorConfig.NumVCPUs, uint32(1))
	assert.Equal(config.HypervisorConfig.DefaultMaxVCPUs, uint32(1))
	assert.Equal(config.HypervisorConfig.MemorySize, uint32(1024))
	assert.Equal(config.HypervisorConfig.MemSlots, uint32(20))
	assert.Equal(config.HypervisorConfig.MemOffset, uint64(512))
	assert.Equal(config.HypervisorConfig.VirtioMem, true)
	assert.Equal(config.HypervisorConfig.MemPrealloc, true)
	assert.Equal(config.HypervisorConfig.FileBackedMemRootDir, "/dev/shm")
	assert.Equal(config.HypervisorConfig.HugePages, true)
	assert.Equal(config.HypervisorConfig.IOMMU, true)
	assert.Equal(config.HypervisorConfig.BlockDeviceDriver, "virtio-scsi")
	assert.Equal(config.HypervisorConfig.DisableBlockDeviceUse, true)
	assert.Equal(config.HypervisorConfig.EnableIOThreads, true)
	assert.Equal(config.HypervisorConfig.BlockDeviceCacheSet, true)
	assert.Equal(config.HypervisorConfig.BlockDeviceCacheDirect, true)
	assert.Equal(config.HypervisorConfig.BlockDeviceCacheNoflush, true)
	assert.Equal(config.HypervisorConfig.SharedFS, "virtio-fs")
	assert.Equal(config.HypervisorConfig.VirtioFSDaemon, "/bin/false")
	assert.Equal(config.HypervisorConfig.VirtioFSCache, "/home/cache")
	assert.ElementsMatch(config.HypervisorConfig.VirtioFSExtraArgs, [2]string{"arg0", "arg1"})
	assert.Equal(config.HypervisorConfig.Msize9p, uint32(512))
	assert.Equal(config.HypervisorConfig.HypervisorMachineType, "q35")
	assert.Equal(config.HypervisorConfig.MachineAccelerators, "nofw")
	assert.Equal(config.HypervisorConfig.CPUFeatures, "pmu=off")
	assert.Equal(config.HypervisorConfig.DisableVhostNet, true)
	assert.Equal(config.HypervisorConfig.GuestHookPath, "/usr/bin/")
	assert.Equal(config.HypervisorConfig.DisableImageNvdimm, true)
	assert.Equal(config.HypervisorConfig.HotplugVFIOOnRootBus, true)
	assert.Equal(config.HypervisorConfig.PCIeRootPort, uint32(2))
	assert.Equal(config.HypervisorConfig.IOMMUPlatform, true)
	assert.Equal(config.HypervisorConfig.SGXEPCSize, int64(67108864))
	assert.Equal(config.HypervisorConfig.LegacySerial, true)
	assert.Equal(config.HypervisorConfig.RxRateLimiterMaxRate, uint64(10000000))
	assert.Equal(config.HypervisorConfig.TxRateLimiterMaxRate, uint64(10000000))

	// In case an absurd large value is provided, the config value if not over-ridden
	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "655536"
	err := addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "-1"
	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "-1"
	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMemory] = fmt.Sprintf("%d", vc.MinHypervisorMemory+1)
	assert.Error(err)
}

func TestCheckPathIsInGlobs(t *testing.T) {
	assert := assert.New(t)

	//nolint: govet
	type testData struct {
		globs    []string
		toMatch  string
		expected bool
	}

	data := []testData{
		{[]string{}, "", false},
		{[]string{}, "nonempty", false},
		{[]string{"simple"}, "simple", false},
		{[]string{"simple"}, "some_simple_text", false},
		{[]string{"/bin/ls"}, "/bin/ls", true},
		{[]string{"/bin/ls", "/bin/false"}, "/bin/ls", true},
		{[]string{"/bin/ls", "/bin/false"}, "/bin/false", true},
		{[]string{"/bin/ls", "/bin/false"}, "/bin/bar", false},
		{[]string{"/bin/*ls*"}, "/bin/ls", true},
		{[]string{"/bin/*ls*"}, "/bin/false", true},
		{[]string{"bin/ls"}, "/bin/ls", false},
		{[]string{"./bin/ls"}, "/bin/ls", false},
		{[]string{"*/bin/ls"}, "/bin/ls", false},
	}

	for _, d := range data {
		matched := checkPathIsInGlobs(d.globs, d.toMatch)
		assert.Equal(d.expected, matched, "%+v", d)
	}
}
