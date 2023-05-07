//go:build linux

// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"strings"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func TestFCGenerateSocket(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	i, err := fc.GenerateSocket("a")
	assert.NoError(err)
	assert.NotNil(i)

	hvsock, ok := i.(types.HybridVSock)
	assert.True(ok)
	assert.NotEmpty(hvsock.UdsPath)

	// Path must be absolute
	assert.True(strings.HasPrefix(hvsock.UdsPath, "/"))

	assert.NotZero(hvsock.Port)
}

func TestFCTruncateID(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}

	testLongID := "3ef98eb7c6416be11e0accfed2f4e6560e07f8e33fa8d31922fd4d61747d7ead"
	expectedID := "3ef98eb7c6416be11e0accfed2f4e656"
	id := fc.truncateID(testLongID)
	assert.Equal(expectedID, id)

	testShortID := "3ef98eb7c6416be11"
	expectedID = "3ef98eb7c6416be11"
	id = fc.truncateID(testShortID)
	assert.Equal(expectedID, id)
}

func TestFCParseVersion(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}

	// correct versions
	for rawVersion, v := range map[string]string{"Firecracker v0.23.1": "0.23.1", "Firecracker v0.25.0\nSupported snapshot data format versions: 0.23.0": "0.25.0"} {
		parsedVersion, err := fc.parseVersion(rawVersion)
		assert.NoError(err)
		assert.Equal(parsedVersion, v)
	}

	// wrong version str
	rawVersion := "Firecracker_v0.23.0"
	parsedVersion, err := fc.parseVersion(rawVersion)
	assert.Error(err)
	assert.Equal(parsedVersion, "")
}

func TestFCCheckVersion(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}

	// correct version
	v := "0.23.0"
	err := fc.checkVersion(v)
	assert.NoError(err)

	// version too low
	v = "0.1.1"
	err = fc.checkVersion(v)
	assert.Error(err)
	b := err.Error()
	assert.True(strings.Contains(b, "version 0.1.1 is not supported")) // sanity

	// version is malformed
	v = "Firecracker v0.23.0"
	err = fc.checkVersion(v)
	assert.Error(err)
	b = err.Error()
	assert.True(strings.Contains(b, "Malformed firecracker version:")) // sanity
}

func TestFCGetVersionNumber(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	_, err := fc.getVersionNumber()
	assert.Error(err)
}

func TestFCDriveIndexToID(t *testing.T) {
	assert := assert.New(t)

	d := fcDriveIndexToID(5)
	assert.Equal(d, "drive_5")
}

func TestFCPauseVM(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	ctx := context.Background()
	err := fc.PauseVM(ctx)
	assert.NoError(err)
}

func TestFCSaveVM(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	err := fc.SaveVM()
	assert.NoError(err)
}

func TestFCResumeVM(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	ctx := context.Background()
	err := fc.ResumeVM(ctx)
	assert.NoError(err)
}

func TestFCGetVirtioFsPid(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	pid := fc.GetVirtioFsPid()
	assert.Nil(pid)
}

func TestFCIsRateLimiterBuiltin(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	rl := fc.IsRateLimiterBuiltin()
	assert.True(rl)
}

func TestFCCheck(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	err := fc.Check()
	assert.NoError(err)
}

func TestFCGetPids(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	pids := fc.GetPids()
	assert.Equal(len(pids), 1)
}

func TestFCCleanup(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	ctx := context.Background()
	err := fc.Cleanup(ctx)
	assert.NoError(err)
}

func TestFCToGrpc(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	ctx := context.Background()
	_, err := fc.toGrpc(ctx)
	assert.Error(err)
}

func TestFCHypervisorConfig(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	config := fc.HypervisorConfig()
	assert.Equal(fc.config, config)
}

func TestFCGetTotalMemoryMB(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	ctx := context.Background()

	var initialMemSize uint32 = 1024

	fc.config.MemorySize = 1024
	memSize := fc.GetTotalMemoryMB(ctx)
	assert.Equal(memSize, initialMemSize)
}

func TestFCClient(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	ctx := context.Background()

	conn := fc.client(ctx)
	assert.Equal(conn, fc.connection)
}

func TestFCVmRunning(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	ctx := context.Background()
	sr := fc.vmRunning(ctx)
	assert.False(sr)
}

func TestFCCreateJailedDrive(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}

	driveID := fcDriveIndexToID(0)
	_, err := fc.createJailedDrive(driveID)
	assert.NoError(err)
}

func TestFcSetConfig(t *testing.T) {
	assert := assert.New(t)

	config := HypervisorConfig{
		HypervisorPath: "/some/where/firecracker",
		KernelPath:     "/some/where/kernel",
		ImagePath:      "/some/where/image",
		JailerPath:     "/some/where/jailer",
		Debug:          true,
	}

	fc := firecracker{}

	assert.Equal(fc.config, HypervisorConfig{})

	err := fc.setConfig(&config)
	assert.NoError(err)

	assert.Equal(fc.config, config)
}
