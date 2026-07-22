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
	"regexp"
	"syscall"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

type noFSSharingHypervisor struct {
	mockHypervisor
}

func (*noFSSharingHypervisor) Capabilities(context.Context) types.Capabilities {
	return types.Capabilities{}
}

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

func TestShareFileName(t *testing.T) {
	testCases := map[string]struct {
		containerID    string
		source         string
		destination    string
		randHex        string
		sandboxScoped  bool
		expectedResult string
	}{
		"container scoped": {
			containerID:    "container-id-abc",
			source:         "/var/lib/kubelet/pods/poduid/volumes/kubernetes.io~empty-dir/cache",
			destination:    "/mnt/cache",
			randHex:        "0011223344556677",
			expectedResult: "container-id-abc-0011223344556677-cache",
		},
		"sandbox scoped source basename": {
			containerID:    "container-id-abc",
			source:         "/var/lib/kubelet/pods/poduid/volumes/kubernetes.io~empty-dir/cache/",
			destination:    "/mnt/different-cache-name",
			randHex:        "0011223344556677",
			sandboxScoped:  true,
			expectedResult: "sandbox-0011223344556677-cache",
		},
	}

	for name, tc := range testCases {
		t.Run(name, func(t *testing.T) {
			assert.Equal(t, tc.expectedResult, shareFileName(tc.containerID, tc.source, tc.destination, tc.randHex, tc.sandboxScoped))
		})
	}
}

func TestShareFileCopyGuestPath(t *testing.T) {
	originalKataGuestSharedDir := kataGuestSharedDir
	kataGuestSharedDir = func() string {
		return "/run/user/1002" + defaultKataGuestSharedDir
	}
	t.Cleanup(func() {
		kataGuestSharedDir = originalKataGuestSharedDir
	})

	sandbox := &Sandbox{
		ctx:        context.Background(),
		id:         "sandbox-id",
		agent:      newMockAgent(),
		hypervisor: &noFSSharingHypervisor{},
	}
	fsShare, err := NewFilesystemShare(sandbox)
	require.NoError(t, err)
	t.Cleanup(func() {
		fsShare.watcher.Close()
	})

	source := filepath.Join(t.TempDir(), "resolv.conf")
	require.NoError(t, os.WriteFile(source, []byte("nameserver 192.0.2.1\n"), 0o644))

	sharedFile, err := fsShare.ShareFile(context.Background(), &Container{id: "container-id"}, &Mount{
		Source:      source,
		Destination: "/etc/resolv.conf",
	})
	require.NoError(t, err)
	require.NotNil(t, sharedFile)

	expected := "^" + regexp.QuoteMeta(defaultKataGuestSharedDir) +
		`container-id-[0-9a-f]{16}-resolv\.conf$`
	assert.Regexp(t, expected, sharedFile.guestPath)
}
