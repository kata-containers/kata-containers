// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"os/exec"
	"reflect"
	"strings"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/sirupsen/logrus"

	"github.com/intel-go/cpuid"
	govmmQemu "github.com/kata-containers/govmm/qemu"
)

type qemuAmd64 struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase

	vmFactory bool

	devLoadersCount uint32
}

const (
	defaultQemuPath = "/usr/bin/qemu-system-x86_64"

	defaultQemuMachineType = QemuQ35

	defaultQemuMachineOptions = "accel=kvm,kernel_irqchip=on"

	qmpMigrationWaitTimeout = 5 * time.Second

	tdxSysFirmwareDir = "/sys/firmware/tdx_seam/"

	tdxCPUFlag = "tdx"

	sevKvmParameterPath = "/sys/module/kvm_amd/parameters/sev"

	// Guest Owner Proxy Client
	// gop-client is a *temporary* component of the confidential containers CCv0 demo.
	//
	// The guest owner proxy (gop-client.py) acts as the local client for
	// a remote Guest Owner server.  The local client fowards encrypted
	// messages between the SEV hardware and the external guest owner.
	//
	// Source: https://github.com/confidential-containers-demo/scripts/tree/main/guest-owner-proxy
	//
	sevGuestOwnerProxyClient = "/opt/sev/guest-owner-proxy/gop-client.py"
)

var qemuPaths = map[string]string{
	QemuQ35:     defaultQemuPath,
	QemuMicrovm: defaultQemuPath,
}

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
	{"console", "hvc0"},
	{"console", "hvc1"},
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
		Type:    QemuVirt,
		Options: defaultQemuMachineOptions,
	},
	{
		Type:    QemuMicrovm,
		Options: defaultQemuMachineOptions,
	},
}

// MaxQemuVCPUs returns the maximum number of vCPUs supported
func MaxQemuVCPUs() uint32 {
	return uint32(240)
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

	factory := false
	if config.BootToBeTemplate || config.BootFromTemplate {
		factory = true
	}

	// IOMMU and Guest Protection require a split IRQ controller for handling interrupts
	// otherwise QEMU won't be able to create the kernel irqchip
	if config.IOMMU || config.ConfidentialGuest {
		mp.Options = "accel=kvm,kernel_irqchip=split"
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
			qemuExePath:          qemuPaths[machineType],
			memoryOffset:         config.MemOffset,
			kernelParamsNonDebug: kernelParamsNonDebug,
			kernelParamsDebug:    kernelParamsDebug,
			kernelParams:         kernelParams,
			disableNvdimm:        config.DisableImageNvdimm,
			dax:                  true,
			protection:           noneProtection,
		},
		vmFactory: factory,
	}

	if config.ConfidentialGuest {
		if err := q.enableProtection(); err != nil {
			return nil, err
		}
	}

	q.handleImagePath(config)

	return q, nil
}

func (q *qemuAmd64) capabilities() types.Capabilities {
	var caps types.Capabilities

	if q.qemuMachine.Type == QemuQ35 ||
		q.qemuMachine.Type == QemuVirt {
		caps.SetBlockDeviceHotplugSupport()
	}

	caps.SetMultiQueueSupport()
	caps.SetFsSharingSupport()

	return caps
}

func (q *qemuAmd64) bridges(number uint32) {
	q.Bridges = genericBridges(number, q.qemuMachine.Type)
}

func (q *qemuAmd64) cpuModel() string {
	cpuModel := defaultCPUModel

	// VMX is not migratable yet.
	// issue: https://github.com/kata-containers/runtime/issues/1750
	if q.vmFactory {
		hvLogger.WithField("subsystem", "qemuAmd64").Warn("VMX is not migratable yet: turning it off")
		cpuModel += ",vmx=off"
	}

	return cpuModel
}

func (q *qemuAmd64) memoryTopology(memoryMb, hostMemoryMb uint64, slots uint8) govmmQemu.Memory {
	return genericMemoryTopology(memoryMb, hostMemoryMb, slots, q.memoryOffset)
}

// Is Memory Hotplug supported by this architecture/machine type combination?
func (q *qemuAmd64) supportGuestMemoryHotplug() bool {
	// true for all amd64 machine types except for microvm.
	return q.qemuMachine.Type != govmmQemu.MachineTypeMicrovm
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
	logger := hvLogger.WithFields(logrus.Fields{
		"subsystem":               "qemuAmd64",
		"machine":                 q.qemuMachine,
		"kernel-params-debug":     q.kernelParamsDebug,
		"kernel-params-non-debug": q.kernelParamsNonDebug,
		"kernel-params":           q.kernelParams})

	switch q.protection {
	case tdxProtection:
		if q.qemuMachine.Options != "" {
			q.qemuMachine.Options += ","
		}
		q.qemuMachine.Options += "kvm-type=tdx,confidential-guest-support=tdx"
		q.kernelParams = append(q.kernelParams, Param{"tdx_guest", ""})
		logger.Info("Enabling TDX guest protection")
		return nil
	case sevProtection:
		if q.qemuMachine.Options != "" {
			q.qemuMachine.Options += ","
		}
		q.qemuMachine.Options += "confidential-guest-support=sev"
		logger.Info("Enabling SEV guest protection")
		return nil

	// TODO: Add support for other x86_64 technologies

	default:
		return fmt.Errorf("This system doesn't support Confidential Computing (Guest Protection)")
	}
}

