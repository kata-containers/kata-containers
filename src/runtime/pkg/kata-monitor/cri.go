// Copyright (c) 2020 Ant Group
// Copyright (c) 2021 Red Hat Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"net/url"
	"regexp"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/types"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	"github.com/xeipuuv/gojsonpointer"
	"google.golang.org/grpc"

	pb "k8s.io/cri-api/pkg/apis/runtime/v1alpha2"
)

const (
	// unixProtocol is the network protocol of unix socket.
	unixProtocol = "unix"

	k8sContainerdNamespace = "k8s.io"
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

func matchesRegex(pattern, target string) bool {
	if pattern == "" {
		return true
	}
	matched, err := regexp.MatchString(pattern, target)
	if err != nil {
		// Assume it's not a match if an error occurs.
		return false
	}
	return matched
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

// getSandboxes get kata sandbox from the container engine.
// this will be called only after monitor start.
func (km *KataMonitor) getSandboxes() (map[string]string, error) {

	sandboxMap := map[string]string{}
	runtimeClient, runtimeConn, err := getRuntimeClient(km.runtimeEndpoint)
	if err != nil {
		return sandboxMap, err
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
		return sandboxMap, err
	}
	monitorLog.Debugf("ListPodSandboxResponse: %v", r)

	for _, pod := range r.Items {
		request := &pb.PodSandboxStatusRequest{
			PodSandboxId: pod.Id,
			Verbose:      true,
		}

		r, err := runtimeClient.PodSandboxStatus(context.Background(), request)
		if err != nil {
			return sandboxMap, err
		}

		lowRuntime := ""
		highRuntime := ""
		var res map[string]interface{}
		if err := json.Unmarshal([]byte(r.Info["info"]), &res); err != nil {
			monitorLog.WithError(err).WithField("pod", r).Error("failed to Unmarshal pod info")
			continue
		} else {
			monitorLog.WithField("pod info", res).Debug("")
			// get high level container runtime
			pointer, _ := gojsonpointer.NewJsonPointer("/runtimeSpec/annotations/io.container.manager")
			rt, _, _ := pointer.Get(res)
			if rt != nil {
				if str, ok := rt.(string); ok {
					if str == "cri-o" {
						highRuntime = RuntimeCRIO
					} else {
						highRuntime = RuntimeContainerd
					}
				}
			}

			// get low level container runtime
			// containerd stores the pod runtime in "/runtimeType" while CRI-O stores it the
			// io.kubernetes.cri-o.RuntimeHandler annotation: check for both.
			keys := []string{"/runtimeType", "/runtimeSpec/annotations/io.kubernetes.cri-o.RuntimeHandler"}
			for _, key := range keys {
				pointer, _ := gojsonpointer.NewJsonPointer(key)
				rt, _, _ := pointer.Get(res)
				if rt != nil {
					if str, ok := rt.(string); ok {
						lowRuntime = str
						break
					}
				}
			}
		}

		// Filter by pod name/namespace regular expressions.
		monitorLog.WithFields(logrus.Fields{
			"low runtime":  lowRuntime,
			"high runtime": highRuntime,
		}).Debug("")
		if matchesRegex(types.KataRuntimeNameRegexp, lowRuntime) || matchesRegex("kata*", lowRuntime) {
			sandboxMap[pod.Id] = highRuntime
		}
	}

	return sandboxMap, nil
}
