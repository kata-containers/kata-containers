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

script_dir="$(realpath "$(dirname "$0")")"
webhook_dir="${script_dir}/../../../tools/testing/kata-webhook"
# shellcheck disable=SC1091 # import based on variable
source "${script_dir}/../lib.sh"
KATA_RUNTIME=${KATA_RUNTIME:-kata-ci}

pushd "${webhook_dir}" >/dev/null
# Build and deploy the webhook
#
info "Builds the kata-webhook"
./create-certs.sh
info "Override our KATA_RUNTIME ConfigMap"
sed -i deploy/webhook.yaml -e "s/runtime_class: .*$/runtime_class: ${KATA_RUNTIME}/g"
info "Deploys the kata-webhook"
oc apply -f deploy/

# Check the webhook was deployed and is working.
RUNTIME_CLASS="${KATA_RUNTIME}" ./webhook-check.sh
popd >/dev/null
