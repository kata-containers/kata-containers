// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"errors"
	"fmt"
	"math"
	"sort"
	"strconv"
	"strings"

	"github.com/container-storage-interface/spec/lib/go/csi"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"k8s.io/klog/v2"

	"kata-containers/csi-kata-directvolume/pkg/state"
	"kata-containers/csi-kata-directvolume/pkg/utils"
)

const (
	// legacyCapacityPoolParameter is the StorageClass parameter which the
	// driver inherited from csi-driver-host-path for selecting a capacity
	// pool. It is accepted as an alias of
	// utils.KataContainersDirectCapacityPool.
	legacyCapacityPoolParameter = "kind"

	// statBlockSize is the unit of unix.Stat_t.Blocks.
	statBlockSize = 512
)

// errNoCapacity is wrapped by the errors which mean that this node cannot
// provide any storage for a request, for example because the requested
// capacity pool is not configured here. GetCapacity reports zero capacity
// for such requests and CreateVolume fails with codes.ResourceExhausted,
// which allows the external-provisioner to reschedule the volume.
var errNoCapacity = errors.New("no capacity on this node")

// volumeParameters are the CreateVolume and GetCapacity parameters which
// the driver understands.
type volumeParameters struct {
	// volumeType is utils.DirectVolumeTypeName or
	// utils.SpdkVolumeTypeName, or empty if the parameters do not ask for
	// a direct volume.
	volumeType string
	fsType     string
	// pool is the capacity pool the volume is reserved from, or empty if
	// the parameters do not select one.
	pool string
}

// parseVolumeParameters extracts the parameters understood by the driver.
// Parameter names are case insensitive. A malformed capacity pool
// parameter is an error: it is wrong on every node.
func parseVolumeParameters(params map[string]string) (volumeParameters, error) {
	var p volumeParameters
	var poolKeys []string
	for key, value := range params {
		switch strings.ToLower(key) {
		case utils.KataContainersDirectVolumeType:
			if value == utils.DirectVolumeTypeName || value == utils.SpdkVolumeTypeName {
				p.volumeType = value
			}
		case utils.KataContainersDirectFsType:
			p.fsType = value
		case utils.KataContainersDirectCapacityPool, legacyCapacityPoolParameter:
			poolKeys = append(poolKeys, key)
		}
	}

	sort.Strings(poolKeys)
	for _, key := range poolKeys {
		pool := params[key]
		if err := utils.ValidateCapacityPoolName(pool); err != nil {
			return volumeParameters{}, status.Errorf(codes.InvalidArgument, "parameter %q: %v", key, err)
		}
		if p.pool != "" && p.pool != pool {
			return volumeParameters{}, status.Errorf(codes.InvalidArgument,
				"parameters %q select different capacity pools %q and %q", poolKeys, p.pool, pool)
		}
		p.pool = pool
	}

	return p, nil
}

// volumeContext builds the volume context of a new volume.
func (p volumeParameters) volumeContext(name string, capacity int64) map[string]string {
	volumeCtx := map[string]string{
		utils.IsDirectVolume:   "False",
		utils.DirectVolumeName: name,
		utils.CapacityInBytes:  strconv.FormatInt(capacity, 10),
	}
	if p.volumeType != "" {
		volumeCtx[utils.IsDirectVolume] = "True"
		volumeCtx[utils.KataContainersDirectVolumeType] = p.volumeType
	}
	if p.fsType != "" {
		volumeCtx[utils.KataContainersDirectFsType] = p.fsType
	}
	if p.pool != "" {
		volumeCtx[utils.KataContainersDirectCapacityPool] = p.pool
	}
	return volumeCtx
}

// compatibleWith reports whether an existing volume could have been
// created with the parameters p.
func (p volumeParameters) compatibleWith(vol state.Volume) bool {
	if vol.Kind != p.pool {
		return false
	}
	if vol.VolumeContext == nil {
		// Created by an older driver which did not record the volume
		// context, only the pool can be compared.
		return true
	}
	return vol.VolumeContext[utils.KataContainersDirectVolumeType] == p.volumeType &&
		vol.VolumeContext[utils.KataContainersDirectFsType] == p.fsType
}

