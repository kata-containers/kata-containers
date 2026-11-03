#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

[[ $# == 2 ]] || { echo "Usage: $0 ASSETS before|after" >&2; exit 1; }
assets=$(realpath "$1")
phase=$2
case "${phase}" in
	before|after) ;;
	*) echo "Unknown verification phase: ${phase}" >&2; exit 1 ;;
esac
results="${ARTIFACTS:?}/asset-verification-${phase}"
mkdir -p "${results}"
(cd "${assets}"; sha256sum -c SHA256SUMS)
arch=$(go env GOARCH)
image_sha=$(sha256sum "${assets}/rootfs.img" | cut -d' ' -f1)
kernel_sha=$(sha256sum "${assets}/vmlinux" | cut -d' ' -f1)
printf 'Candidate rootfs SHA-256: %s\nCandidate kernel SHA-256: %s\n' "${image_sha}" "${kernel_sha}"
kubectl get sandboxconfig microvm -o json > "${results}/sandboxconfig.json"
jq -e --arg arch "${arch}" --arg image "${image_sha}" --arg kernel "${kernel_sha}" '
 .spec.defaultVersion == "kata-ci" and
 ([.spec.versions[] | select(.name == "kata-ci" and .state == "Enabled")] |
 length == 1 and .[0].assets[$arch]["kata-image"].sha256 == $image and
 .[0].assets[$arch]["kata-kernel"].sha256 == $kernel)
 ' "${results}/sandboxconfig.json" > /dev/null || {
	echo 'Live SandboxConfig does not select the candidate rootfs and kernel' >&2
	exit 1
}

# Substrate's sandbox_bundle_hash hashes sorted asset/name=sha256 lines
# without a trailing newline (cmd/atelet/worker_sandbox_versions.go).
bundle_sha=$(jq -j --arg arch "${arch}" '
 .spec.versions[] | select(.name == "kata-ci") | .assets[$arch] | to_entries |
 map("asset/" + .key + "=" + .value.sha256) | sort | join("\n")
 ' "${results}/sandboxconfig.json" | sha256sum | cut -d' ' -f1)
bundle_hash="sha256:${bundle_sha}"
go run ./cmd/kubectl-ate get workers --sandbox-class microvm -o json > "${results}/workers.json"
jq '[.workers[] | select(.status.state == "WORKER_STATE_ACTIVE") |
 {worker: .metadata.name, runtimes: [.status.sandboxClasses[] |
 select(.sandboxClass == "microvm") | .defaultRuntime.version.attributes | from_entries]}]
 ' "${results}/workers.json" > "${results}/worker-runtimes.json"
jq -e --arg arch "${arch}" --arg hash "${bundle_hash}" '
 length > 0 and all(.[]; (.runtimes | length) == 1 and
 .runtimes[0].architecture == $arch and .runtimes[0].sandbox_bundle_hash == $hash)
 ' "${results}/worker-runtimes.json" > /dev/null || {
	cat "${results}/worker-runtimes.json" >&2
	echo "Active microVM workers must report candidate runtime ${bundle_hash} on ${arch}" >&2
	exit 1
}
jq -n --arg phase "${phase}" --arg arch "${arch}" --arg image "${image_sha}" \
	--arg kernel "${kernel_sha}" --arg bundle "${bundle_hash}" \
	--slurpfile workers "${results}/worker-runtimes.json" \
	'{phase: $phase, architecture: $arch, rootfs_sha256: $image,
	kernel_sha256: $kernel, sandbox_bundle_hash: $bundle, workers: $workers[0]}' \
	> "${results}/verified.json"
echo "Verified candidate Kata guest assets ${phase} the Substrate microVM e2e suite:"
cat "${results}/verified.json"
