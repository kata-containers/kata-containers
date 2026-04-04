# Copyright (c) 2019-2022 Alibaba Cloud
# Copyright (c) 2019-2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

# To show variables or targets help on `make help`
# Use the following format:
# '##VAR VARIABLE_NAME: help about variable'
# '##TARGET TARGET_NAME: help about target'
PROJECT_TYPE = kata
PROJECT_NAME = Kata Containers
PROJECT_TAG = kata-containers
PROJECT_URL = https://github.com/kata-containers
PROJECT_COMPONENT = containerd-shim-kata-v2
CONTAINERD_RUNTIME_NAME = io.containerd.kata.v2

# This snippet finds all packages inside runtime-rs. Used for tessting.
PACKAGES := $(shell cargo metadata --no-deps --format-version 1 | \
                    jq -r '.packages[] | select(.manifest_path | contains("runtime-rs")) | .name')
PACKAGE_FLAGS := $(patsubst %,-p %,$(PACKAGES))

include ../../utils.mk

##VAR RELEASE_LTO=true|false|thin|fat LTO setting used for release builds
RELEASE_LTO ?= true
##VAR RELEASE_CODEGEN_UNITS=<number> codegen units used for release builds
RELEASE_CODEGEN_UNITS ?= 1
# Apply release profile overrides only for release builds.
RELEASE_CARGO_PROFILE_ENV = $(if $(findstring release,$(BUILD_TYPE)),CARGO_PROFILE_RELEASE_LTO=$(RELEASE_LTO) CARGO_PROFILE_RELEASE_CODEGEN_UNITS=$(RELEASE_CODEGEN_UNITS),)

ARCH_DIR = arch
ARCH_FILE_SUFFIX = -options.mk
ARCH_FILE = $(ARCH_DIR)/$(ARCH)$(ARCH_FILE_SUFFIX)

UNSUPPORTED_ARCHS := s390x powerpc64le riscv64gc

ifeq ($(filter $(ARCH), $(UNSUPPORTED_ARCHS)),$(ARCH))
default: runtime show-header
test:
	@echo "$(ARCH) is not currently supported"
	exit 0
install: install-runtime install-configs
else
##TARGET default: build code
default: runtime show-header
##TARGET test: run cargo tests for runtime-rs and all its sub-crates.
test: static-checks-build
	@cargo test $(PACKAGE_FLAGS) --target $(TRIPLE) $(EXTRA_RUSTFEATURES) -- --nocapture  --skip bindgen
install: install-runtime install-configs
endif

ifeq (,$(realpath $(ARCH_FILE)))
    $(error "ERROR: invalid architecture: '$(ARCH)'")
else
    # Load architecture-dependent settings
    include $(ARCH_FILE)
endif

ifeq ($(PREFIX),)
PREFIX := /usr
EXEC_PREFIX := $(PREFIX)/local
##VAR BINDIR=<path> is a directory for installing executable programs
BINDIR := $(EXEC_PREFIX)/bin
else
EXEC_PREFIX := $(PREFIX)
# when creating the kata-deploy image, the default installation path for go runtime is $(EXEC_PREFIX)/bin, so we put it here for multiple runtime
BINDIR := $(EXEC_PREFIX)/runtime-rs/bin/
endif

PREFIXDEPS := $(PREFIX)
LIBEXECDIR := $(PREFIXDEPS)/libexec
SHAREDIR := $(PREFIX)/share
DEFAULTSDIR := $(SHAREDIR)/defaults
CLHBINDIR := $(PREFIXDEPS)/bin
QEMUBINDIR := $(PREFIXDEPS)/bin
PROJECT_DIR = $(PROJECT_TAG)
IMAGENAME = $(PROJECT_TAG).img
IMAGECONFIDENTIALNAME = $(PROJECT_TAG)-confidential.img
INITRDNAME = $(PROJECT_TAG)-initrd.img
INITRDCONFIDENTIALNAME = $(PROJECT_TAG)-initrd-confidential.img
TARGET = $(PROJECT_COMPONENT)
SYSCONFDIR := /etc
LOCALSTATEDIR := /var
CONFIG_FILE = configuration.toml
RUNTIMENAME := virt_container
HYPERVISOR_DB = dragonball
HYPERVISOR_FC = firecracker
HYPERVISOR_QEMU = qemu
HYPERVISOR_CLH = cloud-hypervisor
HYPERVISOR_REMOTE = remote

ARCH_SUPPORT_DB := x86_64 aarch64
ifneq ($(filter $(ARCH),$(ARCH_SUPPORT_DB)),)
    # When set to true, builds the built-in Dragonball hypervisor
    USE_BUILTIN_DB := true
else
    USE_BUILTIN_DB := false
    $(info Dragonball does not support ARCH $(ARCH), disabled. \
        Specify "USE_BUILTIN_DB=true" to force enable.)
endif

HYPERVISOR ?= $(HYPERVISOR_QEMU)

##VAR HYPERVISOR=<hypervisor_name> List of hypervisors this build system can generate configuration for.
HYPERVISORS := $(HYPERVISOR_DB) $(HYPERVISOR_FC) $(HYPERVISOR_QEMU) $(HYPERVISOR_CLH) $(HYPERVISOR_REMOTE)

