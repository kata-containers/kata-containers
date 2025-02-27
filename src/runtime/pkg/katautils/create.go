// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
// Copyright (c) 2021 Adobe Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"context"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vf "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	specs "github.com/opencontainers/runtime-spec/specs-go"
)

// createTracingTags defines tags for the trace span
var createTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "katautils",
	"subsystem": "sandbox",
}

// GetKernelParamsFunc use a variable to allow tests to modify its value
var GetKernelParamsFunc = getKernelParams

var systemdKernelParam = []vc.Param{
	{
		Key:   "systemd.unit",
		Value: systemdUnitName,
	},
	{
		Key:   "systemd.mask",
		Value: "systemd-networkd.service",
	},
	{
		Key:   "systemd.mask",
		Value: "systemd-networkd.socket",
	},
}

func getKernelParams(needSystemd, trace bool) []vc.Param {
	p := []vc.Param{}

	if needSystemd {
		p = append(p, systemdKernelParam...)
	}

	return p
}

func needSystemd(config vc.HypervisorConfig) bool {
	return config.ImagePath != ""
}

// HandleFactory  set the factory
func HandleFactory(ctx context.Context, vci vc.VC, runtimeConfig *oci.RuntimeConfig) {
	if !runtimeConfig.FactoryConfig.Template && runtimeConfig.FactoryConfig.VMCacheNumber == 0 {
		return
	}
	factoryConfig := vf.Config{
		Template:        runtimeConfig.FactoryConfig.Template,
		TemplatePath:    runtimeConfig.FactoryConfig.TemplatePath,
		VMCache:         runtimeConfig.FactoryConfig.VMCacheNumber > 0,
		VMCacheEndpoint: runtimeConfig.FactoryConfig.VMCacheEndpoint,
		VMConfig: vc.VMConfig{
			HypervisorType:   runtimeConfig.HypervisorType,
			HypervisorConfig: runtimeConfig.HypervisorConfig,
			AgentConfig:      runtimeConfig.AgentConfig,
		},
	}

	kataUtilsLogger.WithField("factory", factoryConfig).Info("load vm factory")

	f, err := vf.NewFactory(ctx, factoryConfig, true)
	if err != nil && !factoryConfig.VMCache {
		kataUtilsLogger.WithError(err).Warn("load vm factory failed, about to create new one")
		f, err = vf.NewFactory(ctx, factoryConfig, false)
	}
	if err != nil {
		kataUtilsLogger.WithError(err).Warn("create vm factory failed")
		return
	}

	vci.SetFactory(ctx, f)
}

// SetEphemeralStorageType sets the mount type to 'ephemeral'
// if the mount source path is provisioned by k8s for ephemeral storage.
// For the given pod ephemeral volume is created only once
// backed by tmpfs inside the VM. For successive containers
// of the same pod the already existing volume is reused.
func SetEphemeralStorageType(ociSpec specs.Spec, disableGuestEmptyDir bool) specs.Spec {
	for idx, mnt := range ociSpec.Mounts {
		if vc.IsEphemeralStorage(mnt.Source) {
			ociSpec.Mounts[idx].Type = vc.KataEphemeralDevType
		}
		if vc.Isk8sHostEmptyDir(mnt.Source) && !disableGuestEmptyDir {
			ociSpec.Mounts[idx].Type = vc.KataLocalDevType
		}
	}
	return ociSpec
}

