// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Note: To add a new architecture, implement all identifiers beginning "arch".

package main

/*
#include <linux/kvm.h>

const int ioctl_KVM_CREATE_VM = KVM_CREATE_VM;
const int ioctl_KVM_CHECK_EXTENSION = KVM_CHECK_EXTENSION;
*/
import "C"

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"syscall"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

type kernelModule struct {
	// maps parameter names to values
	parameters map[string]string

	// description
	desc string

	// if it is definitely required
	required bool
}

// nolint: structcheck, unused, deadcode
type kvmExtension struct {
	// description
	desc string

	// extension identifier
	id int
}

type vmContainerCapableDetails struct {
	requiredCPUFlags      map[string]string
	requiredCPUAttribs    map[string]string
	requiredKernelModules map[string]kernelModule
	cpuInfoFile           string
}

const (
	moduleParamDir        = "parameters"
	successMessageCapable = "System is capable of running " + katautils.PROJECT
	successMessageCreate  = "System can currently create " + katautils.PROJECT
	failMessage           = "System is not capable of running " + katautils.PROJECT
	kernelPropertyCorrect = "Kernel property value correct"

	// these refer to fields in the procCPUINFO file
	genericCPUFlagsTag    = "flags"      // nolint: varcheck, unused, deadcode
	genericCPUVendorField = "vendor_id"  // nolint: varcheck, unused, deadcode
	genericCPUModelField  = "model name" // nolint: varcheck, unused, deadcode

	// If set, do not perform any network checks
	noNetworkEnvVar = "KATA_CHECK_NO_NETWORK"
)

// variables rather than consts to allow tests to modify them
var (
	procCPUInfo  = "/proc/cpuinfo"
	sysModuleDir = "/sys/module"
	modProbeCmd  = "modprobe"
)

// variables rather than consts to allow tests to modify them
var (
	kvmDevice = "/dev/kvm"
)

// getCPUInfo returns details of the first CPU read from the specified cpuinfo file
func getCPUInfo(cpuInfoFile string) (string, error) {
	text, err := katautils.GetFileContents(cpuInfoFile)
	if err != nil {
		return "", err
	}

	cpus := strings.SplitAfter(text, "\n\n")

	trimmed := strings.TrimSpace(cpus[0])
	if trimmed == "" {
		return "", fmt.Errorf("Cannot determine CPU details")
	}

	return trimmed, nil
}

// findAnchoredString searches haystack for needle and returns true if found
func findAnchoredString(haystack, needle string) bool {
	if haystack == "" || needle == "" {
		return false
	}

	// Ensure the search string is anchored
	pattern := regexp.MustCompile(`\b` + needle + `\b`)

	return pattern.MatchString(haystack)
}

// getCPUFlags returns the CPU flags from the cpuinfo file specified
func getCPUFlags(cpuinfo string) string {
	for _, line := range strings.Split(cpuinfo, "\n") {
		if strings.HasPrefix(line, cpuFlagsTag) {
			fields := strings.Split(line, ":")
			if len(fields) == 2 {
				return strings.TrimSpace(fields[1])
			}
		}
	}

	return ""
}

// haveKernelModule returns true if the specified module exists
// (either loaded or available to be loaded)
func haveKernelModule(module string) bool {
	kmodLog := kataLog.WithField("module", module)

	// First, check to see if the module is already loaded
	path := filepath.Join(sysModuleDir, module)
	if katautils.FileExists(path) {
		return true
	}

	// Only root can load modules
	if os.Getuid() != 0 {
		kmodLog.Error("Module is not loaded and it can not be inserted. Please consider running with sudo or as root")
		return false
	}

	// Now, check if the module is unloaded, but available.
	// And modprobe it if so.
	cmd := exec.Command(modProbeCmd, module)
	if output, err := cmd.CombinedOutput(); err != nil {
		kmodLog.WithError(err).WithField("output", string(output)).Warnf("modprobe insert module failed")
		return false
	}
	return true
}

// checkCPU checks all required CPU attributes modules and returns a count of
// the number of CPU attribute errors (all of which are logged by this
// function). The specified tag is simply used for logging purposes.
func checkCPU(tag, cpuinfo string, attribs map[string]string) (count uint32) {
	if cpuinfo == "" {
		return 0
	}

	for attrib, desc := range attribs {
		fields := logrus.Fields{
			"type":        tag,
			"name":        attrib,
			"description": desc,
		}

		found := findAnchoredString(cpuinfo, attrib)
		if !found {
			kataLog.WithFields(fields).Errorf("CPU property not found")
			count++
			continue

		}

		kataLog.WithFields(fields).Infof("CPU property found")
	}

	return count
}

func checkCPUFlags(cpuflags string, required map[string]string) uint32 {
	return checkCPU("flag", cpuflags, required)
}