CLHPATH := $(CLHBINDIR)/$(CLHCMD)
CLHVALIDHYPERVISORPATHS := [\"$(CLHPATH)\"]

DBVALIDHYPERVISORPATHS := []
PKGDATADIR := $(PREFIXDEPS)/share/$(PROJECT_DIR)
PKGRUNDIR := $(LOCALSTATEDIR)/run/$(PROJECT_DIR)
KERNELDIR := $(PKGDATADIR)
IMAGEPATH := $(PKGDATADIR)/$(IMAGENAME)
IMAGECONFIDENTIALPATH := $(PKGDATADIR)/$(IMAGECONFIDENTIALNAME)
INITRDPATH := $(PKGDATADIR)/$(INITRDNAME)
INITRDCONFIDENTIALPATH := $(PKGDATADIR)/$(INITRDCONFIDENTIALNAME)

ROOTFSTYPE_EXT4 := \"ext4\"
ROOTFSTYPE_XFS := \"xfs\"
ROOTFSTYPE_EROFS := \"erofs\"
DEFROOTFSTYPE := $(ROOTFSTYPE_EXT4)

FCBINDIR := $(PREFIXDEPS)/bin
FCPATH = $(FCBINDIR)/$(FCCMD)
FCVALIDHYPERVISORPATHS := [\"$(FCPATH)\"]
FCJAILERPATH = $(FCBINDIR)/$(FCJAILERCMD)
FCVALIDJAILERPATHS = [\"$(FCJAILERPATH)\"]

PKGLIBEXECDIR := $(LIBEXECDIR)/$(PROJECT_DIR)

# EDK2 firmware names per architecture
ifeq ($(ARCH), x86_64)
    EDK2_NAME := ovmf
endif
ifeq ($(ARCH), aarch64)
    EDK2_NAME := aavmf
endif

# Set firmware path from QEMUFW if defined
FIRMWAREPATH :=
ifneq (,$(QEMUCMD))
    ifneq (,$(QEMUFW))
        FIRMWAREPATH := $(PREFIXDEPS)/share/$(EDK2_NAME)/$(QEMUFW)
    endif
endif

# Firmware path for qemu-nvidia-gpu (OVMF / AAVMF); parity with src/runtime/Makefile FIRMWAREPATH_NV
FIRMWAREPATH_NV :=
ifeq ($(ARCH), x86_64)
    FIRMWAREPATH_NV := $(PREFIXDEPS)/share/$(EDK2_NAME)/OVMF.fd
endif
ifeq ($(ARCH), aarch64)
    FIRMWAREPATH_NV := $(PREFIXDEPS)/share/$(EDK2_NAME)/AAVMF_CODE.fd
endif

KERNELVERITYPARAMS ?= ""

# TDX
DEFSHAREDFS_QEMU_TDX_VIRTIOFS := none
FIRMWARETDVFPATH := $(PREFIXDEPS)/share/ovmf/OVMF.inteltdx.fd

# SEV-SNP
FIRMWARE_SNP_PATH := $(PREFIXDEPS)/share/ovmf/AMDSEV.fd

##VAR DEFVCPUS=<number> Default number of vCPUs
DEFVCPUS := 1
##VAR DEFMAXVCPUS=<number> Default maximum number of vCPUs
DEFMAXVCPUS := 0
##VAR DEFMEMSZ=<number> Default memory size in MiB
DEFMEMSZ := 2048
##VAR DEFMEMSLOTS=<number> Default memory slots
# Cases to consider :
# - nvdimm rootfs image
# - preallocated memory
# - vm template memory
# - hugepage memory
DEFMEMSLOTS := 10
# Default maximum memory in MiB
DEFMAXMEMSZ := 0
##VAR DEFBRIDGES=<number> Default number of bridges
DEFBRIDGES := 1
##VAR DEFNETQUEUES=<number> Default number of network queues
DEFNETQUEUES := 1
DEFENABLEANNOTATIONS := [\"enable_iommu\", \"virtio_fs_extra_args\", \"kernel_params\", \"kernel_verity_params\", \"default_vcpus\", \"default_memory\"]
DEFENABLEANNOTATIONS_COCO := [\"enable_iommu\", \"virtio_fs_extra_args\", \"kernel_params\", \"kernel_verity_params\", \"default_vcpus\", \"default_memory\", \"cc_init_data\"]
DEFDISABLEGUESTSECCOMP := true
DEFDISABLEGUESTEMPTYDIR := false
##VAR DEFAULTEXPFEATURES=[features] Default experimental features enabled
DEFAULTEXPFEATURES := []
DEFDISABLESELINUX := false
##VAR DEFENTROPYSOURCE=[entropy_source] Default entropy source
DEFENTROPYSOURCE := /dev/urandom
DEFVALIDENTROPYSOURCES := [\"/dev/urandom\",\"/dev/random\",\"\"]
DEFDISABLEBLOCK := false
DEFSHAREDFS_CLH_VIRTIOFS := virtio-fs
DEFVIRTIOFSDAEMON := $(LIBEXECDIR)/virtiofsd
DEFVALIDVIRTIOFSDAEMONPATHS := [\"$(DEFVIRTIOFSDAEMON)\"]
##VAR DEFVIRTIOFSCACHESIZE=<cache_size> Default DAX mapping cache size in MiB
# if value is 0, DAX is not enabled
DEFVIRTIOFSCACHESIZE ?= 0
DEFVIRTIOFSCACHE ?= auto
DEFVIRTIOFSQUEUESIZE ?= 1024
# Format example:
#   [\"-o\", \"arg1=xxx,arg2\", \"-o\", \"hello world\", \"--arg3=yyy\"]
#
# see `virtiofsd -h` for possible options.
# Make sure you quote args.
DEFVIRTIOFSEXTRAARGS ?= [\"--thread-pool-size=1\", \"-o\", \"announce_submounts\"]
DEFENABLEIOTHREADS := false
DEFENABLEVHOSTUSERSTORE := false
DEFVHOSTUSERSTOREPATH := $(PKGRUNDIR)/vhost-user
DEFVALIDVHOSTUSERSTOREPATHS := [\"$(DEFVHOSTUSERSTOREPATH)\"]
DEFFILEMEMBACKEND := ""
DEFVALIDFILEMEMBACKENDS := [\"$(DEFFILEMEMBACKEND)\"]
DEFMSIZE9P := 8192
DEFVFIOMODE := guest-kernel
DEFBINDMOUNTS := []
DEFDANCONF := /run/kata-containers/dans
DEFFORCEGUESTPULL := false
QEMUTDXQUOTEGENERATIONSERVICESOCKETPORT := 4050

# Create Container Timeout in seconds
DEFCREATECONTAINERTIMEOUT ?= 30
DEFCREATECONTAINERTIMEOUT_COCO ?= 60
DEFSTATICRESOURCEMGMT_COCO = true
DEFDISABLEIMAGENVDIMM ?= false

SED = sed
CLI_DIR = cmd
SHIMV2 = containerd-shim-kata-v2
SHIMV2_OUTPUT = $(CURDIR)/$(SHIMV2)
SHIMV2_DIR = $(CLI_DIR)/$(SHIMV2)
MONITOR = kata-monitor
MONITOR_OUTPUT = $(CURDIR)/$(MONITOR)
MONITOR_DIR = $(CLI_DIR)/kata-monitor
SOURCES := $(shell find . 2>&1 | grep -E '.*\.(c|h|go)$$')
VERSION := ${shell cat ./VERSION}

# List of configuration files to build and install
CONFIGS =
CONFIG_PATHS =
SYSCONFIG_PATHS =
# List of hypervisors known for the current architecture
KNOWN_HYPERVISORS =

CONFDIR := $(DEFAULTSDIR)/$(PROJECT_DIR)/runtime-rs
SYSCONFDIR := $(SYSCONFDIR)/$(PROJECT_DIR)
##VAR CONFIG_PATH=<path> Main configuration file location for stateless systems
CONFIG_PATH := $(abspath $(CONFDIR)/$(CONFIG_FILE))
##VAR SYSCONFIG=<path> Secondary configuration file location. Note that this takes precedence
# over CONFIG_PATH.
SYSCONFIG := $(abspath $(SYSCONFDIR)/$(CONFIG_FILE))
SHAREDIR := $(SHAREDIR)

ifneq (,$(DBCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_DB)
    CONFIG_FILE_DB = configuration-dragonball.toml
    CONFIG_DB = config/$(CONFIG_FILE_DB)
    CONFIG_DB_IN = $(CONFIG_DB).in
    CONFIG_PATH_DB = $(abspath $(CONFDIR)/$(CONFIG_FILE_DB))
    CONFIG_PATHS += $(CONFIG_PATH_DB)
    SYSCONFIG_DB = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_DB))
    SYSCONFIG_PATHS += $(SYSCONFIG_DB)
    CONFIGS += $(CONFIG_DB)
    # dragonball-specific options (all should be suffixed by "_DB")
    VMROOTFSDRIVER_DB := virtio-blk-pci
    DEFMAXVCPUS_DB := 0
    DEFBLOCKSTORAGEDRIVER_DB := virtio-blk-pci
    DEFNETWORKMODEL_DB := tcfilter
    KERNELPARAMS_DB = console=ttyS1 agent.log_vport=1025
    KERNELTYPE_DB = uncompressed
    KERNEL_NAME_DB = $(call MAKE_KERNEL_NAME_DB,$(KERNELTYPE_DB))
    KERNELPATH_DB = $(KERNELDIR)/$(KERNEL_NAME_DB)
    DEFSANDBOXCGROUPONLY_DB := true
    DEFSTATICRESOURCEMGMT_DB := false
    RUNTIMENAME := virt_container
    PIPESIZE := 1
    DBSHAREDFS := inline-virtio-fs
    DEF_DGB_BRIDGES := 0
endif

ifneq (,$(CLHCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_CLH)

    CONFIG_FILE_CLH = configuration-cloud-hypervisor.toml
    CONFIG_CLH = config/$(CONFIG_FILE_CLH)
    CONFIG_CLH_IN = $(CONFIG_CLH).in

    CONFIG_PATH_CLH = $(abspath $(CONFDIR)/$(CONFIG_FILE_CLH))
    CONFIG_PATHS += $(CONFIG_PATH_CLH)

    SYSCONFIG_CLH = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_CLH))
    SYSCONFIG_PATHS += $(SYSCONFIG_CLH)

    CONFIGS += $(CONFIG_CLH)

    # CLH-specific options (all should be suffixed by "_CLH")
    # currently, huge pages are required for virtiofsd support
    DEFNETWORKMODEL_CLH := tcfilter
    KERNELTYPE_CLH = uncompressed
    KERNEL_NAME_CLH = $(call MAKE_KERNEL_NAME,$(KERNELTYPE_CLH))
    KERNELPATH_CLH = $(KERNELDIR)/$(KERNEL_NAME_CLH)
    VMROOTFSDRIVER_CLH := virtio-blk-pci

    DEFSANDBOXCGROUPONLY_CLH := true
    DEFSTATICRESOURCEMGMT_CLH := false
endif

ifneq (,$(QEMUCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_QEMU)

    CONFIG_FILE_QEMU = configuration-qemu-runtime-rs.toml
    CONFIG_QEMU = config/$(CONFIG_FILE_QEMU)
    CONFIG_QEMU_IN = $(CONFIG_QEMU).in

    QEMUPATH := $(QEMUBINDIR)/$(QEMUCMD)
    QEMUVALIDHYPERVISORPATHS := [\"$(QEMUPATH)\"]

ifneq (,$(QEMUSNPCMD))
    QEMUSNPPATH := $(QEMUBINDIR)/$(QEMUSNPCMD)
    QEMUSNPVALIDHYPERVISORPATHS := [\"$(QEMUSNPPATH)\"]
endif

    CONFIG_PATH_QEMU = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU)

    SYSCONFIG_QEMU = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU)

    CONFIGS += $(CONFIG_QEMU)

    CONFIG_FILE_QEMU_TDX = configuration-qemu-tdx-runtime-rs.toml
    CONFIG_QEMU_TDX = config/$(CONFIG_FILE_QEMU_TDX)
    CONFIG_QEMU_TDX_IN = $(CONFIG_QEMU_TDX).in

    CONFIG_PATH_QEMU_TDX = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_TDX))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_TDX)

    SYSCONFIG_QEMU_TDX = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_TDX))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU_TDX)

    CONFIGS += $(CONFIG_QEMU_TDX)

    CONFIG_FILE_QEMU_SNP = configuration-qemu-snp-runtime-rs.toml
    CONFIG_QEMU_SNP = config/$(CONFIG_FILE_QEMU_SNP)
    CONFIG_QEMU_SNP_IN = $(CONFIG_QEMU_SNP).in

    CONFIG_PATH_QEMU_SNP = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_SNP))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_SNP)

    SYSCONFIG_QEMU_SNP = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_SNP))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU_SNP)

    CONFIGS += $(CONFIG_QEMU_SNP)

    CONFIG_FILE_QEMU_SE = configuration-qemu-se-runtime-rs.toml
    CONFIG_QEMU_SE = config/$(CONFIG_FILE_QEMU_SE)
    CONFIG_QEMU_SE_IN = $(CONFIG_QEMU_SE).in

    CONFIG_PATH_QEMU_SE = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_SE))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_SE)

    SYSCONFIG_QEMU_SE = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_SE))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU_SE)
    DEFVFIOMODE_SE = vfio

    CONFIGS += $(CONFIG_QEMU_SE)

    CONFIG_FILE_QEMU_COCO_DEV = configuration-qemu-coco-dev-runtime-rs.toml
    CONFIG_QEMU_COCO_DEV = config/$(CONFIG_FILE_QEMU_COCO_DEV)
    CONFIG_QEMU_COCO_DEV_IN = $(CONFIG_QEMU_COCO_DEV).in

    CONFIG_PATH_QEMU_COCO_DEV = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_COCO_DEV))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_COCO_DEV)

    SYSCONFIG_QEMU_COCO_DEV = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_COCO_DEV))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU_COCO_DEV)

    CONFIGS += $(CONFIG_QEMU_COCO_DEV)

    CONFIG_FILE_QEMU_NVIDIA_GPU = configuration-qemu-nvidia-gpu-runtime-rs.toml
    CONFIG_QEMU_NVIDIA_GPU = config/$(CONFIG_FILE_QEMU_NVIDIA_GPU)
    CONFIG_QEMU_NVIDIA_GPU_IN = $(CONFIG_QEMU_NVIDIA_GPU).in

    CONFIG_PATH_QEMU_NVIDIA_GPU = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_NVIDIA_GPU))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_NVIDIA_GPU)

    SYSCONFIG_QEMU_NVIDIA_GPU = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_NVIDIA_GPU))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU_NVIDIA_GPU)

    CONFIGS += $(CONFIG_QEMU_NVIDIA_GPU)

