#
# Copyright (c) 2018 Intel Corporation
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

GOARCH=$(shell go env GOARCH)
HOST_ARCH=$(shell arch)

ifeq ($(ARCH),)
	ARCH = $(GOARCH)
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
PROJECT_URL = https://github.com/kata-containers
PROJECT_BUG_URL = $(PROJECT_URL)/kata-containers/issues/new

BIN_PREFIX = $(PROJECT_TYPE)
PROJECT_DIR = $(PROJECT_TAG)
IMAGENAME = $(PROJECT_TAG).img
INITRDNAME = $(PROJECT_TAG)-initrd.img

TARGET = $(BIN_PREFIX)-runtime
TARGET_OUTPUT = $(CURDIR)/$(TARGET)

DESTDIR :=

installing = $(findstring install,$(MAKECMDGOALS))

# Configure the build for a standard system that is
# using OBS-generated packages.
PREFIX        := /usr
BINDIR        := $(PREFIX)/bin
DESTBINDIR    := /usr/local/bin
QEMUBINDIR    := $(BINDIR)
SYSCONFDIR    := /etc
LOCALSTATEDIR := /var

ifeq (,$(installing))
    # Force a rebuild to ensure version details are correct
    # (but only for a non-install build phase).
    EXTRA_DEPS = clean
endif

LIBEXECDIR := $(PREFIX)/libexec
SHAREDIR := $(PREFIX)/share
DEFAULTSDIR := $(SHAREDIR)/defaults

COLLECT_SCRIPT = data/kata-collect-data.sh
COLLECT_SCRIPT_SRC = $(COLLECT_SCRIPT).in

GENERATED_FILES += $(COLLECT_SCRIPT)
SCRIPTS += $(COLLECT_SCRIPT)
SCRIPTS_DIR := $(BINDIR)

PKGDATADIR := $(SHAREDIR)/$(PROJECT_DIR)
PKGLIBDIR := $(LOCALSTATEDIR)/lib/$(PROJECT_DIR)
PKGRUNDIR := $(LOCALSTATEDIR)/run/$(PROJECT_DIR)
PKGLIBEXECDIR := $(LIBEXECDIR)/$(PROJECT_DIR)

KERNELPATH := $(PKGDATADIR)/vmlinuz.container
INITRDPATH := $(PKGDATADIR)/$(INITRDNAME)
IMAGEPATH := $(PKGDATADIR)/$(IMAGENAME)
FIRMWAREPATH :=

QEMUPATH := $(QEMUBINDIR)/$(QEMUCMD)

SHIMCMD := $(BIN_PREFIX)-shim
SHIMPATH := $(PKGLIBEXECDIR)/$(SHIMCMD)

PROXYCMD := $(BIN_PREFIX)-proxy
PROXYPATH := $(PKGLIBEXECDIR)/$(PROXYCMD)

# Default number of vCPUs
DEFVCPUS := 1
# Default memory size in MiB
DEFMEMSZ := 2048
#Default number of bridges
DEFBRIDGES := 1
#Default network model
DEFNETWORKMODEL := macvtap

DEFDISABLEBLOCK := false
DEFBLOCKSTORAGEDRIVER := virtio-scsi
DEFENABLEIOTHREADS := false
DEFENABLEMEMPREALLOC := false
DEFENABLEHUGEPAGES := false
DEFENABLESWAP := false
DEFENABLEDEBUG := false
DEFDISABLENESTINGCHECKS := false

SED = sed

CLI_DIR = cli
SOURCES := $(shell find . 2>&1 | grep -E '.*\.(c|h|go)$$')
VERSION := ${shell cat ./VERSION}
COMMIT_NO := $(shell git rev-parse HEAD 2> /dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no),${COMMIT_NO}-dirty,${COMMIT_NO})

CONFIG_FILE = configuration.toml
CONFIG = $(CLI_DIR)/config/$(CONFIG_FILE)
CONFIG_IN = $(CONFIG).in

DESTTARGET := $(abspath $(DESTBINDIR)/$(TARGET))

DESTCONFDIR := $(DESTDIR)/$(DEFAULTSDIR)/$(PROJECT_DIR)
DESTSYSCONFDIR := $(DESTDIR)/$(SYSCONFDIR)/$(PROJECT_DIR)

# Main configuration file location for stateless systems
DESTCONFIG := $(abspath $(DESTCONFDIR)/$(CONFIG_FILE))

# Secondary configuration file location. Note that this takes precedence
# over DESTCONFIG.
DESTSYSCONFIG := $(abspath $(DESTSYSCONFDIR)/$(CONFIG_FILE))

DESTSHAREDIR := $(DESTDIR)/$(SHAREDIR)

