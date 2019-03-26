//
// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func testCreateNoopContainer() (*Sandbox, *Container, error) {
	defer cleanUp()

	contID := "100"
	config := newTestSandboxConfigNoop()

	ctx := context.Background()
	p, err := CreateSandbox(ctx, config, nil)
	if err != nil {
		return nil, nil, err
	}

	contConfig := newTestContainerConfigNoop(contID)

	p, c, err := CreateContainer(ctx, p.ID(), contConfig)
	if err != nil {
		return nil, nil, err
	}

	return p.(*Sandbox), c.(*Container), nil
}

func TestNoopAgentInit(t *testing.T) {
	n := &noopAgent{}
	sandbox := &Sandbox{}

	err := n.init(context.Background(), sandbox, nil)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentExec(t *testing.T) {
	n := &noopAgent{}
	cmd := types.Cmd{}
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

func TestNoopAgentPauseContainer(t *testing.T) {
	n := &noopAgent{}
	sandbox, container, err := testCreateNoopContainer()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()
	err = n.pauseContainer(sandbox, *container)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentResumeContainer(t *testing.T) {
	n := &noopAgent{}
	sandbox, container, err := testCreateNoopContainer()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()
	err = n.resumeContainer(sandbox, *container)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentConfigure(t *testing.T) {
	n := &noopAgent{}
	h := &mockHypervisor{}
	id := "foobar"
	sharePath := "foobarDir"
	err := n.configure(h, id, sharePath, true, nil)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentGetVMPath(t *testing.T) {
	n := &noopAgent{}
	path := n.getVMPath("")
	if path != "" {
		t.Fatal("getSharePath returns non empty path")
	}
}

func TestNoopAgentGetSharePath(t *testing.T) {
	n := &noopAgent{}
	path := n.getSharePath("")
	if path != "" {
		t.Fatal("getSharePath returns non empty path")
	}
}

func TestNoopAgentStartProxy(t *testing.T) {
	n := &noopAgent{}
	sandbox, _, err := testCreateNoopContainer()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()
	err = n.startProxy(sandbox)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentProcessListContainer(t *testing.T) {
	n := &noopAgent{}
	sandbox, container, err := testCreateNoopContainer()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()
	_, err = n.processListContainer(sandbox, *container, ProcessListOptions{})
	if err != nil {
		t.Fatal(err)
	}
}

func TestNoopAgentReseedRNG(t *testing.T) {
	n := &noopAgent{}
	err := n.reseedRNG([]byte{})
	if err != nil {
		t.Fatal("reseedRNG failed")
	}
}

func TestNoopAgentUpdateInterface(t *testing.T) {
	n := &noopAgent{}
	_, err := n.updateInterface(nil)
	if err != nil {
		t.Fatal("updateInterface failed")
	}
}

func TestNoopAgentListInterfaces(t *testing.T) {
	n := &noopAgent{}
	_, err := n.listInterfaces()
	if err != nil {
		t.Fatal("listInterfaces failed")
	}
}

func TestNoopAgentUpdateRoutes(t *testing.T) {
	n := &noopAgent{}
	_, err := n.updateRoutes(nil)
	if err != nil {
		t.Fatal("updateRoutes failed")
	}
}

func TestNoopAgentListRoutes(t *testing.T) {
	n := &noopAgent{}
	_, err := n.listRoutes()
	if err != nil {
		t.Fatal("listRoutes failed")
	}
}

func TestNoopAgentRSetProxy(t *testing.T) {
	n := &noopAgent{}
	p := &noopProxy{}
	s := &Sandbox{}
	err := n.setProxy(s, p, 0, "")
	if err != nil {
		t.Fatal("set proxy failed")
	}
}

func TestNoopGetAgentUrl(t *testing.T) {
	assert := assert.New(t)
	n := &noopAgent{}

	url, err := n.getAgentURL()
	assert.Nil(err)
	assert.Empty(url)
}

func TestNoopCopyFile(t *testing.T) {
	assert := assert.New(t)
	n := &noopAgent{}

	err := n.copyFile("", "")
	assert.Nil(err)
}
