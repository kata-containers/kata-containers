//
// Copyright 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"errors"
	"fmt"
	"math"
	"path/filepath"

	"golang.org/x/sys/unix"

	"kata-containers/csi-kata-directvolume/pkg/state"
	"kata-containers/csi-kata-directvolume/pkg/utils"
)

// statBlockSize is the unit of unix.Stat_t.Blocks.
const statBlockSize = 512

// backingStorage gives access to the filesystems which hold the backing
// files of volumes. It is an interface so tests can replace the real
// filesystem with a fake.
type backingStorage interface {
	// Statfs returns the number of bytes available on the filesystem
	// holding dir (available = usable by unprivileged writes, matching
	// what a volume creation would consume).
	StatfsFree(dir string) (int64, error)
	// AllocatedBytes returns the number of bytes already allocated on
	// disk for the file at path, or 0 when the file does not exist.
	AllocatedBytes(path string) (int64, error)
}

// hostStorage is the real backingStorage.
type hostStorage struct{}

func (hostStorage) StatfsFree(dir string) (int64, error) {
	var fs unix.Statfs_t
	if err := unix.Statfs(dir, &fs); err != nil {
		return 0, err
	}

	// The integer types of these fields differ between architectures.
	blockSize := int64(fs.Frsize)
	if blockSize <= 0 {
		blockSize = int64(fs.Bsize)
	}
	available := uint64(fs.Bavail)
	free := int64(math.MaxInt64)
	if blockSize > 0 && available < uint64(math.MaxInt64/blockSize) {
		free = int64(available) * blockSize
	}
	return free, nil
}

func (hostStorage) AllocatedBytes(path string) (int64, error) {
	if path == "" {
		return 0, nil
	}
	var st unix.Stat_t
	if err := unix.Stat(path, &st); err != nil {
		if errors.Is(err, unix.ENOENT) {
			return 0, nil
		}
		return 0, err
	}
	return int64(st.Blocks) * statBlockSize, nil
}

// backingDir returns the directory which holds the backing files of
// volumes of the given type.
func (dv *directVolume) backingDir(volumeType string) string {
	if volumeType == utils.SpdkVolumeTypeName {
		return dv.config.SpdkRawPath
	}
	return dv.config.StoragePath
}

// volumeTypeOf returns the type of an existing volume from its state
// entry.
func volumeTypeOf(vol state.Volume) string {
	if volumeType := vol.Metadata["type"]; volumeType != "" {
		return volumeType
	}
	return utils.DirectVolumeTypeName
}

// directvolPath is the layout of the backing disk of a directvol
// volume: storagePath/<volume ID>/directvol-rawdisk.<size in MiB>.
// It mirrors utils.CreateDirectBlockDevice, which creates the file but
// does not export its path.
func directvolPath(storagePath, volID string, volSize int64) string {
	return filepath.Join(storagePath, volID, fmt.Sprintf("directvol-rawdisk.%dM", volSize/utils.MiB))
}

// backingFile returns the path of the file which backs an existing
// volume. It is only used for the space accounting below, so a missing
// disk (recorded but never staged) is acceptable: backing storage then
// reports zero allocated bytes and the whole reservation is subtracted.
func (dv *directVolume) backingFile(vol state.Volume) string {
	if volumeTypeOf(vol) == utils.SpdkVolumeTypeName {
		return vol.Metadata["backingFile"]
	}
	return directvolPath(dv.config.StoragePath, vol.VolID, vol.VolSize)
}

// filesystemRemaining returns how many bytes new volumes of the given
// type can still take from the filesystem which holds their backing
// files: its free space minus the part of every existing volume on that
// same filesystem which is reserved (recorded in state) but not
// allocated on disk yet.
//
// The allocated part of a backing file is already missing from the free
// space, so only the rest of each reservation is subtracted — that way
// no byte is counted twice and sparse backing files (SPDK volumes are
// created sparse and only allocate blocks when the guest writes) cannot
// overcommit the filesystem. This is the real-storage counterpart of
// the configured pool accounting in checkQuota: pools bound what a kind
// may reserve in total, this bound what the host can actually provide
// (#14151).
func (dv *directVolume) filesystemRemaining(volumeType string) (int64, error) {
	dir := dv.backingDir(volumeType)
	free, err := dv.storage.StatfsFree(dir)
	if err != nil {
		return 0, errors.New("check free space of " + dir + ": " + err.Error())
	}

	unallocated := int64(0)
	for _, vol := range dv.state.GetVolumes() {
		if dv.backingDir(volumeTypeOf(vol)) != dir {
			continue
		}
		allocated, err := dv.storage.AllocatedBytes(dv.backingFile(vol))
		if err != nil {
			return 0, errors.New("check allocated size: " + err.Error())
		}
		if rest := vol.VolSize - allocated; rest > 0 {
			unallocated += rest
		}
	}

	return free - unallocated, nil
}
