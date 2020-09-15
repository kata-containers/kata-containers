// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"os"
	"path/filepath"

	containerd_types "github.com/containerd/containerd/api/types"
	"github.com/containerd/containerd/mount"
	taskAPI "github.com/containerd/containerd/runtime/v2/task"
	"github.com/containerd/typeurl"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/pkg/errors"

	// only register the proto type
	_ "github.com/containerd/containerd/runtime/linux/runctypes"
	crioption "github.com/containerd/cri-containerd/pkg/api/runtimeoptions/v1"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/oci"
)

func create(ctx context.Context, s *service, r *taskAPI.CreateTaskRequest) (*container, error) {
	rootFs := vc.RootFs{}
	if len(r.Rootfs) == 1 {
		m := r.Rootfs[0]
		rootFs.Source = m.Source
		rootFs.Type = m.Type
		rootFs.Options = m.Options
	}

	detach := !r.Terminal
	ociSpec, bundlePath, err := loadSpec(r)
	if err != nil {
		return nil, err
	}

	containerType, err := oci.ContainerType(*ociSpec)
	if err != nil {
		return nil, err
	}

	disableOutput := noNeedForOutput(detach, ociSpec.Process.Terminal)
	rootfs := filepath.Join(r.Bundle, "rootfs")

	switch containerType {
	case vc.PodSandbox:
		if s.sandbox != nil {
			return nil, fmt.Errorf("cannot create another sandbox in sandbox: %s", s.sandbox.ID())
		}

		_, err := loadRuntimeConfig(s, r, ociSpec.Annotations)
		if err != nil {
			return nil, err
		}

		if rootFs.Mounted, err = checkAndMount(s, r); err != nil {
			return nil, err
		}

		defer func() {
			if err != nil && rootFs.Mounted {
				if err2 := mount.UnmountAll(rootfs, 0); err2 != nil {
					shimLog.WithField("container-type", containerType).WithError(err2).Warn("failed to cleanup rootfs mount")
				}
			}
		}()

		katautils.HandleFactory(ctx, vci, s.config)

		// Pass service's context instead of local ctx to CreateSandbox(), since local
		// ctx will be canceled after this rpc service call, but the sandbox will live
		// across multiple rpc service calls.
		//
		sandbox, _, err := katautils.CreateSandbox(s.ctx, vci, *ociSpec, *s.config, rootFs, r.ID, bundlePath, "", disableOutput, false)
		if err != nil {
			return nil, err
		}
		s.sandbox = sandbox
		go s.startManagementServer(ctx, ociSpec)

	case vc.PodContainer:
		if s.sandbox == nil {
			return nil, fmt.Errorf("BUG: Cannot start the container, since the sandbox hasn't been created")
		}

		if rootFs.Mounted, err = checkAndMount(s, r); err != nil {
			return nil, err
		}

		defer func() {
			if err != nil && rootFs.Mounted {
				if err2 := mount.UnmountAll(rootfs, 0); err2 != nil {
					shimLog.WithField("container-type", containerType).WithError(err2).Warn("failed to cleanup rootfs mount")
				}
			}
		}()

		_, err = katautils.CreateContainer(ctx, s.sandbox, *ociSpec, rootFs, r.ID, bundlePath, "", disableOutput)
		if err != nil {
			return nil, err
		}
	}

	container, err := newContainer(s, r, containerType, ociSpec, rootFs.Mounted)
	if err != nil {
		return nil, err
	}

	return container, nil
}

func loadSpec(r *taskAPI.CreateTaskRequest) (*specs.Spec, string, error) {
	// Checks the MUST and MUST NOT from OCI runtime specification
	bundlePath, err := validBundle(r.ID, r.Bundle)
	if err != nil {
		return nil, "", err
	}

	ociSpec, err := compatoci.ParseConfigJSON(bundlePath)
	if err != nil {
		return nil, "", err
	}

	return &ociSpec, bundlePath, nil
}

// Config override ordering(high to low):
// 1. podsandbox annotation
// 2. shimv2 create task option
// 3. environment
func loadRuntimeConfig(s *service, r *taskAPI.CreateTaskRequest, anno map[string]string) (*oci.RuntimeConfig, error) {
	if s.config != nil {
		return s.config, nil
	}
	configPath := oci.GetSandboxConfigPath(anno)
	if configPath == "" && r.Options != nil {
		v, err := typeurl.UnmarshalAny(r.Options)
		if err != nil {
			return nil, err
		}
		option, ok := v.(*crioption.Options)
		// cri default runtime handler will pass a linux runc options,
		// and we'll ignore it.
		if ok {
			configPath = option.ConfigPath
		}
	}

	// Try to get the config file from the env KATA_CONF_FILE
	if configPath == "" {
		configPath = os.Getenv("KATA_CONF_FILE")
	}

	_, runtimeConfig, err := katautils.LoadConfiguration(configPath, false, true)
	if err != nil {
		return nil, err
	}

	// For the unit test, the config will be predefined
	if s.config == nil {
		s.config = &runtimeConfig
	}

	return &runtimeConfig, nil
}

func checkAndMount(s *service, r *taskAPI.CreateTaskRequest) (bool, error) {
	if len(r.Rootfs) == 1 {
		m := r.Rootfs[0]

		// Plug the block backed rootfs directly instead of mounting it.
		if katautils.IsBlockDevice(m.Source) && !s.config.HypervisorConfig.DisableBlockDeviceUse {
			return false, nil
		}
	}
	rootfs := filepath.Join(r.Bundle, "rootfs")
	if err := doMount(r.Rootfs, rootfs); err != nil {
		return false, err
	}
	return true, nil
}

func doMount(mounts []*containerd_types.Mount, rootfs string) error {
	if len(mounts) == 0 {
		return nil
	}

	if _, err := os.Stat(rootfs); os.IsNotExist(err) {
		if err := os.Mkdir(rootfs, 0711); err != nil {
			return err
		}
	}

	for _, rm := range mounts {
		m := &mount.Mount{
			Type:    rm.Type,
			Source:  rm.Source,
			Options: rm.Options,
		}
		if err := m.Mount(rootfs); err != nil {
			return errors.Wrapf(err, "failed to mount rootfs component %v", m)
		}
	}
	return nil
}
