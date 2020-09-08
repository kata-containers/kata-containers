// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

type ExampleStruct struct {
	X int
	Y string
}

func TestCompareStruct(t *testing.T) {
	assert := assert.New(t)

	var testStruct1, testStruct2 ExampleStruct

	testStruct1 = ExampleStruct{1, "test"}
	testStruct2 = ExampleStruct{1, "test"}
	result := DeepCompare(testStruct1, testStruct2)
	assert.True(result)

	testStruct2 = ExampleStruct{2, "test"}
	result = DeepCompare(testStruct1, testStruct2)
	assert.False(result)
}

func TestCompareArray(t *testing.T) {
	assert := assert.New(t)

	a1 := [2]string{"test", "array"}
	a2 := [2]string{"test", "array"}
	result := DeepCompare(a1, a2)
	assert.True(result)

	a2 = [2]string{"test", "compare"}
	result = DeepCompare(a1, a2)
	assert.False(result)

	a3 := [3]string{"test", "array", "compare"}
	result = DeepCompare(a1, a3)
	assert.False(result)
}

func TestCompareSlice(t *testing.T) {
	assert := assert.New(t)

	s1 := []int{1, 2, 3}
	s2 := []int{1, 2, 3}
	result := DeepCompare(s1, s2)
	assert.True(result)

	s2 = []int{1, 2, 4}
	result = DeepCompare(s1, s2)
	assert.False(result)

	s2 = []int{1, 2, 3, 4}
	result = DeepCompare(s1, s2)
	assert.False(result)
}

func TestCompareMap(t *testing.T) {
	assert := assert.New(t)

	m1 := make(map[string]int)
	m1["a"] = 1
	m2 := make(map[string]int)
	m2["a"] = 1
	result := DeepCompare(m1, m2)
	assert.True(result)

	m1["b"] = 2
	result = DeepCompare(m1, m2)
	assert.False(result)

	m2["b"] = 3
	result = DeepCompare(m1, m2)
	assert.False(result)
}

func TestDeepCompareValueFailure(t *testing.T) {
	assert := assert.New(t)

	a := [2]string{"test", "array"}
	s := []string{"test", "array"}
	result := DeepCompare(a, s)
	assert.False(result)
}
