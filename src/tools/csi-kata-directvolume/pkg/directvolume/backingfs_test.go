// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"kata-containers/csi-kata-directvolume/pkg/state"
	"kata-containers/csi-kata-directvolume/pkg/utils"
)

// fakeStorage replaces hostStorage for the filesystem-bound tests. It
// is swapped in dv.storage one driver at a time; tests using it must
// remain sequential (no t.Parallel()) like all tests in this package.
type fakeStorage struct {
	free      map[string]int64
	allocated map[string]int64
}

func newFakeStorage() *fakeStorage {
	return &fakeStorage{
		free:      map[string]int64{},
		allocated: map[string]int64{},
	}
}

func (f *fakeStorage) StatfsFree(dir string) (int64, error) {
	if free, ok := f.free[dir]; ok {
		return free, nil
	}
	return giantFreeSpace, nil
}

func (f *fakeStorage) AllocatedBytes(path string) (int64, error) {
	return f.allocated[path], nil
}

// giantFreeSpace is larger than any volume used in the tests, so the
// filesystem bound is inactive in the driver helpers unless a test
// configures f.free explicitly.
const giantFreeSpace int64 = tiB * 1024

func injectStorage(t *testing.T, dv *directVolume, storage *fakeStorage) *fakeStorage {
	t.Helper()
	dv.storage = storage
	return storage
}

// A pool may be configured larger than the backing filesystem. The
// pool check alone would let the volume through and push the failure
// to stage time (as a write error inside the guest); the filesystem
// bound rejects it at CreateVolume time with a diagnosable error
// (#14151).
func TestCreateVolumeEnforcesFilesystemBound(t *testing.T) {
	dv := newSpdkTestDriver(t, map[string]string{"fast": "1Ti"}, tiB)
	fs := injectStorage(t, dv, newFakeStorage())
	fs.free[filepath.Join(dv.config.SpdkRawPath)] = 2 * giB

	_, err := dv.CreateVolume(context.TODO(), createVolumeReq("fs-bound",
		3*giB, spdkvolParams("fast")))
	require.Error(t, err)
	require.Contains(t, err.Error(), "backing filesystem")
	require.Equal(t, codes.ResourceExhausted, status.Convert(err).Code())

	// The rejected volume must not be recorded in state, i.e. it must
	// not consume any part of the filesystem remainder later.
	_, err = dv.state.GetVolumeByName("fs-bound")
	require.Error(t, err)
}

// The filesystem bound considers only the space reserved but not yet
// allocated. A sparse SPDK file that the guest never wrote to has
// allocated nothing on disk, so its full size still counts against the
// remainder; one that is fully allocated was already taken from the
// free space of the statfs call and must not be subtracted again.
func TestFilesystemRemainingSumsOnlyUnallocated(t *testing.T) {
	dv := newCapacityTestDriver(t, nil, tiB)
	fs := injectStorage(t, dv, newFakeStorage())

	// Both the directvol and the SPDK dir report 10Gi free.
	directDir := filepath.Join(dv.config.StoragePath)
	spdkDir := filepath.Join(dv.config.SpdkRawPath)
	fs.free[directDir] = 10 * giB
	fs.free[spdkDir] = 10 * giB

	// One 2Gi directvol volume, backing disk fully allocated.
	volFull := stateVolume("vol-full", 2*giB)
	require.NoError(t, dv.state.UpdateVolume(volFull))
	devicePath := directvolPath(directDir, "vol-full", 2*giB)
	fs.allocated[devicePath] = 2 * giB

	// One 2Gi SPDK volume, sparse file, nothing allocated. Its volume
	// dir differs from the directvol dir, so it does not affect the
	// directvol remainder.
	volSparse := stateVolume("vol-sparse", 2*giB)
	volSparse.Metadata = map[string]string{
		"type":        utils.SpdkVolumeTypeName,
		"backingFile": filepath.Join(spdkDir, "vol-sparse.raw"),
	}
	require.NoError(t, dv.state.UpdateVolume(volSparse))

	// directvol remainder: 10Gi free. The fully allocated disk is
	// already missing from the free space, so nothing is subtracted
	// again (no double count).
	remaining, err := dv.filesystemRemaining(utils.DirectVolumeTypeName)
	require.NoError(t, err)
	require.EqualValues(t, 10*giB, remaining)

	// SPDK remainder: 10Gi free minus the 2Gi reserved but sparse.
	remaining, err = dv.filesystemRemaining(utils.SpdkVolumeTypeName)
	require.NoError(t, err)
	require.EqualValues(t, 8*giB, remaining)
}

// The bound also applies when capacity tracking is disabled: without
// pools the configured limit is still the real filesystem, and the
// driver must not promise more than the host can provide.
func TestFilesystemBoundWithoutCapacityTracking(t *testing.T) {
	var err error
	dv := newCapacityTestDriver(t, nil, tiB)
	fs := injectStorage(t, dv, newFakeStorage())
	fs.free[filepath.Join(dv.config.StoragePath)] = 1 * giB

	_, err = dv.CreateVolume(context.TODO(), createVolumeReq("no-tracking",
		2*giB, directvolParams("")))
	require.Error(t, err)
	require.Contains(t, err.Error(), "backing filesystem")
	require.Equal(t, codes.ResourceExhausted, status.Convert(err).Code())

	_, err = dv.CreateVolume(context.TODO(), createVolumeReq("fits",
		giB/2, directvolParams("")))
	require.NoError(t, err)
}

// "kind" is case-insensitive: the same pool requested under different
// spellings with equal values is accepted, while conflicting values
// are rejected with a stable error (the keys are sorted, so the
// message does not depend on map iteration order). Without the guard,
// the last spelling seen in map iteration order would silently win,
// which differs from run to run.
func TestCreateVolumeRejectsConflictingPoolParameters(t *testing.T) {
	dv := newCapacityTestDriver(t, map[string]string{"fast": "8Gi", "slow": "8Gi"}, tiB)

	params := map[string]string{
		utils.KataContainersDirectVolumeType: utils.DirectVolumeTypeName,
		utils.KataContainersDirectFsType:     "ext4",
		"kind":                               "fast",
		"Kind":                               "slow",
	}
	_, err := dv.CreateVolume(context.TODO(), createVolumeReq("conflict", 1*giB, params))
	require.Error(t, err)
	require.Contains(t, err.Error(), "different capacity pools")
	require.Equal(t, codes.InvalidArgument, status.Convert(err).Code())

	// Same value under two spellings: not a conflict, the canonical
	// key is propagated.
	params["Kind"] = "fast"
	resp, err := dv.CreateVolume(context.TODO(), createVolumeReq("no-conflict", 1*giB, params))
	require.NoError(t, err)
	require.Equal(t, "fast", resp.GetVolume().GetVolumeContext()[utils.StorageKind])
}

// stateVolume builds a state entry for the filesystem-accounting tests
// (the tests do not run the full stage path, which would create the
// real backing files).
func stateVolume(name string, size int64) state.Volume {
	return state.Volume{
		VolName: name,
		VolID:   name,
		VolSize: size,
		VolPath: name,
	}
}
