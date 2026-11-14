// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package directvolume

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	csi "github.com/container-storage-interface/spec/lib/go/csi"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"

	"kata-containers/csi-kata-directvolume/pkg/spdkrpc"
	"kata-containers/csi-kata-directvolume/pkg/state"
	"kata-containers/csi-kata-directvolume/pkg/utils"
)

const (
	testNodeID    = "node-a"
	otherNodeID   = "node-b"
	testFreeSpace = 100 * utils.GiB
)

var volumeTypes = []string{utils.DirectVolumeTypeName, utils.SpdkVolumeTypeName}

// fakeStorage simulates the filesystems which hold the backing files.
type fakeStorage struct {
	// free is the free space of each directory, testFreeSpace if unset.
	free map[string]int64
	// device identifies the filesystem of each directory, 0 if unset.
	device map[string]uint64
	// allocated is the number of bytes allocated for each file.
	allocated map[string]int64
	err       error
}

func newFakeStorage() *fakeStorage {
	return &fakeStorage{
		free:      map[string]int64{},
		device:    map[string]uint64{},
		allocated: map[string]int64{},
	}
}

func (f *fakeStorage) Statfs(dir string) (filesystemStat, error) {
	if f.err != nil {
		return filesystemStat{}, f.err
	}
	free, ok := f.free[dir]
	if !ok {
		free = testFreeSpace
	}
	return filesystemStat{free: free, device: f.device[dir]}, nil
}

func (f *fakeStorage) AllocatedBytes(path string) (int64, error) {
	return f.allocated[path], nil
}

type testDriverOptions struct {
	nodeID          string
	capacity        []string
	maxVolumeSize   int64
	disableTopology bool
	// capacityFiles are written to the capacity config directory.
	capacityFiles map[string]string
}

type testDriver struct {
	*directVolume
	storage *fakeStorage
	spdk    *fakeSpdk
}

// newTestDriver creates a driver whose directories live in a temporary
// directory, with fake SPDK RPCs and a fake backing filesystem.
func newTestDriver(t *testing.T, opts testDriverOptions) *testDriver {
	t.Helper()
	tmp := t.TempDir()

	oldRawDir, oldVhostDir, oldCall := utils.SpdkRawDiskDir, utils.SpdkVhostDir, spdkrpc.Call
	spdk := newFakeSpdk()
	spdkrpc.Call = spdk.fn
	t.Cleanup(func() {
		utils.SpdkRawDiskDir, utils.SpdkVhostDir, spdkrpc.Call = oldRawDir, oldVhostDir, oldCall
	})

	cfg := Config{
		DriverName:     "directvolume.csi.katacontainers.io",
		Endpoint:       "unix://" + filepath.Join(tmp, "csi.sock"),
		NodeID:         opts.nodeID,
		VendorVersion:  "test",
		MaxVolumeSize:  opts.maxVolumeSize,
		EnableTopology: !opts.disableTopology,
		StoragePath:    filepath.Join(tmp, "storage"),
		StateDir:       filepath.Join(tmp, "state"),
		SpdkRawPath:    filepath.Join(tmp, "spdk-raw"),
		SpdkVhostPath:  filepath.Join(tmp, "spdk-vhost"),
	}
	if cfg.NodeID == "" {
		cfg.NodeID = testNodeID
	}
	if cfg.MaxVolumeSize == 0 {
		cfg.MaxVolumeSize = utils.TiB
	}
	for _, c := range opts.capacity {
		require.NoError(t, cfg.Capacity.Set(c))
	}
	if opts.capacityFiles != nil {
		cfg.CapacityConfigDir = filepath.Join(tmp, "capacity")
		require.NoError(t, os.MkdirAll(cfg.CapacityConfigDir, 0750))
		for node, content := range opts.capacityFiles {
			require.NoError(t, os.WriteFile(filepath.Join(cfg.CapacityConfigDir, node), []byte(content), 0600))
		}
	}

	dv, err := NewDirectVolumeDriver(cfg)
	require.NoError(t, err)
	storage := newFakeStorage()
	dv.storage = storage

	return &testDriver{directVolume: dv, storage: storage, spdk: spdk}
}

// restart simulates a driver restart: a new driver instance is created from
// the same configuration and therefore restores the same state file.
func (d *testDriver) restart(t *testing.T) *testDriver {
	t.Helper()
	cfg := d.config
	cfg.VolumeDevices = nil
	dv, err := NewDirectVolumeDriver(cfg)
	require.NoError(t, err)
	dv.storage = d.storage
	return &testDriver{directVolume: dv, storage: d.storage, spdk: d.spdk}
}

func volumeParams(volumeType, pool string) map[string]string {
	params := map[string]string{
		utils.KataContainersDirectVolumeType: volumeType,
		utils.KataContainersDirectFsType:     "ext4",
	}
	if pool != "" {
		params[utils.KataContainersDirectCapacityPool] = pool
	}
	return params
}

func createRequest(name string, size int64, volumeType, pool string) *csi.CreateVolumeRequest {
	return &csi.CreateVolumeRequest{
		Name:               name,
		CapacityRange:      &csi.CapacityRange{RequiredBytes: size},
		Parameters:         volumeParams(volumeType, pool),
		VolumeCapabilities: []*csi.VolumeCapability{singleNodeWriterCap()},
	}
}

func singleNodeWriterCap() *csi.VolumeCapability {
	return &csi.VolumeCapability{
		AccessType: &csi.VolumeCapability_Mount{Mount: &csi.VolumeCapability_MountVolume{}},
		AccessMode: &csi.VolumeCapability_AccessMode{Mode: csi.VolumeCapability_AccessMode_SINGLE_NODE_WRITER},
	}
}