// validateVolumeCapabilities checks that the driver can provide a volume
// which has all of the given capabilities: a filesystem volume that is
// published on a single node.
func validateVolumeCapabilities(caps []*csi.VolumeCapability) error {
	for _, c := range caps {
		if c == nil {
			return errors.New("volume capability must not be empty")
		}
		// The external-provisioner calls GetCapacity with a Mount access
		// type whose MountVolume is nil, so the type is what matters.
		if _, isBlock := c.GetAccessType().(*csi.VolumeCapability_Block); isBlock {
			return errors.New("block access type is not supported")
		}
		switch mode := c.GetAccessMode().GetMode(); mode {
		case csi.VolumeCapability_AccessMode_UNKNOWN,
			csi.VolumeCapability_AccessMode_SINGLE_NODE_WRITER,
			csi.VolumeCapability_AccessMode_SINGLE_NODE_READER_ONLY,
			csi.VolumeCapability_AccessMode_SINGLE_NODE_SINGLE_WRITER,
			csi.VolumeCapability_AccessMode_SINGLE_NODE_MULTI_WRITER:
		default:
			return fmt.Errorf("access mode %s is not supported", mode)
		}
	}
	return nil
}

// localTopologySegments returns the topology segments of this node, as
// reported by NodeGetInfo.
func (dv *directVolume) localTopologySegments() map[string]string {
	if !dv.config.EnableTopology {
		return nil
	}
	return map[string]string{TopologyKeyNode: dv.config.NodeID}
}

// isLocalTopology reports whether volumes on this node are accessible from
// topology: every segment it asks for must match a segment of this node.
func (dv *directVolume) isLocalTopology(topology *csi.Topology) bool {
	local := dv.localTopologySegments()
	for key, value := range topology.GetSegments() {
		if localValue, ok := local[key]; !ok || localValue != value {
			return false
		}
	}
	return true
}

// accessibleFromRequirements reports whether a volume on this node satisfies
// the requisite topologies of a CreateVolume request.
func (dv *directVolume) accessibleFromRequirements(requirements *csi.TopologyRequirement) bool {
	if !dv.config.EnableTopology || len(requirements.GetRequisite()) == 0 {
		return true
	}
	for _, topology := range requirements.GetRequisite() {
		if dv.isLocalTopology(topology) {
			return true
		}
	}
	return false
}

// accessibleTopology returns the topology of volumes created on this node.
func (dv *directVolume) accessibleTopology() []*csi.Topology {
	if !dv.config.EnableTopology {
		return []*csi.Topology{}
	}
	return []*csi.Topology{{Segments: dv.localTopologySegments()}}
}

// reservedBytes returns the sum of the sizes of the volumes which are
// reserved from pool.
func (dv *directVolume) reservedBytes(pool string) int64 {
	var sum int64
	for _, vol := range dv.state.GetVolumes() {
		if vol.Kind == pool {
			sum += vol.VolSize
		}
	}
	return sum
}

// poolRemaining returns the number of bytes which are not reserved yet in
// the capacity pool selected by p. When no pool is configured on this node,
// volumes are not limited by a pool and math.MaxInt64 is returned for
// requests which do not ask for one.
func (dv *directVolume) poolRemaining(p volumeParameters) (int64, error) {
	if !dv.config.Capacity.Enabled() {
		if p.pool != "" {
			return 0, fmt.Errorf("%w: capacity pool %q is not configured on node %q",
				errNoCapacity, p.pool, dv.config.NodeID)
		}
		return math.MaxInt64, nil
	}

	if p.pool == "" {
		return 0, fmt.Errorf("%w: node %q has the capacity pools %s and the %q parameter must select one of them",
			errNoCapacity, dv.config.NodeID, dv.config.Capacity.String(), utils.KataContainersDirectCapacityPool)
	}

	quantity, ok := dv.config.Capacity[p.pool]
	if !ok {
		return 0, fmt.Errorf("%w: capacity pool %q is not configured on node %q, configured pools are %s",
			errNoCapacity, p.pool, dv.config.NodeID, dv.config.Capacity.String())
	}

	return quantity.Value() - dv.reservedBytes(p.pool), nil
}

