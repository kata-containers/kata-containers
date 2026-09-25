// Copyright (c) 2026 NAVER Cloud Corp.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/container-orchestrated-devices/container-device-interface/pkg/cdi"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	podresourcesv1 "k8s.io/kubelet/pkg/apis/podresources/v1"
)

// setCDISpecDir disables the registry's auto-refresh: racing the fsnotify watch against the explicit Refresh() makes lookups flaky.
func setCDISpecDir(t *testing.T, dir string) {
	t.Helper()
	cdi.GetRegistry(cdi.WithSpecDirs(dir), cdi.WithAutoRefresh(false))
}

func writeCDISpec(t *testing.T, dir, name, kind string, devices map[string]string) {
	t.Helper()

	content := "cdiVersion: \"0.5.0\"\nkind: \"" + kind + "\"\ndevices:\n"
	for devName, path := range devices {
		content += "  - name: \"" + devName + "\"\n" +
			"    containerEdits:\n" +
			"      deviceNodes:\n" +
			"      - path: \"" + path + "\"\n"
	}

	err := os.WriteFile(filepath.Join(dir, name+".yaml"), []byte(content), 0o644)
	require.NoError(t, err)
}

// writeCDISpecEnvOnly writes a CDI device that resolves but declares no device
// nodes (only an env edit), to exercise the node-less exemption.
func writeCDISpecEnvOnly(t *testing.T, dir, name, kind, devName string) {
	t.Helper()

	content := "cdiVersion: \"0.5.0\"\nkind: \"" + kind + "\"\ndevices:\n" +
		"  - name: \"" + devName + "\"\n" +
		"    containerEdits:\n" +
		"      env:\n" +
		"      - \"FOO=bar\"\n"

	err := os.WriteFile(filepath.Join(dir, name+".yaml"), []byte(content), 0o644)
	require.NoError(t, err)
}

func withSysDevRoots(t *testing.T, sys, dev string) {
	t.Helper()
	origSys, origDev := sysRoot, devRoot
	sysRoot, devRoot = sys, dev
	t.Cleanup(func() {
		sysRoot, devRoot = origSys, origDev
	})
}

func buildVFIODeviceCdevFixture(t *testing.T, sysDir, vfioName, bdf string) {
	t.Helper()

	linkDir := filepath.Join(sysDir, "class", "vfio-dev", vfioName)
	require.NoError(t, os.MkdirAll(linkDir, 0o755))

	targetDir := filepath.Join(sysDir, "devices", "pci0000:00", bdf)
	require.NoError(t, os.MkdirAll(targetDir, 0o755))

	rel, err := filepath.Rel(linkDir, targetDir)
	require.NoError(t, err)
	require.NoError(t, os.Symlink(rel, filepath.Join(linkDir, "device")))
}

// buildVFIOMdevCdevFixture nests the mdev UUID under the parent BDF, mirroring real sysfs; the UUID basename is the device identity.
func buildVFIOMdevCdevFixture(t *testing.T, sysDir, vfioName, parentBDF, mdevUUID string) {
	t.Helper()

	linkDir := filepath.Join(sysDir, "class", "vfio-dev", vfioName)
	require.NoError(t, os.MkdirAll(linkDir, 0o755))

	targetDir := filepath.Join(sysDir, "devices", "pci0000:00", parentBDF, mdevUUID)
	require.NoError(t, os.MkdirAll(targetDir, 0o755))

	rel, err := filepath.Rel(linkDir, targetDir)
	require.NoError(t, err)
	require.NoError(t, os.Symlink(rel, filepath.Join(linkDir, "device")))
}

func buildIOMMUGroupFixture(t *testing.T, sysDir, group string, bdfs ...string) {
	t.Helper()

	devicesDir := filepath.Join(sysDir, "kernel", "iommu_groups", group, "devices")
	require.NoError(t, os.MkdirAll(devicesDir, 0o755))

	for _, bdf := range bdfs {
		require.NoError(t, os.WriteFile(filepath.Join(devicesDir, bdf), []byte{}, 0o644))
	}
}

