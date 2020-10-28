#
# Copyright (c) 2018-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

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

TARGET = $(BIN_PREFIX)-runtime
TARGET_OUTPUT = $(CURDIR)/$(TARGET)
BINLIST += $(TARGET)

NETMON_DIR = netmon
NETMON_TARGET = $(PROJECT_TYPE)-netmon
NETMON_TARGET_OUTPUT = $(CURDIR)/$(NETMON_TARGET)
BINLIBEXECLIST += $(NETMON_TARGET)

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
ACRNBINDIR    := $(PREFIXDEPS)/bin
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
		CONFIG_ACRN_IN \
		CONFIG_QEMU_IN \
		CONFIG_CLH_IN \
		CONFIG_FC_IN \
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
FIRMWAREPATH :=

# Name of default configuration file the runtime will use.
CONFIG_FILE = configuration.toml

HYPERVISOR_ACRN = acrn
HYPERVISOR_FC = firecracker
HYPERVISOR_QEMU = qemu
HYPERVISOR_CLH = cloud-hypervisor

# Determines which hypervisor is specified in $(CONFIG_FILE).
DEFAULT_HYPERVISOR ?= $(HYPERVISOR_QEMU)

# List of hypervisors this build system can generate configuration for.
HYPERVISORS := $(HYPERVISOR_ACRN) $(HYPERVISOR_FC) $(HYPERVISOR_QEMU) $(HYPERVISOR_CLH)

