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

IMG_SIZE               = 500
AGENT_INIT            ?= no
DISTRO                ?= centos
ROOTFS_BUILD_DEST     := $(PWD)
IMAGES_BUILD_DEST     := $(PWD)
DISTRO_ROOTFS         := $(ROOTFS_BUILD_DEST)/$(DISTRO)_rootfs
DISTRO_ROOTFS_MARKER  := $(ROOTFS_BUILD_DEST)/.$(DISTRO)_rootfs.done
DISTRO_IMAGE          := $(IMAGES_BUILD_DEST)/kata-containers.img
DISTRO_INITRD         := $(IMAGES_BUILD_DEST)/kata-containers-initrd.img

VERSION_FILE   := ./VERSION
VERSION        := $(shell grep -v ^\# $(VERSION_FILE))
COMMIT_NO      := $(shell git rev-parse HEAD 2> /dev/null || true)
COMMIT         := $(if $(shell git status --porcelain --untracked-files=no),${COMMIT_NO}-dirty,${COMMIT_NO})
VERSION_COMMIT := $(if $(COMMIT),$(VERSION)-$(COMMIT),$(VERSION))

################################################################################

rootfs-%: $(ROOTFS_BUILD_DEST)/.%_rootfs.done
	@ # DONT remove. This is not cancellation rule.

.PRECIOUS: $(ROOTFS_BUILD_DEST)/.%_rootfs.done
$(ROOTFS_BUILD_DEST)/.%_rootfs.done:: rootfs-builder/%
	@echo Creating rootfs for "$*"
	$(ROOTFS_BUILDER) -o $(VERSION_COMMIT) -r $(ROOTFS_BUILD_DEST)/$*_rootfs $*
	touch $@

image-%: $(IMAGES_BUILD_DEST)/kata-containers-image-%.img
	@ # DONT remove. This is not cancellation rule.

.PRECIOUS: $(IMAGES_BUILD_DEST)/kata-containers-image-%.img
$(IMAGES_BUILD_DEST)/kata-containers-image-%.img: rootfs-%
	@echo Creating image based on $^
	$(IMAGE_BUILDER) -s $(IMG_SIZE) -o $@ $(ROOTFS_BUILD_DEST)/$*_rootfs

initrd-%: $(IMAGES_BUILD_DEST)/kata-containers-initrd-%.img
	@ # DONT remove. This is not cancellation rule.

.PRECIOUS: $(IMAGES_BUILD_DEST)/kata-containers-initrd-%.img
$(IMAGES_BUILD_DEST)/kata-containers-initrd-%.img: rootfs-%
	@echo Creating initrd image for $*
	$(INITRD_BUILDER) -o $@ $(ROOTFS_BUILD_DEST)/$*_rootfs

.PHONY: all
all: image initrd

.PHONY: rootfs
rootfs: $(DISTRO_ROOTFS_MARKER)

.PHONY: image
image: $(DISTRO_IMAGE)

$(DISTRO_IMAGE): $(DISTRO_ROOTFS_MARKER)
	@echo Creating image based on "$(DISTRO_ROOTFS)"
	$(IMAGE_BUILDER) -s "$(IMG_SIZE)" "$(DISTRO_ROOTFS)"

.PHONY: initrd
initrd: $(DISTRO_INITRD)

$(DISTRO_INITRD): $(DISTRO_ROOTFS_MARKER)
	@echo Creating initrd image based on "$(DISTRO_ROOTFS)"
	$(INITRD_BUILDER) "$(DISTRO_ROOTFS)"

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

.PHONY: clean
clean:
	rm -rf $(DISTRO_ROOTFS_MARKER) $(DISTRO_ROOTFS) $(DISTRO_IMAGE) $(DISTRO_INITRD)
