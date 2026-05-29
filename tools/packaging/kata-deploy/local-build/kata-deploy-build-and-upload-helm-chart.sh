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
helm dependencies update "${tmp}/kata-deploy"
helm package "${tmp}/kata-deploy" -d "${tmp}"
helm push "${tmp}/kata-deploy-${CHART_VERSION}.tgz" "oci://${CHART_REGISTRY}"
