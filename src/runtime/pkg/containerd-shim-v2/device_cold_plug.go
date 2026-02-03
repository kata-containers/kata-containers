// Copyright (c) 2025 NVIDIA CORPORATION.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"net"
	"strings"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/opencontainers/runtime-spec/specs-go"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	podresourcesv1 "k8s.io/kubelet/pkg/apis/podresources/v1"
)

const (
	nameAnnotation      = "io.kubernetes.cri.sandbox-name"
	namespaceAnnotation = "io.kubernetes.cri.sandbox-namespace"
)

// coldPlugDevices handles cold plug of CDI devices into the sandbox
// kubelet's PodResources API is used for determining the devices to be
// cold plugged, if so configured. Otherwise, cdi annotations can be used for
// covering stand alone scenarios.
func coldPlugDevices(ctx context.Context, s *service, ociSpec *specs.Spec) error {
	if s.config.HypervisorConfig.ColdPlugVFIO == config.NoPort {
		// device cold plug is not enabled
		shimLog.Debug("cold_plug_vfio not enabled, skip device cold plug")
		return nil
	}

	kubeletSock := s.config.PodResourceAPISock
	if kubeletSock != "" {
		return coldPlugWithAPI(ctx, s, ociSpec)
	}

	shimLog.Debug("config.PodResourceAPISock not set, skip k8s based device cold plug")

	// Here we deal with CDI devices that are cold-plugged
	// for the single_container (nerdctl, podman, ...) use-case.
	// We can provide additional directories where to search for
	// CDI specs if needed. immutable OS's only have specific
	// directories where applications can write too. For instance /opt/cdi
	_, err := config.WithCDI(ociSpec.Annotations, []string{}, ociSpec)
	if err != nil {
		return fmt.Errorf("CDI device injection failed: %w", err)
	}
	return nil
}

func coldPlugWithAPI(ctx context.Context, s *service, ociSpec *specs.Spec) error {
	ann := ociSpec.Annotations
	devices, err := getDeviceSpec(ctx, s.config.PodResourceAPISock, ann)
	if err != nil {
		return err
	}

	if len(devices) == 0 {
		shimLog.WithField("pod", debugPodID(ann)).Debug("No devices found in Pod Resources, skip cold plug")
		return nil
	}

	err = config.InjectCDIDevices(ociSpec, devices)
	if err != nil {
		return fmt.Errorf("cold plug: CDI device injection failed: %w", err)
	}

	return nil
}

// getDeviceSpec fetches the device information for the pod sandbox using
// kubelet's pod resource api. This is necessary for cold plug because
// the Kubelet does not pass the device information via CRI during
// Sandbox creation.
func getDeviceSpec(ctx context.Context, socket string, ann map[string]string) ([]string, error) {
	podName := ann[nameAnnotation]
	podNs := ann[namespaceAnnotation]

	// create dialer for unix socket
	dialer := func(ctx context.Context, target string) (net.Conn, error) {
		// need this workaround to avoid duplicate prefix
		addr := strings.TrimPrefix(target, "unix://")
		return (&net.Dialer{}).DialContext(ctx, "unix", addr)
	}

	target := fmt.Sprintf("unix://%s", socket)
	conn, err := grpc.NewClient(
		target,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithContextDialer(dialer),
		grpc.WithDefaultCallOptions(grpc.MaxCallRecvMsgSize(16*1024*1024)),
	)

	if err != nil {
		return nil, fmt.Errorf("cold plug: failed to connect to kubelet: %w", err)
	}
	defer conn.Close()

	// create client
	client := podresourcesv1.NewPodResourcesListerClient(conn)

	// get all pod resources
	prr := &podresourcesv1.GetPodResourcesRequest{
		PodName:      podName,
		PodNamespace: podNs,
	}
	resp, err := client.Get(ctx, prr)
	if err != nil {
		return nil, fmt.Errorf("cold plug: GetPodResources failed: %w", err)
	}
	podRes := resp.PodResources
	if podRes == nil {
		return nil, fmt.Errorf("cold plug: PodResources is nil")
	}

	// Process results
	var devices []string
	for _, container := range podRes.Containers {
		for _, d := range container.Devices {
			shimLog.WithField("container", container.Name).Debugf("Pod Resources Device: %s = %v\n",
				d.ResourceName, d.DeviceIds)
			cdiDevs := formatCDIDevIDs(d.ResourceName, d.DeviceIds)
			devices = append(devices, cdiDevs...)
		}
	}

	return devices, nil
}

// formatCDIDevIDs formats the way CDI package expects
func formatCDIDevIDs(specName string, devIDs []string) []string {
	var result []string
	for _, id := range devIDs {
		// Normalize IOMMUFD device IDs: vfio5 -> 5
		cleanID := strings.TrimPrefix(id, "vfio")
		result = append(result, fmt.Sprintf("%s=%s", specName, cleanID))
	}
	return result
}

func debugPodID(ann map[string]string) string {
	return fmt.Sprintf("%s/%s", ann[namespaceAnnotation], ann[nameAnnotation])
}
