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
	"sync"
	"time"

	"kata-containers/csi-kata-directvolume/pkg/spdkrpc"
	"kata-containers/csi-kata-directvolume/pkg/state"
	"kata-containers/csi-kata-directvolume/pkg/utils"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"k8s.io/apimachinery/pkg/api/resource"
	"k8s.io/klog/v2"
	utilexec "k8s.io/utils/exec"
)

const (
	// storageKind is the special parameter which requests
	// storage of a certain kind (only affects capacity checks).
	storageKind = "kind"
)

type directVolume struct {
	mutex sync.Mutex

	config Config
	state  state.State
}

type Config struct {
	DriverName     string
	Endpoint       string
	NodeID         string
	VendorVersion  string
	MaxVolumeSize  int64
	Capacity       utils.Capacity
	ShowVersion    bool
	EnableAttach   bool
	EnableTopology bool

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
		config: cfg,
		state:  s,
	}

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

// createVolume allocates capacity, creates the directory for the direct volume, and
// adds the volume to the list.
// It returns the volume path or err if one occurs. That error is suitable as result of a gRPC call.
func (dv *directVolume) createVolume(volID, name string, cap int64, kind string) (*state.Volume, error) {
	// Check for maximum available capacity
	if cap > dv.config.MaxVolumeSize {
		return nil, status.Errorf(codes.OutOfRange, "Requested capacity %d exceeds maximum allowed %d", cap, dv.config.MaxVolumeSize)
	}
	if dv.config.Capacity.Enabled() {
		if kind == "" {
			// Pick some kind with sufficient remaining capacity.
			for k, c := range dv.config.Capacity {
				if dv.sumVolumeSizes(k)+cap <= c.Value() {
					kind = k
					break
				}
			}
		}

		used := dv.sumVolumeSizes(kind)
		available := dv.config.Capacity[kind]
		if used+cap > available.Value() {
			return nil, status.Errorf(codes.ResourceExhausted, "requested capacity %d exceeds remaining capacity for %q, %s out of %s already used",
				cap, kind, resource.NewQuantity(used, resource.BinarySI).String(), available.String())
		}
	} else if kind != "" {
		return nil, status.Error(codes.InvalidArgument, fmt.Sprintf("capacity tracking disabled, specifying kind %q is invalid", kind))
	}

	path := dv.getVolumePath(volID)

	if err := os.MkdirAll(path, utils.PERM); err != nil {
		klog.Errorf("mkdirAll for path %s failed with error: %v", path, err.Error())
		return nil, err
	}

	volume := state.Volume{
		VolID:   volID,
		VolName: name,
		VolSize: cap,
		VolPath: path,
		Kind:    kind,
	}

	klog.Infof("adding direct volume: %s = %+v", volID, volume)
	if err := dv.state.UpdateVolume(volume); err != nil {
		return nil, err
	}

	return &volume, nil
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

// createSPDKVolume allocates backing file, creates the SPDK bdev, and adds the volume to the list.
// It returns the created volume or an error if one occurs.
func (dv *directVolume) createSPDKVolume(volumeID, volName string, capacity int64, kind string) (*state.Volume, error) {
	backingFile := filepath.Join(utils.SpdkRawDiskDir, fmt.Sprintf("%s.raw", volName))

	if err := os.MkdirAll(utils.SpdkRawDiskDir, 0750); err != nil {
		return nil, status.Errorf(codes.Internal, "failed to create dir %s: %v", utils.SpdkRawDiskDir, err)
	}

	fi, err := os.Stat(backingFile)
	if err == nil {
		klog.Infof("[SPDK] reuse existing backing file %s (size=%d)", backingFile, fi.Size())
	} else if os.IsNotExist(err) {
		f, err := os.Create(backingFile)
		if err != nil {
			return nil, status.Errorf(codes.Internal, "failed to create backing file %s: %v", backingFile, err)
		}
		defer f.Close()

		if err := f.Truncate(capacity); err != nil {
			return nil, status.Errorf(codes.Internal, "failed to truncate backing file %s: %v", backingFile, err)
		}

		klog.Infof("[SPDK] created new sparse backing file %s (%d bytes)", backingFile, capacity)
	} else {
		return nil, status.Errorf(codes.Internal, "stat backing file %s failed: %v", backingFile, err)
	}

	bdevName := fmt.Sprintf("bdev-%s", volName)

	params := map[string]interface{}{
		"name":       bdevName,
		"filename":   backingFile,
		"block_size": 512,
	}
	if _, err := spdkrpc.Call("bdev_aio_create", params); err != nil {
		return nil, status.Errorf(codes.Internal, "bdev_aio_create failed: %v", err)
	}

	vol := &state.Volume{
		VolID:   volumeID,
		VolName: volName,
		VolSize: capacity,
		Kind:    kind,
		Metadata: map[string]string{
			"type":        utils.SpdkVolumeTypeName,
			"bdevName":    bdevName,
			"backingFile": backingFile,
		},
	}
	if err := dv.state.UpdateVolume(*vol); err != nil {
		return nil, err
	}

	klog.Infof("[SPDK] volume %s registered bdev=%s backing=%s", volumeID, bdevName, backingFile)
	return vol, nil
}

func (dv *directVolume) sumVolumeSizes(kind string) (sum int64) {
	for _, volume := range dv.state.GetVolumes() {
		if volume.Kind == kind {
			sum += volume.VolSize
		}
	}
	return
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