// provisionerCapacityCap is the capability which the external-provisioner
// passes to GetCapacity: a Mount access type without MountVolume and an
// unknown access mode.
func provisionerCapacityCap() *csi.VolumeCapability {
	return &csi.VolumeCapability{
		AccessType: &csi.VolumeCapability_Mount{},
		AccessMode: &csi.VolumeCapability_AccessMode{Mode: csi.VolumeCapability_AccessMode_UNKNOWN},
	}
}

func nodeTopology(node string) *csi.Topology {
	return &csi.Topology{Segments: map[string]string{TopologyKeyNode: node}}
}

// capacityRequest builds a GetCapacity request like the one sent by the
// capacity controller of the external-provisioner on the test node.
func capacityRequest(volumeType, pool string) *csi.GetCapacityRequest {
	return &csi.GetCapacityRequest{
		Parameters:         volumeParams(volumeType, pool),
		VolumeCapabilities: []*csi.VolumeCapability{provisionerCapacityCap()},
		AccessibleTopology: nodeTopology(testNodeID),
	}
}

// getCapacity calls GetCapacity and checks the invariants which every
// response must satisfy.
func (d *testDriver) getCapacity(t *testing.T, req *csi.GetCapacityRequest) (available, maximum int64) {
	t.Helper()
	resp, err := d.GetCapacity(context.Background(), req)
	require.NoError(t, err)
	require.NotNil(t, resp.GetMaximumVolumeSize(), "maximum volume size must be reported")

	available = resp.GetAvailableCapacity()
	maximum = resp.GetMaximumVolumeSize().GetValue()
	require.GreaterOrEqual(t, available, int64(0), "available capacity must not be negative")
	require.GreaterOrEqual(t, maximum, int64(0), "maximum volume size must not be negative")
	require.LessOrEqual(t, maximum, available, "maximum volume size must not exceed the available capacity")
	require.LessOrEqual(t, maximum, d.config.MaxVolumeSize, "maximum volume size must not exceed --max-volume-size")
	require.Equal(t, int64(0), resp.GetMinimumVolumeSize().GetValue())

	return available, maximum
}

func (d *testDriver) requireAvailable(t *testing.T, volumeType, pool string, expected int64) {
	t.Helper()
	available, _ := d.getCapacity(t, capacityRequest(volumeType, pool))
	require.Equal(t, expected, available, "available capacity of pool %q for %s", pool, volumeType)
}

func (d *testDriver) mustCreate(t *testing.T, req *csi.CreateVolumeRequest) *csi.Volume {
	t.Helper()
	resp, err := d.CreateVolume(context.Background(), req)
	require.NoError(t, err)
	require.NotNil(t, resp.GetVolume())
	return resp.GetVolume()
}

func (d *testDriver) mustDelete(t *testing.T, volumeID string) {
	t.Helper()
	_, err := d.DeleteVolume(context.Background(), &csi.DeleteVolumeRequest{VolumeId: volumeID})
	require.NoError(t, err)
}

func requireCode(t *testing.T, err error, code codes.Code) {
	t.Helper()
	require.Error(t, err)
	require.Equal(t, code, status.Code(err), "unexpected error: %v", err)
}

func (f *fakeSpdk) called(method, bdevName string) bool {
	for _, c := range f.calls {
		if c.method == method && c.params["name"] == bdevName {
			return true
		}
	}
	return false
}

// requireNoBackend checks that no backend resource exists for a volume
// name: no state entry, no SPDK backing file and no SPDK bdev.
func (d *testDriver) requireNoBackend(t *testing.T, name string) {
	t.Helper()
	_, err := d.state.GetVolumeByName(name)
	require.Error(t, err, "volume %q must not be recorded", name)
	_, err = os.Stat(filepath.Join(utils.SpdkRawDiskDir, name+".raw"))
	require.True(t, os.IsNotExist(err), "backing file of %q must not exist", name)
	require.False(t, d.spdk.called("bdev_aio_create", "bdev-"+name), "bdev of %q must not be created", name)
}

func reservedVolume(id, pool string, size int64) state.Volume {
	return state.Volume{VolID: id, VolName: id, VolSize: size, Kind: pool}
}

