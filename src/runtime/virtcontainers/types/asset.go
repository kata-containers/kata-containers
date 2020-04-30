// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"crypto/sha512"
	"encoding/hex"
	"fmt"
	"io/ioutil"
	"path/filepath"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
)

// AssetType describe a type of assets.
type AssetType string

// Annotations returns the path and hash annotations for a given Asset type.
func (t AssetType) Annotations() (string, string, error) {
	switch t {
	case KernelAsset:
		return annotations.KernelPath, annotations.KernelHash, nil
	case ImageAsset:
		return annotations.ImagePath, annotations.ImageHash, nil
	case InitrdAsset:
		return annotations.InitrdPath, annotations.InitrdHash, nil
	case HypervisorAsset:
		return annotations.HypervisorPath, annotations.HypervisorHash, nil
	case JailerAsset:
		return annotations.JailerPath, annotations.JailerHash, nil
	case FirmwareAsset:
		return annotations.FirmwarePath, annotations.FirmwareHash, nil
	}

	return "", "", fmt.Errorf("Wrong asset type %s", t)
}

const (
	// KernelAsset is a kernel asset.
	KernelAsset AssetType = "kernel"

	// ImageAsset is an image asset.
	ImageAsset AssetType = "image"

	// InitrdAsset is an intird asset.
	InitrdAsset AssetType = "initrd"

	// HypervisorAsset is an hypervisor asset.
	HypervisorAsset AssetType = "hypervisor"

	// HypervisorCtlAsset is an hypervisor control asset.
	HypervisorCtlAsset AssetType = "hypervisorctl"
	// JailerAsset is an jailer asset.
	JailerAsset AssetType = "jailer"

	// FirmwareAsset is a firmware asset.
	FirmwareAsset AssetType = "firmware"
)

// Asset represents a virtcontainers asset.
type Asset struct {
	path         string
	computedHash string
	kind         AssetType
}

// Path returns an asset path.
func (a Asset) Path() string {
	return a.path
}

// Type returns an asset type.
func (a Asset) Type() AssetType {
	return a.kind
}

// Valid checks if an asset is valid or not.
func (a *Asset) Valid() bool {
	if !filepath.IsAbs(a.path) {
		return false
	}

	switch a.kind {
	case KernelAsset:
		return true
	case ImageAsset:
		return true
	case InitrdAsset:
		return true
	case HypervisorAsset:
		return true
	case JailerAsset:
		return true
	case FirmwareAsset:
		return true
	}

	return false
}

// Hash returns the hex encoded string for the asset hash
func (a *Asset) Hash(hashType string) (string, error) {
	var hashEncodedLen int
	var hash string

	// We read the actual asset content
	bytes, err := ioutil.ReadFile(a.path)
	if err != nil {
		return "", err
	}

	if len(bytes) == 0 {
		return "", fmt.Errorf("Empty asset file at %s", a.path)
	}

	// Build the asset hash and convert it to a string.
	// We only support SHA512 for now.
	switch hashType {
	case annotations.SHA512:
		hashComputed := sha512.Sum512(bytes)
		hashEncodedLen = hex.EncodedLen(len(hashComputed))
		hashEncoded := make([]byte, hashEncodedLen)
		hex.Encode(hashEncoded, hashComputed[:])
		hash = string(hashEncoded[:])
	default:
		return "", fmt.Errorf("Invalid hash type %s", hashType)
	}

	a.computedHash = hash

	return hash, nil
}

// NewAsset returns a new asset from a slice of annotations.
func NewAsset(anno map[string]string, t AssetType) (*Asset, error) {
	pathAnnotation, hashAnnotation, err := t.Annotations()
	if err != nil {
		return nil, err
	}

	if pathAnnotation == "" || hashAnnotation == "" {
		return nil, fmt.Errorf("Missing annotation paths for %s", t)
	}

	path, ok := anno[pathAnnotation]
	if !ok || path == "" {
		return nil, nil
	}

	if !filepath.IsAbs(path) {
		return nil, fmt.Errorf("%s is not an absolute path", path)
	}

	a := &Asset{path: path, kind: t}

	hash, ok := anno[hashAnnotation]
	if !ok || hash == "" {
		return a, nil
	}

	// We have a hash annotation, we need to verify the asset against it.
	hashType, ok := anno[annotations.AssetHashType]
	if !ok {
		hashType = annotations.SHA512
	}

	hashComputed, err := a.Hash(hashType)
	if err != nil {
		return a, err
	}

	// If our computed asset hash does not match the passed annotation, we must exit.
	if hashComputed != hash {
		return nil, fmt.Errorf("Invalid hash for %s: computed %s, expecting %s]", a.path, hashComputed, hash)
	}

	return a, nil
}
