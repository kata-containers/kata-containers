// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"bytes"
	"fmt"
	"strconv"
	"strings"
	"time"

	"github.com/kata-containers/tests"
)

const (
	// Docker command
	Docker = "docker"

	// Image used to run containers
	Image = "busybox"

	// AlpineImage is the alpine image
	AlpineImage = "alpine"

	// PostgresImage is the postgres image
	PostgresImage = "postgres"

	// DebianImage is the debian image
	DebianImage = "debian"

	// FedoraImage is the fedora image
	FedoraImage = "fedora"
)

func runDockerCommandWithTimeout(timeout time.Duration, command string, args ...string) (string, string, int) {
	return runDockerCommandWithTimeoutAndPipe(nil, timeout, command, args...)
}

func runDockerCommandWithTimeoutAndPipe(stdin *bytes.Buffer, timeout time.Duration, command string, args ...string) (string, string, int) {
	a := []string{command}
	a = append(a, args...)

	cmd := tests.NewCommand(Docker, a...)
	cmd.Timeout = timeout

	return cmd.RunWithPipe(stdin)
}

func runDockerCommand(command string, args ...string) (string, string, int) {
	return runDockerCommandWithTimeout(time.Duration(tests.Timeout), command, args...)
}

func runDockerCommandWithPipe(stdin *bytes.Buffer, command string, args ...string) (string, string, int) {
	return runDockerCommandWithTimeoutAndPipe(stdin, time.Duration(tests.Timeout), command, args...)
}

// LogsDockerContainer returns the container logs
func LogsDockerContainer(name string) (string, error) {
	args := []string{name}

	stdout, _, exitCode := runDockerCommand("logs", args...)

	if exitCode != 0 {
		return "", fmt.Errorf("failed to run docker logs command")
	}

	return strings.TrimSpace(stdout), nil
}

// StatusDockerContainer returns the container status
func StatusDockerContainer(name string) string {
	args := []string{"-a", "-f", "name=" + name, "--format", "{{.Status}}"}

	stdout, _, exitCode := runDockerCommand("ps", args...)

	if exitCode != 0 || stdout == "" {
		return ""
	}

	state := strings.Split(stdout, " ")
	return state[0]
}

// hasExitedDockerContainer checks if the container has exited.
func hasExitedDockerContainer(name string) (bool, error) {
	args := []string{"--format={{.State.Status}}", name}

	stdout, _, exitCode := runDockerCommand("inspect", args...)

	if exitCode != 0 || stdout == "" {
		return false, fmt.Errorf("failed to run docker inspect command")
	}

	status := strings.TrimSpace(stdout)

	if status == "exited" {
		return true, nil
	}

	return false, nil
}

// ExitCodeDockerContainer returns the container exit code
func ExitCodeDockerContainer(name string, waitForExit bool) (int, error) {
	// It makes no sense to try to retrieve the exit code of the container
	// if it is still running. That's why this infinite loop takes care of
	// waiting for the status to become "exited" before to ask for the exit
	// code.
	// However, we might want to bypass this check on purpose, that's why
	// we check waitForExit boolean.
	if waitForExit {
		errCh := make(chan error)
		exitCh := make(chan bool)

		go func() {
			for {
				exited, err := hasExitedDockerContainer(name)
				if err != nil {
					errCh <- err
				}

				if exited {
					break
				}

				time.Sleep(time.Second)
			}

			close(exitCh)
		}()

		select {
		case <-exitCh:
			break
		case err := <-errCh:
			return -1, err
		case <-time.After(time.Duration(tests.Timeout) * time.Second):
			return -1, fmt.Errorf("Timeout reached after %ds", tests.Timeout)
		}
	}

	args := []string{"--format={{.State.ExitCode}}", name}

	stdout, _, exitCode := runDockerCommand("inspect", args...)

	if exitCode != 0 || stdout == "" {
		return -1, fmt.Errorf("failed to run docker inspect command")
	}

	return strconv.Atoi(strings.TrimSpace(stdout))
}

// WaitForRunningDockerContainer verifies if a docker container
// is running for a certain period of time
// returns an error if the timeout is reached.
func WaitForRunningDockerContainer(name string, running bool) error {
	ch := make(chan bool)
	go func() {
		if IsRunningDockerContainer(name) == running {
			close(ch)
			return
		}

		time.Sleep(time.Second)
	}()

	select {
	case <-ch:
	case <-time.After(time.Duration(tests.Timeout) * time.Second):
		return fmt.Errorf("Timeout reached after %ds", tests.Timeout)
	}

	return nil
}

// IsRunningDockerContainer inspects a container
// returns true if is running
func IsRunningDockerContainer(name string) bool {
	stdout, _, exitCode := runDockerCommand("inspect", "--format={{.State.Running}}", name)

	if exitCode != 0 {
		return false
	}

	output := strings.TrimSpace(stdout)
	tests.LogIfFail("container running: " + output)
	if output == "false" {
		return false
	}

	return true
}

// ExistDockerContainer returns true if any of next cases is true:
// - 'docker ps -a' command shows the container
// - the VM is running (qemu)
// else false is returned
func ExistDockerContainer(name string) bool {
	state := StatusDockerContainer(name)
	if state != "" {
		return true
	}

	return tests.IsVMRunning(name)
}

// RemoveDockerContainer removes a container using docker rm -f
func RemoveDockerContainer(name string) bool {
	_, _, exitCode := dockerRm("-f", name)
	if exitCode != 0 {
		return false
	}

	return true
}