func TestGetCapacity(t *testing.T) {
	testCases := []struct {
		name          string
		capacity      []string
		maxVolumeSize int64
		free          int64
		volumes       []state.Volume
		req           func(volumeType string) *csi.GetCapacityRequest
		available     int64
		maximum       int64
		errCode       codes.Code
	}{
		{
			name:      "no pools, filesystem free space",
			req:       func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "") },
			available: testFreeSpace,
			maximum:   testFreeSpace,
		},
		{
			name:          "no pools, maximum volume size",
			maxVolumeSize: 10 * utils.GiB,
			req:           func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "") },
			available:     testFreeSpace,
			maximum:       10 * utils.GiB,
		},
		{
			name: "no pools, pool parameter",
			req:  func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "fast") },
		},
		{
			name:      "pool before any volume",
			capacity:  []string{"fast=8Gi"},
			req:       func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "fast") },
			available: 8 * utils.GiB,
			maximum:   8 * utils.GiB,
		},
		{
			name:      "pool with reservations",
			capacity:  []string{"fast=8Gi", "slow=8Gi"},
			volumes:   []state.Volume{reservedVolume("v1", "fast", 3*utils.GiB), reservedVolume("v2", "slow", 4*utils.GiB)},
			req:       func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "fast") },
			available: 5 * utils.GiB,
			maximum:   5 * utils.GiB,
		},
		{
			name:          "pool larger than maximum volume size",
			capacity:      []string{"fast=8Gi"},
			maxVolumeSize: 2 * utils.GiB,
			req:           func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "fast") },
			available:     8 * utils.GiB,
			maximum:       2 * utils.GiB,
		},
		{
			name:     "exhausted pool",
			capacity: []string{"fast=8Gi"},
			volumes:  []state.Volume{reservedVolume("v1", "fast", 8*utils.GiB)},
			req:      func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "fast") },
		},
		{
			name:     "overcommitted pool",
			capacity: []string{"fast=2Gi"},
			volumes:  []state.Volume{reservedVolume("v1", "fast", 3*utils.GiB)},
			req:      func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "fast") },
		},
		{
			name:     "missing pool parameter",
			capacity: []string{"fast=8Gi"},
			req:      func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "") },
		},
		{
			name:     "unknown pool",
			capacity: []string{"fast=8Gi"},
			req:      func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "slow") },
		},
		{
			name:      "filesystem limits the pool",
			capacity:  []string{"fast=8Gi"},
			free:      5 * utils.GiB,
			req:       func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "fast") },
			available: 5 * utils.GiB,
			maximum:   5 * utils.GiB,
		},
		{
			name:     "full filesystem",
			capacity: []string{"fast=8Gi"},
			free:     -1,
			req:      func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "fast") },
		},
		{
			name:     "topology of another node",
			capacity: []string{"fast=8Gi"},
			req: func(vt string) *csi.GetCapacityRequest {
				req := capacityRequest(vt, "fast")
				req.AccessibleTopology = nodeTopology(otherNodeID)
				return req
			},
		},
		{
			name:     "topology with an unknown segment",
			capacity: []string{"fast=8Gi"},
			req: func(vt string) *csi.GetCapacityRequest {
				req := capacityRequest(vt, "fast")
				req.AccessibleTopology.Segments["topology.kubernetes.io/zone"] = "zone-1"
				return req
			},
		},
		{
			name:     "no topology",
			capacity: []string{"fast=8Gi"},
			req: func(vt string) *csi.GetCapacityRequest {
				req := capacityRequest(vt, "fast")
				req.AccessibleTopology = nil
				return req
			},
			available: 8 * utils.GiB,
			maximum:   8 * utils.GiB,
		},
		{
			name:     "no volume capabilities",
			capacity: []string{"fast=8Gi"},
			req: func(vt string) *csi.GetCapacityRequest {
				req := capacityRequest(vt, "fast")
				req.VolumeCapabilities = nil
				return req
			},
			available: 8 * utils.GiB,
			maximum:   8 * utils.GiB,
		},
		{
			name:     "single node writer",
			capacity: []string{"fast=8Gi"},
			req: func(vt string) *csi.GetCapacityRequest {
				req := capacityRequest(vt, "fast")
				req.VolumeCapabilities = []*csi.VolumeCapability{singleNodeWriterCap()}
				return req
			},
			available: 8 * utils.GiB,
			maximum:   8 * utils.GiB,
		},
		{
			name:     "block access type",
			capacity: []string{"fast=8Gi"},
			req: func(vt string) *csi.GetCapacityRequest {
				req := capacityRequest(vt, "fast")
				req.VolumeCapabilities = []*csi.VolumeCapability{{
					AccessType: &csi.VolumeCapability_Block{Block: &csi.VolumeCapability_BlockVolume{}},
					AccessMode: &csi.VolumeCapability_AccessMode{Mode: csi.VolumeCapability_AccessMode_SINGLE_NODE_WRITER},
				}}
				return req
			},
		},
		{
			name:     "multi node access mode",
			capacity: []string{"fast=8Gi"},
			req: func(vt string) *csi.GetCapacityRequest {
				req := capacityRequest(vt, "fast")
				req.VolumeCapabilities = []*csi.VolumeCapability{singleNodeWriterCap(), {
					AccessType: &csi.VolumeCapability_Mount{},
					AccessMode: &csi.VolumeCapability_AccessMode{Mode: csi.VolumeCapability_AccessMode_MULTI_NODE_MULTI_WRITER},
				}}
				return req
			},
		},
		{
			name:     "legacy kind parameter",
			capacity: []string{"fast=8Gi"},
			volumes:  []state.Volume{reservedVolume("v1", "fast", 1*utils.GiB)},
			req: func(vt string) *csi.GetCapacityRequest {
				req := capacityRequest(vt, "")
				req.Parameters[legacyCapacityPoolParameter] = "fast"
				return req
			},
			available: 7 * utils.GiB,
			maximum:   7 * utils.GiB,
		},
		{
			name:     "malformed pool parameter",
			capacity: []string{"fast=8Gi"},
			req:      func(vt string) *csi.GetCapacityRequest { return capacityRequest(vt, "fast pool") },
			errCode:  codes.InvalidArgument,
		},
		{
			name:     "conflicting pool parameters",
			capacity: []string{"fast=8Gi", "slow=8Gi"},
			req: func(vt string) *csi.GetCapacityRequest {
				req := capacityRequest(vt, "fast")
				req.Parameters[legacyCapacityPoolParameter] = "slow"
				return req
			},
			errCode: codes.InvalidArgument,
		},
	}

	for _, volumeType := range volumeTypes {
		for _, tc := range testCases {
			t.Run(volumeType+"/"+tc.name, func(t *testing.T) {
				d := newTestDriver(t, testDriverOptions{capacity: tc.capacity, maxVolumeSize: tc.maxVolumeSize})
				if tc.free != 0 {
					d.storage.free[d.backingDir(volumeType)] = tc.free
				}
				for _, vol := range tc.volumes {
					if volumeType == utils.SpdkVolumeTypeName {
						vol.Metadata = map[string]string{
							"type":        utils.SpdkVolumeTypeName,
							"backingFile": filepath.Join(d.config.SpdkRawPath, vol.VolName+".raw"),
						}
					}
					// Fully allocated, so that only the pool limits these cases.
					d.storage.allocated[d.backingFile(vol)] = vol.VolSize
					require.NoError(t, d.state.UpdateVolume(vol))
				}

				req := tc.req(volumeType)
				if tc.errCode != codes.OK {
					_, err := d.GetCapacity(context.Background(), req)
					requireCode(t, err, tc.errCode)
					return
				}
				available, maximum := d.getCapacity(t, req)
				require.Equal(t, tc.available, available, "available capacity")
				require.Equal(t, tc.maximum, maximum, "maximum volume size")
			})
		}
	}
}

