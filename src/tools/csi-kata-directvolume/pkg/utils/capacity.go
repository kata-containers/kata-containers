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
	"math"
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
	if len(parts) != 2 || strings.TrimSpace(parts[0]) == "" || strings.TrimSpace(parts[0]) != parts[0] {
		return errors.New("must be of format <type>=<size>")
	}
	quantity, err := resource.ParseQuantity(parts[1])
	if err != nil {
		return err
	}

	if err := validCapacity(parts[0], quantity); err != nil {
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

// Validate also checks capacities supplied by callers without flag parsing.
func (c Capacity) Validate() error {
	for name, quantity := range c {
		if err := validCapacity(name, quantity); err != nil {
			return err
		}
	}
	return nil
}

func validCapacity(name string, quantity resource.Quantity) error {
	maximum := resource.NewQuantity(math.MaxInt64, resource.BinarySI)
	if name == "" || strings.TrimSpace(name) != name || strings.ContainsAny(name, "=,\n\r#") {
		return fmt.Errorf("invalid capacity pool name %q", name)
	}
	if quantity.Sign() < 0 || quantity.Cmp(*maximum) > 0 {
		return fmt.Errorf("capacity for %q must be between zero and MaxInt64 bytes", name)
	}
	return nil
}

// LoadNode overlays command-line pools with a ConfigMap file named after the
// node. Configuration is read once; changes require a driver restart.
func (c *Capacity) LoadNode(directory, node string) error {
	if directory == "" {
		return nil
	}
	if node == "" || strings.HasPrefix(node, ".") || strings.ContainsAny(node, "/\\") {
		return fmt.Errorf("invalid node name %q", node)
	}
	data, err := os.ReadFile(filepath.Join(directory, node))
	if os.IsNotExist(err) {
		return nil
	}
	if err != nil {
		return err
	}
	overlay := Capacity{}
	for _, line := range strings.Split(string(data), "\n") {
		line, _, _ = strings.Cut(line, "#")
		for _, entry := range strings.Split(line, ",") {
			entry = strings.TrimSpace(entry)
			if entry == "" {
				continue
			}
			name, _, _ := strings.Cut(entry, "=")
			if _, exists := overlay[name]; exists {
				return fmt.Errorf("duplicate capacity pool %q", name)
			}
			if err := overlay.Set(entry); err != nil {
				return fmt.Errorf("capacity entry %q: %w", entry, err)
			}
		}
	}
	if *c == nil {
		*c = Capacity{}
	}
	for name, quantity := range overlay {
		(*c)[name] = quantity
	}
	return nil
}