func checkCPUAttribs(cpuinfo string, attribs map[string]string) uint32 {
	return checkCPU("attribute", cpuinfo, attribs)
}

// kernelParamHandler represents a function that allows kernel module
// parameter errors to be ignored for special scenarios.
//
// The function is passed the following parameters:
//
// onVMM  - `true` if the host is running under a VMM environment
// fields - A set of fields showing the expected and actual module parameter values.
// msg    - The message that would be logged showing the incorrect kernel module
//
//	parameter.
//
// The function must return `true` if the kernel module parameter error should
// be ignored, or `false` if it is a real error.
//
// Note: it is up to the function to add an appropriate log call if the error
// should be ignored.
type kernelParamHandler func(onVMM bool, fields logrus.Fields, msg string) bool

// checkKernelModules checks all required kernel modules modules and returns a count of
// the number of module errors (all of which are logged by this
// function). Only fatal errors result in an error return.
func checkKernelModules(modules map[string]kernelModule, handler kernelParamHandler) (count uint32, err error) {
	onVMM, err := vc.RunningOnVMM(procCPUInfo)
	if err != nil {
		return 0, err
	}

	for module, details := range modules {
		fields := logrus.Fields{
			"type":        "module",
			"name":        module,
			"description": details.desc,
		}

		if !haveKernelModule(module) {
			kataLog.WithFields(fields).Errorf("kernel property %s not found", module)
			if details.required {
				count++
			}
			continue
		}

		kataLog.WithFields(fields).Infof("kernel property found")

		for param, expected := range details.parameters {
			path := filepath.Join(sysModuleDir, module, moduleParamDir, param)
			value, err := katautils.GetFileContents(path)
			if err != nil {
				return 0, err
			}

			value = strings.TrimRight(value, "\n\r")

			fields["parameter"] = param
			fields["value"] = value

			if value != expected {
				fields["expected"] = expected

				msg := "kernel module parameter has unexpected value"

				if handler != nil {
					ignoreError := handler(onVMM, fields, msg)
					if ignoreError {
						continue
					}
				}

				kataLog.WithFields(fields).Error(msg)
				count++
			}

			kataLog.WithFields(fields).Info(kernelPropertyCorrect)
		}
	}

	return count, nil
}

// genericHostIsVMContainerCapable checks to see if the host is theoretically capable
// of creating a VM container.
// nolint: unused,deadcode
func genericHostIsVMContainerCapable(details vmContainerCapableDetails) error {
	cpuinfo, err := getCPUInfo(details.cpuInfoFile)
	if err != nil {
		return err
	}

	cpuFlags := getCPUFlags(cpuinfo)
	if cpuFlags == "" {
		return fmt.Errorf("Cannot find CPU flags")
	}

	// Keep a track of the error count, but don't error until all tests
	// have been performed!
	errorCount := uint32(0)

	count := checkCPUAttribs(cpuinfo, details.requiredCPUAttribs)
	errorCount += count

	count = checkCPUFlags(cpuFlags, details.requiredCPUFlags)
	errorCount += count

	count, err = checkKernelModules(details.requiredKernelModules, archKernelParamHandler)
	if err != nil {
		return err
	}

	errorCount += count

	if errorCount == 0 {
		return nil
	}

	return fmt.Errorf("ERROR: %s", failMessage)
}

