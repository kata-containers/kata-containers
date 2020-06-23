// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bytes"
	"context"
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/stretchr/testify/assert"
)

const (
	testDirMode = os.FileMode(0750)
)

var tc ktu.TestConstraint

func init() {
	tc = ktu.NewTestConstraint(false)
}

func TestIsSystemMount(t *testing.T) {
	assert := assert.New(t)
	tests := []struct {
		mnt      string
		expected bool
	}{
		{"/sys", true},
		{"/sys/", true},
		{"/sys//", true},
		{"/sys/fs", true},
		{"/sys/fs/", true},
		{"/sys/fs/cgroup", true},
		{"/sysfoo", false},
		{"/home", false},
		{"/dev/block/", false},
		{"/mnt/dev/foo", false},
	}

	for _, test := range tests {
		result := isSystemMount(test.mnt)
		assert.Exactly(result, test.expected)
	}
}

func TestIsHostDevice(t *testing.T) {
	assert := assert.New(t)
	tests := []struct {
		mnt      string
		expected bool
	}{
		{"/dev", true},
		{"/dev/zero", true},
		{"/dev/block", true},
		{"/mnt/dev/block", false},
	}

	for _, test := range tests {
		result := isHostDevice(test.mnt)
		assert.Equal(result, test.expected)
	}
}

func TestIsHostDeviceCreateFile(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}
	// Create regular file in /dev

	path := "/dev/foobar"
	f, err := os.Create(path)
	assert.NoError(err)
	f.Close()

	assert.False(isHostDevice(path))
	assert.NoError(os.Remove(path))
}

func TestMajorMinorNumber(t *testing.T) {
	assert := assert.New(t)
	devices := []string{"/dev/zero", "/dev/net/tun"}

	for _, device := range devices {
		cmdStr := fmt.Sprintf("ls -l %s | awk '{print $5$6}'", device)
		cmd := exec.Command("sh", "-c", cmdStr)
		output, err := cmd.Output()
		assert.NoError(err)

		data := bytes.Split(output, []byte(","))
		assert.False(len(data) < 2)

		majorStr := strings.TrimSpace(string(data[0]))
		minorStr := strings.TrimSpace(string(data[1]))

		majorNo, err := strconv.Atoi(majorStr)
		assert.NoError(err)

		minorNo, err := strconv.Atoi(minorStr)
		assert.NoError(err)

		stat := syscall.Stat_t{}
		err = syscall.Stat(device, &stat)
		assert.NoError(err)

		// Get major and minor numbers for the device itself. Note the use of stat.Rdev instead of Dev.
		major := major(stat.Rdev)
		minor := minor(stat.Rdev)

		assert.Equal(minor, minorNo)
		assert.Equal(major, majorNo)
	}
}

func TestGetDeviceForPathRoot(t *testing.T) {
	assert := assert.New(t)
	dev, err := getDeviceForPath("/")
	assert.NoError(err)

	expected := "/"

	assert.Equal(dev.mountPoint, expected)
}

func TestGetDeviceForPathValidMount(t *testing.T) {
	assert := assert.New(t)
	dev, err := getDeviceForPath("/proc")
	assert.NoError(err)

	expected := "/proc"

	assert.Equal(dev.mountPoint, expected)
}

func TestGetDeviceForPathEmptyPath(t *testing.T) {
	assert := assert.New(t)
	_, err := getDeviceForPath("")
	assert.Error(err)
}

func TestGetDeviceForPath(t *testing.T) {
	assert := assert.New(t)

	dev, err := getDeviceForPath("///")
	assert.NoError(err)

	assert.Equal(dev.mountPoint, "/")

	_, err = getDeviceForPath("/../../.././././../.")
	assert.NoError(err)

	_, err = getDeviceForPath("/root/file with spaces")
	assert.Error(err)
}

func TestGetDeviceForPathBindMount(t *testing.T) {
	assert := assert.New(t)

	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	source := filepath.Join(testDir, "testDeviceDirSrc")
	dest := filepath.Join(testDir, "testDeviceDirDest")
	syscall.Unmount(dest, 0)
	os.Remove(source)
	os.Remove(dest)

	err := os.MkdirAll(source, mountPerm)
	assert.NoError(err)

	defer os.Remove(source)

	err = os.MkdirAll(dest, mountPerm)
	assert.NoError(err)

	defer os.Remove(dest)

	err = bindMount(context.Background(), source, dest, false, "private")
	assert.NoError(err)

	defer syscall.Unmount(dest, syscall.MNT_DETACH)

	destFile := filepath.Join(dest, "test")
	_, err = os.Create(destFile)
	assert.NoError(err)

	defer os.Remove(destFile)

	sourceDev, _ := getDeviceForPath(source)
	destDev, _ := getDeviceForPath(destFile)

	assert.Equal(sourceDev, destDev)
}

func TestIsDeviceMapper(t *testing.T) {
	assert := assert.New(t)

	// known major, minor for /dev/tty
	major := 5
	minor := 0

	isDM, err := isDeviceMapper(major, minor)
	assert.NoError(err)
	assert.False(isDM)

	// fake the block device format
	blockFormatTemplate = "/sys/dev/char/%d:%d"
	isDM, err = isDeviceMapper(major, minor)
	assert.NoError(err)
	assert.True(isDM)
}

func TestIsDockerVolume(t *testing.T) {
	assert := assert.New(t)
	path := "/var/lib/docker/volumes/00da1347c7cf4f15db35f/_data"
	isDockerVolume := IsDockerVolume(path)
	assert.True(isDockerVolume)

	path = "/var/lib/testdir"
	isDockerVolume = IsDockerVolume(path)
	assert.False(isDockerVolume)
}

