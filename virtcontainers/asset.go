// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"crypto/sha512"
	"encoding/hex"
	"fmt"
	"io/ioutil"
	"path/filepath"

	"github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
)

type assetType string

func (t assetType) annotations() (string, string, error) {
	switch t {
	case kernelAsset:
		return annotations.KernelPath, annotations.KernelHash, nil
	case imageAsset:
		return annotations.ImagePath, annotations.ImageHash, nil
	case initrdAsset:
		return annotations.InitrdPath, annotations.InitrdHash, nil
	case hypervisorAsset:
		return annotations.HypervisorPath, annotations.HypervisorHash, nil
	case firmwareAsset:
		return annotations.FirmwarePath, annotations.FirmwareHash, nil
	}

	return "", "", fmt.Errorf("Wrong asset type %s", t)
}

const (
	kernelAsset     assetType = "kernel"
	imageAsset      assetType = "image"
	initrdAsset     assetType = "initrd"
	hypervisorAsset assetType = "hypervisor"
	firmwareAsset   assetType = "firmware"
)

type asset struct {
	path         string
	computedHash string
	kind         assetType
}

func (a *asset) valid() bool {
	if !filepath.IsAbs(a.path) {
		return false
	}

	switch a.kind {
	case kernelAsset:
		return true
	case imageAsset:
		return true
	case initrdAsset:
		return true
	case hypervisorAsset:
		return true
	case firmwareAsset:
		return true
	}

	return false
}

// hash returns the hex encoded string for the asset hash
func (a *asset) hash(hashType string) (string, error) {
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
		virtLog.Debugf("Computing %v hash", a.path)
		hashComputed := sha512.Sum512(bytes)
		hashEncodedLen = hex.EncodedLen(len(hashComputed))
		hashEncoded := make([]byte, hashEncodedLen)
		hex.Encode(hashEncoded, hashComputed[:])
		hash = string(hashEncoded[:])
		virtLog.Debugf("%v hash: %s", a.path, hash)
	default:
		return "", fmt.Errorf("Invalid hash type %s", hashType)
	}

	a.computedHash = hash

	return hash, nil
}

// newAsset returns a new asset from the sandbox annotations.
func newAsset(sandboxConfig *SandboxConfig, t assetType) (*asset, error) {
	pathAnnotation, hashAnnotation, err := t.annotations()
	if err != nil {
		return nil, err
	}

	if pathAnnotation == "" || hashAnnotation == "" {
		return nil, fmt.Errorf("Missing annotation paths for %s", t)
	}

	path, ok := sandboxConfig.Annotations[pathAnnotation]
	if !ok || path == "" {
		return nil, nil
	}

	if !filepath.IsAbs(path) {
		return nil, fmt.Errorf("%s is not an absolute path", path)
	}

	a := &asset{path: path, kind: t}

	hash, ok := sandboxConfig.Annotations[hashAnnotation]
	if !ok || hash == "" {
		return a, nil
	}

	// We have a hash annotation, we need to verify the asset against it.
	hashType, ok := sandboxConfig.Annotations[annotations.AssetHashType]
	if !ok {
		virtLog.Warningf("Unrecognized hash type: %s, switching to %s", hashType, annotations.SHA512)
		hashType = annotations.SHA512
	}

	hashComputed, err := a.hash(hashType)
	if err != nil {
		return a, err
	}

	// If our computed asset hash does not match the passed annotation, we must exit.
	if hashComputed != hash {
		return nil, fmt.Errorf("Invalid hash for %s: computed %s, expecting %s]", a.path, hashComputed, hash)
	}

	return a, nil
}
