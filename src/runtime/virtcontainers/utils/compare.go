// Copyright (c) 2019 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import "reflect"

func compareStruct(foo, bar reflect.Value) bool {
	for i := 0; i < foo.NumField(); i++ {
		if !deepCompareValue(foo.Field(i), bar.Field(i)) {
			return false
		}
	}

	return true
}

func compareMap(foo, bar reflect.Value) bool {
	if foo.Len() != bar.Len() {
		return false
	}

	for _, k := range foo.MapKeys() {
		if !deepCompareValue(foo.MapIndex(k), bar.MapIndex(k)) {
			return false
		}
	}

	return true
}

func compareSlice(foo, bar reflect.Value) bool {
	if foo.Len() != bar.Len() {
		return false
	}
	for j := 0; j < foo.Len(); j++ {
		if !deepCompareValue(foo.Index(j), bar.Index(j)) {
			return false
		}
	}
	return true
}

func deepCompareValue(foo, bar reflect.Value) bool {
	if !foo.IsValid() || !bar.IsValid() {
		return foo.IsValid() == bar.IsValid()
	}

	if foo.Type() != bar.Type() {
		return false
	}
	switch foo.Kind() {
	case reflect.Map:
		return compareMap(foo, bar)
	case reflect.Array:
		fallthrough
	case reflect.Slice:
		return compareSlice(foo, bar)
	case reflect.Struct:
		return compareStruct(foo, bar)
	case reflect.Interface:
		return reflect.DeepEqual(foo.Interface(), bar.Interface())
	default:
		return foo.Interface() == bar.Interface()
	}
}

// DeepCompare compare foo and bar.
func DeepCompare(foo, bar interface{}) bool {
	v1 := reflect.ValueOf(foo)
	v2 := reflect.ValueOf(bar)

	return deepCompareValue(v1, v2)
}
