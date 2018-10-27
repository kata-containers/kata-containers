// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
)

func getCgroupDestination(subsystem string) (string, error) {
	f, err := os.Open("/proc/self/mountinfo")
	if err != nil {
		return "", err
	}
	defer f.Close()
	s := bufio.NewScanner(f)
	for s.Scan() {
		if err := s.Err(); err != nil {
			return "", err
		}
		fields := strings.Fields(s.Text())
		for _, opt := range strings.Split(fields[len(fields)-1], ",") {
			if opt == subsystem {
				return fields[4], nil
			}
		}
	}
	return "", fmt.Errorf("failed to find cgroup mountpoint for %q", subsystem)
}

func TestMergeSpecResource(t *testing.T) {
	s := &Sandbox{
		config: &SandboxConfig{
			Containers: []ContainerConfig{
				{
					ID:          "containerA",
					Annotations: make(map[string]string),
				},
				{
					ID:          "containerA",
					Annotations: make(map[string]string),
				},
			},
		},
	}

	contA := s.config.Containers[0]
	contB := s.config.Containers[1]

	getIntP := func(x int64) *int64 { return &x }
	getUintP := func(x uint64) *uint64 { return &x }

	type testData struct {
		first    *specs.LinuxResources
		second   *specs.LinuxResources
		expected *specs.LinuxResources
	}

	for _, testdata := range []testData{
		{
			nil,
			nil,
			&specs.LinuxResources{CPU: &specs.LinuxCPU{}},
		},
		{
			nil,
			&specs.LinuxResources{},
			&specs.LinuxResources{CPU: &specs.LinuxCPU{}},
		},
		{
			&specs.LinuxResources{CPU: &specs.LinuxCPU{Quota: getIntP(0), Period: getUintP(100000)}},
			&specs.LinuxResources{CPU: &specs.LinuxCPU{Quota: getIntP(20000), Period: getUintP(100000)}},
			&specs.LinuxResources{CPU: &specs.LinuxCPU{Quota: getIntP(20000), Period: getUintP(100000)}},
		},
		{
			&specs.LinuxResources{CPU: &specs.LinuxCPU{Quota: getIntP(10000), Period: getUintP(0)}},
			&specs.LinuxResources{CPU: &specs.LinuxCPU{Quota: getIntP(20000), Period: getUintP(100000)}},
			&specs.LinuxResources{CPU: &specs.LinuxCPU{Quota: getIntP(20000), Period: getUintP(100000)}},
		},
		{
			&specs.LinuxResources{CPU: &specs.LinuxCPU{Quota: getIntP(1000), Period: getUintP(2000)}},
			&specs.LinuxResources{CPU: &specs.LinuxCPU{Quota: getIntP(20000), Period: getUintP(100000)}},
			&specs.LinuxResources{CPU: &specs.LinuxCPU{Quota: getIntP(1400), Period: getUintP(2000)}},
		},
	} {
		data, err := json.Marshal(&specs.Spec{
			Linux: &specs.Linux{
				Resources: testdata.first,
			},
		})
		assert.Nil(t, err)
		contA.Annotations[annotations.ConfigJSONKey] = string(data)

		data, err = json.Marshal(&specs.Spec{
			Linux: &specs.Linux{
				Resources: testdata.second,
			},
		})
		assert.Nil(t, err)
		contB.Annotations[annotations.ConfigJSONKey] = string(data)

		rc, err := s.mergeSpecResource()
		assert.Nil(t, err)
		assert.True(t, reflect.DeepEqual(rc, testdata.expected), "should be equal, got: %#v, expected: %#v", rc, testdata.expected)
	}
}

func TestSetupCgroups(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip("Test disabled as requires root privileges")
	}

	s := &Sandbox{
		id:         "test-sandbox",
		hypervisor: &mockHypervisor{},
		config: &SandboxConfig{
			Containers: []ContainerConfig{
				{
					ID:          "containerA",
					Annotations: make(map[string]string),
				},
				{
					ID:          "containerA",
					Annotations: make(map[string]string),
				},
			},
		},
	}

	contA := s.config.Containers[0]
	contB := s.config.Containers[1]

	getIntP := func(x int64) *int64 { return &x }
	getUintP := func(x uint64) *uint64 { return &x }

	data, err := json.Marshal(&specs.Spec{
		Linux: &specs.Linux{
			Resources: &specs.LinuxResources{
				CPU: &specs.LinuxCPU{
					Quota:  getIntP(5000),
					Period: getUintP(10000),
				},
			},
		},
	})
	assert.Nil(t, err)
	contA.Annotations[annotations.ConfigJSONKey] = string(data)

	data, err = json.Marshal(&specs.Spec{
		Linux: &specs.Linux{
			Resources: &specs.LinuxResources{
				CPU: &specs.LinuxCPU{
					Quota:  getIntP(10000),
					Period: getUintP(40000),
				},
			},
		},
	})
	assert.Nil(t, err)
	contB.Annotations[annotations.ConfigJSONKey] = string(data)

	err = s.newCgroups()
	assert.Nil(t, err, "failed to create cgroups")

	defer s.destroyCgroups()

	// test if function works without error
	err = s.setupCgroups()
	assert.Nil(t, err, "setup host cgroup failed")

	// test if the quota and period value are written into cgroup files
	cpu, err := getCgroupDestination("cpu")
	assert.Nil(t, err, "failed to get cpu cgroup path")
	assert.NotEqual(t, "", cpu, "cpu cgroup value can't be empty")

	parentDir := filepath.Join(cpu, defaultCgroupParent, "test-sandbox", "vcpu")
	quotaFile := filepath.Join(parentDir, "cpu.cfs_quota_us")
	periodFile := filepath.Join(parentDir, "cpu.cfs_period_us")

	expectedQuota := "7500\n"
	expectedPeriod := "10000\n"

	fquota, err := os.Open(quotaFile)
	assert.Nil(t, err, "open file %q failed", quotaFile)
	defer fquota.Close()
	data, err = ioutil.ReadAll(fquota)
	assert.Nil(t, err)
	assert.Equal(t, expectedQuota, string(data), "failed to get expected cfs_quota")

	fperiod, err := os.Open(periodFile)
	assert.Nil(t, err, "open file %q failed", periodFile)
	defer fperiod.Close()
	data, err = ioutil.ReadAll(fperiod)
	assert.Nil(t, err)
	assert.Equal(t, expectedPeriod, string(data), "failed to get expected cfs_period")
}
