//
// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
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
	sandbox := &Sandbox{}

	err := n.startSandbox(sandbox)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentStopSandbox(t *testing.T) {
	n := &noopAgent{}
	sandbox := &Sandbox{}

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

	if err := n.startSandbox(sandbox); err != nil {
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

	err = n.startContainer(sandbox, container)
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

	err = n.stopContainer(sandbox, *container)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentStatsContainer(t *testing.T) {
	n := &noopAgent{}
	sandbox, container, err := testCreateNoopContainer()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()
	_, err = n.statsContainer(sandbox, *container)
	if err != nil {
		t.Fatal(err)
	}
}
