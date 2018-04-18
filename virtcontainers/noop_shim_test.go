// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"
)

func TestNoopShimStart(t *testing.T) {
	s := &noopShim{}
	sandbox := Sandbox{}
	params := ShimParams{}
	expected := 0

	pid, err := s.start(sandbox, params)
	if err != nil {
		t.Fatal(err)
	}

	if pid != expected {
		t.Fatalf("PID should be %d", expected)
	}
}
