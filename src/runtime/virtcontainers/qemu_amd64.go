//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"crypto/sha256"
	b64 "encoding/base64"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/sev"
	sevKbs "github.com/kata-containers/kata-containers/src/runtime/pkg/sev/kbs"
	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/simple-kbs"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/sirupsen/logrus"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"

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

	snpCertsPath string

	snpGuestPolicy uint64

	numVCPUs uint32
}

const (
	defaultQemuPath = "/usr/bin/qemu-system-x86_64"

	defaultQemuMachineType = QemuQ35

	defaultQemuMachineOptions = "accel=kvm"

	splitIrqChipMachineOptions = "accel=kvm,kernel_irqchip=split"

	qmpMigrationWaitTimeout = 5 * time.Second

	sevAttestationGrpcTimeout = 10 * time.Second

	sevAttestationTempDir = "sev"

	sevAttestationGodhName = "godh.b64"

	sevAttestationSessionFileName = "session_file.b64"

	// For more info, see AMD SEV API document 55766
	sevPolicyBitSevEs = 0x4
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
		Type:    QemuVirt,
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

	factory := false
	if config.BootToBeTemplate || config.BootFromTemplate {
		factory = true
	}

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
		snpGuestPolicy: config.SNPGuestPolicy,
		snpCertsPath:   config.SnpCertsPath,
		qgsPort:        config.QgsPort,
	}

	if config.ConfidentialGuest {
		if err := q.enableProtection(); err != nil {
			return nil, err
		}

		if !q.qemuArchBase.disableNvdimm {
			hvLogger.WithField("subsystem", "qemuAmd64").Warn("Nvdimm is not supported with confidential guest, disabling it.")
			q.qemuArchBase.disableNvdimm = true
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

	if q.qemuMachine.Type == QemuQ35 ||
		q.qemuMachine.Type == QemuVirt {
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
	q.Bridges = genericBridges(number, q.qemuMachine.Type)
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
		"kernel-params":           q.kernelParams})

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
func (q *qemuAmd64) appendProtectionDevice(devices []govmmQemu.Device, firmware, firmwareVolume string) ([]govmmQemu.Device, string, error) {
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
		return append(devices,
			govmmQemu.Object{
				Type:            govmmQemu.SNPGuest,
				ID:              "snp",
				Debug:           false,
				File:            firmware,
				CBitPos:         cpuid.AMDMemEncrypt.CBitPosition,
				SnpPolicy:       q.snpGuestPolicy,
				ReducedPhysBits: 1,
				SnpCertsPath:    q.snpCertsPath,
			}), "", nil
	case noneProtection:

		return devices, firmware, nil

	default:
		return devices, "", fmt.Errorf("Unsupported guest protection technology: %v", q.protection)
	}
}

// Add the SEV Object qemu parameters for sev guest protection
func (q *qemuAmd64) appendSEVObject(devices []govmmQemu.Device, firmware, firmwareVolume string, config sevKbs.GuestPreAttestationConfig) ([]govmmQemu.Device, string, error) {
	attestationDataPath := filepath.Join(os.TempDir(), sevAttestationTempDir, config.LaunchId)
	sevGodhPath := filepath.Join(attestationDataPath, sevAttestationGodhName)
	sevSessionFilePath := filepath.Join(attestationDataPath, sevAttestationSessionFileName)

	// If attestation is enabled, add the certfile and session file
	// and the kernel hashes flag.
	if len(config.LaunchId) > 0 {
		return append(devices,
			govmmQemu.Object{
				Type:               govmmQemu.SEVGuest,
				ID:                 "sev",
				Debug:              false,
				File:               firmware,
				CBitPos:            cpuid.AMDMemEncrypt.CBitPosition,
				ReducedPhysBits:    cpuid.AMDMemEncrypt.PhysAddrReduction,
				SevPolicy:          config.Policy,
				SevCertFilePath:    sevGodhPath,
				SevSessionFilePath: sevSessionFilePath,
				SevKernelHashes:    true,
			}), "", nil
	} else {
		return append(devices,
			govmmQemu.Object{
				Type:            govmmQemu.SEVGuest,
				ID:              "sev",
				Debug:           false,
				File:            firmware,
				CBitPos:         cpuid.AMDMemEncrypt.CBitPosition,
				ReducedPhysBits: cpuid.AMDMemEncrypt.PhysAddrReduction,
				SevPolicy:       config.Policy,
				SevKernelHashes: true,
			}), "", nil
	}
}

