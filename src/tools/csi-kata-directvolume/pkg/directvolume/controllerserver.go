//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"fmt"
	"strings"

	"github.com/golang/protobuf/ptypes/wrappers"
	"github.com/pborman/uuid"
	"golang.org/x/net/context"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"

	"github.com/container-storage-interface/spec/lib/go/csi"
	"k8s.io/klog/v2"

	"kata-containers/csi-kata-directvolume/pkg/utils"
)

func (dv *directVolume) CreateVolume(ctx context.Context, req *csi.CreateVolumeRequest) (resp *csi.CreateVolumeResponse, finalErr error) {
	if err := dv.validateControllerServiceRequest(csi.ControllerServiceCapability_RPC_CREATE_DELETE_VOLUME); err != nil {
		klog.V(3).Infof("invalid create volume req: %v", req)
		return nil, err
	}

	if len(req.GetName()) == 0 {
		return nil, status.Error(codes.InvalidArgument, "Name missing in request")
	}
	caps := req.GetVolumeCapabilities()
	if caps == nil {
		return nil, status.Error(codes.InvalidArgument, "Volume Capabilities missing in request")
	}
	klog.Infof("createVolume with request: %+v", req)

	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	capacity := int64(req.GetCapacityRange().GetRequiredBytes())
	topologies := []*csi.Topology{}
	if dv.config.EnableTopology {
		topologies = append(topologies, &csi.Topology{Segments: map[string]string{TopologyKeyNode: dv.config.NodeID}})
	}

	volumeCtx := make(map[string]string)
	volumeCtx[utils.IsDirectVolume] = "False"

	for key, value := range req.GetParameters() {
		switch strings.ToLower(key) {
		case utils.KataContainersDirectVolumeType:
			if value == utils.DirectVolumeTypeName {
				volumeCtx[utils.IsDirectVolume] = "True"
			}
		case utils.KataContainersDirectFsType:
			volumeCtx[utils.KataContainersDirectFsType] = value
		default:
			continue
		}
	}

	contentSrc := req.GetVolumeContentSource()

	// Need to check for already existing volume name, and if found
	// check for the requested capacity and already allocated capacity
	// If err is nil, it means the volume with the same name already exists
	// need to check if the size of existing volume is the same as in new
	// request
	if exVol, err := dv.state.GetVolumeByName(req.GetName()); err == nil {
		if exVol.VolSize < capacity {
			return nil, status.Errorf(codes.AlreadyExists, "Volume with the same name: %s but with different size already exist", req.GetName())
		}

		if contentSrc != nil {
			volumeSource := req.VolumeContentSource
			switch volumeSource.Type.(type) {
			case *csi.VolumeContentSource_Volume:
				if volumeSource.GetVolume() != nil && exVol.ParentVolID != volumeSource.GetVolume().GetVolumeId() {
					return nil, status.Error(codes.AlreadyExists, "existing volume source volume id not matching")
				}
			default:
				return nil, status.Errorf(codes.InvalidArgument, "%v not a proper volume source", volumeSource)
			}
		}

		return &csi.CreateVolumeResponse{
			Volume: &csi.Volume{
				VolumeId:           exVol.VolID,
				CapacityBytes:      int64(exVol.VolSize),
				VolumeContext:      volumeCtx,
				ContentSource:      contentSrc,
				AccessibleTopology: topologies,
			},
		}, nil
	}

	volumeID := uuid.NewUUID().String()
	kind := volumeCtx[storageKind]

	vol, err := dv.createVolume(volumeID, req.GetName(), capacity, kind)
	if err != nil {
		klog.Errorf("created volume %s at path %s failed with error: %v", vol.VolID, vol.VolPath, err.Error())
		return nil, err
	}
	klog.Infof("created volume %s at path %s", vol.VolID, vol.VolPath)

	if contentSrc != nil {
		path := dv.getVolumePath(volumeID)
		volumeSource := req.VolumeContentSource
		switch volumeSource.Type.(type) {
		case *csi.VolumeContentSource_Volume:
			if srcVolume := volumeSource.GetVolume(); srcVolume != nil {
				err = dv.loadFromVolume(capacity, srcVolume.GetVolumeId(), path)
				vol.ParentVolID = srcVolume.GetVolumeId()
			}
		default:
			err = status.Errorf(codes.InvalidArgument, "%v not a proper volume source", volumeSource)
		}

		if err != nil {
			klog.V(4).Infof("VolumeSource error: %v", err)
			if delErr := dv.deleteVolume(volumeID); delErr != nil {
				klog.Infof("deleting direct volume %v failed: %v", volumeID, delErr)
			}
			return nil, err
		}
		klog.Infof("successfully populated volume %s", vol.VolID)
	}

	volumeCtx[utils.DirectVolumeName] = req.GetName()
	volumeCtx[utils.CapabilityInBytes] = fmt.Sprintf("%d", capacity)

	return &csi.CreateVolumeResponse{
		Volume: &csi.Volume{
			VolumeId:           volumeID,
			CapacityBytes:      capacity,
			VolumeContext:      volumeCtx,
			ContentSource:      contentSrc,
			AccessibleTopology: topologies,
		},
	}, nil
}

