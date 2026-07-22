#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

all: | update-yaml generate-client-code post-process-generated-code go-fmt
MK_DIR := $(dir $(realpath $(firstword $(MAKEFILE_LIST))))
YQ_INSTALLER := "$(MK_DIR)/../../../../../ci/install_yq.sh"
VERSIONS_FILE := "$(MK_DIR)/../../../../../versions.yaml"
YQ := $(shell command -v yq 2> /dev/null)

generate-client-code: clean-generated-code
	docker run --rm \
		--user $$(id -u):$$(id -g) \
		-v $${PWD}:/local openapitools/openapi-generator-cli:v5.3.0 generate \
		-i /local/cloud-hypervisor.yaml \
		-g go \
		-o /local/client

post-process-generated-code:
	sed -i \
		-e 's/OFF[[:blank:]]*ConsoleMode = "Off"/CONSOLE_MODE_OFF ConsoleMode = "Off"/' \
		client/model_console_mode.go
	sed -i \
		-e 's/OFF[[:blank:]]*CoreSchedulingMode = "Off"/CORE_SCHEDULING_MODE_OFF CoreSchedulingMode = "Off"/' \
		client/model_core_scheduling_mode.go
	find client/docs -name '*.md' -exec sed -i \
		-e 's/[[:blank:]]*$$//' {} +
	find client/docs -name '*.md' -exec perl -0pi -e 's/\n+\z/\n/' {} +

go-fmt:
	rm client/go.mod; \
	go fmt ./...

update-yaml:
ifndef YQ
	$(MK_DIR)/../../../../../ci//install_yq.sh
endif
	clh_version=$(shell yq .assets.hypervisor.cloud_hypervisor.version $(VERSIONS_FILE)); \
	curl -OL https://raw.githubusercontent.com/cloud-hypervisor/cloud-hypervisor/$$clh_version/vmm/src/api/openapi/cloud-hypervisor.yaml

clean-generated-code:
	rm "./client" -rf
