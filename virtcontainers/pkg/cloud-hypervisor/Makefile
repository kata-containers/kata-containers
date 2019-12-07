#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

all: | update-yaml generate-client-code

generate-client-code: clean-generated-code
	docker run --rm \
		--user $$(id -u):$$(id -g) \
		-v $${PWD}:/local openapitools/openapi-generator-cli generate \
		-i /local/cloud-hypervisor.yaml \
		-g go \
		-o /local/client

update-yaml:
	curl -OL https://raw.githubusercontent.com/cloud-hypervisor/cloud-hypervisor/master/vmm/src/api/openapi/cloud-hypervisor.yaml

clean-generated-code:
	rm "./client" -rf
