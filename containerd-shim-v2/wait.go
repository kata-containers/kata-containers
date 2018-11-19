// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"time"

	"github.com/containerd/containerd/api/types/task"
	"github.com/sirupsen/logrus"
)

func wait(s *service, c *container, execID string) (int32, error) {
	var execs *exec
	var err error

	processID := c.id

	if execID == "" {
		//wait until the io closed, then wait the container
		<-c.exitIOch
	}

	ret, err := s.sandbox.WaitProcess(c.id, processID)
	if err != nil {
		logrus.WithError(err).WithFields(logrus.Fields{
			"container": c.id,
			"pid":       processID,
		}).Error("Wait for process failed")
	}

	if execID == "" {
		c.exitCh <- uint32(ret)
	} else {
		execs.exitCh <- uint32(ret)
	}

	timeStamp := time.Now()
	c.mu.Lock()
	if execID == "" {
		c.status = task.StatusStopped
		c.exit = uint32(ret)
		c.time = timeStamp
	} else {
		execs.status = task.StatusStopped
		execs.exitCode = ret
		execs.exitTime = timeStamp
	}
	c.mu.Unlock()

	go cReap(s, int(ret), c.id, execID, timeStamp)

	return ret, nil
}
