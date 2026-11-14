//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"context"
	"fmt"
	"os"
	"sort"
	"strings"

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

	// kind selects the capacity pool that this volume is accounted
	// against. It must be propagated so that the same pool is used by
	// CreateVolume capacity accounting, the persisted volume state,
	// GetCapacity and later NodeStage/NodePublish calls (#14151, gap 3).
	//
	// Parameter names are case-insensitive, so "kind" and "Kind" are
	// the same parameter and a request may carry both spellings. The
	// last spelling seen in map iteration would silently win, and map
	// order is random — the same request could charge one pool and
	// then another on a retry. Conflicting spellings with different
	// values are therefore rejected upfront instead of being resolved
	// nondeterministically.
	kind := ""
	var kindKeys []string

	for key, value := range req.GetParameters() {
		switch strings.ToLower(key) {
		case utils.KataContainersDirectVolumeType:
			if value == utils.DirectVolumeTypeName || value == utils.SpdkVolumeTypeName {
				volumeCtx[utils.IsDirectVolume] = "True"
				volumeCtx[utils.KataContainersDirectVolumeType] = value
			}
		case utils.KataContainersDirectFsType:
			volumeCtx[utils.KataContainersDirectFsType] = value
		case utils.StorageKind:
			kindKeys = append(kindKeys, key)
			if kind == "" {
				kind = value
			}
		default:
			continue
		}
	}

	// Deterministic conflict detection: the kindKeys are sorted, so the
	// error message is stable no matter which map entry iterated first.
	// Equal values under different spellings are fine — they select the
	// same pool and the canonical (lowercase) key is the one propagated
	// in volumeCtx.
	sort.Strings(kindKeys)
	for _, key := range kindKeys {
		if value := req.GetParameters()[key]; value != kind {
			return nil, status.Errorf(codes.InvalidArgument,
				"parameters %q select different capacity pools %q and %q", kindKeys, kind, value)
		}
	}
	if kind != "" {
		volumeCtx[utils.StorageKind] = kind
	}

	// When capacity tracking is configured, the pool must be known:
	// GetCapacity picks the pool from the same parameter, and an
	// unknown or missing pool has no meaningful capacity. Missing or
	// unknown kinds are therefore rejected instead of silently picking
	// an arbitrary pool: Go map iteration order is random, so a
	// fallback would make pool accounting nondeterministic and
	// inconsistent with what GetCapacity advertises, causing
	// mis-scheduling. The validation is symmetric: with tracking
	// enabled, "kind" is required; with tracking disabled, passing
	// "kind" is rejected. Deterministic failure beats lucky success.
	if dv.config.Capacity.Enabled() {
		if kind == "" {
			return nil, status.Errorf(codes.InvalidArgument, "capacity tracking is enabled, storage class parameter %q must select a capacity pool", utils.StorageKind)
		}
		if _, ok := dv.config.Capacity[kind]; !ok {
			return nil, status.Errorf(codes.InvalidArgument, "unknown capacity pool %q, configured pools: %v", kind, dv.config.Capacity.Kinds())
		}
	} else if kind != "" {
		return nil, status.Errorf(codes.InvalidArgument, "capacity tracking is disabled, specifying %q parameter is invalid", utils.StorageKind)
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

	var vol *state.Volume
	var err error
	if volumeCtx[utils.KataContainersDirectVolumeType] == utils.SpdkVolumeTypeName {
		vol, err = dv.createSPDKVolume(volumeID, req.GetName(), capacity, kind)
		if err != nil {
			return nil, err
		}
	} else {
		vol, err = dv.createVolume(volumeID, req.GetName(), capacity, kind)
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

	return &csi.ValidateVolumeCapabilitiesResponse{
		Confirmed: &csi.ValidateVolumeCapabilitiesResponse_Confirmed{
			VolumeContext:      req.GetVolumeContext(),
			VolumeCapabilities: req.GetVolumeCapabilities(),
			Parameters:         req.GetParameters(),
		},
	}, nil
}

// zeroCapacity is the response for a query this node cannot answer
// with usable capacity: a topology it does not serve, an unknown pool,
// or capacity tracking that is not configured. Zero is the honest
// answer for a capacity query (rather than an error): a scheduler
// seeing zero merely skips this node (#14151).
func zeroCapacity() *csi.GetCapacityResponse {
	return &csi.GetCapacityResponse{
		AvailableCapacity: 0,
		MaximumVolumeSize: &wrappers.Int64Value{Value: 0},
		MinimumVolumeSize: &wrappers.Int64Value{Value: 0},
	}
}

// localTopologyMatch reports whether a GetCapacity request's
// accessible topology is satisfiable by this node (#14151, gap 5).
//
// The rules, mirroring how the driver publishes its own topology
// (NodeGetInfo, CreateVolume):
//
//   - No topology requested: satisfiable — the caller does not care
//     where the volume is placed.
//   - Topology requested while the driver does not publish topology
//     (EnableTopology off): the request disagrees with what the node
//     advertises, so it cannot be satisfied here. Returning the
//     node's capacity for a topology it never announced would let a
//     central caller provision on the wrong node.
//   - Topology requested and enabled: satisfiable only when the
//     segment for the driver's own key selects this node. The driver
//     publishes exactly one segment (topology.directvolume.csi/node),
//     so only that key can match; a request carrying other keys
//     alongside is fine as long as the driver's key selects this
//     node, because the external-provisioner forwards the full set
//     of topology segments it picked from the chosen node.
func (dv *directVolume) localTopologyMatch(segments map[string]string) bool {
	if len(segments) == 0 {
		return true
	}
	if !dv.config.EnableTopology {
		return false
	}
	return segments[TopologyKeyNode] == dv.config.NodeID
}

func (dv *directVolume) GetCapacity(ctx context.Context, req *csi.GetCapacityRequest) (*csi.GetCapacityResponse, error) {
	// Now that the capability is advertised (see
	// getControllerServiceCapabilities), the RPC validates it like
	// the other controller RPCs do.
	if err := dv.validateControllerServiceRequest(csi.ControllerServiceCapability_RPC_GET_CAPACITY); err != nil {
		return nil, err
	}

	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	// A topology which this node cannot satisfy (#14151, gap 5): the
	// node may have capacity, but not for that placement, so zero is
	// the honest answer and lets a central caller route the request
	// elsewhere.
	if !dv.localTopologyMatch(req.GetAccessibleTopology().GetSegments()) {
		return zeroCapacity(), nil
	}

	if !dv.config.Capacity.Enabled() {
		// Without configured capacity tracking there is no logical
		// capacity model to report. MaxVolumeSize is a per-volume
		// size limit, not the node's available storage, so reporting
		// it as node capacity (previously the behavior, #14151 gap
		// 2a) would advertise a phantom 1TiB in the default
		// deployment.
		return zeroCapacity(), nil
	}

	// Resolve the same capacity pool that CreateVolume uses (the
	// same "kind" parameter) so that both sides of the ledger agree.
	// An unknown or missing pool has no usable capacity; reporting
	// zero (rather than an error) is the honest answer for a query:
	// "at most nothing" is what the node can offer for that pool.
	kind := req.GetParameters()[storageKind]
	quantity, ok := dv.config.Capacity[kind]
	if !ok {
		return zeroCapacity(), nil
	}

	allocated := dv.sumVolumeSizes(kind)
	// Clamp negative results to zero (#14151, gap 2b): an
	// overcommitted pool (reduced configuration, state drift, or
	// allocation paths that bypass quota enforcement) yields a
	// negative difference. A negative available_capacity violates
	// the CSI spec and is harmful downstream: stored into a
	// CSIStorageCapacity resource.Quantity, negative values can
	// overflow into a huge positive quantity that the scheduler
	// reads as abundant capacity, attracting workloads to the node
	// with the least storage. Zero is the safe answer; the warning
	// keeps the configured and allocated values visible so the drift
	// stays diagnosable.
	available := quantity.Value() - allocated
	if available < 0 {
		klog.Warningf("capacity pool %q is overcommitted: configured %d, allocated %d, reporting zero", kind, quantity.Value(), allocated)
		available = 0
	}

	// Enforce 0 <= maximum_volume_size <= available_capacity, also
	// respecting the configured per-volume maximum (MaxVolumeSize).
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
	// The GET_CAPACITY capability is advertised unconditionally: the
	// RPC answers zero for nodes without configured capacity pools,
	// which is meaningful (it tells the scheduler to skip the node)
	// rather than an error (#14151, gap 6).
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