func TestNormalizeDeviceNodePath(t *testing.T) {
	assert := assert.New(t)
	sysDir := t.TempDir()
	devDir := t.TempDir()
	withSysDevRoots(t, sysDir, devDir)

	t.Run("vfio device cdev resolves to BDF", func(t *testing.T) {
		buildVFIODeviceCdevFixture(t, sysDir, "vfio0", "0000:65:00.0")

		coords, err := normalizeDeviceNodePath(filepath.Join(devDir, "vfio", "devices", "vfio0"))
		assert.NoError(err)
		assert.Equal([]string{"0000:65:00.0"}, coords)
	})

	t.Run("legacy iommu group cdev resolves to member BDFs", func(t *testing.T) {
		buildIOMMUGroupFixture(t, sysDir, "42", "0000:65:00.0", "0000:65:00.1")

		coords, err := normalizeDeviceNodePath(filepath.Join(devDir, "vfio", "42"))
		assert.NoError(err)
		assert.ElementsMatch([]string{"0000:65:00.0", "0000:65:00.1"}, coords)
	})

	t.Run("unrelated path falls back to the path itself", func(t *testing.T) {
		coords, err := normalizeDeviceNodePath("/dev/nvidia0")
		assert.NoError(err)
		assert.Equal([]string{"/dev/nvidia0"}, coords)
	})

	t.Run("vfio device cdev backing an mdev resolves to the mdev instance UUID, not the parent BDF", func(t *testing.T) {
		const parentBDF = "0000:65:00.0"
		const mdevUUID = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
		buildVFIOMdevCdevFixture(t, sysDir, "vfio-mdev0", parentBDF, mdevUUID)

		coords, err := normalizeDeviceNodePath(filepath.Join(devDir, "vfio", "devices", "vfio-mdev0"))
		assert.NoError(err)
		assert.Equal([]string{mdevUUID}, coords)
	})

	t.Run("vfio device cdev with unresolvable symlink errors", func(t *testing.T) {
		_, err := normalizeDeviceNodePath(filepath.Join(devDir, "vfio", "devices", "vfio-missing"))
		assert.Error(err)
	})
}

