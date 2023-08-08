// Copyright contributors to AMD SEV/-ES in Go
//
// SPDX-License-Identifier: Apache-2.0

package sev

import (
	"testing"
)

func TestNewVCPUSig(t *testing.T) {
	if NewVCPUSig(23, 1, 2) != SigEpyc {
		t.Errorf("wrong EPYC CPU signature")
	}
	if NewVCPUSig(23, 49, 0) != SigEpycRome {
		t.Errorf("wrong EPYC-Rome CPU signature")
	}
	if NewVCPUSig(25, 1, 1) != SigEpycMilan {
		t.Errorf("wrong EPYC-Milan CPU signature")
	}
}
