//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"

	"github.com/intel-go/cpuid"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
)

type qemuAmd64 struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase

	snpGuest bool

	vmFactory bool

	devLoadersCount uint32

	sgxEPCSize int64

	qgsPort uint32

	snpIdBlock string

	snpIdAuth string

	snpGuestPolicy *uint64

	// firmwarePath is the host path to the guest firmware blob (OVMF). When
	// non-empty on Q35, hot-pluggable PCI bridges are emitted as
	// pcie-pci-bridge devices behind a dedicated pcie-root-port so that OVMF
	// reserves the required bus/IO/MMIO/pref64 windows. SeaBIOS (empty
	// firmwarePath) keeps the legacy conventional pci-bridge topology.
	firmwarePath string
}

const (
	defaultQemuPath = "/usr/bin/qemu-system-x86_64"

	defaultQemuMachineType = QemuQ35

	defaultQemuMachineOptions = "accel=kvm"

	splitIrqChipMachineOptions = "accel=kvm,kernel_irqchip=split"

	qmpMigrationWaitTimeout = 5 * time.Second
)

var kernelParams = []Param{
	{"tsc", "reliable"},
	{"no_timer_check", ""},
	{"rcupdate.rcu_expedited", "1"},
	{"i8042.direct", "1"},
	{"i8042.dumbkbd", "1"},
	{"i8042.nopnp", "1"},
	{"i8042.noaux", "1"},
	{"noreplace-smp", ""},
	{"reboot", "k"},
	{"cryptomgr.notests", ""},
	{"net.ifnames", "0"},
	{"pci", "lastbus=0"},
}

var supportedQemuMachines = []govmmQemu.Machine{
	{
		Type:    QemuQ35,
		Options: defaultQemuMachineOptions,
	},
	{
		Type:    QemuMicrovm,
		Options: defaultQemuMachineOptions,
	},
}

func newQemuArch(config HypervisorConfig) (qemuArch, error) {
	machineType := config.HypervisorMachineType
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	var mp *govmmQemu.Machine
	for _, m := range supportedQemuMachines {
		if m.Type == machineType {
			mp = &m
			break
		}
	}
	if mp == nil {
		return nil, fmt.Errorf("unrecognised machinetype: %v", machineType)
	}

	factory := config.BootToBeTemplate || config.BootFromTemplate

	// IOMMU and Guest Protection require a split IRQ controller for handling interrupts
	// otherwise QEMU won't be able to create the kernel irqchip
	if config.IOMMU || config.ConfidentialGuest {
		mp.Options = splitIrqChipMachineOptions
	}

	if config.IOMMU {
		kernelParams = append(kernelParams,
			Param{"intel_iommu", "on"})
		kernelParams = append(kernelParams,
			Param{"iommu", "pt"})
	}

	q := &qemuAmd64{
		qemuArchBase: qemuArchBase{
			qemuMachine:          *mp,
			qemuExePath:          defaultQemuPath,
			memoryOffset:         config.MemOffset,
			kernelParamsNonDebug: kernelParamsNonDebug,
			kernelParamsDebug:    kernelParamsDebug,
			kernelParams:         kernelParams,
			disableNvdimm:        config.DisableImageNvdimm,
			dax:                  true,
			protection:           noneProtection,
			legacySerial:         config.LegacySerial,
		},
		vmFactory:      factory,
		snpGuest:       config.SevSnpGuest,
		qgsPort:        config.QgsPort,
		snpIdBlock:     config.SnpIdBlock,
		snpIdAuth:      config.SnpIdAuth,
		snpGuestPolicy: config.SnpGuestPolicy,
		firmwarePath:   config.FirmwarePath,
	}

	if config.ConfidentialGuest {
		if err := q.enableProtection(); err != nil {
			return nil, err
		}

		if !q.disableNvdimm {
			hvLogger.WithField("subsystem", "qemuAmd64").Warn("Nvdimm is not supported with confidential guest, disabling it.")
			q.disableNvdimm = true
		}
	}

	if config.SGXEPCSize != 0 {
		q.sgxEPCSize = config.SGXEPCSize
		if q.qemuMachine.Options != "" {
			q.qemuMachine.Options += ","
		}
		// qemu sandboxes will only support one EPC per sandbox
		// this is because there is only one annotation (sgx.intel.com/epc)
		// to specify the size of the EPC.
		q.qemuMachine.Options += "sgx-epc.0.memdev=epc0,sgx-epc.0.node=0"
	}

	if err := q.handleImagePath(config); err != nil {
		return nil, err
	}

	return q, nil
}