func TestGetCapacityStatfsError(t *testing.T) {
	d := newTestDriver(t, testDriverOptions{capacity: []string{"fast=8Gi"}})
	d.storage.err = errors.New("statfs failed")
	_, err := d.GetCapacity(context.Background(), capacityRequest(utils.DirectVolumeTypeName, "fast"))
	requireCode(t, err, codes.Internal)
}

func TestControllerGetCapabilitiesReportsGetCapacity(t *testing.T) {
	d := newTestDriver(t, testDriverOptions{})
	resp, err := d.ControllerGetCapabilities(context.Background(), &csi.ControllerGetCapabilitiesRequest{})
	require.NoError(t, err)

	var rpcs []csi.ControllerServiceCapability_RPC_Type
	for _, c := range resp.GetCapabilities() {
		rpcs = append(rpcs, c.GetRpc().GetType())
	}
	require.Contains(t, rpcs, csi.ControllerServiceCapability_RPC_CREATE_DELETE_VOLUME)
	require.Contains(t, rpcs, csi.ControllerServiceCapability_RPC_GET_CAPACITY)
}

// TestCapacityAccounting follows the life cycle of volumes in two
// independent pools, for both volume types.
func TestCapacityAccounting(t *testing.T) {
	for _, volumeType := range volumeTypes {
		t.Run(volumeType, func(t *testing.T) {
			d := newTestDriver(t, testDriverOptions{capacity: []string{"fast=8Gi", "slow=4Gi"}})

			// Capacity before any volume is created.
			d.requireAvailable(t, volumeType, "fast", 8*utils.GiB)
			d.requireAvailable(t, volumeType, "slow", 4*utils.GiB)

			// Exact reduction after CreateVolume, only in the selected pool.
			vol1 := d.mustCreate(t, createRequest("vol-1", 3*utils.GiB, volumeType, "fast"))
			require.Equal(t, 3*utils.GiB, vol1.GetCapacityBytes())
			require.Equal(t, "fast", vol1.GetVolumeContext()[utils.KataContainersDirectCapacityPool])
			require.Equal(t, "3221225472", vol1.GetVolumeContext()[utils.CapabilityInBytes])
			require.Equal(t, "vol-1", vol1.GetVolumeContext()[utils.DirectVolumeName])
			require.Equal(t, volumeType, vol1.GetVolumeContext()[utils.KataContainersDirectVolumeType])
			require.Equal(t, []*csi.Topology{nodeTopology(testNodeID)}, vol1.GetAccessibleTopology())
			d.requireAvailable(t, volumeType, "fast", 5*utils.GiB)
			d.requireAvailable(t, volumeType, "slow", 4*utils.GiB)

			persisted, err := d.state.GetVolumeByID(vol1.GetVolumeId())
			require.NoError(t, err)
			require.Equal(t, "fast", persisted.Kind)
			require.Equal(t, vol1.GetVolumeContext(), persisted.VolumeContext)

			// A repeated CreateVolume returns the same volume and does not
			// reserve capacity twice.
			again := d.mustCreate(t, createRequest("vol-1", 3*utils.GiB, volumeType, "fast"))
			require.Equal(t, vol1.GetVolumeId(), again.GetVolumeId())
			require.Equal(t, vol1.GetCapacityBytes(), again.GetCapacityBytes())
			require.Equal(t, vol1.GetVolumeContext(), again.GetVolumeContext())
			d.requireAvailable(t, volumeType, "fast", 5*utils.GiB)

			// Exhaustion: nothing is created when the pool is too small.
			_, err = d.CreateVolume(context.Background(), createRequest("vol-too-big", 5*utils.GiB+1, volumeType, "fast"))
			requireCode(t, err, codes.ResourceExhausted)
			d.requireNoBackend(t, "vol-too-big")
			d.requireAvailable(t, volumeType, "fast", 5*utils.GiB)

			// The remaining capacity can be used up exactly.
			vol2 := d.mustCreate(t, createRequest("vol-2", 5*utils.GiB, volumeType, "fast"))
			d.requireAvailable(t, volumeType, "fast", 0)
			vol3 := d.mustCreate(t, createRequest("vol-3", 4*utils.GiB, volumeType, "slow"))
			d.requireAvailable(t, volumeType, "slow", 0)

			// Reservations are restored from the state after a restart.
			d = d.restart(t)
			d.requireAvailable(t, volumeType, "fast", 0)
			d.requireAvailable(t, volumeType, "slow", 0)
			again = d.mustCreate(t, createRequest("vol-2", 5*utils.GiB, volumeType, "fast"))
			require.Equal(t, vol2.GetVolumeId(), again.GetVolumeId())
			d.requireAvailable(t, volumeType, "fast", 0)

			// DeleteVolume releases the reservation, also when repeated.
			d.mustDelete(t, vol1.GetVolumeId())
			d.requireAvailable(t, volumeType, "fast", 3*utils.GiB)
			d.mustDelete(t, vol1.GetVolumeId())
			d.requireAvailable(t, volumeType, "fast", 3*utils.GiB)

			d = d.restart(t)
			d.requireAvailable(t, volumeType, "fast", 3*utils.GiB)
			d.mustDelete(t, vol2.GetVolumeId())
			d.mustDelete(t, vol3.GetVolumeId())
			d.requireAvailable(t, volumeType, "fast", 8*utils.GiB)
			d.requireAvailable(t, volumeType, "slow", 4*utils.GiB)
			require.Empty(t, d.state.GetVolumes())
		})
	}
}

