//
// Copyright (c) 2018 HyperHQ Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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
