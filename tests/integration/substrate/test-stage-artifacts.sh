#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

[[ $# == 1 ]] || { echo "Usage: $0 ASSETS" >&2; exit 1; }
assets=$(realpath "$1")
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "${work}"' EXIT
mkdir "${work}/bin"
export TEST_STAGE_UPLOADS="${work}/uploads" TEST_STAGE_CONFIG="${work}/config.json"
export TEST_STAGE_UPSTREAM="${work}/upstream.json"
jq -n '{apiVersion: "ate.dev/v1", kind: "SandboxConfig", metadata: {name: "microvm"},
 spec: {sandboxClass: "microvm", defaultVersion: "v2", versions: [
 {name: "v1", state: "Disabled", assets: {}},
 {name: "v2", state: "Enabled", assets: (["amd64", "arm64"] | map(. as $arch |
 {key: $arch, value: (["cloud-hypervisor", "virtiofsd", "kata-kernel", "kata-image"] |
 map({key: ., value: {url: ("s3://upstream/" + $arch + "/" + .), sha256: "original"}}) |
 from_entries)}) | from_entries)}]}}' > "${TEST_STAGE_UPSTREAM}"
cat > "${work}/bin/kubectl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
case "$*" in
	'get sandboxconfig microvm -o json') cat "${TEST_STAGE_UPSTREAM}" ;;
	'-n ate-system rollout status '*|'-n ate-system wait '*) ;;
	'-n ate-system get pods '*) echo test-node ;;
	'-n ate-system get service '*) echo 10.0.0.1 ;;
	'-n ate-system get job '*) printf 'fixture-client@sha256:%064d\n' 0 ;;
	'-n ate-system get deployment '*)
		jq -n '{spec: {template: {spec: {containers: [{env: [
		{name: "RUSTFS_ACCESS_KEY", value: "test"},
		{name: "RUSTFS_SECRET_KEY", value: "test"}]}]}}}}'
		;;
	'apply -f '*) cp "$3" "${TEST_STAGE_CONFIG}" ;;
	*) echo "Unexpected kubectl call: $*" >&2; exit 1 ;;
esac
MOCK
cat > "${work}/bin/docker" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
url=${!#}
digest=$(sha256sum | cut -d' ' -f1)
[[ "${url}" == "s3://ate-snapshots/microvm-assets/$(go env GOARCH)/${digest}" ]]
printf '%s\n' "${url}" >> "${TEST_STAGE_UPLOADS}"
MOCK
chmod +x "${work}/bin/"*
PATH="${work}/bin:${PATH}" ARTIFACTS="${work}/results" \
	bash "${script_dir}/stage-artifacts.sh" "${assets}"
[[ $(wc -l < "${TEST_STAGE_UPLOADS}") == 2 ]]
arch=$(go env GOARCH)
digest=$(sha256sum "${assets}/rootfs.img" | cut -d' ' -f1)
jq -e --arg arch "${arch}" --arg digest "${digest}" --slurpfile upstream "${TEST_STAGE_UPSTREAM}" '
 .spec.versions[2] as $candidate |
 .spec.defaultVersion == "kata-ci" and $candidate.name == "kata-ci" and
 .spec.versions[:2] == $upstream[0].spec.versions and
 $candidate.assets[$arch]["kata-image"].sha256 == $digest and
 ($candidate | del(.name, .assets[$arch]["kata-image"], .assets[$arch]["kata-kernel"])) ==
 ($upstream[0].spec.versions[1] | del(.name, .assets[$arch]["kata-image"], .assets[$arch]["kata-kernel"]))
 ' "${TEST_STAGE_CONFIG}" > /dev/null
jq -r --arg arch "${arch}" '.spec.versions[2].assets[$arch] | .["kata-image"].url, .["kata-kernel"].url' \
	"${TEST_STAGE_CONFIG}" | sort > "${work}/configured"
sort "${TEST_STAGE_UPLOADS}" > "${work}/uploaded"
cmp "${work}/configured" "${work}/uploaded"

# An invalid upstream config must fail before uploading candidate assets.
jq 'del(.spec.versions[1].assets)' "${TEST_STAGE_UPSTREAM}" > "${work}/invalid.json"
mv "${work}/invalid.json" "${TEST_STAGE_UPSTREAM}"
rm "${TEST_STAGE_UPLOADS}"
if PATH="${work}/bin:${PATH}" ARTIFACTS="${work}/results" \
	bash "${script_dir}/stage-artifacts.sh" "${assets}" > "${work}/invalid.log" 2>&1; then
	echo 'Missing upstream assets must fail staging' >&2
	exit 1
fi
[[ ! -f "${TEST_STAGE_UPLOADS}" ]]
echo 'Candidate guest staging preserves upstream host assets and versions'