# list of variables the user may wish to override
USER_VARS += ARCH
USER_VARS += BINDIR
USER_VARS += DESTCONFIG
USER_VARS += DESTDIR
USER_VARS += DESTSYSCONFIG
USER_VARS += DESTTARGET
USER_VARS += IMAGENAME
USER_VARS += IMAGEPATH
USER_VARS += INITRDNAME
USER_VARS += INITRDPATH
USER_VARS += MACHINETYPE
USER_VARS += KERNELPATH
USER_VARS += FIRMWAREPATH
USER_VARS += MACHINEACCELERATORS
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
USER_VARS += PROXYPATH
USER_VARS += QEMUBINDIR
USER_VARS += QEMUCMD
USER_VARS += QEMUPATH
USER_VARS += SHAREDIR
USER_VARS += SHIMPATH
USER_VARS += SYSCONFDIR
USER_VARS += DEFVCPUS
USER_VARS += DEFMEMSZ
USER_VARS += DEFBRIDGES
USER_VARS += DEFNETWORKMODEL
USER_VARS += DEFDISABLEBLOCK
USER_VARS += DEFBLOCKSTORAGEDRIVER
USER_VARS += DEFENABLEIOTHREADS
USER_VARS += DEFENABLEMEMPREALLOC
USER_VARS += DEFENABLEHUGEPAGES
USER_VARS += DEFENABLESWAP
USER_VARS += DEFENABLEDEBUG
USER_VARS += DEFDISABLENESTINGCHECKS

V              = @
Q              = $(V:1=)
QUIET_BUILD    = $(Q:@=@echo    '     BUILD   '$@;)
QUIET_CHECK    = $(Q:@=@echo    '     CHECK   '$@;)
QUIET_CLEAN    = $(Q:@=@echo    '     CLEAN   '$@;)
QUIET_CONFIG   = $(Q:@=@echo    '     CONFIG  '$@;)
QUIET_GENERATE = $(Q:@=@echo    '     GENERATE '$@;)
QUIET_INST     = $(Q:@=@echo    '     INSTALL '$@;)
QUIET_TEST     = $(Q:@=@echo    '     TEST    '$@;)

# Return non-empty string if specified directory exists
define DIR_EXISTS
$(shell test -d $(1) && echo "$(1)")
endef

# $1: name of architecture to display
define SHOW_ARCH
  $(shell printf "\\t%s%s\\\n" "$(1)" $(if $(filter $(ARCH),$(1))," (default)",""))
endef

all: runtime

runtime: $(TARGET_OUTPUT) $(CONFIG)
.DEFAULT: default

build: default

define GENERATED_CODE
// WARNING: This file is auto-generated - DO NOT EDIT!
//
// Note that some variables are "var" to allow them to be modified
// by the tests.
package main

import (
	"fmt"
)

// name is the name of the runtime
const name = "$(TARGET)"

// name of the project
const project = "$(PROJECT_NAME)"

// prefix used to denote non-standard CLI commands and options.
const projectPrefix = "$(PROJECT_TYPE)"

// systemdUnitName is the systemd(1) target used to launch the agent.
const systemdUnitName = "$(PROJECT_TAG).target"

// original URL for this project
const projectURL = "$(PROJECT_URL)"

// commit is the git commit the runtime is compiled from.
var commit = "$(COMMIT)"

// version is the runtime version.
var version = "$(VERSION)"

// project-specific command names
var envCmd = fmt.Sprintf("%s-env", projectPrefix)
var checkCmd = fmt.Sprintf("%s-check", projectPrefix)

// project-specific option names
var configFilePathOption = fmt.Sprintf("%s-config", projectPrefix)
var showConfigPathsOption = fmt.Sprintf("%s-show-default-config-paths", projectPrefix)

var defaultHypervisorPath = "$(QEMUPATH)"
var defaultImagePath = "$(IMAGEPATH)"
var defaultKernelPath = "$(KERNELPATH)"
var defaultInitrdPath = "$(INITRDPATH)"
var defaultFirmwarePath = "$(FIRMWAREPATH)"
var defaultMachineAccelerators = "$(MACHINEACCELERATORS)"
var defaultShimPath = "$(SHIMPATH)"

const defaultKernelParams = "$(KERNELPARAMS)"
const defaultMachineType = "$(MACHINETYPE)"
const defaultRootDirectory = "$(PKGRUNDIR)"

