// Copyright (c) 2019 Ericsson Eurolab Deutschland G.m.b.H.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"net/http"
	"testing"

	chclient "github.com/kata-containers/runtime/virtcontainers/pkg/cloud-hypervisor/client"
	"github.com/stretchr/testify/assert"
)

type clhClientMock struct {
	vmInfo chclient.VmInfo
}

func (c *clhClientMock) VmmPingGet(ctx context.Context) (chclient.VmmPingResponse, *http.Response, error) {
	return chclient.VmmPingResponse{}, nil, nil
}

func (c *clhClientMock) ShutdownVMM(ctx context.Context) (*http.Response, error) {
	return nil, nil
}

func (c *clhClientMock) CreateVM(ctx context.Context, vmConfig chclient.VmConfig) (*http.Response, error) {
	c.vmInfo.State = clhStateCreated
	return nil, nil
}

//nolint:golint
func (c *clhClientMock) VmInfoGet(ctx context.Context) (chclient.VmInfo, *http.Response, error) {
	return c.vmInfo, nil, nil
}

func (c *clhClientMock) BootVM(ctx context.Context) (*http.Response, error) {
	c.vmInfo.State = clhStateRunning
	return nil, nil
}

func TestCloudHypervisorAddVSock(t *testing.T) {
	assert := assert.New(t)
	clh := cloudHypervisor{}

	clh.addVSock(1, "path")
	assert.Equal(clh.vmconfig.Vsock[0].Cid, int64(1))
	assert.Equal(clh.vmconfig.Vsock[0].Sock, "path")

	clh.addVSock(2, "path2")
	assert.Equal(clh.vmconfig.Vsock[1].Cid, int64(2))
	assert.Equal(clh.vmconfig.Vsock[1].Sock, "path2")
}

func TestCloudHypervisorBootVM(t *testing.T) {
	clh := &cloudHypervisor{}
	clh.APIClient = &clhClientMock{}
	var ctx context.Context
	if err := clh.bootVM(ctx); err != nil {
		t.Errorf("cloudHypervisor.bootVM() error = %v", err)
	}
}
