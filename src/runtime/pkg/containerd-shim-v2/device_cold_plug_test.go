// Copyright (c) 2025 NVIDIA CORPORATION.
// Copyright (c) 2026 NAVER Cloud Corp.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"net"
	"os"
	"path/filepath"
	"testing"

	"github.com/container-orchestrated-devices/container-device-interface/pkg/cdi"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc"
	podresourcesv1 "k8s.io/kubelet/pkg/apis/podresources/v1"
)

type fakePodResourcesServer struct {
	podresourcesv1.UnimplementedPodResourcesListerServer
	resp *podresourcesv1.GetPodResourcesResponse
}

func (f *fakePodResourcesServer) Get(ctx context.Context, req *podresourcesv1.GetPodResourcesRequest) (*podresourcesv1.GetPodResourcesResponse, error) {
	return f.resp, nil
}

func startFakePodResourcesServer(t *testing.T, podRes *podresourcesv1.PodResources) string {
	t.Helper()

	sockPath := filepath.Join(t.TempDir(), "kubelet.sock")
	lis, err := net.Listen("unix", sockPath)
	require.NoError(t, err)

	server := grpc.NewServer()
	podresourcesv1.RegisterPodResourcesListerServer(server, &fakePodResourcesServer{
		resp: &podresourcesv1.GetPodResourcesResponse{PodResources: podRes},
	})

	go func() {
		_ = server.Serve(lis)
	}()

	t.Cleanup(server.Stop)

	return sockPath
}

func TestFormatCDIDevIDs(t *testing.T) {
	assert := assert.New(t)

	result := formatCDIDevIDs("vendor.com/gpu", []string{"gpu0", "gpu1"})
	assert.Equal([]string{"vendor.com/gpu=gpu0", "vendor.com/gpu=gpu1"}, result)

	result = formatCDIDevIDs("vendor.com/gpu", []string{})
	assert.Nil(result)
}

func TestCollectPodResourceCDIDevices(t *testing.T) {
	assert := assert.New(t)

	deviceOnly := &podresourcesv1.ContainerResources{
		Name: "ctr-device-only",
		Devices: []*podresourcesv1.ContainerDevices{
			{ResourceName: "vendor.com/gpu", DeviceIds: []string{"gpu0"}},
		},
	}
	assert.Empty(collectPodResourceCDIDevices(deviceOnly))

	draOnly := &podresourcesv1.ContainerResources{
		Name: "ctr-dra-only",
		DynamicResources: []*podresourcesv1.DynamicResource{
			{
				ClaimName:      "gpu-claim",
				ClaimNamespace: "default",
				ClaimResources: []*podresourcesv1.ClaimResource{
					{
						CDIDevices: []*podresourcesv1.CDIDevice{
							{Name: "vendor.com/gpu=gpu0"},
							{Name: "vendor.com/gpu=gpu1"},
						},
					},
				},
			},
		},
	}
	assert.Equal([]string{"vendor.com/gpu=gpu0", "vendor.com/gpu=gpu1"}, collectPodResourceCDIDevices(draOnly))

	mixed := &podresourcesv1.ContainerResources{
		Name: "ctr-mixed",
		DynamicResources: []*podresourcesv1.DynamicResource{
			{
				ClaimName:      "claim-a",
				ClaimNamespace: "ns-a",
				ClaimResources: []*podresourcesv1.ClaimResource{
					{CDIDevices: []*podresourcesv1.CDIDevice{{Name: "vendor.com/gpu=gpu0"}}},
				},
			},
			{
				ClaimName:      "claim-b",
				ClaimNamespace: "ns-b",
				ClaimResources: []*podresourcesv1.ClaimResource{
					{CDIDevices: []*podresourcesv1.CDIDevice{{Name: "vendor.com/nic=nic0"}}},
					{CDIDevices: []*podresourcesv1.CDIDevice{{Name: "vendor.com/nic=nic1"}}},
				},
			},
		},
	}
	assert.Equal(
		[]string{"vendor.com/gpu=gpu0", "vendor.com/nic=nic0", "vendor.com/nic=nic1"},
		collectPodResourceCDIDevices(mixed),
	)

	// duplicates are preserved here: dedup is getDeviceSpec's job, not this helper's
	dup := &podresourcesv1.ContainerResources{
		Name: "ctr-dup",
		DynamicResources: []*podresourcesv1.DynamicResource{
			{
				ClaimName: "claim-dup",
				ClaimResources: []*podresourcesv1.ClaimResource{
					{CDIDevices: []*podresourcesv1.CDIDevice{{Name: "vendor.com/gpu=gpu0"}}},
				},
			},
			{
				ClaimName: "claim-dup-2",
				ClaimResources: []*podresourcesv1.ClaimResource{
					{CDIDevices: []*podresourcesv1.CDIDevice{{Name: "vendor.com/gpu=gpu0"}}},
				},
			},
		},
	}
	assert.Equal([]string{"vendor.com/gpu=gpu0", "vendor.com/gpu=gpu0"}, collectPodResourceCDIDevices(dup))

	// a CDIDevice with an empty Name must be skipped
	withEmpty := &podresourcesv1.ContainerResources{
		Name: "ctr-empty-name",
		DynamicResources: []*podresourcesv1.DynamicResource{
			{
				ClaimName: "claim-empty",
				ClaimResources: []*podresourcesv1.ClaimResource{
					{CDIDevices: []*podresourcesv1.CDIDevice{{Name: ""}, {Name: "vendor.com/gpu=gpu0"}}},
				},
			},
		},
	}
	assert.Equal([]string{"vendor.com/gpu=gpu0"}, collectPodResourceCDIDevices(withEmpty))

	// container resources without any DynamicResources yield nothing
	assert.Empty(collectPodResourceCDIDevices(&podresourcesv1.ContainerResources{Name: "ctr-nil"}))
}

