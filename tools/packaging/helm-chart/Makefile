all: yamls
yamls: kustomize
	$(KUSTOMIZE) build ../kata-deploy/kata-rbac/base > kata-deploy/templates/kata-rbac.yaml
	$(KUSTOMIZE) build ../kata-deploy/kata-deploy/overlays/helm/latest > kata-deploy/templates/kata-deploy-latest.yaml
	#$(KUSTOMIZE) build ../kata-deploy/kata-deploy/overlays/helm/development > kata-deploy/templates/kata-deploy-stable.yaml
package: kustomize yamls helm
	$(HELM) package ./kata-deploy

.PHONY: clean

clean:
	rm kata-deploy/templates/kata-rbac.yaml
	rm kata-deploy/templates/kata-deploy-latest.yaml
	rm kata-deploy/templates/kata-deploy-stable.yaml

	
# go-get-tool will 'go get' any package $2 and install it to $1.
PROJECT_DIR := $(shell dirname $(abspath $(lastword $(MAKEFILE_LIST))))
define go-get-tool
@[ -f $(1) ] || { \
set -e ;\
echo "Downloading $(2)" ;\
GOBIN=$(PROJECT_DIR)/bin go install $(2) ;\
rm -rf $$TMP_DIR ;\
}
endef

HELM = $(shell pwd)/bin/helm
helm: ## Download helm locally if necessary.
	$(call go-get-tool,$(HELM),helm.sh/helm/v3/cmd/helm@latest)

KUSTOMIZE = $(shell pwd)/bin/kustomize
kustomize: ## Download kustomize locally if necessary.
	$(call go-get-tool,$(KUSTOMIZE),sigs.k8s.io/kustomize/kustomize/v4@v4.5.4)
