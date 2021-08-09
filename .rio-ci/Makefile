KUBECONFIG ?= $(HOME)/.kube/config
PRIORITY_CLASS ?= p1
DOCKER ?= docker
BASE_IMAGE ?= "docker.apple.com/aci-compute-containers/ksmith-kata:latest"

GIT_PR_ID ?= dev
GIT_COMMIT_SHORT ?= dev
RIO_BUILD_NUMBER ?= dev
CLUSTER_NAME ?= kata-ci-$(GIT_PR_ID)-$(GIT_COMMIT_SHORT)-$(RIO_BUILD_NUMBER)

FILE_DIR := $(shell dirname $(realpath $(firstword $(MAKEFILE_LIST))))

.PHONY: sc-unit-test
sc-unit-test:
	$(FILE_DIR)/simcloud-job.sh simcloud-unit-test.sh

.PHONY: sc-check
sc-check:
	$(FILE_DIR)/simcloud-job.sh simcloud-check.sh

.PHONY: kubernetes-integration-test
kubernetes-integration-test:
	PRIORITY_CLASS=$(PRIORITY_CLASS) CLUSTER_NAME=$(CLUSTER_NAME) $(FILE_DIR)/integration/kubernetes/setup.sh
	$(FILE_DIR)/integration/kubernetes/build_and_deploy.sh $(CLUSTER_NAME)
	PRIORITY_CLASS=$(PRIORITY_CLASS) CLUSTER_NAME=$(CLUSTER_NAME) $(FILE_DIR)/integration/kubernetes/run_kubernetes_tests.sh

.PHONY: kubernetes-integration-test-local-spawn
kubernetes-integration-test-local-spawn:
	$(DOCKER) run -it -v $(KUBECONFIG):/kubeconfig -v $(shell pwd):/workspace $(BASE_IMAGE) \
	bash -c "export PRIORITY_CLASS=$(PRIORITY_CLASS) && \
	export CLUSTER_NAME=$(CLUSTER_NAME) && \
	export SHOULD_TEARDOWN=false && \
	/workspace/.rio-ci/integration/kubernetes/setup.sh && \
	/workspace/.rio-ci/integration/kubernetes/build_and_deploy.sh $(CLUSTER_NAME) && \
	/workspace/.rio-ci/integration/kubernetes/run_kubernetes_tests.sh && \
	bash"

.PHONY: kubernetes-integration-test-local-teardown
kubernetes-integration-test-local-teardown:
	$(DOCKER) run -v $(KUBECONFIG):/kubeconfig -v $(shell pwd):/workspace $(BASE_IMAGE) \
	bash -c "CLUSTER_NAME=$(CLUSTER_NAME) /workspace/.rio-ci/integration/kubernetes/teardown.sh"