func TestDedupStrings(t *testing.T) {
	assert := assert.New(t)

	assert.Equal(
		[]string{"a", "b", "c"},
		dedupStrings([]string{"a", "b", "a", "c", "b"}),
	)
	assert.Nil(dedupStrings(nil))
	assert.Equal([]string{"a"}, dedupStrings([]string{"a"}))
}

func TestContains(t *testing.T) {
	assert := assert.New(t)

	assert.True(contains([]string{"a", "b"}, "a"))
	assert.False(contains([]string{"a", "b"}, "c"))
	assert.False(contains(nil, "a"))
}

func TestGetDeviceSpecSourceFiltering(t *testing.T) {
	assert := assert.New(t)
	ctx := context.Background()

	specDir := t.TempDir()
	setCDISpecDir(t, specDir)
	writeCDISpec(t, specDir, "spec", "vendor.com/gpu", map[string]string{
		"gpu0":     "/dev/vendor-gpu0",
		"dra-gpu0": "/dev/vendor-dra-gpu0",
	})
	cdi.GetRegistry().Refresh()

	podRes := &podresourcesv1.PodResources{
		Name:      "pod-a",
		Namespace: "ns-a",
		Containers: []*podresourcesv1.ContainerResources{
			{
				Name: "ctr-a",
				Devices: []*podresourcesv1.ContainerDevices{
					{ResourceName: "vendor.com/gpu", DeviceIds: []string{"gpu0"}},
				},
				DynamicResources: []*podresourcesv1.DynamicResource{
					{
						ClaimName: "claim-a",
						ClaimResources: []*podresourcesv1.ClaimResource{
							{CDIDevices: []*podresourcesv1.CDIDevice{{Name: "vendor.com/gpu=dra-gpu0"}}},
						},
					},
				},
			},
		},
	}

	t.Run("device-plugin only source returns only device-plugin devices", func(t *testing.T) {
		sock := startFakePodResourcesServer(t, podRes)
		devices, err := getDeviceSpec(ctx, sock, map[string]string{}, []string{oci.PodResourceDeviceSourceDevicePlugin})
		// device-plugin source selected but DRA data present -> fail closed.
		assert.Error(err)
		assert.Nil(devices)
	})

	t.Run("dra only source returns only dra devices and fails closed on device-plugin data", func(t *testing.T) {
		sock := startFakePodResourcesServer(t, podRes)
		devices, err := getDeviceSpec(ctx, sock, map[string]string{}, []string{oci.PodResourceDeviceSourceDRA})
		// dra source selected but device-plugin data present -> fail closed.
		assert.Error(err)
		assert.Nil(devices)
	})

	t.Run("both sources returns the union", func(t *testing.T) {
		sock := startFakePodResourcesServer(t, podRes)
		devices, err := getDeviceSpec(ctx, sock, map[string]string{}, []string{
			oci.PodResourceDeviceSourceDevicePlugin, oci.PodResourceDeviceSourceDRA,
		})
		assert.NoError(err)
		assert.ElementsMatch([]string{"vendor.com/gpu=gpu0", "vendor.com/gpu=dra-gpu0"}, devices)
	})
}

