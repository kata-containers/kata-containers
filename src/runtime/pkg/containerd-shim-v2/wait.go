// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"os"
	"path"
	"time"

	"github.com/containerd/containerd/api/events"
	"github.com/containerd/containerd/api/types/task"
	"github.com/containerd/containerd/mount"
	"github.com/sirupsen/logrus"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
)

const defaultCheckInterval = 1 * time.Second

func wait(ctx context.Context, s *service, c *container, execID string) (int32, error) {
	var execs *exec
	var err error

	processID := c.id

	if execID == "" {
		//wait until the io closed, then wait the container
		<-c.exitIOch
		shimLog.WithField("container", c.id).Debug("The container io streams closed")
	} else {
		execs, err = c.getExec(execID)
		if err != nil {
			return exitCode255, err
		}
		<-execs.exitIOch
		shimLog.WithFields(logrus.Fields{
			"container": c.id,
			"exec":      execID,
		}).Debug("The container process io streams closed")
		//This wait could be triggered before exec start which
		//will get the exec's id, thus this assignment must after
		//the exec exit, to make sure it get the exec's id.
		processID = execs.id
	}

	ret, err := s.sandbox.WaitProcess(ctx, c.id, processID)
	if err != nil {
		shimLog.WithError(err).WithFields(logrus.Fields{
			"container": c.id,
			"pid":       processID,
		}).Error("Wait for process failed")

		// set return code if wait failed
		if ret == 0 {
			ret = exitCode255
		}
	}

	timeStamp := time.Now()

	if execID == "" {
		s.mu.Lock()
		c.status = task.Status_STOPPED
		c.exit = uint32(ret)
		c.exitTime = timeStamp

		// Cancel the sandbox watcher while holding s.mu so we do not race
		// with watchSandbox()'s own (killed-VMM) teardown path.
		if c.cType.IsSandbox() && s.monitor != nil {
			shimLog.WithField("sandbox", s.sandbox.ID()).Info("cancel watcher")
			s.monitor <- nil
		}

		// Register the teardown before publishing the exit below.  Otherwise
		// containerd could react to the early TaskExit, call Delete() then
		// Shutdown(), and Shutdown()'s teardownWg.Wait() could observe a zero
		// counter and let the shim exit before the teardown even started.
		s.teardownWg.Add(1)
		defer s.teardownWg.Done()
		s.mu.Unlock()

		// Publish the exit code and TaskExit event *before* the sandbox
		// teardown.  Docker/containerd rely on the TaskExit event, not only on
		// the Wait RPC; if teardown (a potentially slow guest shutdown) ran
		// first, containerd could SIGKILL the shim before the exit was
		// published, surfacing exit code 255.
		c.exitCh <- uint32(ret)
		shimLog.WithFields(logrus.Fields{
			"container": c.id,
			"exit-code": ret,
		}).Info("Publishing container exit status")
		shimLog.WithField("container", c.id).Debug("The container status is StatusStopped")

		go cReap(s, int(ret), c.id, execID, timeStamp)

		// Tear the sandbox down synchronously but *without* holding s.mu.
		// Holding s.mu across the (slow) guest shutdown blocks the Delete()
		// RPC that containerd issues right after the early TaskExit above;
		// containerd then gives up on a clean Delete/Shutdown and runs the
		// `shim delete` binary, which re-connects to the now-dead agent and
		// hangs until killed -- surfacing as a failed `docker run --rm`.
		//
		// Shutdown() waits on teardownWg instead, so the sandbox run
		// directory (watched by kata-monitor) is still removed before the
		// shim exits.  teardownOnce serializes with watchSandbox()'s
		// killed-VMM teardown so the (not internally synchronized)
		// Sandbox.Stop/Delete never run concurrently.
		if c.cType.IsSandbox() {
			s.teardownOnce.Do(func() {
				if err = s.sandbox.Stop(ctx, true); err != nil {
					shimLog.WithField("sandbox", s.sandbox.ID()).Error("failed to stop sandbox")
				}

				if err = s.sandbox.Delete(ctx); err != nil {
					shimLog.WithField("sandbox", s.sandbox.ID()).Error("failed to delete sandbox")
				}
			})
		} else {
			if _, err = s.sandbox.StopContainer(ctx, c.id, true); err != nil {
				shimLog.WithError(err).WithField("container", c.id).Warn("stop container failed")
			}
		}
	} else {
		s.mu.Lock()
		execs.status = task.Status_STOPPED
		execs.exitCode = ret
		execs.exitTime = timeStamp
		s.mu.Unlock()

		execs.exitCh <- uint32(ret)
		shimLog.WithFields(logrus.Fields{
			"container": c.id,
			"exec":      execID,
		}).Debug("The container exec status is StatusStopped")

		go cReap(s, int(ret), c.id, execID, timeStamp)
	}

	return ret, nil
}

func watchSandbox(ctx context.Context, s *service) {
	if s.monitor == nil {
		return
	}
	err := <-s.monitor
	shimLog.WithError(err).WithField("sandbox", s.sandbox.ID()).Info("watchSandbox gets an error or stop signal")
	if err == nil {
		return
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	s.monitor = nil

	// sandbox malfunctioning, cleanup as much as we can.
	// teardownOnce serializes with wait()'s teardown so the (not internally
	// synchronized) Sandbox.Stop/Delete never run concurrently.
	shimLog.WithError(err).Warn("sandbox stopped unexpectedly")
	s.teardownOnce.Do(func() {
		if serr := s.sandbox.Stop(ctx, true); serr != nil {
			shimLog.WithError(serr).Warn("stop sandbox failed")
		}
		if derr := s.sandbox.Delete(ctx); derr != nil {
			shimLog.WithError(derr).Warn("delete sandbox failed")
		}
	})

	for _, c := range s.containers {
		if !c.mounted {
			continue
		}
		rootfs := path.Join(c.bundle, "rootfs")
		shimLog.WithField("rootfs", rootfs).WithField("container", c.id).Debug("container umount rootfs")
		if err := mount.UnmountAll(rootfs, 0); err != nil {
			shimLog.WithError(err).Warn("failed to cleanup rootfs mount")
		}
	}

	// Existing container/exec will be cleaned up by its waiters.
	// No need to send async events here.
}

func watchOOMEvents(ctx context.Context, s *service) {
	if s.sandbox == nil {
		return
	}

	for {
		select {
		case <-s.ctx.Done():
			return
		default:
			containerID, err := s.sandbox.GetOOMEvent(ctx)
			if err != nil {
				if err.Error() == "ttrpc: closed" || err.Error() == "Dead agent" {
					shimLog.WithError(err).Info("agent has shutdown, return from watching of OOM events")
					return
				}
				shimLog.WithError(err).Info("failed to get OOM event from sandbox")
				time.Sleep(defaultCheckInterval)
				continue
			}

			// write oom file for CRI-O
			if c, ok := s.containers[containerID]; ok && oci.IsCRIOContainerManager(c.spec) {
				oomPath := path.Join(c.bundle, "oom")
				shimLog.Infof("write oom file to notify CRI-O: %s", oomPath)

				f, err := os.OpenFile(oomPath, os.O_CREATE, 0666)
				if err != nil {
					shimLog.WithError(err).Warnf("failed to write oom file %s", oomPath)
				} else {
					f.Close()
				}
			}

			// publish event for containerd
			s.send(&events.TaskOOM{
				ContainerID: containerID,
			})
		}
	}
}