ifneq (,$(QEMUSNPCMD))
    CONFIG_FILE_QEMU_NVIDIA_GPU_SNP = configuration-qemu-nvidia-gpu-snp-runtime-rs.toml
    CONFIG_QEMU_NVIDIA_GPU_SNP = config/$(CONFIG_FILE_QEMU_NVIDIA_GPU_SNP)
    CONFIG_QEMU_NVIDIA_GPU_SNP_IN = $(CONFIG_QEMU_NVIDIA_GPU_SNP).in

    CONFIG_PATH_QEMU_NVIDIA_GPU_SNP = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_NVIDIA_GPU_SNP))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_NVIDIA_GPU_SNP)

    SYSCONFIG_QEMU_NVIDIA_GPU_SNP = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_NVIDIA_GPU_SNP))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU_NVIDIA_GPU_SNP)

    CONFIGS += $(CONFIG_QEMU_NVIDIA_GPU_SNP)
endif

    KERNELTYPE_QEMU = uncompressed
    KERNEL_NAME_QEMU = $(call MAKE_KERNEL_NAME,$(KERNELTYPE_QEMU))
    KERNELPATH_QEMU = $(KERNELDIR)/$(KERNEL_NAME_QEMU)

    # NVIDIA GPU reference stack (parity with Go configuration-qemu-nvidia-gpu.toml.in)
    KERNELTYPE_NV = compressed
    KERNELNAME_NV = $(call MAKE_KERNEL_NAME_NV,$(KERNELTYPE_NV))
    KERNELPATH_NV = $(KERNELDIR)/$(KERNELNAME_NV)
    IMAGENAME_NV = $(PROJECT_TAG)-nvidia-gpu.img
    IMAGEPATH_NV = $(PKGDATADIR)/$(IMAGENAME_NV)
    KERNELPARAMS_NV := cgroup_no_v1=all pci=realloc pci=nocrs pci=assign-busses

    KERNELPARAMS_CONFIDENTIAL_NV = $(KERNELPARAMS_NV)
    KERNELPARAMS_CONFIDENTIAL_NV += "nvrc.smi.srs=1"

    KERNELVERITYPARAMS_NV ?=
    DEFAULTVCPUS_NV := 1
    DEFAULTMEMORY_NV := 8192
    DEFAULTTIMEOUT_NV := 1200
    DEFAULTLAUNCHPROCESSTIMEOUT_NV := 15
    DEFAULTPCIEROOTPORT_NV := 8
    DEFDISABLEIMAGENVDIMM_NV := true
    DEFSANDBOXCGROUPONLY_NV := true
    DEFENABLEVCPUSPINNING_NV := true
    DEFSTATICRESOURCEMGMT_NV := true
    DEFVFIOMODE_NV := guest-kernel
    DEFKUBELETROOTDIR := /var/lib/kubelet
    DEFPODRESOURCEAPISOCK_NV := "$(DEFKUBELETROOTDIR)/pod-resources/kubelet.sock"
    # NVIDIA profile: rootfs block driver (avoid virtio-pmem/DAX for this stack)
    VMROOTFSDRIVER_NV := virtio-blk-pci
    # Agent client: hybrid vsock retry delay (ms); vsock path uses this as initial backoff (clamped in agent)
    DEFDIALTIMEOUTMS_NV := 1000
    # Go qemu-nvidia-gpu dial_timeout is 1200s; runtime-rs uses ms for reconnect_timeout_ms
    DEFRECONNECTTIMEOUTMS_NV := 1200000

    # NVIDIA GPU confidential (SNP) reference stack
    IMAGENAME_CONFIDENTIAL_NV = $(PROJECT_TAG)-nvidia-gpu-confidential.img
    IMAGEPATH_CONFIDENTIAL_NV = $(PKGDATADIR)/$(IMAGENAME_CONFIDENTIAL_NV)
    KERNELNAME_CONFIDENTIAL_NV = $(call MAKE_KERNEL_NAME_NV,compressed)
    KERNELPATH_CONFIDENTIAL_NV = $(KERNELDIR)/$(KERNELNAME_CONFIDENTIAL_NV)
    KERNELVERITYPARAMS_CONFIDENTIAL_NV ?=
    FIRMWARESNPPATH_NV := $(FIRMWARE_SNP_PATH)

    KERNEL_NAME_QEMU_SE = kata-containers-se.img
    KERNELPATH_QEMU_SE = $(KERNELDIR)/$(KERNEL_NAME_QEMU_SE)

    KERNEL_TYPE_COCO = compressed
    KERNEL_NAME_COCO = $(call MAKE_KERNEL_NAME,$(KERNEL_TYPE_COCO))
    KERNELPATH_COCO = $(KERNELDIR)/$(KERNEL_NAME_COCO)

    # overriding options
    DEFSTATICRESOURCEMGMT_QEMU := false

    # qemu-specific options
    DEFSANDBOXCGROUPONLY_QEMU := false
    DEFENABLEVCPUSPINNING_QEMU := false
