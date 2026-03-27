// Copyright (c) 2020 Ant Group
// Copyright (c) 2021 Red Hat Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"context"
	"fmt"
	"net"
	"net/url"
	"strings"

	"github.com/sirupsen/logrus"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"

	pb "k8s.io/cri-api/pkg/apis/runtime/v1"
)

const (
	// unixProtocol is the network protocol of unix socket.
	unixProtocol = "unix"
)

// Returns (protocol, address, dialer, error)
func getAddressAndDialer(endpoint string) (string, string, func(ctx context.Context, addr string) (net.Conn, error), error) {
	protocol, addr, err := parseEndpointWithFallbackProtocol(endpoint, unixProtocol)
	if err != nil {
		return "", "", nil, err
	}

	// Define the dialer based on the protocol discovered
	dialer := func(ctx context.Context, addr string) (net.Conn, error) {
		return (&net.Dialer{}).DialContext(ctx, protocol, addr)
	}

	return protocol, addr, dialer, nil
}

func getConnection(endPoint string) (*grpc.ClientConn, error) {
	protocol, addr, dialer, err := getAddressAndDialer(endPoint)
	if err != nil {
		return nil, err
	}

	dialTarget := fmt.Sprintf("%s:///%s", protocol, strings.TrimPrefix(addr, "/"))

	wrappedDialer := func(ctx context.Context, target string) (net.Conn, error) {
		cleanAddr := target
		if idx := strings.Index(target, "://"); idx != -1 {
			cleanAddr = strings.TrimPrefix(target[idx+3:], "/")
			if protocol == unixProtocol {
				cleanAddr = "/" + cleanAddr
			}
		}
		return dialer(ctx, cleanAddr)
	}

	monitorLog.Tracef("connecting to %s (resolved target: %s)", endPoint, dialTarget)

	conn, err := grpc.NewClient(dialTarget,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithContextDialer(wrappedDialer),
	)
	if err != nil {
		return nil, fmt.Errorf("gRPC connection failed for %s. make sure you are running as root and the endpoint has been started: %w", endPoint, err)
	}

	return conn, nil
}

func closeConnection(conn *grpc.ClientConn) error {
	if conn == nil {
		return nil
	}
	return conn.Close()
}

func getRuntimeClient(runtimeEndpoint string) (pb.RuntimeServiceClient, *grpc.ClientConn, error) {
	var (
		conn *grpc.ClientConn
		err  error
	)
	// Set up a connection to the server.
	// If no EndPoint set then use the default endpoint types
	conn, err = getConnection(runtimeEndpoint)
	if err != nil {
		return nil, nil, err
	}

	runtimeClient := pb.NewRuntimeServiceClient(conn)
	return runtimeClient, conn, nil
}

func parseEndpointWithFallbackProtocol(endpoint string, fallbackProtocol string) (protocol string, addr string, err error) {
	if protocol, addr, err = parseEndpoint(endpoint); err != nil && protocol == "" {
		fallbackEndpoint := fallbackProtocol + "://" + endpoint
		protocol, addr, err = parseEndpoint(fallbackEndpoint)
		if err == nil {
			monitorLog.Warningf("Using %q as endpoint is deprecated, please consider using full url format %q.", endpoint, fallbackEndpoint)
		}
	}
	return
}

func parseEndpoint(endpoint string) (string, string, error) {
	u, err := url.Parse(endpoint)
	if err != nil {
		return "", "", err
	}

	switch u.Scheme {
	case "tcp":
		return "tcp", u.Host, nil

	case "unix":
		return "unix", u.Path, nil

	case "":
		return "", "", fmt.Errorf("using %q as endpoint is deprecated, please consider using full url format", endpoint)

	default:
		return u.Scheme, "", fmt.Errorf("protocol %q not supported", u.Scheme)
	}
}

// syncSandboxes gets pods metadata from the container manager and updates the sandbox cache.
func (km *KataMonitor) syncSandboxes(sandboxList []string) ([]string, error) {
	runtimeClient, runtimeConn, err := getRuntimeClient(km.runtimeEndpoint)
	if err != nil {
		return sandboxList, err
	}
	defer closeConnection(runtimeConn)

	// TODO: if len(sandboxList) is 1, better we just runtimeClient.PodSandboxStatus(...) targeting the single sandbox
	filter := &pb.PodSandboxFilter{
		State: &pb.PodSandboxStateValue{
			State: pb.PodSandboxState_SANDBOX_READY,
		},
	}

	request := &pb.ListPodSandboxRequest{
		Filter: filter,
	}
	monitorLog.Tracef("ListPodSandboxRequest: %v", request)
	r, err := runtimeClient.ListPodSandbox(context.Background(), request)
	if err != nil {
		return sandboxList, err
	}
	monitorLog.Tracef("ListPodSandboxResponse: %v", r)

	for _, pod := range r.Items {
		for _, sandbox := range sandboxList {
			if pod.Id == sandbox {
				km.sandboxCache.setCRIMetadata(sandbox, sandboxCRIMetadata{
					uid:       pod.Metadata.Uid,
					name:      pod.Metadata.Name,
					namespace: pod.Metadata.Namespace,
				})

				sandboxList = removeFromSandboxList(sandboxList, sandbox)

				monitorLog.WithFields(logrus.Fields{
					"cri-name":      pod.Metadata.Name,
					"cri-namespace": pod.Metadata.Namespace,
					"cri-uid":       pod.Metadata.Uid,
				}).Debugf("Synced KATA POD %s", pod.Id)

				break
			}
		}
	}
	// TODO: here we should mark the sandboxes we failed to retrieve info from: we should try a finite number of times
	// to retrieve their metadata: if we fail resign and remove them from the sanbox cache (with a Warning log).
	return sandboxList, nil
}
