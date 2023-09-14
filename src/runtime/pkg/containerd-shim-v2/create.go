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

	"github.com/container-orchestrated-devices/container-device-interface/pkg/cdi"
	containerd_types "github.com/containerd/containerd/api/types"
	"github.com/containerd/containerd/mount"
	taskAPI "github.com/containerd/containerd/runtime/v2/task"
	"github.com/containerd/typeurl"
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

// CDI (Container Device Interface), is a specification, for container- runtimes,
// to support third-party devices.
// It introduces an abstract notion of a device as a resource. Such devices are
// uniquely specified by a fully-qualified name that is constructed from a
// vendor ID, a device class, and a name that is unique per vendor ID-device
// class pair.
//
// vendor.com/class=unique_name
//
// The combination of vendor ID and device class (vendor.com/class in the
// above example) is referred to as the device kind.
// CDI concerns itself only with enabling containers to be device aware.
// Areas like resource management are explicitly left out of CDI (and are
// expected to be handled by the orchestrator). Because of this focus, the CDI
// specification is simple to implement and allows great flexibility for
// runtimes and orchestrators.
func withCDI(annotations map[string]string, cdiSpecDirs []string, spec *specs.Spec) (*specs.Spec, error) {
	// Add devices from CDI annotations
	_, devsFromAnnotations, err := cdi.ParseAnnotations(annotations)
	if err != nil {
		return nil, fmt.Errorf("failed to parse CDI device annotations: %w", err)
	}
	if len(devsFromAnnotations) == 0 {
		// No devices found, skip device injection
		return spec, nil
	}

	var registry cdi.Registry
	if len(cdiSpecDirs) > 0 {
		// We can override the directories where to search for CDI specs
		// if needed, the default is /etc/cdi /var/run/cdi
		registry = cdi.GetRegistry(cdi.WithSpecDirs(cdiSpecDirs...))
	} else {
		registry = cdi.GetRegistry()
	}

	if err = registry.Refresh(); err != nil {
		// We don't consider registry refresh failure a fatal error.
		// For instance, a dynamically generated invalid CDI Spec file for
		// any particular vendor shouldn't prevent injection of devices of
		// different vendors. CDI itself knows better and it will fail the
		// injection if necessary.
		return nil, fmt.Errorf("CDI registry refresh failed: %w", err)
	}

	if _, err := registry.InjectDevices(spec, devsFromAnnotations...); err != nil {
		return nil, fmt.Errorf("CDI device injection failed: %w", err)
	}

	// One crucial thing to keep in mind is that CDI device injection
	// might add OCI Spec environment variables, hooks, and mounts as
	// well. Therefore it is important that none of the corresponding
	// OCI Spec fields are reset up in the call stack once we return.
	return spec, nil
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
	logrus.Debugf("Sharath containerdshim create() ociSpec - %+v", ociSpec)

	if err != nil {
		return nil, err
	}

	if err := copyLayersToMounts(&rootFs, ociSpec); err != nil {
		return nil, err
	}

	containerType, err := oci.ContainerType(*ociSpec)
	logrus.WithField("type", containerType).Info("Create container type")
	if err != nil {
		return nil, err
	}

	disableOutput := noNeedForOutput(detach, ociSpec.Process.Terminal)
	rootfs := filepath.Join(r.Bundle, "rootfs")

	logrus.Debugf("Sharath containerdshim create() ociSpec VM- %+v", ociSpec.VM)
	logrus.Debugf("Sharath containerdshim create() ociSpec Annotations - %+v", ociSpec.Annotations)
	runtimeConfig, err := loadRuntimeConfig(s, r, ociSpec.Annotations)
	if err != nil {
		return nil, err
	}
	logrus.WithFields(logrus.Fields{"id": r.ID, "type": containerType}).Info("before switch")
	switch containerType {
	case vc.PodSandbox, vc.SingleContainer:
		logrus.Info("podsandbox case")
		if s.sandbox != nil {
			logrus.WithField("return", 1).Info("create exit")
			return nil, fmt.Errorf("cannot create another sandbox in sandbox: %s", s.sandbox.ID())
		}
		// We can provide additional directories where to search for
		// CDI specs if needed. immutable OS's only have specific
		// directories where applications can write too. For instance /opt/cdi
		//
		// _, err = withCDI(ociSpec.Annotations, []string{"/opt/cdi"}, ociSpec)
		//
		_, err = withCDI(ociSpec.Annotations, []string{}, ociSpec)
		if err != nil {
			logrus.WithField("return", 2).Info("create exit")
			return nil, fmt.Errorf("adding CDI devices failed")
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
			logrus.WithField("return", 3).Info("create exit")
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
			logrus.WithField("return", 4).Info("create exit")
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
				logrus.WithField("return", 5).Info("create exit")
				return nil, err
			}
		}

		// Pass service's context instead of local ctx to CreateSandbox(), since local
		// ctx will be canceled after this rpc service call, but the sandbox will live
		// across multiple rpc service calls.
		//
		logrus.Debugf("WILL NOT HAVE - Sharath containerdshim sandboxConfig Hypervisorconfig - %+v", s.config.HypervisorConfig)
		sandbox, _, err := katautils.CreateSandbox(s.ctx, vci, *ociSpec, *s.config, rootFs, r.ID, bundlePath, disableOutput, false)
		if err != nil {
			logrus.WithError(err).Info("*failure in create()")
			logrus.WithField("return", 6).Info("create exit")
			return nil, err
		}
		s.sandbox = sandbox
		pid, err := s.sandbox.GetHypervisorPid()
		if err != nil {
			logrus.WithField("return", 7).Info("create exit")
			return nil, err
		}
		s.hpid = uint32(pid)

		if defaultStartManagementServerFunc != nil {
			defaultStartManagementServerFunc(s, ctx, ociSpec)
		}

	case vc.PodContainer:
		logrus.Info("podcontainer case")
		// Sharath: Removing ctx, since it is not being used due to commenting CreateContainer code
		span, _ := katatrace.Trace(s.ctx, shimLog, "create", shimTracingTags)
		defer span.End()

		if s.sandbox == nil {
			logrus.WithField("return", 8).Info("create exit")
			return nil, fmt.Errorf("BUG: Cannot start the container, since the sandbox hasn't been created")
		}

		if rootFs.Mounted, err = checkAndMount(s, r); err != nil {
			logrus.WithField("return", 9).Info("create exit")
			return nil, err
		}

		defer func() {
			if err != nil && rootFs.Mounted {
				if err2 := mount.UnmountAll(rootfs, 0); err2 != nil {
					shimLog.WithField("container-type", containerType).WithError(err2).Warn("failed to cleanup rootfs mount")
				}
			}
		}()

		// Sharath: Commenting since we don't want to create new Container inside PodSandbox.
		// _, err = katautils.CreateContainer(ctx, s.sandbox, *ociSpec, rootFs, r.ID, bundlePath, disableOutput, runtimeConfig.DisableGuestEmptyDir)
		// if err != nil {
		// 	logrus.WithField("return", 10).Info("create exit")
		// 	return nil, err
		// }
		// Sharath: New Code
		if err = s.sandbox.StoreSandbox(ctx); err != nil {
			return nil, err
		}
	}

	container, err := newContainer(s, r, containerType, ociSpec, rootFs.Mounted)
	if err != nil {
		logrus.WithField("return", 11).Info("create exit")
		return nil, err
	}

	logrus.WithField("return", 12).Info("create exit")
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
	// In the confidential computing, there is no Image information on the host,
	// so there is no Rootfs.
	if s.config.ServiceOffload && len(r.Rootfs) == 0 {
		return false, nil
	}
	if len(r.Rootfs) == 1 {
		m := r.Rootfs[0]

		// Plug the block backed rootfs directly instead of mounting it.
		if katautils.IsBlockDevice(m.Source) && !s.config.HypervisorConfig.DisableBlockDeviceUse {
			return false, nil
		}

		if virtcontainers.HasOptionPrefix(m.Options, annotations.FileSystemLayer) {
			return false, nil
		}

		if m.Type == vc.NydusRootFSType {
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
