#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

[[ $# == 1 ]] || { echo "Usage: $0 ASSETS" >&2; exit 1; }
assets=$(realpath "$1")
(cd "${assets}"; sha256sum -c SHA256SUMS)
arch=$(go env GOARCH)
work=$(mktemp -d)
trap 'rm -rf "${work}"' EXIT
kubectl -n ate-system rollout status deployment/rustfs --timeout=300s
kubectl -n ate-system wait --for=condition=complete job/rustfs-bucket-init --timeout=300s
node=$(kubectl -n ate-system get pods -l app=rustfs -o jsonpath='{.items[0].spec.nodeName}')
endpoint=$(kubectl -n ate-system get service rustfs -o jsonpath='{.spec.clusterIP}')
[[ -n "${node}" && -n "${endpoint}" ]] || { echo 'Rustfs has no node or service address' >&2; exit 1; }
kubectl -n ate-system get deployment rustfs -o json > "${work}/rustfs.json"
export AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY
AWS_ACCESS_KEY_ID=$(jq -er '.spec.template.spec.containers[0].env[] | select(.name == "RUSTFS_ACCESS_KEY") | .value' "${work}/rustfs.json")
AWS_SECRET_ACCESS_KEY=$(jq -er '.spec.template.spec.containers[0].env[] | select(.name == "RUSTFS_SECRET_KEY") | .value' "${work}/rustfs.json")
aws_image=$(kubectl -n ate-system get job rustfs-bucket-init -o jsonpath='{.spec.template.spec.containers[0].image}')
[[ "${aws_image}" == *@sha256:* ]] || { echo 'The upstream S3 client must be pinned by digest' >&2; exit 1; }

# Keep Substrate's host binaries and other architectures from its installed config.
kubectl get sandboxconfig microvm -o json > "${work}/upstream.json"
jq -e --arg arch "${arch}" '
 .spec.defaultVersion as $default |
 [.spec.versions[] | select(.name == $default and .state == "Enabled")] as $versions |
 if ($versions | length) != 1 then error("Missing enabled default microVM version")
 elif any(.spec.versions[]; .name == "kata-ci") then error("kata-ci already exists")
 elif ($versions[0].assets[$arch] | has("cloud-hypervisor") and has("virtiofsd") and
       has("kata-kernel") and has("kata-image")) | not then error("Incomplete upstream assets")
 else .spec.versions += [($versions[0] | .name = "kata-ci")] |
      .spec.defaultVersion = "kata-ci" |
      .metadata |= {name, annotations, labels} | del(.status)
 end' "${work}/upstream.json" > "${work}/sandboxconfig.json"
for file in vmlinux rootfs.img; do
	digest=$(sha256sum "${assets}/${file}" | cut -d' ' -f1)
	docker run --rm -i --network "container:${node}" \
		-e AWS_ACCESS_KEY_ID -e AWS_SECRET_ACCESS_KEY -e AWS_REGION=us-east-1 \
		-e "AWS_ENDPOINT_URL=http://${endpoint}:9000" "${aws_image}" \
		s3 cp - "s3://ate-snapshots/microvm-assets/${arch}/${digest}" < "${assets}/${file}"
	case "${file}" in
		vmlinux) key=kata-kernel ;;
		rootfs.img) key=kata-image ;;
	esac
	jq --arg arch "${arch}" --arg key "${key}" --arg digest "${digest}" \
		'(.spec.versions[] | select(.name == "kata-ci").assets[$arch][$key]) = {sha256: $digest,
		url: ("s3://ate-snapshots/microvm-assets/" + $arch + "/" + $digest)}' \
		"${work}/sandboxconfig.json" > "${work}/next.json"
	mv "${work}/next.json" "${work}/sandboxconfig.json"
done
kubectl apply -f "${work}/sandboxconfig.json"
# Record the expected image digest alongside the test results.
mkdir -p "${ARTIFACTS:?}"
cp "${work}/sandboxconfig.json" "${work}/upstream.json" "${assets}/SHA256SUMS" "${ARTIFACTS}/"