// filesystemStat describes the filesystem which holds a directory.
type filesystemStat struct {
	// free is the number of bytes available on the filesystem.
	free int64
	// device identifies the filesystem.
	device uint64
}

// backingStorage gives access to the filesystems which hold the backing
// files of volumes. Tests replace it with a fake.
type backingStorage interface {
	// Statfs describes the filesystem which holds the directory dir.
	Statfs(dir string) (filesystemStat, error)
	// AllocatedBytes returns the number of bytes allocated on disk for
	// the file at path, or 0 if it does not exist.
	AllocatedBytes(path string) (int64, error)
	// CreateRawDisk allocates the raw disk of a directvol volume below
	// storagePath and returns its path. It is idempotent.
	CreateRawDisk(storagePath, volID string, size int64) (string, error)
}

type hostStorage struct{}

func (hostStorage) Statfs(dir string) (filesystemStat, error) {
	var fs unix.Statfs_t
	if err := unix.Statfs(dir, &fs); err != nil {
		return filesystemStat{}, err
	}
	var st unix.Stat_t
	if err := unix.Stat(dir, &st); err != nil {
		return filesystemStat{}, err
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

	return filesystemStat{free: free, device: uint64(st.Dev)}, nil
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

func (hostStorage) CreateRawDisk(storagePath, volID string, size int64) (string, error) {
	devicePath, err := utils.CreateDirectBlockDevice(volID, strconv.FormatInt(size, 10), storagePath)
	if err != nil {
		return "", err
	}
	return *devicePath, nil
}

// volumeTypeOf returns the type of an existing volume.
func volumeTypeOf(vol state.Volume) string {
	if volumeType := vol.Metadata["type"]; volumeType != "" {
		return volumeType
	}
	if volumeType := vol.VolumeContext[utils.KataContainersDirectVolumeType]; volumeType != "" {
		return volumeType
	}
	return utils.DirectVolumeTypeName
}

// backingDir returns the directory which holds the backing files of
// volumes of the given type.
func (dv *directVolume) backingDir(volumeType string) string {
	if volumeType == utils.SpdkVolumeTypeName {
		return dv.config.SpdkRawPath
	}
	return dv.config.StoragePath
}

// backingFile returns the path of the file which backs an existing volume,
// or "" if it has none yet. CreateVolume records the raw disk of a
// directvol volume and the backing file of an SPDK volume in the state.
func (dv *directVolume) backingFile(vol state.Volume) string {
	return vol.Metadata["backingFile"]
}

// filesystemRemaining returns how many bytes new volumes of the given type
// can still take from the filesystem which holds their backing files: its
// free space minus the part of every existing volume on that filesystem
// which is reserved but not allocated yet.
//
// The raw disk of a directvol volume is allocated when the volume is
// created, SPDK backing files are sparse and only get allocated when the
// guest writes to them. The allocated part of a backing file is already missing
// from the free space, so only the rest of each reservation is subtracted.
// That way no byte is counted twice and sparse files cannot be
// overcommitted.
func (dv *directVolume) filesystemRemaining(volumeType string) (int64, error) {
	dir := dv.backingDir(volumeType)
	fs, err := dv.storage.Statfs(dir)
	if err != nil {
		return 0, status.Errorf(codes.Internal, "check free space of %q: %v", dir, err)
	}

	devices := map[string]uint64{dir: fs.device}
	var unallocated int64
	for _, vol := range dv.state.GetVolumes() {
		volDir := dv.backingDir(volumeTypeOf(vol))
		device, known := devices[volDir]
		if !known {
			volFS, err := dv.storage.Statfs(volDir)
			if err != nil {
				return 0, status.Errorf(codes.Internal, "check free space of %q: %v", volDir, err)
			}
			device = volFS.device
			devices[volDir] = device
		}
		if device != fs.device {
			continue
		}

		file := dv.backingFile(vol)
		allocated, err := dv.storage.AllocatedBytes(file)
		if err != nil {
			return 0, status.Errorf(codes.Internal, "check allocated size of %q: %v", file, err)
		}
		if rest := vol.VolSize - allocated; rest > 0 {
			unallocated += rest
		}
	}

	// Guests allocate blocks of sparse files at any time. Read the free space
	// after the allocations: a block allocated in between then counts twice,
	// which is safe, instead of not at all.
	fs, err = dv.storage.Statfs(dir)
	if err != nil {
		return 0, status.Errorf(codes.Internal, "check free space of %q: %v", dir, err)
	}

	return fs.free - unallocated, nil
}

// remainingCapacity returns how many bytes a new volume with the parameters
// p can still reserve on this node: what is left in the selected capacity
// pool, limited by what the backing filesystem can still provide. The result
// is never negative. Errors wrapping errNoCapacity mean that this node cannot
// provide storage for p at all.
func (dv *directVolume) remainingCapacity(p volumeParameters) (int64, error) {
	remaining, err := dv.poolRemaining(p)
	if err != nil {
		return 0, err
	}

	fsRemaining, err := dv.filesystemRemaining(p.volumeType)
	if err != nil {
		return 0, err
	}

	return max(min(remaining, fsRemaining), 0), nil
}

// admitVolume checks that a new volume of the given size can be reserved
// with the parameters p. It must be called, with dv.mutex held, before any
// backend resource is created for the volume.
func (dv *directVolume) admitVolume(size int64, p volumeParameters) error {
	if size > dv.config.MaxVolumeSize {
		return status.Errorf(codes.OutOfRange, "requested capacity %d exceeds maximum allowed %d", size, dv.config.MaxVolumeSize)
	}

	remaining, err := dv.remainingCapacity(p)
	if errors.Is(err, errNoCapacity) {
		return status.Error(codes.ResourceExhausted, err.Error())
	}
	if err != nil {
		return err
	}

	if size > remaining {
		pool := "no capacity pool"
		if p.pool != "" {
			pool = fmt.Sprintf("capacity pool %q", p.pool)
		}
		return status.Errorf(codes.ResourceExhausted,
			"requested capacity %d exceeds the remaining capacity %d on node %q (%s)",
			size, remaining, dv.config.NodeID, pool)
	}

	return nil
}

// logCapacityState logs the configured pools and what is reserved in them,
// and warns about reservations which no longer fit the configuration.
func (dv *directVolume) logCapacityState() {
	reserved := map[string]int64{}
	for _, vol := range dv.state.GetVolumes() {
		reserved[vol.Kind] += vol.VolSize
	}

	pools := make([]string, 0, len(reserved))
	for pool := range reserved {
		pools = append(pools, pool)
	}
	sort.Strings(pools)

	if dv.config.Capacity.Enabled() {
		klog.Infof("capacity pools on node %q: %s", dv.config.NodeID, dv.config.Capacity.String())
	} else {
		klog.Infof("no capacity pools are configured on node %q, volumes are only limited by the free space of their backing filesystem",
			dv.config.NodeID)
	}
	for _, pool := range pools {
		if pool == "" {
			klog.Infof("%d bytes are reserved by volumes without a capacity pool", reserved[pool])
			continue
		}
		quantity, configured := dv.config.Capacity[pool]
		switch {
		case !configured:
			klog.Warningf("%d bytes are reserved from capacity pool %q, which is no longer configured", reserved[pool], pool)
		case reserved[pool] > quantity.Value():
			klog.Warningf("%d bytes are reserved from capacity pool %q, which only has %d bytes", reserved[pool], pool, quantity.Value())
		default:
			klog.Infof("%d of %d bytes are reserved from capacity pool %q", reserved[pool], quantity.Value(), pool)
		}
	}
}