ifeq ($(ARCH), s390x)
    VMROOTFSDRIVER_QEMU := virtio-blk-ccw
    DEFBLOCKSTORAGEDRIVER_QEMU := virtio-blk-ccw
    # runtime-rs memory hotplug hard-codes pc-dimm, which is x86-only.
    # s390x (s390-ccw-virtio) requires virtio-mem-ccw instead. Use static
    # sandbox resource management to size the VM upfront and bypass hotplug.
    DEFSTATICRESOURCEMGMT_QEMU := true
else ifeq ($(ARCH), aarch64)
    # NVDIMM/virtio-pmem has issues on arm64 (cache coherency problems with DAX),
    # so we use virtio-blk-pci instead.
    VMROOTFSDRIVER_QEMU := virtio-blk-pci
    DEFBLOCKSTORAGEDRIVER_QEMU := virtio-scsi
    # runtime-rs lacks ARM CPU hotplug support (pflash/UEFI, SMP topology,
    # QMP vCPU add for virt machine type). Use static sandbox resource
    # management to size the VM upfront instead.
    DEFSTATICRESOURCEMGMT_QEMU := true
else
    VMROOTFSDRIVER_QEMU := virtio-pmem
    DEFBLOCKSTORAGEDRIVER_QEMU := virtio-scsi
