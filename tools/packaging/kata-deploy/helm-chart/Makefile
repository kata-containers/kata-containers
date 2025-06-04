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

schema: plugins
	cd kata-deploy && helm schema -input values.yaml kata-deploy/values.schema.json && cd .. 

plugins:
	helm plugin install https://github.com/losisin/helm-values-schema-json.git || true
