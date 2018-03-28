//
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
//

package virtcontainers

import (
	"bytes"
	"fmt"
	"github.com/stretchr/testify/assert"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
)

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
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
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

	err = bindMount(source, dest, false)
	if err != nil {
		t.Fatal(err)
	}

	defer syscall.Unmount(dest, 0)

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
	_, _, err := getDevicePathAndFsType("")

	if err == nil {
		t.Fatal()
	}
}

func TestGetDevicePathAndFsTypeSuccessful(t *testing.T) {
	path, fstype, err := getDevicePathAndFsType("/proc")

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

func TestGetVirtDriveNameInvalidIndex(t *testing.T) {
	_, err := getVirtDriveName(-1)

	if err == nil {
		t.Fatal(err)
	}
}

func TestGetVirtDriveName(t *testing.T) {
	tests := []struct {
		index         int
		expectedDrive string
	}{
		{0, "vda"},
		{25, "vdz"},
		{27, "vdab"},
		{704, "vdaac"},
		{18277, "vdzzz"},
	}

	for _, test := range tests {
		driveName, err := getVirtDriveName(test.index)
		if err != nil {
			t.Fatal(err)
		}
		if driveName != test.expectedDrive {
			t.Fatalf("Incorrect drive Name: Got: %s, Expecting :%s", driveName, test.expectedDrive)

		}
	}
}

func TestGetSCSIIdLun(t *testing.T) {
	tests := []struct {
		index          int
		expectedScsiID int
		expectedLun    int
	}{
		{0, 0, 0},
		{1, 0, 1},
		{2, 0, 2},
		{255, 0, 255},
		{256, 1, 0},
		{257, 1, 1},
		{258, 1, 2},
		{512, 2, 0},
		{513, 2, 1},
	}

	for _, test := range tests {
		scsiID, lun, err := getSCSIIdLun(test.index)
		assert.Nil(t, err)

		if scsiID != test.expectedScsiID && lun != test.expectedLun {
			t.Fatalf("Expecting scsi-id:lun %d:%d,  Got %d:%d", test.expectedScsiID, test.expectedLun, scsiID, lun)
		}
	}

	_, _, err := getSCSIIdLun(maxSCSIDevices + 1)
	assert.NotNil(t, err)
}

func TestGetSCSIAddress(t *testing.T) {
	tests := []struct {
		index               int
		expectedSCSIAddress string
	}{
		{0, "0:0"},
		{200, "0:200"},
		{255, "0:255"},
		{258, "1:2"},
		{512, "2:0"},
	}

	for _, test := range tests {
		scsiAddr, err := getSCSIAddress(test.index)
		assert.Nil(t, err)
		assert.Equal(t, scsiAddr, test.expectedSCSIAddress)
	}
}
