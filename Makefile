#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

DESTBINDIR = /usr/local/bin

KATA_RUNTIME_NAME = kata-runtime
KATA_RUNTIME_PATH = $(DESTBINDIR)/$(KATA_RUNTIME_NAME)

ifeq (,$(KATA_RUNTIME))
    # no argument specified
    ifeq (,$(MAKECMDGOALS))
        fail = true
    endif

    # remove goals that don't require the variable to be set
    remaining=$(filter-out help,$(MAKECMDGOALS))

    ifneq (,$(remaining))
        fail = true
    endif

    ifeq ($(fail),true)
        $(error "ERROR: KATA_RUNTIME not set - run 'make help'")
    endif
endif

ifeq (cc,$(KATA_RUNTIME))
	RUNTIME_DIR = cc-runtime
	RUNTIME_NAME = kata-runtime-cc
	TARGET = $(RUNTIME_NAME)
	DESTTARGET = $(DESTBINDIR)/$(TARGET)
endif

ifeq (runv,$(KATA_RUNTIME))
	RUNTIME_DIR = runv
	RUNTIME_NAME = runv
	BUILD_ROOT = $(shell pwd)/build-runv
	HYPER_ROOT = $(BUILD_ROOT)/src/github.com/hyperhq
	RUNV_ROOT = $(HYPER_ROOT)/runv
	DESTTARGET = $(DESTBINDIR)/$(RUNTIME_NAME)
endif

default: build

build:
ifeq (cc,$(KATA_RUNTIME))
	make -C $(RUNTIME_DIR) build-kata-system TARGET=$(TARGET) DESTTARGET=$(DESTTARGET)
endif
ifeq (runv,$(KATA_RUNTIME))
	mkdir -p $(HYPER_ROOT) && ln -sf $(shell pwd)/$(RUNTIME_DIR) $(RUNV_ROOT)
	cd $(RUNV_ROOT) && [ -e configure ] || ./autogen.sh && ./configure && make GOPATH=$(BUILD_ROOT)
endif

install: install-runtime create-symlink

install-runtime:
ifeq (cc,$(KATA_RUNTIME))
	make -C $(RUNTIME_DIR) install-kata-system TARGET=$(TARGET) DESTTARGET=$(DESTTARGET)
endif
ifeq (runv,$(KATA_RUNTIME))
	make -C $(RUNV_ROOT) install-exec-local
endif

create-symlink:
	ln -sf $(DESTTARGET) $(KATA_RUNTIME_PATH)

remove-symlink:
	rm -f $(KATA_RUNTIME_PATH)

clean: remove-symlink
ifeq (cc,$(KATA_RUNTIME))
	make -C $(RUNTIME_DIR) clean TARGET=$(TARGET)
endif
ifeq (runv,$(KATA_RUNTIME))
	make -C $(RUNV_ROOT) clean
endif

help:
	@printf "To build a Kata Containers runtime:\n"
	@printf "\n"
	@printf "  \$$ make KATA_RUNTIME={cc|runv} [install]\n"
	@printf "\n"
	@printf "Project home: https://github.com/kata-containers\n"