// TestCreateVolumeEnforcement checks that both volume types reject the same
// requests with the same codes, before creating any backend resource.
func TestCreateVolumeEnforcement(t *testing.T) {
	testCases := []struct {
		name          string
		capacity      []string
		maxVolumeSize int64
		free          int64
		reserved      int64
		modify        func(req *csi.CreateVolumeRequest)
		errCode       codes.Code
	}{
		{
			name:          "larger than the maximum volume size",
			capacity:      []string{"fast=8Gi"},
			maxVolumeSize: 2 * utils.GiB,
			errCode:       codes.OutOfRange,
		},
		{
			name:     "larger than the remaining pool capacity",
			capacity: []string{"fast=8Gi"},
			reserved: 6 * utils.GiB,
			errCode:  codes.ResourceExhausted,
		},
		{
			name:     "larger than the free filesystem space",
			capacity: []string{"fast=8Gi"},
			free:     2 * utils.GiB,
			errCode:  codes.ResourceExhausted,
		},
		{
			name: "larger than the free filesystem space without pools",
			free: 2 * utils.GiB,
			modify: func(req *csi.CreateVolumeRequest) {
				delete(req.Parameters, utils.KataContainersDirectCapacityPool)
			},
			errCode: codes.ResourceExhausted,
		},
		{
			name:     "missing pool",
			capacity: []string{"fast=8Gi"},
			modify: func(req *csi.CreateVolumeRequest) {
				delete(req.Parameters, utils.KataContainersDirectCapacityPool)
			},
			errCode: codes.ResourceExhausted,
		},
		{
			name:     "unknown pool",
			capacity: []string{"slow=8Gi"},
			errCode:  codes.ResourceExhausted,
		},
		{
			name:    "pool without configured pools",
			errCode: codes.ResourceExhausted,
		},
		{
			name:     "malformed pool",
			capacity: []string{"fast=8Gi"},
			modify: func(req *csi.CreateVolumeRequest) {
				req.Parameters[utils.KataContainersDirectCapacityPool] = "-fast"
			},
			errCode: codes.InvalidArgument,
		},
		{
			name:     "requisite topology of another node",
			capacity: []string{"fast=8Gi"},
			modify: func(req *csi.CreateVolumeRequest) {
				req.AccessibilityRequirements = &csi.TopologyRequirement{
					Requisite: []*csi.Topology{nodeTopology(otherNodeID)},
					Preferred: []*csi.Topology{nodeTopology(otherNodeID)},
				}
			},
			errCode: codes.ResourceExhausted,
		},
		{
			name:     "block access type",
			capacity: []string{"fast=8Gi"},
			modify: func(req *csi.CreateVolumeRequest) {
				req.VolumeCapabilities = []*csi.VolumeCapability{{
					AccessType: &csi.VolumeCapability_Block{Block: &csi.VolumeCapability_BlockVolume{}},
				}}
			},
			errCode: codes.InvalidArgument,
		},
		{
			name:     "multi node access mode",
			capacity: []string{"fast=8Gi"},
			modify: func(req *csi.CreateVolumeRequest) {
				req.VolumeCapabilities[0].AccessMode.Mode = csi.VolumeCapability_AccessMode_MULTI_NODE_READER_ONLY
			},
			errCode: codes.InvalidArgument,
		},
		{
			name:     "no volume capabilities",
			capacity: []string{"fast=8Gi"},
			modify: func(req *csi.CreateVolumeRequest) {
				req.VolumeCapabilities = nil
			},
			errCode: codes.InvalidArgument,
		},
		{
			name:     "required bytes above the limit",
			capacity: []string{"fast=8Gi"},
			modify: func(req *csi.CreateVolumeRequest) {
				req.CapacityRange.LimitBytes = utils.GiB
			},
			errCode: codes.InvalidArgument,
		},
		{
			name:     "negative capacity range",
			capacity: []string{"fast=8Gi"},
			modify: func(req *csi.CreateVolumeRequest) {
				req.CapacityRange.RequiredBytes = -1
			},
			errCode: codes.InvalidArgument,
		},
	}

	for _, volumeType := range volumeTypes {
		for _, tc := range testCases {
			t.Run(volumeType+"/"+tc.name, func(t *testing.T) {
				d := newTestDriver(t, testDriverOptions{capacity: tc.capacity, maxVolumeSize: tc.maxVolumeSize})
				if tc.free != 0 {
					d.storage.free[d.backingDir(volumeType)] = tc.free
				}
				if tc.reserved != 0 {
					vol := reservedVolume("reserved", "fast", tc.reserved)
					d.storage.allocated[d.backingFile(vol)] = vol.VolSize
					require.NoError(t, d.state.UpdateVolume(vol))
				}

				req := createRequest("vol-rejected", 4*utils.GiB, volumeType, "fast")
				if tc.modify != nil {
					tc.modify(req)
				}
				_, err := d.CreateVolume(context.Background(), req)
				requireCode(t, err, tc.errCode)
				d.requireNoBackend(t, "vol-rejected")
			})
		}
	}
}

