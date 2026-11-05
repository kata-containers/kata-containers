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
