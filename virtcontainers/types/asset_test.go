// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"io/ioutil"
	"os"
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
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

	anno := map[string]string{
		annotations.KernelPath: tmpfile.Name(),
		annotations.KernelHash: assetContentHash,
	}
	a, err := NewAsset(anno, ImageAsset)
	assert.Nil(err)
	assert.Nil(a)

	a, err = NewAsset(anno, KernelAsset)
	assert.Nil(err)
	assert.Equal(assetContentHash, a.computedHash)

	anno = map[string]string{
		annotations.KernelPath: tmpfile.Name(),
		annotations.KernelHash: assetContentWrongHash,
	}

	_, err = NewAsset(anno, KernelAsset)
	assert.NotNil(err)
}
