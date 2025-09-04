// Copyright (c) 2025 Nvidia
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net"
	"runtime"
	"strconv"
	"strings"
	"time"

	"github.com/container-orchestrated-devices/container-device-interface/pkg/cdi"
	"github.com/containerd/ttrpc"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/cdiresolver"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/pkg/errors"
)

const (
	resourceAnnotation  = "io.katacontainers.pod-resources"
	nameAnnotation      = "io.kubernetes.cri.sandbox-name"
	namespaceAnnotation = "io.kubernetes.cri.sandbox-namespace"
	cnAnnotation        = "io.kubernetes.cri.container-name"
	cleanupTO           = 5 * time.Second
)

type PodResourceSpec struct {
	Containers     map[string]ResourceSpec `json:"containers"`
	InitContainers map[string]ResourceSpec `json:"initContainers"`
}

// ResourceSpec is a string maps of the form resourceName : quantity,
// originating from the Pod spec
type ResourceSpec struct {
	Requests map[string]string `json:"requests"`
	Limits   map[string]string `json:"limits"`
}

// aggregate returns a map of devices and counts that need to be cold plugged
// based on the PodResourceSpec
func (prs *PodResourceSpec) aggregate() (map[string]int, error) {
	tally := func(in map[string]ResourceSpec, out map[string]int) error {

		for _, crs := range in {
			// Honor only limits
			for device, count := range crs.Limits {
				num, err := strconv.Atoi(count)
				if err != nil {
					return wrappedError(err)
				}
				out[device] += num
			}
		}
		return nil
	}
	// create a consolidated map of devices and count
	ctrDevs := make(map[string]int)
	if err := tally(prs.Containers, ctrDevs); err != nil {
		return nil, err
	}
	initDevs := make(map[string]int)
	if err := tally(prs.InitContainers, initDevs); err != nil {
		return nil, err
	}

	// combine into a single map of greater of the two tallies
	agg := make(map[string]int)
	for device, count := range ctrDevs {
		if count > initDevs[device] {
			agg[device] = count
		} else {
			agg[device] = initDevs[device]
		}
	}
	for device, count := range initDevs {
		if _, exists := agg[device]; !exists {
			agg[device] = count
		}
	}

	return agg, nil
}

// resolveCDIAnnotations processes annotations for cold plug of CDI devices into
// the sandbox
func resolveCDIAnnotations(ctx context.Context, s *service, anno map[string]string) error {
	coldplugs, err := getColdPlugs(anno)
	if err != nil {
		return wrappedError(err)
	}

	if coldplugs == nil {
		// no annotations to process
		return nil
	}

	s.kubePodID = getPodID(anno)
	// convert resolver list into a map
	resolvers := getResolvers(s.config)

	if len(resolvers) == 0 {
		return fmt.Errorf("%s present, but no resolvers", resourceAnnotation)
	}

	for device, count := range coldplugs {
		shimLog.WithField("device", device).Infof("resolve %d", count)
		res := resolvers[device]
		// add CDI annotations
		proxyResolveCDI(ctx, res, count, anno)
	}

	return nil
}

// updateCDIDevices updates the device spec based on the overrides provided by
// resolver based on the cold plug
func updateCDIDevices(ctx context.Context, ociSpec *specs.Spec, config *oci.RuntimeConfig) error {
	podID := getPodID(ociSpec.Annotations)
	containerID := formatContainerID(podID, ociSpec.Annotations[cnAnnotation])
	resolvers := getResolvers(config)
	if len(resolvers) == 0 {
		// no resolvers configured
		return nil
	}

	if ociSpec.Linux == nil {
		return nil
	}

	var devPaths []string
	devIndices := make(map[string]int)
	for ix, dev := range ociSpec.Linux.Devices {
		if dev.Path != "" {
			devPaths = append(devPaths, dev.Path)
			devIndices[dev.Path] = ix
		}
	}
	if len(devPaths) == 0 {
		// Nothing to process
		return nil
	}

	req := &pb.ContainerRequest{
		PodID:            podID,
		ContainerID:      containerID,
		VirtualDeviceIDs: devPaths,
	}

	// pass the device paths via each device resolver and apply edits
	for devName, res := range resolvers {
		if res == nil {
			return wrappedError(fmt.Errorf("missing resolver"))
		}
		resp, err := invokeTTRPC(ctx, true, res, req)
		if err != nil {
			return wrappedError(err)
		}
		for ix := 0; ix < len(resp.PhysicalDeviceIDs); ix++ {
			devIndex := devIndices[resp.VirtualDeviceIDs[ix]]
			shimLog.WithField("device", devName).Infof("update %s -> %s", ociSpec.Linux.Devices[devIndex].Path, resp.PhysicalDeviceIDs[ix])
			ociSpec.Linux.Devices[devIndex].Path = resp.PhysicalDeviceIDs[ix]
		}
	}
	return nil
}

