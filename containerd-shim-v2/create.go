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

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"

	taskAPI "github.com/containerd/containerd/runtime/v2/task"

	"github.com/kata-containers/runtime/pkg/katautils"
	"github.com/opencontainers/runtime-spec/specs-go"
)

func create(ctx context.Context, s *service, r *taskAPI.CreateTaskRequest, netns string,
	runtimeConfig *oci.RuntimeConfig) (*container, error) {

	detach := !r.Terminal

	// Checks the MUST and MUST NOT from OCI runtime specification
	bundlePath, err := validBundle(r.ID, r.Bundle)
	if err != nil {
		return nil, err
	}

	ociSpec, err := oci.ParseConfigJSON(bundlePath)
	if err != nil {
		return nil, err
	}

	containerType, err := ociSpec.ContainerType()
	if err != nil {
		return nil, err
	}

	// Todo:
	// Since there is a bug in kata for sharedPidNs, here to
	// remove the pidns to disable the sharePidNs temporarily,
	// once kata fixed this issue, we can remove this line.
	// For the bug, please see:
	// https://github.com/kata-containers/runtime/issues/930
	removeNamespace(&ociSpec, specs.PIDNamespace)

	//set the network namespace path
	//this set will be applied to sandbox's
	//network config and has nothing to
	//do with containers in the sandbox since
	//networkNamespace has been ignored by
	//kata-agent in sandbox.

	for _, n := range ociSpec.Linux.Namespaces {
		if n.Type != specs.NetworkNamespace {
			continue
		}

		if n.Path == "" {
			n.Path = netns
		}
	}

	katautils.HandleFactory(ctx, vci, runtimeConfig)

	disableOutput := noNeedForOutput(detach, ociSpec.Process.Terminal)

	switch containerType {
	case vc.PodSandbox:
		if s.sandbox != nil {
			return nil, fmt.Errorf("cannot create another sandbox in sandbox: %s", s.sandbox.ID())
		}

		sandbox, _, err := katautils.CreateSandbox(ctx, vci, ociSpec, *runtimeConfig, r.ID, bundlePath, "", disableOutput, false, true)
		if err != nil {
			return nil, err
		}
		s.sandbox = sandbox

	case vc.PodContainer:
		if s.sandbox == nil {
			return nil, fmt.Errorf("BUG: Cannot start the container, since the sandbox hasn't been created")
		}

		_, err = katautils.CreateContainer(ctx, vci, s.sandbox, ociSpec, r.ID, bundlePath, "", disableOutput, true)
		if err != nil {
			return nil, err
		}
	}

	container, err := newContainer(s, r, containerType, &ociSpec)
	if err != nil {
		return nil, err
	}

	return container, nil
}
