#!/usr/bin/env bash
#
# Copyright 2017 The Kubernetes Authors.
# Copyright (c) 2023 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
set -o pipefail

BASE_DIR=$(dirname "$0")
DEPLOY_DIR=${BASE_DIR}/kata-directvolume

TEMP_DIR="$( mktemp -d )"
trap 'rm -rf ${TEMP_DIR}' EXIT

: ${UPDATE_RBAC_RULES:=true}
function rbac_version () {
    yaml="$1"
    image="$2"
    update_rbac="$3"

    # get version from `image: quay.io/k8scsi/csi-attacher:v1.0.1`, ignoring comments
    version="$(sed -e 's/ *#.*$//' "$yaml" | grep "image:.*$image" | sed -e 's/ *#.*//' -e 's/.*://')"

    if $update_rbac; then
        # apply overrides
        varname=$(echo $image | tr - _ | tr a-z A-Z)
        eval version=\${${varname}_TAG:-\${IMAGE_TAG:-\$version}}
    fi

    echo "$version"
}

# https://raw.githubusercontent.com/kubernetes-csi/external-provisioner/${VERSION}/deploy/kubernetes/rbac.yaml
CSI_PROVISIONER_RBAC_YAML="https://raw.githubusercontent.com/kubernetes-csi/external-provisioner/$(rbac_version "${BASE_DIR}/kata-directvolume/csi-directvol-plugin.yaml" csi-provisioner false)/deploy/kubernetes/rbac.yaml"
: ${CSI_PROVISIONER_RBAC:=https://raw.githubusercontent.com/kubernetes-csi/external-provisioner/$(rbac_version "${BASE_DIR}/kata-directvolume/csi-directvol-plugin.yaml" csi-provisioner "${UPDATE_RBAC_RULES}")/deploy/kubernetes/rbac.yaml}

run () {
    echo "$@" >&2
    "$@"
}

# namespace kata-directvolume
DIRECTVOL_NAMESPACE="kata-directvolume"

# create namespace kata-directvolume
echo "Creating Namespace kata-directvolume ..."
        cat <<- EOF > "${TEMP_DIR}"/kata-directvol-ns.yaml
apiVersion: v1
kind: Namespace
metadata:
  labels:
    kubernetes.io/metadata.name: ${DIRECTVOL_NAMESPACE}
  name: ${DIRECTVOL_NAMESPACE}
spec:
  finalizers:
  - kubernetes
EOF

run kubectl apply -f "${TEMP_DIR}"/kata-directvol-ns.yaml
echo "Namespace kata-directvolume created Done !"

# rbac rules
echo "Applying RBAC rules ..."

eval component="CSI_PROVISIONER"
eval current="\${${component}_RBAC}"
eval original="\${${component}_RBAC_YAML}"

if [[ "${current}" =~ ^http:// ]] || [[ "${current}" =~ ^https:// ]]; then
    run curl "${current}" --output "${TEMP_DIR}"/rbac.yaml --silent --location
fi

# replace the default namespace with specified namespace kata-directvolume
sed -e "s/namespace: default/namespace: kata-directvolume/g" "${TEMP_DIR}"/rbac.yaml > "${DEPLOY_DIR}/kata-directvol-rbac.yaml"

# apply the kata-directvol-rbac.yaml
run kubectl apply -f "${DEPLOY_DIR}/kata-directvol-rbac.yaml"
echo "Applying RBAC rules Done!"