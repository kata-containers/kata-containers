#
# Copyright (c) 2018-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

distro := $(shell \
for file in /etc/os-release /usr/lib/os-release; do \
    if [ -e $$file ]; then \
        grep ^ID= $$file|cut -d= -f2-|tr -d '"'; \
        break; \
    fi \
done)

SKIP_GO_VERSION_CHECK=
include golang.mk

#Get ARCH.
ifneq (,$(golang_version_raw))
    GOARCH=$(shell go env GOARCH)
    ifeq ($(ARCH),)
        ARCH = $(GOARCH)
    endif
else
    ARCH = $(shell uname -m)
    ifeq ($(ARCH),x86_64)
        ARCH = amd64
    endif
    ifeq ($(ARCH),aarch64)
        ARCH = arm64
    endif
endif

ARCH_DIR = arch
ARCH_FILE_SUFFIX = -options.mk
ARCH_FILE = $(ARCH_DIR)/$(ARCH)$(ARCH_FILE_SUFFIX)
ARCH_FILES = $(wildcard arch/*$(ARCH_FILE_SUFFIX))
ALL_ARCHES = $(patsubst $(ARCH_DIR)/%$(ARCH_FILE_SUFFIX),%,$(ARCH_FILES))

# Load architecture-dependent settings
include $(ARCH_FILE)

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
INITRDNAME = $(PROJECT_TAG)-initrd.img

TARGET = $(BIN_PREFIX)-runtime
TARGET_OUTPUT = $(CURDIR)/$(TARGET)
BINLIST += $(TARGET)

NETMON_DIR = netmon
NETMON_TARGET = $(PROJECT_TYPE)-netmon
NETMON_TARGET_OUTPUT = $(CURDIR)/$(NETMON_TARGET)
BINLIBEXECLIST += $(NETMON_TARGET)

DESTDIR := /

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
ACRNBINDIR    := $(PREFIXDEPS)/bin
VIRTIOFSDBINDIR := $(PREFIXDEPS)/bin
SYSCONFDIR    := /etc
LOCALSTATEDIR := /var

LIBEXECDIR := $(PREFIXDEPS)/libexec
SHAREDIR := $(PREFIX)/share
DEFAULTSDIR := $(SHAREDIR)/defaults

COLLECT_SCRIPT = data/kata-collect-data.sh
COLLECT_SCRIPT_SRC = $(COLLECT_SCRIPT).in

GENERATED_FILES += $(COLLECT_SCRIPT)
SCRIPTS += $(COLLECT_SCRIPT)
SCRIPTS_DIR := $(BINDIR)

BASH_COMPLETIONS := data/completions/bash/kata-runtime
BASH_COMPLETIONSDIR := $(SHAREDIR)/bash-completion/completions

PKGDATADIR := $(PREFIXDEPS)/share/$(PROJECT_DIR)
PKGLIBDIR := $(LOCALSTATEDIR)/lib/$(PROJECT_DIR)
PKGRUNDIR := $(LOCALSTATEDIR)/run/$(PROJECT_DIR)
PKGLIBEXECDIR := $(LIBEXECDIR)/$(PROJECT_DIR)

KERNELDIR := $(PKGDATADIR)

INITRDPATH := $(PKGDATADIR)/$(INITRDNAME)
IMAGEPATH := $(PKGDATADIR)/$(IMAGENAME)
FIRMWAREPATH :=

# Name of default configuration file the runtime will use.
CONFIG_FILE = configuration.toml

HYPERVISOR_ACRN = acrn
HYPERVISOR_FC = firecracker
JAILER_FC = jailer
HYPERVISOR_QEMU = qemu
HYPERVISOR_CLH = cloud-hypervisor
HYPERVISOR_QEMU_VIRTIOFS = qemu-virtiofs

# Determines which hypervisor is specified in $(CONFIG_FILE).
DEFAULT_HYPERVISOR ?= $(HYPERVISOR_QEMU)

# List of hypervisors this build system can generate configuration for.
HYPERVISORS := $(HYPERVISOR_ACRN) $(HYPERVISOR_FC) $(HYPERVISOR_QEMU) $(HYPERVISOR_QEMU_VIRTIOFS) $(HYPERVISOR_CLH)

QEMUPATH := $(QEMUBINDIR)/$(QEMUCMD)

QEMUVIRTIOFSPATH := $(QEMUBINDIR)/$(QEMUVIRTIOFSCMD)

CLHPATH := $(CLHBINDIR)/$(CLHCMD)

FCPATH = $(FCBINDIR)/$(FCCMD)
FCJAILERPATH = $(FCBINDIR)/$(FCJAILERCMD)

ACRNPATH := $(ACRNBINDIR)/$(ACRNCMD)
ACRNCTLPATH := $(ACRNBINDIR)/$(ACRNCTLCMD)

SHIMCMD := $(BIN_PREFIX)-shim
SHIMPATH := $(PKGLIBEXECDIR)/$(SHIMCMD)

NETMONCMD := $(BIN_PREFIX)-netmon
NETMONPATH := $(PKGLIBEXECDIR)/$(NETMONCMD)

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
#Default number of bridges
DEFBRIDGES := 1
DEFDISABLEGUESTSECCOMP := true
#Default experimental features enabled
DEFAULTEXPFEATURES := []

#Default entropy source
DEFENTROPYSOURCE := /dev/urandom

DEFDISABLEBLOCK := false
DEFSHAREDFS := virtio-9p
DEFSHAREDFS_QEMU_VIRTIOFS := virtio-fs
DEFVIRTIOFSDAEMON := $(VIRTIOFSDBINDIR)/virtiofsd
# Default DAX mapping cache size in MiB
#if value is 0, DAX is not enabled
DEFVIRTIOFSCACHESIZE := 0
DEFVIRTIOFSCACHE ?= auto
# Format example:
#   [\"-o\", \"arg1=xxx,arg2\", \"-o\", \"hello world\", \"--arg3=yyy\"]
#
# see `virtiofsd -h` for possible options.
# Make sure you quote args.
DEFVIRTIOFSEXTRAARGS ?= []
DEFENABLEIOTHREADS := false
DEFENABLEMEMPREALLOC := false
DEFENABLEHUGEPAGES := false
DEFENABLEVHOSTUSERSTORE := false
DEFVHOSTUSERSTOREPATH := $(PKGRUNDIR)/vhost-user
DEFENABLESWAP := false
DEFENABLEDEBUG := false
DEFDISABLENESTINGCHECKS := false
DEFMSIZE9P := 8192
DEFHOTPLUGVFIOONROOTBUS := false
DEFPCIEROOTPORT := 0

# Default cgroup model
DEFSANDBOXCGROUPONLY ?= false

# Features
FEATURE_SELINUX ?= check

SED = sed

CLI_DIR = cli
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
    CONFIG_QEMU = $(CLI_DIR)/config/$(CONFIG_FILE_QEMU)
    CONFIG_QEMU_IN = $(CONFIG_QEMU).in

    CONFIG_PATH_QEMU = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU)

    SYSCONFIG_QEMU = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU)

    CONFIGS += $(CONFIG_QEMU)

    # qemu-specific options (all should be suffixed by "_QEMU")
    DEFBLOCKSTORAGEDRIVER_QEMU := virtio-scsi
    DEFNETWORKMODEL_QEMU := tcfilter
    KERNELTYPE = uncompressed
    KERNELNAME = $(call MAKE_KERNEL_NAME,$(KERNELTYPE))
    KERNELPATH = $(KERNELDIR)/$(KERNELNAME)
endif

ifneq (,$(QEMUVIRTIOFSCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_QEMU_VIRTIOFS)

    CONFIG_FILE_QEMU_VIRTIOFS = configuration-qemu-virtiofs.toml
    CONFIG_QEMU_VIRTIOFS = $(CLI_DIR)/config/$(CONFIG_FILE_QEMU_VIRTIOFS)
    CONFIG_QEMU_VIRTIOFS_IN = $(CONFIG_QEMU_VIRTIOFS).in

    CONFIG_PATH_QEMU_VIRTIOFS = $(abspath $(CONFDIR)/$(CONFIG_FILE_QEMU_VIRTIOFS))
    CONFIG_PATHS += $(CONFIG_PATH_QEMU_VIRTIOFS)

    SYSCONFIG_QEMU_VIRTIOFS = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_QEMU_VIRTIOFS))
    SYSCONFIG_PATHS += $(SYSCONFIG_QEMU_VIRTIOFS)

    CONFIGS += $(CONFIG_QEMU_VIRTIOFS)

    # qemu-specific options (all should be suffixed by "_QEMU")
    DEFBLOCKSTORAGEDRIVER_QEMU_VIRTIOFS := virtio-fs
    DEFNETWORKMODEL_QEMU := tcfilter
    KERNELNAMEVIRTIOFS = $(call MAKE_KERNEL_VIRTIOFS_NAME,$(KERNELTYPE))
    KERNELVIRTIOFSPATH = $(KERNELDIR)/$(KERNELNAMEVIRTIOFS)
endif

ifneq (,$(CLHCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_CLH)

    CONFIG_FILE_CLH = configuration-clh.toml
    CONFIG_CLH = $(CLI_DIR)/config/$(CONFIG_FILE_CLH)
    CONFIG_CLH_IN = $(CONFIG_CLH).in

    CONFIG_PATH_CLH = $(abspath $(CONFDIR)/$(CONFIG_FILE_CLH))
    CONFIG_PATHS += $(CONFIG_PATH_CLH)

    SYSCONFIG_CLH = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_CLH))
    SYSCONFIG_PATHS += $(SYSCONFIG_CLH)

    CONFIGS += $(CONFIG_CLH)

    # CLH-specific options (all should be suffixed by "_CLH")
    # currently, huge pages are required for virtiofsd support
    DEFENABLEHUGEPAGES_CLH := true
    DEFNETWORKMODEL_CLH := tcfilter
    KERNELTYPE_CLH = uncompressed
    KERNEL_NAME_CLH = $(call MAKE_KERNEL_VIRTIOFS_NAME,$(KERNELTYPE_CLH))
    KERNELPATH_CLH = $(KERNELDIR)/$(KERNEL_NAME_CLH)
endif

ifneq (,$(FCCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_FC)

    CONFIG_FILE_FC = configuration-fc.toml
    CONFIG_FC = $(CLI_DIR)/config/$(CONFIG_FILE_FC)
    CONFIG_FC_IN = $(CONFIG_FC).in

    CONFIG_PATH_FC = $(abspath $(CONFDIR)/$(CONFIG_FILE_FC))
    CONFIG_PATHS += $(CONFIG_PATH_FC)

    SYSCONFIG_FC = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_FC))
    SYSCONFIG_PATHS += $(SYSCONFIG_FC)

    CONFIGS += $(CONFIG_FC)

    # firecracker-specific options (all should be suffixed by "_FC")
    DEFBLOCKSTORAGEDRIVER_FC := virtio-mmio
    DEFNETWORKMODEL_FC := tcfilter
    KERNELTYPE_FC = uncompressed
    KERNEL_NAME_FC = $(call MAKE_KERNEL_NAME,$(KERNELTYPE_FC))
    KERNELPATH_FC = $(KERNELDIR)/$(KERNEL_NAME_FC)
endif

ifneq (,$(ACRNCMD))
    KNOWN_HYPERVISORS += $(HYPERVISOR_ACRN)

    CONFIG_FILE_ACRN = configuration-acrn.toml
    CONFIG_ACRN = $(CLI_DIR)/config/$(CONFIG_FILE_ACRN)
    CONFIG_ACRN_IN = $(CONFIG_ACRN).in

    CONFIG_PATH_ACRN = $(abspath $(CONFDIR)/$(CONFIG_FILE_ACRN))
    CONFIG_PATHS += $(CONFIG_PATH_ACRN)

    SYSCONFIG_ACRN = $(abspath $(SYSCONFDIR)/$(CONFIG_FILE_ACRN))
    SYSCONFIG_PATHS += $(SYSCONFIG_ACRN)

    CONFIGS += $(CONFIG_ACRN)

    # acrn-specific options (all should be suffixed by "_ACRN")
    DEFMAXVCPUS_ACRN := 1
    DEFBLOCKSTORAGEDRIVER_ACRN := virtio-blk
    DEFNETWORKMODEL_ACRN := macvtap
    KERNEL_NAME_ACRN = $(call MAKE_KERNEL_NAME,$(KERNELTYPE))
    KERNELPATH_ACRN = $(KERNELDIR)/$(KERNEL_NAME_ACRN)
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

ifeq ($(DEFAULT_HYPERVISOR),$(HYPERVISOR_ACRN))
    DEFAULT_HYPERVISOR_CONFIG = $(CONFIG_FILE_ACRN)
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
USER_VARS += CONFIG_PATH
USER_VARS += DESTDIR
USER_VARS += DEFAULT_HYPERVISOR
USER_VARS += ACRNCMD
USER_VARS += ACRNCTLCMD
USER_VARS += ACRNPATH
USER_VARS += ACRNCTLPATH
USER_VARS += FCCMD
USER_VARS += FCPATH
USER_VARS += FCJAILERPATH
USER_VARS += SYSCONFIG
USER_VARS += IMAGENAME
USER_VARS += IMAGEPATH
USER_VARS += INITRDNAME
USER_VARS += INITRDPATH
USER_VARS += MACHINETYPE
USER_VARS += KERNELDIR
USER_VARS += KERNELTYPE
USER_VARS += KERNELTYPE_FC
USER_VARS += KERNELTYPE_ACRN
USER_VARS += KERNELTYPE_CLH
USER_VARS += FIRMWAREPATH
USER_VARS += MACHINEACCELERATORS
USER_VARS += CPUFEATURES
USER_VARS += DEFMACHINETYPE_CLH
USER_VARS += KERNELPARAMS
USER_VARS += LIBEXECDIR
USER_VARS += LOCALSTATEDIR
USER_VARS += PKGDATADIR
USER_VARS += PKGLIBDIR
USER_VARS += PKGLIBEXECDIR
USER_VARS += PKGRUNDIR
USER_VARS += PREFIX
USER_VARS += PROJECT_NAME
USER_VARS += PROJECT_PREFIX
USER_VARS += PROJECT_TYPE
USER_VARS += NETMONPATH
USER_VARS += QEMUBINDIR
USER_VARS += QEMUCMD
USER_VARS += QEMUPATH
USER_VARS += QEMUVIRTIOFSCMD
USER_VARS += QEMUVIRTIOFSPATH
USER_VARS += SHAREDIR
USER_VARS += SHIMPATH
USER_VARS += SYSCONFDIR
USER_VARS += DEFVCPUS
USER_VARS += DEFMAXVCPUS
USER_VARS += DEFMAXVCPUS_ACRN
USER_VARS += DEFMEMSZ
USER_VARS += DEFMEMSLOTS
USER_VARS += DEFBRIDGES
USER_VARS += DEFNETWORKMODEL_ACRN
USER_VARS += DEFNETWORKMODEL_FC
USER_VARS += DEFNETWORKMODEL_QEMU
USER_VARS += DEFDISABLEGUESTSECCOMP
USER_VARS += DEFAULTEXPFEATURES
USER_VARS += DEFDISABLEBLOCK
USER_VARS += DEFBLOCKSTORAGEDRIVER_ACRN
USER_VARS += DEFBLOCKSTORAGEDRIVER_FC
USER_VARS += DEFBLOCKSTORAGEDRIVER_QEMU
USER_VARS += DEFBLOCKSTORAGEDRIVER_QEMU_VIRTIOFS
USER_VARS += DEFSHAREDFS
USER_VARS += DEFSHAREDFS_QEMU_VIRTIOFS
USER_VARS += DEFVIRTIOFSDAEMON
USER_VARS += DEFVIRTIOFSCACHESIZE
USER_VARS += DEFVIRTIOFSCACHE
USER_VARS += DEFVIRTIOFSEXTRAARGS
USER_VARS += DEFENABLEIOTHREADS
USER_VARS += DEFENABLEMEMPREALLOC
USER_VARS += DEFENABLEHUGEPAGES
USER_VARS += DEFENABLEVHOSTUSERSTORE
USER_VARS += DEFVHOSTUSERSTOREPATH
USER_VARS += DEFENABLESWAP
USER_VARS += DEFENABLEDEBUG
USER_VARS += DEFDISABLENESTINGCHECKS
USER_VARS += DEFMSIZE9P
USER_VARS += DEFHOTPLUGVFIOONROOTBUS
USER_VARS += DEFPCIEROOTPORT
USER_VARS += DEFENTROPYSOURCE
USER_VARS += DEFSANDBOXCGROUPONLY
USER_VARS += FEATURE_SELINUX
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

ifneq ($(FEATURE_SELINUX),no)
    SELINUXTAG := $(shell ./hack/selinux_tag.sh)

    ifneq ($(SELINUXTAG),)
        override FEATURE_SELINUX = yes
        BUILDTAGS += --tags "$(SELINUXTAG)"
    else
        ifeq ($(FEATURE_SELINUX),yes)
            $(error "ERROR: SELinux support requested, but libselinux is not available")
        endif

        override FEATURE_SELINUX = no
    endif
endif

# go build common flags
BUILDFLAGS := -buildmode=pie -mod=vendor ${BUILDTAGS}

# whether stipping the binary
ifeq ($(STRIP),yes)
       KATA_LDFLAGS := -ldflags "-w -s"
endif

# Return non-empty string if specified directory exists
define DIR_EXISTS
$(shell test -d $(1) && echo "$(1)")
endef

# $1: name of architecture to display
define SHOW_ARCH
  $(shell printf "\\t%s%s\\\n" "$(1)" $(if $(filter $(ARCH),$(1))," (default)",""))
endef

all: runtime containerd-shim-v2 netmon monitor

# Targets that depend on .git-commit can use $(shell cat .git-commit) to get a
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

netmon: $(NETMON_TARGET_OUTPUT)

$(NETMON_TARGET_OUTPUT): $(SOURCES) VERSION
	$(QUIET_BUILD)(cd $(NETMON_DIR) && go build $(BUILDFLAGS) -o $@ -ldflags "-X main.version=$(VERSION)" $(KATA_LDFLAGS))

runtime: $(TARGET_OUTPUT) $(CONFIGS)
.DEFAULT: default

build: default

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

GENERATED_CONFIG = $(abspath $(CLI_DIR)/config-generated.go)

GENERATED_FILES += $(GENERATED_CONFIG)
GENERATED_FILES += pkg/katautils/config-settings.go

$(TARGET_OUTPUT): $(SOURCES) $(GENERATED_FILES) $(MAKEFILE_LIST) | show-summary
	$(QUIET_BUILD)(cd $(CLI_DIR) && go build $(KATA_LDFLAGS) $(BUILDFLAGS) -o $@ .)

$(SHIMV2_OUTPUT): $(SOURCES) $(GENERATED_FILES) $(MAKEFILE_LIST)
	$(QUIET_BUILD)(cd $(SHIMV2_DIR)/ && ln -fs $(GENERATED_CONFIG))
	$(QUIET_BUILD)(cd $(SHIMV2_DIR)/ && go build $(KATA_LDFLAGS) $(BUILDFLAGS) -o $@ .)

$(MONITOR_OUTPUT): $(SOURCES) $(GENERATED_FILES) $(MAKEFILE_LIST)
	$(QUIET_BUILD)(cd $(MONITOR_DIR)/ && go build $(KATA_LDFLAGS) $(BUILDFLAGS) -o $@ .)

.PHONY: \
	check \
	check-go-static \
	coverage \
	default \
	install \
	show-header \
	show-summary \
	show-variables

$(TARGET).coverage: $(SOURCES) $(GENERATED_FILES) $(MAKEFILE_LIST)
	$(QUIET_TEST)go test -o $@ -covermode count

GENERATED_FILES += $(CONFIGS)

$(GENERATED_FILES): %: %.in $(MAKEFILE_LIST) VERSION .git-commit
	$(QUIET_GENERATE)$(SED) \
		-e "s|@COMMIT@|$(shell cat .git-commit)|g" \
		-e "s|@VERSION@|$(VERSION)|g" \
		-e "s|@CONFIG_ACRN_IN@|$(CONFIG_ACRN_IN)|g" \
		-e "s|@CONFIG_QEMU_IN@|$(CONFIG_QEMU_IN)|g" \
		-e "s|@CONFIG_QEMU_VIRTIOFS_IN@|$(CONFIG_QEMU_VIRTIOFS_IN)|g" \
		-e "s|@CONFIG_CLH_IN@|$(CONFIG_CLH_IN)|g" \
		-e "s|@CONFIG_FC_IN@|$(CONFIG_FC_IN)|g" \
		-e "s|@CONFIG_PATH@|$(CONFIG_PATH)|g" \
		-e "s|@FCPATH@|$(FCPATH)|g" \
		-e "s|@FCJAILERPATH@|$(FCJAILERPATH)|g" \
		-e "s|@ACRNPATH@|$(ACRNPATH)|g" \
		-e "s|@ACRNCTLPATH@|$(ACRNCTLPATH)|g" \
		-e "s|@CLHPATH@|$(CLHPATH)|g" \
		-e "s|@SYSCONFIG@|$(SYSCONFIG)|g" \
		-e "s|@IMAGEPATH@|$(IMAGEPATH)|g" \
		-e "s|@KERNELPATH_ACRN@|$(KERNELPATH_ACRN)|g" \
		-e "s|@KERNELPATH_FC@|$(KERNELPATH_FC)|g" \
		-e "s|@KERNELPATH_CLH@|$(KERNELPATH_CLH)|g" \
		-e "s|@KERNELPATH@|$(KERNELPATH)|g" \
		-e "s|@KERNELVIRTIOFSPATH@|$(KERNELVIRTIOFSPATH)|g" \
		-e "s|@INITRDPATH@|$(INITRDPATH)|g" \
		-e "s|@FIRMWAREPATH@|$(FIRMWAREPATH)|g" \
		-e "s|@MACHINEACCELERATORS@|$(MACHINEACCELERATORS)|g" \
		-e "s|@CPUFEATURES@|$(CPUFEATURES)|g" \
		-e "s|@FIRMWAREPATH_CLH@|$(FIRMWAREPATH_CLH)|g" \
		-e "s|@DEFMACHINETYPE_CLH@|$(DEFMACHINETYPE_CLH)|g" \
		-e "s|@KERNELPARAMS@|$(KERNELPARAMS)|g" \
		-e "s|@LOCALSTATEDIR@|$(LOCALSTATEDIR)|g" \
		-e "s|@PKGLIBEXECDIR@|$(PKGLIBEXECDIR)|g" \
		-e "s|@PKGRUNDIR@|$(PKGRUNDIR)|g" \
		-e "s|@NETMONPATH@|$(NETMONPATH)|g" \
		-e "s|@PROJECT_BUG_URL@|$(PROJECT_BUG_URL)|g" \
		-e "s|@PROJECT_ORG@|$(PROJECT_ORG)|g" \
		-e "s|@PROJECT_URL@|$(PROJECT_URL)|g" \
		-e "s|@PROJECT_NAME@|$(PROJECT_NAME)|g" \
		-e "s|@PROJECT_TAG@|$(PROJECT_TAG)|g" \
		-e "s|@PROJECT_TYPE@|$(PROJECT_TYPE)|g" \
		-e "s|@QEMUPATH@|$(QEMUPATH)|g" \
		-e "s|@QEMUVIRTIOFSPATH@|$(QEMUVIRTIOFSPATH)|g" \
		-e "s|@RUNTIME_NAME@|$(TARGET)|g" \
		-e "s|@MACHINETYPE@|$(MACHINETYPE)|g" \
		-e "s|@SHIMPATH@|$(SHIMPATH)|g" \
		-e "s|@DEFVCPUS@|$(DEFVCPUS)|g" \
		-e "s|@DEFMAXVCPUS@|$(DEFMAXVCPUS)|g" \
		-e "s|@DEFMAXVCPUS_ACRN@|$(DEFMAXVCPUS_ACRN)|g" \
		-e "s|@DEFMEMSZ@|$(DEFMEMSZ)|g" \
		-e "s|@DEFMEMSLOTS@|$(DEFMEMSLOTS)|g" \
		-e "s|@DEFBRIDGES@|$(DEFBRIDGES)|g" \
		-e "s|@DEFNETWORKMODEL_ACRN@|$(DEFNETWORKMODEL_ACRN)|g" \
		-e "s|@DEFNETWORKMODEL_CLH@|$(DEFNETWORKMODEL_CLH)|g" \
		-e "s|@DEFNETWORKMODEL_FC@|$(DEFNETWORKMODEL_FC)|g" \
		-e "s|@DEFNETWORKMODEL_QEMU@|$(DEFNETWORKMODEL_QEMU)|g" \
		-e "s|@DEFDISABLEGUESTSECCOMP@|$(DEFDISABLEGUESTSECCOMP)|g" \
		-e "s|@DEFAULTEXPFEATURES@|$(DEFAULTEXPFEATURES)|g" \
		-e "s|@DEFDISABLEBLOCK@|$(DEFDISABLEBLOCK)|g" \
		-e "s|@DEFBLOCKSTORAGEDRIVER_ACRN@|$(DEFBLOCKSTORAGEDRIVER_ACRN)|g" \
		-e "s|@DEFBLOCKSTORAGEDRIVER_FC@|$(DEFBLOCKSTORAGEDRIVER_FC)|g" \
		-e "s|@DEFBLOCKSTORAGEDRIVER_QEMU@|$(DEFBLOCKSTORAGEDRIVER_QEMU)|g" \
		-e "s|@DEFBLOCKSTORAGEDRIVER_QEMU_VIRTIOFS@|$(DEFBLOCKSTORAGEDRIVER_QEMU_VIRTIOFS)|g" \
		-e "s|@DEFSHAREDFS@|$(DEFSHAREDFS)|g" \
		-e "s|@DEFSHAREDFS_QEMU_VIRTIOFS@|$(DEFSHAREDFS_QEMU_VIRTIOFS)|g" \
		-e "s|@DEFVIRTIOFSDAEMON@|$(DEFVIRTIOFSDAEMON)|g" \
		-e "s|@DEFVIRTIOFSCACHESIZE@|$(DEFVIRTIOFSCACHESIZE)|g" \
		-e "s|@DEFVIRTIOFSCACHE@|$(DEFVIRTIOFSCACHE)|g" \
		-e "s|@DEFVIRTIOFSEXTRAARGS@|$(DEFVIRTIOFSEXTRAARGS)|g" \
		-e "s|@DEFENABLEIOTHREADS@|$(DEFENABLEIOTHREADS)|g" \
		-e "s|@DEFENABLEMEMPREALLOC@|$(DEFENABLEMEMPREALLOC)|g" \
		-e "s|@DEFENABLEHUGEPAGES@|$(DEFENABLEHUGEPAGES)|g" \
		-e "s|@DEFENABLEVHOSTUSERSTORE@|$(DEFENABLEVHOSTUSERSTORE)|g" \
		-e "s|@DEFVHOSTUSERSTOREPATH@|$(DEFVHOSTUSERSTOREPATH)|g" \
		-e "s|@DEFENABLEMSWAP@|$(DEFENABLESWAP)|g" \
		-e "s|@DEFENABLEDEBUG@|$(DEFENABLEDEBUG)|g" \
		-e "s|@DEFDISABLENESTINGCHECKS@|$(DEFDISABLENESTINGCHECKS)|g" \
		-e "s|@DEFMSIZE9P@|$(DEFMSIZE9P)|g" \
		-e "s|@DEFHOTPLUGVFIOONROOTBUS@|$(DEFHOTPLUGVFIOONROOTBUS)|g" \
		-e "s|@DEFPCIEROOTPORT@|$(DEFPCIEROOTPORT)|g" \
		-e "s|@DEFENTROPYSOURCE@|$(DEFENTROPYSOURCE)|g" \
		-e "s|@DEFSANDBOXCGROUPONLY@|$(DEFSANDBOXCGROUPONLY)|g" \
		-e "s|@FEATURE_SELINUX@|$(FEATURE_SELINUX)|g" \
		$< > $@

generate-config: $(CONFIGS)

check: check-go-static

test: go-test

go-test: $(GENERATED_FILES)
	go test -v -mod=vendor ./...

check-go-static:
	$(QUIET_CHECK)../../ci/static-checks.sh
	$(QUIET_CHECK)../../ci/go-no-os-exit.sh ./cli
	$(QUIET_CHECK)../../ci/go-no-os-exit.sh ./virtcontainers

coverage:
	go test -v -mod=vendor -covermode=atomic -coverprofile=coverage.txt ./...
	go tool cover -html=coverage.txt -o coverage.html

install: default install-runtime install-containerd-shim-v2 install-monitor install-netmon

install-bin: $(BINLIST)
	$(QUIET_INST)$(foreach f,$(BINLIST),$(call INSTALL_EXEC,$f,$(BINDIR)))

install-runtime: runtime install-scripts install-completions install-configs install-bin

install-netmon: install-bin-libexec

install-containerd-shim-v2: $(SHIMV2)
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

clean:
	$(QUIET_CLEAN)rm -f \
		$(CONFIGS) \
		$(GENERATED_FILES) \
		$(NETMON_TARGET) \
		$(MONITOR) \
		$(SHIMV2) \
		$(SHIMV2_DIR)/$(notdir $(GENERATED_CONFIG)) \
		$(TARGET) \
		.git-commit .git-commit.tmp

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
	@printf "\tcheck                      : run tests.\n"
	@printf "\tclean                      : remove built files.\n"
	@printf "\tcontainerd-shim-v2         : only build containerd shim v2.\n"
	@printf "\tcoverage                   : run coverage tests.\n"
	@printf "\tdefault                    : same as 'make build' (or just 'make').\n"
	@printf "\tgenerate-config            : create configuration file.\n"
	@printf "\tinstall                    : install everything.\n"
	@printf "\tinstall-containerd-shim-v2 : only install containerd shim v2 files.\n"
	@printf "\tinstall-netmon             : only install netmon files.\n"
	@printf "\tinstall-runtime            : only install runtime files.\n"
	@printf "\tnetmon                     : only build netmon.\n"
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
	@printf "%s - version %s (commit %s)\n\n" $(TARGET) $(VERSION) $(shell cat .git-commit)

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
	@printf "• Features:\n"
	@printf "\tSELinux (FEATURE_SELINUX): $(FEATURE_SELINUX)\n"
	@printf "\n"
	@printf "• Summary:\n"
	@printf "\n"
	@printf "\tdestination install path (DESTDIR) : %s\n" $(abspath $(DESTDIR))
	@printf "\tbinary installation path (BINDIR) : %s\n" $(abspath $(BINDIR))
	@printf "\tbinaries to install :\n"
	@printf \
          "$(foreach b,$(sort $(BINLIST)),$(shell printf "\\t - $(shell readlink -m $(DESTDIR)/$(BINDIR)/$(b))\\\n"))"
	@printf \
          "$(foreach b,$(sort $(SHIMV2)),$(shell printf "\\t - $(shell readlink -m $(DESTDIR)/$(BINDIR)/$(b))\\\n"))"
	@printf \
          "$(foreach b,$(sort $(MONITOR)),$(shell printf "\\t - $(shell readlink -m $(DESTDIR)/$(BINDIR)/$(b))\\\n"))"
	@printf \
          "$(foreach b,$(sort $(BINLIBEXECLIST)),$(shell printf "\\t - $(shell readlink -m $(DESTDIR)/$(PKGLIBEXECDIR)/$(b))\\\n"))"
	@printf \
          "$(foreach s,$(sort $(SCRIPTS)),$(shell printf "\\t - $(shell readlink -m $(DESTDIR)/$(BINDIR)/$(s))\\\n"))"
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
ifneq (,$(findstring $(HYPERVISOR_ACRN),$(KNOWN_HYPERVISORS)))
	@printf "\t$(HYPERVISOR_ACRN) hypervisor path (ACRNPATH) : %s\n" $(abspath $(ACRNPATH))
endif
	@printf "\tassets path (PKGDATADIR) : %s\n" $(abspath $(PKGDATADIR))
	@printf "\tshim path (PKGLIBEXECDIR) : %s\n" $(abspath $(PKGLIBEXECDIR))
	@printf "\n"
