// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"fmt"
	"path/filepath"
	"strconv"
	"strings"
)

const minVsockUDSForwardPort = 1025

// ParseVsockUDSForward parses runtime configuration value "port:/absolute/unix/path".
// An empty value disables the feature (returns port 0 and empty uds).
func ParseVsockUDSForward(val string) (port uint32, uds string, err error) {
	val = strings.TrimSpace(val)
	if val == "" {
		return 0, "", nil
	}

	parts := strings.SplitN(val, ":", 2)
	if len(parts) != 2 {
		return 0, "", fmt.Errorf("%q: expected port:/absolute/unix/path", val)
	}

	p, err := strconv.ParseUint(parts[0], 10, 32)
	if err != nil {
		return 0, "", fmt.Errorf("%q: invalid port: %w", parts[0], err)
	}
	if p < minVsockUDSForwardPort {
		return 0, "", fmt.Errorf("port %d must be greater than 1024", p)
	}

	uds = parts[1]
	if uds == "" {
		return 0, "", fmt.Errorf("unix socket path must not be empty")
	}
	if !filepath.IsAbs(uds) {
		return 0, "", fmt.Errorf("unix socket path must be absolute: %q", uds)
	}

	return uint32(p), uds, nil
}

// ParseVsockUDSForwardList parses the first vsock_uds_forward list entry.
// An empty list disables the feature. Additional entries are ignored.
func ParseVsockUDSForwardList(vals []string) (port uint32, uds string, err error) {
	if len(vals) == 0 {
		return 0, "", nil
	}

	return ParseVsockUDSForward(vals[0])
}
