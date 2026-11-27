// Copyright (c) 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"context"
	"path/filepath"
	"testing"

	csi "github.com/container-storage-interface/spec/lib/go/csi"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"kata-containers/csi-kata-directvolume/pkg/spdkrpc"
	"kata-containers/csi-kata-directvolume/pkg/state"
	"kata-containers/csi-kata-directvolume/pkg/utils"
)

// newCapacityTestDriver returns a driver with the given capacity pools
// configured and a stubbed SPDK RPC layer.
func newCapacityTestDriver(t *testing.T, capacities map[string]string, maxVolumeSize int64) *directVolume {
	t.Helper()

	// The directvol path never touches SPDK, but stub the RPC out so
	// tests cannot hit a real SPDK socket by accident.
	oldCall := spdkrpc.Call
	spdkrpc.Call = func(method string, params map[string]any) (any, error) {
		t.Fatalf("unexpected SPDK RPC %s in directvol test", method)
		return nil, nil
	}
	t.Cleanup(func() { spdkrpc.Call = oldCall })

	tmp := t.TempDir()
	cfg := Config{
		DriverName:    "directvolume.csi.katacontainers.io",
		Endpoint:      "unix:///tmp/fake.sock",
		NodeID:        "node-capacity-test",
		StoragePath:   filepath.Join(tmp, "stor"),
		StateDir:      filepath.Join(tmp, "st"),
		MaxVolumeSize: maxVolumeSize,
		SpdkRawPath:   filepath.Join(tmp, "spdk-raw"),
		SpdkVhostPath: filepath.Join(tmp, "spdk-vhost"),
	}
	for kind, size := range capacities {
		require.NoError(t, cfg.Capacity.Set(kind+"="+size))
	}

	dv, err := NewDirectVolumeDriver(cfg)
	require.NoError(t, err)
	return dv
}

func directvolParams(kind string) map[string]string {
	params := map[string]string{
		utils.KataContainersDirectVolumeType: utils.DirectVolumeTypeName,
		utils.KataContainersDirectFsType:     "ext4",
	}
	if kind != "" {
		params[utils.StorageKind] = kind
	}
	return params
}

func createVolumeReq(name string, size int64, params map[string]string) *csi.CreateVolumeRequest {
	return &csi.CreateVolumeRequest{
		Name: name,
		CapacityRange: &csi.CapacityRange{
			RequiredBytes: size,
		},
		Parameters:         params,
		VolumeCapabilities: []*csi.VolumeCapability{mountCap()},
	}
}

func getCapacityReq(kind string) *csi.GetCapacityRequest {
	params := map[string]string{}
	if kind != "" {
		params[utils.StorageKind] = kind
	}
	return &csi.GetCapacityRequest{Parameters: params}
}

const (
	giB int64 = 1024 * 1024 * 1024
	tiB       = giB * 1024
)

func TestGetCapacityWithoutConfiguredPools(t *testing.T) {
	// Issue gap 2a: with capacity tracking disabled, MaxVolumeSize (the
	// per-volume limit, default 1TiB) was reported as node capacity.
	// Without a logical capacity model there is nothing trustworthy to
	// report, so the response must be zero.
	dv := newCapacityTestDriver(t, nil, tiB)

	resp, err := dv.GetCapacity(context.TODO(), getCapacityReq(""))
	require.NoError(t, err)
	require.EqualValues(t, 0, resp.GetAvailableCapacity())
	require.EqualValues(t, 0, resp.GetMaximumVolumeSize().GetValue())
	require.EqualValues(t, 0, resp.GetMinimumVolumeSize().GetValue())
}

