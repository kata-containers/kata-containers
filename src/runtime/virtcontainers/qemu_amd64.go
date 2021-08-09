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

	defaultQemuMachineOptions = "accel=kvm,kernel_irqchip"

	qmpMigrationWaitTimeout = 5 * time.Second

	tdxSysFirmwareDir = "/sys/firmware/tdx_seam/"

	tdxCPUFlag = "tdx"

	sevKvmParameterPath = "/sys/module/kvm_amd/parameters/sev"

	sevGuestOwnerProxyClient = "/opt/sev/guest-owner-proxy/gop-client.py"
)

var qemuPaths = map[string]string{
	QemuPCLite:  "/usr/bin/qemu-lite-system-x86_64",
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
		Type:    QemuPCLite,
		Options: defaultQemuMachineOptions,
	},
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
		virtLog.WithField("subsystem", "qemuAmd64").Warn("VMX is not migratable yet: turning it off")
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

// appendBridges appends to devices the given bridges
func (q *qemuAmd64) appendBridges(devices []govmmQemu.Device) []govmmQemu.Device {
	return genericAppendBridges(devices, q.Bridges, q.qemuMachine.Type)
}

// enable protection
func (q *qemuAmd64) enableProtection() error {
	var err error
	q.protection, err = availableGuestProtection()
	if err != nil {
		return err
	}
	logger := virtLog.WithFields(logrus.Fields{
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
	logger := virtLog.WithFields(logrus.Fields{
		"subsystem":               "qemuAmd64",
		"machine":                 q.qemuMachine,
		"kernel-params-debug":     q.kernelParamsDebug,
		"kernel-params-non-debug": q.kernelParamsNonDebug,
		"kernel-params":           q.kernelParams})
	switch q.protection {
	case sevProtection:
		logger.Info("SEV attestation: Pulling launch argument...")
		logger.Info("SEV attestation: Server %s", proxy)
		logger.Info("SEV attestation: Path %s", path)

		// start VM in stalled state
		config.Knobs.Stopped = true

		// Pull the launch blob and godh from GOP
		// Use an external script until GOP client logic is built into the kata-runtime
		cmd := exec.Command(sevGuestOwnerProxyClient, "GetBundle", proxy, path)
		logger.Info("SEV attestation: GetBundle Command: ", cmd.String())
		out, err := cmd.CombinedOutput()
		cmd.Wait()
		if err != nil {
			logger.Info("SEV attestation: GetBundle FAILED: %s", err)
			return config, err
		}
		// TODO: error check
		out_string := strings.TrimSuffix(string(out), "\n")
		logger.Info("SEV attestation: Received GOP results: %s", out_string)
		gop_result := strings.Split(string(out_string), ",")
		fmt.Println(gop_result)
		logger.Info("SEV attestation: godh %s", gop_result[0])
		logger.Info("SEV attestation: launch measure %s", gop_result[1])
		logger.Info("SEV attestation: connection id %s", gop_result[2])

		// Place launch args into qemuConfig.Devices struct
		for i := range config.Devices {
			if reflect.TypeOf(config.Devices[i]).String() == "qemu.Object" {
				if config.Devices[i].(govmmQemu.Object).Type == govmmQemu.SEVGuest {
					logger.Info("SEV attestation: UPDATING DEVICE")
					dev := config.Devices[i].(govmmQemu.Object)
					dev.CertFilePath = gop_result[0]
					dev.SessionFilePath = gop_result[1]
					dev.DeviceID = gop_result[2]
					dev.SevPolicy = 0
					config.Devices[i] = dev
					break
				}
			} else {
				logger.Info("SEV attestation: ELSE %s", reflect.TypeOf(config.Devices[i]).String())
			}
		}
		return config, nil
	default:
		return config, nil
	}
}

// wait for prelaunch attestation to complete
func (q *qemuArchBase) prelaunchAttestation(ctx context.Context, qmp *govmmQemu.QMP, config govmmQemu.Config, path string, proxy string, keyset string) error {
	logger := virtLog.WithFields(logrus.Fields{
		"subsystem":               "qemuAmd64",
		"machine":                 q.qemuMachine,
		"kernel-params-debug":     q.kernelParamsDebug,
		"kernel-params-non-debug": q.kernelParamsNonDebug,
		"kernel-params":           q.kernelParams})
	switch q.protection {
	case sevProtection:
		// This will block and wait for the Guest Owner Proxy to validate the
		// prelaunch attestation measurement.
		logger.Info("SEV attestation: Processing prelaunch attestation")
		logger.Info("SEV attestation: Server %s", proxy)
		logger.Info("SEV attestation: Path %s", path)
		logger.Info("SEV attestation: Keyset %s", keyset)
		connection_id := ""
		var sev_policy uint32 = 0
		for i := range config.Devices {
			if reflect.TypeOf(config.Devices[i]).String() == "qemu.Object" {
				if config.Devices[i].(govmmQemu.Object).Type == govmmQemu.SEVGuest {
					dev := config.Devices[i].(govmmQemu.Object)
					connection_id = dev.DeviceID
					sev_policy = dev.SevPolicy
					break
				}
			}
		}
		// Pull the launch measurement from VM
		launch_measure, err := qmp.ExecuteQuerySEVLaunchMeasure(ctx)
		if err != nil {
			return err
		}
		logger.Info("SEV attestation: Connection ID: ", connection_id)
		logger.Info("SEV attestation: Policy: ", sev_policy)
		logger.Info("SEV attestation: Launch Measure: ", launch_measure.Measurement)

		// Pass launch measurement to GOP, get secret in return
		// Use an external script until GOP client logic is built into the kata-runtime
		// nsenter is used to move child process back to the host default netns
		cmd := exec.Command("nsenter", "-t", "1", "-n", "--", sevGuestOwnerProxyClient, "GetSecret", "-c", connection_id, "-i", keyset, "-m", string(launch_measure.Measurement), proxy, path)
		logger.Info("SEV attestation: GetSecret Command: ", cmd.String())
		out, err := cmd.CombinedOutput()
		cmd.Wait()
		if err != nil {
			logger.Info("SEV attestation: GetSecret FAILED: ", err, string(out))
			return err
		}
		out_string := strings.TrimSuffix(string(out), "\n")
		gop_result := strings.Split(out_string, ",")
		logger.Info("SEV attestation: Received GOP secrets: ", out_string)
		logger.Info("SEV attestation: secret header: ", gop_result[0])
		logger.Info("SEV attestation: secret: ", gop_result[1])
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
