// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package experimental

import (
	"fmt"
)

// Feature to be experimental
type Feature string

var (
	supportedFeatures = make(map[Feature]struct{})
)

// Register register a new experimental feature
func Register(feature Feature) error {
	if _, ok := supportedFeatures[feature]; ok {
		return fmt.Errorf("Feature %q had been registered before", feature)
	}
	supportedFeatures[feature] = struct{}{}
	return nil
}

// Supported check if the feature is supported
func Supported(feature Feature) bool {
	_, ok := supportedFeatures[feature]
	return ok
}