func TestGetCapacityBasicsPerPool(t *testing.T) {
	dv := newCapacityTestDriver(t, map[string]string{"fast": "8Gi", "slow": "2Gi"}, tiB)

	t.Run("empty volume created", func(t *testing.T) {
		resp, err := dv.GetCapacity(context.TODO(), getCapacityReq("fast"))
		require.NoError(t, err)
		require.EqualValues(t, 8*giB, resp.GetAvailableCapacity())
		// max volume size clamped by per-volume maximum and available
		require.EqualValues(t, 8*giB, resp.GetMaximumVolumeSize().GetValue())
	})

	t.Run("pools are independent", func(t *testing.T) {
		dv2 := newCapacityTestDriver(t, map[string]string{"fast": "8Gi", "slow": "2Gi"}, tiB)
		_, err := dv2.CreateVolume(context.TODO(), createVolumeReq("vol-in-fast", 3*giB, directvolParams("fast")))
		require.NoError(t, err)

		fast, err := dv2.GetCapacity(context.TODO(), getCapacityReq("fast"))
		require.NoError(t, err)
		require.EqualValues(t, 5*giB, fast.GetAvailableCapacity())

		slow, err := dv2.GetCapacity(context.TODO(), getCapacityReq("slow"))
		require.NoError(t, err)
		require.EqualValues(t, 2*giB, slow.GetAvailableCapacity())
	})

	t.Run("creation reduces, deletion restores", func(t *testing.T) {
		resp, err := dv.CreateVolume(context.TODO(), createVolumeReq("vol-fast", 3*giB, directvolParams("fast")))
		require.NoError(t, err)

		after, err := dv.GetCapacity(context.TODO(), getCapacityReq("fast"))
		require.NoError(t, err)
		require.EqualValues(t, 5*giB, after.GetAvailableCapacity())

		_, err = dv.DeleteVolume(context.TODO(), &csi.DeleteVolumeRequest{VolumeId: resp.GetVolume().GetVolumeId()})
		require.NoError(t, err)

		restored, err := dv.GetCapacity(context.TODO(), getCapacityReq("fast"))
		require.NoError(t, err)
		require.EqualValues(t, 8*giB, restored.GetAvailableCapacity())
	})
}

func TestGetCapacityUnknownPoolReportsZero(t *testing.T) {
	dv := newCapacityTestDriver(t, map[string]string{"fast": "8Gi"}, tiB)

	for _, kind := range []string{"", "unknown", "fast "} {
		resp, err := dv.GetCapacity(context.TODO(), getCapacityReq(kind))
		require.NoError(t, err, "kind %q", kind)
		require.EqualValues(t, 0, resp.GetAvailableCapacity(), "kind %q", kind)
		require.EqualValues(t, 0, resp.GetMaximumVolumeSize().GetValue(), "kind %q", kind)
	}
}

// Inject an overcommitted pool directly into state to simulate the
// situations the issue describes: configuration reduced after volumes
// were recorded, or allocation which bypassed quota enforcement.
func TestGetCapacityClampsNegativeToZero(t *testing.T) {
	dv := newCapacityTestDriver(t, map[string]string{"fast": "2Gi"}, 1*giB)

	// Direct state manipulation: record 5GiB against a 2GiB pool.
	// createVolume with 5GiB would now be rejected by MaxVolumeSize, so
	// record it the way a state/config drift or bypassing path would.
	vol := createVolumeReq("drifted", 5*giB, directvolParams("fast"))
	resp, err := dv.CreateVolume(context.TODO(), vol)
	require.Error(t, err) // 5GiB > max volume size 1GiB, correctly rejected
	require.Nil(t, resp)

	err = dv.state.UpdateVolume(state.Volume{VolID: "drifted", VolName: "drifted", VolSize: 5 * giB, Kind: "fast"})
	require.NoError(t, err)

	after, err := dv.GetCapacity(context.TODO(), getCapacityReq("fast"))
	require.NoError(t, err)
	require.EqualValues(t, 0, after.GetAvailableCapacity(), "negative capacity must be clamped to zero")
	require.EqualValues(t, 0, after.GetMaximumVolumeSize().GetValue())
}

