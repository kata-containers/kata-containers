// Copyright (c) 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"kata-containers/csi-kata-directvolume/pkg/spdkrpc"
	"kata-containers/csi-kata-directvolume/pkg/utils"
)

// newSpdkTestDriver returns a driver whose SPDK RPCs succeed via stub.
func newSpdkTestDriver(t *testing.T, capacities map[string]string, maxVolumeSize int64) *directVolume {
	t.Helper()

	oldCall := spdkrpc.Call
	spdkrpc.Call = func(method string, params map[string]any) (any, error) {
		switch method {
		case "bdev_aio_create", "bdev_aio_delete":
			return nil, nil
		default:
			t.Fatalf("unexpected SPDK RPC %s", method)
			return nil, nil
		}
	}
	t.Cleanup(func() { spdkrpc.Call = oldCall })

	tmp := t.TempDir()
	cfg := Config{
		DriverName:    "directvolume.csi.katacontainers.io",
		Endpoint:      "unix:///tmp/fake.sock",
		NodeID:        "node-spdk-test",
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

func spdkvolParams(kind string) map[string]string {
	params := map[string]string{
		utils.KataContainersDirectVolumeType: utils.SpdkVolumeTypeName,
	}
	if kind != "" {
		params[utils.StorageKind] = kind
	}
	return params
}

// The spdkvol path previously created volumes without any quota check
// (#14151, gap 4): neither the per-volume maximum nor the pool's
// remaining capacity was enforced, so the pool could be over-allocated.
func TestCreateSPDKVolumeEnforcesPoolQuota(t *testing.T) {
	dv := newSpdkTestDriver(t, map[string]string{"fast": "2Gi"}, tiB)

	_, err := dv.CreateVolume(context.TODO(), createVolumeReq("spdk-filler", 3*giB/2, spdkvolParams("fast")))
	require.NoError(t, err)

	// Only 0.5Gi left, a 1Gi request must be rejected.
	_, err = dv.CreateVolume(context.TODO(), createVolumeReq("spdk-second", 1*giB, spdkvolParams("fast")))
	require.Error(t, err)
	require.Contains(t, err.Error(), "remaining capacity")
	require.Equal(t, codes.ResourceExhausted, status.Convert(err).Code())

	// The rejected volume must not be recorded in state, otherwise it
	// would count against the pool without existing.
	_, err = dv.state.GetVolumeByName("spdk-second")
	require.Error(t, err)
}

func TestCreateSPDKVolumeEnforcesMaxVolumeSize(t *testing.T) {
	dv := newSpdkTestDriver(t, map[string]string{"fast": "8Gi"}, 1*giB)

	_, err := dv.CreateVolume(context.TODO(), createVolumeReq("spdk-too-big", 2*giB, spdkvolParams("fast")))
	require.Error(t, err)
	require.Contains(t, err.Error(), "maximum allowed")
	require.Equal(t, codes.OutOfRange, status.Convert(err).Code())
}

// Reusing an existing backing file (retry after the state file was
// lost, or idempotent CreateVolume retry) must not re-run the quota
// check: the reservation is already accounted for, and re-checking
// would double-count it or wrongly reject the recovery.
func TestCreateSPDKVolumeReuseSkipsQuotaRecheck(t *testing.T) {
	dv := newSpdkTestDriver(t, map[string]string{"fast": "10Gi"}, tiB)

	resp, err := dv.CreateVolume(context.TODO(), createVolumeReq("spdk-reuse", 3*giB, spdkvolParams("fast")))
	require.NoError(t, err)

	// The 3GiB backing file exists on disk now.
	backingFile := filepath.Join(dv.config.SpdkRawPath, "spdk-reuse.raw")
	fi, err := os.Stat(backingFile)
	require.NoError(t, err)
	require.EqualValues(t, 3*giB, fi.Size())

	// Re-register the same volume after shrinking the pool to exactly
	// the used size: reuse must not re-run the quota check (the
	// reservation is already accounted for), so recovery succeeds
	// instead of being rejected by a double check.
	require.NoError(t, dv.config.Capacity.Set("fast=3Gi"))
	vol, err := dv.createSPDKVolume(resp.GetVolume().GetVolumeId(), "spdk-reuse", 3*giB, "fast")
	require.NoError(t, err)
	require.EqualValues(t, 3*giB, vol.VolSize)

	// A genuinely new volume in the exhausted pool is still rejected.
	_, err = dv.createSPDKVolume("new-vol", "new-vol", 1*giB, "fast")
	require.Error(t, err)
	require.Contains(t, err.Error(), "remaining capacity")
}

// Legacy volumes (created before the kind parameter existed, state
// entry lost) have no kind in their VolumeContext. With capacity
// tracking enabled they must still be re-registered by the
// stage/publish backfill path: they are recorded with an empty kind
// and are not counted against any pool (#14151, gap 8).
func TestBackfillLegacyVolumeWithoutKind(t *testing.T) {
	dv := newCapacityTestDriver(t, map[string]string{"fast": "8Gi"}, tiB)

	vol, err := dv.backfillVolume("legacy-vol", "legacy-vol", 2*giB, "")
	require.NoError(t, err)
	require.Equal(t, "", vol.Kind)

	// The volume is recorded in state and stageable/publishable.
	got, err := dv.state.GetVolumeByID("legacy-vol")
	require.NoError(t, err)
	require.EqualValues(t, 2*giB, got.VolSize)

	// It is not charged to any pool.
	cap, err := dv.GetCapacity(context.TODO(), getCapacityReq("fast"))
	require.NoError(t, err)
	require.EqualValues(t, 8*giB, cap.GetAvailableCapacity(), "legacy volume must not count against a pool")
}

// A non-legacy backfill (kind present) behaves like a normal creation:
// charged to the named pool.
func TestBackfillVolumeWithKindIsAccounted(t *testing.T) {
	dv := newCapacityTestDriver(t, map[string]string{"fast": "8Gi"}, tiB)

	vol, err := dv.backfillVolume("modern-vol", "modern-vol", 2*giB, "fast")
	require.NoError(t, err)
	require.Equal(t, "fast", vol.Kind)

	cap, err := dv.GetCapacity(context.TODO(), getCapacityReq("fast"))
	require.NoError(t, err)
	require.EqualValues(t, 6*giB, cap.GetAvailableCapacity())
}

// Even untracked legacy volumes are still bounded by the per-volume
// maximum: a backfill beyond it is rejected.
func TestBackfillLegacyVolumeEnforcesMaxVolumeSize(t *testing.T) {
	dv := newCapacityTestDriver(t, map[string]string{"fast": "8Gi"}, 1*giB)

	_, err := dv.backfillVolume("legacy-too-big", "legacy-too-big", 2*giB, "")
	require.Error(t, err)
	require.Contains(t, err.Error(), "maximum allowed")
	require.Equal(t, codes.OutOfRange, status.Convert(err).Code())
}

// With capacity tracking disabled there is no kind and nothing to
// backfill around: the volume is created as before.
func TestBackfillWithoutCapacityTracking(t *testing.T) {
	dv := newCapacityTestDriver(t, nil, tiB)

	vol, err := dv.backfillVolume("plain-vol", "plain-vol", 2*giB, "")
	require.NoError(t, err)
	require.Equal(t, "", vol.Kind)
}
