// Copyright (c) 2026 Haiyu Wang
//
// SPDX-License-Identifier: Apache-2.0
//

package shimclient

import (
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
	"time"

	shim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestDoGet(t *testing.T) {
	// Exercise the public client and its Rust-socket fallback without a VM.
	// The production socket layout requires creating directories under /run.
	if os.Geteuid() != 0 {
		t.Skip("requires root to create a sandbox management socket under /run/kata")
	}
	require.NoError(t, os.MkdirAll(shim.GetSandboxesStoragePathRust(), 0755))

	testCases := []struct {
		name     string
		status   int
		response string
	}{
		{name: "ok", status: http.StatusOK, response: "*filter\nCOMMIT\n"},
		{name: "empty_ok", status: http.StatusOK},
		{name: "binary_ok", status: http.StatusOK, response: "\x00\xffiptables\n"},
		{name: "unexpected_created", status: http.StatusCreated, response: "unexpected status"},
		{name: "unexpected_no_content", status: http.StatusNoContent},
		{name: "forbidden", status: http.StatusForbidden, response: "blocked by policy"},
		{name: "not_found", status: http.StatusNotFound, response: "URL NOT FOUND"},
		{name: "internal_error", status: http.StatusInternalServerError, response: "Internal server error\n"},
		{name: "unavailable", status: http.StatusServiceUnavailable, response: "agent unavailable"},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			dir, err := os.MkdirTemp(shim.GetSandboxesStoragePathRust(), "shimclient-get-")
			require.NoError(t, err)
			t.Cleanup(func() {
				require.NoError(t, os.RemoveAll(dir))
			})
			sandboxID := filepath.Base(dir)
			_, err = os.Stat(shim.SocketPathGo(sandboxID))
			require.ErrorIs(t, err, os.ErrNotExist)

			listener, err := net.Listen("unix", shim.SocketPathRust(sandboxID))
			require.NoError(t, err)
			t.Cleanup(func() {
				_ = listener.Close()
			})
			server := httptest.NewUnstartedServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				assert.Equal(t, http.MethodGet, r.Method)
				assert.Equal(t, shim.IPTablesURL, r.URL.Path)
				w.WriteHeader(tc.status)
				if tc.response != "" {
					_, err := io.WriteString(w, tc.response)
					assert.NoError(t, err)
				}
			}))
			require.NoError(t, server.Listener.Close())
			server.Listener = listener
			server.Start()
			t.Cleanup(server.Close)

			data, err := DoGet(sandboxID, 5*time.Second, shim.IPTablesURL)
			if tc.status != http.StatusOK {
				require.Error(t, err)
				assert.Nil(t, data)
				assert.Contains(t, err.Error(), "url: "+shim.IPTablesURL)
				assert.Contains(t, err.Error(), fmt.Sprintf("status code: %d", tc.status))
				if tc.response != "" {
					assert.Contains(t, err.Error(), tc.response)
				}
				return
			}

			require.NoError(t, err)
			assert.Equal(t, []byte(tc.response), data)
		})
	}
}
