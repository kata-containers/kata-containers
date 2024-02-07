#!/usr/bin/env bash

# Copyright (c) 2024 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# Provides a library to deal with the CoCo KBS
#

set -o errexit
set -o nounset
set -o pipefail

kubernetes_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=1091
source "${kubernetes_dir}/../../gha-run-k8s-common.sh"

# Where the kbs sources will be cloned
readonly COCO_KBS_DIR="/tmp/kbs"
# The k8s namespace where the kbs service is deployed
readonly KBS_NS="coco-tenant"
# The kbs service name
readonly KBS_SVC_NAME="kbs"

# Delete the kbs on Kubernetes
#
# Note: assume the kbs sources were cloned to $COCO_KBS_DIR
#
function kbs_k8s_delete() {
	pushd "$COCO_KBS_DIR"
	kubectl delete -k kbs/config/kubernetes/overlays
	popd
}

# Deploy the kbs on Kubernetes
#
function kbs_k8s_deploy() {
	local image
	local image_tag
	local repo
	local kbs_ip
	local kbs_port
	local version

	# yq is needed by get_from_kata_deps
	ensure_yq

	# Read from versions.yaml
	repo=$(get_from_kata_deps "externals.coco-kbs.url")
	version=$(get_from_kata_deps "externals.coco-kbs.version")
	image=$(get_from_kata_deps "externals.coco-kbs.image")
	image_tag=$(get_from_kata_deps "externals.coco-kbs.image_tag")

	if [ -d "$COCO_KBS_DIR" ]; then
		rm -rf "$COCO_KBS_DIR"
	fi

	echo "::group::Clone the kbs sources"
	git clone --depth 1 "${repo}" "$COCO_KBS_DIR"
	pushd "$COCO_KBS_DIR"
	git fetch --depth=1 origin "${version}"
	git checkout FETCH_HEAD -b kbs_$$
	echo "::endgroup::"

	pushd kbs/config/kubernetes/

	# Tests should fill kbs resources later, however, the deployment
	# expects at least one secret served at install time.
	echo "somesecret" > overlays/key.bin

	echo "::group::Update the kbs container image"
	install_kustomize
	pushd base
	kustomize edit set image "kbs-container-image=${image}:${image_tag}"
	popd
	echo "::endgroup::"

	echo "::group::Deploy the KBS"
	./deploy-kbs.sh
	popd
	popd

	if ! waitForProcess "120" "10" "kubectl -n \"$KBS_NS\" get pods | \
		grep -q '^kbs-.*Running.*'"; then
		echo "ERROR: KBS service pod isn't running"
		echo "::group::DEBUG - describe kbs deployments"
		kubectl -n "$KBS_NS" get deployments || true
		echo "::endgroup::"
		echo "::group::DEBUG - describe kbs pod"
		kubectl -n "$KBS_NS" describe pod -l app=kbs || true
		echo "::endgroup::"
		return 1
	fi
	echo "::endgroup::"

	# By default, the KBS service is reachable within the cluster only,
	# thus the following healthy checker should run from a pod. So start a
	# debug pod where it will try to get a response from the service. The
	# expected response is '404 Not Found' because it will request an endpoint
	# that does not exist.
	#
	echo "::group::Check the service healthy"
	kbs_ip=$(kubectl get -o jsonpath='{.spec.clusterIP}' svc "$KBS_SVC_NAME" -n "$KBS_NS" 2>/dev/null)
	kbs_port=$(kubectl get -o jsonpath='{.spec.ports[0].port}' svc "$KBS_SVC_NAME" -n "$KBS_NS" 2>/dev/null)
	local pod=kbs-checker-$$
	kubectl run "$pod" --image=quay.io/prometheus/busybox --restart=Never -- \
		sh -c "wget -O- --timeout=5 \"${kbs_ip}:${kbs_port}\" || true"
	if ! waitForProcess "60" "10" "kubectl logs \"$pod\" 2>/dev/null | grep -q \"404 Not Found\""; then
		echo "ERROR: KBS service is not responding to requests"
		echo "::group::DEBUG - kbs logs"
		kubectl -n "$KBS_NS" logs -l app=kbs || true
		echo "::endgroup::"
		kubectl delete pod "$pod"
		return 1
	fi
	kubectl delete pod "$pod"
	echo "KBS service respond to requests"
	echo "::endgroup::"
}