//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"errors"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strconv"
	"sync"
	"time"

	"kata-containers/csi-kata-directvolume/pkg/spdkrpc"
	"kata-containers/csi-kata-directvolume/pkg/state"
	"kata-containers/csi-kata-directvolume/pkg/utils"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"k8s.io/klog/v2"
	utilexec "k8s.io/utils/exec"
)

type directVolume struct {
	mutex sync.Mutex

	config  Config
	state   state.State
	storage backingStorage
}

type Config struct {
	DriverName    string
	Endpoint      string
	NodeID        string
	VendorVersion string
	MaxVolumeSize int64
	// Capacity holds the capacity pools of this node.
	Capacity utils.Capacity
	// CapacityConfigDir, if set, is a directory which may hold a file named
	// after the node with additional capacity pools for this node.
	CapacityConfigDir string
	ShowVersion       bool
	EnableAttach      bool
	EnableTopology    bool

	StateDir       string
	VolumeDevices  map[string]string
	StoragePath    string
	IsDirectVolume bool
	safeMounter    *utils.SafeMountFormater

	SpdkRPCTimeout time.Duration
	SpdkRawPath    string
	SpdkVhostPath  string
}

func NewDirectVolumeDriver(cfg Config) (*directVolume, error) {
	if cfg.DriverName == "" {
		return nil, errors.New("no driver name provided")
	}

	if cfg.NodeID == "" {
		return nil, errors.New("no node id provided")
	}

	if cfg.Endpoint == "" {
		return nil, errors.New("no driver endpoint provided")
	}

	if cfg.StoragePath == "" {
		return nil, errors.New("no storage path provided")
	}

	if cfg.MaxVolumeSize < 0 {
		return nil, fmt.Errorf("maximum volume size must not be negative, got %d", cfg.MaxVolumeSize)
	}

	if cfg.CapacityConfigDir != "" {
		nodeCapacity, found, err := utils.LoadNodeCapacity(cfg.CapacityConfigDir, cfg.NodeID)
		if err != nil {
			return nil, err
		}
		if found {
			klog.Infof("loaded capacity pools %s for node %q from %s", nodeCapacity.String(), cfg.NodeID, cfg.CapacityConfigDir)
			cfg.Capacity = cfg.Capacity.Merge(nodeCapacity)
		} else {
			klog.Infof("%s has no capacity file for node %q", cfg.CapacityConfigDir, cfg.NodeID)
		}
	}

	if err := utils.MakeFullPath(cfg.StoragePath); err != nil {
		return nil, fmt.Errorf("failed to mkdir -p storage path %v", cfg.StoragePath)
	}

	if err := utils.MakeFullPath(cfg.StateDir); err != nil {
		return nil, fmt.Errorf("failed to mkdir -p state dir%v", cfg.StateDir)
	}

	if err := utils.MakeFullPath(cfg.SpdkRawPath); err != nil {
		return nil, fmt.Errorf("failed to mkdir -p spdk raw path %v", cfg.SpdkRawPath)
	}

	if err := utils.MakeFullPath(cfg.SpdkVhostPath); err != nil {
		return nil, fmt.Errorf("failed to mkdir -p spdk vhost path %v", cfg.SpdkVhostPath)
	}

	utils.SpdkRawDiskDir = cfg.SpdkRawPath
	utils.SpdkVhostDir = cfg.SpdkVhostPath

	if cfg.safeMounter == nil {
		safeMnt := utils.NewSafeMountFormater()
		cfg.safeMounter = &safeMnt
	}

	cfg.VolumeDevices = make(map[string]string)

	klog.Infof("\nDriver: %v \nVersion: %s\nStoragePath: %s\nStatePath: %s\n", cfg.DriverName, cfg.VendorVersion, cfg.StoragePath, cfg.StateDir)

	s, err := state.New(path.Join(cfg.StateDir, "state.json"))
	if err != nil {
		return nil, err
	}
	dv := &directVolume{
		config:  cfg,
		state:   s,
		storage: hostStorage{},
	}
	dv.logCapacityState()

	return dv, nil
}

func (dv *directVolume) Run() error {
	s := NewNonBlockingGRPCServer()

	// dv itself implements ControllerServer, NodeServer, and IdentityServer.
	s.Start(dv.config.Endpoint, dv, dv, dv)
	s.Wait()

	return nil
}

// getVolumePath returns the canonical path for direct volume
func (dv *directVolume) getVolumePath(volID string) string {
	return filepath.Join(dv.config.StateDir, volID)
}

