// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"
)

func TestCCProxyStart(t *testing.T) {
	proxy := &ccProxy{}

	testProxyStart(t, nil, proxy, CCProxyType)
}