func TestCheckCrossSourcePhysicalOverlap(t *testing.T) {
	sysDir := t.TempDir()
	devDir := t.TempDir()
	specDir := t.TempDir()
	withSysDevRoots(t, sysDir, devDir)
	setCDISpecDir(t, specDir)

	t.Run("no overlap when sets are disjoint", func(t *testing.T) {
		assert := assert.New(t)
		buildVFIODeviceCdevFixture(t, sysDir, "vfio0", "0000:65:00.0")
		buildVFIODeviceCdevFixture(t, sysDir, "vfio1", "0000:66:00.0")

		writeCDISpec(t, specDir, "dp", "vendor.com/gpu", map[string]string{
			"gpu0": filepath.Join(devDir, "vfio", "devices", "vfio0"),
		})
		writeCDISpec(t, specDir, "dra", "vendor.com/dra", map[string]string{
			"gpu1": filepath.Join(devDir, "vfio", "devices", "vfio1"),
		})
		cdi.GetRegistry().Refresh()

		err := checkCrossSourcePhysicalOverlap(
			[]string{"vendor.com/gpu=gpu0"},
			[]string{"vendor.com/dra=gpu1"},
		)
		assert.NoError(err)
	})

	t.Run("direct overlap: same BDF via vfio device cdev on both sides", func(t *testing.T) {
		assert := assert.New(t)
		buildVFIODeviceCdevFixture(t, sysDir, "vfio2", "0000:70:00.0")

		writeCDISpec(t, specDir, "dp2", "vendor.com/gpu", map[string]string{
			"gpu2": filepath.Join(devDir, "vfio", "devices", "vfio2"),
		})
		writeCDISpec(t, specDir, "dra2", "vendor.com/dra", map[string]string{
			"gpu2": filepath.Join(devDir, "vfio", "devices", "vfio2"),
		})
		cdi.GetRegistry().Refresh()

		err := checkCrossSourcePhysicalOverlap(
			[]string{"vendor.com/gpu=gpu2"},
			[]string{"vendor.com/dra=gpu2"},
		)
		assert.Error(err)
		assert.Contains(err.Error(), "0000:70:00.0")
	})

	// Key regression: the same BDF via a legacy iommu-group cdev on one side
	// and a per-device vfio cdev on the other; a path-string comparison misses it.
	t.Run("aliased overlap: legacy group cdev vs vfio device cdev for the same BDF", func(t *testing.T) {
		assert := assert.New(t)

		const bdf = "0000:99:00.0"
		buildVFIODeviceCdevFixture(t, sysDir, "vfio9", bdf)
		buildIOMMUGroupFixture(t, sysDir, "9", bdf)

		writeCDISpec(t, specDir, "dp3", "vendor.com/gpu", map[string]string{
			"gpu9": filepath.Join(devDir, "vfio", "9"),
		})
		writeCDISpec(t, specDir, "dra3", "vendor.com/dra", map[string]string{
			"gpu9": filepath.Join(devDir, "vfio", "devices", "vfio9"),
		})
		cdi.GetRegistry().Refresh()

		err := checkCrossSourcePhysicalOverlap(
			[]string{"vendor.com/gpu=gpu9"},
			[]string{"vendor.com/dra=gpu9"},
		)
		assert.Error(err)
		assert.Contains(err.Error(), bdf)
	})

	t.Run("distinct mdev slices of the same parent GPU never collide", func(t *testing.T) {
		assert := assert.New(t)
		const parentBDF = "0000:88:00.0"
		const uuid1 = "11111111-1111-1111-1111-111111111111"
		const uuid2 = "22222222-2222-2222-2222-222222222222"
		buildVFIOMdevCdevFixture(t, sysDir, "vfio-mdev1", parentBDF, uuid1)
		buildVFIOMdevCdevFixture(t, sysDir, "vfio-mdev2", parentBDF, uuid2)

		writeCDISpec(t, specDir, "dp-mdev", "vendor.com/gpu", map[string]string{
			"slice1": filepath.Join(devDir, "vfio", "devices", "vfio-mdev1"),
		})
		writeCDISpec(t, specDir, "dra-mdev", "vendor.com/dra", map[string]string{
			"slice2": filepath.Join(devDir, "vfio", "devices", "vfio-mdev2"),
		})
		cdi.GetRegistry().Refresh()

		err := checkCrossSourcePhysicalOverlap(
			[]string{"vendor.com/gpu=slice1"},
			[]string{"vendor.com/dra=slice2"},
		)
		assert.NoError(err)
	})

	t.Run("same mdev instance UUID on both sides collides", func(t *testing.T) {
		assert := assert.New(t)
		const parentBDF = "0000:89:00.0"
		const uuid = "33333333-3333-3333-3333-333333333333"
		buildVFIOMdevCdevFixture(t, sysDir, "vfio-mdev3", parentBDF, uuid)

		writeCDISpec(t, specDir, "dp-mdev-same", "vendor.com/gpu", map[string]string{
			"slice3": filepath.Join(devDir, "vfio", "devices", "vfio-mdev3"),
		})
		writeCDISpec(t, specDir, "dra-mdev-same", "vendor.com/dra", map[string]string{
			"slice3": filepath.Join(devDir, "vfio", "devices", "vfio-mdev3"),
		})
		cdi.GetRegistry().Refresh()

		err := checkCrossSourcePhysicalOverlap(
			[]string{"vendor.com/gpu=slice3"},
			[]string{"vendor.com/dra=slice3"},
		)
		assert.Error(err)
		assert.Contains(err.Error(), uuid)
	})

	t.Run("device node normalization failure fails closed instead of being skipped", func(t *testing.T) {
		assert := assert.New(t)
		// CDI-resolvable devices whose vfio-dev sysfs symlink is missing:
		// the normalization failure must propagate, not be skipped.
		writeCDISpec(t, specDir, "dp-broken", "vendor.com/gpu", map[string]string{
			"gpu-broken": filepath.Join(devDir, "vfio", "devices", "vfio-broken-missing"),
		})
		writeCDISpec(t, specDir, "dra-broken", "vendor.com/dra", map[string]string{
			"gpu-broken-2": filepath.Join(devDir, "vfio", "devices", "vfio-broken-missing-2"),
		})
		cdi.GetRegistry().Refresh()

		err := checkCrossSourcePhysicalOverlap(
			[]string{"vendor.com/gpu=gpu-broken"},
			[]string{"vendor.com/dra=gpu-broken-2"},
		)
		assert.Error(err)
	})

	t.Run("empty inputs never error", func(t *testing.T) {
		assert := assert.New(t)
		assert.NoError(checkCrossSourcePhysicalOverlap(nil, nil))
		assert.NoError(checkCrossSourcePhysicalOverlap([]string{"vendor.com/gpu=gpu0"}, nil))
		assert.NoError(checkCrossSourcePhysicalOverlap(nil, []string{"vendor.com/dra=gpu0"}))
	})

	t.Run("unresolvable CDI device names are skipped silently", func(t *testing.T) {
		assert := assert.New(t)
		err := checkCrossSourcePhysicalOverlap(
			[]string{"vendor.com/gpu=does-not-exist"},
			[]string{"vendor.com/dra=also-does-not-exist"},
		)
		assert.NoError(err)
	})
}