func TestIsEphemeralStorage(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	dir, err := ioutil.TempDir(testDir, "foo")
	assert.NoError(err)
	defer os.RemoveAll(dir)

	sampleEphePath := filepath.Join(dir, K8sEmptyDir, "tmp-volume")
	err = os.MkdirAll(sampleEphePath, testDirMode)
	assert.Nil(err)

	err = syscall.Mount("tmpfs", sampleEphePath, "tmpfs", 0, "")
	assert.NoError(err)
	defer syscall.Unmount(sampleEphePath, 0)

	isEphe := IsEphemeralStorage(sampleEphePath)
	assert.True(isEphe)

	isHostEmptyDir := Isk8sHostEmptyDir(sampleEphePath)
	assert.False(isHostEmptyDir)

	sampleEphePath = "/var/lib/kubelet/pods/366c3a75-4869-11e8-b479-507b9ddd5ce4/volumes/cache-volume"
	isEphe = IsEphemeralStorage(sampleEphePath)
	assert.False(isEphe)

	isHostEmptyDir = Isk8sHostEmptyDir(sampleEphePath)
	assert.False(isHostEmptyDir)
}

func TestBindMountInvalidSourceSymlink(t *testing.T) {
	source := filepath.Join(testDir, "fooFile")
	os.Remove(source)

	err := bindMount(context.Background(), source, "", false, "private")
	assert.Error(t, err)
}

func TestBindMountFailingMount(t *testing.T) {
	source := filepath.Join(testDir, "fooLink")
	fakeSource := filepath.Join(testDir, "fooFile")
	os.Remove(source)
	os.Remove(fakeSource)
	assert := assert.New(t)

	_, err := os.OpenFile(fakeSource, os.O_CREATE, mountPerm)
	assert.NoError(err)

	err = os.Symlink(fakeSource, source)
	assert.NoError(err)

	err = bindMount(context.Background(), source, "", false, "private")
	assert.Error(err)
}

func TestBindMountSuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	source := filepath.Join(testDir, "fooDirSrc")
	dest := filepath.Join(testDir, "fooDirDest")
	syscall.Unmount(dest, 0)
	os.Remove(source)
	os.Remove(dest)

	err := os.MkdirAll(source, mountPerm)
	assert.NoError(err)

	err = os.MkdirAll(dest, mountPerm)
	assert.NoError(err)

	err = bindMount(context.Background(), source, dest, false, "private")
	assert.NoError(err)

	syscall.Unmount(dest, 0)
}

func TestBindMountReadonlySuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	source := filepath.Join(testDir, "fooDirSrc")
	dest := filepath.Join(testDir, "fooDirDest")
	syscall.Unmount(dest, 0)
	os.Remove(source)
	os.Remove(dest)

	err := os.MkdirAll(source, mountPerm)
	assert.NoError(err)

	err = os.MkdirAll(dest, mountPerm)
	assert.NoError(err)

	err = bindMount(context.Background(), source, dest, true, "private")
	assert.NoError(err)

	defer syscall.Unmount(dest, 0)

	// should not be able to create file in read-only mount
	destFile := filepath.Join(dest, "foo")
	_, err = os.OpenFile(destFile, os.O_CREATE, mountPerm)
	assert.Error(err)
}

func TestBindMountInvalidPgtypes(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	source := filepath.Join(testDir, "fooDirSrc")
	dest := filepath.Join(testDir, "fooDirDest")
	syscall.Unmount(dest, 0)
	os.Remove(source)
	os.Remove(dest)

	err := os.MkdirAll(source, mountPerm)
	assert.NoError(err)

	err = os.MkdirAll(dest, mountPerm)
	assert.NoError(err)

	err = bindMount(context.Background(), source, dest, false, "foo")
	expectedErr := fmt.Sprintf("Wrong propagation type %s", "foo")
	assert.EqualError(err, expectedErr)
}

// TestBindUnmountContainerRootfsENOENTNotError tests that if a file
// or directory attempting to be unmounted doesn't exist, then it
// is not considered an error
func TestBindUnmountContainerRootfsENOENTNotError(t *testing.T) {
	if os.Getuid() != 0 {
		t.Skip("Test disabled as requires root user")
	}
	testMnt := "/tmp/test_mount"
	sID := "sandIDTest"
	cID := "contIDTest"
	assert := assert.New(t)

	// check to make sure the file doesn't exist
	testPath := filepath.Join(testMnt, sID, cID, rootfsDir)
	if _, err := os.Stat(testPath); !os.IsNotExist(err) {
		assert.NoError(os.Remove(testPath))
	}

	err := bindUnmountContainerRootfs(context.Background(), filepath.Join(testMnt, sID), cID)
	assert.NoError(err)
}

func TestBindUnmountContainerRootfsRemoveRootfsDest(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	sID := "sandIDTestRemoveRootfsDest"
	cID := "contIDTestRemoveRootfsDest"

	testPath := filepath.Join(testDir, sID, cID, rootfsDir)
	syscall.Unmount(testPath, 0)
	os.Remove(testPath)

	err := os.MkdirAll(testPath, mountPerm)
	assert.NoError(err)
	defer os.RemoveAll(filepath.Join(testDir, sID))

	bindUnmountContainerRootfs(context.Background(), filepath.Join(testDir, sID), cID)

	if _, err := os.Stat(testPath); err == nil {
		t.Fatal("empty rootfs dest should be removed")
	} else if !os.IsNotExist(err) {
		t.Fatal(err)
	}
}
