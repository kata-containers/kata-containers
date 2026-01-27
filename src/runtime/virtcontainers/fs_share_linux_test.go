// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"os"
	"path"
	"path/filepath"
	"syscall"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

func TestSandboxSharedFilesystem(t *testing.T) {
	if os.Getuid() != 0 {
		t.Skip("Test disabled as requires root user")
	}

	assert := assert.New(t)
	// create temporary files to mount:
	testMountPath := t.TempDir()

	// create a new shared directory for our test:
	kataHostSharedDirSaved := kataHostSharedDir
	testHostDir := t.TempDir()
	kataHostSharedDir = func() string {
		return testHostDir
	}
	defer func() {
		kataHostSharedDir = kataHostSharedDirSaved
	}()

	m1Path := filepath.Join(testMountPath, "foo.txt")
	f1, err := os.Create(m1Path)
	assert.NoError(err)
	defer f1.Close()

	m2Path := filepath.Join(testMountPath, "bar.txt")
	f2, err := os.Create(m2Path)
	assert.NoError(err)
	defer f2.Close()

	// create sandbox for mounting into
	sandbox := &Sandbox{
		ctx: context.Background(),
		id:  "foobar",
		config: &SandboxConfig{
			SandboxBindMounts: []string{m1Path, m2Path},
		},
	}

	fsShare, err := NewFilesystemShare(sandbox)
	assert.Nil(err)
	sandbox.fsShare = fsShare

	// make the shared directory for our test:
	dir := kataHostSharedDir()
	err = os.MkdirAll(path.Join(dir, sandbox.id), 0777)
	assert.Nil(err)

	// Test the prepare function. We expect it to succeed
	err = sandbox.fsShare.Prepare(sandbox.ctx)
	assert.NoError(err)

	// Test the Cleanup function. We expect it to succeed for the mount to be removed.
	err = sandbox.fsShare.Cleanup(sandbox.ctx)
	assert.NoError(err)

	// After successful Cleanup, verify there are not any mounts left behind.
	stat := syscall.Stat_t{}
	mount1CheckPath := filepath.Join(getMountPath(sandbox.id), sandboxMountsDir, filepath.Base(m1Path))
	err = syscall.Stat(mount1CheckPath, &stat)
	assert.Error(err)
	assert.True(os.IsNotExist(err))

	mount2CheckPath := filepath.Join(getMountPath(sandbox.id), sandboxMountsDir, filepath.Base(m2Path))
	err = syscall.Stat(mount2CheckPath, &stat)
	assert.Error(err)
	assert.True(os.IsNotExist(err))

	// Verify that Prepare is idempotent.
	err = sandbox.fsShare.Prepare(sandbox.ctx)
	assert.NoError(err)
	err = sandbox.fsShare.Prepare(sandbox.ctx)
	assert.NoError(err)

	// Verify that Cleanup is idempotent.
	err = sandbox.fsShare.Cleanup(sandbox.ctx)
	assert.NoError(err)
	err = sandbox.fsShare.Cleanup(sandbox.ctx)
	assert.NoError(err)
}

