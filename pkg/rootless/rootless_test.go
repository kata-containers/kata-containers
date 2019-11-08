// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package rootless

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

type uidMapping struct {
	userNSUID int
	hostUID   int
	rangeUID  int
}

type testScenario struct {
	isRootless bool
	uidMap     []uidMapping
}

var uidMapPathStore = uidMapPath

func createTestUIDMapFile(input string) error {
	f, err := os.Create(uidMapPath)
	if err != nil {
		return err
	}
	defer f.Close()

	_, err = f.WriteString(input)
	if err != nil {
		return err
	}

	return nil
}

func uidMapString(userNSUID, hostUID, rangeUID int) string {
	return fmt.Sprintf("\t%d\t%d\t%d", userNSUID, hostUID, rangeUID)
}

func testWithUIDMapContent(content string, expectedRootless bool, t *testing.T) {
	assert := assert.New(t)
	// Create a test-specific message that is added to each assert
	// call. It will be displayed if any assert test fails.
	msg := fmt.Sprintf("isRootless[%t]: %s", expectedRootless, content)

	tmpDir, err := ioutil.TempDir("", "")
	assert.NoError(err)

	uidMapPath = filepath.Join(tmpDir, "testUIDMapFile")
	defer func() {
		uidMapPath = uidMapPathStore
		os.RemoveAll(uidMapPath)
		os.RemoveAll(tmpDir)
		isRootless = false
		initRootless = false
	}()

	err = createTestUIDMapFile(content)
	assert.NoError(err, msg)

	// make call to IsRootless, this should also call
	// SetRootless
	assert.Equal(expectedRootless, IsRootless(), msg)
}

func TestIsRootless(t *testing.T) {
	assert := assert.New(t)

	// by default isRootless should be set to false initially
	assert.False(isRootless)

	allScenarios := []testScenario{
		//"User NS UID is not root UID"
		{
			isRootless: false,
			uidMap: []uidMapping{
				{1, 0, 1},
				{1, 0, 1000},

				{1, 1000, 1},
				{1, 1000, 1000},

				{1000, 1000, 1},
				{1000, 1000, 1000},

				{1000, 1000, 5555},
			},
		},

		//"Host NS UID is root UID"
		{
			isRootless: false,
			uidMap: []uidMapping{
				{0, 0, 1},
				{0, 0, 1000},

				{1, 0, 1},
				{1, 0, 1000},

				{1000, 0, 0},
				{1000, 0, 1},
				{1000, 0, 1000},
			},
		},

		//"UID range is zero"
		{
			isRootless: false,
			uidMap: []uidMapping{
				{0, 0, 0},
				{1, 0, 0},
				{0, 1, 0},
				{1, 1000, 0},
				{1000, 1000, 0},
			},
		},

		//"Negative UIDs"
		{
			isRootless: false,
			uidMap: []uidMapping{
				{-1, 0, 0},
				{-1, 0, 1},
				{-1, 0, 1000},

				{0, -1, 0},
				{0, -1, 1},
				{0, -1, 1000},

				{1000, 1000, -1},
				{1000, 1000, -1},
				{1000, 1000, -1000},
			},
		},

		//"User NS UID is root UID, host UID is not root UID"
		{
			isRootless: true,
			uidMap: []uidMapping{
				{0, 1, 1},
				{0, 1000, 1},
				{0, 1000, 5555},
			},
		},
	}

	// Run the tests
	for _, scenario := range allScenarios {
		for _, uidMap := range scenario.uidMap {
			mapping := uidMapString(uidMap.userNSUID, uidMap.hostUID, uidMap.rangeUID)
			testWithUIDMapContent(mapping, scenario.isRootless, t)
		}
	}

	testWithUIDMapContent("", false, t)

	testWithUIDMapContent("This is not a mapping", false, t)
}
