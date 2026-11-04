//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"context"
	"errors"
	"fmt"
	"os"

	"github.com/golang/protobuf/ptypes/wrappers"
	"github.com/pborman/uuid"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"

	"github.com/container-storage-interface/spec/lib/go/csi"
	"k8s.io/klog/v2"

	"kata-containers/csi-kata-directvolume/pkg/spdkrpc"
	"kata-containers/csi-kata-directvolume/pkg/state"
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
	if len(caps) == 0 {
		return nil, status.Error(codes.InvalidArgument, "Volume Capabilities missing in request")
	}
	if err := validateVolumeCapabilities(caps); err != nil {
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	params, err := parseVolumeParameters(req.GetParameters())
	if err != nil {
		return nil, err
	}
	capacity, err := requiredCapacity(req.GetCapacityRange())
	if err != nil {
		return nil, err
	}
	klog.Infof("createVolume with request: %+v", req)

	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	topologies := dv.accessibleTopology()
	contentSrc := req.GetVolumeContentSource()

	// Need to check for already existing volume name, and if found
	// check that it is compatible with the request. A repeated request
	// returns the existing volume and does not reserve capacity again.
	if exVol, err := dv.state.GetVolumeByName(req.GetName()); err == nil {
		if exVol.VolSize < capacity {
			return nil, status.Errorf(codes.AlreadyExists, "Volume with the same name: %s but with different size already exist", req.GetName())
		}
		if limit := req.GetCapacityRange().GetLimitBytes(); limit > 0 && exVol.VolSize > limit {
			return nil, status.Errorf(codes.AlreadyExists, "Volume with the same name: %s already exists with %d bytes, more than the limit of %d bytes",
				req.GetName(), exVol.VolSize, limit)
		}
		if !params.compatibleWith(exVol) {
			return nil, status.Errorf(codes.AlreadyExists, "Volume with the same name: %s already exists with different parameters", req.GetName())
		}
		if !dv.accessibleFromRequirements(req.GetAccessibilityRequirements()) {
			return nil, status.Errorf(codes.AlreadyExists, "Volume with the same name: %s already exists on node %q, which does not satisfy the accessibility requirements",
				req.GetName(), dv.config.NodeID)
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

		volumeCtx := exVol.VolumeContext
		if volumeCtx == nil {
			volumeCtx = params.volumeContext(exVol.VolName, exVol.VolSize)
		}

		return &csi.CreateVolumeResponse{
			Volume: &csi.Volume{
				VolumeId:           exVol.VolID,
				CapacityBytes:      exVol.VolSize,
				VolumeContext:      volumeCtx,
				ContentSource:      contentSrc,
				AccessibleTopology: topologies,
			},
		}, nil
	}

	// Volumes are node-local: honour the node selected by the scheduler.
	if !dv.accessibleFromRequirements(req.GetAccessibilityRequirements()) {
		return nil, status.Errorf(codes.ResourceExhausted, "volumes on node %q do not satisfy the requisite topologies %v",
			dv.config.NodeID, req.GetAccessibilityRequirements().GetRequisite())
	}

	volumeID := uuid.NewUUID().String()
	volumeCtx := params.volumeContext(req.GetName(), capacity)

	var vol *state.Volume
	if params.volumeType == utils.SpdkVolumeTypeName {
		vol, err = dv.createSPDKVolume(volumeID, req.GetName(), capacity, params, volumeCtx)
		if err != nil {
			klog.Errorf("failed to create SPDK volume %s: %v", volumeID, err)
			return nil, err
		}
	} else {
		vol, err = dv.createVolume(volumeID, req.GetName(), capacity, params, volumeCtx)
		if err != nil {
			klog.Errorf("failed to create volume %s: %v", volumeID, err)
			return nil, err
		}
		klog.Infof("created volume %s at path %s", vol.VolID, vol.VolPath)
	}

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

		if err == nil {
			err = dv.state.UpdateVolume(*vol)
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

	if vol.Metadata["type"] == utils.SpdkVolumeTypeName {
		bdevName := vol.Metadata["bdevName"]
		backingFile := vol.Metadata["backingFile"]

		if bdevName != "" {
			_, err := spdkrpc.Call("bdev_aio_delete", map[string]any{"name": bdevName})
			if err != nil {
				if se, ok := err.(*spdkrpc.SpdkError); ok && se.Code == spdkrpc.SpdkErrNoDevice {
					klog.Infof("SPDK bdev %s already absent (code=%d), treat as success", bdevName, se.Code)
				} else {
					klog.Errorf("Failed to delete SPDK bdev %s: %v", bdevName, err)
					return nil, status.Errorf(codes.Internal, "Failed to delete SPDK bdev: %v", err)
				}
			} else {
				klog.Infof("SPDK bdev %s deleted for volume %s", bdevName, volId)
			}
		} else {
			klog.Warningf("SPDK volume %s has no bdevName, skip SPDK deletion", volId)
		}

		if backingFile != "" {
			if err := os.Remove(backingFile); err != nil && !os.IsNotExist(err) {
				klog.Warningf("Failed to remove backing file %s for volume %s: %v", backingFile, volId, err)
			} else {
				klog.Infof("Removed backing file %s for volume %s", backingFile, volId)
			}
		}
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

	if err := validateVolumeCapabilities(req.GetVolumeCapabilities()); err != nil {
		return &csi.ValidateVolumeCapabilitiesResponse{Message: err.Error()}, nil
	}

	return &csi.ValidateVolumeCapabilitiesResponse{
		Confirmed: &csi.ValidateVolumeCapabilitiesResponse_Confirmed{
			VolumeContext:      req.GetVolumeContext(),
			VolumeCapabilities: req.GetVolumeCapabilities(),
			Parameters:         req.GetParameters(),
		},
	}, nil
}

// GetCapacity reports how much storage a new volume with the requested
// parameters can still get on this node. It uses the same calculation as
// the admission check of CreateVolume. Requests which this node cannot
// serve at all (unsupported capabilities, the topology of another node,
// a capacity pool which is not configured here) get zero capacity.
func (dv *directVolume) GetCapacity(ctx context.Context, req *csi.GetCapacityRequest) (*csi.GetCapacityResponse, error) {
	params, err := parseVolumeParameters(req.GetParameters())
	if err != nil {
		return nil, err
	}

	if err := validateVolumeCapabilities(req.GetVolumeCapabilities()); err != nil {
		klog.V(4).Infof("GetCapacity: no capacity for the requested volume capabilities: %v", err)
		return capacityResponse(0, 0), nil
	}

	if topology := req.GetAccessibleTopology(); topology != nil && !dv.isLocalTopology(topology) {
		klog.V(4).Infof("GetCapacity: no capacity for topology %v on node %q", topology.GetSegments(), dv.config.NodeID)
		return capacityResponse(0, 0), nil
	}

	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	available, err := dv.remainingCapacity(params)
	if errors.Is(err, errNoCapacity) {
		klog.V(4).Infof("GetCapacity: %v", err)
		return capacityResponse(0, 0), nil
	}
	if err != nil {
		return nil, err
	}

	return capacityResponse(available, min(available, dv.config.MaxVolumeSize)), nil
}

// capacityResponse builds a GetCapacity response. Both values must be
// non-negative and maximumVolumeSize must not exceed available.
func capacityResponse(available, maximumVolumeSize int64) *csi.GetCapacityResponse {
	return &csi.GetCapacityResponse{
		AvailableCapacity: available,
		MaximumVolumeSize: &wrappers.Int64Value{Value: maximumVolumeSize},
		MinimumVolumeSize: &wrappers.Int64Value{Value: 0},
	}
}

// requiredCapacity returns the size of a new volume for the given capacity
// range.
func requiredCapacity(capacityRange *csi.CapacityRange) (int64, error) {
	required := capacityRange.GetRequiredBytes()
	limit := capacityRange.GetLimitBytes()
	if required < 0 || limit < 0 {
		return 0, status.Errorf(codes.InvalidArgument, "capacity range %v must not be negative", capacityRange)
	}
	if limit > 0 && required > limit {
		return 0, status.Errorf(codes.InvalidArgument, "required bytes %d exceed the limit of %d bytes", required, limit)
	}
	return required, nil
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
		csi.ControllerServiceCapability_RPC_GET_CAPACITY,
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