// CreateSandbox create a sandbox container
func CreateSandbox(ctx context.Context, vci vc.VC, ociSpec specs.Spec, runtimeConfig oci.RuntimeConfig, rootFs vc.RootFs,
	containerID, bundlePath string, disableOutput, systemdCgroup bool) (_ vc.VCSandbox, _ vc.Process, err error) {
	span, ctx := katatrace.Trace(ctx, nil, "CreateSandbox", createTracingTags)
	katatrace.AddTags(span, "container_id", containerID)
	defer span.End()

	sandboxConfig, err := oci.SandboxConfig(ociSpec, runtimeConfig, bundlePath, containerID, disableOutput, systemdCgroup)
	if err != nil {
		return nil, vc.Process{}, err
	}

	// setup shared path in hypervisor config:
	sandboxConfig.HypervisorConfig.SharedPath = vc.GetSharePath(containerID)

	if err := checkForFIPS(&sandboxConfig); err != nil {
		return nil, vc.Process{}, err
	}

	if !rootFs.Mounted && len(sandboxConfig.Containers) == 1 {
		if rootFs.Source != "" && !vc.HasOptionPrefix(rootFs.Options, vc.VirtualVolumePrefix) {
			realPath, err := ResolvePath(rootFs.Source)
			if err != nil {
				return nil, vc.Process{}, err
			}
			rootFs.Source = realPath
		}
		sandboxConfig.Containers[0].RootFs = rootFs
	}

	// Important to create the network namespace before the sandbox is
	// created, because it is not responsible for the creation of the
	// netns if it does not exist.
	if err := SetupNetworkNamespace(&sandboxConfig.NetworkConfig); err != nil {
		return nil, vc.Process{}, err
	}

	defer func() {
		// cleanup netns if kata creates it
		ns := sandboxConfig.NetworkConfig
		if err != nil && ns.NetworkCreated {
			if ex := cleanupNetNS(ns.NetworkID); ex != nil {
				kataUtilsLogger.WithField("id", ns.NetworkID).WithError(ex).Warn("failed to cleanup network")
			}
		}
	}()

	if ociSpec.Annotations == nil {
		ociSpec.Annotations = make(map[string]string)
	}
	ociSpec.Annotations["nerdctl/network-namespace"] = sandboxConfig.NetworkConfig.NetworkID
	sandboxConfig.Annotations["nerdctl/network-namespace"] = ociSpec.Annotations["nerdctl/network-namespace"]

	// The value of this annotation is sent to the sandbox using SetPolicy.
	delete(ociSpec.Annotations, vcAnnotations.Policy)
	delete(sandboxConfig.Annotations, vcAnnotations.Policy)

	sandbox, err := vci.CreateSandbox(ctx, sandboxConfig, func(ctx context.Context) error {
		// Run pre-start OCI hooks, in the runtime namespace.
		if err := PreStartHooks(ctx, ociSpec, containerID, bundlePath); err != nil {
			return err
		}

		// Run create runtime OCI hooks, in the runtime namespace.
		if err := CreateRuntimeHooks(ctx, ociSpec, containerID, bundlePath); err != nil {
			return err
		}

		return nil
	})
	if err != nil {
		return nil, vc.Process{}, err
	}

	sid := sandbox.ID()
	kataUtilsLogger = kataUtilsLogger.WithField("sandbox", sid)
	katatrace.AddTags(span, "sandbox_id", sid)

	containers := sandbox.GetAllContainers()
	if len(containers) != 1 {
		return nil, vc.Process{}, fmt.Errorf("BUG: Container list from sandbox is wrong, expecting only one container, found %d containers", len(containers))
	}

	return sandbox, containers[0].Process(), nil
}

var procFIPS = "/proc/sys/crypto/fips_enabled"

func checkForFIPS(sandboxConfig *vc.SandboxConfig) error {
	content, err := os.ReadFile(procFIPS)
	if err != nil {
		// In case file cannot be found or read, simply return
		return nil
	}

	enabled, err := strconv.Atoi(strings.Trim(string(content), "\n\t "))
	if err != nil {
		// Unexpected format, ignore and simply return early
		return nil
	}

	if enabled == 1 {
		param := vc.Param{
			Key:   "fips",
			Value: "1",
		}

		if err := sandboxConfig.HypervisorConfig.AddKernelParam(param); err != nil {
			return fmt.Errorf("Error enabling fips mode : %v", err)
		}
	}

	return nil
}

// CreateContainer create a container
func CreateContainer(ctx context.Context, sandbox vc.VCSandbox, ociSpec specs.Spec, rootFs vc.RootFs, containerID, bundlePath string, disableOutput bool, disableGuestEmptyDir bool) (vc.Process, error) {
	var c vc.VCContainer

	span, ctx := katatrace.Trace(ctx, nil, "CreateContainer", createTracingTags)
	katatrace.AddTags(span, "container_id", containerID)
	defer span.End()

	// The value of this annotation is sent to the sandbox using SetPolicy.
	delete(ociSpec.Annotations, vcAnnotations.Policy)

	ociSpec = SetEphemeralStorageType(ociSpec, disableGuestEmptyDir)

	contConfig, err := oci.ContainerConfig(ociSpec, bundlePath, containerID, disableOutput)
	if err != nil {
		return vc.Process{}, err
	}

	if !rootFs.Mounted {
		if rootFs.Source != "" && !vc.IsNydusRootFSType(rootFs.Type) {
			realPath, err := ResolvePath(rootFs.Source)
			if err != nil {
				return vc.Process{}, err
			}
			rootFs.Source = realPath
		}
		contConfig.RootFs = rootFs
	}
	sandboxID, err := oci.SandboxID(ociSpec)
	if err != nil {
		return vc.Process{}, err
	}

	katatrace.AddTags(span, "sandbox_id", sandboxID)

	c, err = sandbox.CreateContainer(ctx, contConfig)
	if err != nil {
		return vc.Process{}, err
	}

	hid, err := sandbox.GetHypervisorPid()
	if err != nil {
		return vc.Process{}, err
	}
	ctx = context.WithValue(ctx, vc.HypervisorPidKey{}, hid)

	err = EnterNetNS(sandbox.GetNetNs(), func() error {
		// Run pre-start OCI hooks, in the runtime namespace.
		if err := PreStartHooks(ctx, ociSpec, containerID, bundlePath); err != nil {
			return err
		}

		// Run create runtime OCI hooks, in the runtime namespace.
		if err := CreateRuntimeHooks(ctx, ociSpec, containerID, bundlePath); err != nil {
			return err
		}

		return nil
	})
	if err != nil {
		return vc.Process{}, err
	}

	return c.Process(), nil
}
