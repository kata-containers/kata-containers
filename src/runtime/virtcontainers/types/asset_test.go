// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"fmt"
	"io/ioutil"
	"os"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/stretchr/testify/assert"
)

var assetContent = []byte("FakeAsset fake asset FAKE ASSET")
var assetContentHash = "92549f8d2018a95a294d28a65e795ed7d1a9d150009a28cea108ae10101178676f04ab82a6950d0099e4924f9c5e41dcba8ece56b75fc8b4e0a7492cb2a8c880"
var assetContentWrongHash = "92549f8d2018a95a294d28a65e795ed7d1a9d150009a28cea108ae10101178676f04ab82a6950d0099e4924f9c5e41dcba8ece56b75fc8b4e0a7492cb2a8c881"

func TestAssetWrongHashType(t *testing.T) {
	assert := assert.New(t)

	tmpfile, err := ioutil.TempFile("", "virtcontainers-test-")
	assert.Nil(err)

	defer func() {
		tmpfile.Close()
		os.Remove(tmpfile.Name()) // clean up
	}()

	_, err = tmpfile.Write(assetContent)
	assert.Nil(err)

	a := &Asset{
		path: tmpfile.Name(),
	}

	h, err := a.Hash("shafoo")
	assert.Equal(h, "")
	assert.NotNil(err)
}

func TestAssetHash(t *testing.T) {
	assert := assert.New(t)

	tmpfile, err := ioutil.TempFile("", "virtcontainers-test-")
	assert.Nil(err)

	defer func() {
		tmpfile.Close()
		os.Remove(tmpfile.Name()) // clean up
	}()

	_, err = tmpfile.Write(assetContent)
	assert.Nil(err)

	a := &Asset{
		path: tmpfile.Name(),
	}

	hash, err := a.Hash(annotations.SHA512)
	assert.Nil(err)
	assert.Equal(assetContentHash, hash)
	assert.Equal(assetContentHash, a.computedHash)
}

func testPath(t *testing.T, a *Asset, correctPath string, msg string) {
	assert := assert.New(t)

	returnedPath := a.Path()
	assert.Equal(returnedPath, correctPath, msg)
}

func testType(t *testing.T, a *Asset, correctType AssetType, msg string) {
	assert := assert.New(t)

	returnedType := a.Type()
	assert.Equal(returnedType, correctType, msg)
}

func testValid(t *testing.T, a *Asset, msg string) {
	assert := assert.New(t)

	v := a.Valid()
	assert.True(v, msg)
}

func TestAssetNew(t *testing.T) {
	assert := assert.New(t)

	tmpfile, err := ioutil.TempFile("", "virtcontainers-test-")
	assert.Nil(err)

	defer func() {
		tmpfile.Close()
		os.Remove(tmpfile.Name()) // clean up
	}()

	_, err = tmpfile.Write(assetContent)
	assert.Nil(err)

	type testData struct {
		inputPathVar   string
		inputHashVar   string
		inputAssetType AssetType
		inputHash      string
		expectError    bool
		expectNilAsset bool
	}

	data := []testData{
		// Successful with correct hash
		{annotations.KernelPath, annotations.KernelHash, KernelAsset, assetContentHash, false, false},
		{annotations.ImagePath, annotations.ImageHash, ImageAsset, assetContentHash, false, false},
		{annotations.InitrdPath, annotations.InitrdHash, InitrdAsset, assetContentHash, false, false},
		{annotations.HypervisorPath, annotations.HypervisorHash, HypervisorAsset, assetContentHash, false, false},
		{annotations.HypervisorCtlPath, annotations.HypervisorCtlHash, HypervisorCtlAsset, assetContentHash, false, false},
		{annotations.JailerPath, annotations.JailerHash, JailerAsset, assetContentHash, false, false},
		{annotations.FirmwarePath, annotations.FirmwareHash, FirmwareAsset, assetContentHash, false, false},

		// Failure with incorrect hash
		{annotations.KernelPath, annotations.KernelHash, KernelAsset, assetContentWrongHash, true, false},
		{annotations.ImagePath, annotations.ImageHash, ImageAsset, assetContentWrongHash, true, false},
		{annotations.InitrdPath, annotations.InitrdHash, InitrdAsset, assetContentWrongHash, true, false},
		{annotations.HypervisorPath, annotations.HypervisorHash, HypervisorAsset, assetContentWrongHash, true, false},
		{annotations.HypervisorCtlPath, annotations.HypervisorCtlHash, HypervisorCtlAsset, assetContentWrongHash, true, false},
		{annotations.JailerPath, annotations.JailerHash, JailerAsset, assetContentWrongHash, true, false},
		{annotations.FirmwarePath, annotations.FirmwareHash, FirmwareAsset, assetContentWrongHash, true, false},

		// Other failures
		{annotations.KernelPath, annotations.KernelHash, ImageAsset, assetContentHash, false, true},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		anno := map[string]string{
			d.inputPathVar: tmpfile.Name(),
			d.inputHashVar: d.inputHash,
		}

		if d.expectNilAsset {
			a, err := NewAsset(anno, d.inputAssetType)
			assert.NoError(err, msg)
			assert.Nil(a, msg)
		} else if d.expectError {
			_, err := NewAsset(anno, d.inputAssetType)
			assert.NotNil(err, msg)
		} else {
			a, err := NewAsset(anno, d.inputAssetType)
			assert.Nil(err, msg)
			assert.Equal(assetContentHash, a.computedHash, msg)

			testPath(t, a, tmpfile.Name(), msg)
			testType(t, a, d.inputAssetType, msg)
			testValid(t, a, msg)
		}
	}
}