func (dv *directVolume) DeleteVolume(ctx context.Context, req *csi.DeleteVolumeRequest) (*csi.DeleteVolumeResponse, error) {
	if err := dv.validateControllerServiceRequest(csi.ControllerServiceCapability_RPC_CREATE_DELETE_VOLUME); err != nil {
		klog.V(3).Infof("invalid delete volume req: %v", req)
		return nil, err
	}

	if len(req.GetVolumeId()) == 0 {
		return nil, status.Error(codes.InvalidArgument, "Volume ID missing in request")
	}

	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	volId := req.GetVolumeId()
	vol, err := dv.state.GetVolumeByID(volId)
	if err != nil {
		klog.Warningf("Volume ID %s not found: might have already deleted", volId)
		return &csi.DeleteVolumeResponse{}, nil
	}

	if vol.Attached || !vol.Published.Empty() || !vol.Staged.Empty() {
		msg := fmt.Sprintf("Volume '%s' is still used (attached: %v, staged: %v, published: %v) by '%s' node",
			vol.VolID, vol.Attached, vol.Staged, vol.Published, vol.NodeID)
		klog.Warning(msg)
	}

	if err := dv.deleteVolume(volId); err != nil {
		return nil, status.Error(codes.Internal, fmt.Sprintf("failed to delete volume %v: %v", volId, err))
	}
	klog.Infof("volume %v successfully deleted", volId)

	return &csi.DeleteVolumeResponse{}, nil
}

func (dv *directVolume) ControllerGetCapabilities(ctx context.Context, req *csi.ControllerGetCapabilitiesRequest) (*csi.ControllerGetCapabilitiesResponse, error) {
	return &csi.ControllerGetCapabilitiesResponse{
		Capabilities: dv.getControllerServiceCapabilities(),
	}, nil
}

func (dv *directVolume) ValidateVolumeCapabilities(ctx context.Context, req *csi.ValidateVolumeCapabilitiesRequest) (*csi.ValidateVolumeCapabilitiesResponse, error) {
	if len(req.GetVolumeId()) == 0 {
		return nil, status.Error(codes.InvalidArgument, "Volume ID cannot be empty")
	}
	if len(req.VolumeCapabilities) == 0 {
		return nil, status.Error(codes.InvalidArgument, req.VolumeId)
	}

	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	if _, err := dv.state.GetVolumeByID(req.GetVolumeId()); err != nil {
		klog.Warning("Validate volume vapability failed. Volume not found: might have already deleted")
		return nil, err
	}

	return &csi.ValidateVolumeCapabilitiesResponse{
		Confirmed: &csi.ValidateVolumeCapabilitiesResponse_Confirmed{
			VolumeContext:      req.GetVolumeContext(),
			VolumeCapabilities: req.GetVolumeCapabilities(),
			Parameters:         req.GetParameters(),
		},
	}, nil
}

