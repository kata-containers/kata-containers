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
	} else {
		execs, err = c.getExec(execID)
		if err != nil {
			return exitCode255, err
		}
		<-execs.exitIOch
		//This wait could be triggered before exec start which
		//will get the exec's id, thus this assignment must after
		//the exec exit, to make sure it get the exec's id.
		processID = execs.id
	}

	ret, err := s.sandbox.WaitProcess(c.id, processID)
	if err != nil {
		logrus.WithError(err).WithFields(logrus.Fields{
			"container": c.id,
			"pid":       processID,
		}).Error("Wait for process failed")
	}

	timeStamp := time.Now()

	s.mu.Lock()
	if execID == "" {
		// Take care of the use case where it is a sandbox.
		// Right after the container representing the sandbox has
		// been deleted, let's make sure we stop and delete the
		// sandbox.

		if c.cType.IsSandbox() {
			if err = s.sandbox.Stop(); err != nil {
				logrus.WithField("sandbox", s.sandbox.ID()).Error("failed to stop sandbox")
			}

			if err = s.sandbox.Delete(); err != nil {
				logrus.WithField("sandbox", s.sandbox.ID()).Error("failed to delete sandbox")
			}
		} else {
			if _, err = s.sandbox.StopContainer(c.id); err != nil {
				logrus.WithError(err).WithField("container", c.id).Warn("stop container failed")
			}
		}
		c.status = task.StatusStopped
		c.exit = uint32(ret)
		c.exitTime = timeStamp

		c.exitCh <- uint32(ret)

	} else {
		execs.status = task.StatusStopped
		execs.exitCode = ret
		execs.exitTime = timeStamp

		execs.exitCh <- uint32(ret)
	}
	s.mu.Unlock()

	go cReap(s, int(ret), c.id, execID, timeStamp)

	return ret, nil
}
