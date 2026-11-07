// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/container-storage-interface/spec/lib/go/csi"
	"github.com/golang/protobuf/ptypes/wrappers"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"k8s.io/apimachinery/pkg/api/resource"

	"kata-containers/csi-kata-directvolume/pkg/state"
	"kata-containers/csi-kata-directvolume/pkg/utils"
)

const capacityTestPool = "fast"

type capacityTestNode struct {
	name   string
	driver *directVolume
}

func newCapacityTestDriver(t *testing.T, nodeID string, poolCapacity, maxVolumeSize int64, volumes ...state.Volume) *directVolume {
	t.Helper()

	tmp := t.TempDir()
	capacity := utils.Capacity{}
	if poolCapacity >= 0 {
		capacity[capacityTestPool] = *resource.NewQuantity(poolCapacity, resource.BinarySI)
	}

	dv, err := NewDirectVolumeDriver(Config{
		DriverName:     "directvolume.csi.katacontainers.io",
		Endpoint:       "unix:///tmp/capacity-test.sock",
		NodeID:         nodeID,
		StoragePath:    filepath.Join(tmp, "storage"),
		StateDir:       filepath.Join(tmp, "state"),
		SpdkRawPath:    filepath.Join(tmp, "spdk-raw"),
		SpdkVhostPath:  filepath.Join(tmp, "spdk-vhost"),
		MaxVolumeSize:  maxVolumeSize,
		Capacity:       capacity,
		EnableTopology: true,
	})
	require.NoError(t, err)

	for _, volume := range volumes {
		require.NoError(t, dv.state.UpdateVolume(volume))
	}
	return dv
}

func getCapacityForTest(t *testing.T, dv *directVolume, kind string) *csi.GetCapacityResponse {
	t.Helper()

	parameters := map[string]string{}
	if kind != "" {
		parameters[storageKind] = kind
	}
	response, err := dv.GetCapacity(context.Background(), &csi.GetCapacityRequest{Parameters: parameters})
	require.NoError(t, err)
	require.NotNil(t, response)
	return response
}

// capacityTestSchedulingLimit models the kube-scheduler VolumeBinding predicate. It uses
// maximum_volume_size when present and falls back to available_capacity.
func capacityTestSchedulingLimit(response *csi.GetCapacityResponse) (int64, bool) {
	if response == nil {
		return 0, false
	}
	if response.MaximumVolumeSize != nil {
		return response.MaximumVolumeSize.GetValue(), true
	}
	return response.AvailableCapacity, true
}

func filterCapacityTestNodes(t *testing.T, requiredBytes int64, nodes ...capacityTestNode) []string {
	t.Helper()

	var eligible []string
	for _, node := range nodes {
		// GetCapacity reports an insufficient pool as a smaller value, not as
		// an RPC error. An error here must fail the test rather than be treated
		// as ordinary scheduler filtering.
		response := getCapacityForTest(t, node.driver, capacityTestPool)
		limit, reported := capacityTestSchedulingLimit(response)
		if reported && limit >= requiredBytes {
			eligible = append(eligible, node.name)
		}
	}
	return eligible
}

func createCapacityTestVolume(dv *directVolume, name string, requiredBytes int64) error {
	_, err := dv.CreateVolume(context.Background(), &csi.CreateVolumeRequest{
		Name: name,
		CapacityRange: &csi.CapacityRange{
			RequiredBytes: requiredBytes,
		},
		Parameters: map[string]string{
			storageKind:                          capacityTestPool,
			utils.KataContainersDirectVolumeType: utils.DirectVolumeTypeName,
		},
		VolumeCapabilities: []*csi.VolumeCapability{mountCap()},
	})
	return err
}

func TestCapacityAwareSchedulingFiltersNodes(t *testing.T) {
	nodes := []capacityTestNode{
		{
			name:   "node-small",
			driver: newCapacityTestDriver(t, "node-small", 2*utils.GiB, 8*utils.GiB),
		},
		{
			name: "node-large",
			driver: newCapacityTestDriver(t, "node-large", 10*utils.GiB, 8*utils.GiB,
				state.Volume{VolID: "pv-one", Kind: capacityTestPool, VolSize: 2 * utils.GiB},
				state.Volume{VolID: "pv-two", Kind: capacityTestPool, VolSize: 3 * utils.GiB},
				state.Volume{VolID: "pv-other-pool", Kind: "archive", VolSize: 7 * utils.GiB}),
		},
	}

	// The small node is filtered by its value, without a GetCapacity error.
	// On the large node, only the two fast-pool reservations (2 + 3 GiB) are
	// deducted; the archive-pool volume must not affect the result.
	require.Equal(t, []string{"node-large"}, filterCapacityTestNodes(t, 4*utils.GiB, nodes...))
	largeCapacity := getCapacityForTest(t, nodes[1].driver, capacityTestPool)
	require.Equal(t, int64(5*utils.GiB), largeCapacity.AvailableCapacity)
	require.Equal(t, int64(5*utils.GiB), largeCapacity.MaximumVolumeSize.GetValue())

	// A request larger than every reported limit leaves the workload pending;
	// filtering must not turn into a driver error or an arbitrary selection.
	require.Empty(t, filterCapacityTestNodes(t, 6*utils.GiB, nodes...))
}

