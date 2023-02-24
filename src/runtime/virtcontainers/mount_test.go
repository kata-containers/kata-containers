// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"os"
	"path/filepath"
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
		{"/../sys", true},
		{"/../sys/", true},
		{"/../sys/fs/cgroup", true},
		{"/../sysfoo", false},
	}

	for _, test := range tests {
		result := isSystemMount(test.mnt)
		assert.Exactly(result, test.expected)
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

func TestGetDeviceForPathRoot(t *testing.T) {
	assert := assert.New(t)
	dev, err := getDeviceForPath("/")
	assert.NoError(err)

	expected := "/"

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

func TestIsDockerVolume(t *testing.T) {
	assert := assert.New(t)
	path := "/var/lib/docker/volumes/00da1347c7cf4f15db35f/_data"
	isDockerVolume := IsDockerVolume(path)
	assert.True(isDockerVolume)

	path = "/var/lib/testdir"
	isDockerVolume = IsDockerVolume(path)
	assert.False(isDockerVolume)
}

func TestIsEmtpyDir(t *testing.T) {
	assert := assert.New(t)
	path := "/var/lib/kubelet/pods/5f0861a0-a987-4a3a-bb0f-1058ddb9678f/volumes/kubernetes.io~empty-dir/foobar"
	result := isEmptyDir(path)
	assert.True(result)

	// expect the empty-dir to be second to last in path
	result = isEmptyDir(filepath.Join(path, "bazzzzz"))
	assert.False(result)
}

func TestIsConfigMap(t *testing.T) {
	assert := assert.New(t)
	path := "/var/lib/kubelet/pods/5f0861a0-a987-4a3a-bb0f-1058ddb9678f/volumes/kubernetes.io~configmap/config"
	result := isConfigMap(path)
	assert.True(result)

	// expect the empty-dir to be second to last in path
	result = isConfigMap(filepath.Join(path, "bazzzzz"))
	assert.False(result)

}
func TestIsSecret(t *testing.T) {
	assert := assert.New(t)
	path := "/var/lib/kubelet/pods/5f0861a0-a987-4a3a-bb0f-1058ddb9678f/volumes/kubernetes.io~secret"
	result := isSecret(path)
	assert.False(result)

	// expect the empty-dir to be second to last in path
	result = isSecret(filepath.Join(path, "sweet-token"))
	assert.True(result)

	result = isConfigMap(filepath.Join(path, "sweet-token-dir", "whoops"))
	assert.False(result)
}

func TestIsWatchable(t *testing.T) {
	if os.Getuid() != 0 {
		t.Skip("Test disabled as requires root user")
	}

	assert := assert.New(t)

	path := ""
	result := isWatchableMount(path)
	assert.False(result)

	// path does not exist, failure expected:
	path = "/var/lib/kubelet/pods/5f0861a0-a987-4a3a-bb0f-1058ddb9678f/volumes/kubernetes.io~empty-dir/foobar"
	result = isWatchableMount(path)
	assert.False(result)

	testPath := t.TempDir()

	// Verify secret is successful (single file mount):
	//   /tmppath/kubernetes.io~secret/super-secret-thing
	secretpath := filepath.Join(testPath, K8sSecret)
	err := os.MkdirAll(secretpath, 0777)
	assert.NoError(err)
	secret := filepath.Join(secretpath, "super-secret-thing")
	_, err = os.Create(secret)
	assert.NoError(err)
	result = isWatchableMount(secret)
	assert.True(result)

	// Verify that if we have too many files, it will no longer be watchable:
	// /tmp/kubernetes.io~configmap/amazing-dir-of-configs/
	//                                  | - c0
	//                                  | - c1
	//                                    ...
	//                                  | - c7
	// should be okay.
	//
	// 9 files should cause the mount to be deemed "not watchable"
	configs := filepath.Join(testPath, K8sConfigMap, "amazing-dir-of-configs")
	err = os.MkdirAll(configs, 0777)
	assert.NoError(err)

	for i := 0; i < 8; i++ {
		_, err := os.Create(filepath.Join(configs, fmt.Sprintf("c%v", i)))
		assert.NoError(err)
		result = isWatchableMount(configs)
		assert.True(result)
	}
	_, err = os.Create(filepath.Join(configs, "toomuch"))
	assert.NoError(err)
	result = isWatchableMount(configs)
	assert.False(result)
}