func TestShareRootFilesystem(t *testing.T) {
	requireNewFilesystemShare := func(sandbox *Sandbox) *FilesystemShare {
		fsShare, err := NewFilesystemShare(sandbox)
		assert.NoError(t, err)
		return fsShare
	}

	testCases := map[string]struct {
		fsSharer       *FilesystemShare
		container      *Container
		wantErr        bool
		wantSharedFile *SharedFile
	}{
		"force guest pull successful": {
			fsSharer: requireNewFilesystemShare(&Sandbox{
				config: &SandboxConfig{
					ForceGuestPull: true,
				},
			}),
			container: &Container{
				id:           "container-id-abc",
				rootfsSuffix: "test-suffix",
				config: &ContainerConfig{
					Annotations: map[string]string{
						"io.kubernetes.cri.image-name": "test-image-name",
					},
					CustomSpec: &specs.Spec{
						Annotations: map[string]string{
							"io.kubernetes.cri.container-type": "",
						},
					},
				},
			},
			wantSharedFile: &SharedFile{
				containerStorages: []*grpc.Storage{{
					Fstype:     "overlay",
					Source:     "test-image-name",
					MountPoint: "/run/kata-containers/container-id-abc/test-suffix",
					Driver:     "image_guest_pull",
					DriverOptions: []string{
						"image_guest_pull={\"metadata\":{\"io.kubernetes.cri.image-name\":\"test-image-name\"}}",
					},
				}},
				guestPath: "/run/kata-containers/container-id-abc/test-suffix",
			},
		},
		"force guest pull image name missing": {
			fsSharer: requireNewFilesystemShare(&Sandbox{
				config: &SandboxConfig{
					ForceGuestPull: true,
				},
			}),
			container: &Container{
				id:           "container-id-abc",
				rootfsSuffix: "test-suffix",
				config: &ContainerConfig{
					Annotations: map[string]string{},
					CustomSpec: &specs.Spec{
						Annotations: map[string]string{
							"io.kubernetes.cri.container-type": "",
						},
					},
				},
			},
			wantErr: true,
		},
		"block device rootfs with mount options": {
			fsSharer: requireNewFilesystemShare(&Sandbox{
				config: &SandboxConfig{
					HypervisorConfig: HypervisorConfig{
						BlockDeviceDriver: "virtio-scsi",
					},
				},
				devManager: &mockDeviceManager{
					devices: map[string]api.Device{
						"block-device-123": &mockBlockDevice{
							deviceInfo: &config.BlockDrive{
								SCSIAddr: "0:0:0:0",
							},
						},
					},
				},
			}),
			container: &Container{
				id:           "container-id-abc",
				rootfsSuffix: "test-suffix",
				rootFs: RootFs{
					Source:  "/dev/sda1",
					Type:    "ext4",
					Options: []string{"rw", "discard", "noatime"},
				},
				state: types.ContainerState{
					Fstype:        "ext4",
					BlockDeviceID: "block-device-123",
				},
			},
			wantSharedFile: &SharedFile{
				containerStorages: []*grpc.Storage{{
					MountPoint: "/run/kata-containers/shared/containers/container-id-abc",
					Source:     "0:0:0:0",
					Fstype:     "ext4",
					Driver:     "scsi",
					Options:    []string{"rw", "discard", "noatime"},
				}},
				guestPath: "/run/kata-containers/shared/containers/container-id-abc/test-suffix",
			},
		},
		"xfs block device rootfs with nouuid": {
			fsSharer: requireNewFilesystemShare(&Sandbox{
				config: &SandboxConfig{
					HypervisorConfig: HypervisorConfig{
						BlockDeviceDriver: "virtio-scsi",
					},
				},
				devManager: &mockDeviceManager{
					devices: map[string]api.Device{
						"block-device-456": &mockBlockDevice{
							deviceInfo: &config.BlockDrive{
								SCSIAddr: "0:0:0:1",
							},
						},
					},
				},
			}),
			container: &Container{
				id:           "container-id-xyz",
				rootfsSuffix: "test-suffix",
				rootFs: RootFs{
					Source:  "/dev/sdb1",
					Type:    "xfs",
					Options: []string{"rw", "noatime"},
				},
				state: types.ContainerState{
					Fstype:        "xfs",
					BlockDeviceID: "block-device-456",
				},
			},
			wantSharedFile: &SharedFile{
				containerStorages: []*grpc.Storage{{
					MountPoint: "/run/kata-containers/shared/containers/container-id-xyz",
					Source:     "0:0:0:1",
					Fstype:     "xfs",
					Driver:     "scsi",
					Options:    []string{"rw", "noatime", "nouuid"},
				}},
				guestPath: "/run/kata-containers/shared/containers/container-id-xyz/test-suffix",
			},
		},
		"xfs block device rootfs with existing nouuid": {
			fsSharer: requireNewFilesystemShare(&Sandbox{
				config: &SandboxConfig{
					HypervisorConfig: HypervisorConfig{
						BlockDeviceDriver: "virtio-scsi",
					},
				},
				devManager: &mockDeviceManager{
					devices: map[string]api.Device{
						"block-device-789": &mockBlockDevice{
							deviceInfo: &config.BlockDrive{
								SCSIAddr: "0:0:0:2",
							},
						},
					},
				},
			}),
			container: &Container{
				id:           "container-id-def",
				rootfsSuffix: "test-suffix",
				rootFs: RootFs{
					Source:  "/dev/sdc1",
					Type:    "xfs",
					Options: []string{"rw", "nouuid", "noatime"},
				},
				state: types.ContainerState{
					Fstype:        "xfs",
					BlockDeviceID: "block-device-789",
				},
			},
			wantSharedFile: &SharedFile{
				containerStorages: []*grpc.Storage{{
					MountPoint: "/run/kata-containers/shared/containers/container-id-def",
					Source:     "0:0:0:2",
					Fstype:     "xfs",
					Driver:     "scsi",
					Options:    []string{"rw", "nouuid", "noatime"},
				}},
				guestPath: "/run/kata-containers/shared/containers/container-id-def/test-suffix",
			},
		},
	}

	for name, tc := range testCases {
		t.Run(name, func(t *testing.T) {
			assert := assert.New(t)

			sharedFile, err := tc.fsSharer.ShareRootFilesystem(context.Background(), tc.container)
			if tc.wantErr {
				assert.Error(err)
				return
			}
			assert.NoError(err)

			assert.Equal(tc.wantSharedFile, sharedFile)
		})
	}
}

