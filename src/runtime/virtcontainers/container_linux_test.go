// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/manager"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	"github.com/stretchr/testify/assert"
)

func TestUnmountHostMountsRemoveBindHostPath(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	createFakeMountDir := func(t *testing.T, dir, prefix string) string {
		name, err := os.MkdirTemp(dir, "test-mnt-"+prefix+"-")
		if err != nil {
			t.Fatal(err)
		}
		return name
	}

	createFakeMountFile := func(t *testing.T, dir, prefix string) string {
		f, err := os.CreateTemp(dir, "test-mnt-"+prefix+"-")
		if err != nil {
			t.Fatal(err)
		}
		f.Close()
		return f.Name()
	}

	doUnmountCheck := func(s *Sandbox, src, dest, hostPath, nonEmptyHostpath, devPath string) {
		mounts := []Mount{
			{
				Source:      src,
				Destination: dest,
				HostPath:    hostPath,
				Type:        "bind",
			},
			{
				Source:      src,
				Destination: dest,
				HostPath:    nonEmptyHostpath,
				Type:        "bind",
			},
			{
				Source:      src,
				Destination: dest,
				HostPath:    devPath,
				Type:        "dev",
			},
		}

		c := Container{
			mounts:  mounts,
			ctx:     context.Background(),
			id:      "fooabr",
			sandbox: s,
		}

		if err := bindMount(c.ctx, src, hostPath, false, "private"); err != nil {
			t.Fatal(err)
		}
		defer syscall.Unmount(hostPath, 0)
		if err := bindMount(c.ctx, src, nonEmptyHostpath, false, "private"); err != nil {
			t.Fatal(err)
		}
		defer syscall.Unmount(nonEmptyHostpath, 0)
		if err := bindMount(c.ctx, src, devPath, false, "private"); err != nil {
			t.Fatal(err)
		}
		defer syscall.Unmount(devPath, 0)

		err := c.unmountHostMounts(c.ctx)
		if err != nil {
			t.Fatal(err)
		}

		for _, path := range [3]string{src, dest, devPath} {
			if _, err := os.Stat(path); err != nil {
				if os.IsNotExist(err) {
					t.Fatalf("path %s should not be removed", path)
				} else {
					t.Fatal(err)
				}
			}
		}

		if _, err := os.Stat(hostPath); err == nil {
			t.Fatal("empty host-path should be removed")
		} else if !os.IsNotExist(err) {
			t.Fatal(err)
		}

		if _, err := os.Stat(nonEmptyHostpath); err != nil {
			if os.IsNotExist(err) {
				t.Fatal("non-empty host-path should not be removed")
			} else {
				t.Fatal(err)
			}
		}
	}

	src := createFakeMountDir(t, testDir, "src")
	dest := createFakeMountDir(t, testDir, "dest")
	hostPath := createFakeMountDir(t, testDir, "host-path")
	nonEmptyHostpath := createFakeMountDir(t, testDir, "non-empty-host-path")
	devPath := createFakeMountDir(t, testDir, "dev-hostpath")
	// create sandbox for mounting into
	sandbox := &Sandbox{
		ctx:    context.Background(),
		id:     "foobar",
		config: &SandboxConfig{},
		agent:  newMockAgent(),
	}

	fsShare, err := NewFilesystemShare(sandbox)
	if err != nil {
		t.Fatal(err)
	}
	sandbox.fsShare = fsShare

	createFakeMountDir(t, nonEmptyHostpath, "nop")
	doUnmountCheck(sandbox, src, dest, hostPath, nonEmptyHostpath, devPath)

	src = createFakeMountFile(t, testDir, "src")
	dest = createFakeMountFile(t, testDir, "dest")
	hostPath = createFakeMountFile(t, testDir, "host-path")
	nonEmptyHostpath = createFakeMountFile(t, testDir, "non-empty-host-path")
	devPath = createFakeMountFile(t, testDir, "dev-host-path")
	f, err := os.OpenFile(nonEmptyHostpath, os.O_WRONLY, os.FileMode(0640))
	if err != nil {
		t.Fatal(err)
	}
	f.WriteString("nop\n")
	f.Close()
	doUnmountCheck(sandbox, src, dest, hostPath, nonEmptyHostpath, devPath)
}