func TestGetDeviceSpecCrossSourceOverlapGuard(t *testing.T) {
	assert := assert.New(t)
	ctx := context.Background()

	sysDir := t.TempDir()
	devDir := t.TempDir()
	specDir := t.TempDir()
	withSysDevRoots(t, sysDir, devDir)
	setCDISpecDir(t, specDir)

	const bdf = "0000:AA:00.0"
	buildVFIODeviceCdevFixture(t, sysDir, "vfioAA", bdf)

	writeCDISpec(t, specDir, "dp", "vendor.com/gpu", map[string]string{
		"gpuAA": filepath.Join(devDir, "vfio", "devices", "vfioAA"),
	})
	writeCDISpec(t, specDir, "dra", "vendor.com/dra", map[string]string{
		"gpuAA": filepath.Join(devDir, "vfio", "devices", "vfioAA"),
	})
	cdi.GetRegistry().Refresh()

	podRes := &podresourcesv1.PodResources{
		Name:      "pod-a",
		Namespace: "ns-a",
		Containers: []*podresourcesv1.ContainerResources{
			{
				Name: "ctr-a",
				Devices: []*podresourcesv1.ContainerDevices{
					{ResourceName: "vendor.com/gpu", DeviceIds: []string{"gpuAA"}},
				},
				DynamicResources: []*podresourcesv1.DynamicResource{
					{
						ClaimName: "claim-a",
						ClaimResources: []*podresourcesv1.ClaimResource{
							{CDIDevices: []*podresourcesv1.CDIDevice{{Name: "vendor.com/dra=gpuAA"}}},
						},
					},
				},
			},
		},
	}

	sock := startFakePodResourcesServer(t, podRes)
	devices, err := getDeviceSpec(ctx, sock, map[string]string{}, []string{
		oci.PodResourceDeviceSourceDevicePlugin, oci.PodResourceDeviceSourceDRA,
	})
	assert.Error(err)
	assert.Nil(devices)
	assert.Contains(err.Error(), bdf)
}
