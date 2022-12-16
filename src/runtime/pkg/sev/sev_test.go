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

func TestCalculateLaunchDigestWithKernelHashesSevEs(t *testing.T) {
	ld, err := CalculateSEVESLaunchDigest(1, SigEpycV4, "testdata/ovmf_suffix.bin", "/dev/null", "/dev/null", "")
	if err != nil {
		t.Fatalf("unexpected err value: %s", err)
	}
	hexld := hex.EncodeToString(ld[:])
	if hexld != "7e5c26fb454621eb466978b4d0242b3c04b44a034de7fc0a2d8dac60ea2b6403" {
		t.Fatalf("wrong measurement: %s", hexld)
	}
}

func TestCalculateLaunchDigestWithKernelHashesSevEsAndSmp(t *testing.T) {
	ld, err := CalculateSEVESLaunchDigest(4, SigEpycV4, "testdata/ovmf_suffix.bin", "/dev/null", "/dev/null", "")
	if err != nil {
		t.Fatalf("unexpected err value: %s", err)
	}
	hexld := hex.EncodeToString(ld[:])
	if hexld != "b2111b0051fc3a06ec216899b2c78da99fb9d56c6ff2e8261dd3fe6cff79ecbc" {
		t.Fatalf("wrong measurement: %s", hexld)
	}
}