endif
    DEFVCPUS_QEMU := 1
    DEFMAXVCPUS_QEMU := 0
    DEFSHAREDFS_QEMU_VIRTIOFS := virtio-fs
    DEFSHAREDFS_QEMU_SEL_VIRTIOFS := none
    DEFSHAREDFS_QEMU_COCO_DEV_VIRTIOFS := none
    DEFBLOCKDEVICEAIO_QEMU := io_uring
    DEFNETWORKMODEL_QEMU := tcfilter
    DEFDISABLEGUESTSELINUX := true
    # Default is empty string "" to match Rust default None (when commented out in config).
    # Most users will want to set this to "on,obsolete=deny,spawn=deny,resourcecontrol=deny"
    # for better security. Note: "elevateprivileges=deny" doesn't work with daemonize option.
    DEFSECCOMPSANDBOXPARAM := ""
endif

ifneq (,$(FCCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_FC)
    CONFIG_FILE_FC = configuration-rs-fc.toml
    CONFIG_FC = config/$(CONFIG_FILE_FC)
    CONFIG_FC_IN = $(CONFIG_FC).in
    CONFIG_PATH_FC = $(abspath $(CONFDIR)/$(CONFIG_FILE_FC))
    CONFIG_PATHS += $(CONFIG_PATH_FC)
    SYSCONFIG_FC = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_FC))
    SYSCONFIG_PATHS += $(SYSCONFIG_FC)
    CONFIGS += $(CONFIG_FC)
    # firecracker-specific options (all should be suffixed by "_FC")
    DEFBLOCKSTORAGEDRIVER_FC := virtio-blk-mmio
    DEFMAXMEMSZ_FC := 2048
    DEFNETWORKMODEL_FC := tcfilter
    KERNELPARAMS_FC = console=ttyS0 agent.log_vport=1025
    KERNELTYPE_FC = uncompressed
    KERNEL_NAME_FC = $(call MAKE_KERNEL_NAME_FC,$(KERNELTYPE_FC))
    KERNELPATH_FC = $(KERNELDIR)/$(KERNEL_NAME_FC)
    DEFSANDBOXCGROUPONLY_FC := true
    RUNTIMENAME := virt_container
    DEFSTATICRESOURCEMGMT_FC := true
endif

ifneq (,$(REMOTE))
    KNOWN_HYPERVISORS += $(HYPERVISOR_REMOTE)
    CONFIG_FILE_REMOTE = configuration-remote.toml
    CONFIG_REMOTE = config/$(CONFIG_FILE_REMOTE)
    CONFIG_REMOTE_IN = $(CONFIG_REMOTE).in
    CONFIG_PATH_REMOTE = $(abspath $(CONFDIR)/$(CONFIG_FILE_REMOTE))
    CONFIG_PATHS += $(CONFIG_PATH_REMOTE)
    SYSCONFDIR_REMOTE = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_REMOTE))
    SYSCONFIG_PATHS += $(SYSCONFDIR_REMOTE)
    CONFIGS += $(CONFIG_REMOTE)
    # remote-specific options (all should be suffixed by "_REMOTE")
    DEFSANDBOXCGROUPONLY_REMOTE := false
endif

ifeq ($(HYPERVISOR),$(HYPERVISOR_DB))
    DEFAULT_HYPERVISOR_CONFIG = $(CONFIG_FILE_DB)
endif

ifeq ($(HYPERVISOR),$(HYPERVISOR_QEMU))
    DEFAULT_HYPERVISOR_CONFIG = $(CONFIG_FILE_QEMU)
endif
ifeq ($(HYPERVISOR),$(HYPERVISOR_FC))
    DEFAULT_HYPERVISOR_CONFIG = $(CONFIG_FILE_FC)
endif
ifeq ($(DEFAULT_HYPERVISOR),$(HYPERVISOR_REMOTE))
    DEFAULT_HYPERVISOR_CONFIG = $(CONFIG_FILE_REMOTE)