// Mock types for testing block device rootfs functionality

type mockDeviceManager struct {
	devices map[string]api.Device
}

func (m *mockDeviceManager) NewDevice(config.DeviceInfo) (api.Device, error) {
	return nil, nil
}

func (m *mockDeviceManager) RemoveDevice(string) error {
	return nil
}

func (m *mockDeviceManager) AttachDevice(context.Context, string, api.DeviceReceiver) error {
	return nil
}

func (m *mockDeviceManager) DetachDevice(context.Context, string, api.DeviceReceiver) error {
	return nil
}

func (m *mockDeviceManager) IsDeviceAttached(string) bool {
	return false
}

func (m *mockDeviceManager) GetDeviceByID(id string) api.Device {
	return m.devices[id]
}

func (m *mockDeviceManager) GetAllDevices() []api.Device {
	return nil
}

func (m *mockDeviceManager) LoadDevices([]config.DeviceState) {
}

func (m *mockDeviceManager) FindDevice(*config.DeviceInfo) api.Device {
	return nil
}

type mockBlockDevice struct {
	deviceInfo *config.BlockDrive
}

func (m *mockBlockDevice) DeviceID() string {
	return "mock-device-id"
}

func (m *mockBlockDevice) DeviceType() config.DeviceType {
	return config.DeviceBlock
}

func (m *mockBlockDevice) GetDeviceInfo() interface{} {
	return m.deviceInfo
}

func (m *mockBlockDevice) Attach(context.Context, api.DeviceReceiver) error {
	return nil
}

func (m *mockBlockDevice) Detach(context.Context, api.DeviceReceiver) error {
	return nil
}

func (m *mockBlockDevice) GetMajorMinor() (int64, int64) {
	return 8, 0
}

func (m *mockBlockDevice) GetHostPath() string {
	return "/dev/mock"
}

func (m *mockBlockDevice) GetAttachCount() uint {
	return 0
}

func (m *mockBlockDevice) Reference() uint {
	return 0
}

func (m *mockBlockDevice) Dereference() uint {
	return 0
}

func (m *mockBlockDevice) Save() config.DeviceState {
	return config.DeviceState{}
}

func (m *mockBlockDevice) Load(config.DeviceState) {
}
