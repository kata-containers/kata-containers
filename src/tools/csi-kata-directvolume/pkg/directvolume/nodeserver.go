//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"

	"kata-containers/csi-kata-directvolume/pkg/utils"

	"github.com/container-storage-interface/spec/lib/go/csi"
	"golang.org/x/net/context"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"k8s.io/klog/v2"
)

const (
	TopologyKeyNode = "topology.directvolume.csi/node"
)

func (dv *directVolume) NodePublishVolume(ctx context.Context, req *csi.NodePublishVolumeRequest) (*csi.NodePublishVolumeResponse, error) {
	klog.V(4).Infof("node publish volume with request %v", req)

	// Check arguments
	if req.GetVolumeCapability() == nil {
		return nil, status.Error(codes.InvalidArgument, "Volume capability missing in request")
	}
	if len(req.GetVolumeId()) == 0 {
		return nil, status.Error(codes.InvalidArgument, "Volume ID missing in request")
	}
	if len(req.GetTargetPath()) == 0 {
		return nil, status.Error(codes.InvalidArgument, "Target path missing in request")
	}
	if !isDirectVolume(req.VolumeContext) {
		return nil, status.Errorf(codes.FailedPrecondition, "volume %q is not direct-volume.", req.VolumeId)
	}

	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	targetPath := req.GetTargetPath()
	if req.GetVolumeCapability().GetMount() == nil {
		return nil, status.Error(codes.InvalidArgument, "It Must be mount access type")
	}

	fsType := req.VolumeContext[utils.KataContainersDirectFsType]
	if len(fsType) == 0 {
		fsType = utils.DefaultFsType
		klog.Warningf("volume context has no fsType, set default fstype %v\n", fsType)
	}

	volType := req.VolumeContext[utils.KataContainersDirectVolumeType]
	if len(volType) == 0 {
		volType = "directvol"
		klog.Warningf("volume context has no volumeType, set default volume type %v\n", volType)
	}

	readOnly := req.GetReadonly()
	volumeID := req.GetVolumeId()
	attrib := req.GetVolumeContext()

	devicePath := dv.config.VolumeDevices[volumeID]
	klog.Infof("target %v\nfstype %v\ndevice %v\nreadonly %v\nvolumeID %v\n",
		targetPath, fsType, devicePath, readOnly, volumeID)

	options := []string{"bind"}
	if readOnly {
		options = append(options, "ro")
	} else {
		options = append(options, "rw")
	}

	stagingTargetPath := req.GetStagingTargetPath()

	if canDoMnt, err := utils.CanDoBindmount(dv.config.safeMounter, targetPath); err != nil {
		return nil, err
	} else if !canDoMnt {
		klog.V(3).Infof("cannot do bindmount target path: %s", targetPath)
		return &csi.NodePublishVolumeResponse{}, nil
	}

	if err := dv.config.safeMounter.DoBindmount(stagingTargetPath, targetPath, "", options); err != nil {
		errMsg := fmt.Sprintf("failed to bindmount device: %s at %s: %s", stagingTargetPath, targetPath, err.Error())
		klog.Infof("do bindmount failed: %v.", errMsg)
		return nil, status.Error(codes.Aborted, errMsg)
	}

	// kata-containers DirectVolume add
	mountInfo := utils.MountInfo{
		VolumeType: volType,
		Device:     devicePath,
		FsType:     fsType,
		Metadata:   attrib,
		Options:    options,
	}
	if err := utils.AddDirectVolume(targetPath, mountInfo); err != nil {
		klog.Errorf("add direct volume with source %s and mountInfo %v failed", targetPath, mountInfo)
		return nil, err
	}
	klog.Infof("add direct volume successfully.")

	volInStat, err := dv.state.GetVolumeByID(volumeID)
	if err != nil {
		capInt64, _ := strconv.ParseInt(req.VolumeContext[utils.CapabilityInBytes], 10, 64)
		volName := req.VolumeContext[utils.DirectVolumeName]
		kind := req.VolumeContext[storageKind]
		vol, err := dv.createVolume(volumeID, volName, capInt64, kind)
		if err != nil {
			return nil, err
		}
		vol.NodeID = dv.config.NodeID
		vol.Published.Add(targetPath)
		klog.Infof("create volume %v successfully", vol)

		return &csi.NodePublishVolumeResponse{}, nil
	}

	volInStat.NodeID = dv.config.NodeID
	volInStat.Published.Add(targetPath)
	if err := dv.state.UpdateVolume(volInStat); err != nil {
		return nil, err
	}

	klog.Infof("directvolume: volume %s has been published.", targetPath)

	return &csi.NodePublishVolumeResponse{}, nil
}