endif
# list of variables the user may wish to override
USER_VARS += ARCH
USER_VARS += BINDIR
USER_VARS += CONFIG_DB_IN
USER_VARS += CONFIG_FC_IN
USER_VARS += CONFIG_PATH
USER_VARS += CONFIG_QEMU_IN
USER_VARS += CONFIG_QEMU_SE_IN
USER_VARS += CONFIG_REMOTE_IN
USER_VARS += CONFIG_QEMU_COCO_DEV_IN
USER_VARS += CONFIG_QEMU_NVIDIA_GPU_IN
USER_VARS += DESTDIR
USER_VARS += HYPERVISOR
USER_VARS += USE_BUILTIN_DB
USER_VARS += DBCMD
USER_VARS += DBCTLCMD
USER_VARS += FCCTLCMD
USER_VARS += DBPATH
USER_VARS += DBVALIDHYPERVISORPATHS
USER_VARS += DBCTLPATH
USER_VARS += DBVALIDCTLPATHS
USER_VARS += CLHPATH
USER_VARS += CLHVALIDHYPERVISORPATHS
USER_VARS += QEMUPATH
USER_VARS += QEMUVALIDHYPERVISORPATHS
USER_VARS += QEMUSNPCMD
USER_VARS += QEMUSNPPATH
USER_VARS += QEMUSNPVALIDHYPERVISORPATHS
USER_VARS += FIRMWAREPATH_CLH
USER_VARS += KERNELPATH_CLH
USER_VARS += FCCMD
USER_VARS += FCPATH
USER_VARS += FCVALIDHYPERVISORPATHS
USER_VARS += FCJAILERPATH
USER_VARS += FCVALIDJAILERPATHS
USER_VARS += FCVALIDJAILERPATHS
USER_VARS += DEFMAXMEMSZ_FC
USER_VARS += SYSCONFIG
USER_VARS += IMAGENAME
USER_VARS += IMAGENAME_NV
USER_VARS += IMAGECONFIDENTIALNAME
USER_VARS += IMAGEPATH
USER_VARS += IMAGEPATH_NV
USER_VARS += IMAGECONFIDENTIALPATH
USER_VARS += INITRDNAME
USER_VARS += INITRDCONFIDENTIALNAME
USER_VARS += INITRDPATH
USER_VARS += INITRDCONFIDENTIALPATH
USER_VARS += DEFROOTFSTYPE
USER_VARS += VMROOTFSDRIVER_DB
USER_VARS += VMROOTFSDRIVER_CLH
USER_VARS += VMROOTFSDRIVER_QEMU
USER_VARS += MACHINETYPE
USER_VARS += KERNELDIR
USER_VARS += KERNELTYPE
USER_VARS += KERNELPATH_DB
USER_VARS += KERNELPATH_QEMU
USER_VARS += KERNELPATH_NV
USER_VARS += KERNELPATH_QEMU_SE
USER_VARS += KERNELPATH_FC
USER_VARS += KERNELPATH_COCO
USER_VARS += KERNELPATH
USER_VARS += KERNELVIRTIOFSPATH
USER_VARS += FIRMWAREPATH
USER_VARS += FIRMWAREPATH_NV
USER_VARS += FIRMWAREVOLUMEPATH
USER_VARS += MACHINEACCELERATORS
USER_VARS += CPUFEATURES
USER_VARS += DEFMACHINETYPE_CLH
USER_VARS += KERNELPARAMS
USER_VARS += KERNELVERITYPARAMS
USER_VARS += KERNELPARAMS_DB
USER_VARS += KERNELPARAMS_FC
USER_VARS += LIBEXECDIR
USER_VARS += LOCALSTATEDIR
USER_VARS += PKGDATADIR
USER_VARS += PKGLIBEXECDIR
USER_VARS += PKGRUNDIR
USER_VARS += PREFIX
USER_VARS += PROJECT_BUG_URL
USER_VARS += PROJECT_NAME
USER_VARS += PROJECT_ORG
USER_VARS += PROJECT_PREFIX
USER_VARS += PROJECT_TAG
USER_VARS += PROJECT_TYPE
USER_VARS += RUNTIME_NAME
USER_VARS += SHAREDIR
USER_VARS += SYSCONFDIR
USER_VARS += DEFVCPUS
USER_VARS += DEFVCPUS_QEMU
USER_VARS += DEFMAXVCPUS
USER_VARS += DEFMAXVCPUS_DB
USER_VARS += DEFMAXVCPUS_QEMU
USER_VARS += DEFMEMSZ
USER_VARS += DEFMEMSLOTS
USER_VARS += DEFMAXMEMSZ
USER_VARS += DEFBRIDGES
USER_VARS += DEFNETQUEUES
USER_VARS += DEFNETWORKMODEL_DB
USER_VARS += DEFNETWORKMODEL_CLH
USER_VARS += DEFNETWORKMODEL_QEMU
USER_VARS += DEFNETWORKMODEL_FC
USER_VARS += DEFDISABLEGUESTEMPTYDIR
USER_VARS += DEFDISABLEGUESTSECCOMP
USER_VARS += DEFDISABLESELINUX
USER_VARS += DEFDISABLEGUESTSELINUX
USER_VARS += DEFAULTEXPFEATURES
USER_VARS += DEFDISABLEBLOCK
USER_VARS += DEFBLOCKSTORAGEDRIVER_DB
USER_VARS += DEFBLOCKSTORAGEDRIVER_QEMU
USER_VARS += DEFBLOCKDEVICEAIO_QEMU
USER_VARS += DEFBLOCKSTORAGEDRIVER_FC
USER_VARS += DEFSHAREDFS_CLH_VIRTIOFS
USER_VARS += DEFSHAREDFS_QEMU_VIRTIOFS
USER_VARS += DEFSHAREDFS_QEMU_SEL_VIRTIOFS
USER_VARS += DEFSHAREDFS_QEMU_COCO_DEV_VIRTIOFS
USER_VARS += DEFVIRTIOFSDAEMON
USER_VARS += DEFVALIDVIRTIOFSDAEMONPATHS
USER_VARS += DEFVIRTIOFSCACHESIZE
USER_VARS += DEFVIRTIOFSCACHE
USER_VARS += DEFVIRTIOFSQUEUESIZE
USER_VARS += DEFVIRTIOFSEXTRAARGS
USER_VARS += DEFENABLEANNOTATIONS
USER_VARS += DEFENABLEANNOTATIONS_COCO
USER_VARS += DEFENABLEIOTHREADS
USER_VARS += DEFSECCOMPSANDBOXPARAM
USER_VARS += DEFENABLEVHOSTUSERSTORE
USER_VARS += DEFVHOSTUSERSTOREPATH
USER_VARS += DEFVALIDVHOSTUSERSTOREPATHS
USER_VARS += DEFFILEMEMBACKEND
USER_VARS += DEFVALIDFILEMEMBACKENDS
USER_VARS += DEFMSIZE9P
USER_VARS += DEFENTROPYSOURCE
USER_VARS += DEFVALIDENTROPYSOURCES
USER_VARS += DEFSANDBOXCGROUPONLY_QEMU
USER_VARS += DEFSANDBOXCGROUPONLY_DB
USER_VARS += DEFSANDBOXCGROUPONLY_FC
USER_VARS += DEFSANDBOXCGROUPONLY_CLH
USER_VARS += DEFSANDBOXCGROUPONLY_REMOTE
USER_VARS += DEFENABLEVCPUSPINNING_QEMU
USER_VARS += DEFSTATICRESOURCEMGMT_DB
USER_VARS += DEFSTATICRESOURCEMGMT_FC
USER_VARS += DEFSTATICRESOURCEMGMT_CLH
USER_VARS += DEFSTATICRESOURCEMGMT_QEMU
USER_VARS += DEFSTATICRESOURCEMGMT_COCO
USER_VARS += DEFDISABLEIMAGENVDIMM
USER_VARS += DEFBINDMOUNTS
USER_VARS += DEFVFIOMODE
USER_VARS += DEFVFIOMODE_SE
USER_VARS += BUILDFLAGS
USER_VARS += RUNTIMENAME
USER_VARS += HYPERVISOR_DB
USER_VARS += HYPERVISOR_CLH
USER_VARS += HYPERVISOR_QEMU
USER_VARS += HYPERVISOR_FC
USER_VARS += PIPESIZE
USER_VARS += DBSHAREDFS
USER_VARS += DEF_DGB_BRIDGES
USER_VARS += KATA_INSTALL_GROUP
USER_VARS += KATA_INSTALL_OWNER
USER_VARS += KATA_INSTALL_CFG_PERMS
USER_VARS += DEFDANCONF
USER_VARS += DEFFORCEGUESTPULL
USER_VARS += QEMUTDXQUOTEGENERATIONSERVICESOCKETPORT
USER_VARS += DEFCREATECONTAINERTIMEOUT
USER_VARS += DEFCREATECONTAINERTIMEOUT_COCO
USER_VARS += QEMUTDXEXPERIMENTALCMD
USER_VARS += FIRMWARE_SNP_PATH
USER_VARS += KERNELTDXPARAMS
USER_VARS += DEFSHAREDFS_QEMU_TDX_VIRTIOFS
USER_VARS += FIRMWARETDVFPATH
USER_VARS += KERNELPARAMS_NV
USER_VARS += KERNELPARAMS_CONFIDENTIAL_NV
USER_VARS += KERNELVERITYPARAMS_NV
USER_VARS += DEFAULTVCPUS_NV
USER_VARS += DEFAULTMEMORY_NV
USER_VARS += DEFAULTTIMEOUT_NV
USER_VARS += DEFAULTLAUNCHPROCESSTIMEOUT_NV
USER_VARS += DEFAULTPCIEROOTPORT_NV
USER_VARS += DEFDISABLEIMAGENVDIMM_NV
USER_VARS += DEFSANDBOXCGROUPONLY_NV
USER_VARS += DEFENABLEVCPUSPINNING_NV
USER_VARS += DEFSTATICRESOURCEMGMT_NV
USER_VARS += DEFVFIOMODE_NV
USER_VARS += DEFKUBELETROOTDIR
USER_VARS += DEFPODRESOURCEAPISOCK_NV
USER_VARS += VMROOTFSDRIVER_NV
USER_VARS += DEFDIALTIMEOUTMS_NV
USER_VARS += DEFRECONNECTTIMEOUTMS_NV
USER_VARS += IMAGENAME_CONFIDENTIAL_NV
USER_VARS += IMAGEPATH_CONFIDENTIAL_NV
USER_VARS += KERNELNAME_CONFIDENTIAL_NV
USER_VARS += KERNELPATH_CONFIDENTIAL_NV
USER_VARS += KERNELVERITYPARAMS_CONFIDENTIAL_NV
USER_VARS += FIRMWARESNPPATH_NV
USER_VARS += CONFIG_QEMU_NVIDIA_GPU_SNP_IN

