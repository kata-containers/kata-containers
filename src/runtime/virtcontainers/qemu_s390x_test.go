// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"testing"

	govmmQemu "github.com/kata-containers/govmm/qemu"
	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
)

func newTestQemu(assert *assert.Assertions, machineType string) qemuArch {
	config := HypervisorConfig{
		HypervisorMachineType: machineType,
	}
	arch, err := newQemuArch(config)
	assert.NoError(err)
	return arch
}

func TestQemuS390xCPUModel(t *testing.T) {
	assert := assert.New(t)
	s390x := newTestQemu(assert, QemuCCWVirtio)

	expectedOut := defaultCPUModel
	model := s390x.cpuModel()
	assert.Equal(expectedOut, model)

	s390x.enableNestingChecks()
	expectedOut = defaultCPUModel
	model = s390x.cpuModel()
	assert.Equal(expectedOut, model)
}

func TestQemuS390xMemoryTopology(t *testing.T) {
	assert := assert.New(t)
	s390x := newTestQemu(assert, QemuCCWVirtio)

	hostMem := uint64(1024)
	mem := uint64(120)
	slots := uint8(10)
	expectedMemory := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  slots,
		MaxMem: fmt.Sprintf("%dM", hostMem),
	}

	m := s390x.memoryTopology(mem, hostMem, slots)
	assert.Equal(expectedMemory, m)
}

func TestQemuS390xAppendVhostUserDevice(t *testing.T) {
	assert := assert.New(t)
	qemu := newTestQemu(assert, QemuCCWVirtio)

	// test devices that should not work
	for _, deviceType := range []config.DeviceType{config.VhostUserSCSI, config.VhostUserNet, config.VhostUserBlk} {
		vhostUserDevice := config.VhostUserDeviceAttrs{
			Type: deviceType,
		}
		_, err := qemu.appendVhostUserDevice(context.Background(), nil, vhostUserDevice)
		assert.Error(err)
	}

	// test vhost user fs (virtio-fs)
	socketPath := "nonexistentpath.sock"
	id := "deadbeef"
	tag := "shared"
	var cacheSize uint32 = 0

	expected := []govmmQemu.Device{
		govmmQemu.VhostUserDevice{
			SocketPath:    socketPath,
			CharDevID:     fmt.Sprintf("char-%s", id),
			TypeDevID:     fmt.Sprintf("fs-%s", id),
			Tag:           tag,
			CacheSize:     cacheSize,
			VhostUserType: govmmQemu.VhostUserFS,
			DevNo:         "fe.0.0001",
		},
	}

	vhostUserDevice := config.VhostUserDeviceAttrs{
		DevID:      id,
		SocketPath: socketPath,
		Type:       config.VhostUserFS,
		Tag:        tag,
		CacheSize:  cacheSize,
	}

	var devices []govmmQemu.Device
	devices, err := qemu.appendVhostUserDevice(context.Background(), devices, vhostUserDevice)

	assert.NoError(err)
	assert.Equal(devices, expected)
}
