# Copyright (c) 2024 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

VERSION_FILE := ../../../../VERSION

release:
	sed -i 's/appVersion: .*/appVersion: $(shell cat $(VERSION_FILE))/g' kata-deploy/Chart.yaml
all: package

package: helm release
	$(HELM) package ./kata-deploy

.PHONY: clean

clean:
	rm kata-deploy-*.tgz
	
# go-get-tool will 'go get' any package $2 and install it to $1.
PROJECT_DIR := $(shell dirname $(abspath $(lastword $(MAKEFILE_LIST))))
define go-get-tool
@[ -f $(PROJECT_DIR)/bin/$(1) ] || command -v $(1) >/dev/null 2>&1 || { \
set -e ;\
echo "Downloading $(2)" ;\
GOBIN=$(PROJECT_DIR)/bin go install $(2) ;\
rm -rf $$TMP_DIR ;\
}
endef

HELM = $(shell pwd)/bin/helm
helm: ## Download helm locally if necessary.
	$(call go-get-tool,$(HELM),helm.sh/helm/v3/cmd/helm@latest)