// cleanupCDIResolver performs clean up of the resolver when a container or the
// sandbox is deleted
func cleanupCDIResolver(s *service, c *container) error {
	var errString string

	if s == nil {
		return nil
	}
	podID := s.kubePodID
	config := s.config
	resolvers := getResolvers(config)
	if len(resolvers) == 0 {
		// no resolvers configured
		return nil
	}

	ctx, cancel := context.WithTimeout(context.Background(), cleanupTO)
	defer cancel()
	var req interface{}
	for _, res := range resolvers {
		if c == nil || c.cType.IsSandbox() {
			req = &pb.PodRequest{
				DeviceType: res.SpecName,
				PodID:      podID,
			}
		} else {
			if c.spec == nil {
				shimLog.Warn("c.spec is nil, can't clean up")
				return nil
			}
			containerID := formatContainerID(podID, c.spec.Annotations[cnAnnotation])
			req = &pb.ContainerRequest{
				PodID:       podID,
				ContainerID: containerID,
			}
		}

		_, err := invokeTTRPC(ctx, false, res, req)
		if err != nil {
			errString = errString + fmt.Sprintf("%s : %v ", res.SpecName, err)
		}
	}

	if len(errString) == 0 {
		return nil
	}

	return wrappedError(fmt.Errorf("%s", errString))
}

func getColdPlugs(anno map[string]string) (map[string]int, error) {
	prs, err := getPodResourceSpec(anno)
	if err != nil {
		return nil, err
	}

	return prs.aggregate()
}

func getPodResourceSpec(anno map[string]string) (*PodResourceSpec, error) {
	var jsonCleaner = strings.NewReplacer(
		"\\n", "",
		"\\r", "",
		"\\t", "",
		"\\\"", "\"",
	)

	specJson, found := anno[resourceAnnotation]
	if !found {
		return nil, nil
	}

	// clean json string
	specJson = jsonCleaner.Replace(specJson)
	//os.WriteFile("/tmp/ann.text", []byte(specJson), 0644)
	var prs PodResourceSpec
	if err := json.Unmarshal([]byte(specJson), &prs); err != nil {
		return nil, wrappedError(errors.Wrapf(err, "(annotation: %s)", specJson))
	}

	return &prs, nil
}

func proxyResolveCDI(ctx context.Context, res *oci.Resolver, count int, anno map[string]string) error {
	req := &pb.PodRequest{
		DeviceType: res.SpecName,
		PodID:      getPodID(anno),
		Count:      int32(count),
	}
	resp, err := invokeTTRPC(ctx, true, res, req)
	if err != nil {
		return wrappedError(err)
	}
	key := getCDIKey(res.SpecName)
	val := formatDevIds(resp, res.SpecName)
	anno[key] = val
	return nil
}

func invokeTTRPC(ctx context.Context, allocate bool, res *oci.Resolver, req interface{}) (*pb.PhysicalDeviceResponse, error) {
	addr := res.Socket
	if addr == "" {
		// no resolver specified
		return nil, fmt.Errorf("No resolver specified for %s", res.SpecName)
	}

	conn, err := net.DialTimeout("unix", res.Socket, 5*time.Second)
	if err != nil {
		return nil, wrappedError(err)
	}
	defer conn.Close()

	// Create ttrpc client
	client := ttrpc.NewClient(conn)
	defer client.Close()

	rc := pb.NewCDIResolverClient(client)

	switch reqType := req.(type) {
	case *pb.PodRequest:
		pr := req.(*pb.PodRequest)
		if allocate {
			return rc.AllocatePodDevices(ctx, pr)
		}
		return rc.FreePodDevices(ctx, pr)
	case *pb.ContainerRequest:
		cr := req.(*pb.ContainerRequest)
		if allocate {
			return rc.AllocateContainerDevices(ctx, cr)
		}
		return rc.FreeContainerDevices(ctx, cr)
	default:
		log.Fatalf("Unknown type %v", reqType)
	}

	return nil, nil
}

func getPodID(anno map[string]string) string {
	podName := anno[nameAnnotation]
	podNs := anno[namespaceAnnotation]
	return podNs + "_" + podName
}

func getResolvers(config *oci.RuntimeConfig) map[string]*oci.Resolver {
	// convert resolver list into a map
	resolvers := make(map[string]*oci.Resolver)
	for _, res := range config.ProxyCDIResolvers {
		resolvers[res.SpecName] = &res
	}

	return resolvers
}

func getCDIKey(device string) string {
	return cdi.AnnotationPrefix + device
}
func formatDevIds(resp *pb.PhysicalDeviceResponse, device string) string {
	val := device + "=" + resp.PhysicalDeviceIDs[0]
	for ix := 1; ix < len(resp.PhysicalDeviceIDs); ix++ {
		val = val + "," + device + "=" + resp.PhysicalDeviceIDs[ix]
	}

	return val
}

func formatContainerID(podID, containerName string) string {
	return podID + "_" + containerName
}

func wrappedError(e error) error {
	_, file, line, ok := runtime.Caller(1)
	if ok {
		return errors.Wrapf(e, "[%s:%d]", file, line)
	}

	return errors.Wrapf(e, "[no caller info]")
}
