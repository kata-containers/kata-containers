#!/bin/sh
# Copyright (c) 2026 Kata Containers community
# SPDX-License-Identifier: Apache-2.0

set -eu

fail() {
    echo "kata-dan-cni installer failed: $1" >&2
    exit 1
}

KATA_DAN_NODE_NAME=${KATA_DAN_NODE_NAME:-}
KATA_DAN_RUNTIME_CLASS=${KATA_DAN_RUNTIME_CLASS:-}
KATA_DAN_CONF_DIR=${KATA_DAN_CONF_DIR:-}

[ -n "${KATA_DAN_NODE_NAME}" ] || fail "KATA_DAN_NODE_NAME is required"
[ -n "${KATA_DAN_RUNTIME_CLASS}" ] || fail "KATA_DAN_RUNTIME_CLASS is required"
[ -n "${KATA_DAN_CONF_DIR}" ] || fail "KATA_DAN_CONF_DIR is required"

CNI_BIN_SRC=${KATA_DAN_CNI_BIN_SRC:-/kata-dan-cni/kata-dan-cni}
CNI_BIN_DIR=${KATA_DAN_CNI_BIN_DIR:-/opt/cni/bin}
CNI_BIN_DST=${CNI_BIN_DIR}/kata-dan-cni
CNI_BIN_TMP=${CNI_BIN_DIR}/.kata-dan-cni.tmp.$$

CNI_CONF_TEMPLATE=${KATA_DAN_CNI_CONF_TEMPLATE:-/kata-dan-cni/01-kata-dan.conflist.template}
CNI_CONF_DIR=${KATA_DAN_CNI_CONF_DIR:-/etc/cni/net.d}
CNI_CONF_DST=${CNI_CONF_DIR}/01-kata-dan.conflist
CNI_CONF_TMP=${CNI_CONF_DIR}/.01-kata-dan.conflist.tmp.$$
CONFIG_RENDERER=${KATA_DAN_CONFIG_RENDERER:-/kata-dan-cni/render-cni-config}

[ -f "${CNI_BIN_SRC}" ] || fail "CNI binary source does not exist: ${CNI_BIN_SRC}"
[ -f "${CNI_CONF_TEMPLATE}" ] || fail "conflist template does not exist: ${CNI_CONF_TEMPLATE}"
[ -x "${CONFIG_RENDERER}" ] || fail "config renderer is not executable: ${CONFIG_RENDERER}"

# containerd loads configurations by filename; reject a shadowed entry point.
for candidate in "${CNI_CONF_DIR}"/*.conf "${CNI_CONF_DIR}"/*.conflist "${CNI_CONF_DIR}"/*.json; do
    [ -f "${candidate}" ] || continue
    [ "${candidate}" = "${CNI_CONF_DST}" ] && continue
    first=$(printf '%s\n' "${CNI_CONF_DST##*/}" "${candidate##*/}" | LC_ALL=C sort | head -n 1)
    [ "${first}" = "${CNI_CONF_DST##*/}" ] ||
        fail "another CNI configuration precedes Kata DAN: ${candidate}"
done

cleanup() {
    rm -f "${CNI_BIN_TMP}" "${CNI_CONF_TMP}"
}
trap cleanup 0 1 2 15

mkdir -p "${CNI_BIN_DIR}" "${CNI_CONF_DIR}"

if ! "${CONFIG_RENDERER}" \
    "${CNI_CONF_TEMPLATE}" \
    "${KATA_DAN_NODE_NAME}" \
    "${KATA_DAN_RUNTIME_CLASS}" \
    "${KATA_DAN_CONF_DIR}" >"${CNI_CONF_TMP}"; then
    fail "could not render the Kata DAN conflist template"
fi
chmod 0600 "${CNI_CONF_TMP}"

cp "${CNI_BIN_SRC}" "${CNI_BIN_TMP}"
chmod 0755 "${CNI_BIN_TMP}"
mv -f "${CNI_BIN_TMP}" "${CNI_BIN_DST}"

mv -f "${CNI_CONF_TMP}" "${CNI_CONF_DST}"

echo "installed ${CNI_BIN_DST}"
echo "installed ${CNI_CONF_DST}"
