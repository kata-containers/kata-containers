// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"net"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNetInterworkingModelIsValid(t *testing.T) {
	tests := []struct {
		name string
		n    NetInterworkingModel
		want bool
	}{
		{"Invalid Model", NetXConnectInvalidModel, false},
		{"Default Model", NetXConnectDefaultModel, true},
		{"TC Filter Model", NetXConnectTCFilterModel, true},
		{"Macvtap Model", NetXConnectMacVtapModel, true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.n.IsValid(); got != tt.want {
				t.Errorf("NetInterworkingModel.IsValid() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestNetInterworkingModelSetModel(t *testing.T) {
	var n NetInterworkingModel
	tests := []struct {
		name      string
		modelName string
		wantErr   bool
	}{
		{"Invalid Model", "Invalid", true},
		{"default Model", defaultNetModelStr, false},
		{"macvtap Model", macvtapNetModelStr, false},
		{"tcfilter Model", tcFilterNetModelStr, false},
		{"none Model", noneNetModelStr, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if err := n.SetModel(tt.modelName); (err != nil) != tt.wantErr {
				t.Errorf("NetInterworkingModel.SetModel() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestGenerateRandomPrivateMacAdd(t *testing.T) {
	assert := assert.New(t)

	addr1, err := generateRandomPrivateMacAddr()
	assert.NoError(err)

	_, err = net.ParseMAC(addr1)
	assert.NoError(err)

	addr2, err := generateRandomPrivateMacAddr()
	assert.NoError(err)

	_, err = net.ParseMAC(addr2)
	assert.NoError(err)

	assert.NotEqual(addr1, addr2)
}