func (q *qemuAmd64) capabilities(hConfig HypervisorConfig) types.Capabilities {
	var caps types.Capabilities

	if q.qemuMachine.Type == QemuQ35 {
		caps.SetBlockDeviceHotplugSupport()
		caps.SetNetworkDeviceHotplugSupported()
	}

	caps.SetMultiQueueSupport()
	if hConfig.SharedFS != config.NoSharedFS {
		caps.SetFsSharingSupport()
	}

	return caps
}

func (q *qemuAmd64) bridges(number uint32) {
	// On Q35 + OVMF, the conventional pci-bridge does not get its IO/MMIO/
	// pref64 hot-plug windows reserved by the firmware (OVMF only honours
	// the PCI Firmware Spec resource-reservation hints on PCIe root/
	// downstream ports). Devices hot-plugged behind it therefore have no
	// usable resource window and the guest kernel never sees them, which
	// breaks network (and any other) PCI hot-plug.
	//
	// Emit a pcie-pci-bridge sitting behind a dedicated pcie-root-port
	// instead: OVMF reserves the windows on the root port, the
	// pcie-pci-bridge inherits them, and the existing PCI hot-plug code
	// path keeps working (just with one extra level in the guest PCI
	// path).
	if q.qemuMachine.Type == QemuQ35 && q.firmwarePath != "" {
		q.Bridges = nestedPCIeBridges(number)
		return
	}
	q.Bridges = genericBridges(number, q.qemuMachine.Type)
}

// nestedPCIeBridges builds hot-plug-capable PCI bridges that sit behind a
// per-bridge pcie-root-port. From the guest's point of view the bridge's
// secondary bus is still a conventional PCI bus (so we keep types.PCI for the
// hot-plug bookkeeping), but it is emitted on the QEMU command line as a
// pcie-pci-bridge cold-plugged onto a pcie-root-port so OVMF reserves the
// required bus/IO/MMIO/pref64 windows. The "this is nested" signal is the
// ParentID/ParentAddr pair carried by the bridge itself.
func nestedPCIeBridges(number uint32) []types.Bridge {
	var bridges []types.Bridge
	for i := uint32(0); i < number; i++ {
		bridgeID := fmt.Sprintf("%s-bridge-%d", types.PCI, i)
		parentID := fmt.Sprintf("rp-%s", bridgeID)
		// Addr/ParentAddr will be assigned by genericAppendBridges when
		// the QEMU command line is built; we leave Addr=0 because the
		// pcie-pci-bridge sits at slot 0 of its root port's downstream
		// bus.
		bridges = append(bridges, types.NewNestedBridge(
			types.PCI,
			bridgeID,
			make(map[uint32]string),
			0, /* Addr: slot 0 on root-port's secondary bus */
			parentID,
			0, /* ParentAddr: filled in by genericAppendBridges */
		))
	}
	return bridges
}

func (q *qemuAmd64) cpuModel() string {
	var err error
	cpuModel := defaultCPUModel

	// Temporary until QEMU cpu model 'host' supports AMD SEV-SNP
	protection, err := availableGuestProtection()
	if err == nil {
		if protection == snpProtection && q.snpGuest {
			cpuModel = "EPYC-v4"
		}
	}

	return cpuModel
}

func (q *qemuAmd64) memoryTopology(memoryMb, hostMemoryMb uint64, slots uint8) govmmQemu.Memory {
	return genericMemoryTopology(memoryMb, hostMemoryMb, slots, q.memoryOffset)
}

// Is Memory Hotplug supported by this architecture/machine type combination?
func (q *qemuAmd64) supportGuestMemoryHotplug() bool {
	// true for all amd64 machine types except for microvm.
	if q.qemuMachine.Type == govmmQemu.MachineTypeMicrovm {
		return false
	}

	return q.protection == noneProtection
}

