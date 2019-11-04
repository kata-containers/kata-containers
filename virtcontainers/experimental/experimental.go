// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package experimental

import (
	"context"
	"fmt"
	"regexp"
)

const (
	nameRegStr = "^[a-z][a-z0-9_]*$"
)

// Feature to be experimental
type Feature struct {
	Name        string
	Description string
	// the expected release version to move out from experimental
	ExpRelease string
}

type contextKey struct{}

var (
	supportedFeatures = make(map[string]Feature)
	expContextKey     = contextKey{}
)

// Register register a new experimental feature
func Register(feature Feature) error {
	if err := validateFeature(feature); err != nil {
		return err
	}

	if _, ok := supportedFeatures[feature.Name]; ok {
		return fmt.Errorf("Feature %q had been registered before", feature.Name)
	}
	supportedFeatures[feature.Name] = feature
	return nil
}

// Get returns Feature with requested name
func Get(name string) *Feature {
	if f, ok := supportedFeatures[name]; ok {
		return &f
	}
	return nil
}

func validateFeature(feature Feature) error {
	if len(feature.Name) == 0 ||
		len(feature.Description) == 0 ||
		len(feature.ExpRelease) == 0 {
		return fmt.Errorf("experimental feature must have valid name, description and expected release")
	}

	reg := regexp.MustCompile(nameRegStr)
	if !reg.MatchString(feature.Name) {
		return fmt.Errorf("feature name must in the format %q", nameRegStr)
	}

	return nil
}

func ContextWithExp(ctx context.Context, names []string) context.Context {
	return context.WithValue(ctx, expContextKey, names)
}

func ExpFromContext(ctx context.Context) []string {
	value := ctx.Value(expContextKey)
	if value == nil {
		return nil
	}
	names := value.([]string)
	return names
}
