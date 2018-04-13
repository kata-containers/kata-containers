//
// Copyright (c) 2016 Intel Corporation
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
)

func testCreateNoopContainer() (*Sandbox, *Container, error) {
	contID := "100"
	config := newTestSandboxConfigNoop()

	p, err := CreateSandbox(config)
	if err != nil {
		return nil, nil, err
	}

	contConfig := newTestContainerConfigNoop(contID)

	p, c, err := CreateContainer(p.ID(), contConfig)
	if err != nil {
		return nil, nil, err
	}

	return p.(*Sandbox), c.(*Container), nil
}

func TestNoopAgentInit(t *testing.T) {
	n := &noopAgent{}
	sandbox := &Sandbox{}

	err := n.init(sandbox, nil)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentExec(t *testing.T) {
	n := &noopAgent{}
	cmd := Cmd{}
	sandbox, container, err := testCreateNoopContainer()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	if _, err = n.exec(sandbox, *container, cmd); err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentStartSandbox(t *testing.T) {
	n := &noopAgent{}
	sandbox := Sandbox{}

	err := n.startSandbox(sandbox)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentStopSandbox(t *testing.T) {
	n := &noopAgent{}
	sandbox := Sandbox{}

	err := n.stopSandbox(sandbox)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentCreateContainer(t *testing.T) {
	n := &noopAgent{}
	sandbox, container, err := testCreateNoopContainer()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	if err := n.startSandbox(*sandbox); err != nil {
		t.Fatal(err)
	}

	if _, err := n.createContainer(sandbox, container); err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentStartContainer(t *testing.T) {
	n := &noopAgent{}
	sandbox, container, err := testCreateNoopContainer()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	err = n.startContainer(*sandbox, container)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentStopContainer(t *testing.T) {
	n := &noopAgent{}
	sandbox, container, err := testCreateNoopContainer()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	err = n.stopContainer(*sandbox, *container)
	if err != nil {
		t.Fatal(err)
	}
}