// setup prelaunch attestation for AMD SEV guests
func (q *qemuAmd64) setupSEVGuestPreAttestation(ctx context.Context, config sevKbs.GuestPreAttestationConfig) (string, error) {

	logger := virtLog.WithField("subsystem", "SEV attestation")
	logger.Info("Set up prelaunch attestation")

	certChainBin, err := os.ReadFile(config.CertChainPath)
	if err != nil {
		return "", fmt.Errorf("Attestation certificate chain file not found: %v", err)
	}

	certChain := b64.StdEncoding.EncodeToString([]byte(certChainBin))

	conn, err := grpc.Dial(config.Proxy, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return "", fmt.Errorf("Could not connect to attestation proxy: %v", err)
	}

	client := pb.NewKeyBrokerServiceClient(conn)
	clientContext, cancel := context.WithTimeout(context.Background(), sevAttestationGrpcTimeout)
	defer cancel()

	request := pb.BundleRequest{
		CertificateChain: string(certChain),
		Policy:           config.Policy,
	}
	bundleResponse, err := client.GetBundle(clientContext, &request)
	if err != nil {
		return "", fmt.Errorf("Error receiving launch bundle from attestation proxy: %v", err)
	}

	attestationId := bundleResponse.LaunchId
	if attestationId == "" {
		return "", fmt.Errorf("Error receiving launch ID from attestation proxy: %v", err)
	}
	attestationDataPath := filepath.Join(os.TempDir(), sevAttestationTempDir, attestationId)
	err = os.MkdirAll(attestationDataPath, os.ModePerm)
	if err != nil {
		return "", fmt.Errorf("Could not create attestation directory: %v", err)
	}

	sevGodhPath := filepath.Join(attestationDataPath, sevAttestationGodhName)
	sevSessionFilePath := filepath.Join(attestationDataPath, sevAttestationSessionFileName)

	err = os.WriteFile(sevGodhPath, []byte(bundleResponse.GuestOwnerPublicKey), 0777)
	if err != nil {
		return "", fmt.Errorf("Could not write godh file: %v", err)
	}
	err = os.WriteFile(sevSessionFilePath, []byte(bundleResponse.LaunchBlob), 0777)
	if err != nil {
		return "", fmt.Errorf("Could not write session file: %v", err)
	}

	return attestationId, nil
}

func getCPUSig(cpuModel string) sev.VCPUSig {
	// This is for the special case for SNP (see cpuModel()).
	if cpuModel == "EPYC-v4" {
		return sev.SigEpycV4
	}
	return sev.NewVCPUSig(cpuid.DisplayFamily, cpuid.DisplayModel, cpuid.SteppingId)
}

func calculateGuestLaunchDigest(config sevKbs.GuestPreAttestationConfig, numVCPUs int, cpuModel string) ([sha256.Size]byte, error) {
	if config.Policy&sevPolicyBitSevEs != 0 {
		// SEV-ES guest
		return sev.CalculateSEVESLaunchDigest(
			numVCPUs,
			getCPUSig(cpuModel),
			config.FwPath,
			config.KernelPath,
			config.InitrdPath,
			config.KernelParameters)
	}

	// SEV guest
	return sev.CalculateLaunchDigest(
		config.FwPath,
		config.KernelPath,
		config.InitrdPath,
		config.KernelParameters)
}

// wait for prelaunch attestation to complete
func (q *qemuAmd64) sevGuestPreAttestation(ctx context.Context,
	qmp *govmmQemu.QMP, config sevKbs.GuestPreAttestationConfig) error {

	logger := virtLog.WithField("subsystem", "SEV attestation")
	logger.Info("Processing prelaunch attestation")

	// Pull the launch measurement from VM
	launchMeasure, err := qmp.ExecuteQuerySEVLaunchMeasure(ctx)
	if err != nil {
		return fmt.Errorf("ExecuteQuerySEVLaunchMeasure error: %v", err)
	}

	qemuSevInfo, err := qmp.ExecuteQuerySEV(ctx)
	if err != nil {
		return fmt.Errorf("ExecuteQuerySEV error: %v", err)
	}

	// gRPC connection
	conn, err := grpc.Dial(config.Proxy, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		log.Fatalf("did not connect: %v", err)
		return fmt.Errorf("Could not connected to attestation proxy: %v", err)
	}

	client := pb.NewKeyBrokerServiceClient(conn)
	clientContext, cancel := context.WithTimeout(context.Background(), sevAttestationGrpcTimeout)
	defer cancel()

	requestDetails := pb.RequestDetails{
		Guid:       config.SecretGuid,
		Format:     "JSON",
		SecretType: config.SecretType,
		Id:         config.Keyset,
	}

	secrets := []*pb.RequestDetails{&requestDetails}

	launchDigest, err := calculateGuestLaunchDigest(config, int(q.numVCPUs), q.cpuModel())
	if err != nil {
		return fmt.Errorf("Could not calculate SEV/SEV-ES launch digest: %v", err)
	}
	launchDigestBase64 := b64.StdEncoding.EncodeToString(launchDigest[:])

	request := pb.SecretRequest{
		LaunchMeasurement: launchMeasure.Measurement,
		LaunchId:          config.LaunchId,      // stored from bundle request
		Policy:            config.Policy,        // Stored from startup
		ApiMajor:          qemuSevInfo.APIMajor, // from qemu.SEVInfo
		ApiMinor:          qemuSevInfo.APIMinor,
		BuildId:           qemuSevInfo.BuildId,
		FwDigest:          launchDigestBase64,
		LaunchDescription: "shim launch",
		SecretRequests:    secrets,
	}
	logger.Info("requesting secrets")
	secretResponse, err := client.GetSecret(clientContext, &request)
	if err != nil {
		return fmt.Errorf("Unable to acquire launch secret from KBS: %v", err)
	}

	secretHeader := secretResponse.LaunchSecretHeader
	secret := secretResponse.LaunchSecretData

	// Inject secret into VM
	if err := qmp.ExecuteSEVInjectLaunchSecret(ctx, secretHeader, secret); err != nil {
		return err
	}

	// Clean up attestation state
	err = os.RemoveAll(filepath.Join(os.TempDir(), sevAttestationTempDir, config.LaunchId))
	if err != nil {
		logger.Warning("Unable to clean up attestation directory")
	}

	// Continue the VM
	logger.Info("Launch secrets injected. Continuing the VM.")
	return qmp.ExecuteCont(ctx)
}
