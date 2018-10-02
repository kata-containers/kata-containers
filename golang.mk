#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Check that the system golang version is within the required version range
# for this project.

golang_version_min=$(shell yq r versions.yaml languages.golang.version)

ifeq (,$(golang_version_min))
    $(error "ERROR: cannot determine minimum golang version")
endif

golang_version_min_fields=$(subst ., ,$(golang_version_min))

golang_version_min_major=$(word 1,$(golang_version_min_fields))
golang_version_min_minor=$(word 2,$(golang_version_min_fields))

# for error messages
golang_version_needed=$(golang_version_min_major).$(golang_version_min_minor)

golang_version_raw=$(shell go version 2>/dev/null)

ifeq (,$(golang_version_raw))
    $(error "ERROR: cannot determine golang version")
endif

# determine actual version of golang
golang_version=$(subst go,,$(word 3,$(golang_version_raw)))

golang_version_fields=$(subst ., ,$(golang_version))

golang_version_major=$(word 1,$(golang_version_fields))
golang_version_minor=$(word 2,$(golang_version_fields))

golang_major_ok=$(shell test $(golang_version_major) -ge $(golang_version_min_major) && echo ok)
golang_minor_ok=$(shell test $(golang_version_major) -eq $(golang_version_min_major) -a $(golang_version_minor) -ge $(golang_version_min_minor) && echo ok)

ifeq (,$(golang_major_ok))
    $(error "ERROR: golang major version too old: got $(golang_version), need atleast $(golang_version_needed)")
endif

ifeq (,$(golang_minor_ok))
    $(error "ERROR: golang minor version too old: got $(golang_version), need atleast $(golang_version_needed)")
endif