func TestCreateVolumeKindParameterRequiredWithCapacity(t *testing.T) {
	dv := newCapacityTestDriver(t, map[string]string{"fast": "8Gi"}, tiB)

	// Issue gap 3: the kind parameter was dropped by CreateVolume's
	// parameter filter, so volumes were never accounted against the
	// pool the StorageClass named.
	_, err := dv.CreateVolume(context.TODO(), createVolumeReq("no-kind", 1*giB, directvolParams("")))
	require.Error(t, err)
	require.Contains(t, err.Error(), "capacity pool")

	_, err = dv.CreateVolume(context.TODO(), createVolumeReq("bad-kind", 1*giB, directvolParams("unknown")))
	require.Error(t, err)
	require.Contains(t, err.Error(), "unknown capacity pool")

	// A valid kind is accepted, propagated and persisted.
	resp, err := dv.CreateVolume(context.TODO(), createVolumeReq("good-kind", 1*giB, directvolParams("fast")))
	require.NoError(t, err)
	require.Equal(t, "fast", resp.GetVolume().GetVolumeContext()[utils.StorageKind])

	vol, err := dv.state.GetVolumeByName("good-kind")
	require.NoError(t, err)
	require.Equal(t, "fast", vol.Kind)

	// Accounting went to the requested pool.
	cap, err := dv.GetCapacity(context.TODO(), getCapacityReq("fast"))
	require.NoError(t, err)
	require.EqualValues(t, 7*giB, cap.GetAvailableCapacity())
}

func TestCreateVolumeKindRejectedWithoutCapacity(t *testing.T) {
	dv := newCapacityTestDriver(t, nil, tiB)

	_, err := dv.CreateVolume(context.TODO(), createVolumeReq("kind-no-capacity", 1*giB, directvolParams("fast")))
	require.Error(t, err)
	require.Contains(t, err.Error(), "capacity tracking")

	// Without the kind parameter, volume creation is unaffected.
	_, err = dv.CreateVolume(context.TODO(), createVolumeReq("plain", 1*giB, directvolParams("")))
	require.NoError(t, err)
}

func TestCreateVolumePoolExhaustion(t *testing.T) {
	dv := newCapacityTestDriver(t, map[string]string{"fast": "2Gi"}, tiB)

	// Fill the pool: 1.5GiB then request 1GiB (only 0.5GiB left).
	_, err := dv.CreateVolume(context.TODO(), createVolumeReq("filler", 3*giB/2, directvolParams("fast")))
	require.NoError(t, err)

	_, err = dv.CreateVolume(context.TODO(), createVolumeReq("second", 1*giB, directvolParams("fast")))
	require.Error(t, err)
	require.Contains(t, err.Error(), "remaining capacity")
	require.Equal(t, codes.ResourceExhausted, status.Convert(err).Code())
}

func TestCreateVolumeIdempotent(t *testing.T) {
	dv := newCapacityTestDriver(t, map[string]string{"fast": "8Gi"}, tiB)

	resp1, err := dv.CreateVolume(context.TODO(), createVolumeReq("same", 2*giB, directvolParams("fast")))
	require.NoError(t, err)
	resp2, err := dv.CreateVolume(context.TODO(), createVolumeReq("same", 2*giB, directvolParams("fast")))
	require.NoError(t, err)

	// Same volume returned, capacity reserved exactly once.
	require.Equal(t, resp1.GetVolume().GetVolumeId(), resp2.GetVolume().GetVolumeId())
	cap, err := dv.GetCapacity(context.TODO(), getCapacityReq("fast"))
	require.NoError(t, err)
	require.EqualValues(t, 6*giB, cap.GetAvailableCapacity())

	// Smaller re-request is also idempotent (recorded size is kept).
	_, err = dv.CreateVolume(context.TODO(), createVolumeReq("same", 1*giB, directvolParams("fast")))
	require.NoError(t, err)
	cap, err = dv.GetCapacity(context.TODO(), getCapacityReq("fast"))
	require.NoError(t, err)
	require.EqualValues(t, 6*giB, cap.GetAvailableCapacity())
}

