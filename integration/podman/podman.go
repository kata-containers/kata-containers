// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package podman

import (
	"bytes"
	"fmt"
	"io/ioutil"
	"log"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/kata-containers/tests"
	ginkgoconf "github.com/onsi/ginkgo/config"
)

const (
	// Podman command
	Podman = "podman"

	// Image used to run containers
	Image = "docker.io/library/busybox"
)

// cidDirectory is the directory where container ID files are created.
var cidDirectory string

var images []string

func init() {
	var err error
	cidDirectory, err = ioutil.TempDir("", "cid")
	if err != nil {
		log.Fatalf("Could not create cid directory: %v\n", err)
	}

	images = []string{
		Image,
	}
}

func cidFilePath(containerName string) string {
	return filepath.Join(cidDirectory, containerName)
}

func runPodmanCommandWithTimeout(timeout time.Duration, command string, args ...string) (string, string, int) {
	return runPodmanCommandWithTimeoutAndPipe(nil, timeout, command, args...)
}

func runPodmanCommandWithTimeoutAndPipe(stdin *bytes.Buffer, timeout time.Duration, command string, args ...string) (string, string, int) {
	a := []string{command}

	// --cidfile must be specified when the container is created (run/create)
	if command == "run" || command == "create" {
		for i := 0; i < len(args); i++ {
			// looks for container name
			if args[i] == "--name" && i+1 < len(args) {
				a = append(a, "--cidfile", cidFilePath(args[i+1]))
			}
		}
	}

	a = append(a, args...)

	cmd := tests.NewCommand(Podman, a...)
	cmd.Timeout = timeout

	return cmd.RunWithPipe(stdin)
}

func runPodmanCommand(command string, args ...string) (string, string, int) {
	return runPodmanCommandWithTimeout(time.Duration(tests.Timeout), command, args...)
}

// LogsPodmanContainer returns the container logs
func LogsPodmanContainer(name string) (string, error) {
	args := []string{name}

	stdout, _, exitCode := runPodmanCommand("logs", args...)

	if exitCode != 0 {
		return "", fmt.Errorf("failed to run podman logs command")
	}

	return strings.TrimSpace(stdout), nil
}

// StatusPodmanContainer returns the container status
func StatusPodmanContainer(name string) string {
	args := []string{"-a", "-f", "name=" + name, "--format", "{{.Status}}"}

	stdout, _, exitCode := runPodmanCommand("ps", args...)

	if exitCode != 0 || stdout == "" {
		return ""
	}

	state := strings.Split(stdout, " ")
	return state[0]
}

// hasExitedPodmanContainer checks if the container has exited.
func hasExitedPodmanContainer(name string) (bool, error) {
	args := []string{"--format={{.State.Status}}", name}

	stdout, _, exitCode := runPodmanCommand("inspect", args...)

	if exitCode != 0 || stdout == "" {
		return false, fmt.Errorf("failed to run podman inspect command")
	}

	status := strings.TrimSpace(stdout)

	if status == "exited" {
		return true, nil
	}

	return false, nil
}

// ExitCodePodmanContainer returns the container exit code
func ExitCodePodmanContainer(name string, waitForExit bool) (int, error) {
	if waitForExit {
		errCh := make(chan error)
		exitCh := make(chan bool)

		go func() {
			for {
				exited, err := hasExitedPodmanContainer(name)
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

	stdout, _, exitCode := runPodmanCommand("inspect", args...)

	if exitCode != 0 || stdout == "" {
		return -1, fmt.Errorf("failed to run podman inspect command")
	}

	return strconv.Atoi(strings.TrimSpace(stdout))
}

// WaitForRunningPodmanContainer verifies if a podman container
// is running for a certain period of time
// returns an error if the timeout is reached.
func WaitForRunningPodmanContainer(name string, running bool) error {
	ch := make(chan bool)
	go func() {
		if IsRunningPodmanContainer(name) == running {
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

// IsRunningPodmanContainer inspects a container
// returns true if is running
func IsRunningPodmanContainer(name string) bool {
	stdout, _, exitCode := runPodmanCommand("inspect", "--format={{.State.Running}}", name)

	if exitCode != 0 {
		return false
	}

	output := strings.TrimSpace(stdout)
	tests.LogIfFail("container running: " + output)
	return !(output == "false")
}

// ExistPodmanContainer returns true if any of next cases is true:
// - 'podman ps -a' command shows the container
func ExistPodmanContainer(name string) bool {
	if name == "" {
		tests.LogIfFail("Container name is empty")
		return false
	}

	state := StatusPodmanContainer(name)
	if state != "" {
		return true
	}

	// If we reach this point means that the container doesn't exist in podman,
	// but we have to check that the components (qemu, shim, proxy) are not running.
	// Read container ID from file created by run/create
	path := cidFilePath(name)
	defer os.Remove(path)
	content, err := ioutil.ReadFile(path)
	if err != nil {
		tests.LogIfFail("Could not read container ID file: %v\n", err)
		return false
	}

	// Use container ID to check if kata components are still running.
	cid := string(content)
	exitCh := make(chan bool)
	go func() {
		for {
			if !tests.HypervisorRunning(cid) &&
				!tests.ProxyRunning(cid) &&
				!tests.ShimRunning(cid) {
				close(exitCh)
				return
			}
			time.Sleep(time.Second)
		}
	}()

	select {
	case <-exitCh:
		return false
	case <-time.After(time.Duration(tests.Timeout) * time.Second):
		tests.LogIfFail("Timeout reached after %ds", tests.Timeout)
		return true
	}
}

// RemovePodmanContainer removes a container using podman rm -f
func RemovePodmanContainer(name string) bool {
	_, _, exitCode := podmanRm("-f", name)
	return (exitCode == 0)
}

// StopPodmanContainer stops a container
func StopPodmanContainer(name string) bool {
	_, _, exitCode := podmanStop(name)
	return (exitCode == 0)
}

// KillPodmanContainer kills a container
func KillPodmanContainer(name string) bool {
	_, _, exitCode := podmanKill(name)
	return (exitCode == 0)
}

func randomPodmanName() string {
	return tests.RandID(29) + fmt.Sprint(ginkgoconf.GinkgoConfig.ParallelNode)
}

// podmanStop stops a container
// returns true on success else false
func podmanStop(args ...string) (string, string, int) {
	return runPodmanCommand("stop", args...)
}

// podmanPull downloads the specific image
func podmanPull(args ...string) (string, string, int) {
	// 10 minutes should be enough to download a image
	return runPodmanCommandWithTimeout(600, "pull", args...)
}

// podmanRun runs a container
func podmanRun(args ...string) (string, string, int) {
	if tests.Runtime != "" {
		args = append(args, []string{"", ""}...)
		copy(args[2:], args[:])
		args[0] = "--runtime"
		args[1] = tests.Runtime
	}

	return runPodmanCommand("run", args...)
}

// podmanKill kills a container
func podmanKill(args ...string) (string, string, int) {
	return runPodmanCommand("kill", args...)
}

// podmanRm removes a container
func podmanRm(args ...string) (string, string, int) {
	return runPodmanCommand("rm", args...)
}
