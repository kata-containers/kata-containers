package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	device "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/uuid"
	"github.com/sirupsen/logrus"
)

// VM is abstraction of a virtual machine.
type VM struct {
	hypervisor vc.Hypervisor
	id         string
	cpu        uint32
	memory     uint32
}

// VMConfig is a collection of all info that a new blackbox VM needs.
type VMConfig struct {
	HypervisorType   vc.HypervisorType
	HypervisorConfig vc.HypervisorConfig
}

// Valid Check VMConfig validity.
func (c *VMConfig) Valid() error {
	return c.HypervisorConfig.Valid()
}

// NewVM creates a new VM based on provided VMConfig.
func NewVM(ctx context.Context, config VMConfig) (*VM, error) {
	// 1. setup hypervisor
	hypervisor, err := vc.NewHypervisor(config.HypervisorType)
	if err != nil {
		return nil, err
	}

	if err = config.Valid(); err != nil {
		return nil, err
	}

	id := uuid.Generate().String()

	if err = hypervisor.CreateVM(ctx, id, vc.NetworkNamespace{}, &config.HypervisorConfig); err != nil {
		return nil, err
	}

	return &VM{
		id:         id,
		hypervisor: hypervisor,
		cpu:        config.HypervisorConfig.NumVCPUs,
		memory:     config.HypervisorConfig.MemorySize,
	}, nil
}

func (v *VM) logger() logrus.FieldLogger {
	return logrus.WithFields(logrus.Fields{"vm": v.id})
}

// Pause pauses a VM.
func (v *VM) Pause(ctx context.Context) error {
	v.logger().Info("pause vm")
	return v.hypervisor.PauseVM(ctx)
}

func (v *VM) Save() error {
	// TODO: Not implemented
	v.logger().Info("Save vm")
	return v.hypervisor.SaveVM()
}

// Resume resumes a paused VM.
func (v *VM) Resume(ctx context.Context) error {
	v.logger().Info("resume vm")
	return v.hypervisor.ResumeVM(ctx)
}

// Start kicks off a configured VM.
func (v *VM) Start(ctx context.Context) error {
	v.logger().Info("start vm")
	return v.hypervisor.StartVM(ctx, vc.VmStartTimeout)
}

// Stop stops a VM process.
func (v *VM) Stop(ctx context.Context) error {
	v.logger().Info("stop vm")

	return v.hypervisor.StopVM(ctx, false)
}

func main() {
	var useQemu bool
	flag.BoolVar(&useQemu, "qemu", false, "use qemu. default cloud hypervisor")
	flag.Parse()

	vmCfg := VMConfig{}
	var bootDisk device.BlockDrive

	if useQemu {
		vmCfg.HypervisorType = vc.QemuHypervisor
		vmCfg.HypervisorConfig = vc.HypervisorConfig{
			IsSandbox:             false,
			HypervisorMachineType: "q35",
			NumVCPUs:              2,
			DefaultMaxVCPUs:       2,
			MemorySize:            2048,
			DefaultBridges:        1,
			MemSlots:              1,
			Debug:                 true,
			MemPrealloc:           false,
			HugePages:             false,
			IOMMU:                 false,
			Realtime:              false,
			Mlock:                 false,
		}
		bootDisk = device.BlockDrive{
			File:   "focal-server-cloudimg-amd64.img",
			Format: "qcow2",
			ID:     "bootdisk",
			Index:  1,
		}
	} else {
		vmCfg.HypervisorType = vc.ClhHypervisor
		vmCfg.HypervisorConfig = vc.HypervisorConfig{
			IsSandbox:         false,
			KernelPath:        "hypervisor-fw",
			HypervisorPath:    "cloud-hypervisor-static",
			EntropySource:     "/dev/urandom",
			NumVCPUs:          2,
			DefaultMaxVCPUs:   2,
			MemorySize:        2048,
			DefaultBridges:    1,
			MemSlots:          1,
			Debug:             true,
			BlockDeviceDriver: device.VirtioBlock, // For CLH
		}
		bootDisk = device.BlockDrive{
			File:   "focal-server-cloudimg-amd64.raw",
			Format: "raw",
			ID:     "bootdisk",
			Index:  1,
		}
	}
	cfgDrive := device.BlockDrive{
		File:   "config-drive.img",
		Format: "raw",
		ID:     "configdrive",
		Index:  2,
	}

	ctx := context.Background()

	vm, err := NewVM(ctx, vmCfg)
	if err != nil {
		fmt.Printf("Failed to create VM: %s\n", err)
		os.Exit(1)
	}
	fmt.Println("VM Created:", vm.id)

	if err := vm.hypervisor.AddDevice(ctx, bootDisk, vc.BlockDev); err != nil {
		fmt.Printf("Failed to attach boot drive: %s\n", err)
		os.Exit(1)
	}

	// Cloud init
	if err := vm.hypervisor.AddDevice(ctx, cfgDrive, vc.BlockDev); err != nil {
		fmt.Printf("Failed to attach config drive: %s\n", err)
		os.Exit(1)
	}

	macAddr := "0e:49:61:0f:c3:11"
	primaryNet := &vc.TuntapEndpoint{
		EndpointType: "tap",
		TuntapInterface: vc.TuntapInterface{
			Name: "clhtap",
			TAPIface: vc.NetworkInterface{
				Name:     "clhtap",
				HardAddr: macAddr,
			},
		},
		NetPair: vc.NetworkInterfacePair{
			TapInterface: vc.TapInterface{
				TAPIface: vc.NetworkInterface{
					Name:     "clhtap",
					HardAddr: macAddr,
				},
			},
		},
	}
	primaryNet.TuntapInterface.TAPIface.HardAddr = macAddr

	if err := vm.hypervisor.AddDevice(ctx, primaryNet, vc.NetDev); err != nil {
		fmt.Printf("Failed to attach network device: %s\n", err)
		os.Exit(1)
	}

	if err := vm.Start(ctx); err != nil {
		fmt.Printf("Failed to start VM: %s\n", err)
	}

	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt, syscall.SIGTERM)
	<-c

	fmt.Println("Shutting down")

	if err := vm.Stop(ctx); err != nil {
		fmt.Printf("Failed to stop VM: %s\n", err)
	}
}
