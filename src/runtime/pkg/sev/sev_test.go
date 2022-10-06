// Copyright contributors to AMD SEV/-ES in Go
//
// SPDX-License-Identifier: Apache-2.0

package sev

import (
	"encoding/hex"
	"testing"
)

func TestCalculateLaunchDigestWithoutKernelHashes(t *testing.T) {
	ld, err := CalculateLaunchDigest("testdata/ovmf_suffix.bin", "", "", "")
	if err != nil {
		t.Fatalf("unexpected err value: %s", err)
	}
	hexld := hex.EncodeToString(ld[:])
	if hexld != "b184e06e012366fd7b33ebfb361a515d05f00d354dca07b36abbc1e1e177ced5" {
		t.Fatalf("wrong measurement: %s", hexld)
	}
}

func TestCalculateLaunchDigestWithKernelHashes(t *testing.T) {
	ld, err := CalculateLaunchDigest("testdata/ovmf_suffix.bin", "/dev/null", "/dev/null", "")
	if err != nil {
		t.Fatalf("unexpected err value: %s", err)
	}
	hexld := hex.EncodeToString(ld[:])
	if hexld != "d59d7696efd7facfaa653758586e6120c4b6eaec3e327771d278cc6a44786ba5" {
		t.Fatalf("wrong measurement: %s", hexld)
	}
}