func (dv *directVolume) NodeUnpublishVolume(ctx context.Context, req *csi.NodeUnpublishVolumeRequest) (*csi.NodeUnpublishVolumeResponse, error) {

	// Check arguments
	if len(req.GetVolumeId()) == 0 {
		return nil, status.Error(codes.InvalidArgument, "Volume ID missing in request")
	}
	if len(req.GetTargetPath()) == 0 {
		return nil, status.Error(codes.InvalidArgument, "Target path missing in request")
	}

	targetPath := req.GetTargetPath()
	volumeID := req.GetVolumeId()

	// Lock before acting on global state. A production-quality
	// driver might use more fine-grained locking.
	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	// Unmount only if the target path is really a mount point.
	if isMnt, err := dv.config.safeMounter.IsMountPoint(targetPath); err != nil {
		return nil, status.Error(codes.Internal, fmt.Sprintf("check target path: %v", err))
	} else if isMnt {
		// Unmounting the image or filesystem.
		err = dv.config.safeMounter.Unmount(targetPath)
		if err != nil {
			return nil, status.Error(codes.Internal, fmt.Sprintf("unmount target path: %v", err))
		}
	}

	// Delete the mount point.
	// Does not return error for non-existent path, repeated calls OK for idempotency.
	if err := os.RemoveAll(targetPath); err != nil {
		return nil, status.Error(codes.Internal, fmt.Sprintf("remove target path: %v", err))
	}

	if err := utils.RemoveDirectVolume(targetPath); err != nil {
		klog.V(4).Infof("remove direct volume failed.")
		return nil, status.Error(codes.Internal, fmt.Sprintf("remove direct volume failed: %v", err))
	}

	klog.Infof("direct volume %s has been cleaned up.", targetPath)

	vol, err := dv.state.GetVolumeByID(volumeID)
	if err != nil {
		klog.Warningf("volume id %s not found in volume list, nothing to do.", volumeID)
		return &csi.NodeUnpublishVolumeResponse{}, nil
	}

	if !vol.Published.Has(targetPath) {
		klog.V(4).Infof("volume %q is not published at %q, nothing to do.", volumeID, targetPath)
		return &csi.NodeUnpublishVolumeResponse{}, nil
	}

	vol.Published.Remove(targetPath)
	if err := dv.state.UpdateVolume(vol); err != nil {
		return nil, err
	}
	klog.Infof("volume %s has been unpublished.", targetPath)

	return &csi.NodeUnpublishVolumeResponse{}, nil
}

func isDirectVolume(VolumeCtx map[string]string) bool {
	return VolumeCtx[utils.IsDirectVolume] == "True"
}

