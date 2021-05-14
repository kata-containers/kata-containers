// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"runtime"

	deviceApi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/api"
	deviceConfig "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/cgroups"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
	"github.com/sirupsen/logrus"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/label"
	otelTrace "go.opentelemetry.io/otel/trace"
)

func init() {
	runtime.LockOSThread()
}

var virtLog = logrus.WithField("source", "virtcontainers")

// trace creates a new tracing span based on the specified name and parent
// context.
func trace(parent context.Context, name string) (otelTrace.Span, context.Context) {
	tracer := otel.Tracer("kata")
	ctx, span := tracer.Start(parent, name, otelTrace.WithAttributes(label.String("source", "runtime"), label.String("packages", "virtcontainers"), label.String("subsystem", "api")))

	return span, ctx
}

// SetLogger sets the logger for virtcontainers package.
func SetLogger(ctx context.Context, logger *logrus.Entry) {
	fields := virtLog.Data
	virtLog = logger.WithFields(fields)

	deviceApi.SetLogger(virtLog)
	compatoci.SetLogger(virtLog)
	deviceConfig.SetLogger(virtLog)
	cgroups.SetLogger(virtLog)
}

// CreateSandbox is the virtcontainers sandbox creation entry point.
// CreateSandbox creates a sandbox and its containers. It does not start them.
func CreateSandbox(ctx context.Context, sandboxConfig SandboxConfig, factory Factory) (VCSandbox, error) {
	span, ctx := trace(ctx, "CreateSandbox")
	defer span.End()

	s, err := createSandboxFromConfig(ctx, sandboxConfig, factory)

	return s, err
}

func createSandboxFromConfig(ctx context.Context, sandboxConfig SandboxConfig, factory Factory) (_ *Sandbox, err error) {
	span, ctx := trace(ctx, "createSandboxFromConfig")
	defer span.End()

	// Create the sandbox.
	s, err := createSandbox(ctx, sandboxConfig, factory)
	if err != nil {
		return nil, err
	}

	// cleanup sandbox resources in case of any failure
	defer func() {
		if err != nil {
			s.Delete(ctx)
		}
	}()

	// Create the sandbox network
	if err = s.createNetwork(ctx); err != nil {
		return nil, err
	}

	// network rollback
	defer func() {
		if err != nil {
			s.removeNetwork(ctx)
		}
	}()

	// Move runtime to sandbox cgroup so all process are created there.
	if s.config.SandboxCgroupOnly {
		if err := s.createCgroupManager(); err != nil {
			return nil, err
		}

		if err := s.setupSandboxCgroup(); err != nil {
			return nil, err
		}
	}

	// Start the VM
	if err = s.startVM(ctx); err != nil {
		return nil, err
	}

	// rollback to stop VM if error occurs
	defer func() {
		if err != nil {
			s.stopVM(ctx)
		}
	}()

	s.postCreatedNetwork(ctx)

	if err = s.getAndStoreGuestDetails(ctx); err != nil {
		return nil, err
	}

	// Create Containers
	if err = s.createContainers(ctx); err != nil {
		return nil, err
	}

	// The sandbox is completely created now, we can store it.
	if err = s.storeSandbox(ctx); err != nil {
		return nil, err
	}

	return s, nil
}

// CleanupContainer is used by shimv2 to stop and delete a container exclusively, once there is no container
// in the sandbox left, do stop the sandbox and delete it. Those serial operations will be done exclusively by
// locking the sandbox.
func CleanupContainer(ctx context.Context, sandboxID, containerID string, force bool) error {
	span, ctx := trace(ctx, "CleanupContainer")
	defer span.End()

	if sandboxID == "" {
		return vcTypes.ErrNeedSandboxID
	}

	if containerID == "" {
		return vcTypes.ErrNeedContainerID
	}

	unlock, err := rwLockSandbox(sandboxID)
	if err != nil {
		return err
	}
	defer unlock()

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return err
	}
	defer s.Release(ctx)

	_, err = s.StopContainer(ctx, containerID, force)
	if err != nil && !force {
		return err
	}

	_, err = s.DeleteContainer(ctx, containerID)
	if err != nil && !force {
		return err
	}

	if len(s.GetAllContainers()) > 0 {
		return nil
	}

	if err = s.Stop(ctx, force); err != nil && !force {
		return err
	}

	if err = s.Delete(ctx); err != nil {
		return err
	}

	return nil
}