// StopDockerContainer stops a container
func StopDockerContainer(name string) bool {
	_, _, exitCode := dockerStop(name)
	if exitCode != 0 {
		return false
	}

	return true
}

// KillDockerContainer kills a container
func KillDockerContainer(name string) bool {
	_, _, exitCode := dockerKill(name)
	if exitCode != 0 {
		return false
	}

	return true
}

// dockerRm removes a container
func dockerRm(args ...string) (string, string, int) {
	return runDockerCommand("rm", args...)
}

// dockerStop stops a container
// returns true on success else false
func dockerStop(args ...string) (string, string, int) {
	// docker stop takes ~15 seconds
	return runDockerCommand("stop", args...)
}

// dockerPull downloads the specific image
func dockerPull(args ...string) (string, string, int) {
	// 10 minutes should be enough to download a image
	return runDockerCommandWithTimeout(600, "pull", args...)
}

// dockerRun runs a container
func dockerRun(args ...string) (string, string, int) {
	if tests.Runtime != "" {
		args = append(args, []string{"", ""}...)
		copy(args[2:], args[:])
		args[0] = "--runtime"
		args[1] = tests.Runtime
	}

	return runDockerCommand("run", args...)
}

// Runs a container with stdin
func dockerRunWithPipe(stdin *bytes.Buffer, args ...string) (string, string, int) {
	if tests.Runtime != "" {
		args = append(args, []string{"", ""}...)
		copy(args[2:], args[:])
		args[0] = "--runtime"
		args[1] = tests.Runtime
	}

	return runDockerCommandWithPipe(stdin, "run", args...)
}

// dockerKill kills a container
func dockerKill(args ...string) (string, string, int) {
	return runDockerCommand("kill", args...)
}

// dockerVolume manages volumes
func dockerVolume(args ...string) (string, string, int) {
	return runDockerCommand("volume", args...)
}

// dockerAttach attach to a running container
func dockerAttach(args ...string) (string, string, int) {
	return runDockerCommand("attach", args...)
}

// dockerCommit creates a new image from a container's changes
func dockerCommit(args ...string) (string, string, int) {
	return runDockerCommand("commit", args...)
}

// dockerImages list images
func dockerImages(args ...string) (string, string, int) {
	return runDockerCommand("images", args...)
}

// dockerImport imports the contents from a tarball to create a filesystem image
func dockerImport(args ...string) (string, string, int) {
	return runDockerCommand("import", args...)
}

// dockerRmi removes one or more images
func dockerRmi(args ...string) (string, string, int) {
	// docker takes more than 5 seconds to remove an image, it depends
	// of the image size and this operation does not involve to the
	// runtime
	return runDockerCommand("rmi", args...)
}

// dockerCp copies files/folders between a container and the local filesystem
func dockerCp(args ...string) (string, string, int) {
	return runDockerCommand("cp", args...)
}

// dockerExec runs a command in a running container
func dockerExec(args ...string) (string, string, int) {
	return runDockerCommand("exec", args...)
}

// dockerPs list containers
func dockerPs(args ...string) (string, string, int) {
	return runDockerCommand("ps", args...)
}

// dockerSearch searches docker hub images
func dockerSearch(args ...string) (string, string, int) {
	return runDockerCommand("search", args...)
}

// dockerCreate creates a new container
func dockerCreate(args ...string) (string, string, int) {
	return runDockerCommand("create", args...)
}

// dockerDiff inspect changes to files or directories on a container’s filesystem
func dockerDiff(args ...string) (string, string, int) {
	return runDockerCommand("diff", args...)
}

// dockerBuild builds an image from a Dockerfile
func dockerBuild(args ...string) (string, string, int) {
	return runDockerCommand("build", args...)
}

// dockerNetwork manages networks
func dockerNetwork(args ...string) (string, string, int) {
	return runDockerCommand("network", args...)
}

// dockerExport will export a container’s filesystem as a tar archive
func dockerExport(args ...string) (string, string, int) {
	return runDockerCommand("export", args...)
}

// dockerInfo displays system-wide information
func dockerInfo() (string, string, int) {
	return runDockerCommand("info")
}

// dockerInspect returns low-level information on Docker objects
func dockerInspect(args ...string) (string, string, int) {
	return runDockerCommand("inspect", args...)
}

// dockerLoad loads a tarred repository
func dockerLoad(args ...string) (string, string, int) {
	return runDockerCommand("load", args...)
}

// dockerPort starts one or more stopped containers
func dockerPort(args ...string) (string, string, int) {
	return runDockerCommand("port", args...)
}

// dockerRestart starts one or more stopped containers
func dockerRestart(args ...string) (string, string, int) {
	return runDockerCommand("restart", args...)
}

// dockerSwarm manages swarm
func dockerSwarm(args ...string) (string, string, int) {
	return runDockerCommand("swarm", args...)
}

// dockerSave saves one or more images
func dockerSave(args ...string) (string, string, int) {
	return runDockerCommand("save", args...)
}

// dockerService manages services
func dockerService(args ...string) (string, string, int) {
	return runDockerCommand("service", args...)
}

// dockerStart starts one or more stopped containers
func dockerStart(args ...string) (string, string, int) {
	return runDockerCommand("start", args...)
}

// dockerPause pauses all processes within one or more containers
func dockerPause(args ...string) (string, string, int) {
	return runDockerCommand("pause", args...)
}

// dockerUnpause unpauses all processes within one or more containers
func dockerUnpause(args ...string) (string, string, int) {
	return runDockerCommand("unpause", args...)
}

// dockerTop displays the running processes of a container
func dockerTop(args ...string) (string, string, int) {
	return runDockerCommand("top", args...)
}