func TestCreateVolumeTopology(t *testing.T) {
	testCases := []struct {
		name            string
		requirements    *csi.TopologyRequirement
		disableTopology bool
		topology        []*csi.Topology
	}{
		{
			name:     "no requirements",
			topology: []*csi.Topology{nodeTopology(testNodeID)},
		},
		{
			name: "selected node",
			requirements: &csi.TopologyRequirement{
				Requisite: []*csi.Topology{nodeTopology(testNodeID)},
				Preferred: []*csi.Topology{nodeTopology(testNodeID)},
			},
			topology: []*csi.Topology{nodeTopology(testNodeID)},
		},
		{
			name: "one of several nodes",
			requirements: &csi.TopologyRequirement{
				Requisite: []*csi.Topology{nodeTopology(otherNodeID), nodeTopology(testNodeID)},
			},
			topology: []*csi.Topology{nodeTopology(testNodeID)},
		},
		{
			name: "only preferred topologies",
			requirements: &csi.TopologyRequirement{
				Preferred: []*csi.Topology{nodeTopology(otherNodeID)},
			},
			topology: []*csi.Topology{nodeTopology(testNodeID)},
		},
		{
			name:            "topology disabled",
			disableTopology: true,
			requirements: &csi.TopologyRequirement{
				Requisite: []*csi.Topology{nodeTopology(otherNodeID)},
			},
			topology: []*csi.Topology{},
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			d := newTestDriver(t, testDriverOptions{capacity: []string{"fast=8Gi"}, disableTopology: tc.disableTopology})
			req := createRequest("vol-1", utils.GiB, utils.DirectVolumeTypeName, "fast")
			req.AccessibilityRequirements = tc.requirements
			vol := d.mustCreate(t, req)
			require.Equal(t, tc.topology, vol.GetAccessibleTopology())
		})
	}
}

func TestCreateVolumeIdempotency(t *testing.T) {
	for _, volumeType := range volumeTypes {
		t.Run(volumeType, func(t *testing.T) {
			d := newTestDriver(t, testDriverOptions{capacity: []string{"fast=8Gi", "slow=8Gi"}})
			vol := d.mustCreate(t, createRequest("vol-1", 2*utils.GiB, volumeType, "fast"))

			// A smaller request is satisfied by the existing volume.
			smaller := d.mustCreate(t, createRequest("vol-1", utils.GiB, volumeType, "fast"))
			require.Equal(t, vol.GetVolumeId(), smaller.GetVolumeId())
			require.Equal(t, 2*utils.GiB, smaller.GetCapacityBytes())
			d.requireAvailable(t, volumeType, "fast", 6*utils.GiB)

			incompatible := map[string]*csi.CreateVolumeRequest{
				"larger":        createRequest("vol-1", 3*utils.GiB, volumeType, "fast"),
				"other pool":    createRequest("vol-1", 2*utils.GiB, volumeType, "slow"),
				"other fs type": createRequest("vol-1", 2*utils.GiB, volumeType, "fast"),
				"below limit":   createRequest("vol-1", utils.GiB, volumeType, "fast"),
				"other node":    createRequest("vol-1", 2*utils.GiB, volumeType, "fast"),
			}
			incompatible["other fs type"].Parameters[utils.KataContainersDirectFsType] = "xfs"
			incompatible["below limit"].CapacityRange.LimitBytes = utils.GiB
			incompatible["other node"].AccessibilityRequirements = &csi.TopologyRequirement{
				Requisite: []*csi.Topology{nodeTopology(otherNodeID)},
			}
			for name, req := range incompatible {
				_, err := d.CreateVolume(context.Background(), req)
				requireCode(t, err, codes.AlreadyExists)
				require.Len(t, d.state.GetVolumes(), 1, name)
			}
			d.requireAvailable(t, volumeType, "fast", 6*utils.GiB)
			d.requireAvailable(t, volumeType, "slow", 8*utils.GiB)
		})
	}
}

// TestCreateVolumeIdempotencyLegacyState checks the response for a volume
// which an older driver recorded without its volume context.
func TestCreateVolumeIdempotencyLegacyState(t *testing.T) {
	d := newTestDriver(t, testDriverOptions{})
	require.NoError(t, d.state.UpdateVolume(state.Volume{VolID: "legacy-id", VolName: "vol-1", VolSize: utils.GiB}))

	vol := d.mustCreate(t, createRequest("vol-1", utils.GiB, utils.DirectVolumeTypeName, ""))
	require.Equal(t, "legacy-id", vol.GetVolumeId())
	require.Equal(t, map[string]string{
		utils.IsDirectVolume:                 "True",
		utils.KataContainersDirectVolumeType: utils.DirectVolumeTypeName,
		utils.KataContainersDirectFsType:     "ext4",
		utils.DirectVolumeName:               "vol-1",
		utils.CapabilityInBytes:              "1073741824",
	}, vol.GetVolumeContext())
}

