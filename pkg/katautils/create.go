// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"context"
	"fmt"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vf "github.com/kata-containers/runtime/virtcontainers/factory"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
)

// GetKernelParamsFunc use a variable to allow tests to modify its value
var GetKernelParamsFunc = getKernelParams

var systemdKernelParam = []vc.Param{
	{
		Key:   "init",
		Value: "/usr/lib/systemd/systemd",
	},
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

func getKernelParams(needSystemd bool) []vc.Param {
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
	if !runtimeConfig.FactoryConfig.Template {
		return
	}

	factoryConfig := vf.Config{
		Template: true,
		VMConfig: vc.VMConfig{
			HypervisorType:   runtimeConfig.HypervisorType,
			HypervisorConfig: runtimeConfig.HypervisorConfig,
			AgentType:        runtimeConfig.AgentType,
			AgentConfig:      runtimeConfig.AgentConfig,
		},
	}

	kataUtilsLogger.WithField("factory", factoryConfig).Info("load vm factory")

	f, err := vf.NewFactory(ctx, factoryConfig, true)
	if err != nil {
		kataUtilsLogger.WithError(err).Warn("load vm factory failed, about to create new one")
		f, err = vf.NewFactory(ctx, factoryConfig, false)
		if err != nil {
			kataUtilsLogger.WithError(err).Warn("create vm factory failed")
			return
		}
	}

	vci.SetFactory(ctx, f)
}

// SetEphemeralStorageType sets the mount type to 'ephemeral'
// if the mount source path is provisioned by k8s for ephemeral storage.
// For the given pod ephemeral volume is created only once
// backed by tmpfs inside the VM. For successive containers
// of the same pod the already existing volume is reused.
func SetEphemeralStorageType(ociSpec oci.CompatOCISpec) oci.CompatOCISpec {
	for idx, mnt := range ociSpec.Mounts {
		if IsEphemeralStorage(mnt.Source) {
			ociSpec.Mounts[idx].Type = "ephemeral"
		}
	}
	return ociSpec
}

// CreateSandbox create a sandbox container
func CreateSandbox(ctx context.Context, vci vc.VC, ociSpec oci.CompatOCISpec, runtimeConfig oci.RuntimeConfig,
	containerID, bundlePath, console string, disableOutput, systemdCgroup, builtIn bool) (vc.VCSandbox, vc.Process, error) {
	span, ctx := Trace(ctx, "createSandbox")
	defer span.Finish()

	sandboxConfig, err := oci.SandboxConfig(ociSpec, runtimeConfig, bundlePath, containerID, console, disableOutput, systemdCgroup)
	if err != nil {
		return nil, vc.Process{}, err
	}

	if builtIn {
		sandboxConfig.Stateful = true
	}

	// Important to create the network namespace before the sandbox is
	// created, because it is not responsible for the creation of the
	// netns if it does not exist.
	if err := SetupNetworkNamespace(&sandboxConfig.NetworkConfig); err != nil {
		return nil, vc.Process{}, err
	}

	// Run pre-start OCI hooks.
	err = EnterNetNS(sandboxConfig.NetworkConfig.NetNSPath, func() error {
		return PreStartHooks(ctx, ociSpec, containerID, bundlePath)
	})
	if err != nil {
		return nil, vc.Process{}, err
	}

	sandbox, err := vci.CreateSandbox(ctx, sandboxConfig)
	if err != nil {
		return nil, vc.Process{}, err
	}

	sid := sandbox.ID()
	kataUtilsLogger = kataUtilsLogger.WithField("sandbox", sid)
	span.SetTag("sandbox", sid)

	containers := sandbox.GetAllContainers()
	if len(containers) != 1 {
		return nil, vc.Process{}, fmt.Errorf("BUG: Container list from sandbox is wrong, expecting only one container, found %d containers", len(containers))
	}

	if !builtIn {
		err = AddContainerIDMapping(ctx, containerID, sandbox.ID())
		if err != nil {
			return nil, vc.Process{}, err
		}
	}

	return sandbox, containers[0].Process(), nil
}

// CreateContainer create a container
func CreateContainer(ctx context.Context, vci vc.VC, sandbox vc.VCSandbox, ociSpec oci.CompatOCISpec, containerID, bundlePath, console string, disableOutput, builtIn bool) (vc.Process, error) {
	var c vc.VCContainer

	span, ctx := Trace(ctx, "createContainer")
	defer span.Finish()

	ociSpec = SetEphemeralStorageType(ociSpec)

	contConfig, err := oci.ContainerConfig(ociSpec, bundlePath, containerID, console, disableOutput)
	if err != nil {
		return vc.Process{}, err
	}

	sandboxID, err := ociSpec.SandboxID()
	if err != nil {
		return vc.Process{}, err
	}

	span.SetTag("sandbox", sandboxID)

	if builtIn {
		c, err = sandbox.CreateContainer(contConfig)
		if err != nil {
			return vc.Process{}, err
		}
	} else {
		kataUtilsLogger = kataUtilsLogger.WithField("sandbox", sandboxID)

		sandbox, c, err = vci.CreateContainer(ctx, sandboxID, contConfig)
		if err != nil {
			return vc.Process{}, err
		}

		if err := AddContainerIDMapping(ctx, containerID, sandboxID); err != nil {
			return vc.Process{}, err
		}

		kataUtilsLogger = kataUtilsLogger.WithField("sandbox", sandboxID)

		if err := AddContainerIDMapping(ctx, containerID, sandboxID); err != nil {
			return vc.Process{}, err
		}
	}

	// Run pre-start OCI hooks.
	err = EnterNetNS(sandbox.GetNetNs(), func() error {
		return PreStartHooks(ctx, ociSpec, containerID, bundlePath)
	})
	if err != nil {
		return vc.Process{}, err
	}

	return c.Process(), nil
}