SOURCES := \
  $(shell find . 2>&1 | grep -E '.*\.rs$$') \
  Cargo.toml

VERSION_FILE := ./VERSION
VERSION := $(shell grep -v ^\# $(VERSION_FILE) 2>/dev/null || echo "unknown")
COMMIT_NO := $(shell git rev-parse HEAD 2>/dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no 2>/dev/null || true),${COMMIT_NO}-dirty,${COMMIT_NO})
COMMIT_MSG = $(if $(COMMIT),$(COMMIT),unknown)

EXTRA_RUSTFEATURES :=

# if use dragonball hypervisor, add the feature to build dragonball in runtime
ifeq ($(USE_BUILTIN_DB),true)
    EXTRA_RUSTFEATURES +=  dragonball
endif

ifneq ($(EXTRA_RUSTFEATURES),)
    override EXTRA_RUSTFEATURES := --features $(EXTRA_RUSTFEATURES)
endif


TARGET_PATH = ../../target/$(TRIPLE)/$(BUILD_TYPE)/$(TARGET)

##VAR DESTDIR=<path> is a directory prepended to each installed target file
DESTDIR ?= /

GENERATED_CODE = crates/shim/src/config.rs

RUNTIME_NAME=$(TARGET)
RUNTIME_VERSION=$(VERSION)

GENERATED_VARS = \
		VERSION \
		CONFIG_DB_IN \
		CONFIG_FC_IN \
		CONFIG_QEMU_TDX_IN \
		CONFIG_QEMU_SNP_IN \
		CONFIG_QEMU_NVIDIA_GPU_IN \
		CONFIG_QEMU_NVIDIA_GPU_SNP_IN \
		$(USER_VARS)


GENERATED_REPLACEMENTS= \
    PROJECT_NAME \
    RUNTIME_NAME \
    CONTAINERD_RUNTIME_NAME \
    RUNTIME_VERSION \
    BINDIR \
    COMMIT

GENERATED_FILES :=

GENERATED_FILES += $(GENERATED_CODE)

# Display name of command and it's version (or a message if not available).
#
# Arguments:
#
# 1: Name of command
define get_command_version
$(shell printf "%s: %s\\n" $(1) "$(or $(shell $(1) --version 2>/dev/null), (not available))")
endef

define get_toolchain_version
$(shell printf "%s: %s\\n" "toolchain" "$(or $(shell rustup show active-toolchain 2>/dev/null), (unknown))")
endef

# Install a configuration file
# params:
# $1 : file to install
# $2 : directory path where file will be installed
define INSTALL_FILE
	install --mode 0644 -D $1 $(DESTDIR)$2/$(notdir $1);
endef

# Returns the name of the kernel file to use based on the provided KERNELTYPE.
# $1 : KERNELTYPE (compressed or uncompressed)
define MAKE_KERNEL_NAME_DB
$(if $(findstring uncompressed,$1),vmlinux-dragonball-experimental.container,vmlinuz-dragonball-experimental.container)
endef
define MAKE_KERNEL_NAME_FC
$(if $(findstring uncompressed,$1),vmlinux.container,vmlinuz.container)
endef

# Returns the name of the kernel file to use based on the provided KERNELTYPE.
#   # $1 : KERNELTYPE (compressed or uncompressed)
define MAKE_KERNEL_NAME
$(if $(findstring uncompressed,$1),vmlinux.container,vmlinuz.container)
endef

define MAKE_KERNEL_NAME_NV
$(if $(findstring uncompressed,$1),vmlinux-nvidia-gpu.container,vmlinuz-nvidia-gpu.container)
endef

.DEFAULT_GOAL := default

GENERATED_FILES += $(CONFIGS)

runtime: $(TARGET)

static-checks-build: $(GENERATED_FILES)

$(TARGET): $(GENERATED_FILES) $(TARGET_PATH)

$(TARGET_PATH): $(SOURCES) | show-summary
	@$(RELEASE_CARGO_PROFILE_ENV) RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build -p runtime-rs --target $(TRIPLE) $(if $(findstring release,$(BUILD_TYPE)),--release) $(EXTRA_RUSTFEATURES)

$(GENERATED_FILES): %: %.in
	@sed \
        $(foreach r,$(GENERATED_REPLACEMENTS),-e 's|@$r@|$($r)|g') \
        $(foreach v,$(GENERATED_VARS),-e "s|@$v@|$($v)|g") \
        $< > $@

