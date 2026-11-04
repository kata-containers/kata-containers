// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"context"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	csi "github.com/container-storage-interface/spec/lib/go/csi"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/credentials/insecure"

	"kata-containers/csi-kata-directvolume/pkg/utils"
)

// serveGRPC serves the CSI services of d on a unix socket and returns a
// client connection to it.
func serveGRPC(t *testing.T, d *testDriver) *grpc.ClientConn {
	t.Helper()
	sock := filepath.Join(t.TempDir(), "csi.sock")
	listener, err := net.Listen("unix", sock)
	require.NoError(t, err)

	server := grpc.NewServer()
	csi.RegisterIdentityServer(server, d.directVolume)
	csi.RegisterControllerServer(server, d.directVolume)
	csi.RegisterNodeServer(server, d.directVolume)
	go func() {
		_ = server.Serve(listener)
	}()
	t.Cleanup(server.Stop)

	conn, err := grpc.NewClient("unix://"+sock, grpc.WithTransportCredentials(insecure.NewCredentials()))
	require.NoError(t, err)
	t.Cleanup(func() { _ = conn.Close() })
	return conn
}

// TestGetCapacityOverGRPC exercises capacity reporting through the CSI
// gRPC API, the way the external-provisioner uses it.
func TestGetCapacityOverGRPC(t *testing.T) {
	d := newTestDriver(t, testDriverOptions{capacity: []string{"fast=8Gi"}, maxVolumeSize: 6 * utils.GiB})
	conn := serveGRPC(t, d)
	ctx := context.Background()

	identity := csi.NewIdentityClient(conn)
	pluginCaps, err := identity.GetPluginCapabilities(ctx, &csi.GetPluginCapabilitiesRequest{})
	require.NoError(t, err)
	var services []csi.PluginCapability_Service_Type
	for _, c := range pluginCaps.GetCapabilities() {
		services = append(services, c.GetService().GetType())
	}
	require.Contains(t, services, csi.PluginCapability_Service_VOLUME_ACCESSIBILITY_CONSTRAINTS)

	node := csi.NewNodeClient(conn)
	info, err := node.NodeGetInfo(ctx, &csi.NodeGetInfoRequest{})
	require.NoError(t, err)
	require.Equal(t, testNodeID, info.GetNodeId())
	require.Equal(t, map[string]string{TopologyKeyNode: testNodeID}, info.GetAccessibleTopology().GetSegments())

	controller := csi.NewControllerClient(conn)
	controllerCaps, err := controller.ControllerGetCapabilities(ctx, &csi.ControllerGetCapabilitiesRequest{})
	require.NoError(t, err)
	var rpcs []csi.ControllerServiceCapability_RPC_Type
	for _, c := range controllerCaps.GetCapabilities() {
		rpcs = append(rpcs, c.GetRpc().GetType())
	}
	require.Contains(t, rpcs, csi.ControllerServiceCapability_RPC_GET_CAPACITY)

	// The segment comes from NodeGetInfo, as in the external-provisioner.
	capacityReq := capacityRequest(utils.DirectVolumeTypeName, "fast")
	capacityReq.AccessibleTopology = info.GetAccessibleTopology()
	requireGRPCCapacity := func(available, maximum int64) {
		t.Helper()
		resp, err := controller.GetCapacity(ctx, capacityReq)
		require.NoError(t, err)
		require.Equal(t, available, resp.GetAvailableCapacity())
		require.NotNil(t, resp.GetMaximumVolumeSize())
		require.Equal(t, maximum, resp.GetMaximumVolumeSize().GetValue())
	}
	requireGRPCCapacity(8*utils.GiB, 6*utils.GiB)

	createReq := createRequest("vol-1", 5*utils.GiB, utils.DirectVolumeTypeName, "fast")
	createReq.AccessibilityRequirements = &csi.TopologyRequirement{
		Requisite: []*csi.Topology{info.GetAccessibleTopology()},
		Preferred: []*csi.Topology{info.GetAccessibleTopology()},
	}
	created, err := controller.CreateVolume(ctx, createReq)
	require.NoError(t, err)
	requireGRPCCapacity(3*utils.GiB, 3*utils.GiB)

	_, err = controller.CreateVolume(ctx, createRequest("vol-2", 4*utils.GiB, utils.DirectVolumeTypeName, "fast"))
	requireCode(t, err, codes.ResourceExhausted)

	_, err = controller.DeleteVolume(ctx, &csi.DeleteVolumeRequest{VolumeId: created.GetVolume().GetVolumeId()})
	require.NoError(t, err)
	requireGRPCCapacity(8*utils.GiB, 6*utils.GiB)

	// No capacity is reported for another node or an unknown pool.
	otherNode := capacityRequest(utils.DirectVolumeTypeName, "fast")
	otherNode.AccessibleTopology = nodeTopology(otherNodeID)
	resp, err := controller.GetCapacity(ctx, otherNode)
	require.NoError(t, err)
	require.Zero(t, resp.GetAvailableCapacity())
	resp, err = controller.GetCapacity(ctx, capacityRequest(utils.DirectVolumeTypeName, "slow"))
	require.NoError(t, err)
	require.Zero(t, resp.GetAvailableCapacity())
}

// TestHostStorage checks the filesystem accounting against real files.
func TestHostStorage(t *testing.T) {
	dir := t.TempDir()
	var storage hostStorage

	fs, err := storage.Statfs(dir)
	require.NoError(t, err)
	require.Positive(t, fs.free)
	other, err := storage.Statfs(dir)
	require.NoError(t, err)
	require.Equal(t, fs.device, other.device)
	_, err = storage.Statfs(filepath.Join(dir, "missing"))
	require.Error(t, err)

	allocated, err := storage.AllocatedBytes(filepath.Join(dir, "missing"))
	require.NoError(t, err)
	require.Zero(t, allocated)
	allocated, err = storage.AllocatedBytes("")
	require.NoError(t, err)
	require.Zero(t, allocated)

	// A sparse file, like the backing file of an SPDK volume.
	sparse := filepath.Join(dir, "sparse.raw")
	require.NoError(t, createSparseFile(sparse, 64*utils.MiB))
	allocated, err = storage.AllocatedBytes(sparse)
	require.NoError(t, err)
	require.Less(t, allocated, utils.MiB)

	// A preallocated file, like the raw disk of a directvol volume.
	if _, err := exec.LookPath("fallocate"); err != nil {
		t.Skip("fallocate is not installed")
	}
	preallocated := filepath.Join(dir, "directvol-rawdisk.16M")
	if out, err := exec.Command("fallocate", "-l", "16M", preallocated).CombinedOutput(); err != nil {
		t.Skipf("fallocate is not supported here: %v: %s", err, out)
	}
	allocated, err = storage.AllocatedBytes(preallocated)
	require.NoError(t, err)
	require.GreaterOrEqual(t, allocated, 16*utils.MiB)

	require.NoError(t, os.Remove(preallocated))
}