func TestGetDeviceSpecUnlistedSourceNoDeviceNodesExempt(t *testing.T) {
	assert := assert.New(t)
	ctx := context.Background()

	specDir := t.TempDir()
	setCDISpecDir(t, specDir)
	// gpu0 resolves in the registry but declares no device nodes (env-only CDI):
	// nothing gets cold-plugged for it, so an unlisted source must not fail closed.
	writeCDISpecEnvOnly(t, specDir, "spec", "vendor.com/gpu", "gpu0")
	cdi.GetRegistry().Refresh()

	podRes := &podresourcesv1.PodResources{
		Name:      "pod-a",
		Namespace: "ns-a",
		Containers: []*podresourcesv1.ContainerResources{
			{
				Name: "ctr-a",
				Devices: []*podresourcesv1.ContainerDevices{
					{ResourceName: "vendor.com/gpu", DeviceIds: []string{"gpu0"}},
				},
			},
		},
	}
	sock := startFakePodResourcesServer(t, podRes)
	devices, err := getDeviceSpec(ctx, sock, map[string]string{}, []string{oci.PodResourceDeviceSourceDRA})
	assert.NoError(err)
	assert.Empty(devices)
}

func TestGetDeviceSpecFailClosed(t *testing.T) {
	assert := assert.New(t)
	ctx := context.Background()

	specDir := t.TempDir()
	setCDISpecDir(t, specDir)
	writeCDISpec(t, specDir, "spec", "vendor.com/gpu", map[string]string{
		"gpu0":     "/dev/vendor-gpu0",
		"dra-gpu0": "/dev/vendor-dra-gpu0",
	})
	cdi.GetRegistry().Refresh()

	t.Run("device-plugin data present but sources=[dra] errors", func(t *testing.T) {
		podRes := &podresourcesv1.PodResources{
			Name:      "pod-a",
			Namespace: "ns-a",
			Containers: []*podresourcesv1.ContainerResources{
				{
					Name: "ctr-a",
					Devices: []*podresourcesv1.ContainerDevices{
						{ResourceName: "vendor.com/gpu", DeviceIds: []string{"gpu0"}},
					},
				},
			},
		}
		sock := startFakePodResourcesServer(t, podRes)
		devices, err := getDeviceSpec(ctx, sock, map[string]string{}, []string{oci.PodResourceDeviceSourceDRA})
		assert.Error(err)
		assert.Nil(devices)
		assert.Contains(err.Error(), "pod_resource_device_sources")
		assert.Contains(err.Error(), oci.PodResourceDeviceSourceDevicePlugin)
	})

	t.Run("dra data present but default sources=[device-plugin] errors", func(t *testing.T) {
		podRes := &podresourcesv1.PodResources{
			Name:      "pod-a",
			Namespace: "ns-a",
			Containers: []*podresourcesv1.ContainerResources{
				{
					Name: "ctr-a",
					DynamicResources: []*podresourcesv1.DynamicResource{
						{
							ClaimName: "claim-a",
							ClaimResources: []*podresourcesv1.ClaimResource{
								{CDIDevices: []*podresourcesv1.CDIDevice{{Name: "vendor.com/gpu=dra-gpu0"}}},
							},
						},
					},
				},
			},
		}
		sock := startFakePodResourcesServer(t, podRes)
		devices, err := getDeviceSpec(ctx, sock, map[string]string{}, []string{oci.PodResourceDeviceSourceDevicePlugin})
		assert.Error(err)
		assert.Nil(devices)
		assert.Contains(err.Error(), "pod_resource_device_sources")
		assert.Contains(err.Error(), oci.PodResourceDeviceSourceDRA)
	})

	t.Run("unlisted device-plugin data that is not CDI-resolvable does not error", func(t *testing.T) {
		// vendor.com/nic is never registered in the CDI registry, so it is
		// not cold-pluggable and exempt from the fail-closed guard.
		podRes := &podresourcesv1.PodResources{
			Name:      "pod-a",
			Namespace: "ns-a",
			Containers: []*podresourcesv1.ContainerResources{
				{
					Name: "ctr-a",
					Devices: []*podresourcesv1.ContainerDevices{
						{ResourceName: "vendor.com/nic", DeviceIds: []string{"nic0"}},
					},
				},
			},
		}
		sock := startFakePodResourcesServer(t, podRes)
		devices, err := getDeviceSpec(ctx, sock, map[string]string{}, []string{oci.PodResourceDeviceSourceDRA})
		assert.NoError(err)
		assert.Empty(devices)
	})

	t.Run("unlisted device-plugin data with one CDI-resolvable entry among non-resolvable ones still errors", func(t *testing.T) {
		podRes := &podresourcesv1.PodResources{
			Name:      "pod-a",
			Namespace: "ns-a",
			Containers: []*podresourcesv1.ContainerResources{
				{
					Name: "ctr-a",
					Devices: []*podresourcesv1.ContainerDevices{
						{ResourceName: "vendor.com/nic", DeviceIds: []string{"nic0"}},
						{ResourceName: "vendor.com/gpu", DeviceIds: []string{"gpu0"}},
					},
				},
			},
		}
		sock := startFakePodResourcesServer(t, podRes)
		devices, err := getDeviceSpec(ctx, sock, map[string]string{}, []string{oci.PodResourceDeviceSourceDRA})
		assert.Error(err)
		assert.Nil(devices)
		assert.Contains(err.Error(), "vendor.com/gpu=gpu0")
		assert.NotContains(err.Error(), "vendor.com/nic=nic0")
	})
}

