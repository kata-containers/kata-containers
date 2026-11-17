// Copyright (c) 2026 Kata Contributors
// SPDX-License-Identifier: Apache-2.0

package directvolume

import (
	"strings"

	"github.com/container-storage-interface/spec/lib/go/csi"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"kata-containers/csi-kata-directvolume/pkg/state"
)

// capacityKind is shared by GetCapacity and CreateVolume. Conflicting spellings
// must fail deterministically even though Go map iteration is unordered.
func capacityKind(parameters map[string]string) (string, error) {
	kind := ""
	seen := false
	for key, value := range parameters {
		if !strings.EqualFold(key, storageKind) {
			continue
		}
		if value == "" || strings.TrimSpace(value) != value {
			return "", status.Error(codes.InvalidArgument, "capacity kind must be non-empty without surrounding whitespace")
		}
		if seen && value != kind {
			return "", status.Error(codes.InvalidArgument, "conflicting capacity kind parameters")
		}
		kind, seen = value, true
	}
	return kind, nil
}

// poolAvailable subtracts persisted reservations without overflowing int64.
// An unknown pool (including an unconfigured driver) has no advertised capacity.
func (dv *directVolume) poolAvailable(kind string) int64 {
	quantity, ok := dv.config.Capacity[kind]
	if !ok || kind == "" {
		return 0
	}
	available := quantity.Value()
	for _, volume := range dv.state.GetVolumes() {
		if volume.Kind != kind {
			continue
		}
		// Invalid restored reservations must not make a pool appear larger.
		if volume.VolSize < 0 || volume.VolSize >= available {
			return 0
		}
		available -= volume.VolSize
	}
	return max(0, available)
}

func (dv *directVolume) checkQuota(size int64, kind string) error {
	if size < 0 {
		return status.Error(codes.InvalidArgument, "negative volume size")
	}
	if dv.config.Capacity.Enabled() {
		if _, ok := dv.config.Capacity[kind]; !ok || kind == "" {
			return status.Error(codes.InvalidArgument, "a configured capacity kind is required")
		}
	} else if kind != "" {
		return status.Error(codes.InvalidArgument, "capacity tracking is disabled")
	}
	if size > dv.config.MaxVolumeSize {
		return status.Error(codes.OutOfRange, "requested size exceeds maximum volume size")
	}
	if dv.config.Capacity.Enabled() && size > dv.poolAvailable(kind) {
		return status.Error(codes.ResourceExhausted, "capacity pool is exhausted")
	}
	return nil
}

func (dv *directVolume) restoreVolume(id, name string, size int64, kind string) (*state.Volume, error) {
	if kind != "" {
		return dv.createVolume(id, name, size, kind)
	}
	// Volumes provisioned before capacity tracking have no pool. Do not assign
	// them to an arbitrary pool during node-stage/publish recovery.
	if size < 0 || size > dv.config.MaxVolumeSize {
		return nil, status.Error(codes.OutOfRange, "invalid restored volume size")
	}
	return dv.recordDirectVolume(id, name, size, kind)
}

func (dv *directVolume) matchesTopology(topology *csi.Topology) bool {
	if topology == nil {
		return true
	}
	if !dv.config.EnableTopology {
		return false
	}
	// A topology is an intersection of segments. Unknown segments cannot be
	// promised by a driver which publishes only its node key.
	for key, value := range topology.GetSegments() {
		if key != TopologyKeyNode || value != dv.config.NodeID {
			return false
		}
	}
	return true
}

func (dv *directVolume) accessible(requirements *csi.TopologyRequirement) bool {
	if requirements == nil {
		return true
	}
	if !dv.config.EnableTopology {
		return false
	}
	requisite := requirements.GetRequisite()
	if len(requisite) == 0 {
		return true
	}
	for _, topology := range requisite {
		if dv.matchesTopology(topology) {
			return true
		}
	}
	return false
}

// The provisioner capacity controller does not know a PVC's access mode yet
// and sends UNKNOWN. Accept that only for capacity queries, never creation.
func supportedCapacityCapabilities(capabilities []*csi.VolumeCapability, allowUnspecified bool) bool {
	for _, capability := range capabilities {
		if capability == nil || capability.GetMount() == nil {
			return false
		}
		mode := capability.GetAccessMode().GetMode()
		if allowUnspecified && mode == csi.VolumeCapability_AccessMode_UNKNOWN {
			continue
		}
		switch mode {
		case csi.VolumeCapability_AccessMode_SINGLE_NODE_WRITER,
			csi.VolumeCapability_AccessMode_SINGLE_NODE_READER_ONLY,
			csi.VolumeCapability_AccessMode_SINGLE_NODE_SINGLE_WRITER,
			csi.VolumeCapability_AccessMode_SINGLE_NODE_MULTI_WRITER:
		default:
			return false
		}
	}
	return true
}

func (dv *directVolume) validateCapacityRequest(req *csi.CreateVolumeRequest) (int64, string, error) {
	if req.GetName() == "" {
		return 0, "", status.Error(codes.InvalidArgument, "volume name is required")
	}
	capabilities := req.GetVolumeCapabilities()
	if len(capabilities) == 0 || !supportedCapacityCapabilities(capabilities, false) {
		return 0, "", status.Error(codes.InvalidArgument, "supported volume capabilities are required")
	}
	if !dv.accessible(req.GetAccessibilityRequirements()) {
		return 0, "", status.Error(codes.ResourceExhausted, "requested topology does not include this node")
	}
	kind, err := capacityKind(req.GetParameters())
	if err != nil {
		return 0, "", err
	}
	size, limit := req.GetCapacityRange().GetRequiredBytes(), req.GetCapacityRange().GetLimitBytes()
	if size < 0 || limit < 0 || (limit > 0 && size > limit) {
		return 0, "", status.Error(codes.InvalidArgument, "invalid capacity range")
	}
	return max(1, size), kind, nil
}
