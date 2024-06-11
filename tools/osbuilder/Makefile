#
# Copyright (c) 2017 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
MK_DIR         := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
TEST_RUNNER    := $(MK_DIR)/tests/test_images.sh
ROOTFS_BUILDER := $(MK_DIR)/rootfs-builder/rootfs.sh
INITRD_BUILDER := $(MK_DIR)/initrd-builder/initrd_builder.sh
IMAGE_BUILDER  := $(MK_DIR)/image-builder/image_builder.sh

DISTRO                ?= ubuntu
BUILD_METHOD          := distro
BUILD_METHOD_LIST     := distro dracut
AGENT_INIT            ?= no
USE_DOCKER            ?= true
ROOTFS_BUILD_DEST     := $(shell pwd)
IMAGES_BUILD_DEST     := $(shell pwd)
ROOTFS_MARKER_SUFFIX   := _rootfs.done
TARGET_ROOTFS         := $(ROOTFS_BUILD_DEST)/$(DISTRO)_rootfs
TARGET_ROOTFS_MARKER  := $(ROOTFS_BUILD_DEST)/.$(DISTRO)$(ROOTFS_MARKER_SUFFIX)
TARGET_IMAGE          := $(IMAGES_BUILD_DEST)/kata-containers.img
TARGET_INITRD         := $(IMAGES_BUILD_DEST)/kata-containers-initrd.img

