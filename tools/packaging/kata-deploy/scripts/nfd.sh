#!/usr/bin/env bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# External dependencies (not present in bare minimum busybox image):
#   - kubectl
#

function setup_nfd_rules() {
	local expand_runtime_classes_for_nfd=false
	if kubectl get crds nodefeaturerules.nfd.k8s-sigs.io &>/dev/null; then
		arch="$(uname -m)"
		if [[ ${arch} == "x86_64" ]]; then
			node_feature_rule_file="/opt/kata-artifacts/node-feature-rules/${arch}-tee-keys.yaml"

			kubectl apply -f "${node_feature_rule_file}"
			expand_runtime_classes_for_nfd=true

			info "As NFD is deployed on the node, rules for ${arch} TEEs have been created"
		fi
	fi
	echo "${expand_runtime_classes_for_nfd}"
}

function remove_nfd_rules() {
	if kubectl get crds nodefeaturerules.nfd.k8s-sigs.io &>/dev/null; then
		arch="$(uname -m)"
		if [[ ${arch} == "x86_64" ]]; then
			node_feature_rule_file="/opt/kata-artifacts/node-feature-rules/${arch}-tee-keys.yaml"

			kubectl delete  --ignore-not-found -f "${node_feature_rule_file}"

			info "As NFD is deployed on the node, rules for ${arch} TEEs have been deleted"
		fi
	fi
}

