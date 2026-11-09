//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"k8s.io/apimachinery/pkg/api/resource"
)

func TestParseCapacityConfig(t *testing.T) {
	tests := []struct {
		name     string
		content  string
		expected map[string]string // pool -> expected quantity as string
		errMsg   string            // non-empty: must be contained in the error
	}{
		{
			name:     "single pool",
			content:  "gold=2Gi\n",
			expected: map[string]string{"gold": "2Gi"},
		},
		{
			name:     "comma separated entries",
			content:  "gold=2Gi,silver=8Gi\n",
			expected: map[string]string{"gold": "2Gi", "silver": "8Gi"},
		},
		{
			name:     "newlines and blank lines",
			content:  "gold=2Gi\n\nsilver=8Gi\n\n",
			expected: map[string]string{"gold": "2Gi", "silver": "8Gi"},
		},
		{
			name:     "comment lines and trailing newline are skipped",
			content:  "# per-node pools\ngold=2Gi\n# second comment\nsilver=8Gi\n",
			expected: map[string]string{"gold": "2Gi", "silver": "8Gi"},
		},
		{
			name:     "whitespace around entries and separator",
			content:  "  gold = 2Gi ,  silver = 8Gi  \n",
			expected: map[string]string{"gold": "2Gi", "silver": "8Gi"},
		},
		{
			name:     "empty content",
			content:  "",
			expected: map[string]string{},
		},
		{
			name:    "malformed entry without quantity",
			content: "gold=2Gi\nnovalue\n",
			errMsg:  "must be of format <pool>=<quantity>",
		},
		{
			name:    "entry missing pool name",
			content: "=2Gi\n",
			errMsg:  "must be of format <pool>=<quantity>",
		},
		{
			name:    "invalid quantity",
			content: "gold=2banana\n",
			errMsg:  "capacity entry",
		},
		{
			name:    "duplicate pool on different lines",
			content: "gold=2Gi\ngold=4Gi\n",
			errMsg:  `capacity pool "gold" is configured more than once`,
		},
		{
			name:    "duplicate pool on same line",
			content: "gold=2Gi,gold=4Gi\n",
			errMsg:  `capacity pool "gold" is configured more than once`,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			capacity, err := ParseCapacityConfig(tt.content)
			if tt.errMsg != "" {
				if err == nil {
					t.Fatalf("expected error containing %q, got nil", tt.errMsg)
				}
				if !strings.Contains(err.Error(), tt.errMsg) {
					t.Fatalf("expected error containing %q, got %q", tt.errMsg, err.Error())
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if len(capacity) != len(tt.expected) {
				t.Fatalf("expected %d pools, got %d (%v)", len(tt.expected), len(capacity), capacity)
			}
			for pool, want := range tt.expected {
				quantity, ok := capacity[pool]
				if !ok {
					t.Fatalf("expected pool %q missing, got %v", pool, capacity)
				}
				if quantity.String() != want {
					t.Fatalf("expected pool %q = %s, got %s", pool, want, quantity.String())
				}
			}
		})
	}
}

func TestLoadNodeCapacity(t *testing.T) {
	dir := t.TempDir()

	writeFile := func(name, content string) {
		t.Helper()
		if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0644); err != nil {
			t.Fatalf("write %s: %v", name, err)
		}
	}

	t.Run("entry for this node", func(t *testing.T) {
		writeFile("node1", "gold=2Gi\nsilver=8Gi\n")
		capacity, found, err := LoadNodeCapacity(dir, "node1")
		if err != nil || !found {
			t.Fatalf("expected entry to load: found=%v err=%v", found, err)
		}
		if got := capacity["gold"]; got.Cmp(resource.MustParse("2Gi")) != 0 {
			t.Errorf("expected gold=2Gi, got %s", got.String())
		}
		if got := capacity["silver"]; got.Cmp(resource.MustParse("8Gi")) != 0 {
			t.Errorf("expected silver=8Gi, got %s", got.String())
		}
		if !capacity.Enabled() {
			t.Error("loaded capacity must be Enabled")
		}
	})

	t.Run("no entry for this node", func(t *testing.T) {
		writeFile("node1", "gold=2Gi\n")
		capacity, found, err := LoadNodeCapacity(dir, "node2")
		if err != nil {
			t.Fatalf("missing entry must not be an error: %v", err)
		}
		if found || capacity != nil {
			t.Fatalf("expected (nil, false, nil), got (%v, %v, nil)", capacity, found)
		}
	})

	t.Run("malformed entry for this node", func(t *testing.T) {
		writeFile("node3", "not-an-entry\n")
		_, _, err := LoadNodeCapacity(dir, "node3")
		if err == nil {
			t.Fatal("expected parse error for malformed entry")
		}
	})

	t.Run("invalid node id", func(t *testing.T) {
		for _, nodeID := range []string{
			"",        // empty
			"..",      // parent
			".",       // cwd
			"a/b",     // path separator
			".hidden", // hidden file
		} {
			if _, _, err := LoadNodeCapacity(dir, nodeID); err == nil {
				t.Errorf("node id %q must be rejected as a capacity file name", nodeID)
			}
		}
	})

	t.Run("empty entry means no pools", func(t *testing.T) {
		writeFile("node4", "# comment only\n")
		capacity, found, err := LoadNodeCapacity(dir, "node4")
		if err != nil || !found {
			t.Fatalf("expected (empty, true, nil), got (%v, %v, %v)", capacity, found, err)
		}
		if capacity.Enabled() {
			t.Error("comment-only entry must not enable capacity tracking")
		}
	})
}

func TestLoadNodeCapacityNonexistentDir(t *testing.T) {
	// The ConfigMap is optional; a directory that does not exist yet
	// (or a node without a file in it) must not fail startup.
	capacity, found, err := LoadNodeCapacity(filepath.Join(t.TempDir(), "capacity"), "node1")
	if err != nil {
		t.Fatalf("nonexistent directory must not be an error: %v", err)
	}
	if found || capacity != nil {
		t.Fatalf("expected (nil, false, nil), got (%v, %v, nil)", capacity, found)
	}
}
