// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import "testing"

func TestParseVsockUDSForward(t *testing.T) {
	tests := []struct {
		name    string
		val     string
		port    uint32
		uds     string
		wantErr bool
	}{
		{
			name: "empty disables",
			val:  "",
			port: 0,
			uds:  "",
		},
		{
			name: "valid",
			val:  "1234:/tmp/foo.sock",
			port: 1234,
			uds:  "/tmp/foo.sock",
		},
		{
			name:    "port must be greater than 1024",
			val:     "1024:/tmp/foo.sock",
			wantErr: true,
		},
		{
			name:    "relative path rejected",
			val:     "1234:tmp/foo.sock",
			wantErr: true,
		},
		{
			name: "path with colons",
			val:  "5000:/tmp/a:b/c.sock",
			port: 5000,
			uds:  "/tmp/a:b/c.sock",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			port, uds, err := ParseVsockUDSForward(tc.val)
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if port != tc.port || uds != tc.uds {
				t.Fatalf("got port=%d uds=%q, want port=%d uds=%q", port, uds, tc.port, tc.uds)
			}
		})
	}
}

func TestParseVsockUDSForwardList(t *testing.T) {
	port, uds, err := ParseVsockUDSForwardList(nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if port != 0 || uds != "" {
		t.Fatalf("got port=%d uds=%q, want disabled", port, uds)
	}

	port, uds, err = ParseVsockUDSForwardList([]string{"1234:/tmp/foo.sock"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if port != 1234 || uds != "/tmp/foo.sock" {
		t.Fatalf("got port=%d uds=%q", port, uds)
	}
}
