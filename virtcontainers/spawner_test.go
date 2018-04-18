// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"reflect"
	"testing"
)

var testSpawnerTypeList = []SpawnerType{
	NsEnter,
}

func TestSpawnerTypeSet(t *testing.T) {
	var s SpawnerType
	var err error

	for _, sType := range testSpawnerTypeList {
		err = (&s).Set(string(sType))
		if err != nil {
			t.Fatal(err)
		}

		if s != sType {
			t.Fatal()
		}
	}
}

func TestWrongSpawnerTypeSet(t *testing.T) {
	var s SpawnerType

	err := (&s).Set("noType")
	if err == nil || s != "" {
		t.Fatal()
	}
}

func TestSpawnerTypeString(t *testing.T) {
	for _, sType := range testSpawnerTypeList {
		s := sType

		result := (&s).String()
		if result != string(NsEnter) {
			t.Fatal()
		}
	}
}

func TestWrongSpawnerTypeString(t *testing.T) {
	var s = SpawnerType("noType")

	result := (&s).String()
	if result != "" {
		t.Fatal()
	}
}

func testSpawnerNewSpawner(t *testing.T, sType SpawnerType, expected interface{}) {
	spawner := newSpawner(sType)

	if spawner == nil {
		t.Fatal()
	}

	if reflect.DeepEqual(spawner, expected) == false {
		t.Fatal()
	}
}

func TestSpawnerNsEnterNewSpawner(t *testing.T) {
	expectedOut := &nsenter{}

	testSpawnerNewSpawner(t, NsEnter, expectedOut)
}

func TestWrongSpawnerNewSpawner(t *testing.T) {
	spawner := newSpawner(SpawnerType("noType"))

	if spawner != nil {
		t.Fatal()
	}
}
