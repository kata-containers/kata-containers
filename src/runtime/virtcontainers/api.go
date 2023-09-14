// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"runtime"

	deviceApi "github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	deviceConfig "github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	resCtrl "github.com/kata-containers/kata-containers/src/runtime/pkg/resourcecontrol"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/sirupsen/logrus"
)

// apiTracingTags defines tags for the trace span
var apiTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "api",
}

func init() {
	runtime.LockOSThread()
}

var virtLog = logrus.WithField("source", "virtcontainers")

// SetLogger sets the logger for virtcontainers package.
func SetLogger(ctx context.Context, logger *logrus.Entry) {
	fields := virtLog.Data
	virtLog = logger.WithFields(fields)
	SetHypervisorLogger(virtLog) // TODO: this will move to hypervisors pkg
	deviceApi.SetLogger(virtLog)
	compatoci.SetLogger(virtLog)
	deviceConfig.SetLogger(virtLog)
	resCtrl.SetLogger(virtLog)
}

// CreateSandbox is the virtcontainers sandbox creation entry point.
// CreateSandbox creates a sandbox and its containers. It does not start them.
func CreateSandbox(ctx context.Context, sandboxConfig SandboxConfig, factory Factory, prestartHookFunc func(context.Context) error) (VCSandbox, error) {
	span, ctx := katatrace.Trace(ctx, virtLog, "CreateSandbox", apiTracingTags)
	defer span.End()

	logrus.WithField("func", "CreateSandbox").Debug("CreateSandbox() trace")
	_, file, no, ok := runtime.Caller(1)
	if ok {
		logrus.Debug("called from ", "file", file, "number", no)
	}
	logrus.Debugf("Sharath CreateSandbox() trace sandboxConfig HypervisorConfig - %v", sandboxConfig.HypervisorConfig)
	s, err := createSandboxFromConfig(ctx, sandboxConfig, factory, prestartHookFunc)

	return s, err
}

func createSandboxFromConfig(ctx context.Context, sandboxConfig SandboxConfig, factory Factory, prestartHookFunc func(context.Context) error) (_ *Sandbox, err error) {
	logrus.WithField("func", "createSandboxFromConfig").Info("create() trace")
	span, ctx := katatrace.Trace(ctx, virtLog, "createSandboxFromConfig", apiTracingTags)
	defer span.End()

	// Create the sandbox.
	logrus.Debugf("createSandboxFromConfig Sharath Starting sandboxConfig - 2 - %+v", sandboxConfig.HypervisorConfig)
	s, err := createSandbox(ctx, sandboxConfig, factory)
	if err != nil {
		logrus.WithError(err).Info("create() trace")
		return nil, err
	}
	s.Logger().Debugf("createSandboxFromConfig Sharath Starting factory - %+v", s.factory)
	s.Logger().Debugf("createSandboxFromConfig Sharath Starting Hypervisor - %+v", s.hypervisor)

	// Cleanup sandbox resources in case of any failure
	defer func() {
		if err != nil {
			logrus.WithError(err).Info("create() trace")
			s.Delete(ctx)
		}
	}()

	// Create the sandbox network
	if err = s.createNetwork(ctx); err != nil {
		logrus.WithError(err).Info("create() trace")
		return nil, err
	}

	// network rollback
	defer func() {
		if err != nil {
			logrus.WithError(err).Info("create() trace")
			s.removeNetwork(ctx)
		}
	}()

	// Set the sandbox host cgroups.
	if err := s.setupResourceController(); err != nil {
		logrus.WithError(err).Info("create() trace")
		return nil, err
	}

	// Start the VM
	if err = s.startVM(ctx, prestartHookFunc); err != nil {
		logrus.WithError(err).Info("create() trace")
		return nil, err
	}

	// rollback to stop VM if error occurs
	defer func() {
		if err != nil {
			s.stopVM(ctx)
		}
	}()

	// s.postCreatedNetwork(ctx)

	// if err = s.getAndStoreGuestDetails(ctx); err != nil {
	// 	return nil, err
	// }

	// // Create Containers
	// if err = s.createContainers(ctx); err != nil {
	// 	return nil, err
	// }

	s.Logger().Info("created standbox from config !!!")

	return s, nil
}

// CleanupContainer is used by shimv2 to stop and delete a container exclusively, once there is no container
// in the sandbox left, do stop the sandbox and delete it. Those serial operations will be done exclusively by
// locking the sandbox.
func CleanupContainer(ctx context.Context, sandboxID, containerID string, force bool) error {
	logrus.Debugf("Entering CleanupContainer for Container ID - %v", containerID)
	span, ctx := katatrace.Trace(ctx, virtLog, "CleanupContainer", apiTracingTags)
	defer span.End()

	// debugging in process, was trying to figure out which return was causing an error

	if sandboxID == "" {
		return vcTypes.ErrNeedSandboxID
	}

	// if containerID == "" {
	// 	return vcTypes.ErrNeedContainerID
	// }

	unlock, err := rwLockSandbox(sandboxID)
	if err != nil {
		logrus.WithError(err).Info("failing at rwLock")
		return err
	}
	defer unlock()

	logrus.Debug("Entering fetchSandbox")
	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		logrus.WithError(err).Info("failing at fetch Sandbox")
		return err
	}
	defer s.Release(ctx)

	// _, err = s.StopContainer(ctx, containerID, force)
	// if err != nil && !force {
	// 	return err
	// }

	// _, err = s.DeleteContainer(ctx, containerID)
	// if err != nil && !force {
	// 	return err
	// }

	if len(s.GetAllContainers()) > 0 {
		logrus.Debugf("CleanupContainer - Failing out with non-zero containers: %d", len(s.GetAllContainers()))
		return nil
	}

	logrus.Debug("Entering Stop")
	if err = s.Stop(ctx, force); err != nil && !force {
		logrus.Info("failing at stop")
		return err
	}

	logrus.Debug("Entering Delete")
	if err = s.Delete(ctx); err != nil {
		logrus.Info("failing at delete")
		return err
	}

	logrus.Debug("CleanupContainer() trace from api.go")
	return nil
}
