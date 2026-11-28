//go:build linux

// Copyright 2026 The Kata Containers Authors.
// SPDX-License-Identifier: Apache-2.0

package resourcecontrol

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	specs "github.com/opencontainers/runtime-spec/specs-go"
)

// This test moves only its own process into a temporary systemd scope.
// Run it separately as root on a cgroup v2 host with systemd.
func TestSystemdSandboxAppliesKernelResourceLimits(t *testing.T) {
	if os.Getenv("KATA_TEST_SYSTEMD") != "1" || os.Geteuid() != 0 {
		t.Skip("requires root and KATA_TEST_SYSTEMD=1 on a systemd host")
	}
	readPath := func() string {
		data, err := os.ReadFile("/proc/self/cgroup")
		if err != nil {
			t.Fatal(err)
		}
		for _, line := range strings.Split(string(data), "\n") {
			if strings.HasPrefix(line, "0::/") {
				return strings.TrimPrefix(line, "0::")
			}
		}
		t.Fatal("cgroup v2 is required")
		return ""
	}
	original := readPath()
	quota, period, memory := int64(125000), uint64(100000), int64(2684354560)
	resources := &specs.LinuxResources{
		Pids:   &specs.LinuxPids{Limit: 512},
		CPU:    &specs.LinuxCPU{Quota: &quota, Period: &period},
		Memory: &specs.LinuxMemory{Limit: &memory},
	}
	identity := fmt.Sprintf("system.slice:kata-containment-test:%d-%d", os.Getpid(), time.Now().UnixNano())
	controller, err := NewSandboxResourceController(identity, resources, true, false)
	if err != nil {
		t.Fatal(err)
	}
	owned := readPath()
	defer func() {
		if err := controller.MoveTo(original); err != nil {
			t.Errorf("return to original leaf: %v", err)
			return
		}
		err := controller.Delete()
		_, pathErr := os.Stat(filepath.Join("/sys/fs/cgroup", owned))
		if err != nil && !os.IsNotExist(pathErr) {
			t.Errorf("delete owned scope: %v", err)
		}
		if !os.IsNotExist(pathErr) {
			t.Error("owned systemd scope remains")
		}
	}()
	if !strings.HasPrefix(filepath.Base(owned), "kata-containment-test-") {
		t.Fatal("systemd did not place this fixture in its own scope")
	}
	if !controller.(*LinuxCgroup).sandboxCgroupOnly {
		t.Error("new systemd scope lost its cleanup ownership mode")
	}
	for name, expected := range map[string]string{"pids.max": "512", "cpu.max": "125000 100000", "memory.max": "2684354560"} {
		data, err := os.ReadFile(filepath.Join("/sys/fs/cgroup", owned, name))
		if err != nil || strings.TrimSpace(string(data)) != expected {
			t.Errorf("%s: got %q, expected %s, error %v", name, data, expected, err)
		}
	}
}