func (q *qemuAmd64) appendImage(ctx context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	if !q.disableNvdimm {
		return q.appendNvdimmImage(devices, path)
	}
	return q.appendBlockImage(ctx, devices, path)
}

// enable protection
func (q *qemuAmd64) enableProtection() error {
	var err error
	q.protection, err = availableGuestProtection()
	if err != nil {
		return err
	}
	// Configure SNP only if specified in config
	if q.protection == snpProtection && !q.snpGuest {
		q.protection = sevProtection
	}

	logger := hvLogger.WithFields(logrus.Fields{
		"subsystem":               "qemuAmd64",
		"machine":                 q.qemuMachine,
		"kernel-params-debug":     q.kernelParamsDebug,
		"kernel-params-non-debug": q.kernelParamsNonDebug,
		"kernel-params":           q.kernelParams,
	})

	switch q.protection {
	case tdxProtection:
		if q.qemuMachine.Options != "" {
			q.qemuMachine.Options += ","
		}
		q.qemuMachine.Options += "confidential-guest-support=tdx"
		logger.Info("Enabling TDX guest protection")
		return nil
	case sevProtection:
		if q.qemuMachine.Options != "" {
			q.qemuMachine.Options += ","
		}
		q.qemuMachine.Options += "confidential-guest-support=sev"
		logger.Info("Enabling SEV guest protection")
		return nil
	case snpProtection:
		if q.qemuMachine.Options != "" {
			q.qemuMachine.Options += ","
		}
		q.qemuMachine.Options += "confidential-guest-support=snp"
		logger.Info("Enabling SNP guest protection")
		return nil

	// TODO: Add support for other x86_64 technologies

	default:
		return fmt.Errorf("This system doesn't support Confidential Computing (Guest Protection)")
	}
}

// append protection device
func (q *qemuAmd64) appendProtectionDevice(devices []govmmQemu.Device, firmware, firmwareVolume string, initdataDigest []byte) ([]govmmQemu.Device, string, error) {
	if q.sgxEPCSize != 0 {
		devices = append(devices,
			govmmQemu.Object{
				Type:     govmmQemu.MemoryBackendEPC,
				ID:       "epc0",
				Prealloc: true,
				Size:     uint64(q.sgxEPCSize),
			})
	}

	switch q.protection {
	case tdxProtection:
		id := q.devLoadersCount
		q.devLoadersCount += 1
		return append(devices,
			govmmQemu.Object{
				Driver:         govmmQemu.Loader,
				Type:           govmmQemu.TDXGuest,
				QgsPort:        q.qgsPort,
				ID:             "tdx",
				DeviceID:       fmt.Sprintf("fd%d", id),
				Debug:          false,
				File:           firmware,
				FirmwareVolume: firmwareVolume,
				InitdataDigest: initdataDigest,
			}), "", nil
	case sevProtection:
		return append(devices,
			govmmQemu.Object{
				Type:            govmmQemu.SEVGuest,
				ID:              "sev",
				Debug:           false,
				File:            firmware,
				CBitPos:         cpuid.AMDMemEncrypt.CBitPosition,
				ReducedPhysBits: 1,
			}), "", nil
	case snpProtection:
		obj := govmmQemu.Object{
			Type:            govmmQemu.SNPGuest,
			ID:              "snp",
			Debug:           false,
			File:            firmware,
			CBitPos:         cpuid.AMDMemEncrypt.CBitPosition,
			ReducedPhysBits: 1,
			InitdataDigest:  initdataDigest,
			SnpGuestPolicy:  q.snpGuestPolicy,
		}
		if q.snpIdBlock != "" && q.snpIdAuth != "" {
			obj.SnpIdBlock = q.snpIdBlock
			obj.SnpIdAuth = q.snpIdAuth
		} else if q.snpIdBlock != "" {
			return nil, "", errors.New("specifying SNP IDBlock without SNP IDAuth is not allowed")
		} else if q.snpIdAuth != "" {
			return nil, "", errors.New("specifying SNP IDAuth without SNP IDBlock is not allowed")
		}
		return append(devices, obj), "", nil
	case noneProtection:

		return devices, firmware, nil

	default:
		return devices, "", fmt.Errorf("Unsupported guest protection technology: %v", q.protection)
	}
}