const defaultVCPUCount uint32 = $(DEFVCPUS)
const defaultMemSize uint32 = $(DEFMEMSZ) // MiB
const defaultBridgesCount uint32 = $(DEFBRIDGES)
const defaultInterNetworkingModel = "$(DEFNETWORKMODEL)"
const defaultDisableBlockDeviceUse bool = $(DEFDISABLEBLOCK)
const defaultBlockDeviceDriver = "$(DEFBLOCKSTORAGEDRIVER)"
const defaultEnableIOThreads bool = $(DEFENABLEIOTHREADS)
const defaultEnableMemPrealloc bool = $(DEFENABLEMEMPREALLOC)
const defaultEnableHugePages bool = $(DEFENABLEHUGEPAGES)
const defaultEnableSwap bool = $(DEFENABLESWAP)
const defaultEnableDebug bool = $(DEFENABLEDEBUG)
const defaultDisableNestingChecks bool = $(DEFDISABLENESTINGCHECKS)

// Default config file used by stateless systems.
var defaultRuntimeConfiguration = "$(DESTCONFIG)"

// Alternate config file that takes precedence over
// defaultRuntimeConfiguration.
var defaultSysConfRuntimeConfiguration = "$(DESTSYSCONFIG)"

var defaultProxyPath = "$(PROXYPATH)"
endef

export GENERATED_CODE

#Install an executable file
# params:
# $1 : file to install
# $2 : directory path where file will be installed
define INSTALL_EXEC
	$(QUIET_INST)install -D $1 $(DESTDIR)$2/$(notdir $1);
endef

GENERATED_CONFIG = $(CLI_DIR)/config-generated.go

GENERATED_GO_FILES += $(GENERATED_CONFIG)

$(GENERATED_CONFIG): Makefile VERSION
	$(QUIET_GENERATE)echo "$$GENERATED_CODE" >$@

$(TARGET_OUTPUT): $(EXTRA_DEPS) $(SOURCES) $(GENERATED_GO_FILES) $(GENERATED_FILES) Makefile | show-summary
	$(QUIET_BUILD)(cd $(CLI_DIR) && go build -i -o $@ .)

.PHONY: \
	check \
	check-go-static \
	check-go-test \
	coverage \
	default \
	install \
	show-header \
	show-summary \
	show-variables

$(TARGET).coverage: $(SOURCES) $(GENERATED_FILES) Makefile
	$(QUIET_TEST)go test -o $@ -covermode count

GENERATED_FILES += $(CONFIG)

$(GENERATED_FILES): %: %.in Makefile VERSION
	$(QUIET_CONFIG)$(SED) \
		-e "s|@COMMIT@|$(COMMIT)|g" \
		-e "s|@VERSION@|$(VERSION)|g" \
		-e "s|@CONFIG_IN@|$(CONFIG_IN)|g" \
		-e "s|@DESTCONFIG@|$(DESTCONFIG)|g" \
		-e "s|@DESTSYSCONFIG@|$(DESTSYSCONFIG)|g" \
		-e "s|@IMAGEPATH@|$(IMAGEPATH)|g" \
		-e "s|@KERNELPATH@|$(KERNELPATH)|g" \
		-e "s|@INITRDPATH@|$(INITRDPATH)|g" \
		-e "s|@FIRMWAREPATH@|$(FIRMWAREPATH)|g" \
		-e "s|@MACHINEACCELERATORS@|$(MACHINEACCELERATORS)|g" \
		-e "s|@KERNELPARAMS@|$(KERNELPARAMS)|g" \
		-e "s|@LOCALSTATEDIR@|$(LOCALSTATEDIR)|g" \
		-e "s|@PKGLIBEXECDIR@|$(PKGLIBEXECDIR)|g" \
		-e "s|@PROXYPATH@|$(PROXYPATH)|g" \
		-e "s|@PROJECT_BUG_URL@|$(PROJECT_BUG_URL)|g" \
		-e "s|@PROJECT_URL@|$(PROJECT_URL)|g" \
		-e "s|@PROJECT_NAME@|$(PROJECT_NAME)|g" \
		-e "s|@PROJECT_TAG@|$(PROJECT_TAG)|g" \
		-e "s|@PROJECT_TYPE@|$(PROJECT_TYPE)|g" \
		-e "s|@QEMUPATH@|$(QEMUPATH)|g" \
		-e "s|@RUNTIME_NAME@|$(TARGET)|g" \
		-e "s|@MACHINETYPE@|$(MACHINETYPE)|g" \
		-e "s|@SHIMPATH@|$(SHIMPATH)|g" \
		-e "s|@DEFVCPUS@|$(DEFVCPUS)|g" \
		-e "s|@DEFMEMSZ@|$(DEFMEMSZ)|g" \
		-e "s|@DEFBRIDGES@|$(DEFBRIDGES)|g" \
		-e "s|@DEFNETWORKMODEL@|$(DEFNETWORKMODEL)|g" \
		-e "s|@DEFDISABLEBLOCK@|$(DEFDISABLEBLOCK)|g" \
		-e "s|@DEFBLOCKSTORAGEDRIVER@|$(DEFBLOCKSTORAGEDRIVER)|g" \
		-e "s|@DEFENABLEIOTHREADS@|$(DEFENABLEIOTHREADS)|g" \
		-e "s|@DEFENABLEMEMPREALLOC@|$(DEFENABLEMEMPREALLOC)|g" \
		-e "s|@DEFENABLEHUGEPAGES@|$(DEFENABLEHUGEPAGES)|g" \
		-e "s|@DEFENABLEMSWAP@|$(DEFENABLESWAP)|g" \
		-e "s|@DEFENABLEDEBUG@|$(DEFENABLEDEBUG)|g" \
		-e "s|@DEFDISABLENESTINGCHECKS@|$(DEFDISABLENESTINGCHECKS)|g" \
		$< > $@

