// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"). You may
// not use this file except in compliance with the License. A copy of the
// License is located at
//
//	http://aws.amazon.com/apache2.0/
//
// or in the "license" file accompanying this file. This file is distributed
// on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either
// express or implied. See the License for the specific language governing
// permissions and limitations under the License.

package firecracker

import (
	"context"
	"net"
	"net/http"
	"time"

	"github.com/go-openapi/strfmt"
	"github.com/sirupsen/logrus"

	"github.com/firecracker-microvm/firecracker-go-sdk/client"
	models "github.com/firecracker-microvm/firecracker-go-sdk/client/models"
	ops "github.com/firecracker-microvm/firecracker-go-sdk/client/operations"
	httptransport "github.com/go-openapi/runtime/client"
)

const firecrackerRequestTimeout = 500 * time.Millisecond

// FirecrackerClient is a client for interacting with the Firecracker API
type FirecrackerClient struct {
	client *client.Firecracker
}

// NewFirecrackerClient creates a FirecrackerClient
func NewFirecrackerClient(socketPath string, logger *logrus.Entry, debug bool) *FirecrackerClient {
	httpClient := client.NewHTTPClient(strfmt.NewFormats())

	socketTransport := &http.Transport{
		DialContext: func(ctx context.Context, network, path string) (net.Conn, error) {
			addr, err := net.ResolveUnixAddr("unix", socketPath)
			if err != nil {
				return nil, err
			}

			return net.DialUnix("unix", nil, addr)
		},
	}

	transport := httptransport.New(client.DefaultHost, client.DefaultBasePath, client.DefaultSchemes)
	transport.Transport = socketTransport

	if debug {
		transport.SetDebug(debug)
	}

	if logger != nil {
		transport.SetLogger(logger)
	}

	httpClient.SetTransport(transport)

	return &FirecrackerClient{client: httpClient}
}

func (f *FirecrackerClient) PutLogger(ctx context.Context, logger *models.Logger) (*ops.PutLoggerNoContent, error) {
	timeout, cancel := context.WithTimeout(ctx, firecrackerRequestTimeout)
	defer cancel()

	loggerParams := ops.NewPutLoggerParamsWithContext(timeout)
	loggerParams.SetBody(logger)

	return f.client.Operations.PutLogger(loggerParams)
}

func (f *FirecrackerClient) PutMachineConfiguration(ctx context.Context, cfg *models.MachineConfiguration) (*ops.PutMachineConfigurationNoContent, error) {
	timeout, cancel := context.WithTimeout(ctx, firecrackerRequestTimeout)
	defer cancel()

	mc := ops.NewPutMachineConfigurationParamsWithContext(timeout)
	mc.SetBody(cfg)

	return f.client.Operations.PutMachineConfiguration(mc)
}

func (f *FirecrackerClient) PutGuestBootSource(ctx context.Context, source *models.BootSource) (*ops.PutGuestBootSourceNoContent, error) {
	timeout, cancel := context.WithTimeout(ctx, firecrackerRequestTimeout)
	defer cancel()

	bootSource := ops.NewPutGuestBootSourceParamsWithContext(timeout)
	bootSource.SetBody(source)

	return f.client.Operations.PutGuestBootSource(bootSource)
}

func (f *FirecrackerClient) PutGuestNetworkInterfaceByID(ctx context.Context, ifaceID string, ifaceCfg *models.NetworkInterface) (*ops.PutGuestNetworkInterfaceByIDNoContent, error) {
	timeout, cancel := context.WithTimeout(ctx, firecrackerRequestTimeout)
	defer cancel()

	cfg := ops.NewPutGuestNetworkInterfaceByIDParamsWithContext(timeout)
	cfg.SetBody(ifaceCfg)
	cfg.SetIfaceID(ifaceID)

	return f.client.Operations.PutGuestNetworkInterfaceByID(cfg)
}

func (f *FirecrackerClient) PutGuestDriveByID(ctx context.Context, driveID string, drive *models.Drive) (*ops.PutGuestDriveByIDNoContent, error) {
	timeout, cancel := context.WithTimeout(ctx, 250*time.Millisecond)
	defer cancel()

	params := ops.NewPutGuestDriveByIDParamsWithContext(timeout)
	params.SetDriveID(driveID)
	params.SetBody(drive)

	return f.client.Operations.PutGuestDriveByID(params)
}

func (f *FirecrackerClient) PutGuestVsockByID(ctx context.Context, vsockID string, vsock *models.Vsock) (*ops.PutGuestVsockByIDCreated, *ops.PutGuestVsockByIDNoContent, error) {
	params := ops.NewPutGuestVsockByIDParams()
	params.SetContext(ctx)
	params.SetID(vsockID)
	params.SetBody(vsock)
	return f.client.Operations.PutGuestVsockByID(params)
}

func (f *FirecrackerClient) CreateSyncAction(ctx context.Context, info *models.InstanceActionInfo) (*ops.CreateSyncActionNoContent, error) {
	params := ops.NewCreateSyncActionParams()
	params.SetContext(ctx)
	params.SetInfo(info)

	return f.client.Operations.CreateSyncAction(params)
}

func (f *FirecrackerClient) PutMmds(ctx context.Context, metadata interface{}) (*ops.PutMmdsNoContent, error) {
	params := ops.NewPutMmdsParams()
	params.SetContext(ctx)
	params.SetBody(metadata)

	return f.client.Operations.PutMmds(params)
}

func (f *FirecrackerClient) GetMachineConfig() (*ops.GetMachineConfigOK, error) {
	p := ops.NewGetMachineConfigParams()
	p.SetTimeout(firecrackerRequestTimeout)

	return f.client.Operations.GetMachineConfig(p)
}