func TestCapacityAwareSchedulingUsesMaximumVolumeSize(t *testing.T) {
	response := &csi.GetCapacityResponse{
		AvailableCapacity: 8 * utils.GiB,
		MaximumVolumeSize: &wrappers.Int64Value{Value: 3 * utils.GiB},
	}

	limit, reported := capacityTestSchedulingLimit(response)
	require.True(t, reported)
	require.Equal(t, int64(3*utils.GiB), limit)
	require.True(t, capacityTestSchedulingFits(response, 3*utils.GiB))
	require.False(t, capacityTestSchedulingFits(response, 4*utils.GiB))
	require.False(t, capacityTestSchedulingFits(nil, utils.GiB))
}

func capacityTestSchedulingFits(response *csi.GetCapacityResponse, requiredBytes int64) bool {
	limit, reported := capacityTestSchedulingLimit(response)
	return reported && limit >= requiredBytes
}

func TestCapacityAwareSchedulingMissingInformation(t *testing.T) {
	t.Run("unknown pool is not eligible", func(t *testing.T) {
		dv := newCapacityTestDriver(t, "node-unknown-pool", 8*utils.GiB, 8*utils.GiB)
		response, err := dv.GetCapacity(context.Background(), &csi.GetCapacityRequest{
			Parameters: map[string]string{storageKind: "missing"},
		})
		require.NoError(t, err)
		require.Zero(t, response.AvailableCapacity)
		require.Zero(t, response.MaximumVolumeSize.GetValue())
		require.False(t, capacityTestSchedulingFits(response, utils.GiB))
	})

	t.Run("missing pool parameter is not eligible", func(t *testing.T) {
		dv := newCapacityTestDriver(t, "node-missing-pool", 8*utils.GiB, 8*utils.GiB)
		response := getCapacityForTest(t, dv, "")
		require.Zero(t, response.AvailableCapacity)
		require.False(t, capacityTestSchedulingFits(response, utils.GiB))
	})

	t.Run("unconfigured capacity uses explicit per-volume fallback", func(t *testing.T) {
		dv := newCapacityTestDriver(t, "node-unconfigured", -1, 4*utils.GiB)
		response := getCapacityForTest(t, dv, capacityTestPool)

		// This fallback is the configured per-volume maximum, not measured free
		// disk space. Keep the behavior explicit so it is not mistaken for a
		// physical-capacity report when capacity scheduling is enabled.
		require.Equal(t, int64(4*utils.GiB), response.AvailableCapacity)
		require.Equal(t, int64(4*utils.GiB), response.MaximumVolumeSize.GetValue())
		require.True(t, capacityTestSchedulingFits(response, 4*utils.GiB))
		require.False(t, capacityTestSchedulingFits(response, 4*utils.GiB+1))
	})
}

func TestCapacityAwareSchedulingStaleReportFallback(t *testing.T) {
	nodeA := newCapacityTestDriver(t, "node-a", 4*utils.GiB, 8*utils.GiB)
	nodeB := newCapacityTestDriver(t, "node-b", 8*utils.GiB, 8*utils.GiB)

	// Take the snapshot which a scheduler could have observed before another
	// provisioner consumed the remaining capacity on node A.
	staleReport := getCapacityForTest(t, nodeA, capacityTestPool)
	require.True(t, capacityTestSchedulingFits(staleReport, 4*utils.GiB))
	require.NoError(t, createCapacityTestVolume(nodeA, "other-reservation", 4*utils.GiB))

	// The stale report must not be treated as a reservation. CreateVolume is
	// the authoritative revalidation and rejects the exhausted local pool.
	err := createCapacityTestVolume(nodeA, "requested-volume", 4*utils.GiB)
	require.Error(t, err)
	require.Equal(t, codes.ResourceExhausted, status.Code(err))
	require.Len(t, nodeA.state.GetVolumes(), 1)

	// This is the fallback contract used by delayed binding: after the failed
	// attempt, fresh capacity information excludes node A and node B remains.
	eligible := filterCapacityTestNodes(t, 4*utils.GiB,
		capacityTestNode{name: "node-a", driver: nodeA},
		capacityTestNode{name: "node-b", driver: nodeB})
	require.Equal(t, []string{"node-b"}, eligible)
	require.NoError(t, createCapacityTestVolume(nodeB, "requested-volume", 4*utils.GiB))
}
