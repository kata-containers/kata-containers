// +build cgo,linux
// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"fmt"
	"time"

	"github.com/kata-containers/runtime/pkg/katautils"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

var kataOverheadCLICommand = cli.Command{
	Name:  "kata-overhead",
	Usage: "provides kata overhead at sandbox level",
	ArgsUsage: `<sandbox-id> [sandbox-id...]

   <sandbox-id> is your name for the instance of the sandbox.`,

	Description: `The kata-overhead command shows the overhead of a running Kata sandbox. Overhead 
       is calculated as the sum of pod resource utilization as measured on host cgroup minus the total
       container usage measured inside the Kata guest for each container's cgroup.`,

	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		args := context.Args()
		if !args.Present() {
			return fmt.Errorf("Missing container ID, should at least provide one")
		}

		for _, cID := range []string(args) {
			if err := overhead(ctx, cID); err != nil {
				return err
			}
		}

		return nil
	},
}

func overhead(ctx context.Context, containerID string) error {
	span, _ := katautils.Trace(ctx, "overhead")
	defer span.Finish()

	kataLog = kataLog.WithField("container", containerID)
	setExternalLoggers(ctx, kataLog)
	span.SetTag("container", containerID)

	status, sandboxID, err := getExistingContainerInfo(ctx, containerID)
	if err != nil {
		return err
	}

	containerID = status.ID

	kataLog = kataLog.WithFields(logrus.Fields{
		"container": containerID,
		"sandbox":   sandboxID,
	})

	setExternalLoggers(ctx, kataLog)
	span.SetTag("container", containerID)
	span.SetTag("sandbox", sandboxID)

	if status.State.State == types.StateStopped {
		return fmt.Errorf("container with id %s is not running", status.ID)
	}

	initTime := time.Now().UnixNano()

	initialSandboxStats, initialContainerStats, err := vci.StatsSandbox(ctx, sandboxID)
	if err != nil {
		return err
	}

	hostInitCPU := initialSandboxStats.CgroupStats.CPUStats.CPUUsage.TotalUsage
	guestInitCPU := uint64(0)
	for _, cs := range initialContainerStats {
		guestInitCPU += cs.CgroupStats.CPUStats.CPUUsage.TotalUsage
	}

	// Wait for 1 second to calculate CPU usage
	time.Sleep(time.Second * 1)
	finishtTime := time.Now().UnixNano()

	finishSandboxStats, finishContainersStats, err := vci.StatsSandbox(ctx, sandboxID)
	if err != nil {
		return err
	}

	hostFinalCPU := finishSandboxStats.CgroupStats.CPUStats.CPUUsage.TotalUsage
	guestFinalCPU := uint64(0)
	for _, cs := range finishContainersStats {
		guestFinalCPU += cs.CgroupStats.CPUStats.CPUUsage.TotalUsage
	}

	var guestMemoryUsage uint64
	for _, cs := range finishContainersStats {
		guestMemoryUsage += cs.CgroupStats.MemoryStats.Usage.Usage
	}

	hostMemoryUsage := finishSandboxStats.CgroupStats.MemoryStats.Usage.Usage
	deltaTime := finishtTime - initTime

	cpuUsageGuest := float64(guestFinalCPU-guestInitCPU) / float64(deltaTime) * 100
	cpuUsageHost := float64(hostFinalCPU-hostInitCPU) / float64(deltaTime) * 100

	fmt.Printf("Sandbox overhead for container: %s\n", containerID)
	fmt.Printf("cpu_overhead=%f\n", cpuUsageHost-cpuUsageGuest)
	fmt.Printf("memory_overhead_bytes=%d\n\n", hostMemoryUsage-guestMemoryUsage)
	fmt.Printf(" --CPU details--\n")
	fmt.Printf("cpu_host=%f\n", cpuUsageHost)
	fmt.Printf("\tcpu_host_init=%d\n", hostInitCPU)
	fmt.Printf("\tcpu_host_final=%d\n", hostFinalCPU)
	fmt.Printf("cpu_guest=%f\n", cpuUsageGuest)
	fmt.Printf("\tcpu_guest_init=%d\n", guestInitCPU)
	fmt.Printf("\tcpu_guest_final=%d\n", guestFinalCPU)
	fmt.Printf("Number of available vCPUs=%d\n", finishSandboxStats.Cpus)
	fmt.Printf(" --Memory details--\n")
	fmt.Printf("memory_host_bytes=%d\n", hostMemoryUsage)
	fmt.Printf("memory_guest_bytes=%d\n\n", guestMemoryUsage)

	return nil
}