func (dv *directVolume) GetCapacity(ctx context.Context, req *csi.GetCapacityRequest) (*csi.GetCapacityResponse, error) {
	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	// Topology and capabilities are irrelevant. We only
	// distinguish based on the "kind" parameter, if at all.
	// Without configured capacity, we just have the maximum size.
	available := dv.config.MaxVolumeSize
	if dv.config.Capacity.Enabled() {
		// Empty "kind" will return "zero capacity". There is no fallback
		// to some arbitrary kind here because in practice it always should
		// be set.
		kind := req.GetParameters()[storageKind]
		quantity := dv.config.Capacity[kind]
		allocated := dv.sumVolumeSizes(kind)
		available = quantity.Value() - allocated
	}
	maxVolumeSize := dv.config.MaxVolumeSize
	if maxVolumeSize > available {
		maxVolumeSize = available
	}

	return &csi.GetCapacityResponse{
		AvailableCapacity: available,
		MaximumVolumeSize: &wrappers.Int64Value{Value: maxVolumeSize},
		MinimumVolumeSize: &wrappers.Int64Value{Value: 0},
	}, nil
}

func (dv *directVolume) validateControllerServiceRequest(c csi.ControllerServiceCapability_RPC_Type) error {
	if c == csi.ControllerServiceCapability_RPC_UNKNOWN {
		return nil
	}

	for _, cap := range dv.getControllerServiceCapabilities() {
		if c == cap.GetRpc().GetType() {
			return nil
		}
	}
	return status.Errorf(codes.InvalidArgument, "unsupported capability %s", c)
}

func (dv *directVolume) getControllerServiceCapabilities() []*csi.ControllerServiceCapability {
	cl := []csi.ControllerServiceCapability_RPC_Type{
		csi.ControllerServiceCapability_RPC_CREATE_DELETE_VOLUME,
	}

	var csc []*csi.ControllerServiceCapability

	for _, cap := range cl {
		csc = append(csc, &csi.ControllerServiceCapability{
			Type: &csi.ControllerServiceCapability_Rpc{
				Rpc: &csi.ControllerServiceCapability_RPC{
					Type: cap,
				},
			},
		})
	}

	return csc
}

func (dv *directVolume) ControllerModifyVolume(context.Context, *csi.ControllerModifyVolumeRequest) (*csi.ControllerModifyVolumeResponse, error) {
	return nil, status.Error(codes.Unimplemented, "controllerModifyVolume is not supported")
}

func (dv *directVolume) ListVolumes(ctx context.Context, req *csi.ListVolumesRequest) (*csi.ListVolumesResponse, error) {
	return &csi.ListVolumesResponse{}, nil
}

func (dv *directVolume) ControllerGetVolume(ctx context.Context, req *csi.ControllerGetVolumeRequest) (*csi.ControllerGetVolumeResponse, error) {
	return &csi.ControllerGetVolumeResponse{}, nil
}

func (dv *directVolume) ControllerPublishVolume(ctx context.Context, req *csi.ControllerPublishVolumeRequest) (*csi.ControllerPublishVolumeResponse, error) {

	return &csi.ControllerPublishVolumeResponse{}, nil
}

func (dv *directVolume) ControllerUnpublishVolume(ctx context.Context, req *csi.ControllerUnpublishVolumeRequest) (*csi.ControllerUnpublishVolumeResponse, error) {

	return &csi.ControllerUnpublishVolumeResponse{}, nil
}

func (dv *directVolume) CreateSnapshot(ctx context.Context, req *csi.CreateSnapshotRequest) (*csi.CreateSnapshotResponse, error) {

	return &csi.CreateSnapshotResponse{}, nil
}

func (dv *directVolume) DeleteSnapshot(ctx context.Context, req *csi.DeleteSnapshotRequest) (*csi.DeleteSnapshotResponse, error) {

	return &csi.DeleteSnapshotResponse{}, nil
}

func (dv *directVolume) ListSnapshots(ctx context.Context, req *csi.ListSnapshotsRequest) (*csi.ListSnapshotsResponse, error) {

	return &csi.ListSnapshotsResponse{}, nil
}

func (dv *directVolume) ControllerExpandVolume(ctx context.Context, req *csi.ControllerExpandVolumeRequest) (*csi.ControllerExpandVolumeResponse, error) {

	return &csi.ControllerExpandVolumeResponse{}, nil
}
