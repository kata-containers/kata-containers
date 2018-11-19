// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"

	"github.com/containerd/containerd/api/types/task"
	"github.com/kata-containers/runtime/pkg/katautils"
)

func startContainer(ctx context.Context, s *service, c *container) error {
	//start a container
	if c.cType == "" {
		err := fmt.Errorf("Bug, the container %s type is empty", c.id)
		return err
	}

	if s.sandbox == nil {
		err := fmt.Errorf("Bug, the sandbox hasn't been created for this container %s", c.id)
		return err
	}

	if c.cType.IsSandbox() {
		err := s.sandbox.Start()
		if err != nil {
			return err
		}
	} else {
		_, err := s.sandbox.StartContainer(c.id)
		if err != nil {
			return err
		}
	}

	// Run post-start OCI hooks.
	err := katautils.EnterNetNS(s.sandbox.GetNetNs(), func() error {
		return katautils.PostStartHooks(ctx, *c.spec, s.sandbox.ID(), c.bundle)
	})
	if err != nil {
		return err
	}

	c.status = task.StatusRunning

	stdin, stdout, stderr, err := s.sandbox.IOStream(c.id, c.id)
	if err != nil {
		return err
	}

	if c.stdin != "" || c.stdout != "" || c.stderr != "" {
		tty, err := newTtyIO(ctx, c.stdin, c.stdout, c.stderr, c.terminal)
		if err != nil {
			return err
		}
		c.ttyio = tty
		go ioCopy(c.exitIOch, tty, stdin, stdout, stderr)
	} else {
		//close the io exit channel, since there is no io for this container,
		//otherwise the following wait goroutine will hang on this channel.
		close(c.exitIOch)
	}

	go wait(s, c, "")

	return nil
}
