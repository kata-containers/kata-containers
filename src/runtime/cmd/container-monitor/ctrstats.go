package main

import (
	gocontext "context"
	"fmt"
	"net"
	"path/filepath"
	"strings"

	v1 "github.com/containerd/cgroups/stats/v1"
	"github.com/containerd/containerd"
	"github.com/containerd/containerd/namespaces"
	"github.com/containerd/containerd/runtime/v2/shim"
	"github.com/containerd/containerd/runtime/v2/task"
	"github.com/containerd/ttrpc"
	"github.com/containerd/typeurl"
	"github.com/opencontainers/runtime-spec/specs-go"
)

type Container struct {
	sandboxID        string
	containerID      string
	podID            string
	podName          string
	sandboxNamespace string
	containerName    string
}

func GetContainers(address, namespace string) ([]Container, error) {
	var cids []Container

	ctx := gocontext.Background()

	// TODO: do we want a timeout?
	client, err := containerd.New(address, containerd.WithDefaultNamespace(namespace))
	if err != nil {
		monitorLog.WithError(err).Errorf("failure to create new containerd client")

		return cids, err
	}
	defer client.Close()

	containers, err := client.Containers(ctx)
	if err != nil {
		monitorLog.WithError(err).Errorf("client containers request failure")

		return cids, err
	}

	for _, c := range containers {
		var sandboxID, podName, sandboxNamespace, containerName string

		container, err := c.Info(ctx)
		if err != nil {
			monitorLog.WithField("container-id", c.ID()).Warn("could not get container info")

			continue
		}

		if container.Spec != nil && container.Spec.GetValue() != nil {
			v, err := typeurl.UnmarshalAny(container.Spec)
			if err != nil {
				monitorLog.WithField("container-id", c.ID()).Warn("could not get container Spec")
				continue
			}

			spec := v.(*specs.Spec)
			sandboxID = spec.Annotations["io.kubernetes.cri.sandbox-id"]
			podName = spec.Annotations["io.kubernetes.cri.sandbox-name"]
			sandboxNamespace = spec.Annotations["io.kubernetes.cri.sandbox-namespace"]
			containerName = spec.Annotations["io.kubernetes.cri.container-name"]
		}

		containerInfo := Container{
			containerID:      c.ID(),
			sandboxID:        sandboxID,
			podID:            container.Labels["io.kubernetes.pod.uid"],
			podName:          podName,
			sandboxNamespace: sandboxNamespace,
			containerName:    containerName,
		}

		// If there isn't a sandboxID, then we're probably dealing with
		// standard container (not a pod - launched with ctr)-- set sandboxID to containerID
		if containerInfo.sandboxID == "" {
			monitorLog.WithField("container-id", containerInfo.containerID).Info("container identified without corresponding sandbox id")
			containerInfo.sandboxID = containerInfo.containerID
		}

		cids = append(cids, containerInfo)
	}

	return cids, nil
}

func GetContainerStats(address, namespace string, c Container) (*v1.Metrics, error) {
	client, service, err := getTaskService(address, namespace, c.sandboxID)
	if err != nil {
		return nil, err
	}
	defer client.Close()

	r, err := service.Stats(gocontext.Background(), &task.StatsRequest{
		ID: c.containerID,
	})
	if err != nil {
		return nil, err
	}

	if r.Stats.GetValue() != nil {
		s, err := typeurl.UnmarshalAny(r.Stats)
		if err != nil {
			return nil, err
		}

		stats := s.(*v1.Metrics)

		return stats, nil
	}

	return nil, fmt.Errorf("no stats obtained for container: %s", c.containerID)
}

func getTaskService(address, ns, id string) (*ttrpc.Client, task.TaskService, error) {
	if id == "" {
		return nil, nil, fmt.Errorf("container id must be specified")
	}

	if ns == "" {
		return nil, nil, fmt.Errorf("namespace must be specified")
	}

	s1 := filepath.Join(string(filepath.Separator), "containerd-shim", ns, id, "shim.sock")
	ctx := namespaces.WithNamespace(gocontext.Background(), ns)
	s2, _ := shim.SocketAddress(ctx, address, id)
	s2 = strings.TrimPrefix(s2, "unix://")

	for _, socket := range []string{s2, "\x00" + s1} {
		conn, err := net.Dial("unix", socket)
		if err == nil {
			client := ttrpc.NewClient(conn)
			return client, task.NewTaskClient(client), nil
		}
	}

	return nil, nil, fmt.Errorf("fail to connect to container %s's shim", id)
}