// createVolume reserves capacity for a new directvol volume, creates the
// directory for the direct volume, and adds the volume to the list. The
// backing file itself is only created when the volume is staged.
// It returns the volume or err if one occurs. That error is suitable as result of a gRPC call.
func (dv *directVolume) createVolume(volID, name string, size int64, params volumeParameters, volumeCtx map[string]string) (*state.Volume, error) {
	if err := dv.admitVolume(size, params); err != nil {
		return nil, err
	}

	path := dv.getVolumePath(volID)

	if err := os.MkdirAll(path, utils.PERM); err != nil {
		klog.Errorf("mkdirAll for path %s failed with error: %v", path, err.Error())
		return nil, status.Errorf(codes.Internal, "create volume directory %s: %v", path, err)
	}

	volume := state.Volume{
		VolID:         volID,
		VolName:       name,
		VolSize:       size,
		VolPath:       path,
		Kind:          params.pool,
		VolumeContext: volumeCtx,
	}

	klog.Infof("adding direct volume: %s = %+v", volID, volume)
	if err := dv.state.UpdateVolume(volume); err != nil {
		if rmErr := os.RemoveAll(path); rmErr != nil {
			klog.Warningf("failed to remove %s after a failed state update: %v", path, rmErr)
		}
		return nil, err
	}

	return &volume, nil
}

// adoptVolume records a volume which the node service is asked to use but
// which is missing from the state, for example because the state file was
// lost. The volume has already been provisioned, so its reservation is
// recorded without enforcing the capacity limits; GetCapacity then stops
// advertising the space it takes.
func (dv *directVolume) adoptVolume(volID string, volumeCtx map[string]string) (state.Volume, error) {
	size, err := strconv.ParseInt(volumeCtx[utils.CapabilityInBytes], 10, 64)
	if err != nil || size < 0 {
		return state.Volume{}, status.Errorf(codes.InvalidArgument, "volume %s has an invalid %s %q in its volume context",
			volID, utils.CapabilityInBytes, volumeCtx[utils.CapabilityInBytes])
	}

	pool := volumeCtx[utils.KataContainersDirectCapacityPool]
	if pool != "" {
		if err := utils.ValidateCapacityPoolName(pool); err != nil {
			return state.Volume{}, status.Errorf(codes.InvalidArgument, "volume %s: %v", volID, err)
		}
		remaining, err := dv.poolRemaining(volumeParameters{pool: pool})
		switch {
		case err != nil:
			klog.Warningf("adopting volume %s of %d bytes: %v", volID, size, err)
		case remaining < size:
			klog.Warningf("adopted volume %s of %d bytes overcommits capacity pool %q on node %q", volID, size, pool, dv.config.NodeID)
		}
	}

	path := dv.getVolumePath(volID)
	if err := os.MkdirAll(path, utils.PERM); err != nil {
		return state.Volume{}, status.Errorf(codes.Internal, "create volume directory %s: %v", path, err)
	}

	recordedCtx := make(map[string]string, len(volumeCtx))
	for key, value := range volumeCtx {
		recordedCtx[key] = value
	}
	volume := state.Volume{
		VolID:         volID,
		VolName:       volumeCtx[utils.DirectVolumeName],
		VolSize:       size,
		VolPath:       path,
		Kind:          pool,
		VolumeContext: recordedCtx,
	}
	klog.Infof("adopting direct volume which is missing from the state: %s = %+v", volID, volume)
	if err := dv.state.UpdateVolume(volume); err != nil {
		return state.Volume{}, err
	}

	return volume, nil
}

// deleteVolume deletes the directory for the direct volume.
func (dv *directVolume) deleteVolume(volID string) error {
	klog.V(4).Infof("starting to delete direct volume: %s", volID)

	vol, err := dv.state.GetVolumeByID(volID)
	if err != nil {
		klog.Warning("deleteVolume with Volume not found.")
		// Return OK if the volume is not found.
		return nil
	}

	path := dv.getVolumePath(volID)
	if err := os.RemoveAll(path); err != nil && !os.IsNotExist(err) {
		return err
	}
	if err := dv.state.DeleteVolume(volID); err != nil {
		return err
	}
	klog.V(4).Infof("deleted direct volume: %s = %+v", volID, vol)

	return nil
}

