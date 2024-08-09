// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
// Copyright (c) 2021 Adobe Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"os"
	"os/user"
	"path"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	containerd_types "github.com/containerd/containerd/api/types"
	"github.com/containerd/containerd/mount"
	"github.com/containerd/typeurl/v2"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"

	// only register the proto type
	crioption "github.com/containerd/containerd/pkg/runtimeoptions/v1"
	_ "github.com/containerd/containerd/runtime/linux/runctypes"
	_ "github.com/containerd/containerd/runtime/v2/runc/options"
	oldcrioption "github.com/containerd/cri-containerd/pkg/api/runtimeoptions/v1"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
)

type startManagementServerFunc func(s *service, ctx context.Context, ociSpec *specs.Spec)

var defaultStartManagementServerFunc startManagementServerFunc = func(s *service, ctx context.Context, ociSpec *specs.Spec) {
	go s.startManagementServer(ctx, ociSpec)
	shimLog.Info("management server started")
}

func copyLayersToMounts(rootFs *vc.RootFs, spec *specs.Spec) error {
	for _, o := range rootFs.Options {
		if !strings.HasPrefix(o, annotations.FileSystemLayer) {
			continue
		}

		fields := strings.Split(o[len(annotations.FileSystemLayer):], ",")
		if len(fields) < 2 {
			return fmt.Errorf("Missing fields in rootfs layer: %q", o)
		}

		spec.Mounts = append(spec.Mounts, specs.Mount{
			Destination: "/run/kata-containers/sandbox/layers/" + filepath.Base(fields[0]),
			Type:        fields[1],
			Source:      fields[0],
			Options:     fields[2:],
		})
	}

	return nil
}

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

	if err := copyLayersToMounts(&rootFs, ociSpec); err != nil {
		return nil, err
	}

	containerType, err := oci.ContainerType(*ociSpec)
	if err != nil {
		return nil, err
	}

	disableOutput := noNeedForOutput(detach, ociSpec.Process.Terminal)
	rootfs := filepath.Join(r.Bundle, "rootfs")

	runtimeConfig, err := loadRuntimeConfig(s, r, ociSpec.Annotations)
	if err != nil {
		return nil, err
	}

	switch containerType {
	case vc.PodSandbox, vc.SingleContainer:
		if s.sandbox != nil {
			return nil, fmt.Errorf("cannot create another sandbox in sandbox: %s", s.sandbox.ID())
		}
		// We can provide additional directories where to search for
		// CDI specs if needed. immutable OS's only have specific
		// directories where applications can write too. For instance /opt/cdi
		//
		// _, err = withCDI(ociSpec.Annotations, []string{"/opt/cdi"}, ociSpec)
		//
		// Only inject CDI devices if single_container we do not want
		// CDI devices in the pod_sandbox
		if containerType == vc.SingleContainer {
			_, err = config.WithCDI(ociSpec.Annotations, []string{}, ociSpec)
			if err != nil {
				return nil, fmt.Errorf("adding CDI devices failed")
			}
		}

		s.config = runtimeConfig

		// create tracer
		// This is the earliest location we can create the tracer because we must wait
		// until the runtime config is loaded
		jaegerConfig := &katatrace.JaegerConfig{
			JaegerEndpoint: s.config.JaegerEndpoint,
			JaegerUser:     s.config.JaegerUser,
			JaegerPassword: s.config.JaegerPassword,
		}
		_, err = katatrace.CreateTracer("kata", jaegerConfig)
		if err != nil {
			return nil, err
		}

		// create root span
		// rootSpan will be ended when the entire trace is ended
		rootSpan, newCtx := katatrace.Trace(s.ctx, shimLog, "rootSpan", shimTracingTags)
		s.rootCtx = newCtx
		s.rootSpan = rootSpan

		// create span
		span, newCtx := katatrace.Trace(s.rootCtx, shimLog, "create", shimTracingTags)
		s.ctx = newCtx
		defer span.End()

		// Sandbox sizing information *may* be provided in two scenarios:
		//   1. The upper layer runtime (ie, containerd or crio) provide sandbox sizing information as an annotation
		//	in the 'sandbox container's' spec. This would typically be a scenario where as part of a create sandbox
		//	request the upper layer runtime receives this information as part of a pod, and makes it available to us
		//	for sizing purposes.
		//   2. If this is not a sandbox infrastructure container, but instead a standalone single container (analogous to "docker run..."),
		//	then the container spec itself will contain appropriate sizing information for the entire sandbox (since it is
		//	a single container.
		if containerType == vc.PodSandbox {
			s.config.SandboxCPUs, s.config.SandboxMemMB = oci.CalculateSandboxSizing(ociSpec)
		} else {
			s.config.SandboxCPUs, s.config.SandboxMemMB = oci.CalculateContainerSizing(ociSpec)
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
		rootless.SetRootless(s.config.HypervisorConfig.Rootless)
		if rootless.IsRootless() {
			if err := configureNonRootHypervisor(s.config, r.ID); err != nil {
				return nil, err
			}
		}

		// Pass service's context instead of local ctx to CreateSandbox(), since local
		// ctx will be canceled after this rpc service call, but the sandbox will live
		// across multiple rpc service calls.
		//
		sandbox, _, err := katautils.CreateSandbox(s.ctx, vci, *ociSpec, *s.config, rootFs, r.ID, bundlePath, disableOutput, false)
		if err != nil {
			return nil, err
		}
		s.sandbox = sandbox
		pid, err := s.sandbox.GetHypervisorPid()
		if err != nil {
			return nil, err
		}
		s.hpid = uint32(pid)

		if defaultStartManagementServerFunc != nil {
			defaultStartManagementServerFunc(s, ctx, ociSpec)
		}

	case vc.PodContainer:
		span, ctx := katatrace.Trace(s.ctx, shimLog, "create", shimTracingTags)
		defer span.End()

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

		_, err = katautils.CreateContainer(ctx, s.sandbox, *ociSpec, rootFs, r.ID, bundlePath, disableOutput, runtimeConfig.DisableGuestEmptyDir)
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
		} else {
			// Some versions of containerd, such as 1.4.3, and 1.4.4
			// still rely on the runtime options coming from
			// github.com/containerd/cri-containerd/pkg/api/runtimeoptions/v1
			// Knowing that, instead of breaking compatibility with such
			// versions, let's work this around on our side
			oldOption, ok := v.(*oldcrioption.Options)
			if ok {
				configPath = oldOption.ConfigPath
			}
		}
	}

	// Try to get the config file from the env KATA_CONF_FILE
	if configPath == "" {
		configPath = os.Getenv("KATA_CONF_FILE")
	}

	_, runtimeConfig, err := katautils.LoadConfiguration(configPath, false)
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

		if virtcontainers.HasOptionPrefix(m.Options, annotations.FileSystemLayer) {
			return false, nil
		}

		if vc.IsNydusRootFSType(m.Type) {
			// if kata + nydus, do not mount
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

func configureNonRootHypervisor(runtimeConfig *oci.RuntimeConfig, sandboxId string) error {
	userName, err := utils.CreateVmmUser()
	if err != nil {
		return err
	}
	defer func() {
		if err != nil {
			shimLog.WithFields(logrus.Fields{
				"user_name":  userName,
				"sandbox_id": sandboxId,
			}).WithError(err).Warn("configure non root hypervisor failed, delete the user")
			if err2 := utils.RemoveVmmUser(userName); err2 != nil {
				shimLog.WithField("userName", userName).WithError(err).Warn("failed to remove user")
			}
		}
	}()

	u, err := user.Lookup(userName)
	if err != nil {
		return err
	}

	uid, err := strconv.Atoi(u.Uid)
	if err != nil {
		return err
	}
	gid, err := strconv.Atoi(u.Gid)
	if err != nil {
		return err
	}
	runtimeConfig.HypervisorConfig.Uid = uint32(uid)
	runtimeConfig.HypervisorConfig.User = userName
	runtimeConfig.HypervisorConfig.Gid = uint32(gid)
	shimLog.WithFields(logrus.Fields{
		"user_name":  userName,
		"uid":        uid,
		"gid":        gid,
		"sandbox_id": sandboxId,
	}).Debug("successfully created a non root user for the hypervisor")

	userTmpDir := path.Join("/run/user/", fmt.Sprint(uid))
	_, err = os.Stat(userTmpDir)
	// Clean up the directory created by the previous run
	if !os.IsNotExist(err) {
		if err = os.RemoveAll(userTmpDir); err != nil {
			return err
		}
	}

	if err = os.Mkdir(userTmpDir, vc.DirMode); err != nil {
		return err
	}
	defer func() {
		if err != nil {
			if err = os.RemoveAll(userTmpDir); err != nil {
				shimLog.WithField("userTmpDir", userTmpDir).WithError(err).Warn("failed to remove userTmpDir")
			}
		}
	}()
	if err = syscall.Chown(userTmpDir, uid, gid); err != nil {
		return err
	}

	if err := os.Setenv("XDG_RUNTIME_DIR", userTmpDir); err != nil {
		return err
	}

	info, err := os.Stat("/dev/kvm")
	if err != nil {
		return err
	}
	if stat, ok := info.Sys().(*syscall.Stat_t); ok {
		// Add the kvm group to the hypervisor supplemental group so that the hypervisor process can access /dev/kvm
		runtimeConfig.HypervisorConfig.Groups = append(runtimeConfig.HypervisorConfig.Groups, stat.Gid)
		return nil
	}
	return fmt.Errorf("failed to get the gid of /dev/kvm")
}
