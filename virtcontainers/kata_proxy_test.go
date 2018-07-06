// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"
)

func TestKataProxyStart(t *testing.T) {
	agent := &kataAgent{}
	proxy := &kataProxy{}

	testProxyStart(t, agent, proxy, KataProxyType)
}
