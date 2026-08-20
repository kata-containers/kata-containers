#!/usr/bin/env bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[[ -z "${DEBUG:-}" ]] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

REGISTRY="${1:?REGISTRY required (e.g. quay.io/myuser/kata-deploy)}"
TAG="${2:?TAG required (image tag)}"
CHART_REGISTRY="${3:?CHART_REGISTRY required (e.g. quay.io/myuser/kata-deploy-charts)}"
CHART_VERSION="${4:?CHART_VERSION required (chart semver)}"
KEEP_TMPDIR="${KEEP_TMPDIR:-}"

CHART_SRC="$(cd "$(dirname "${0}")/../helm-chart/kata-deploy" && pwd)"

tmp="$(mktemp -d)"
trap '[[ -n "${KEEP_TMPDIR}" ]] && echo "kept: ${tmp}" || rm -rf "${tmp}"' EXIT

cp -r "${CHART_SRC}" "${tmp}/"

yq eval ".version = \"${CHART_VERSION}\" | .appVersion = \"${CHART_VERSION}\"" -i "${tmp}/kata-deploy/Chart.yaml"
yq eval ".image.reference = \"${REGISTRY}\" | .image.tag = \"${TAG}\"" -i "${tmp}/kata-deploy/values.yaml"

# Optional values overlay baked into the packaged chart's defaults, so a
# plain `helm install` of the published chart gives the intended profile
# without every consumer having to pass -f. Relative paths resolve inside
# the chart, so the bundled try-*.values.yaml presets work as-is.
if [[ -n "${CHART_VALUES_OVERLAY:-}" ]]; then
	overlay="${CHART_VALUES_OVERLAY}"
	[[ "${overlay}" == /* ]] || overlay="${tmp}/kata-deploy/${overlay}"
	[[ -f "${overlay}" ]] || { echo "CHART_VALUES_OVERLAY: no such values file: ${overlay}" >&2; exit 1; }
	yq eval-all -i 'select(fileIndex == 0) * select(fileIndex == 1)' \
		"${tmp}/kata-deploy/values.yaml" "${overlay}"
fi
# NFD is vendored under charts/*.tgz; no helm dependency fetch needed.
helm package "${tmp}/kata-deploy" -d "${tmp}"
helm push "${tmp}/kata-deploy-${CHART_VERSION}.tgz" "oci://${CHART_REGISTRY}"