func TestCapacitySurvivesRestart(t *testing.T) {
	tmp := t.TempDir()

	build := func() *directVolume {
		cfg := Config{
			DriverName:    "directvolume.csi.katacontainers.io",
			Endpoint:      "unix:///tmp/fake.sock",
			NodeID:        "node-restart-test",
			StoragePath:   filepath.Join(tmp, "stor"),
			StateDir:      filepath.Join(tmp, "st"),
			MaxVolumeSize: tiB,
			SpdkRawPath:   filepath.Join(tmp, "spdk-raw"),
			SpdkVhostPath: filepath.Join(tmp, "spdk-vhost"),
		}
		require.NoError(t, cfg.Capacity.Set("fast=8Gi"))
		oldCall := spdkrpc.Call
		spdkrpc.Call = func(string, map[string]any) (any, error) { return nil, nil }
		t.Cleanup(func() { spdkrpc.Call = oldCall })

		dv, err := NewDirectVolumeDriver(cfg)
		require.NoError(t, err)
		return dv
	}

	dv := build()
	_, err := dv.CreateVolume(context.TODO(), createVolumeReq("persistent", 3*giB, directvolParams("fast")))
	require.NoError(t, err)

	// Simulate daemon restart: same state dir, fresh driver instance.
	dv2 := build()
	cap, err := dv2.GetCapacity(context.TODO(), getCapacityReq("fast"))
	require.NoError(t, err)
	require.EqualValues(t, 5*giB, cap.GetAvailableCapacity(), "reservations must survive restart")
}

func TestGetCapacityMaximumVolumeSizeInvariant(t *testing.T) {
	// 0 <= maximum_volume_size <= available_capacity, also when the
	// per-volume maximum is larger than the remaining pool space.
	dv := newCapacityTestDriver(t, map[string]string{"fast": "2Gi"}, 8*giB)

	_, err := dv.CreateVolume(context.TODO(), createVolumeReq("vol", 3*giB/2, directvolParams("fast")))
	require.NoError(t, err)

	resp, err := dv.GetCapacity(context.TODO(), getCapacityReq("fast"))
	require.NoError(t, err)
	require.EqualValues(t, 1*giB/2, resp.GetAvailableCapacity())
	require.EqualValues(t, 1*giB/2, resp.GetMaximumVolumeSize().GetValue())
	require.LessOrEqual(t, resp.GetMaximumVolumeSize().GetValue(), resp.GetAvailableCapacity())
	require.GreaterOrEqual(t, resp.GetMaximumVolumeSize().GetValue(), int64(0))
}

// getCapacityReqWithTopology is getCapacityReq plus accessibility
// segments replicating what the external-provisioner forwards after
// picking a node (#14151, gap 5).
func getCapacityReqWithTopology(kind string, segments map[string]string) *csi.GetCapacityRequest {
	req := getCapacityReq(kind)
	req.AccessibleTopology = &csi.Topology{Segments: segments}
	return req
}

// topologyTestDriver is newCapacityTestDriver with a controlled
// EnableTopology flag.
func topologyTestDriver(t *testing.T, capacities map[string]string, maxVolumeSize int64, enableTopology bool) *directVolume {
	t.Helper()
	old := newCapacityTestDriver(t, capacities, maxVolumeSize)
	// newCapacityTestDriver leaves EnableTopology at its zero value
	// (disabled); flip it when the test needs the enabled behavior.
	if enableTopology {
		old.config.EnableTopology = true
	}
	return old
}

// A GetCapacity request carrying the topology of another node must
// report zero, not this node's capacity (#14151, gap 5): a central
// caller asking "how much can you provision on node-b?" must not get
// an answer that is only true for node-a.
func TestGetCapacityForeignTopologyReportsZero(t *testing.T) {
	dv := topologyTestDriver(t, map[string]string{"fast": "8Gi"}, tiB, true)

	segments := map[string]string{TopologyKeyNode: "some-other-node"}
	resp, err := dv.GetCapacity(context.TODO(), getCapacityReqWithTopology("fast", segments))
	require.NoError(t, err)
	require.EqualValues(t, 0, resp.GetAvailableCapacity())
	require.EqualValues(t, 0, resp.GetMaximumVolumeSize().GetValue())
	require.EqualValues(t, 0, resp.GetMinimumVolumeSize().GetValue())
}

