// Copyright (c) 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"os"
	"path/filepath"
	"testing"

	csi "github.com/container-storage-interface/spec/lib/go/csi"
	"github.com/stretchr/testify/require"
	"golang.org/x/net/context"

	"kata-containers/csi-kata-directvolume/pkg/spdkrpc"
	"kata-containers/csi-kata-directvolume/pkg/utils"
)

type call struct {
	method string
	params map[string]any
}
type fakeSpdk struct {
	calls []call
	ret   map[string]any
	err   map[string]error
}

func newFakeSpdk() *fakeSpdk {
	return &fakeSpdk{
		ret: map[string]any{},
		err: map[string]error{},
	}
}
func (f *fakeSpdk) fn(method string, params map[string]any) (any, error) {
	f.calls = append(f.calls, call{method, params})
	if e := f.err[method]; e != nil {
		return nil, e
	}
	return f.ret[method], nil
}

func mountCap() *csi.VolumeCapability {
	return &csi.VolumeCapability{
		AccessType: &csi.VolumeCapability_Mount{
			Mount: &csi.VolumeCapability_MountVolume{},
		},
	}
}

func TestSPDKVolume(t *testing.T) {
	tmp := t.TempDir()

	oldDir := utils.SpdkRawDiskDir
	utils.SpdkRawDiskDir = filepath.Join(tmp, "rawdisks")
	defer func() { utils.SpdkRawDiskDir = oldDir }()
	require.NoError(t, os.MkdirAll(utils.SpdkRawDiskDir, 0o750))

	oldCall := spdkrpc.Call
	fs := newFakeSpdk()
	spdkrpc.Call = fs.fn
	defer func() { spdkrpc.Call = oldCall }()

	cfg := Config{
		DriverName:    "directvolume.csi.katacontainers.io",
		Endpoint:      "unix:///tmp/fake.sock",
		NodeID:        "node-test",
		StoragePath:   filepath.Join(tmp, "stor"),
		StateDir:      filepath.Join(tmp, "st"),
		MaxVolumeSize: 1 << 40,
		SpdkRawPath:   filepath.Join(tmp, "spdk-raw"),
		SpdkVhostPath: filepath.Join(tmp, "spdk-vhost"),
	}
	dv, err := NewDirectVolumeDriver(cfg)
	require.NoError(t, err)

	req := &csi.CreateVolumeRequest{
		Name: "vol-spdk",
		CapacityRange: &csi.CapacityRange{
			RequiredBytes: 1 << 20,
		},
		Parameters: map[string]string{
			utils.KataContainersDirectVolumeType: utils.SpdkVolumeTypeName,
			utils.KataContainersDirectFsType:     "ext4",
		},
		VolumeCapabilities: []*csi.VolumeCapability{mountCap()},
	}
	resp, err := dv.CreateVolume(context.TODO(), req)
	require.NoError(t, err)
	require.NotNil(t, resp.GetVolume())
	volID := resp.GetVolume().GetVolumeId()
	require.NotEmpty(t, volID)

	backing := filepath.Join(utils.SpdkRawDiskDir, "vol-spdk.raw")
	info, statErr := os.Stat(backing)
	require.NoError(t, statErr, "backing file should exist")
	require.EqualValues(t, 1<<20, info.Size())

	require.NotEmpty(t, fs.calls)
	foundCreate := false
	for _, c := range fs.calls {
		if c.method == "bdev_aio_create" {
			foundCreate = true
			require.Equal(t, backing, c.params["filename"])
			require.Equal(t, "bdev-vol-spdk", c.params["name"])
		}
	}
	require.True(t, foundCreate, "should call bdev_aio_create")

	_, err = dv.DeleteVolume(context.TODO(), &csi.DeleteVolumeRequest{VolumeId: volID})
	require.NoError(t, err)

	foundDelete := false
	for _, c := range fs.calls {
		if c.method == "bdev_aio_delete" {
			foundDelete = true
			require.Equal(t, "bdev-vol-spdk", c.params["name"])
		}
	}
	require.True(t, foundDelete, "should call bdev_aio_delete")

	_, err = os.Stat(backing)
	require.True(t, os.IsNotExist(err), "backing file should be removed after DeleteVolume")

	_, err = dv.DeleteVolume(context.TODO(), &csi.DeleteVolumeRequest{VolumeId: volID})
	require.NoError(t, err)
}
