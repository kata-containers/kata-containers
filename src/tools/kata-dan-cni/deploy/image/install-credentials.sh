#!/bin/sh
# Copyright (c) 2026 Kata Containers community
# SPDX-License-Identifier: Apache-2.0

set -eu

SOURCE_DIR=${KATA_DAN_CREDENTIAL_SOURCE_DIR:-/var/run/secrets/kata-dan-cni}
DEST_DIR=${KATA_DAN_CREDENTIAL_DIR:-/etc/kata-dan-cni}

TOKEN_SOURCE=${SOURCE_DIR}/token
CA_SOURCE=${SOURCE_DIR}/ca.crt
TOKEN_DEST=${DEST_DIR}/token
CA_DEST=${DEST_DIR}/ca.crt
KUBECONFIG_DEST=${DEST_DIR}/kubeconfig

: "${KUBERNETES_SERVICE_HOST:?KUBERNETES_SERVICE_HOST is required}"
: "${KUBERNETES_SERVICE_PORT_HTTPS:?KUBERNETES_SERVICE_PORT_HTTPS is required}"

if [ ! -s "${TOKEN_SOURCE}" ]; then
    echo "projected ServiceAccount token is missing or empty" >&2
    exit 1
fi
if [ ! -s "${CA_SOURCE}" ]; then
    echo "projected Kubernetes CA is missing or empty" >&2
    exit 1
fi

umask 077
mkdir -p "${DEST_DIR}"
chmod 0700 "${DEST_DIR}"

TOKEN_TMP=${DEST_DIR}/.token.tmp.$$
CA_TMP=${DEST_DIR}/.ca.crt.tmp.$$
KUBECONFIG_TMP=${DEST_DIR}/.kubeconfig.tmp.$$

cleanup() {
    rm -f "${TOKEN_TMP}" "${CA_TMP}" "${KUBECONFIG_TMP}"
}
trap cleanup 0 1 2 15

replace_file() {
    source_file=$1
    destination_file=$2
    temporary_file=$3
    mode=$4

    cp "${source_file}" "${temporary_file}"
    chmod "${mode}" "${temporary_file}"

    if [ -f "${destination_file}" ] && cmp -s "${temporary_file}" "${destination_file}"; then
        chmod "${mode}" "${destination_file}"
        rm -f "${temporary_file}"
    else
        mv -f "${temporary_file}" "${destination_file}"
    fi
}

replace_file "${TOKEN_SOURCE}" "${TOKEN_DEST}" "${TOKEN_TMP}" 0600
replace_file "${CA_SOURCE}" "${CA_DEST}" "${CA_TMP}" 0644

case "${KUBERNETES_SERVICE_HOST}" in
    *:*)
        API_SERVER="https://[${KUBERNETES_SERVICE_HOST}]:${KUBERNETES_SERVICE_PORT_HTTPS}"
        ;;
    *)
        API_SERVER="https://${KUBERNETES_SERVICE_HOST}:${KUBERNETES_SERVICE_PORT_HTTPS}"
        ;;
esac

cat >"${KUBECONFIG_TMP}" <<EOF
apiVersion: v1
kind: Config
clusters:
- name: kubernetes
  cluster:
    server: "${API_SERVER}"
    certificate-authority: "${CA_DEST}"
users:
- name: kata-dan-cni
  user:
    tokenFile: "${TOKEN_DEST}"
contexts:
- name: kata-dan-cni
  context:
    cluster: kubernetes
    user: kata-dan-cni
current-context: kata-dan-cni
EOF

chmod 0600 "${KUBECONFIG_TMP}"
if [ -f "${KUBECONFIG_DEST}" ] && cmp -s "${KUBECONFIG_TMP}" "${KUBECONFIG_DEST}"; then
    chmod 0600 "${KUBECONFIG_DEST}"
    rm -f "${KUBECONFIG_TMP}"
else
    mv -f "${KUBECONFIG_TMP}" "${KUBECONFIG_DEST}"
fi
