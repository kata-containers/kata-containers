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

	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	"google.golang.org/grpc"

	pb "k8s.io/cri-api/pkg/apis/runtime/v1alpha2"
)

const (
	// unixProtocol is the network protocol of unix socket.
	unixProtocol = "unix"
)

// getAddressAndDialer returns the address parsed from the given endpoint and a context dialer.
func getAddressAndDialer(endpoint string) (string, func(ctx context.Context, addr string) (net.Conn, error), error) {
	protocol, addr, err := parseEndpointWithFallbackProtocol(endpoint, unixProtocol)
	if err != nil {
		return "", nil, err
	}
	if protocol != unixProtocol {
		return "", nil, fmt.Errorf("only support unix socket endpoint")
	}

	return addr, dial, nil
}

func getConnection(endPoint string) (*grpc.ClientConn, error) {
	var conn *grpc.ClientConn
	monitorLog.Debugf("connect using endpoint '%s' with '%s' timeout", endPoint, defaultTimeout)
	addr, dialer, err := getAddressAndDialer(endPoint)
	if err != nil {
		return nil, err
	}
	ctx, cancel := context.WithTimeout(context.Background(), defaultTimeout)
	defer cancel()
	conn, err = grpc.DialContext(ctx, addr, grpc.WithInsecure(), grpc.WithBlock(), grpc.WithContextDialer(dialer))
	if err != nil {
		errMsg := errors.Wrapf(err, "connect endpoint '%s', make sure you are running as root and the endpoint has been started", endPoint)
		return nil, errMsg
	}
	monitorLog.Debugf("connected successfully using endpoint: %s", endPoint)
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

func dial(ctx context.Context, addr string) (net.Conn, error) {
	return (&net.Dialer{}).DialContext(ctx, unixProtocol, addr)
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

// getSandboxes gets ready sandboxes from the container engine and returns an updated sandboxMap
func (km *KataMonitor) getSandboxes(sandboxMap map[string]bool) (map[string]bool, error) {
	newMap := make(map[string]bool)
	runtimeClient, runtimeConn, err := getRuntimeClient(km.runtimeEndpoint)
	if err != nil {
		return newMap, err
	}
	defer closeConnection(runtimeConn)

	filter := &pb.PodSandboxFilter{
		State: &pb.PodSandboxStateValue{
			State: pb.PodSandboxState_SANDBOX_READY,
		},
	}

	request := &pb.ListPodSandboxRequest{
		Filter: filter,
	}
	monitorLog.Debugf("ListPodSandboxRequest: %v", request)
	r, err := runtimeClient.ListPodSandbox(context.Background(), request)
	if err != nil {
		return newMap, err
	}
	monitorLog.Debugf("ListPodSandboxResponse: %v", r)

	for _, pod := range r.Items {
		// Use the cached data if available
		if isKata, ok := sandboxMap[pod.Id]; ok {
			newMap[pod.Id] = isKata
			continue
		}

		// Check if a directory associated with the POD ID exist on the kata fs:
		// if so we know that the POD is a kata one.
		newMap[pod.Id] = checkSandboxFSExists(pod.Id)
		monitorLog.WithFields(logrus.Fields{
			"id":      pod.Id,
			"is kata": newMap[pod.Id],
			"pod":     pod,
		}).Debug("")
	}

	return newMap, nil
}
