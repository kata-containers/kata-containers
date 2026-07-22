// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package drivers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestCheckIgnorePCIClass(t *testing.T) {
	type testData struct {
		pciClass string
		ignored  bool
	}

	data := []testData{
		{"0x060000", true},  // Host Bridge
		{"0x060400", true},  // PCI-to-PCI Bridge
		{"0x068000", false}, // NVSwitch ("Bridge: Other"), must be passed through
		{"0x030200", false}, // 3D controller (GPU)
		{"0x040300", false}, // Audio device
		{"", false},
	}

	for _, d := range data {
		ignored, err := checkIgnorePCIClass(d.pciClass, "0000:00:00.0")
		assert.NoError(t, err, "pciClass %q", d.pciClass)
		assert.Equal(t, d.ignored, ignored, "pciClass %q", d.pciClass)
	}
}
