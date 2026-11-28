//go:build linux

// Copyright 2026 GRG
// SPDX-License-Identifier: Apache-2.0

package resourcecontrol

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/opencontainers/runtime-spec/specs-go"
)

func TestSystemdSandboxReturnsConnectionError(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip("root uses systemd's private socket instead of DBUS_SYSTEM_BUS_ADDRESS")
	}

	socket := filepath.Join(t.TempDir(), "missing-system-bus")
	t.Setenv("DBUS_SYSTEM_BUS_ADDRESS", "unix:path="+socket)
	_, err := NewSandboxResourceController("system.slice:kata-test:missing-bus", &specs.LinuxResources{}, true, false)
	if err == nil || !strings.Contains(err.Error(), socket) {
		t.Fatalf("got %v, want the systemd connection error for %s", err, socket)
	}
}