VERSION_FILE   := ./VERSION
VERSION        := $(shell grep -v ^\# $(VERSION_FILE) 2>/dev/null || echo "unknown")
COMMIT_NO      := $(shell git rev-parse HEAD 2>/dev/null || true)
COMMIT         := $(if $(shell git status --porcelain --untracked-files=no 2>/dev/null || true),${COMMIT_NO}-dirty,${COMMIT_NO})
VERSION_COMMIT := $(if $(COMMIT),$(VERSION)-$(COMMIT),$(VERSION))

ifeq ($(filter $(BUILD_METHOD),$(BUILD_METHOD_LIST)),)
    $(error Invalid BUILD_METHOD value '$(BUILD_METHOD)'. Supported values: $(BUILD_METHOD_LIST))
endif

ifeq (dracut,$(BUILD_METHOD))
  DISTRO                :=
  TARGET_ROOTFS         := dracut_rootfs
  TARGET_ROOTFS_MARKER  := $(ROOTFS_BUILD_DEST)/.dracut$(ROOTFS_MARKER_SUFFIX)
  # dracut specific variables
  DRACUT_KVERSION    :=
  DRACUT_OVERLAY_DIR := $(MK_DIR)/dracut_overlay
  DRACUT_DIR         := $(MK_DIR)/dracut
  DRACUT_CONF_DIR    := $(DRACUT_DIR)/dracut.conf.d
  DRACUT_OPTIONS     := --no-compress --conf /dev/null --confdir $(DRACUT_CONF_DIR)

  ifneq (,$(DRACUT_KVERSION))
    # Explicitly use bash, which is what dracut uses to process conf files
    DRACUT_KMODULES  := $(shell bash -c 'source $(DRACUT_CONF_DIR)/10-drivers.conf; echo "$$drivers"')
  else
    # If a kernel version is not specified, do not make systemd load modules
    # at startup
    DRACUT_OPTIONS += --no-kernel
  endif

  ifeq (no,$(AGENT_INIT))
    AGENT_PATH := $(DRACUT_OVERLAY_DIR)/usr/bin/kata-agent
  else
    AGENT_PATH := $(DRACUT_OVERLAY_DIR)/sbin/init
  endif

  ifeq (,$(DRACUT_OVERLAY_DIR))
    $(error DRACUT_OVERLAY_DIR cannot be empty)
  endif
endif

# Set the variable to silent logs using chronic
OSBUILDER_USE_CHRONIC :=

# silent_run allows running make recipes using the chronic wrapper, so logs are
# muted if the recipe command succeeds.
# Arguments:
# - Message
# - Command to run
ifeq (,$(OSBUILDER_USE_CHRONIC))
  define silent_run
    @echo $(1)
    $(2)
  endef
else
  define silent_run
    @echo $(1) with command: $(2)
    @chronic $(2)
  endef
endif

################################################################################

.PHONY: all
all: image initrd

rootfs-%: $(ROOTFS_BUILD_DEST)/.%$(ROOTFS_MARKER_SUFFIX)
	@ # DONT remove. This is not cancellation rule.

.PRECIOUS: $(ROOTFS_BUILD_DEST)/.%$(ROOTFS_MARKER_SUFFIX)
$(ROOTFS_BUILD_DEST)/.%$(ROOTFS_MARKER_SUFFIX):: rootfs-builder/%
	$(call silent_run,Creating rootfs for "$*",$(ROOTFS_BUILDER) -o $(VERSION_COMMIT) -r $(ROOTFS_BUILD_DEST)/$*_rootfs $*)
	@touch $@

# To generate a dracut rootfs, we first generate a dracut initrd and then
# extract it in a local folder.
# Notes:
# - assuming a not compressed initrd.
ifeq (dracut,$(BUILD_METHOD))
.PRECIOUS: $(ROOTFS_BUILD_DEST)/.dracut$(ROOTFS_MARKER_SUFFIX)
$(ROOTFS_BUILD_DEST)/.dracut$(ROOTFS_MARKER_SUFFIX): $(TARGET_INITRD)
	mkdir -p $(TARGET_ROOTFS)
	(cd $(TARGET_ROOTFS); cat $< | cpio --extract --preserve-modification-time --make-directories)
	@touch $@
endif

image-%: $(IMAGES_BUILD_DEST)/kata-containers-image-%.img
	@ # DONT remove. This is not cancellation rule.

.PRECIOUS: $(IMAGES_BUILD_DEST)/kata-containers-image-%.img
$(IMAGES_BUILD_DEST)/kata-containers-image-%.img: rootfs-%
	$(call silent_run,Creating image based on $^,$(IMAGE_BUILDER) -o $@ $(ROOTFS_BUILD_DEST)/$*_rootfs)

initrd-%: $(IMAGES_BUILD_DEST)/kata-containers-initrd-%.img
	@ # DONT remove. This is not cancellation rule.

.PRECIOUS: $(IMAGES_BUILD_DEST)/kata-containers-initrd-%.img
$(IMAGES_BUILD_DEST)/kata-containers-initrd-%.img: rootfs-%
	$(call silent_run,Creating initrd image for $*,$(INITRD_BUILDER) -o $@ $(ROOTFS_BUILD_DEST)/$*_rootfs)

.PHONY: rootfs
rootfs: $(TARGET_ROOTFS_MARKER)

.PHONY: image
image: $(TARGET_IMAGE)

$(TARGET_IMAGE): $(TARGET_ROOTFS_MARKER)
	$(call silent_run,Creating image based on "$(TARGET_ROOTFS)",$(IMAGE_BUILDER) -o $@ "$(TARGET_ROOTFS)")


.PHONY: initrd
initrd: $(TARGET_INITRD)

ifeq (distro,$(BUILD_METHOD))
$(TARGET_INITRD): $(TARGET_ROOTFS_MARKER)
	$(call silent_run,Creating initrd image based on "$(TARGET_ROOTFS)",$(INITRD_BUILDER) "$(TARGET_ROOTFS)")
else
$(TARGET_INITRD): $(DRACUT_OVERLAY_DIR)
	@echo Creating initrd image based on the host OS using dracut
	$(DRACUT_DIR)/add_libs.sh $(AGENT_PATH) > $(DRACUT_CONF_DIR)/15-extra-libs.conf
	dracut $(DRACUT_OPTIONS) --include $< / $@ $(DRACUT_KVERSION)
endif

# Notes on overlay dir:
# - If user specified any kernel module in the dracut conf file,
#   we need to make sure these are pre-loaded at startup using
#   systemd modules-load.d
$(DRACUT_OVERLAY_DIR):
	mkdir -p $@
	# Modules preload
	$(ROOTFS_BUILDER) -o $(VERSION_COMMIT) -r $@
	mkdir -p $@/etc/modules-load.d
	echo $(DRACUT_KMODULES) | tr " " "\n" > $@/etc/modules-load.d/kata-modules.conf

.PHONY: test
test:
	$(TEST_RUNNER) "$(DISTRO)"

.PHONY: test-image-only
test-image-only:
	$(TEST_RUNNER) --test-images-only "$(DISTRO)"

.PHONY: test-initrd-only
test-initrd-only:
	$(TEST_RUNNER) --test-initrds-only "$(DISTRO)"

.PHONY: list-distros
list-distros:
	@ $(ROOTFS_BUILDER) -l

DESTDIR := /
KATADIR := /usr/libexec/kata-containers
OSBUILDER_DIR := $(KATADIR)/osbuilder
INSTALL_DIR :=$(DESTDIR)/$(OSBUILDER_DIR)
DIST_CONFIGS:= $(wildcard rootfs-builder/*/config.sh)

SCRIPTS :=
SCRIPTS += rootfs-builder/rootfs.sh
SCRIPTS += image-builder/image_builder.sh
SCRIPTS += initrd-builder/initrd_builder.sh

HELPER_FILES :=
HELPER_FILES += scripts/lib.sh
HELPER_FILES += image-builder/nsdax.gpl.c

define INSTALL_FILE
	echo "Installing $(abspath $2/$1)";
	install -m 644 -D $1 $2/$1;
endef

define INSTALL_SCRIPT
	echo "Installing $(abspath $2/$1)";
	install -m 755 -D $1 $(abspath $2/$1);
endef

.PHONY: install-scripts
install-scripts:
	@echo "Installing scripts"
	@$(foreach f,$(SCRIPTS),$(call INSTALL_SCRIPT,$f,$(INSTALL_DIR)))
	@echo "Installing helper files"
	@$(foreach f,$(HELPER_FILES),$(call INSTALL_FILE,$f,$(INSTALL_DIR)))
	@echo "Installing config files"
	@$(foreach f,$(DIST_CONFIGS),$(call INSTALL_FILE,$f,$(INSTALL_DIR)))

.PHONY: clean
clean:
	rm -rf $(TARGET_ROOTFS_MARKER) $(TARGET_ROOTFS) $(TARGET_IMAGE) $(TARGET_INITRD) $(DRACUT_OVERLAY_DIR)

# Prints the name of the variable passed as suffix to the print- target,
# E.g., if Makefile contains:
# MY_MAKE_VAR := foobar
# Then:
# $ make printf-MY_MAKE_VAR
# Will print "foobar"
print-%:
	@echo $($*)
