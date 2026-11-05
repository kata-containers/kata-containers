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

	// kind 参数用于选择本卷记账的容量池（capacity pool）。
	// 该参数必须透传：CreateVolume 的容量记账、持久化的卷状态、
	// GetCapacity 的容量汇报、以及后续 NodeStage/NodePublish 都必须
	// 使用同一个池，两侧账本才能一致（issue #14151 gap 3）。
	// The "kind" parameter selects the capacity pool this volume is
	// accounted against and must be propagated to all consumers.
	kind := ""

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
			kind = value
			volumeCtx[utils.StorageKind] = value
		default:
			continue
		}
	}

	// 容量跟踪开启时，池标识必须存在且已知：GetCapacity 从同一个
	// 参数解析池，缺失或未知的池没有可报告的容量。此处宁可
	// 确定性地拒绝（InvalidArgument），也不静默地随机选择一个池——
	// Go map 遍历顺序是随机的，随机选池会让卷的记账去向不可复现，
	// 且与 GetCapacity 通报的池不一致，造成误调度。
	// 校验规则保持对称：跟踪开启时 kind 为必需品（缺失/未知均拒绝），
	// 跟踪关闭时 kind 为违禁品（出现即拒绝），不存在中间地带。
	// With capacity tracking on, the pool must be present and known;
	// deterministic rejection beats nondeterministic pool selection.
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

func (dv *directVolume) GetCapacity(ctx context.Context, req *csi.GetCapacityRequest) (*csi.GetCapacityResponse, error) {
	dv.mutex.Lock()
	defer dv.mutex.Unlock()

	// 拓扑与卷能力暂不参与容量计算（issue #14151 gap 5 另行处理），
	// 仅按 "kind" 参数区分容量池。
	// Topology and volume capabilities are not part of capacity yet.
	if !dv.config.Capacity.Enabled() {
		// 未配置容量跟踪时，驱动没有可报告的逻辑容量模型。
		// 注意：MaxVolumeSize 是"单卷尺寸上限"，不是节点的可用
		// 存储，把它当节点容量上报属于语义挪用（issue #14151
		// gap 2a）：默认部署下会对外谎报 1TiB。此处如实返回零，
		// 调度器看到零只会跳过本节点，是安全的行为。
		// No logical capacity model: report zero. MaxVolumeSize is a
		// per-volume limit, not node capacity.
		return &csi.GetCapacityResponse{
			AvailableCapacity: 0,
			MaximumVolumeSize: &wrappers.Int64Value{Value: 0},
			MinimumVolumeSize: &wrappers.Int64Value{Value: 0},
		}, nil
	}

	// 与 CreateVolume 解析同一个容量池（同一个 "kind" 参数），
	// 保证读写两侧账本同页。未知或缺失的池没有可用容量，
	// 返回零容量而不是报错——GetCapacity 是查询型 RPC，
	// "最多为零"是如实的回答。
	// Resolve the same pool CreateVolume uses; unknown pools have
	// no capacity to offer.
	kind := req.GetParameters()[storageKind]
	quantity, ok := dv.config.Capacity[kind]
	if !ok {
		return &csi.GetCapacityResponse{
			AvailableCapacity: 0,
			MaximumVolumeSize: &wrappers.Int64Value{Value: 0},
			MinimumVolumeSize: &wrappers.Int64Value{Value: 0},
		}, nil
	}

	allocated := dv.sumVolumeSizes(kind)
	// 负值钳制为零（issue #14151 gap 2b）：池超配（配置被调小、
	// 状态漂移、或绕过配额检查的分配路径）会让差值为负。
	// 负的 available_capacity 违反 CSI 规范，且流入 external-
	// provisioner 的 CSIStorageCapacity（resource.Quantity）后可能
	// 溢出为巨大正数，被调度器误解为"容量无穷大"，反而把 workload
	// 调度到最没空间的节点。钳成零是安全的；同时打一条含
	// 配置值/已分配值的告警日志，为诊断状态漂移留线索。
	// Clamp negatives to zero: negative capacity must not leak into
	// CSIStorageCapacity where it can overflow into a huge quantity.
	available := quantity.Value() - allocated
	if available < 0 {
		klog.Warningf("capacity pool %q is overcommitted: configured %d, allocated %d, reporting zero", kind, quantity.Value(), allocated)
		available = 0
	}

	// 维持不变式 0 <= maximum_volume_size <= available_capacity，
	// 同时尊重配置的单卷尺寸上限（MaxVolumeSize）。
	// Enforce 0 <= maximum_volume_size <= available_capacity.
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
