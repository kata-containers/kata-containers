// Copyright (c) 2026 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"encoding/json"
	"testing"

	bootapi "github.com/containerd/containerd/api/runtime/bootstrap/v1"
	"github.com/stretchr/testify/assert"
)

// legacyResult mirrors the JSON structure that CRI-O's ParseShimAddress and
// containerd <= 2.2 expect on stdout from a shim start call.
type legacyResult struct {
	Version  int    `json:"version"`
	Address  string `json:"address"`
	Protocol string `json:"protocol"`
}

func TestLegacyBootstrapJSON(t *testing.T) {
	const addr = "unix:///run/containerd/s/abc123"

	tests := []struct {
		name              string
		containerdVersion string
		wantLegacy        bool
		wantVersion       int
		wantAddress       string
		wantProtocol      string
	}{
		{
			name:              "empty ContainerdVersion triggers legacy path",
			containerdVersion: "",
			wantLegacy:        true,
			wantVersion:       2,
			wantAddress:       addr,
			wantProtocol:      "ttrpc",
		},
		{
			name:              "non-empty ContainerdVersion skips legacy path",
			containerdVersion: "v2.3.6",
			wantLegacy:        false,
		},
		{
			// The detection only checks for a non-empty value, not the
			// version itself. Real containerd <= 2.2 never reaches this
			// case: it leaves ContainerdVersion empty, like the first
			// case above.
			name:              "hypothetical version string takes the modern path",
			containerdVersion: "v2.2.0",
			wantLegacy:        false,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			params := &bootapi.BootstrapParams{
				ContainerdVersion: tc.containerdVersion,
			}

			data, legacy := legacyBootstrapJSON(params, addr)

			assert.Equal(t, tc.wantLegacy, legacy)

			if !legacy {
				assert.Nil(t, data)
				return
			}

			// Verify the JSON round-trips correctly into what CRI-O expects.
			var got legacyResult
			err := json.Unmarshal(data, &got)
			assert.NoError(t, err)
			assert.Equal(t, tc.wantVersion, got.Version)
			assert.Equal(t, tc.wantAddress, got.Address)
			assert.Equal(t, tc.wantProtocol, got.Protocol)
		})
	}
}