// TestGetDeviceSpecSharedClaimDedup: a claim shared across containers reports
// the same CDI device once per container; the result must be deduplicated.
func TestGetDeviceSpecSharedClaimDedup(t *testing.T) {
	assert := assert.New(t)
	ctx := context.Background()

	podRes := &podresourcesv1.PodResources{
		Name:      "pod-a",
		Namespace: "ns-a",
		Containers: []*podresourcesv1.ContainerResources{
			{
				Name: "ctr-a",
				DynamicResources: []*podresourcesv1.DynamicResource{
					{
						ClaimName: "shared-claim",
						ClaimResources: []*podresourcesv1.ClaimResource{
							{CDIDevices: []*podresourcesv1.CDIDevice{{Name: "vendor.com/gpu=gpu0"}}},
						},
					},
				},
			},
			{
				Name: "ctr-b",
				DynamicResources: []*podresourcesv1.DynamicResource{
					{
						// Same claim shared with ctr-a, surfaces the same CDI device again.
						ClaimName: "shared-claim",
						ClaimResources: []*podresourcesv1.ClaimResource{
							{CDIDevices: []*podresourcesv1.CDIDevice{{Name: "vendor.com/gpu=gpu0"}}},
						},
					},
				},
			},
		},
	}

	sock := startFakePodResourcesServer(t, podRes)
	devices, err := getDeviceSpec(ctx, sock, map[string]string{}, []string{oci.PodResourceDeviceSourceDRA})
	assert.NoError(err)
	assert.Equal([]string{"vendor.com/gpu=gpu0"}, devices)
}

func TestGetDeviceSpecNoDevices(t *testing.T) {
	assert := assert.New(t)
	ctx := context.Background()

	podRes := &podresourcesv1.PodResources{
		Name:      "pod-a",
		Namespace: "ns-a",
		Containers: []*podresourcesv1.ContainerResources{
			{Name: "ctr-a"},
		},
	}

	for _, sources := range [][]string{
		{oci.PodResourceDeviceSourceDevicePlugin},
		{oci.PodResourceDeviceSourceDRA},
		{oci.PodResourceDeviceSourceDevicePlugin, oci.PodResourceDeviceSourceDRA},
	} {
		sock := startFakePodResourcesServer(t, podRes)
		devices, err := getDeviceSpec(ctx, sock, map[string]string{}, sources)
		assert.NoError(err, "sources=%v", sources)
		assert.Empty(devices, "sources=%v", sources)
	}
}

func TestKubeletPodResourceSocketAvailable(t *testing.T) {
	assert.False(t, kubeletPodResourceSocketAvailable(""))

	tmpDir := t.TempDir()
	sockPath := filepath.Join(tmpDir, "kubelet.sock")
	assert.False(t, kubeletPodResourceSocketAvailable(sockPath))

	f, err := os.Create(sockPath)
	assert.NoError(t, err)
	assert.NoError(t, f.Close())

	assert.False(t, kubeletPodResourceSocketAvailable(sockPath))

	assert.NoError(t, os.Remove(sockPath))
	l, err := net.Listen("unix", sockPath)
	assert.NoError(t, err)
	defer l.Close()

	assert.True(t, kubeletPodResourceSocketAvailable(sockPath))
}
