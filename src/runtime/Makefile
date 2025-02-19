#
# Copyright (c) 2018-2019 Intel Corporation
# Copyright (c) 2021 Adobe Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
include golang.mk

#Get ARCH.
ifeq ($(ARCH),)
    ifneq (,$(golang_version_raw))
        override ARCH = $(shell go env GOARCH)
    else
        override ARCH = $(shell uname -m)
    endif
endif
ifeq ($(ARCH),x86_64)
    override ARCH = amd64
endif
ifeq ($(ARCH),aarch64)
    override ARCH = arm64
endif

ARCH_DIR = arch
ARCH_FILE_SUFFIX = -options.mk
ARCH_FILE = $(ARCH_DIR)/$(ARCH)$(ARCH_FILE_SUFFIX)
ARCH_FILES = $(wildcard arch/*$(ARCH_FILE_SUFFIX))
ALL_ARCHES = $(patsubst $(ARCH_DIR)/%$(ARCH_FILE_SUFFIX),%,$(ARCH_FILES))

# Build as safely as possible
export CGO_CPPFLAGS = -D_FORTIFY_SOURCE=2 -fstack-protector

ifeq (,$(realpath $(ARCH_FILE)))
    $(error "ERROR: invalid architecture: '$(ARCH)'")
else
    # Load architecture-dependent settings
    include $(ARCH_FILE)
endif

PROJECT_TYPE = kata
PROJECT_NAME = Kata Containers
PROJECT_TAG = kata-containers
PROJECT_ORG = $(PROJECT_TAG)
PROJECT_URL = https://github.com/$(PROJECT_ORG)
PROJECT_BUG_URL = $(PROJECT_URL)/kata-containers/issues/new

# list of scripts to install
SCRIPTS :=

# list of binaries to install
BINLIST :=
BINLIBEXECLIST :=

BIN_PREFIX = $(PROJECT_TYPE)
PROJECT_DIR = $(PROJECT_TAG)
IMAGENAME = $(PROJECT_TAG).img
IMAGECONFIDENTIALNAME = $(PROJECT_TAG)-confidential.img
INITRDNAME = $(PROJECT_TAG)-initrd.img
INITRDCONFIDENTIALNAME = $(PROJECT_TAG)-initrd-confidential.img

IMAGENAME_NV = $(PROJECT_TAG)-nvidia-gpu.img
IMAGENAME_CONFIDENTIAL_NV = $(PROJECT_TAG)-nvidia-gpu-confidential.img
INITRDNAME_NV = $(PROJECT_TAG)-initrd-nvidia-gpu.img
INITRDNAME_CONFIDENTIAL_NV = $(PROJECT_TAG)-initrd-nvidia-gpu-confidential.img

TARGET = $(BIN_PREFIX)-runtime
RUNTIME_OUTPUT = $(CURDIR)/$(TARGET)
RUNTIME_DIR = $(CLI_DIR)/$(TARGET)
BINLIST += $(TARGET)

DESTDIR ?= /

ifeq ($(PREFIX),)
PREFIX        := /usr
EXEC_PREFIX   := $(PREFIX)/local
else
EXEC_PREFIX   := $(PREFIX)
endif
# Prefix where depedencies are installed
PREFIXDEPS    := $(PREFIX)
BINDIR        := $(EXEC_PREFIX)/bin
QEMUBINDIR    := $(PREFIXDEPS)/bin
CLHBINDIR    := $(PREFIXDEPS)/bin
FCBINDIR      := $(PREFIXDEPS)/bin
STRATOVIRTBINDIR := $(PREFIXDEPS)/bin
SYSCONFDIR    := /etc
LOCALSTATEDIR := /var

LIBEXECDIR := $(PREFIXDEPS)/libexec
SHAREDIR := $(PREFIX)/share
DEFAULTSDIR := $(SHAREDIR)/defaults

COLLECT_SCRIPT = data/kata-collect-data.sh

# @RUNTIME_NAME@ should be replaced with the target in generated files
RUNTIME_NAME = $(TARGET)

GENERATED_FILES += $(COLLECT_SCRIPT)
GENERATED_VARS = \
		VERSION \
		CONFIG_QEMU_IN \
        CONFIG_QEMU_COCO_DEV_IN \
		CONFIG_QEMU_NVIDIA_GPU_IN \
		CONFIG_QEMU_NVIDIA_GPU_SNP_IN \
		CONFIG_QEMU_NVIDIA_GPU_TDX_IN \
		CONFIG_QEMU_SEV_IN \
		CONFIG_QEMU_TDX_IN \
		CONFIG_QEMU_SNP_IN \
		CONFIG_CLH_IN \
		CONFIG_FC_IN \
		CONFIG_STRATOVIRT_IN \
		CONFIG_REMOTE_IN \
		CONFIG_QEMU_SE_IN \
		$(USER_VARS)
SCRIPTS += $(COLLECT_SCRIPT)
SCRIPTS_DIR := $(BINDIR)

BASH_COMPLETIONS := data/completions/bash/kata-runtime
BASH_COMPLETIONSDIR := $(SHAREDIR)/bash-completion/completions

PKGDATADIR := $(PREFIXDEPS)/share/$(PROJECT_DIR)
PKGRUNDIR := $(LOCALSTATEDIR)/run/$(PROJECT_DIR)
PKGLIBEXECDIR := $(LIBEXECDIR)/$(PROJECT_DIR)

KERNELDIR := $(PKGDATADIR)

IMAGEPATH := $(PKGDATADIR)/$(IMAGENAME)
IMAGECONFIDENTIALPATH := $(PKGDATADIR)/$(IMAGECONFIDENTIALNAME)
INITRDPATH := $(PKGDATADIR)/$(INITRDNAME)
INITRDCONFIDENTIALPATH := $(PKGDATADIR)/$(INITRDCONFIDENTIALNAME)

IMAGEPATH_NV := $(PKGDATADIR)/$(IMAGENAME_NV)
IMAGEPATH_CONFIDENTIAL_NV := $(PKGDATADIR)/$(IMAGENAME_CONFIDENTIAL_NV)
INITRDPATH_NV := $(PKGDATADIR)/$(INITRDNAME_NV)
INITRDPATH_CONFIDENTIAL_NV := $(PKGDATADIR)/$(INITRDNAME_CONFIDENTIAL_NV)

ROOTFSTYPE_EXT4 := \"ext4\"
ROOTFSTYPE_XFS := \"xfs\"
ROOTFSTYPE_EROFS := \"erofs\"
DEFROOTFSTYPE := $(ROOTFSTYPE_EXT4)

FIRMWAREPATH :=
FIRMWAREVOLUMEPATH :=

#FIRMWARETDVFPATH := $(PREFIXDEPS)/share/tdvf/OVMF.fd
FIRMWARETDVFPATH := PLACEHOLDER_FOR_DISTRO_OVMF_WITH_TDX_SUPPORT
FIRMWARETDVFVOLUMEPATH :=

FIRMWARESEVPATH := $(PREFIXDEPS)/share/ovmf/OVMF.fd
FIRMWARESNPPATH := $(PREFIXDEPS)/share/ovmf/AMDSEV.fd

ROOTMEASURECONFIG ?= ""
KERNELTDXPARAMS += $(ROOTMEASURECONFIG)
KERNELQEMUCOCODEVPARAMS += $(ROOTMEASURECONFIG)

# Name of default configuration file the runtime will use.
CONFIG_FILE = configuration.toml

HYPERVISOR_FC = firecracker
HYPERVISOR_QEMU = qemu
HYPERVISOR_CLH = cloud-hypervisor
HYPERVISOR_STRATOVIRT = stratovirt
HYPERVISOR_REMOTE = remote

# Determines which hypervisor is specified in $(CONFIG_FILE).
DEFAULT_HYPERVISOR ?= $(HYPERVISOR_QEMU)

# List of hypervisors this build system can generate configuration for.
HYPERVISORS := $(HYPERVISOR_FC) $(HYPERVISOR_QEMU) $(HYPERVISOR_CLH) $(HYPERVISOR_STRATOVIRT) $(HYPERVISOR_REMOTE)

QEMUPATH := $(QEMUBINDIR)/$(QEMUCMD)
QEMUVALIDHYPERVISORPATHS := [\"$(QEMUPATH)\"]

#QEMUTDXPATH := $(QEMUBINDIR)/$(QEMUTDXCMD)
QEMUTDXPATH := PLACEHOLDER_FOR_DISTRO_QEMU_WITH_TDX_SUPPORT
QEMUTDXVALIDHYPERVISORPATHS := [\"$(QEMUTDXPATH)\"]
QEMUTDXQUOTEGENERATIONSERVICESOCKETPORT := 4050

QEMUSNPPATH := $(QEMUBINDIR)/$(QEMUSNPCMD)
QEMUSNPVALIDHYPERVISORPATHS := [\"$(QEMUSNPPATH)\"]

QEMUVIRTIOFSPATH := $(QEMUBINDIR)/$(QEMUVIRTIOFSCMD)

CLHPATH := $(CLHBINDIR)/$(CLHCMD)
CLHVALIDHYPERVISORPATHS := [\"$(CLHPATH)\"]

FCPATH = $(FCBINDIR)/$(FCCMD)
FCVALIDHYPERVISORPATHS := [\"$(FCPATH)\"]
FCJAILERPATH = $(FCBINDIR)/$(FCJAILERCMD)
FCVALIDJAILERPATHS = [\"$(FCJAILERPATH)\"]

STRATOVIRTPATH = $(STRATOVIRTBINDIR)/$(STRATOVIRTCMD)
STRATOVIRTVALIDHYPERVISORPATHS := [\"$(STRATOVIRTPATH)\"]

# Default number of vCPUs
DEFVCPUS := 1
# Default maximum number of vCPUs
DEFMAXVCPUS := 0
# Default memory size in MiB
DEFMEMSZ := 2048
# Default memory slots
# Cases to consider :
# - nvdimm rootfs image
# - preallocated memory
# - vm template memory
# - hugepage memory
DEFMEMSLOTS := 10
# Default maximum memory in MiB
DEFMAXMEMSZ := 0
#Default number of bridges
DEFBRIDGES := 1
DEFENABLEANNOTATIONS := [\"enable_iommu\", \"virtio_fs_extra_args\", \"kernel_params\"]
DEFENABLEANNOTATIONSTEE := [\"enable_iommu\", \"virtio_fs_extra_args\", \"kernel_params\", \"default_vcpus\", \"default_memory\"]
DEFDISABLEGUESTSECCOMP := true
DEFDISABLEGUESTEMPTYDIR := false
#Default experimental features enabled
DEFAULTEXPFEATURES := []

DEFDISABLESELINUX := false

# Default guest SELinux configuration
DEFDISABLEGUESTSELINUX := true
DEFGUESTSELINUXLABEL := system_u:system_r:container_t

#Default SeccomSandbox param
#The same default policy is used by libvirt
#More explanation on https://lists.gnu.org/archive/html/qemu-devel/2017-02/msg03348.html
# Note: "elevateprivileges=deny" doesn't work with daemonize option, so it's removed from the seccomp sandbox
DEFSECCOMPSANDBOXPARAM := on,obsolete=deny,spawn=deny,resourcecontrol=deny

#Default entropy source
DEFENTROPYSOURCE := /dev/urandom
DEFVALIDENTROPYSOURCES := [\"/dev/urandom\",\"/dev/random\",\"\"]

DEFDISABLEBLOCK := false
DEFSHAREDFS_CLH_VIRTIOFS := virtio-fs
DEFSHAREDFS_QEMU_VIRTIOFS := virtio-fs
# Please keep DEFSHAREDFS_QEMU_COCO_DEV_VIRTIOFS in sync with TDX/SEV/SNP
DEFSHAREDFS_QEMU_COCO_DEV_VIRTIOFS := none
DEFSHAREDFS_STRATOVIRT_VIRTIOFS := virtio-fs
DEFSHAREDFS_QEMU_TDX_VIRTIOFS := none
DEFSHAREDFS_QEMU_SEV_VIRTIOFS := none
DEFSHAREDFS_QEMU_SNP_VIRTIOFS := none
DEFVIRTIOFSDAEMON := $(LIBEXECDIR)/virtiofsd
DEFVALIDVIRTIOFSDAEMONPATHS := [\"$(DEFVIRTIOFSDAEMON)\"]
# Default DAX mapping cache size in MiB
#if value is 0, DAX is not enabled
DEFVIRTIOFSCACHESIZE ?= 0
DEFVIRTIOFSCACHE ?= auto
DEFVIRTIOFSQUEUESIZE ?= 1024
# Format example:
#   [\"-o\", \"arg1=xxx,arg2\", \"-o\", \"hello world\", \"--arg3=yyy\"]
#
# see `virtiofsd -h` for possible options.
# Make sure you quote args.
DEFVIRTIOFSEXTRAARGS ?= [\"--thread-pool-size=1\", \"--announce-submounts\"]
DEFENABLEIOTHREADS := false
DEFENABLEVHOSTUSERSTORE := false
DEFVHOSTUSERSTOREPATH := $(PKGRUNDIR)/vhost-user
DEFVALIDVHOSTUSERSTOREPATHS := [\"$(DEFVHOSTUSERSTOREPATH)\"]
DEFFILEMEMBACKEND := ""
DEFVALIDFILEMEMBACKENDS := [\"$(DEFFILEMEMBACKEND)\"]
DEFMSIZE9P := 8192
DEFVFIOMODE := guest-kernel

# Default cgroup model
DEFSANDBOXCGROUPONLY ?= false

DEFSTATICRESOURCEMGMT ?= false
DEFSTATICRESOURCEMGMT_TEE = true

DEFBINDMOUNTS := []

# Create Container Timeout in seconds
DEFCREATECONTAINERTIMEOUT ?= 60

# Default directory of directly attachable network config.
DEFDANCONF := /run/kata-containers/dans

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

ifneq (,$(QEMUCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_QEMU)

    CONFIG_FILE_QEMU = configuration-qemu.toml
    CONFIG_QEMU = config/$(CONFIG_FILE_QEMU)
    CONFIG_QEMU_IN = $(CONFIG_QEMU).in

    CONFIG_PATH_QEMU = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU)

    SYSCONFIG_QEMU = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU)

    CONFIGS += $(CONFIG_QEMU)

    # Begin configuration-qemu-coco-dev
    CONFIG_FILE_QEMU_COCO_DEV = configuration-qemu-coco-dev.toml
    CONFIG_QEMU_COCO_DEV = config/$(CONFIG_FILE_QEMU_COCO_DEV)
    CONFIG_QEMU_COCO_DEV_IN = $(CONFIG_QEMU_COCO_DEV).in

    SYSCONFIG_QEMU_COCO_DEV = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_COCO_DEV))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU_COCO_DEV)

    CONFIGS += $(CONFIG_QEMU_COCO_DEV)
    # End configuration-qemu-coco-dev

    CONFIG_FILE_QEMU_TDX = configuration-qemu-tdx.toml
    CONFIG_QEMU_TDX = config/$(CONFIG_FILE_QEMU_TDX)
    CONFIG_QEMU_TDX_IN = $(CONFIG_QEMU_TDX).in

    CONFIG_PATH_QEMU_TDX = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_TDX))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_TDX)

    SYSCONFIG_QEMU_TDX = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_TDX))
    SYSCONFIG_PATHS_TDX += $(SYSCONFIG_QEMU_TDX)

    CONFIGS += $(CONFIG_QEMU_TDX)

    CONFIG_FILE_QEMU_SEV = configuration-qemu-sev.toml
    CONFIG_QEMU_SEV = config/$(CONFIG_FILE_QEMU_SEV)
    CONFIG_QEMU_SEV_IN = $(CONFIG_QEMU_SEV).in

    CONFIG_PATH_QEMU_SEV = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_SEV))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_SEV)

    SYSCONFIG_QEMU_SEV = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_SEV))
    SYSCONFIG_PATHS_SEV += $(SYSCONFIG_QEMU_SEV)

    CONFIGS += $(CONFIG_QEMU_SEV)

    CONFIG_FILE_QEMU_SNP = configuration-qemu-snp.toml
    CONFIG_QEMU_SNP = config/$(CONFIG_FILE_QEMU_SNP)
    CONFIG_QEMU_SNP_IN = $(CONFIG_QEMU_SNP).in

    CONFIG_PATH_QEMU_SNP = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_SNP))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_SNP)

    SYSCONFIG_QEMU_SNP = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_SNP))
    SYSCONFIG_PATHS_SNP += $(SYSCONFIG_QEMU_SNP)

    CONFIGS += $(CONFIG_QEMU_SNP)

    CONFIG_FILE_QEMU_NVIDIA_GPU = configuration-qemu-nvidia-gpu.toml
    CONFIG_QEMU_NVIDIA_GPU = config/$(CONFIG_FILE_QEMU_NVIDIA_GPU)
    CONFIG_QEMU_NVIDIA_GPU_IN = $(CONFIG_QEMU_NVIDIA_GPU).in

    CONFIGS += $(CONFIG_QEMU_NVIDIA_GPU)

    CONFIG_FILE_QEMU_NVIDIA_GPU_SNP = configuration-qemu-nvidia-gpu-snp.toml
    CONFIG_QEMU_NVIDIA_GPU_SNP = config/$(CONFIG_FILE_QEMU_NVIDIA_GPU_SNP)
    CONFIG_QEMU_NVIDIA_GPU_SNP_IN = $(CONFIG_QEMU_NVIDIA_GPU_SNP).in

    CONFIG_PATH_QEMU_NVIDIA_GPU_SNP = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_NVIDIA_GPU_SNP))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_NVIDIA_GPU_SNP)

    SYSCONFIG_QEMU_NVIDIA_GPU_SNP = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_NVIDIA_GPU_SNP))
    SYSCONFIG_PATHS_SNP += $(SYSCONFIG_QEMU_NVIDIA_GPU_SNP)

    CONFIGS += $(CONFIG_QEMU_NVIDIA_GPU_SNP)

    CONFIG_FILE_QEMU_NVIDIA_GPU_TDX = configuration-qemu-nvidia-gpu-tdx.toml
    CONFIG_QEMU_NVIDIA_GPU_TDX = config/$(CONFIG_FILE_QEMU_NVIDIA_GPU_TDX)
    CONFIG_QEMU_NVIDIA_GPU_TDX_IN = $(CONFIG_QEMU_NVIDIA_GPU_TDX).in

    CONFIG_PATH_QEMU_NVIDIA_GPU_TDX = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_NVIDIA_GPU_TDX))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_NVIDIA_GPU_TDX)

    SYSCONFIG_QEMU_NVIDIA_GPU_TDX = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_NVIDIA_GPU_TDX))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU_NVIDIA_GPU_TDX)

    CONFIGS += $(CONFIG_QEMU_NVIDIA_GPU_TDX)

    CONFIG_FILE_REMOTE = configuration-remote.toml
    CONFIG_REMOTE = config/$(CONFIG_FILE_REMOTE)
    CONFIG_REMOTE_IN = $(CONFIG_REMOTE).in

    CONFIG_PATH_REMOTE = $(abspath $(CONFDIR)/$(CONFIG_FILE_REMOTE))
    CONFIG_PATHS += $(CONFIG_PATH_REMOTE)

    SYSCONFIG_REMOTE = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_REMOTE))
    SYSCONFIG_PATHS += $(SYSCONFIG_REMOTE)

    CONFIGS += $(CONFIG_REMOTE)

    CONFIG_FILE_QEMU_SE = configuration-qemu-se.toml
    CONFIG_QEMU_SE = config/$(CONFIG_FILE_QEMU_SE)
    CONFIG_QEMU_SE_IN = $(CONFIG_QEMU_SE).in

    CONFIG_PATH_QEMU_SE = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_SE))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_SE)

    SYSCONFIG_QEMU_SE = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_SE))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU_SE)

    CONFIGS += $(CONFIG_QEMU_SE)

    # qemu-specific options (all should be suffixed by "_QEMU")
    DEFBLOCKSTORAGEDRIVER_QEMU := virtio-scsi
    DEFBLOCKDEVICEAIO_QEMU := io_uring
    DEFNETWORKMODEL_QEMU := tcfilter

    KERNELTYPE = uncompressed
    KERNELNAME = $(call MAKE_KERNEL_NAME,$(KERNELTYPE))
    KERNELPATH = $(KERNELDIR)/$(KERNELNAME)

    KERNELCONFIDENTIALTYPE = compressed
    KERNELCONFIDENTIALNAME = $(call MAKE_KERNEL_CONFIDENTIAL_NAME,$(KERNELCONFIDENTIALTYPE))
    KERNELCONFIDENTIALPATH = $(KERNELDIR)/$(KERNELCONFIDENTIALNAME)

    KERNELSENAME = kata-containers-se.img
    KERNELSEPATH = $(KERNELDIR)/$(KERNELSENAME)

    # NVIDIA GPU specific options (all should be suffixed by _NV)
    KERNELNAME_NV = $(call MAKE_KERNEL_NAME_NV,$(KERNELTYPE))
    KERNELPATH_NV = $(KERNELDIR)/$(KERNELNAME_NV)

    KERNELNAME_CONFIDENTIAL_NV = $(call MAKE_KERNEL_CONFIDENTIAL_NAME_NV,$(KERNELCONFIDENTIALTYPE))
    KERNELPATH_CONFIDENTIAL_NV = $(KERNELDIR)/$(KERNELNAME_CONFIDENTIAL_NV)

    DEFAULTVCPUS_NV = 1
    DEFAULTMEMORY_NV = 2048
    DEFAULTTIMEOUT_NV = 500
    DEFAULTVFIOPORT_NV = root-port
    DEFAULTPCIEROOTPORT_NV = 8

    KERNELPARAMS_NV =  "agent.hotplug_timeout=20"
    KERNELPARAMS_NV += "cgroup_no_v1=all"

    KERNELTDXPARAMS_NV = $(KERNELPARAMS_NV)
    KERNELTDXPARAMS_NV += "clearcpuid=mtrr"
    KERNELTDXPARAMS_NV += "authorize_allow_devs=pci:ALL"

    KERNELSNPPARAMS_NV = $(KERNELPARAMS_NV)

    # Setting this to false can lead to cgroup leakages in the host
    # Best practice for production is to set this to true
    DEFSANDBOXCGROUPONLY_NV = true
    # The latest OVMF build should be good for both TDX and SNP
    FIRMWAREPATH_NV := $(PREFIXDEPS)/share/ovmf/OVMF.fd
endif

ifneq (,$(CLHCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_CLH)

    CONFIG_FILE_CLH = configuration-clh.toml
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
endif

ifneq (,$(STRATOVIRTCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_STRATOVIRT)

    CONFIG_FILE_STRATOVIRT = configuration-stratovirt.toml
    CONFIG_STRATOVIRT = config/$(CONFIG_FILE_STRATOVIRT)
    CONFIG_STRATOVIRT_IN = $(CONFIG_STRATOVIRT).in

    CONFIG_PATH_STRATOVIRT = $(abspath $(CONFDIR)/$(CONFIG_FILE_STRATOVIRT))
    CONFIG_PATHS += $(CONFIG_PATH_STRATOVIRT)

    SYSCONFIG_STRATOVIRT = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_STRATOVIRT))
    SYSCONFIG_PATHS += $(SYSCONFIG_STRATOVIRT)

    CONFIGS += $(CONFIG_STRATOVIRT)

    # stratovirt-specific options (all should be suffixed by "_STRATOVIRT")
    DEFMACHINETYPE_STRATOVIRT := microvm
    DEFBLOCKSTORAGEDRIVER_STRATOVIRT := virtio-mmio
    DEFNETWORKMODEL_STRATOVIRT := tcfilter
    DEFSTATICRESOURCEMGMT_STRATOVIRT = true
ifeq ($(ARCH),amd64)
    KERNELTYPE_STRATOVIRT = compressed
endif
ifeq ($(ARCH),arm64)
    KERNELTYPE_STRATOVIRT = uncompressed
endif
    KERNEL_NAME_STRATOVIRT = $(call MAKE_KERNEL_NAME,$(KERNELTYPE_STRATOVIRT))
    KERNELPATH_STRATOVIRT = $(KERNELDIR)/$(KERNEL_NAME_STRATOVIRT)
endif

ifneq (,$(FCCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_FC)

    CONFIG_FILE_FC = configuration-fc.toml
    CONFIG_FC = config/$(CONFIG_FILE_FC)
    CONFIG_FC_IN = $(CONFIG_FC).in

    CONFIG_PATH_FC = $(abspath $(CONFDIR)/$(CONFIG_FILE_FC))
    CONFIG_PATHS += $(CONFIG_PATH_FC)

    SYSCONFIG_FC = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_FC))
    SYSCONFIG_PATHS += $(SYSCONFIG_FC)

    CONFIGS += $(CONFIG_FC)

    # firecracker-specific options (all should be suffixed by "_FC")
    DEFBLOCKSTORAGEDRIVER_FC := virtio-mmio
    DEFNETWORKMODEL_FC := tcfilter
    DEFSTATICRESOURCEMGMT_FC = true
    KERNELTYPE_FC = uncompressed
    KERNEL_NAME_FC = $(call MAKE_KERNEL_NAME,$(KERNELTYPE_FC))
    KERNELPATH_FC = $(KERNELDIR)/$(KERNEL_NAME_FC)
endif

ifeq (,$(KNOWN_HYPERVISORS))
    $(error "ERROR: No hypervisors known for architecture $(ARCH) (looked for: $(HYPERVISORS))")
endif

ifeq (,$(findstring $(DEFAULT_HYPERVISOR),$(HYPERVISORS)))
    $(error "ERROR: Invalid default hypervisor: '$(DEFAULT_HYPERVISOR)'")
endif

ifeq (,$(findstring $(DEFAULT_HYPERVISOR),$(KNOWN_HYPERVISORS)))
    $(error "ERROR: Default hypervisor '$(DEFAULT_HYPERVISOR)' not known for architecture $(ARCH)")
endif

ifeq ($(DEFAULT_HYPERVISOR),$(HYPERVISOR_QEMU))
    DEFAULT_HYPERVISOR_CONFIG = $(CONFIG_FILE_QEMU)
endif

ifeq ($(DEFAULT_HYPERVISOR),$(HYPERVISOR_QEMU_VIRTIOFS))
    DEFAULT_HYPERVISOR_CONFIG = $(CONFIG_FILE_QEMU_VIRTIOFS)
endif

ifeq ($(DEFAULT_HYPERVISOR),$(HYPERVISOR_FC))
    DEFAULT_HYPERVISOR_CONFIG = $(CONFIG_FILE_FC)
endif

ifeq ($(DEFAULT_HYPERVISOR),$(HYPERVISOR_CLH))
    DEFAULT_HYPERVISOR_CONFIG = $(CONFIG_FILE_CLH)
endif

CONFDIR := $(DEFAULTSDIR)/$(PROJECT_DIR)
SYSCONFDIR := $(SYSCONFDIR)/$(PROJECT_DIR)

# Main configuration file location for stateless systems
CONFIG_PATH := $(abspath $(CONFDIR)/$(CONFIG_FILE))

# Secondary configuration file location. Note that this takes precedence
# over CONFIG_PATH.
SYSCONFIG := $(abspath $(SYSCONFDIR)/$(CONFIG_FILE))

SHAREDIR := $(SHAREDIR)

# list of variables the user may wish to override
USER_VARS += ARCH
USER_VARS += BINDIR
USER_VARS += CONFIG_CLH_IN
USER_VARS += CONFIG_FC_IN
USER_VARS += CONFIG_STRATOVIRT_IN
USER_VARS += CONFIG_PATH
USER_VARS += CONFIG_QEMU_IN
USER_VARS += CONFIG_REMOTE_IN
USER_VARS += DESTDIR
USER_VARS += DEFAULT_HYPERVISOR
USER_VARS += CLHPATH
USER_VARS += CLHVALIDHYPERVISORPATHS
USER_VARS += FIRMWAREPATH_CLH
USER_VARS += FCCMD
USER_VARS += FCPATH
USER_VARS += FCVALIDHYPERVISORPATHS
USER_VARS += FCJAILERPATH
USER_VARS += FCVALIDJAILERPATHS
USER_VARS += STRATOVIRTPATH
USER_VARS += STRATOVIRTVALIDHYPERVISORPATHS
USER_VARS += SYSCONFIG
USER_VARS += IMAGENAME
USER_VARS += IMAGECONFIDENTIALNAME
USER_VARS += IMAGEPATH
USER_VARS += IMAGECONFIDENTIALPATH
USER_VARS += INITRDNAME
USER_VARS += INITRDCONFIDENTIALNAME
USER_VARS += INITRDPATH
USER_VARS += INITRDCONFIDENTIALPATH
USER_VARS += IMAGENAME_NV
USER_VARS += IMAGENAME_CONFIDENTIAL_NV
USER_VARS += IMAGEPATH_NV
USER_VARS += IMAGEPATH_CONFIDENTIAL_NV
USER_VARS += INITRDNAME_NV
USER_VARS += INITRDNAME_CONFIDENTIAL_NV
USER_VARS += INITRDPATH_NV
USER_VARS += INITRDPATH_CONFIDENTIAL_NV
USER_VARS += KERNELNAME_NV
USER_VARS += KERNELPATH_NV
USER_VARS += KERNELNAME_CONFIDENTIAL_NV
USER_VARS += KERNELPATH_CONFIDENTIAL_NV
USER_VARS += DEFAULTVCPUS_NV
USER_VARS += DEFAULTMEMORY_NV
USER_VARS += DEFAULTVFIOPORT_NV
USER_VARS += DEFAULTPCIEROOTPORT_NV
USER_VARS += KERNELPARAMS_NV
USER_VARS += KERNELTDXPARAMS_NV
USER_VARS += KERNELSNPPARAMS_NV
USER_VARS += DEFAULTTIMEOUT_NV
USER_VARS += DEFSANDBOXCGROUPONLY_NV
USER_VARS += FIRMWAREPATH_NV
USER_VARS += DEFROOTFSTYPE
USER_VARS += MACHINETYPE
USER_VARS += KERNELDIR
USER_VARS += KERNELTYPE
USER_VARS += KERNELTYPE_FC
USER_VARS += KERNELTYPE_CLH
USER_VARS += KERNELPATH
USER_VARS += KERNELCONFIDENTIALPATH
USER_VARS += KERNELSEPATH
USER_VARS += KERNELPATH_CLH
USER_VARS += KERNELPATH_FC
USER_VARS += KERNELPATH_STRATOVIRT
USER_VARS += KERNELVIRTIOFSPATH
USER_VARS += FIRMWAREPATH
USER_VARS += FIRMWARESEVPATH
USER_VARS += FIRMWARETDVFPATH
USER_VARS += FIRMWAREVOLUMEPATH
USER_VARS += FIRMWARETDVFVOLUMEPATH
USER_VARS += FIRMWARESNPPATH
USER_VARS += MACHINEACCELERATORS
USER_VARS += CPUFEATURES
USER_VARS += TDXCPUFEATURES
USER_VARS += DEFMACHINETYPE_CLH
USER_VARS += DEFMACHINETYPE_STRATOVIRT
USER_VARS += KERNELPARAMS
USER_VARS += KERNELTDXPARAMS
USER_VARS += KERNELQEMUCOCODEVPARAMS
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
USER_VARS += PROJECT_URL
USER_VARS += QEMUBINDIR
USER_VARS += QEMUCMD
USER_VARS += QEMUTDXCMD
USER_VARS += QEMUSNPCMD
USER_VARS += QEMUPATH
USER_VARS += QEMUTDXPATH
USER_VARS += QEMUTDXQUOTEGENERATIONSERVICESOCKETPORT
USER_VARS += QEMUSNPPATH
USER_VARS += QEMUVALIDHYPERVISORPATHS
USER_VARS += QEMUTDXVALIDHYPERVISORPATHS
USER_VARS += QEMUSNPVALIDHYPERVISORPATHS
USER_VARS += QEMUVIRTIOFSCMD
USER_VARS += QEMUVIRTIOFSPATH
USER_VARS += RUNTIME_NAME
USER_VARS += SHAREDIR
USER_VARS += SYSCONFDIR
USER_VARS += DEFVCPUS
USER_VARS += DEFMAXVCPUS
USER_VARS += DEFMEMSZ
USER_VARS += DEFMEMSLOTS
USER_VARS += DEFMAXMEMSZ
USER_VARS += DEFBRIDGES
USER_VARS += DEFNETWORKMODEL_CLH
USER_VARS += DEFNETWORKMODEL_FC
USER_VARS += DEFNETWORKMODEL_QEMU
USER_VARS += DEFNETWORKMODEL_STRATOVIRT
USER_VARS += DEFDISABLEGUESTEMPTYDIR
USER_VARS += DEFDISABLEGUESTSECCOMP
USER_VARS += DEFDISABLESELINUX
USER_VARS += DEFDISABLEGUESTSELINUX
USER_VARS += DEFGUESTSELINUXLABEL
USER_VARS += DEFAULTEXPFEATURES
USER_VARS += DEFDISABLEBLOCK
USER_VARS += DEFBLOCKSTORAGEDRIVER_FC
USER_VARS += DEFBLOCKSTORAGEDRIVER_QEMU
USER_VARS += DEFBLOCKSTORAGEDRIVER_STRATOVIRT
USER_VARS += DEFBLOCKDEVICEAIO_QEMU
USER_VARS += DEFSHAREDFS_CLH_VIRTIOFS
USER_VARS += DEFSHAREDFS_QEMU_VIRTIOFS
USER_VARS += DEFSHAREDFS_QEMU_COCO_DEV_VIRTIOFS
USER_VARS += DEFSHAREDFS_STRATOVIRT_VIRTIOFS
USER_VARS += DEFSHAREDFS_QEMU_TDX_VIRTIOFS
USER_VARS += DEFSHAREDFS_QEMU_SEV_VIRTIOFS
USER_VARS += DEFSHAREDFS_QEMU_SNP_VIRTIOFS
USER_VARS += DEFVIRTIOFSDAEMON
USER_VARS += DEFVALIDVIRTIOFSDAEMONPATHS
USER_VARS += DEFVIRTIOFSCACHESIZE
USER_VARS += DEFVIRTIOFSCACHE
USER_VARS += DEFVIRTIOFSQUEUESIZE
USER_VARS += DEFVIRTIOFSEXTRAARGS
USER_VARS += DEFENABLEANNOTATIONS
USER_VARS += DEFENABLEANNOTATIONSTEE
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
USER_VARS += DEFSANDBOXCGROUPONLY
USER_VARS += DEFSTATICRESOURCEMGMT
USER_VARS += DEFSTATICRESOURCEMGMT_CLH
USER_VARS += DEFSTATICRESOURCEMGMT_FC
USER_VARS += DEFSTATICRESOURCEMGMT_STRATOVIRT
USER_VARS += DEFSTATICRESOURCEMGMT_TEE
USER_VARS += DEFBINDMOUNTS
USER_VARS += DEFCREATECONTAINERTIMEOUT
USER_VARS += DEFDANCONF
USER_VARS += DEFVFIOMODE
USER_VARS += BUILDFLAGS


V              = @
Q              = $(V:1=)
QUIET_BUILD    = $(Q:@=@echo    '     BUILD    '$@;)
QUIET_CHECK    = $(Q:@=@echo    '     CHECK    '$@;)
QUIET_CLEAN    = $(Q:@=@echo    '     CLEAN    '$@;)
QUIET_GENERATE = $(Q:@=@echo    '     GENERATE '$@;)
QUIET_INST     = $(Q:@=@echo    '     INSTALL  '$@;)
QUIET_TEST     = $(Q:@=@echo    '     TEST     '$@;)

BUILDTAGS :=

# go build common flags
BUILDFLAGS := -buildmode=pie -mod=vendor ${BUILDTAGS}

# whether stipping the binary
ifeq ($(STRIP),yes)
       KATA_LDFLAGS = -w -s
endif

ifeq ($(ARCH),s390x)
    KATA_LDFLAGS += -extldflags=-Wl,--s390-pgste
endif

# Return non-empty string if specified directory exists
define DIR_EXISTS
$(shell test -d $(1) && echo "$(1)")
endef

# $1: name of architecture to display
define SHOW_ARCH
  $(shell printf "\\t%s%s\\\n" "$(1)" $(if $(filter $(ARCH),$(1))," (default)",""))
endef

all: runtime containerd-shim-v2 monitor

# Targets that depend on .git-commit can use $(shell git rev-parse HEAD) to get a
# git revision string.  They will only be rebuilt if the revision string
# actually changes.
.PHONY: .git-commit.tmp
.git-commit: .git-commit.tmp
	@cmp $< $@ >/dev/null 2>&1 || cp $< $@
.git-commit.tmp:
	@echo -n "$$(git rev-parse HEAD 2>/dev/null)" >$@
	@test -n "$$(git status --porcelain --untracked-files=no)" && echo -n "-dirty" >>$@ || true

containerd-shim-v2: $(SHIMV2_OUTPUT)

monitor: $(MONITOR_OUTPUT)

runtime: $(RUNTIME_OUTPUT) $(CONFIGS)
.DEFAULT: default

build: all

#Install an executable file
# params:
# $1 : file to install
# $2 : directory path where file will be installed
define INSTALL_EXEC
	install -D $1 $(DESTDIR)$2/$(notdir $1);
endef

# Install a configuration file
# params:
# $1 : file to install
# $2 : directory path where file will be installed
define INSTALL_CONFIG
	install --mode 0644 -D $1 $(DESTDIR)$2/$(notdir $1);
endef

# Returns the name of the kernel file to use based on the provided KERNELTYPE.
# $1 : KERNELTYPE (compressed or uncompressed)
define MAKE_KERNEL_NAME
$(if $(findstring uncompressed,$1),vmlinux.container,vmlinuz.container)
endef

define MAKE_KERNEL_VIRTIOFS_NAME
$(if $(findstring uncompressed,$1),vmlinux-virtiofs.container,vmlinuz-virtiofs.container)
endef

define MAKE_KERNEL_CONFIDENTIAL_NAME
$(if $(findstring uncompressed,$1),vmlinux-confidential.container,vmlinuz-confidential.container)
endef

define MAKE_KERNEL_NAME_NV
$(if $(findstring uncompressed,$1),vmlinux-nvidia-gpu.container,vmlinuz-nvidia-gpu.container)
endef

define MAKE_KERNEL_CONFIDENTIAL_NAME_NV
$(if $(findstring uncompressed,$1),vmlinux-nvidia-gpu-confidential.container,vmlinuz-nvidia-gpu-confidential.container)
endef


GENERATED_FILES += pkg/katautils/config-settings.go

$(RUNTIME_OUTPUT): $(SOURCES) $(GENERATED_FILES) $(MAKEFILE_LIST) | show-summary
	$(QUIET_BUILD)(cd $(RUNTIME_DIR) && go build -ldflags "$(KATA_LDFLAGS)" $(BUILDFLAGS) -o $@ .)

$(SHIMV2_OUTPUT): $(SOURCES) $(GENERATED_FILES) $(MAKEFILE_LIST)
	$(QUIET_BUILD)(cd $(SHIMV2_DIR)/ && go build -ldflags "$(KATA_LDFLAGS)" $(BUILDFLAGS) -o $@ .)

$(MONITOR_OUTPUT): $(SOURCES) $(GENERATED_FILES) $(MAKEFILE_LIST) .git-commit
	$(QUIET_BUILD)(cd $(MONITOR_DIR)/ && go build \
		--ldflags "-X main.GitCommit=$(shell git rev-parse HEAD)" $(BUILDFLAGS) -o $@ .)

.PHONY: \
	check \
	coverage \
	default \
	install \
	lint \
	pre-commit \
	show-header \
	show-summary \
	show-variables \
	vendor

$(TARGET).coverage: $(SOURCES) $(GENERATED_FILES) $(MAKEFILE_LIST)
	$(QUIET_TEST)go test -o $@ -covermode count

GENERATED_FILES += $(CONFIGS)

$(GENERATED_FILES): %: %.in $(MAKEFILE_LIST) VERSION .git-commit
	$(QUIET_GENERATE)$(SED) \
		-e "s|@COMMIT@|$(shell git rev-parse HEAD)|g" \
		$(foreach v,$(GENERATED_VARS),-e "s|@$v@|$($v)|g") \
		$< > $@

generate-config: $(CONFIGS)

test: hook go-test

hook:
	make -C pkg/katautils/mockhook

go-test: $(GENERATED_FILES)
	go clean -testcache
	$(QUIET_TEST)./go-test.sh

fast-test: $(GENERATED_FILES)
	go clean -testcache
	for s in $$(go list ./...); do if ! go test -failfast -v -mod=vendor -p 1 $$s; then break; fi; done

GOLANGCI_LINT_FILE := ../../tests/.golangci.yml
GOLANGCI_LINT_NAME = golangci-lint
GOLANGCI_LINT_CMD := $(shell command -v $(GOLANGCI_LINT_NAME) 2>/dev/null)
lint: all
	if [ -z $(GOLANGCI_LINT_CMD) ] ; \
	then \
		echo "ERROR: command $(GOLANGCI_LINT_NAME) not found. Please install it first." >&2; exit 1; \
	fi

	if [ -f $(GOLANGCI_LINT_FILE) ] ; \
	then \
		echo "running $(GOLANGCI_LINT_NAME)..."; \
		$(GOLANGCI_LINT_NAME) run -c $(GOLANGCI_LINT_FILE) ; \
	else \
		echo "ERROR: file $(GOLANGCI_LINT_FILE) not found. You should clone https://github.com/kata-containers/tests to run $(GOLANGCI_LINT_NAME) locally." >&2; exit 1; \
	fi;

pre-commit: lint fast-test

coverage:
	go test -v -mod=vendor -covermode=atomic -coverprofile=coverage.txt ./...
	go tool cover -html=coverage.txt -o coverage.html

install: all install-runtime install-containerd-shim-v2 install-monitor

install-bin: $(BINLIST)
	$(QUIET_INST)$(foreach f,$(BINLIST),$(call INSTALL_EXEC,$f,$(BINDIR)))

install-runtime: runtime install-scripts install-completions install-configs install-bin

install-containerd-shim-v2: $(SHIMV2_OUTPUT)
	$(QUIET_INST)$(call INSTALL_EXEC,$<,$(BINDIR))

install-monitor: $(MONITOR)
	$(QUIET_INST)$(call INSTALL_EXEC,$<,$(BINDIR))

install-bin-libexec: $(BINLIBEXECLIST)
	$(QUIET_INST)$(foreach f,$(BINLIBEXECLIST),$(call INSTALL_EXEC,$f,$(PKGLIBEXECDIR)))

install-configs: $(CONFIGS)
	$(QUIET_INST)$(foreach f,$(CONFIGS),$(call INSTALL_CONFIG,$f,$(dir $(CONFIG_PATH))))
	$(QUIET_INST)ln -sf $(DEFAULT_HYPERVISOR_CONFIG) $(DESTDIR)/$(CONFIG_PATH)

install-scripts: $(SCRIPTS)
	$(QUIET_INST)$(foreach f,$(SCRIPTS),$(call INSTALL_EXEC,$f,$(SCRIPTS_DIR)))

install-completions:
	$(QUIET_INST)install --mode 0644 -D  $(BASH_COMPLETIONS) $(DESTDIR)/$(BASH_COMPLETIONSDIR)/$(notdir $(BASH_COMPLETIONS));

handle_vendor:
	go mod tidy
	go mod vendor
	go mod verify

vendor: handle_vendor
	./hack/tree_status.sh

static-checks-build: $(GENERATED_FILES)

clean: clean-generated-files
	$(QUIET_CLEAN)rm -f \
		$(MONITOR) \
		$(SHIMV2) \
		$(TARGET) \
		.git-commit .git-commit.tmp

clean-generated-files:
	$(QUIET_CLEAN)rm -f $(GENERATED_FILES)

show-usage: show-header
	@printf "• Overview:\n"
	@printf "\n"
	@printf "\tTo build $(TARGET), just run, \"make\".\n"
	@printf "\n"
	@printf "\tFor a verbose build, run \"make V=1\".\n"
	@printf "\n"
	@printf "• Additional targets:\n"
	@printf "\n"
	@printf "\tbuild                      : standard build (build everything).\n"
	@printf "\ttest                       : run tests.\n"
	@printf "\tpre-commit                 : run $(GOLANGCI_LINT_NAME) and tests locally.\n"
	@printf "\tlint                       : run $(GOLANGCI_LINT_NAME).\n"
	@printf "\tfast-test                  : run tests with failfast option.\n"
	@printf "\tcheck                      : run code checks.\n"
	@printf "\tclean                      : remove built files.\n"
	@printf "\tclean-generated-files      : remove generated files.\n"
	@printf "\tcontainerd-shim-v2         : only build containerd shim v2.\n"
	@printf "\tcoverage                   : run coverage tests.\n"
	@printf "\tdefault                    : same as 'make build' (or just 'make').\n"
	@printf "\tgenerate-config            : create configuration file.\n"
	@printf "\tinstall                    : install everything.\n"
	@printf "\tinstall-containerd-shim-v2 : only install containerd shim v2 files.\n"
	@printf "\tinstall-runtime            : only install runtime files.\n"
	@printf "\truntime                    : only build runtime.\n"
	@printf "\tshow-arches                : show supported architectures (ARCH variable values).\n"
	@printf "\tshow-summary               : show install locations.\n"
	@printf "\n"

handle_help: show-usage show-summary show-variables show-footer

usage: handle_help
help: handle_help

show-variables:
	@printf "• Variables affecting the build:\n\n"
	@printf \
          "$(foreach v,$(sort $(USER_VARS)),$(shell printf "\\t$(v)='$($(v))'\\\n"))"
	@printf "\n"

show-header: .git-commit
	@printf "%s - version %s (commit %s)\n\n" $(TARGET) $(VERSION) $(shell git rev-parse HEAD)

show-arches: show-header
	@printf "Supported architectures (possible values for ARCH variable):\n\n"
	@printf \
		"$(foreach v,$(ALL_ARCHES),$(call SHOW_ARCH,$(v)))\n"

show-footer:
	@printf "• Project:\n"
	@printf "\tHome: $(PROJECT_URL)\n"
	@printf "\tBugs: $(PROJECT_BUG_URL)\n\n"

show-summary: show-header
ifneq (,$(golang_version_raw))
	@printf "• architecture:\n"
	@printf "\tHost: $(HOST_ARCH)\n"
	@printf "\tgolang: $(GOARCH)\n"
	@printf "\tBuild: $(ARCH)\n"
	@printf "\n"
	@printf "• golang:\n"
	@printf "\t"
	@go version
else
	@printf "• No GO command or GOPATH not set:\n"
	@printf "\tCan only install prebuilt binaries\n"
endif
	@printf "\n"
	@printf "• hypervisors:\n"
	@printf "\tDefault: $(DEFAULT_HYPERVISOR)\n"
	@printf "\tKnown: $(sort $(HYPERVISORS))\n"
	@printf "\tAvailable for this architecture: $(sort $(KNOWN_HYPERVISORS))\n"
	@printf "\n"
	@printf "• Summary:\n"
	@printf "\n"
	@printf "\tdestination install path (DESTDIR) : %s\n" $(abspath $(DESTDIR))
	@printf "\tbinary installation path (BINDIR) : %s\n" $(abspath $(BINDIR))
	@printf "\tbinaries to install :\n"
	@printf \
          "$(foreach b,$(sort $(BINLIST)),$(shell printf "\\t - $(abspath $(DESTDIR)/$(BINDIR)/$(b))\\\n"))"
	@printf \
          "$(foreach b,$(sort $(SHIMV2)),$(shell printf "\\t - $(abspath $(DESTDIR)/$(BINDIR)/$(b))\\\n"))"
	@printf \
          "$(foreach b,$(sort $(MONITOR)),$(shell printf "\\t - $(abspath $(DESTDIR)/$(BINDIR)/$(b))\\\n"))"
	@printf \
          "$(foreach b,$(sort $(BINLIBEXECLIST)),$(shell printf "\\t - $(abspath $(DESTDIR)/$(PKGLIBEXECDIR)/$(b))\\\n"))"
	@printf \
          "$(foreach s,$(sort $(SCRIPTS)),$(shell printf "\\t - $(abspath $(DESTDIR)/$(BINDIR)/$(s))\\\n"))"
	@printf "\tconfigs to install (CONFIGS) :\n"
	@printf \
	  "$(foreach c,$(sort $(CONFIGS)),$(shell printf "\\t - $(c)\\\n"))"
	@printf "\tinstall paths (CONFIG_PATHS) :\n"
	@printf \
	  "$(foreach c,$(sort $(CONFIG_PATHS)),$(shell printf "\\t - $(c)\\\n"))"
	@printf "\talternate config paths (SYSCONFIG_PATHS) : %s\n"
	@printf \
	  "$(foreach c,$(sort $(SYSCONFIG_PATHS)),$(shell printf "\\t - $(c)\\\n"))"

	@printf "\tdefault install path for $(DEFAULT_HYPERVISOR) (CONFIG_PATH) : %s\n" $(abspath $(CONFIG_PATH))
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
ifneq (,$(findstring $(HYPERVISOR_STRATOVIRT),$(KNOWN_HYPERVISORS)))
	@printf "\t$(HYPERVISOR_STRATOVIRT) hypervisor path (STRATOVIRTPATH) : %s\n" $(abspath $(STRATOVIRTPATH))
endif
	@printf "\tassets path (PKGDATADIR) : %s\n" $(abspath $(PKGDATADIR))
	@printf "\tshim path (PKGLIBEXECDIR) : %s\n" $(abspath $(PKGLIBEXECDIR))
	@printf "\n"