##TARGET optimize: optimized  build
optimize: $(SOURCES) | show-summary show-header
	@$(RELEASE_CARGO_PROFILE_ENV) RUSTFLAGS="-C link-arg=-s $(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) --$(BUILD_TYPE) $(EXTRA_RUSTFEATURES)

##TARGET clean: clean build
clean: clean-generated-files
	@cargo clean
	@rm -f tarpaulin-report.html
	@rm -f $(CONFIGS)

##TARGET clean-generated-files: clean generated files
clean-generated-files:
	@rm -f $(GENERATED_FILES)

vendor:
	@cargo vendor

ifeq ($(ARCH),x86_64)
##TARGET check: run test
check: $(GENERATED_FILES) standard_rust_check
else
check:
	@echo "$(ARCH) is not currently supported"
	exit 0
endif

##TARGET run: build and run agent
run:
	@cargo run -p runtime-rs --target $(TRIPLE)

show-header:
	@printf "%s - version %s (commit %s)\n\n" "$(TARGET)" "$(VERSION)" "$(COMMIT_MSG)"

show-summary: show-header
	@printf "• Project:\n"
	@printf "  name: $(PROJECT_NAME)\n"
	@printf "  url: $(PROJECT_URL)\n"
	@printf "  component: $(PROJECT_COMPONENT)\n"
	@printf "\n"
	@printf "• Target: $(TARGET)\n"
	@printf "\n"
	@printf "• Architecture: $(ARCH)\n"
	@printf "\n"
	@printf "• Rust:\n"
	@printf "  %s\n" "$(call get_command_version,cargo)"
	@printf "  %s\n" "$(call get_command_version,rustc)"
	@printf "  %s\n" "$(call get_command_version,rustup)"
	@printf "  %s\n" "$(call get_toolchain_version)"
	@printf "\n"
	@printf "• Hypervisors:\n"
	@printf "\tDefault: $(HYPERVISOR)\n"
	@printf "\tKnown: $(sort $(HYPERVISORS))\n"
	@printf "\tAvailable for this architecture: $(sort $(KNOWN_HYPERVISORS))\n"
	@printf "\n"
	@printf "• Summary:\n"
	@printf "\n"
	@printf "\tdestination install path (DESTDIR) : %s\n" $(abspath $(DESTDIR))
	@printf "\tbinary installation path (BINDIR) : %s\n" $(abspath $(BINDIR))
	@printf "\tbinaries to install :\n"
	@printf \
          "$(foreach b,$(sort $(SHIMV2)),$(shell printf "\\t - $(shell readlink -m $(DESTDIR)/$(BINDIR)/$(b))\\\n"))"
	@printf "\tconfigs to install (CONFIGS) :\n"
	@printf \
	  "$(foreach c,$(sort $(CONFIGS)),$(shell printf "\\t - $(c)\\\n"))"
	@printf "\tinstall paths (CONFIG_PATHS) :\n"
	@printf \
	  "$(foreach c,$(sort $(CONFIG_PATHS)),$(shell printf "\\t - $(c)\\\n"))"
	@printf "\talternate config paths (SYSCONFIG_PATHS) : %s\n"
	@printf \
	  "$(foreach c,$(sort $(SYSCONFIG_PATHS)),$(shell printf "\\t - $(c)\\\n"))"
	@printf "\tdefault install path for $(HYPERVISOR) (CONFIG_PATH) : %s\n" $(abspath $(CONFIG_PATH))
	@printf "\tdefault alternate config path (SYSCONFIG) : %s\n" $(abspath $(SYSCONFIG))
ifneq (,$(findstring $(HYPERVISOR_QEMU),$(KNOWN_HYPERVISORS)))
	@printf "\t$(HYPERVISOR_QEMU) hypervisor path (QEMUPATH) : %s\n" $(abspath $(QEMUPATH))
endif
ifneq (,$(findstring $(HYPERVISOR_QEMU_VIRTIOFS),$(KNOWN_HYPERVISORS)))
	@printf "\t$(HYPERVISOR_QEMU_VIRTIOFS) hypervisor path (QEMUVIRTIOFSPATH) : %s\n" $(abspath $(QEMUVIRTIOFSPATH))
endif
ifneq (,$(findstring $(HYPERVISOR_CLH),$(KNOWN_HYPERVISORS)))
	@printf "\t$(HYPERVISOR_CLH) hypervisor path (CLHPATH) : %s\n" $(abspath $(CLHPATH))
endif
ifneq (,$(findstring $(HYPERVISOR_FC),$(KNOWN_HYPERVISORS)))
	@printf "\t$(HYPERVISOR_FC) hypervisor path (FCPATH) : %s\n" $(abspath $(FCPATH))
endif
	@printf "\tassets path (PKGDATADIR) : %s\n" $(abspath $(PKGDATADIR))
	@printf "\tshim path (PKGLIBEXECDIR) : %s\n" $(abspath $(PKGLIBEXECDIR))
	@printf "\n"
##TARGET help: Show help comments that start with `##VAR` and `##TARGET` in runtime-rs makefile
help: Makefile show-summary
	@echo "========================== Help ============================="
	@echo "Variables:"
	@sed -n 's/^##VAR//p' $< | sort
	@echo ""
	@echo "Targets:"
	@sed -n 's/^##TARGET//p' $< | sort

TARPAULIN_ARGS:=-v --workspace
install-tarpaulin:
	cargo install cargo-tarpaulin

# Check if cargo tarpaulin is installed
HAS_TARPAULIN:= $(shell cargo --list | grep tarpaulin 2>/dev/null)
check_tarpaulin:
ifndef  HAS_TARPAULIN
	$(error "tarpaulin is not available please: run make install-tarpaulin ")
else
	$(info OK: tarpaulin installed)
endif

##TARGET codecov: Generate code coverage report
codecov: check_tarpaulin
	cargo tarpaulin $(TARPAULIN_ARGS)

##TARGET codecov-html: Generate code coverage html report
codecov-html: check_tarpaulin
	cargo tarpaulin $(TARPAULIN_ARGS) -o Html

install-runtime: runtime
	install -D $(TARGET_PATH) $(DESTDIR)$(BINDIR)/$(notdir $(TARGET_PATH))

install-configs: $(CONFIGS)
	$(foreach f,$(CONFIGS),$(call INSTALL_FILE,$f,$(dir $(CONFIG_PATH)))) \
	ln -sf $(DEFAULT_HYPERVISOR_CONFIG) $(DESTDIR)/$(CONFIG_PATH)

.PHONY: \
	help \
	optimize \
	show-header \
	show-summary \
	vendor