// append protection device
func (q *qemuAmd64) appendProtectionDevice(devices []govmmQemu.Device, firmware string) ([]govmmQemu.Device, string, error) {
	switch q.protection {
	case tdxProtection:
		id := q.devLoadersCount
		q.devLoadersCount += 1
		return append(devices,
			govmmQemu.Object{
				Driver:   govmmQemu.Loader,
				Type:     govmmQemu.TDXGuest,
				ID:       "tdx",
				DeviceID: fmt.Sprintf("fd%d", id),
				Debug:    false,
				File:     firmware,
			}), "", nil
	case sevProtection:
		return append(devices,
			govmmQemu.Object{
				Type:            govmmQemu.SEVGuest,
				ID:              "sev",
				Debug:           false,
				File:            firmware,
				CBitPos:         cpuid.AMDMemEncrypt.CBitPosition,
				ReducedPhysBits: cpuid.AMDMemEncrypt.PhysAddrReduction,
			}), "", nil
	case noneProtection:
		return devices, firmware, nil

	default:
		return devices, "", fmt.Errorf("Unsupported guest protection technology: %v", q.protection)
	}
}

// setup prelaunch attestation
func (q *qemuArchBase) setupGuestAttestation(ctx context.Context, config govmmQemu.Config, path string, proxy string) (govmmQemu.Config, error) {
	switch q.protection {
	case sevProtection:
		logger := virtLog.WithField("subsystem", "SEV attestation")
		logger.Info("Set up prelaunch attestation")

		// start VM in stalled state
		config.Knobs.Stopped = true
		// Pull the launch bundle and guest DH key from GOP client
		// nsenter moves child process back to the host network namespace
		cmd := exec.Command(sevGuestOwnerProxyClient, "GetBundle", proxy, path)
		out, err := cmd.CombinedOutput()
		cmd.Wait()
		if err != nil {
			logger.Error("GetBundle Failed: %s", err)
			return config, err
		}
		// TODO: error check
		out_string := strings.TrimSuffix(string(out), "\n")
		gop_result := strings.Split(string(out_string), ",")

		// Place launch args into qemuConfig.Devices struct
		for i := range config.Devices {
			if reflect.TypeOf(config.Devices[i]).String() == "qemu.Object" {
				if config.Devices[i].(govmmQemu.Object).Type == govmmQemu.SEVGuest {
					dev := config.Devices[i].(govmmQemu.Object)
					dev.CertFilePath = gop_result[0]
					dev.SessionFilePath = gop_result[1]
					dev.DeviceID = gop_result[2]
					dev.KernelHashes = true
					config.Devices[i] = dev
					break
				}
			}
		}
		return config, nil
	default:
		return config, nil
	}
}

// wait for prelaunch attestation to complete
func (q *qemuArchBase) prelaunchAttestation(ctx context.Context, qmp *govmmQemu.QMP, config govmmQemu.Config, path string, proxy string, keyset string) error {
	switch q.protection {
	case sevProtection:
		logger := virtLog.WithField("subsystem", "SEV attestation")
		logger.Info("Processing prelaunch attestation")
		connection_id := ""
		for i := range config.Devices {
			if reflect.TypeOf(config.Devices[i]).String() == "qemu.Object" {
				if config.Devices[i].(govmmQemu.Object).Type == govmmQemu.SEVGuest {
					dev := config.Devices[i].(govmmQemu.Object)
					connection_id = dev.DeviceID
					break
				}
			}
		}
		// Pull the launch measurement from VM
		launch_measure, err := qmp.ExecuteQuerySEVLaunchMeasure(ctx)
		if err != nil {
			return err
		}
		// Pass launch measurement to GOP client, get secret bundle in return
		// nsenter moves child process back to the host network namespace
		cmd := exec.Command("nsenter", "-t", "1", "-n", "--", sevGuestOwnerProxyClient, "GetSecret", "-c", connection_id, "-i", keyset, "-m", string(launch_measure.Measurement), proxy, path)
		out, err := cmd.CombinedOutput()
		cmd.Wait()
		if err != nil {
			logger.Error("GetSecret FAILED: ", err, string(out))
			return err
		}
		out_string := strings.TrimSuffix(string(out), "\n")
		gop_result := strings.Split(out_string, ",")
		logger.Info("Received secret bundle from guest owner")
		secret_header := gop_result[0]
		secret := gop_result[1]

		// Inject secret into VM
		if err := qmp.ExecuteSEVInjectLaunchSecret(ctx, secret_header, secret); err != nil {
			return err
		}
		// Continue the VM
		return qmp.ExecuteCont(ctx)
	default:
		return nil
	}
}
