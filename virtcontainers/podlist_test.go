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

func TestPodListOperations(t *testing.T) {
	p := &Pod{id: "testpodListpod"}
	l := &podList{pods: make(map[string]*Pod)}
	err := l.addPod(p)
	assert.Nil(t, err, "addPod failed")

	err = l.addPod(p)
	assert.NotNil(t, err, "add same pod should fail")

	np, err := l.lookupPod(p.id)
	assert.Nil(t, err, "lookupPod failed")
	assert.Equal(t, np, p, "lookupPod returns different pod %v:%v", np, p)

	_, err = l.lookupPod("some-non-existing-pod-name")
	assert.NotNil(t, err, "lookupPod for non-existing pod should fail")

	l.removePod(p.id)
}