func TestFilesystemCapacity(t *testing.T) {
	const free = 20 * utils.GiB

	testCases := []struct {
		name      string
		sameFS    bool
		directvol int64
		spdkvol   int64
	}{
		// directvol: 2Gi not staged yet; spdkvol: 7Gi of a sparse file not written yet.
		{name: "separate filesystems", directvol: free - 2*utils.GiB, spdkvol: free - 7*utils.GiB},
		{name: "shared filesystem", sameFS: true, directvol: free - 9*utils.GiB, spdkvol: free - 9*utils.GiB},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			d := newTestDriver(t, testDriverOptions{})
			d.storage.free[d.config.StoragePath] = free
			d.storage.free[d.config.SpdkRawPath] = free
			d.storage.device[d.config.StoragePath] = 1
			d.storage.device[d.config.SpdkRawPath] = 2
			if tc.sameFS {
				d.storage.device[d.config.SpdkRawPath] = 1
			}

			staged := state.Volume{VolID: "staged", VolName: "staged", VolSize: 4 * utils.GiB}
			unstaged := state.Volume{VolID: "unstaged", VolName: "unstaged", VolSize: 2 * utils.GiB}
			sparse := state.Volume{
				VolID:    "sparse",
				VolName:  "sparse",
				VolSize:  8 * utils.GiB,
				Metadata: map[string]string{"type": utils.SpdkVolumeTypeName, "backingFile": "/spdk/sparse.raw"},
			}
			d.storage.allocated[d.backingFile(staged)] = staged.VolSize
			d.storage.allocated[d.backingFile(sparse)] = utils.GiB
			for _, vol := range []state.Volume{staged, unstaged, sparse} {
				require.NoError(t, d.state.UpdateVolume(vol))
			}

			d.requireAvailable(t, utils.DirectVolumeTypeName, "", tc.directvol)
			d.requireAvailable(t, utils.SpdkVolumeTypeName, "", tc.spdkvol)

			// Writing to the sparse file moves bytes from the reservation to
			// the filesystem usage without changing the result.
			d.storage.allocated[d.backingFile(sparse)] += utils.GiB
			d.storage.free[d.config.SpdkRawPath] -= utils.GiB
			if tc.sameFS {
				d.storage.free[d.config.StoragePath] -= utils.GiB
			}
			d.requireAvailable(t, utils.DirectVolumeTypeName, "", tc.directvol)
			d.requireAvailable(t, utils.SpdkVolumeTypeName, "", tc.spdkvol)
		})
	}
}

// TestFilesystemCapacityAdmission checks that CreateVolume counts the
// unallocated part of earlier reservations against the free space.
func TestFilesystemCapacityAdmission(t *testing.T) {
	for _, volumeType := range volumeTypes {
		t.Run(volumeType, func(t *testing.T) {
			d := newTestDriver(t, testDriverOptions{})
			d.storage.free[d.backingDir(volumeType)] = 10 * utils.GiB

			d.mustCreate(t, createRequest("vol-1", 6*utils.GiB, volumeType, ""))
			d.requireAvailable(t, volumeType, "", 4*utils.GiB)

			_, err := d.CreateVolume(context.Background(), createRequest("vol-2", 6*utils.GiB, volumeType, ""))
			requireCode(t, err, codes.ResourceExhausted)
			d.requireNoBackend(t, "vol-2")
		})
	}
}

func TestSPDKCreateRollback(t *testing.T) {
	d := newTestDriver(t, testDriverOptions{capacity: []string{"fast=8Gi"}})
	d.spdk.err["bdev_aio_create"] = errors.New("bdev_aio_create failed")

	_, err := d.CreateVolume(context.Background(), createRequest("vol-1", utils.GiB, utils.SpdkVolumeTypeName, "fast"))
	requireCode(t, err, codes.Internal)
	_, statErr := os.Stat(filepath.Join(utils.SpdkRawDiskDir, "vol-1.raw"))
	require.True(t, os.IsNotExist(statErr), "backing file must be removed after a failed bdev_aio_create")
	require.Empty(t, d.state.GetVolumes())
	d.requireAvailable(t, utils.SpdkVolumeTypeName, "fast", 8*utils.GiB)

	// A later attempt succeeds and reserves the capacity once.
	delete(d.spdk.err, "bdev_aio_create")
	d.mustCreate(t, createRequest("vol-1", utils.GiB, utils.SpdkVolumeTypeName, "fast"))
	d.requireAvailable(t, utils.SpdkVolumeTypeName, "fast", 7*utils.GiB)
}

func TestSPDKStaleBackingFile(t *testing.T) {
	d := newTestDriver(t, testDriverOptions{capacity: []string{"fast=8Gi"}})
	stale := filepath.Join(utils.SpdkRawDiskDir, "vol-1.raw")
	require.NoError(t, os.WriteFile(stale, []byte("stale"), 0600))

	_, err := d.CreateVolume(context.Background(), createRequest("vol-1", utils.GiB, utils.SpdkVolumeTypeName, "fast"))
	requireCode(t, err, codes.Internal)
	require.False(t, d.spdk.called("bdev_aio_create", "bdev-vol-1"))
	content, err := os.ReadFile(stale)
	require.NoError(t, err)
	require.Equal(t, "stale", string(content), "a file which was not created by this request must be kept")
}

