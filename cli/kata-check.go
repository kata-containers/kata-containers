// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Note: To add a new architecture, implement all identifiers beginning "arch".

package main

/*
#include <linux/kvm.h>

const int ioctl_KVM_CREATE_VM = KVM_CREATE_VM;
*/
import "C"

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"syscall"

	"github.com/kata-containers/runtime/pkg/katautils"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

type kernelModule struct {
	// description
	desc string

	// maps parameter names to values
	parameters map[string]string
}

type vmContainerCapableDetails struct {
	cpuInfoFile           string
	requiredCPUFlags      map[string]string
	requiredCPUAttribs    map[string]string
	requiredKernelModules map[string]kernelModule
}

const (
	moduleParamDir        = "parameters"
	successMessageCapable = "System is capable of running " + project
	successMessageCreate  = "System can currently create " + project
	failMessage           = "System is not capable of running " + project
	kernelPropertyCorrect = "Kernel property value correct"

	// these refer to fields in the procCPUINFO file
	genericCPUFlagsTag    = "flags"
	genericCPUVendorField = "vendor_id"
	genericCPUModelField  = "model name"
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
	// First, check to see if the module is already loaded
	path := filepath.Join(sysModuleDir, module)
	if katautils.FileExists(path) {
		return true
	}

	// Now, check if the module is unloaded, but available.
	// And modprobe it if so.
	cmd := exec.Command(modProbeCmd, module)
	err := cmd.Run()
	return err == nil
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
//          parameter.
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
			kataLog.WithFields(fields).Error("kernel property not found")
			count++
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
	Name:  checkCmd,
	Usage: "tests if system can run " + project,
	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		span, _ := katautils.Trace(ctx, "kata-check")
		defer span.Finish()

		err = setCPUtype()
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

		kataLog.Info(successMessageCapable)

		if os.Geteuid() == 0 {
			err = archHostCanCreateVMContainer()
			if err != nil {
				return err
			}

			kataLog.Info(successMessageCreate)
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
	flags := syscall.O_RDWR | syscall.O_CLOEXEC

	f, err := syscall.Open(kvmDevice, flags, 0)
	if err != nil {
		return err
	}
	defer syscall.Close(f)

	fieldLogger := kataLog.WithField("check-type", "full")

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