generate-config: $(CONFIG)

check: check-go-static

test: go-test

go-test: $(GENERATED_FILES)
	$(QUIET_TEST).ci/go-test.sh

check-go-static:
	$(QUIET_CHECK).ci/static-checks.sh
	$(QUIET_CHECK).ci/go-no-os-exit.sh

coverage:
	$(QUIET_TEST).ci/go-test.sh html-coverage

install: default install-scripts
	$(QUIET_INST)install -D $(TARGET) $(DESTTARGET)
	$(QUIET_INST)install -D $(CONFIG) $(DESTCONFIG)

install-scripts:
	$(foreach f,$(SCRIPTS),$(call INSTALL_EXEC,$f,$(SCRIPTS_DIR)))

clean:
	$(QUIET_CLEAN)rm -f $(TARGET) $(CONFIG) $(GENERATED_GO_FILES) $(GENERATED_FILES) $(COLLECT_SCRIPT)

show-usage: show-header
	@printf "• Overview:\n"
	@printf "\n"
	@printf "\tTo build $(TARGET), just run, \"make\".\n"
	@printf "\n"
	@printf "\tFor a verbose build, run \"make V=1\".\n"
	@printf "\n"
	@printf "• Additional targets:\n"
	@printf "\n"
	@printf "\tbuild               : standard build.\n"
	@printf "\tcheck               : run tests.\n"
	@printf "\tclean               : remove built files.\n"
	@printf "\tcoverage            : run coverage tests.\n"
	@printf "\tdefault             : same as 'make build' (or just 'make').\n"
	@printf "\tgenerate-config     : create configuration file.\n"
	@printf "\tinstall             : install files.\n"
	@printf "\tshow-arches         : show supported architectures (ARCH variable values).\n"
	@printf "\tshow-summary        : show install locations.\n"
	@printf "\n"

handle_help: show-usage show-summary show-variables show-footer

usage: handle_help
help: handle_help

show-variables:
	@printf "• Variables affecting the build:\n\n"
	@printf \
          "$(foreach v,$(sort $(USER_VARS)),$(shell printf "\\t$(v)='$($(v))'\\\n"))"
	@printf "\n"

show-header:
	@printf "%s - version %s (commit %s)\n\n" $(TARGET) $(VERSION) $(COMMIT)

show-arches: show-header
	@printf "Supported architectures (possible values for ARCH variable):\n\n"
	@printf \
		"$(foreach v,$(ALL_ARCHES),$(call SHOW_ARCH,$(v)))\n"

show-footer:
	@printf "• Project:\n"
	@printf "\tHome: $(PROJECT_URL)\n"
	@printf "\tBugs: $(PROJECT_BUG_URL)\n\n"

show-summary: show-header
	@printf "• architecture:\n"
	@printf "\tHost: $(HOST_ARCH)\n"
	@printf "\tgolang: $(GOARCH)\n"
	@printf "\tBuild: $(ARCH)\n"
	@printf "\n"
	@printf "• golang:\n"
	@printf "\t"
	@go version
	@printf "\n"
	@printf "• Summary:\n"
	@printf "\n"
	@printf "\tbinary install path (DESTTARGET)      : %s\n" $(DESTTARGET)
	@printf "\tconfig install path (DESTCONFIG)      : %s\n" $(DESTCONFIG)
	@printf "\talternate config path (DESTSYSCONFIG) : %s\n" $(DESTSYSCONFIG)
	@printf "\thypervisor path (QEMUPATH)            : %s\n" $(QEMUPATH)
	@printf "\tassets path (PKGDATADIR)              : %s\n" $(PKGDATADIR)
	@printf "\tproxy+shim path (PKGLIBEXECDIR)       : %s\n" $(PKGLIBEXECDIR)
	@printf "\n"
