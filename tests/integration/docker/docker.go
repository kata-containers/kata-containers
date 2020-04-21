// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"bytes"
	"fmt"
	"io/ioutil"
	"log"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"time"

	"gopkg.in/yaml.v2"

	"github.com/kata-containers/tests"
	ginkgoconf "github.com/onsi/ginkgo/config"
)

const (
	// Docker command
	Docker = "docker"

	// Image used to run containers
	Image = "busybox"

	// DebianImage is the debian image
	DebianImage = "debian"

	// FedoraImage is the fedora image
	FedoraImage = "fedora"

	// Fedora30Image is the fedora 30 image
	// This Fedora version is used mainly because of https://github.com/kata-containers/tests/issues/2358
	Fedora30Image = "fedora:30"

	// StressImage is the vish/stress image
	StressImage = "vish/stress"

	// StressDockerFile is the dockerfile to build vish/stress image
	StressDockerFile = "src/github.com/kata-containers/tests/stress/."

	// VersionsPath is the path for the versions.yaml
	VersionsPath = "src/github.com/kata-containers/tests/versions.yaml"
)

// cidDirectory is the directory where container ID files are created.
var cidDirectory string

// AlpineImage is the alpine image
var AlpineImage string

var images []string

// versionDockerImage is the definition in the yaml for the Alpine image
type versionDockerImage struct {
	Description string `yaml:"description"`
	URL         string `yaml:"url"`
	Version     string `yaml:"version"`
}

// versionDockerImages is the complete information for docker images in the versions yaml
type versionDockerImages struct {
	Description string `yaml:"description"`
	Alpine      versionDockerImage
}

// Versions will be used to parse the versions yaml
type Versions struct {
	Docker versionDockerImages `yaml:"docker_images"`
}

func init() {
	var err error
	cidDirectory, err = ioutil.TempDir("", "cid")
	if err != nil {
		log.Fatalf("Could not create cid directory: %v\n", err)
	}

	// Check versions.yaml
	gopath := os.Getenv("GOPATH")
	entirePath := filepath.Join(gopath, VersionsPath)

	// Read versions.yaml
	data, err := ioutil.ReadFile(entirePath)
	if err != nil {
		log.Fatalf("Could not read versions.yaml")
	}

	// Parse versions.yaml
	var versions Versions
	err = yaml.Unmarshal(data, &versions)
	if err != nil {
		log.Fatalf("Could not get alpine version")
	}

	// Define Alpine image with its proper version
	AlpineImage = "alpine:" + versions.Docker.Alpine.Version

	images = []string{
		Image,
		AlpineImage,
		DebianImage,
		FedoraImage,
		Fedora30Image,
		CentosImage,
		StressImage,
	}
}

func cidFilePath(containerName string) string {
	return filepath.Join(cidDirectory, containerName)
}

func runDockerCommandWithTimeout(timeout time.Duration, command string, args ...string) (string, string, int) {
	return runDockerCommandWithTimeoutAndPipe(nil, timeout, command, args...)
}

func runDockerCommandWithTimeoutAndPipe(stdin *bytes.Buffer, timeout time.Duration, command string, args ...string) (string, string, int) {
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
	return !(output == "false")
}

// ExistDockerContainer returns true if any of next cases is true:
// - 'docker ps -a' command shows the container
// - the VM is running (qemu)
// - the proxy is running
// - the shim is running
// else false is returned
func ExistDockerContainer(name string) bool {
	if name == "" {
		tests.LogIfFail("Container name is empty")
		return false
	}

	state := StatusDockerContainer(name)
	if state != "" {
		return true
	}

	// If we reach this point means that the container doesn't exist in docker,
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

// RemoveDockerContainer removes a container using docker rm -f
func RemoveDockerContainer(name string) bool {
	_, _, exitCode := dockerRm("-f", name)
	return (exitCode == 0)
}

// StopDockerContainer stops a container
func StopDockerContainer(name string) bool {
	_, _, exitCode := dockerStop(name)
	return (exitCode == 0)
}

// KillDockerContainer kills a container
func KillDockerContainer(name string) bool {
	_, _, exitCode := dockerKill(name)
	return (exitCode == 0)
}

func randomDockerName() string {
	return tests.RandID(29) + fmt.Sprint(ginkgoconf.GinkgoConfig.ParallelNode)
}

// returns a random and valid repository name
func randomDockerRepoName() string {
	return strings.ToLower(tests.RandID(14)) + fmt.Sprint(ginkgoconf.GinkgoConfig.ParallelNode)
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
	// 10 minutes should be enough to build a image
	return runDockerCommandWithTimeout(600, "build", args...)
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

// dockerSave saves one or more images
func dockerSave(args ...string) (string, string, int) {
	return runDockerCommand("save", args...)
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

// dockerUpdate updates configuration of one or more containers
func dockerUpdate(args ...string) (string, string, int) {
	return runDockerCommand("update", args...)
}

// createLoopDevice creates a new disk file using 'dd' command, returns the path to disk file and
// its loop device representation
func createLoopDevice() (string, string, error) {
	f, err := ioutil.TempFile("", "dd")
	if err != nil {
		return "", "", err
	}
	defer f.Close()

	// create disk file
	ddArgs := []string{"if=/dev/zero", fmt.Sprintf("of=%s", f.Name()), "count=1", "bs=50M"}
	ddCmd := tests.NewCommand("dd", ddArgs...)
	if _, stderr, exitCode := ddCmd.Run(); exitCode != 0 {
		return "", "", fmt.Errorf("%s", stderr)
	}

	// partitioning disk file
	fdiskArgs := []string{"-c", fmt.Sprintf(`printf "g\nn\n\n\n\nw\n" | fdisk %s`, f.Name())}
	fdiskCmd := tests.NewCommand("bash", fdiskArgs...)
	if _, stderr, exitCode := fdiskCmd.Run(); exitCode != 0 {
		return "", "", fmt.Errorf("%s", stderr)
	}

	// create loop device
	losetupCmd := tests.NewCommand("losetup", "-fP", f.Name())
	if _, stderr, exitCode := losetupCmd.Run(); exitCode != 0 {
		return "", "", fmt.Errorf("%s", stderr)
	}

	// get loop device path
	getLoopPath := tests.NewCommand("losetup", "-j", f.Name())
	stdout, stderr, exitCode := getLoopPath.Run()
	if exitCode != 0 {
		return "", "", fmt.Errorf("exitCode: %d, stdout: %s, stderr: %s ", exitCode, stdout, stderr)
	}
	re := regexp.MustCompile("/dev/loop[0-9]+")
	loopPath := re.FindStringSubmatch(stdout)
	if len(loopPath) == 0 {
		return "", "", fmt.Errorf("Unable to get loop device path, stdout: %s, stderr: %s", stdout, stderr)
	}
	return f.Name(), loopPath[0], nil
}

// deleteLoopDevice removes loopdevices
func deleteLoopDevice(loopFile string) error {
	partxCmd := tests.NewCommand("losetup", "-d", loopFile)
	_, stderr, exitCode := partxCmd.Run()
	if exitCode != 0 {
		return fmt.Errorf("%s", stderr)
	}

	return nil
}
