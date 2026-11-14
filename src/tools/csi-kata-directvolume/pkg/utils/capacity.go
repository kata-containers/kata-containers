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
	"sort"
	"strings"

	"k8s.io/apimachinery/pkg/api/resource"
	"k8s.io/apimachinery/pkg/util/validation"
)

// Capacity holds the logical size of every capacity pool configured on a
// node, keyed by pool name. A volume is reserved from the pool named by the
// KataContainersDirectCapacityPool StorageClass parameter, and the space
// still available in a pool is its size minus the sizes of all volumes that
// are reserved from it.
//
// Pools are configured with the command line flag
// --capacity <pool>=<size>, where <size> is a quantity such as 1Ti or
// 500Gi. The flag can be repeated to configure several pools. Per-node
// values can be loaded from a file with LoadNodeCapacity and merged in
// with Merge.
//
// The underlying map will be initialized if needed by Set,
// which makes it possible to define and use a Capacity instance
// without explicit initialization (`var capacity Capacity` or as
// member in a struct).
type Capacity map[string]resource.Quantity

// Set is an implementation of flag.Value.Set.
func (c *Capacity) Set(arg string) error {
	pool, quantity, err := parseCapacityEntry(arg)
	if err != nil {
		return err
	}

	// We overwrite any previous value.
	if *c == nil {
		*c = Capacity{}
	}
	(*c)[pool] = quantity
	return nil
}

// String lists the pools as "<pool>=<size>" entries, sorted by pool name.
func (c Capacity) String() string {
	pools := make([]string, 0, len(c))
	for pool := range c {
		pools = append(pools, pool)
	}
	sort.Strings(pools)
	entries := make([]string, 0, len(pools))
	for _, pool := range pools {
		quantity := c[pool]
		entries = append(entries, fmt.Sprintf("%s=%s", pool, quantity.String()))
	}
	return strings.Join(entries, ",")
}

var _ flag.Value = &Capacity{}

// Enabled returns true if capacities are configured.
func (c Capacity) Enabled() bool {
	return len(c) > 0
}

// Merge returns a new Capacity which contains all pools of c, with the
// pools of override added or replacing those of c.
func (c Capacity) Merge(override Capacity) Capacity {
	merged := make(Capacity, len(c)+len(override))
	for pool, quantity := range c {
		merged[pool] = quantity
	}
	for pool, quantity := range override {
		merged[pool] = quantity
	}
	return merged
}

// ValidateCapacityPoolName checks that name can be used as a capacity pool
// name. Pool names follow the syntax of Kubernetes label values, so that
// they can be used safely in StorageClass parameters and in labels.
func ValidateCapacityPoolName(name string) error {
	if name == "" {
		return errors.New("capacity pool name must not be empty")
	}
	if errs := validation.IsValidLabelValue(name); len(errs) > 0 {
		return fmt.Errorf("invalid capacity pool name %q: %s", name, strings.Join(errs, "; "))
	}
	return nil
}

// parseCapacityEntry parses a single "<pool>=<size>" entry.
func parseCapacityEntry(entry string) (string, resource.Quantity, error) {
	parts := strings.SplitN(entry, "=", 2)
	if len(parts) != 2 {
		return "", resource.Quantity{}, fmt.Errorf("invalid capacity %q: must be of format <pool>=<size>", entry)
	}

	pool := strings.TrimSpace(parts[0])
	if err := ValidateCapacityPoolName(pool); err != nil {
		return "", resource.Quantity{}, err
	}

	size := strings.TrimSpace(parts[1])
	quantity, err := resource.ParseQuantity(size)
	if err != nil {
		return "", resource.Quantity{}, fmt.Errorf("invalid size %q for capacity pool %q: %w", size, pool, err)
	}
	if quantity.Sign() < 0 {
		return "", resource.Quantity{}, fmt.Errorf("size of capacity pool %q must not be negative, got %s", pool, size)
	}

	return pool, quantity, nil
}

// ParseCapacityConfig parses the content of a per-node capacity file. The
// file holds "<pool>=<size>" entries, separated by newlines or commas.
// Blank entries and lines starting with '#' are ignored. A pool must not be
// listed twice.
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
			pool, quantity, err := parseCapacityEntry(entry)
			if err != nil {
				return nil, err
			}
			if _, duplicate := capacity[pool]; duplicate {
				return nil, fmt.Errorf("capacity pool %q is configured more than once", pool)
			}
			capacity[pool] = quantity
		}
	}
	return capacity, nil
}

// LoadNodeCapacity reads the capacity file of a node from dir. The file is
// named after the node, so that a ConfigMap with one key per node can be
// mounted at dir. It returns found == false if dir has no file for the node.
func LoadNodeCapacity(dir, nodeID string) (capacity Capacity, found bool, err error) {
	if nodeID == "" || nodeID != filepath.Base(nodeID) || strings.HasPrefix(nodeID, ".") {
		return nil, false, fmt.Errorf("node id %q cannot be used as a capacity file name", nodeID)
	}

	path := filepath.Join(dir, nodeID)
	content, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		return nil, false, nil
	}
	if err != nil {
		return nil, false, fmt.Errorf("read capacity file %s: %w", path, err)
	}

	capacity, err = ParseCapacityConfig(string(content))
	if err != nil {
		return nil, false, fmt.Errorf("parse capacity file %s: %w", path, err)
	}
	return capacity, true, nil
}
