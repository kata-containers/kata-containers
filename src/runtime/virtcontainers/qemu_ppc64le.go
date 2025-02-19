//go:build linux

// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/sirupsen/logrus"
)

type qemuPPC64le struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase
}

const defaultQemuPath = "/usr/bin/qemu-system-ppc64"

const defaultQemuMachineType = QemuPseries

const defaultQemuMachineOptions = "accel=kvm,usb=off,cap-ail-mode-3=off"

const qmpMigrationWaitTimeout = 5 * time.Second

const pefSysFirmwareDir = "/sys/firmware/ultravisor/"

const pefID = "pef0"

const tpmID = "tpm0"

const tpmHostPath = "/dev/tpmrm0"

var kernelParams = []Param{
	{"rcupdate.rcu_expedited", "1"},
	{"reboot", "k"},
	{"cryptomgr.notests", ""},
	{"net.ifnames", "0"},
}

var supportedQemuMachine = govmmQemu.Machine{
	Type:    QemuPseries,
	Options: defaultQemuMachineOptions,
}

// Logger returns a logrus logger appropriate for logging qemu messages
func (q *qemuPPC64le) Logger() *logrus.Entry {
	return hvLogger.WithField("subsystem", "qemuPPC64le")
}

func newQemuArch(config HypervisorConfig) (qemuArch, error) {
	machineType := config.HypervisorMachineType
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	if machineType != defaultQemuMachineType {
		return nil, fmt.Errorf("unrecognised machinetype: %v", machineType)
	}

	q := &qemuPPC64le{
		qemuArchBase{
			qemuMachine:          supportedQemuMachine,
			qemuExePath:          defaultQemuPath,
			memoryOffset:         config.MemOffset,
			kernelParamsNonDebug: kernelParamsNonDebug,
			kernelParamsDebug:    kernelParamsDebug,
			kernelParams:         kernelParams,
			protection:           noneProtection,
			legacySerial:         config.LegacySerial,
		},
	}

	if config.ConfidentialGuest {
		if err := q.enableProtection(); err != nil {
			return nil, err
		}

		if !q.qemuArchBase.disableNvdimm {
			hvLogger.WithField("subsystem", "qemuPPC64le").Warn("Nvdimm is not supported with confidential guest, disabling it.")
			q.qemuArchBase.disableNvdimm = true
		}
	}

	if err := q.handleImagePath(config); err != nil {
		return nil, err
	}

	q.memoryOffset = config.MemOffset

	return q, nil
}

func (q *qemuPPC64le) capabilities(hConfig HypervisorConfig) types.Capabilities {
	var caps types.Capabilities

	// pseries machine type supports hotplugging drives
	if q.qemuMachine.Type == QemuPseries {
		caps.SetBlockDeviceHotplugSupport()
		caps.SetNetworkDeviceHotplugSupported()
	}

	caps.SetMultiQueueSupport()
	if hConfig.SharedFS != config.NoSharedFS {
		caps.SetFsSharingSupport()
	}

	return caps
}

func (q *qemuPPC64le) bridges(number uint32) {
	q.Bridges = genericBridges(number, q.qemuMachine.Type)
}

func (q *qemuPPC64le) memoryTopology(memoryMb, hostMemoryMb uint64, slots uint8) govmmQemu.Memory {

	q.Logger().Debug("Aligning maxmem to multiples of 256MB. Assumption: Kernel Version >= 4.11")
	hostMemoryMb -= (hostMemoryMb % 256)
	return genericMemoryTopology(memoryMb, hostMemoryMb, slots, q.memoryOffset)
}

func (q *qemuPPC64le) appendIOMMU(devices []govmmQemu.Device) ([]govmmQemu.Device, error) {
	return devices, fmt.Errorf("PPC64le does not support appending a vIOMMU")
}

// Enables guest protection
func (q *qemuPPC64le) enableProtection() error {
	var err error
	q.protection, err = availableGuestProtection()
	if err != nil {
		return err
	}

	switch q.protection {
	case pefProtection:
		if q.qemuMachine.Options != "" {
			q.qemuMachine.Options += ","
		}
		q.qemuMachine.Options += fmt.Sprintf("confidential-guest-support=%s", pefID)
		hvLogger.WithFields(logrus.Fields{
			"subsystem":     "qemuPPC64le",
			"machine":       q.qemuMachine,
			"kernel-params": q.kernelParams,
		}).Info("Enabling PEF protection")
		return nil

	default:
		return fmt.Errorf("This system doesn't support Confidential Computing (Guest Protection)")
	}
}

// append protection device
func (q *qemuPPC64le) appendProtectionDevice(devices []govmmQemu.Device, firmware, firmwareVolume string) ([]govmmQemu.Device, string, error) {
	switch q.protection {
	case pefProtection:
		return append(devices,
			govmmQemu.Object{
				Driver:   govmmQemu.SpaprTPMProxy,
				Type:     govmmQemu.PEFGuest,
				ID:       pefID,
				DeviceID: tpmID,
				File:     tpmHostPath,
			}), firmware, nil
	case noneProtection:
		return devices, firmware, nil

	default:
		return devices, "", fmt.Errorf("Unsupported guest protection technology: %v", q.protection)
	}
}
