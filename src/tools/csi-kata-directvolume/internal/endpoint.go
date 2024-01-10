//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package endpoint

import (
	"fmt"
	"net"
	"os"
	"strings"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

func Parse(ep string) (string, string, error) {
	if strings.HasPrefix(strings.ToLower(ep), "unix://") || strings.HasPrefix(strings.ToLower(ep), "tcp://") {
		s := strings.SplitN(ep, "://", 2)
		if s[1] != "" {
			return s[0], s[1], nil
		}
		return "", "", status.Error(codes.InvalidArgument, fmt.Sprintf("Invalid endpoint: %v", ep))
	}

	return "unix", ep, nil
}

func Listen(endpoint string) (net.Listener, func(), error) {
	proto, addr, err := Parse(endpoint)
	if err != nil {
		return nil, nil, err
	}

	cleanup := func() {}
	if proto == "unix" {
		addr = "/" + addr
		if err := os.Remove(addr); err != nil && !os.IsNotExist(err) {
			return nil, nil, status.Error(codes.Internal, fmt.Sprintf("%s: %q", addr, err))
		}
		cleanup = func() {
			os.Remove(addr)
		}
	}

	l, err := net.Listen(proto, addr)
	return l, cleanup, err
}
