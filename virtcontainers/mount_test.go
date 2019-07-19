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

	ktu "github.com/kata-containers/runtime/pkg/katatestutils"
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
		if result != test.expected {
			t.Fatalf("Expected result for path %s : %v, got %v", test.mnt, test.expected, result)
		}
	}
}

func TestIsHostDevice(t *testing.T) {
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
		if result != test.expected {
			t.Fatalf("Expected result for path %s : %v, got %v", test.mnt, test.expected, result)
		}
	}
}

func TestIsHostDeviceCreateFile(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}
	// Create regular file in /dev

	path := "/dev/foobar"
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	f.Close()

	if isHostDevice(path) != false {
		t.Fatalf("Expected result for path %s : %v, got %v", path, false, true)
	}

	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
}

func TestMajorMinorNumber(t *testing.T) {
	devices := []string{"/dev/zero", "/dev/net/tun"}

	for _, device := range devices {
		cmdStr := fmt.Sprintf("ls -l %s | awk '{print $5$6}'", device)
		cmd := exec.Command("sh", "-c", cmdStr)
		output, err := cmd.Output()

		if err != nil {
			t.Fatal(err)
		}

		data := bytes.Split(output, []byte(","))
		if len(data) < 2 {
			t.Fatal()
		}

		majorStr := strings.TrimSpace(string(data[0]))
		minorStr := strings.TrimSpace(string(data[1]))

		majorNo, err := strconv.Atoi(majorStr)
		if err != nil {
			t.Fatal(err)
		}

		minorNo, err := strconv.Atoi(minorStr)
		if err != nil {
			t.Fatal(err)
		}

		stat := syscall.Stat_t{}
		err = syscall.Stat(device, &stat)
		if err != nil {
			t.Fatal(err)
		}

		// Get major and minor numbers for the device itself. Note the use of stat.Rdev instead of Dev.
		major := major(stat.Rdev)
		minor := minor(stat.Rdev)

		if minor != minorNo {
			t.Fatalf("Expected minor number for device %s: %d, Got :%d", device, minorNo, minor)
		}

		if major != majorNo {
			t.Fatalf("Expected major number for device %s : %d, Got :%d", device, majorNo, major)
		}
	}
}

func TestGetDeviceForPathRoot(t *testing.T) {
	dev, err := getDeviceForPath("/")
	if err != nil {
		t.Fatal(err)
	}

	expected := "/"

	if dev.mountPoint != expected {
		t.Fatalf("Expected %s mountpoint, got %s", expected, dev.mountPoint)
	}
}

func TestGetDeviceForPathValidMount(t *testing.T) {
	dev, err := getDeviceForPath("/proc")
	if err != nil {
		t.Fatal(err)
	}

	expected := "/proc"

	if dev.mountPoint != expected {
		t.Fatalf("Expected %s mountpoint, got %s", expected, dev.mountPoint)
	}
}

func TestGetDeviceForPathEmptyPath(t *testing.T) {
	_, err := getDeviceForPath("")
	if err == nil {
		t.Fatal()
	}
}

func TestGetDeviceForPath(t *testing.T) {
	dev, err := getDeviceForPath("///")
	if err != nil {
		t.Fatal(err)
	}

	if dev.mountPoint != "/" {
		t.Fatal(err)
	}

	_, err = getDeviceForPath("/../../.././././../.")
	if err != nil {
		t.Fatal(err)
	}

	_, err = getDeviceForPath("/root/file with spaces")
	if err == nil {
		t.Fatal()
	}
}

func TestGetDeviceForPathBindMount(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	source := filepath.Join(testDir, "testDeviceDirSrc")
	dest := filepath.Join(testDir, "testDeviceDirDest")
	syscall.Unmount(dest, 0)
	os.Remove(source)
	os.Remove(dest)

	err := os.MkdirAll(source, mountPerm)
	if err != nil {
		t.Fatal(err)
	}

	defer os.Remove(source)

	err = os.MkdirAll(dest, mountPerm)
	if err != nil {
		t.Fatal(err)
	}

	defer os.Remove(dest)

	err = bindMount(context.Background(), source, dest, false)
	if err != nil {
		t.Fatal(err)
	}

	defer syscall.Unmount(dest, syscall.MNT_DETACH)

	destFile := filepath.Join(dest, "test")
	_, err = os.Create(destFile)
	if err != nil {
		fmt.Println("Could not create test file:", err)
		t.Fatal(err)
	}

	defer os.Remove(destFile)

	sourceDev, _ := getDeviceForPath(source)
	destDev, _ := getDeviceForPath(destFile)

	if sourceDev != destDev {
		t.Fatal()
	}
}

func TestGetDevicePathAndFsTypeEmptyMount(t *testing.T) {
	_, _, err := GetDevicePathAndFsType("")

	if err == nil {
		t.Fatal()
	}
}

func TestGetDevicePathAndFsTypeSuccessful(t *testing.T) {
	path, fstype, err := GetDevicePathAndFsType("/proc")

	if err != nil {
		t.Fatal(err)
	}

	if path != "proc" || fstype != "proc" {
		t.Fatal(err)
	}
}

func TestIsDeviceMapper(t *testing.T) {
	// known major, minor for /dev/tty
	major := 5
	minor := 0

	isDM, err := isDeviceMapper(major, minor)
	if err != nil {
		t.Fatal(err)
	}

	if isDM {
		t.Fatal()
	}

	// fake the block device format
	blockFormatTemplate = "/sys/dev/char/%d:%d"
	isDM, err = isDeviceMapper(major, minor)
	if err != nil {
		t.Fatal(err)
	}

	if !isDM {
		t.Fatal()
	}
}

func TestIsDockerVolume(t *testing.T) {
	path := "/var/lib/docker/volumes/00da1347c7cf4f15db35f/_data"
	isDockerVolume := IsDockerVolume(path)
	assert.True(t, isDockerVolume)

	path = "/var/lib/testdir"
	isDockerVolume = IsDockerVolume(path)
	assert.False(t, isDockerVolume)
}

func TestIsEphemeralStorage(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	dir, err := ioutil.TempDir(testDir, "foo")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	sampleEphePath := filepath.Join(dir, K8sEmptyDir, "tmp-volume")
	err = os.MkdirAll(sampleEphePath, testDirMode)
	assert.Nil(t, err)

	err = syscall.Mount("tmpfs", sampleEphePath, "tmpfs", 0, "")
	assert.Nil(t, err)
	defer syscall.Unmount(sampleEphePath, 0)

	isEphe := IsEphemeralStorage(sampleEphePath)
	assert.True(t, isEphe)

	isHostEmptyDir := Isk8sHostEmptyDir(sampleEphePath)
	assert.False(t, isHostEmptyDir)

	sampleEphePath = "/var/lib/kubelet/pods/366c3a75-4869-11e8-b479-507b9ddd5ce4/volumes/cache-volume"
	isEphe = IsEphemeralStorage(sampleEphePath)
	assert.False(t, isEphe)

	isHostEmptyDir = Isk8sHostEmptyDir(sampleEphePath)
	assert.False(t, isHostEmptyDir)
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

	// check to make sure the file doesn't exist
	testPath := filepath.Join(testMnt, sID, cID, rootfsDir)
	if _, err := os.Stat(testPath); !os.IsNotExist(err) {
		if err := os.Remove(testPath); err != nil {
			t.Fatalf("test mount file should not exist, and cannot be removed: %s", err)
		}
	}

	err := bindUnmountContainerRootfs(context.Background(), testMnt, sID, cID)
	assert.Nil(t, err)
}