var kataCheckCLICommand = cli.Command{
	Name:    "check",
	Aliases: []string{"kata-check"},
	Usage:   "tests if system can run " + katautils.PROJECT,
	Flags: []cli.Flag{
		cli.BoolFlag{
			Name:  "check-version-only",
			Usage: "Only compare the current and latest available versions (requires network, non-root only)",
		},
		cli.BoolFlag{
			Name:  "include-all-releases",
			Usage: "Don't filter out pre-release release versions",
		},
		cli.BoolFlag{
			Name:  "no-network-checks, n",
			Usage: "Do not run any checks using the network",
		},
		cli.BoolFlag{
			Name:  "only-list-releases",
			Usage: "Only list newer available releases (non-root only)",
		},
		cli.BoolFlag{
			Name:  "strict, s",
			Usage: "perform strict checking",
		},
		cli.BoolFlag{
			Name:  "verbose, v",
			Usage: "display the list of checks performed",
		},
	},
	Description: fmt.Sprintf(`tests if system can run %s and version is current.

ENVIRONMENT VARIABLES:

- %s: If set to any value, act as if "--no-network-checks" was specified.

EXAMPLES:

- Perform basic checks:

  $ %s check

- Local basic checks only:

  $ %s check --no-network-checks

- Perform further checks:

  $ sudo %s check

- Just check if a newer version is available:

  $ %s check --check-version-only

- List available releases (shows output in format "version;release-date;url"):

  $ %s check --only-list-releases

- List all available releases (includes pre-release versions):

  $ %s check --only-list-releases --include-all-releases
`,
		katautils.PROJECT,
		noNetworkEnvVar,
		katautils.NAME,
		katautils.NAME,
		katautils.NAME,
		katautils.NAME,
		katautils.NAME,
		katautils.NAME,
	),

	Action: func(context *cli.Context) error {
		verbose := context.Bool("verbose")
		if verbose {
			kataLog.Logger.SetLevel(logrus.InfoLevel)
		}

		if !context.Bool("no-network-checks") && os.Getenv(noNetworkEnvVar) == "" {
			cmd := RelCmdCheck

			if context.Bool("only-list-releases") {
				cmd = RelCmdList
			}

			if os.Geteuid() == 0 {
				kataLog.Warn("Not running network checks as super user")
			} else {
				err := HandleReleaseVersions(cmd, katautils.VERSION, context.Bool("include-all-releases"))
				if err != nil {
					return err
				}
			}
		}

		if context.Bool("check-version-only") || context.Bool("only-list-releases") {
			return nil
		}

		runtimeConfig, ok := context.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return errors.New("check: cannot determine runtime config")
		}

		err := setCPUtype(runtimeConfig.HypervisorType)
		if err != nil {
			return err
		}

		details := vmContainerCapableDetails{
			cpuInfoFile:           procCPUInfo,
			requiredCPUFlags:      archRequiredCPUFlags,
			requiredCPUAttribs:    archRequiredCPUAttribs,
			requiredKernelModules: archRequiredKernelModules,
		}

		err = hostIsVMContainerCapable(details)
		if err != nil {
			return err
		}
		fmt.Println(successMessageCapable)

		if os.Geteuid() == 0 {
			err = archHostCanCreateVMContainer(runtimeConfig.HypervisorType)
			if err != nil {
				return err
			}

			fmt.Println(successMessageCreate)
		}

		return nil
	},
}

func genericArchKernelParamHandler(onVMM bool, fields logrus.Fields, msg string) bool {
	param, ok := fields["parameter"].(string)
	if !ok {
		return false
	}

	// This option is not required when
	// already running under a hypervisor.
	if param == "unrestricted_guest" && onVMM {
		kataLog.WithFields(fields).Warn(kernelPropertyCorrect)
		return true
	}

	if param == "nested" {
		kataLog.WithFields(fields).Warn(msg)
		return true
	}

	// don't ignore the error
	return false
}

// genericKvmIsUsable determines if it will be possible to create a full virtual machine
// by creating a minimal VM and then deleting it.
func genericKvmIsUsable() error {
	fieldLogger := kataLog.WithField("check-type", "full")

	f, err := syscall.Open(kvmDevice, syscall.O_RDWR|syscall.O_CLOEXEC, 0)
	if err != nil {
		fieldLogger.WithField("device", kvmDevice).Errorf("cannot open kvm device: %v", err)
		return err
	}
	defer syscall.Close(f)

	fieldLogger.WithField("device", kvmDevice).Info("device available")

	vm, _, errno := syscall.Syscall(syscall.SYS_IOCTL,
		uintptr(f),
		uintptr(C.ioctl_KVM_CREATE_VM),
		0)
	if errno != 0 {
		if errno == syscall.EBUSY {
			fieldLogger.WithField("reason", "another hypervisor running").Error("cannot create VM")
		}

		return errno
	}
	defer syscall.Close(int(vm))

	fieldLogger.WithField("feature", "create-vm").Info("feature available")

	return nil
}

// genericCheckKVMExtension allows to query about the specific kvm extensions
// nolint: unused, deadcode
func genericCheckKVMExtensions(extensions map[string]kvmExtension) (map[string]int, error) {
	results := make(map[string]int)

	flags := syscall.O_RDWR | syscall.O_CLOEXEC
	kvm, err := syscall.Open(kvmDevice, flags, 0)
	if err != nil {
		return results, err
	}
	defer syscall.Close(kvm)

	for name, extension := range extensions {
		fields := logrus.Fields{
			"type":        "kvm extension",
			"name":        name,
			"description": extension.desc,
			"id":          extension.id,
		}

		ret, _, errno := syscall.Syscall(syscall.SYS_IOCTL,
			uintptr(kvm),
			uintptr(C.ioctl_KVM_CHECK_EXTENSION),
			uintptr(extension.id))

		// Generally return value(ret) 0 means no and 1 means yes,
		// but some extensions may report additional information in the integer return value.
		if errno != 0 {
			kataLog.WithFields(fields).Error("is not supported")
			return results, errno
		}

		results[name] = int(ret)
		kataLog.WithFields(fields).Info("kvm extension is supported")
	}

	return results, nil
}
