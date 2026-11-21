//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"errors"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"k8s.io/apimachinery/pkg/api/resource"
)

// Capacity simulates linear storage of certain types ("fast",
// "slow"). To calculate the amount of allocated space, the size of
// all currently existing volumes of the same kind is summed up.
//
// Available capacity is configurable with a command line flag
// -capacity <type>=<size> where <type> is a string and <size>
// is a quantity (1T, 1Gi). More than one of those
// flags can be used.
//
// The underlying map will be initialized if needed by Set,
// which makes it possible to define and use a Capacity instance
// without explicit initialization (`var capacity Capacity` or as
// member in a struct).
type Capacity map[string]resource.Quantity

// Set is an implementation of flag.Value.Set.
func (c *Capacity) Set(arg string) error {
	parts := strings.SplitN(arg, "=", 2)
	if len(parts) != 2 {
		return errors.New("must be of format <type>=<size>")
	}
	quantity, err := resource.ParseQuantity(parts[1])
	if err != nil {
		return err
	}

	// We overwrite any previous value.
	if *c == nil {
		*c = Capacity{}
	}
	(*c)[parts[0]] = quantity
	return nil
}

func (c *Capacity) String() string {
	return fmt.Sprintf("%v", map[string]resource.Quantity(*c))
}

var _ flag.Value = &Capacity{}

// Enabled returns true if capacities are configured.
func (c *Capacity) Enabled() bool {
	return len(*c) > 0
}

// Kinds returns the list of configured pool names. It is intended
// for error messages; callers must not rely on the order.
func (c *Capacity) Kinds() []string {
	kinds := make([]string, 0, len(*c))
	for kind := range *c {
		kinds = append(kinds, kind)
	}
	return kinds
}

// ParseCapacityConfig parses the per-node capacity configuration
// format: one <pool>=<quantity> entry per item, items separated by
// commas or newlines, lines starting with # are comments, blank
// items are ignored. A pool configured more than once is an error —
// unlike the repeated --capacity flag, where the last value silently
// wins, a config file that contradicts itself signals a mistake
// (#14151, per-node capacity proposal).
func ParseCapacityConfig(content string) (Capacity, error) {
	capacity := Capacity{}
	for _, line := range strings.Split(content, "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		for _, entry := range strings.Split(line, ",") {
			entry = strings.TrimSpace(entry)
			if entry == "" {
				continue
			}
			parts := strings.SplitN(entry, "=", 2)
			if len(parts) != 2 || strings.TrimSpace(parts[0]) == "" {
				return nil, fmt.Errorf("capacity entry %q must be of format <pool>=<quantity>", entry)
			}
			pool := strings.TrimSpace(parts[0])
			quantity, err := resource.ParseQuantity(strings.TrimSpace(parts[1]))
			if err != nil {
				return nil, fmt.Errorf("capacity entry %q: %v", entry, err)
			}
			if _, duplicate := capacity[pool]; duplicate {
				return nil, fmt.Errorf("capacity pool %q is configured more than once", pool)
			}
			capacity[pool] = quantity
		}
	}
	return capacity, nil
}

// LoadNodeCapacity reads the capacity pools of the given node from a
// directory whose entries are named after node ids (the optional
// csi-kata-directvol-capacity ConfigMap mounted by the DaemonSet, or
// any equivalent directory). A node without an entry is not an error
// — it runs in no-pool mode and is only bounded by the free space of
// its backing filesystem.
func LoadNodeCapacity(dir, nodeID string) (Capacity, bool, error) {
	// The node id becomes a file name: reject anything that could
	// escape the directory ("" for a join with the cwd, separators
	// for a traversal, "."/".." and their dotted siblings for hidden
	// or parent entries).
	if nodeID == "" || nodeID != filepath.Base(nodeID) || nodeID == "." || nodeID == ".." || strings.HasPrefix(nodeID, ".") {
		return nil, false, fmt.Errorf("node id %q cannot be used as a capacity file name", nodeID)
	}

	file := filepath.Join(dir, nodeID)
	content, err := os.ReadFile(file)
	if errors.Is(err, os.ErrNotExist) {
		return nil, false, nil
	}
	if err != nil {
		return nil, false, fmt.Errorf("read capacity file %s: %w", file, err)
	}

	capacity, err := ParseCapacityConfig(string(content))
	if err != nil {
		return nil, false, fmt.Errorf("parse capacity file %s: %w", file, err)
	}
	return capacity, true, nil
}