// createSPDKVolume reserves capacity for a new spdkvol volume, allocates its
// sparse backing file, creates the SPDK bdev, and adds the volume to the list.
// The capacity is checked before any backend resource is created, and the
// resources created here are released again if a later step fails.
// It returns the created volume or an error if one occurs.
func (dv *directVolume) createSPDKVolume(volumeID, volName string, capacity int64, params volumeParameters, volumeCtx map[string]string) (*state.Volume, error) {
	if err := dv.admitVolume(capacity, params); err != nil {
		return nil, err
	}

	backingFile := filepath.Join(utils.SpdkRawDiskDir, fmt.Sprintf("%s.raw", volName))

	if err := os.MkdirAll(utils.SpdkRawDiskDir, 0750); err != nil {
		return nil, status.Errorf(codes.Internal, "failed to create dir %s: %v", utils.SpdkRawDiskDir, err)
	}

	createdFile := false
	fi, err := os.Stat(backingFile)
	if err == nil {
		// Left behind by an earlier attempt which did not finish.
		if fi.Size() != capacity {
			return nil, status.Errorf(codes.Internal, "backing file %s already exists with %d bytes instead of %d",
				backingFile, fi.Size(), capacity)
		}
		klog.Infof("[SPDK] reuse existing backing file %s (size=%d)", backingFile, fi.Size())
	} else if os.IsNotExist(err) {
		if err := createSparseFile(backingFile, capacity); err != nil {
			return nil, status.Errorf(codes.Internal, "failed to create backing file %s: %v", backingFile, err)
		}
		createdFile = true
		klog.Infof("[SPDK] created new sparse backing file %s (%d bytes)", backingFile, capacity)
	} else {
		return nil, status.Errorf(codes.Internal, "stat backing file %s failed: %v", backingFile, err)
	}
	removeCreatedFile := func() {
		if !createdFile {
			return
		}
		if err := os.Remove(backingFile); err != nil && !os.IsNotExist(err) {
			klog.Warningf("[SPDK] failed to remove backing file %s: %v", backingFile, err)
		}
	}

	bdevName := fmt.Sprintf("bdev-%s", volName)

	bdevParams := map[string]interface{}{
		"name":       bdevName,
		"filename":   backingFile,
		"block_size": 512,
	}
	if _, err := spdkrpc.Call("bdev_aio_create", bdevParams); err != nil {
		removeCreatedFile()
		return nil, status.Errorf(codes.Internal, "bdev_aio_create failed: %v", err)
	}

	vol := &state.Volume{
		VolID:         volumeID,
		VolName:       volName,
		VolSize:       capacity,
		Kind:          params.pool,
		VolumeContext: volumeCtx,
		Metadata: map[string]string{
			"type":        utils.SpdkVolumeTypeName,
			"bdevName":    bdevName,
			"backingFile": backingFile,
		},
	}
	if err := dv.state.UpdateVolume(*vol); err != nil {
		if _, delErr := spdkrpc.Call("bdev_aio_delete", map[string]any{"name": bdevName}); delErr != nil {
			klog.Warningf("[SPDK] failed to delete bdev %s after a failed state update: %v", bdevName, delErr)
		}
		removeCreatedFile()
		return nil, err
	}

	klog.Infof("[SPDK] volume %s registered bdev=%s backing=%s", volumeID, bdevName, backingFile)
	return vol, nil
}

// createSparseFile creates a sparse file of the given size.
func createSparseFile(path string, size int64) (err error) {
	f, err := os.OpenFile(path, os.O_RDWR|os.O_CREATE|os.O_EXCL, 0666)
	if err != nil {
		return err
	}
	defer func() {
		if closeErr := f.Close(); err == nil {
			err = closeErr
		}
		if err != nil {
			_ = os.Remove(path)
		}
	}()

	return f.Truncate(size)
}

// loadFromVolume populates the given destPath with data from the srcVolumeID
func (dv *directVolume) loadFromVolume(size int64, srcVolumeId, destPath string) error {
	directVolume, err := dv.state.GetVolumeByID(srcVolumeId)
	if err != nil {
		klog.Error("loadFromVolume failed with get volume by ID error Volume not found")
		return err
	}
	if directVolume.VolSize > size {
		return status.Errorf(codes.InvalidArgument, "volume %v size %v is greater than requested volume size %v", srcVolumeId, directVolume.VolSize, size)
	}

	return loadFromPersitStorage(directVolume, destPath)
}

func loadFromPersitStorage(directVolume state.Volume, destPath string) error {
	srcPath := directVolume.VolPath
	isEmpty, err := utils.IsPathEmpty(srcPath)
	if err != nil {
		return fmt.Errorf("failed verification check of source direct volume %v: %w", directVolume.VolID, err)
	}

	// If the source direct volume is empty it's a noop and we just move along, otherwise the cp call will
	// fail with a a file stat error DNE
	if !isEmpty {
		args := []string{"-a", srcPath + "/.", destPath + "/"}
		executor := utilexec.New()
		out, err := executor.Command("cp", args...).CombinedOutput()
		if err != nil {
			return fmt.Errorf("failed pre-populate data from volume %v: %s: %w", directVolume.VolID, out, err)
		}
	}
	return nil
}