// The local node's own topology still gets the full answer. A caller
// may forward more keys than the driver publishes (the
// external-provisioner forwards the full segment set of the node it
// picked), so extra keys alongside the driver's own must not
// disqualify an otherwise local request.
func TestGetCapacityLocalTopologyReportsCapacity(t *testing.T) {
	dv := topologyTestDriver(t, map[string]string{"fast": "8Gi"}, tiB, true)

	segments := map[string]string{
		TopologyKeyNode: dv.config.NodeID,
	}
	resp, err := dv.GetCapacity(context.TODO(), getCapacityReqWithTopology("fast", segments))
	require.NoError(t, err)
	require.EqualValues(t, 8*giB, resp.GetAvailableCapacity())

	// A fortiori: additional keys alongside the driver's own.
	segments["some.example.com/zone"] = "zone-a"
	resp, err = dv.GetCapacity(context.TODO(), getCapacityReqWithTopology("fast", segments))
	require.NoError(t, err)
	require.EqualValues(t, 8*giB, resp.GetAvailableCapacity())
}

// Without topology, the caller does not restrict placement and the
// node answers with its own capacity.
func TestGetCapacityNoTopologyReportsCapacity(t *testing.T) {
	dv := topologyTestDriver(t, map[string]string{"fast": "8Gi"}, tiB, true)

	resp, err := dv.GetCapacity(context.TODO(), getCapacityReq("fast"))
	require.NoError(t, err)
	require.EqualValues(t, 8*giB, resp.GetAvailableCapacity())
}

// A topology request on a driver that does not publish topology is
// unsatisfiable here: the node never announced any segment, so it must
// not claim capacity for a placement it cannot be selected for.
func TestGetCapacityTopologyWithoutPublicationReportsZero(t *testing.T) {
	dv := topologyTestDriver(t, map[string]string{"fast": "8Gi"}, tiB, false)

	segments := map[string]string{TopologyKeyNode: dv.config.NodeID}
	resp, err := dv.GetCapacity(context.TODO(), getCapacityReqWithTopology("fast", segments))
	require.NoError(t, err)
	require.EqualValues(t, 0, resp.GetAvailableCapacity())
}

// The controller must advertise GET_CAPACITY now that the RPC is
// implemented and called by the external-provisioner capacity
// controller (#14151, gap 6).
func TestControllerGetCapabilitiesIncludesGetCapacity(t *testing.T) {
	dv := newCapacityTestDriver(t, nil, tiB)

	resp, err := dv.ControllerGetCapabilities(context.TODO(), &csi.ControllerGetCapabilitiesRequest{})
	require.NoError(t, err)

	var rpcs []csi.ControllerServiceCapability_RPC_Type
	for _, c := range resp.GetCapabilities() {
		rpcs = append(rpcs, c.GetRpc().GetType())
	}
	require.Contains(t, rpcs, csi.ControllerServiceCapability_RPC_CREATE_DELETE_VOLUME)
	require.Contains(t, rpcs, csi.ControllerServiceCapability_RPC_GET_CAPACITY)
}

// GetCapacity validates the advertised capability like the other
// controller RPCs; it never reached this validation before the
// capability was advertised.
func TestGetCapacityValidatesServerCapability(t *testing.T) {
	dv := newCapacityTestDriver(t, map[string]string{"fast": "8Gi"}, tiB)
	require.NoError(t, dv.validateControllerServiceRequest(csi.ControllerServiceCapability_RPC_GET_CAPACITY))
}

// Topology zeros out only the capacity of mismatching requests; the
// pool check itself must still reject an unknown pool with zero (not
// an error), preserving the semantics from before the topology check
// existed.
func TestGetCapacityForeignTopologyUnknownPoolStillZero(t *testing.T) {
	dv := topologyTestDriver(t, map[string]string{"fast": "8Gi"}, tiB, true)

	resp, err := dv.GetCapacity(context.TODO(), getCapacityReqWithTopology("nosuch", map[string]string{TopologyKeyNode: "other"}))
	require.NoError(t, err)
	require.EqualValues(t, 0, resp.GetAvailableCapacity())
}
