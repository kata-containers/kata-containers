#!/bin/bash
#
# Copyright (c) 2021 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This script builds the kata-webhook and deploys it in the test cluster.
#
# You should export the KATA_RUNTIME variable with the runtimeclass name
# configured in your cluster in case it is not the default "kata-ci".
#
set -e
set -o nounset
set -o pipefail

script_dir="$(realpath $(dirname $0))"
webhook_dir="${script_dir}/../../../tools/testing/kata-webhook"
source "${script_dir}/../lib.sh"
KATA_RUNTIME=${KATA_RUNTIME:-kata-ci}

pushd "${webhook_dir}" >/dev/null
# Build and deploy the webhook
#
info "Builds the kata-webhook"
./create-certs.sh
info "Deploys the kata-webhook"
oc apply -f deploy/

info "Override our KATA_RUNTIME ConfigMap"
RUNTIME_CLASS="${KATA_RUNTIME}" \
	envsubst < "${script_dir}/deployments/configmap_kata-webhook.yaml.in" \
	| oc apply -f -

# Check the webhook was deployed and is working.
RUNTIME_CLASS="${KATA_RUNTIME}" ./webhook-check.sh
popd >/dev/null
