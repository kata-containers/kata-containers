// Copyright (c) 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	csi "github.com/container-storage-interface/spec/lib/go/csi"
	"github.com/stretchr/testify/require"
	"golang.org/x/net/context"

	"kata-containers/csi-kata-directvolume/pkg/spdkrpc"
	"kata-containers/csi-kata-directvolume/pkg/utils"
)

type nsCall struct {
	method string
	params map[string]any
}

type fakeSpdkNS struct {
	calls     []nsCall
	ret       map[string]any
	err       map[string]error
	lastCtrlr string
}

func newFakeSpdkNS() *fakeSpdkNS {
	return &fakeSpdkNS{
		ret: map[string]any{},
		err: map[string]error{},
	}
}

func (f *fakeSpdkNS) fn(method string, params map[string]any) (any, error) {
	f.calls = append(f.calls, nsCall{method, params})

	switch method {
	case "vhost_get_controllers":
		if f.lastCtrlr != "" {
			return []interface{}{
				map[string]interface{}{"ctrlr": f.lastCtrlr},
			}, nil
		}
		return []interface{}{}, nil
	case "vhost_create_blk_controller":
		if ctrlr, _ := params["ctrlr"].(string); ctrlr != "" {
			f.lastCtrlr = ctrlr
			f.ret["vhost_get_controllers"] = []map[string]any{
				{"ctrlr": ctrlr},
			}
		}
	case "vhost_delete_controller":
		if f.lastCtrlr != "" {
			f.ret["vhost_get_controllers"] = []map[string]any{}
		}
	}

	if e := f.err[method]; e != nil {
		return nil, e
	}
	return f.ret[method], nil
}

func TestSPDKStage(t *testing.T) {
	tmp := t.TempDir()

	oldRaw := utils.SpdkRawDiskDir
	utils.SpdkRawDiskDir = filepath.Join(tmp, "rawdisks")
	defer func() { utils.SpdkRawDiskDir = oldRaw }()
	require.NoError(t, os.MkdirAll(utils.SpdkRawDiskDir, 0o750))

	oldCall := spdkrpc.Call
	fs := newFakeSpdkNS()
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

	createResp, err := dv.CreateVolume(context.TODO(), &csi.CreateVolumeRequest{
		Name: "vol-spdk",
		CapacityRange: &csi.CapacityRange{
			RequiredBytes: 8 << 20,
		},
		Parameters: map[string]string{
			utils.KataContainersDirectVolumeType: utils.SpdkVolumeTypeName,
			utils.KataContainersDirectFsType:     "ext4",
		},
		VolumeCapabilities: []*csi.VolumeCapability{mountCap()},
	})
	require.NoError(t, err)
	volID := createResp.GetVolume().GetVolumeId()
	require.NotEmpty(t, volID)
	volCtx := createResp.GetVolume().GetVolumeContext()

	stagePath := filepath.Join(tmp, "stage", volID)
	require.NoError(t, os.MkdirAll(stagePath, 0o750))

	_, err = dv.NodeStageVolume(context.TODO(), &csi.NodeStageVolumeRequest{
		VolumeId:          volID,
		StagingTargetPath: stagePath,
		VolumeCapability:  mountCap(),
		VolumeContext:     volCtx,
	})
	require.NoError(t, err)

	foundCreate := false
	var createdCtrlrName string
	for _, c := range fs.calls {
		if c.method == "vhost_create_blk_controller" {
			foundCreate = true
			if ctrlr, ok := c.params["ctrlr"].(string); ok {
				createdCtrlrName = ctrlr
			}
			_, hasDev := c.params["dev_name"]
			require.True(t, hasDev, "vhost_create_blk_controller should have dev_name")
		}
	}
	require.True(t, foundCreate, "should call vhost_create_blk_controller at NodeStage")

	stateFile := filepath.Join(cfg.StateDir, "state.json")
	data, readErr := os.ReadFile(stateFile)
	require.NoError(t, readErr, "state.json should exist after NodeStage")
	require.Contains(t, string(data), `"devicePath"`, "devicePath should be recorded in state")
	require.Contains(t, string(data), volID, "state should include this volume")

	_, err = dv.NodeStageVolume(context.TODO(), &csi.NodeStageVolumeRequest{
		VolumeId:          volID,
		StagingTargetPath: stagePath,
		VolumeCapability:  mountCap(),
		VolumeContext:     volCtx,
	})
	require.NoError(t, err)

	_, err = dv.NodeUnstageVolume(context.TODO(), &csi.NodeUnstageVolumeRequest{
		VolumeId:          volID,
		StagingTargetPath: stagePath,
	})
	require.NoError(t, err)

	foundDelete := false
	for _, c := range fs.calls {
		if c.method == "vhost_delete_controller" {
			foundDelete = true
			if createdCtrlrName != "" {
				require.Equal(t, createdCtrlrName, c.params["ctrlr"])
			} else {
				_, has := c.params["ctrlr"]
				require.True(t, has, "vhost_delete_controller should have ctrlr")
			}
		}
	}
	require.True(t, foundDelete, "should call vhost_delete_controller at NodeUnstage")

	_, err = dv.NodeUnstageVolume(context.TODO(), &csi.NodeUnstageVolumeRequest{
		VolumeId:          volID,
		StagingTargetPath: stagePath,
	})
	if err != nil {
		require.Contains(t, err.Error(), "file does not exist")
	}

	_, err = dv.DeleteVolume(context.TODO(), &csi.DeleteVolumeRequest{VolumeId: volID})
	require.NoError(t, err)

	backing := filepath.Join(utils.SpdkRawDiskDir, "vol-spdk.raw")
	_, statErr := os.Stat(backing)
	require.True(t, os.IsNotExist(statErr), "backing file should be removed after DeleteVolume")

	var callsJoined strings.Builder
	for _, c := range fs.calls {
		callsJoined.WriteString(c.method)
		callsJoined.WriteString("|")
	}
	require.Contains(t, callsJoined.String(), "vhost_get_controllers",
		"should call vhost_get_controllers at least once")
}