QEMUPATH := $(QEMUBINDIR)/$(QEMUCMD)
QEMUVALIDHYPERVISORPATHS := [\"$(QEMUPATH)\"]

QEMUVIRTIOFSPATH := $(QEMUBINDIR)/$(QEMUVIRTIOFSCMD)

CLHPATH := $(CLHBINDIR)/$(CLHCMD)
CLHVALIDHYPERVISORPATHS := [\"$(CLHPATH)\"]

FCPATH = $(FCBINDIR)/$(FCCMD)
FCVALIDHYPERVISORPATHS := [\"$(FCPATH)\"]
FCJAILERPATH = $(FCBINDIR)/$(FCJAILERCMD)
FCVALIDJAILERPATHS = [\"$(FCJAILERPATH)\"]

ACRNPATH := $(ACRNBINDIR)/$(ACRNCMD)
ACRNVALIDHYPERVISORPATHS := [\"$(ACRNPATH)\"]
ACRNCTLPATH := $(ACRNBINDIR)/$(ACRNCTLCMD)
ACRNVALIDCTLPATHS := [\"$(ACRNCTLPATH)\"]

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
DEFENABLEANNOTATIONS := []
DEFDISABLEGUESTSECCOMP := true
#Default experimental features enabled
DEFAULTEXPFEATURES := []

#Default entropy source
DEFENTROPYSOURCE := /dev/urandom
DEFVALIDENTROPYSOURCES := [\"/dev/urandom\",\"/dev/random\",\"\"]

DEFDISABLEBLOCK := false
DEFSHAREDFS_QEMU_VIRTIOFS := virtio-fs
DEFVIRTIOFSDAEMON := $(LIBEXECDIR)/kata-qemu/virtiofsd
DEFVALIDVIRTIOFSDAEMONPATHS := [\"$(DEFVIRTIOFSDAEMON)\"]
# Default DAX mapping cache size in MiB
#if value is 0, DAX is not enabled
DEFVIRTIOFSCACHESIZE ?= 0
DEFVIRTIOFSCACHE ?= auto
# Format example:
#   [\"-o\", \"arg1=xxx,arg2\", \"-o\", \"hello world\", \"--arg3=yyy\"]
#
# see `virtiofsd -h` for possible options.
# Make sure you quote args.
DEFVIRTIOFSEXTRAARGS ?= [\"--thread-pool-size=1\"]
DEFENABLEIOTHREADS := false
DEFENABLEVHOSTUSERSTORE := false
DEFVHOSTUSERSTOREPATH := $(PKGRUNDIR)/vhost-user
DEFVALIDVHOSTUSERSTOREPATHS := [\"$(DEFVHOSTUSERSTOREPATH)\"]
DEFFILEMEMBACKEND := ""
DEFVALIDFILEMEMBACKENDS := [\"$(DEFFILEMEMBACKEND)\"]
DEFMSIZE9P := 8192

# Default cgroup model
DEFSANDBOXCGROUPONLY ?= false

DEFBINDMOUNTS := []

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
    DEFNETWORKMODEL_CLH := tcfilter
    KERNELTYPE_CLH = uncompressed
    KERNEL_NAME_CLH = $(call MAKE_KERNEL_NAME,$(KERNELTYPE_CLH))
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
USER_VARS += CONFIG_ACRN_IN
USER_VARS += CONFIG_CLH_IN
USER_VARS += CONFIG_FC_IN
USER_VARS += CONFIG_PATH
USER_VARS += CONFIG_QEMU_IN
USER_VARS += DESTDIR
USER_VARS += DEFAULT_HYPERVISOR
USER_VARS += DEFENABLEMSWAP
USER_VARS += ACRNCMD
USER_VARS += ACRNCTLCMD
USER_VARS += ACRNPATH
USER_VARS += ACRNVALIDHYPERVISORPATHS
USER_VARS += ACRNCTLPATH
USER_VARS += ACRNVALIDCTLPATHS
USER_VARS += CLHPATH
USER_VARS += CLHVALIDHYPERVISORPATHS
USER_VARS += FIRMWAREPATH_CLH
USER_VARS += FCCMD
USER_VARS += FCPATH
USER_VARS += FCVALIDHYPERVISORPATHS
USER_VARS += FCJAILERPATH
USER_VARS += FCVALIDJAILERPATHS
USER_VARS += SYSCONFIG
USER_VARS += IMAGENAME
USER_VARS += IMAGEPATH
USER_VARS += MACHINETYPE
USER_VARS += KERNELDIR
USER_VARS += KERNELTYPE
USER_VARS += KERNELTYPE_FC
USER_VARS += KERNELTYPE_ACRN
USER_VARS += KERNELTYPE_CLH
USER_VARS += KERNELPATH_ACRN
USER_VARS += KERNELPATH
USER_VARS += KERNELPATH_CLH
USER_VARS += KERNELPATH_FC
USER_VARS += KERNELVIRTIOFSPATH
USER_VARS += FIRMWAREPATH
USER_VARS += MACHINEACCELERATORS
USER_VARS += CPUFEATURES
USER_VARS += DEFMACHINETYPE_CLH
USER_VARS += KERNELPARAMS
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
USER_VARS += NETMONPATH
USER_VARS += QEMUBINDIR
USER_VARS += QEMUCMD
USER_VARS += QEMUPATH
USER_VARS += QEMUVALIDHYPERVISORPATHS
USER_VARS += QEMUVIRTIOFSCMD
USER_VARS += QEMUVIRTIOFSPATH
USER_VARS += RUNTIME_NAME
USER_VARS += SHAREDIR
USER_VARS += SYSCONFDIR
USER_VARS += DEFVCPUS
USER_VARS += DEFMAXVCPUS
USER_VARS += DEFMAXVCPUS_ACRN
USER_VARS += DEFMEMSZ
USER_VARS += DEFMEMSLOTS
USER_VARS += DEFBRIDGES
USER_VARS += DEFNETWORKMODEL_ACRN
USER_VARS += DEFNETWORKMODEL_CLH
USER_VARS += DEFNETWORKMODEL_FC
USER_VARS += DEFNETWORKMODEL_QEMU
USER_VARS += DEFDISABLEGUESTSECCOMP
USER_VARS += DEFAULTEXPFEATURES
USER_VARS += DEFDISABLEBLOCK
USER_VARS += DEFBLOCKSTORAGEDRIVER_ACRN
USER_VARS += DEFBLOCKSTORAGEDRIVER_FC
USER_VARS += DEFBLOCKSTORAGEDRIVER_QEMU
USER_VARS += DEFSHAREDFS_QEMU_VIRTIOFS
USER_VARS += DEFVIRTIOFSDAEMON
USER_VARS += DEFVALIDVIRTIOFSDAEMONPATHS
USER_VARS += DEFVIRTIOFSCACHESIZE
USER_VARS += DEFVIRTIOFSCACHE
USER_VARS += DEFVIRTIOFSEXTRAARGS
USER_VARS += DEFENABLEANNOTATIONS
USER_VARS += DEFENABLEIOTHREADS
USER_VARS += DEFENABLEVHOSTUSERSTORE
USER_VARS += DEFVHOSTUSERSTOREPATH
USER_VARS += DEFVALIDVHOSTUSERSTOREPATHS
USER_VARS += DEFFILEMEMBACKEND
USER_VARS += DEFVALIDFILEMEMBACKENDS
USER_VARS += DEFMSIZE9P
USER_VARS += DEFENTROPYSOURCE
USER_VARS += DEFVALIDENTROPYSOURCES
USER_VARS += DEFSANDBOXCGROUPONLY
USER_VARS += DEFBINDMOUNTS
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

$(MONITOR_OUTPUT): $(SOURCES) $(GENERATED_FILES) $(MAKEFILE_LIST) .git-commit
	$(QUIET_BUILD)(cd $(MONITOR_DIR)/ && CGO_ENABLED=0 go build \
		--ldflags "-X main.GitCommit=$(shell cat .git-commit)" $(BUILDFLAGS) -buildmode=exe -o $@ .)

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
		$(foreach v,$(GENERATED_VARS),-e "s|@$v@|$($v)|g") \
		$< > $@

generate-config: $(CONFIGS)

check: check-go-static

test: install-hook go-test

install-hook:
	make -C virtcontainers hook
ifeq ($(shell id -u), 0)
	echo "installing mock hook"
	make -C virtcontainers install
endif

go-test: $(GENERATED_FILES)
	go test -v -mod=vendor ./...

check-go-static:
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
	@printf "\ttest                       : run tests.\n"
	@printf "\tcheck                      : run code checks.\n"
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