func (dv *directVolume) NodeStageVolume(ctx context.Context, req *csi.NodeStageVolumeRequest) (*csi.NodeStageVolumeResponse, error) {
	klog.V(4).Infof("NodeStageVolumeRequest with request %v", req)

	volumeID := req.GetVolumeId()
	// Check arguments
	if len(volumeID) == 0 {
		return nil, status.Error(codes.InvalidArgument, "Volume ID missing in request")
	}
	stagingTargetPath := req.GetStagingTargetPath()
	if stagingTargetPath == "" {
		return nil, status.Error(codes.InvalidArgument, "Target path missing in request")
	}
	if req.GetVolumeCapability() == nil {
		return nil, status.Error(codes.InvalidArgument, "Volume Capability missing in request")
	}

	if !isDirectVolume(req.VolumeContext) {
		return nil, status.Errorf(codes.FailedPrecondition, "volume %q is not direct-volume.", req.VolumeId)
	}

	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	capacityInBytes := req.VolumeContext[utils.CapabilityInBytes]
	devicePath, err := utils.CreateDirectBlockDevice(volumeID, capacityInBytes, dv.config.StoragePath)
	if err != nil {
		errMsg := status.Errorf(codes.Internal, "setup storage for volume '%s' failed", volumeID)
		return &csi.NodeStageVolumeResponse{}, errMsg
	}

	// /full_path_on_host/VolumeId/
	deviceUpperPath := filepath.Dir(*devicePath)
	if canMnt, err := utils.CanDoBindmount(dv.config.safeMounter, stagingTargetPath); err != nil {
		return nil, err
	} else if !canMnt {
		klog.Infof("staging target path: %s already mounted", stagingTargetPath)
		return &csi.NodeStageVolumeResponse{}, nil
	}

	options := []string{"bind"}
	if err := dv.config.safeMounter.DoBindmount(deviceUpperPath, stagingTargetPath, "", options); err != nil {
		klog.Errorf("safe mounter: %v do bind mount %v failed, with error: %v", deviceUpperPath, stagingTargetPath, err.Error())
		return nil, err
	}

	fsType, ok := req.VolumeContext[utils.KataContainersDirectFsType]
	if !ok {
		klog.Infof("fstype not set, default fstype will be set: %v\n", utils.DefaultFsType)
		fsType = utils.DefaultFsType
	}

	if err := dv.config.safeMounter.SafeFormatWithFstype(*devicePath, fsType, options); err != nil {
		return nil, err
	}

	dv.config.VolumeDevices[volumeID] = *devicePath

	klog.Infof("directvolume: volume %s has been staged.", stagingTargetPath)

	volInStat, err := dv.state.GetVolumeByID(req.VolumeId)
	if err != nil {
		capInt64, _ := strconv.ParseInt(req.VolumeContext[utils.CapabilityInBytes], 10, 64)
		volName := req.VolumeContext[utils.DirectVolumeName]
		kind := req.VolumeContext[storageKind]
		vol, err := dv.createVolume(volumeID, volName, capInt64, kind)
		if err != nil {
			return nil, err
		}
		vol.Staged.Add(stagingTargetPath)

		klog.Infof("create volume %v successfully", vol)
		return &csi.NodeStageVolumeResponse{}, nil
	}

	if volInStat.Staged.Has(stagingTargetPath) {
		klog.V(4).Infof("Volume %q is already staged at %q, nothing to do.", req.VolumeId, stagingTargetPath)
		return &csi.NodeStageVolumeResponse{}, nil
	}

	if !volInStat.Staged.Empty() {
		return nil, status.Errorf(codes.FailedPrecondition, "volume %q is already staged at %v", req.VolumeId, volInStat.Staged)
	}

	volInStat.Staged.Add(stagingTargetPath)
	if err := dv.state.UpdateVolume(volInStat); err != nil {
		return nil, err
	}

	return &csi.NodeStageVolumeResponse{}, nil
}

