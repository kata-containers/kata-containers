// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestSandboxListOperations(t *testing.T) {
	p := &Sandbox{id: "testsandboxListsandbox"}
	l := &sandboxList{sandboxes: make(map[string]*Sandbox)}
	err := l.addSandbox(p)
	assert.Nil(t, err, "addSandbox failed")

	err = l.addSandbox(p)
	assert.NotNil(t, err, "add same sandbox should fail")

	np, err := l.lookupSandbox(p.id)
	assert.Nil(t, err, "lookupSandbox failed")
	assert.Equal(t, np, p, "lookupSandbox returns different sandbox %v:%v", np, p)

	_, err = l.lookupSandbox("some-non-existing-sandbox-name")
	assert.NotNil(t, err, "lookupSandbox for non-existing sandbox should fail")

	l.removeSandbox(p.id)
}