func testSetupFakeRootfs(t *testing.T) (testRawFile, loopDev, mntDir string, err error) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	tmpDir := t.TempDir()

	testRawFile = filepath.Join(tmpDir, "raw.img")
	_, err = os.Stat(testRawFile)
	assert.True(os.IsNotExist(err))

	output, err := exec.Command("losetup", "-f").CombinedOutput()
	assert.NoError(err)
	loopDev = strings.TrimSpace(string(output[:]))

	_, err = exec.Command("fallocate", "-l", "256K", testRawFile).CombinedOutput()
	assert.NoError(err)

	_, err = exec.Command("mkfs.ext4", "-F", testRawFile).CombinedOutput()
	assert.NoError(err)

	_, err = exec.Command("losetup", loopDev, testRawFile).CombinedOutput()
	assert.NoError(err)

	mntDir = filepath.Join(tmpDir, "rootfs")
	err = os.Mkdir(mntDir, DirMode)
	assert.NoError(err)

	err = syscall.Mount(loopDev, mntDir, "ext4", uintptr(0), "")
	assert.NoError(err)
	return
}

func cleanupFakeRootfsSetup(testRawFile, loopDev, mntDir string) {
	// unmount loop device
	if mntDir != "" {
		syscall.Unmount(mntDir, 0)
	}

	// detach loop device
	if loopDev != "" {
		exec.Command("losetup", "-d", loopDev).CombinedOutput()
	}

	if _, err := os.Stat(testRawFile); err == nil {
		tmpDir := filepath.Dir(testRawFile)
		os.RemoveAll(tmpDir)
	}
}

func TestContainerAddDriveDir(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	testRawFile, loopDev, fakeRootfs, err := testSetupFakeRootfs(t)

	defer cleanupFakeRootfsSetup(testRawFile, loopDev, fakeRootfs)

	assert.NoError(err)

	sandbox := &Sandbox{
		ctx:        context.Background(),
		id:         testSandboxID,
		devManager: manager.NewDeviceManager(config.VirtioSCSI, false, "", 0, nil),
		hypervisor: &mockHypervisor{},
		agent:      &mockAgent{},
		config: &SandboxConfig{
			HypervisorConfig: HypervisorConfig{
				DisableBlockDeviceUse: false,
			},
		},
	}

	sandbox.store, err = persist.GetDriver()
	assert.NoError(err)
	assert.NotNil(sandbox.store)

	defer sandbox.store.Destroy(sandbox.id)

	contID := "100"
	container := Container{
		sandbox: sandbox,
		id:      contID,
		rootFs:  RootFs{Target: fakeRootfs, Mounted: true},
	}

	// Make the checkStorageDriver func variable point to a fake Check function
	savedFunc := checkStorageDriver
	checkStorageDriver = func(major, minor int) (bool, error) {
		return true, nil
	}

	defer func() {
		checkStorageDriver = savedFunc
	}()

	container.state.Fstype = ""

	err = container.hotplugDrive(sandbox.ctx)
	assert.NoError(err)

	assert.NotEmpty(container.state.Fstype)
}

func TestContainerRootfsPath(t *testing.T) {

	testRawFile, loopDev, fakeRootfs, err := testSetupFakeRootfs(t)
	defer cleanupFakeRootfsSetup(testRawFile, loopDev, fakeRootfs)
	assert.Nil(t, err)

	truecheckstoragedriver := checkStorageDriver
	checkStorageDriver = func(major, minor int) (bool, error) {
		return true, nil
	}
	defer func() {
		checkStorageDriver = truecheckstoragedriver
	}()

	sandbox := &Sandbox{
		ctx:        context.Background(),
		id:         "rootfstestsandbox",
		agent:      &mockAgent{},
		hypervisor: &mockHypervisor{},
		config: &SandboxConfig{
			HypervisorConfig: HypervisorConfig{
				DisableBlockDeviceUse: false,
			},
		},
	}

	container := Container{
		id:           "rootfstestcontainerid",
		sandbox:      sandbox,
		rootFs:       RootFs{Target: fakeRootfs, Mounted: true},
		rootfsSuffix: "rootfs",
	}

	container.hotplugDrive(sandbox.ctx)
	assert.Empty(t, container.rootfsSuffix)

	// Reset the value to test the other case
	container.rootFs = RootFs{Target: fakeRootfs + "/rootfs", Mounted: true}
	container.rootfsSuffix = "rootfs"

	container.hotplugDrive(sandbox.ctx)
	assert.Equal(t, container.rootfsSuffix, "rootfs")
}