func (dv *directVolume) NodeUnstageVolume(ctx context.Context, req *csi.NodeUnstageVolumeRequest) (*csi.NodeUnstageVolumeResponse, error) {
	// Check arguments
	volumeID := req.GetVolumeId()
	if len(volumeID) == 0 {
		return nil, status.Error(codes.InvalidArgument, "Volume ID missing in request")
	}
	stagingTargetPath := req.GetStagingTargetPath()
	if stagingTargetPath == "" {
		return nil, status.Error(codes.InvalidArgument, "Target path missing in request")
	}

	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	// Unmount only if the target path is really a mount point.
	if isMnt, err := dv.config.safeMounter.IsMountPoint(stagingTargetPath); err != nil {
		return nil, status.Error(codes.Internal, fmt.Sprintf("check staging target path: %v", err))
	} else if isMnt {
		err = dv.config.safeMounter.Unmount(stagingTargetPath)
		if err != nil {
			return nil, status.Error(codes.Internal, fmt.Sprintf("unmount staging target path: %v", err))
		}
	}

	if deviceUpperPath, err := utils.GetStoragePath(dv.config.StoragePath, volumeID); err != nil {
		return nil, status.Error(codes.Internal, fmt.Sprintf("get device UpperPath %s failed: %v", deviceUpperPath, err))
	} else {
		if err = os.RemoveAll(deviceUpperPath); err != nil {
			return nil, status.Error(codes.Internal, fmt.Sprintf("remove device upper path: %s failed %v", deviceUpperPath, err.Error()))
		}
		klog.Infof("direct volume %s has been removed.", deviceUpperPath)
	}

	if err := os.RemoveAll(stagingTargetPath); err != nil {
		return nil, status.Error(codes.Internal, fmt.Sprintf("remove staging target path: %v", err))
	}

	klog.Infof("directvolume: volume %s has been unstaged.", stagingTargetPath)
	vol, err := dv.state.GetVolumeByID(volumeID)
	if err != nil {
		klog.Warning("Volume not found: might have already deleted")
		return &csi.NodeUnstageVolumeResponse{}, nil
	}

	if !vol.Staged.Has(stagingTargetPath) {
		klog.V(4).Infof("Volume %q is not staged at %q, nothing to do.", volumeID, stagingTargetPath)
		return &csi.NodeUnstageVolumeResponse{}, nil
	}

	if !vol.Published.Empty() {
		return nil, status.Errorf(codes.Internal, "volume %q is still published at %q on node %q", vol.VolID, vol.Published, vol.NodeID)
	}

	vol.Staged.Remove(stagingTargetPath)
	if err := dv.state.UpdateVolume(vol); err != nil {
		return nil, err
	}

	return &csi.NodeUnstageVolumeResponse{}, nil
}

func (dv *directVolume) NodeGetInfo(ctx context.Context, req *csi.NodeGetInfoRequest) (*csi.NodeGetInfoResponse, error) {
	resp := &csi.NodeGetInfoResponse{
		NodeId: dv.config.NodeID,
	}

	if dv.config.EnableTopology {
		resp.AccessibleTopology = &csi.Topology{
			Segments: map[string]string{TopologyKeyNode: dv.config.NodeID},
		}
	}

	return resp, nil
}

func (dv *directVolume) NodeGetCapabilities(ctx context.Context, req *csi.NodeGetCapabilitiesRequest) (*csi.NodeGetCapabilitiesResponse, error) {
	caps := []*csi.NodeServiceCapability{
		{
			Type: &csi.NodeServiceCapability_Rpc{
				Rpc: &csi.NodeServiceCapability_RPC{
					Type: csi.NodeServiceCapability_RPC_STAGE_UNSTAGE_VOLUME,
				},
			},
		},
	}

	return &csi.NodeGetCapabilitiesResponse{Capabilities: caps}, nil
}

func (dv *directVolume) NodeGetVolumeStats(ctx context.Context, in *csi.NodeGetVolumeStatsRequest) (*csi.NodeGetVolumeStatsResponse, error) {
	return &csi.NodeGetVolumeStatsResponse{}, nil
}

func (dv *directVolume) NodeExpandVolume(ctx context.Context, req *csi.NodeExpandVolumeRequest) (*csi.NodeExpandVolumeResponse, error) {

	return &csi.NodeExpandVolumeResponse{}, nil
}
