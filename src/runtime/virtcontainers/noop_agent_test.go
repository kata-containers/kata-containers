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
	assert := assert.New(t)

	disableVMShutdown, err := n.init(context.Background(), sandbox, nil)
	assert.NoError(err)
	assert.False(disableVMShutdown)
}

func TestNoopAgentExec(t *testing.T) {
	n := &noopAgent{}
	cmd := types.Cmd{}
	assert := assert.New(t)

	sandbox, container, err := testCreateNoopContainer()
	assert.NoError(err)
	defer cleanUp()

	_, err = n.exec(sandbox, *container, cmd)
	assert.NoError(err)
}

func TestNoopAgentStartSandbox(t *testing.T) {
	n := &noopAgent{}
	sandbox := &Sandbox{}
	assert := assert.New(t)

	err := n.startSandbox(sandbox)
	assert.NoError(err)
}

func TestNoopAgentStopSandbox(t *testing.T) {
	n := &noopAgent{}
	sandbox := &Sandbox{}
	assert := assert.New(t)

	err := n.stopSandbox(sandbox)
	assert.NoError(err)
}

func TestNoopAgentCreateContainer(t *testing.T) {
	n := &noopAgent{}
	assert := assert.New(t)
	sandbox, container, err := testCreateNoopContainer()
	assert.NoError(err)
	defer cleanUp()

	err = n.startSandbox(sandbox)
	assert.NoError(err)

	_, err = n.createContainer(sandbox, container)
	assert.NoError(err)
}

func TestNoopAgentStartContainer(t *testing.T) {
	n := &noopAgent{}
	assert := assert.New(t)

	sandbox, container, err := testCreateNoopContainer()
	assert.NoError(err)
	defer cleanUp()

	err = n.startContainer(sandbox, container)
	assert.NoError(err)
}

func TestNoopAgentStopContainer(t *testing.T) {
	n := &noopAgent{}
	assert := assert.New(t)
	sandbox, container, err := testCreateNoopContainer()
	assert.NoError(err)
	defer cleanUp()

	err = n.stopContainer(sandbox, *container)
	assert.NoError(err)
}

func TestNoopAgentStatsContainer(t *testing.T) {
	n := &noopAgent{}
	assert := assert.New(t)
	sandbox, container, err := testCreateNoopContainer()
	assert.NoError(err)

	defer cleanUp()
	_, err = n.statsContainer(sandbox, *container)
	assert.NoError(err)
}

func TestNoopAgentPauseContainer(t *testing.T) {
	n := &noopAgent{}
	assert := assert.New(t)
	sandbox, container, err := testCreateNoopContainer()
	assert.NoError(err)

	defer cleanUp()
	err = n.pauseContainer(sandbox, *container)
	assert.NoError(err)
}

func TestNoopAgentResumeContainer(t *testing.T) {
	n := &noopAgent{}
	assert := assert.New(t)
	sandbox, container, err := testCreateNoopContainer()
	assert.NoError(err)
	defer cleanUp()
	err = n.resumeContainer(sandbox, *container)
	assert.NoError(err)
}

func TestNoopAgentConfigure(t *testing.T) {
	n := &noopAgent{}
	h := &mockHypervisor{}
	id := "foobar"
	sharePath := "foobarDir"
	assert := assert.New(t)
	err := n.configure(h, id, sharePath, true, nil)
	assert.NoError(err)
}

func TestNoopAgentGetSharePath(t *testing.T) {
	n := &noopAgent{}
	path := n.getSharePath("")
	assert := assert.New(t)
	assert.Empty(path)
}

func TestNoopAgentStartProxy(t *testing.T) {
	assert := assert.New(t)
	n := &noopAgent{}
	sandbox, _, err := testCreateNoopContainer()

	assert.NoError(err)
	defer cleanUp()
	err = n.startProxy(sandbox)
	assert.NoError(err)
}

func TestNoopAgentProcessListContainer(t *testing.T) {
	assert := assert.New(t)
	n := &noopAgent{}
	sandbox, container, err := testCreateNoopContainer()
	assert.NoError(err)
	defer cleanUp()
	_, err = n.processListContainer(sandbox, *container, ProcessListOptions{})
	assert.NoError(err)
}

func TestNoopAgentReseedRNG(t *testing.T) {
	assert := assert.New(t)
	n := &noopAgent{}
	err := n.reseedRNG([]byte{})
	assert.NoError(err)
}

func TestNoopAgentUpdateInterface(t *testing.T) {
	assert := assert.New(t)
	n := &noopAgent{}
	_, err := n.updateInterface(nil)
	assert.NoError(err)
}

func TestNoopAgentListInterfaces(t *testing.T) {
	assert := assert.New(t)
	n := &noopAgent{}
	_, err := n.listInterfaces()
	assert.NoError(err)
}

func TestNoopAgentUpdateRoutes(t *testing.T) {
	assert := assert.New(t)
	n := &noopAgent{}
	_, err := n.updateRoutes(nil)
	assert.NoError(err)
}

func TestNoopAgentListRoutes(t *testing.T) {
	n := &noopAgent{}
	assert := assert.New(t)
	_, err := n.listRoutes()
	assert.NoError(err)
}

func TestNoopAgentRSetProxy(t *testing.T) {
	n := &noopAgent{}
	p := &noopProxy{}
	s := &Sandbox{}
	assert := assert.New(t)
	err := n.setProxy(s, p, 0, "")
	assert.NoError(err)
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
