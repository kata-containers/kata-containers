// Copyright (c) 2025 NVIDIA CORPORATION.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"net"
	"os"
	"strings"

	"github.com/container-orchestrated-devices/container-device-interface/pkg/cdi"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	podresourcesv1 "k8s.io/kubelet/pkg/apis/podresources/v1"
)

const (
	// containerd CRI annotations
	nameAnnotation      = "io.kubernetes.cri.sandbox-name"
	namespaceAnnotation = "io.kubernetes.cri.sandbox-namespace"

	// CRI-O annotations
	crioNameAnnotation      = "io.kubernetes.cri-o.KubeName"
	crioNamespaceAnnotation = "io.kubernetes.cri-o.Namespace"
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

	if kubeletPodResourceSocketAvailable(s.config.PodResourceAPISock) {
		return coldPlugWithAPI(ctx, s, ociSpec)
	}

	if s.config.PodResourceAPISock != "" {
		shimLog.Debugf("config.PodResourceAPISock %q not available, fall back to CDI annotations", s.config.PodResourceAPISock)
	} else {
		shimLog.Debug("config.PodResourceAPISock not set, use CDI annotations for cold plug")
	}

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

// kubeletPodResourceSocketAvailable reports whether the configured kubelet Pod
// Resources API socket exists and can be used for Kubernetes-based cold plug.
func kubeletPodResourceSocketAvailable(sock string) bool {
	if sock == "" {
		return false
	}
	fi, err := os.Stat(sock)
	if err != nil {
		return false
	}

	return (fi.Mode() & os.ModeSocket) != 0
}

func coldPlugWithAPI(ctx context.Context, s *service, ociSpec *specs.Spec) error {
	ann := ociSpec.Annotations
	sources := s.config.PodResourceDeviceSources
	if len(sources) == 0 {
		// Config loading defaults this to ["device-plugin"]; if it is empty
		// anyway, fail closed rather than guess a source.
		return fmt.Errorf("cold plug: pod_resource_device_sources is empty, refusing to guess a device source")
	}

	devices, err := getDeviceSpec(ctx, s.config.PodResourceAPISock, ann, sources)
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
//
// sources selects which PodResources fields are read: "device-plugin"
// (container.Devices) and/or "dra" (DynamicResources CDI devices). List both
// only for disjoint device sets: kubelet double-counts a device advertised via
// both at scheduling; a same-device collision here is caught by the overlap check.
// Fail closed: an unlisted source carrying CDI-resolvable data is an error,
// so misconfiguration cannot silently boot the guest without its devices;
// data that never resolves in the CDI registry is not cold-pluggable and
// is exempt.
func getDeviceSpec(ctx context.Context, socket string, ann map[string]string, sources []string) ([]string, error) {
	wantDevicePlugin := contains(sources, oci.PodResourceDeviceSourceDevicePlugin)
	wantDRA := contains(sources, oci.PodResourceDeviceSourceDRA)

	podName, podNs := getPodIdentifiers(ann)

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
		return nil, fmt.Errorf("cold plug: GetPodResources failed for pod(%s) in namespace(%s): %w", podName, podNs, err)
	}
	podRes := resp.PodResources
	if podRes == nil {
		return nil, fmt.Errorf("cold plug: PodResources is nil")
	}

	// Process results
	var devices, allDevicePluginDevs, allDRADevs []string
	for _, container := range podRes.Containers {
		if container == nil {
			// A nil entry would panic on the field accesses below.
			continue
		}
		var devicePluginDevs []string
		for _, d := range container.Devices {
			shimLog.WithField("container", container.Name).Debugf("Pod Resources Device: %s = %v\n",
				d.ResourceName, d.DeviceIds)
			devicePluginDevs = append(devicePluginDevs, formatCDIDevIDs(d.ResourceName, d.DeviceIds)...)
		}

		draDevs := collectPodResourceCDIDevices(container)

		if !wantDevicePlugin && len(devicePluginDevs) > 0 {
			if resolvable := cdiResolvableDevices(devicePluginDevs); len(resolvable) > 0 {
				return nil, fmt.Errorf(
					"cold plug: container %q has cold-pluggable (CDI-resolvable) device-plugin PodResources data (%v) but %q is not in pod_resource_device_sources=%v; "+
						"add %q to the config option or this data will be silently dropped",
					container.Name, resolvable, oci.PodResourceDeviceSourceDevicePlugin, sources, oci.PodResourceDeviceSourceDevicePlugin)
			}
			shimLog.WithField("container", container.Name).Debug(
				"cold plug: container has device-plugin PodResources data but none of it is CDI-resolvable; ignoring (delivered via the regular OCI container path)")
		}
		if !wantDRA && len(draDevs) > 0 {
			if resolvable := cdiResolvableDevices(draDevs); len(resolvable) > 0 {
				return nil, fmt.Errorf(
					"cold plug: container %q has cold-pluggable (CDI-resolvable) DRA PodResources data (%v) but %q is not in pod_resource_device_sources=%v; "+
						"add %q to the config option or this data will be silently dropped",
					container.Name, resolvable, oci.PodResourceDeviceSourceDRA, sources, oci.PodResourceDeviceSourceDRA)
			}
			shimLog.WithField("container", container.Name).Debug(
				"cold plug: container has DRA PodResources data but none of it is CDI-resolvable; ignoring (delivered via the regular OCI container path)")
		}

		if wantDevicePlugin {
			deduped := dedupStrings(devicePluginDevs)
			devices = append(devices, deduped...)
			allDevicePluginDevs = append(allDevicePluginDevs, deduped...)
		}
		if wantDRA {
			deduped := dedupStrings(draDevs)
			devices = append(devices, deduped...)
			allDRADevs = append(allDRADevs, deduped...)
		}
	}

	if wantDevicePlugin && wantDRA {
		if err := checkCrossSourcePhysicalOverlap(dedupStrings(allDevicePluginDevs), dedupStrings(allDRADevs)); err != nil {
			return nil, err
		}
	}

	return dedupStrings(devices), nil
}

// collectPodResourceCDIDevices returns the CDI device names in a container's
// DynamicResources (KEP-3695): DRA allocations are reported only there, never
// in the device-plugin Devices field.
func collectPodResourceCDIDevices(container *podresourcesv1.ContainerResources) []string {
	var devices []string
	for _, dr := range container.GetDynamicResources() {
		for _, cr := range dr.GetClaimResources() {
			for _, cdiDev := range cr.GetCDIDevices() {
				name := cdiDev.GetName()
				if name == "" {
					continue
				}
				shimLog.WithFields(logrus.Fields{
					"container":      container.Name,
					"claim":          dr.GetClaimName(),
					"claimNamespace": dr.GetClaimNamespace(),
				}).Debugf("Pod Resources DRA CDI Device: %s\n", name)
				devices = append(devices, name)
			}
		}
	}
	return devices
}

// dedupStrings deduplicates preserving order: a ResourceClaim shared by
// several containers reports the same CDI device once per container, and
// plugging it twice would duplicate its OCI edits.
func dedupStrings(in []string) []string {
	if len(in) == 0 {
		return in
	}
	seen := make(map[string]struct{}, len(in))
	out := make([]string, 0, len(in))
	for _, s := range in {
		if _, ok := seen[s]; ok {
			continue
		}
		seen[s] = struct{}{}
		out = append(out, s)
	}
	return out
}

// cdiResolvableDevices returns the names that resolve to at least one device
// node in the CDI registry; only node-bearing devices are cold-plugged
// (InjectCDIDevices acts on device nodes), so only those matter to the
// fail-closed unlisted-source check. A CDI device with only env/mount edits
// injects nothing here and must not trip the guard. A stale registry view can
// only soften the check into a debug log: InjectCDIDevices re-checks after an
// explicit Refresh.
func cdiResolvableDevices(devs []string) []string {
	if len(devs) == 0 {
		return nil
	}

	var resolvable []string
	registry := cdi.GetRegistry()
	for _, name := range devs {
		dev := registry.DeviceDB().GetDevice(name)
		if dev == nil || dev.Device == nil {
			continue
		}
		for _, node := range dev.ContainerEdits.DeviceNodes {
			if node != nil && node.Path != "" {
				resolvable = append(resolvable, name)
				break
			}
		}
	}
	return resolvable
}

func contains(list []string, s string) bool {
	for _, v := range list {
		if v == s {
			return true
		}
	}
	return false
}

// formatCDIDevIDs formats the way CDI package expects
func formatCDIDevIDs(specName string, devIDs []string) []string {
	var result []string
	for _, id := range devIDs {
		result = append(result, fmt.Sprintf("%s=%s", specName, id))
	}
	return result
}

// getPodIdentifiers returns the pod name and namespace from annotations.
// It first checks containerd CRI annotations, then falls back to CRI-O annotations.
func getPodIdentifiers(ann map[string]string) (podName, podNamespace string) {
	podName = ann[nameAnnotation]
	podNamespace = ann[namespaceAnnotation]

	// Fall back to CRI-O annotations if containerd annotations are empty
	if podName == "" {
		podName = ann[crioNameAnnotation]
	}
	if podNamespace == "" {
		podNamespace = ann[crioNamespaceAnnotation]
	}

	return podName, podNamespace
}

func debugPodID(ann map[string]string) string {
	podName, podNamespace := getPodIdentifiers(ann)
	return fmt.Sprintf("%s/%s", podNamespace, podName)
}