func TestValidateVolumeCapabilities(t *testing.T) {
	d := newTestDriver(t, testDriverOptions{})
	vol := d.mustCreate(t, createRequest("vol-1", utils.GiB, utils.DirectVolumeTypeName, ""))

	resp, err := d.ValidateVolumeCapabilities(context.Background(), &csi.ValidateVolumeCapabilitiesRequest{
		VolumeId:           vol.GetVolumeId(),
		VolumeCapabilities: []*csi.VolumeCapability{singleNodeWriterCap()},
	})
	require.NoError(t, err)
	require.NotNil(t, resp.GetConfirmed())

	resp, err = d.ValidateVolumeCapabilities(context.Background(), &csi.ValidateVolumeCapabilitiesRequest{
		VolumeId: vol.GetVolumeId(),
		VolumeCapabilities: []*csi.VolumeCapability{{
			AccessType: &csi.VolumeCapability_Mount{},
			AccessMode: &csi.VolumeCapability_AccessMode{Mode: csi.VolumeCapability_AccessMode_MULTI_NODE_MULTI_WRITER},
		}},
	})
	require.NoError(t, err)
	require.Nil(t, resp.GetConfirmed())
	require.Contains(t, resp.GetMessage(), "not supported")
}

func TestNodeCapacityConfig(t *testing.T) {
	files := map[string]string{
		testNodeID:  "default=2Gi",
		otherNodeID: "default=8Gi\nfast=1Gi",
	}
	testCases := []struct {
		node     string
		capacity []string
		expected string
	}{
		{node: testNodeID, capacity: []string{"default=4Gi", "slow=1Ti"}, expected: "default=2Gi,slow=1Ti"},
		{node: otherNodeID, expected: "default=8Gi,fast=1Gi"},
		{node: "node-c", capacity: []string{"default=4Gi"}, expected: "default=4Gi"},
	}

	for _, tc := range testCases {
		t.Run(tc.node, func(t *testing.T) {
			d := newTestDriver(t, testDriverOptions{nodeID: tc.node, capacity: tc.capacity, capacityFiles: files})
			require.Equal(t, tc.expected, d.config.Capacity.String())
		})
	}

	t.Run("invalid file", func(t *testing.T) {
		tmp := t.TempDir()
		require.NoError(t, os.WriteFile(filepath.Join(tmp, testNodeID), []byte("default=-1Gi"), 0600))
		_, err := NewDirectVolumeDriver(Config{
			DriverName:        "directvolume.csi.katacontainers.io",
			Endpoint:          "unix://" + filepath.Join(tmp, "csi.sock"),
			NodeID:            testNodeID,
			StoragePath:       filepath.Join(tmp, "storage"),
			StateDir:          filepath.Join(tmp, "state"),
			SpdkRawPath:       filepath.Join(tmp, "spdk-raw"),
			SpdkVhostPath:     filepath.Join(tmp, "spdk-vhost"),
			CapacityConfigDir: tmp,
		})
		require.ErrorContains(t, err, "must not be negative")
	})
}

func TestAdoptVolume(t *testing.T) {
	d := newTestDriver(t, testDriverOptions{capacity: []string{"fast=4Gi"}})
	vol := d.mustCreate(t, createRequest("vol-1", 3*utils.GiB, utils.DirectVolumeTypeName, "fast"))
	volumeCtx := vol.GetVolumeContext()

	// The state loses the volume and its space is handed out again.
	require.NoError(t, d.state.DeleteVolume(vol.GetVolumeId()))
	d.mustCreate(t, createRequest("vol-2", 2*utils.GiB, utils.DirectVolumeTypeName, "fast"))
	d.requireAvailable(t, utils.DirectVolumeTypeName, "fast", 2*utils.GiB)

	// The node service records the volume again when it is staged, even
	// though that overcommits the pool, and no more space is advertised.
	adopted, err := d.adoptVolume(vol.GetVolumeId(), volumeCtx)
	require.NoError(t, err)
	require.Equal(t, "fast", adopted.Kind)
	require.Equal(t, 3*utils.GiB, adopted.VolSize)
	require.Equal(t, "vol-1", adopted.VolName)
	d.requireAvailable(t, utils.DirectVolumeTypeName, "fast", 0)

	_, err = d.adoptVolume("broken", map[string]string{utils.CapabilityInBytes: "lots"})
	requireCode(t, err, codes.InvalidArgument)
}

func TestPublishDevicePathAfterRestart(t *testing.T) {
	d := newTestDriver(t, testDriverOptions{})
	vol := d.mustCreate(t, createRequest("vol-1", 64*utils.MiB, utils.DirectVolumeTypeName, ""))
	volumeCtx := vol.GetVolumeContext()

	// Not staged yet: there is no raw disk to publish.
	_, err := d.publishDevicePath(vol.GetVolumeId(), volumeCtx)
	requireCode(t, err, codes.FailedPrecondition)

	// Staging creates the raw disk and remembers it in memory only.
	rawDisk := utils.DirectBlockDevicePath(d.config.StoragePath, vol.GetVolumeId(), 64*utils.MiB)
	require.NoError(t, os.MkdirAll(filepath.Dir(rawDisk), 0750))
	require.NoError(t, os.WriteFile(rawDisk, nil, 0600))

	d = d.restart(t)
	require.Empty(t, d.config.VolumeDevices)
	devicePath, err := d.publishDevicePath(vol.GetVolumeId(), volumeCtx)
	require.NoError(t, err)
	require.Equal(t, rawDisk, devicePath)
	require.Equal(t, rawDisk, d.config.VolumeDevices[vol.GetVolumeId()])
}
