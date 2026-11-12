#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "${work}"' EXIT
mkdir "${work}/bin" "${work}/assets"
printf 'candidate image\n' > "${work}/assets/rootfs.img"
printf 'candidate kernel\n' > "${work}/assets/vmlinux"
(cd "${work}/assets"; sha256sum rootfs.img vmlinux > SHA256SUMS)
image_sha=$(sha256sum "${work}/assets/rootfs.img" | cut -d' ' -f1)
kernel_sha=$(sha256sum "${work}/assets/vmlinux" | cut -d' ' -f1)
bundle_hash="sha256:$(printf 'asset/cloud-hypervisor=vmm\nasset/kata-image=%s\nasset/kata-kernel=%s\nasset/virtiofsd=fs' \
	"${image_sha}" "${kernel_sha}" | sha256sum | cut -d' ' -f1)"
export TEST_VERIFY_CONFIG="${work}/config.json" TEST_VERIFY_WORKERS="${work}/workers.json"
jq -n --arg image "${image_sha}" --arg kernel "${kernel_sha}" \
	'{spec: {defaultVersion: "kata-ci", versions: [{name: "kata-ci", state: "Enabled", assets: {
	amd64: {"kata-image": {sha256: $image}, "kata-kernel": {sha256: $kernel},
	"cloud-hypervisor": {sha256: "vmm"}, "virtiofsd": {sha256: "fs"}}}}]}}' > "${TEST_VERIFY_CONFIG}"
jq -n --arg bundle "${bundle_hash}" \
	'{workers: [{metadata: {name: "test-worker"}, status: {state: "WORKER_STATE_ACTIVE",
	sandboxClasses: [{sandboxClass: "microvm", defaultRuntime: {version: {attributes: [
	{key: "architecture", value: "amd64"}, {key: "sandbox_bundle_hash", value: $bundle}]}}}]}}]}' \
	> "${TEST_VERIFY_WORKERS}"
cp "${TEST_VERIFY_CONFIG}" "${work}/good-config.json"
cp "${TEST_VERIFY_WORKERS}" "${work}/good-workers.json"
cat > "${work}/bin/kubectl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
[[ "$*" == 'get sandboxconfig microvm -o json' ]]
cat "${TEST_VERIFY_CONFIG}"
MOCK
cat > "${work}/bin/go" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
case "$*" in
	'env GOARCH') echo amd64 ;;
	'run ./cmd/kubectl-ate get workers --sandbox-class microvm -o json') cat "${TEST_VERIFY_WORKERS}" ;;
	*) echo "Unexpected Go invocation: $*" >&2; exit 1 ;;
esac
MOCK
chmod +x "${work}/bin/"*
export PATH="${work}/bin:${PATH}" ARTIFACTS="${work}/results"
bash "${script_dir}/verify-artifacts.sh" "${work}/assets" before > "${work}/success.log"
jq -e --arg hash "${bundle_hash}" '.sandbox_bundle_hash == $hash and (.workers | length) == 1' \
	"${ARTIFACTS}/asset-verification-before/verified.json" > /dev/null

for failure in wrong-config wrong-default wrong-architecture no-workers; do
	cp "${work}/good-config.json" "${TEST_VERIFY_CONFIG}"
	cp "${work}/good-workers.json" "${TEST_VERIFY_WORKERS}"
	case "${failure}" in
		wrong-config)
			jq '.spec.versions[0].assets.amd64["kata-image"].sha256 = "stock-image"' \
				"${work}/good-config.json" > "${TEST_VERIFY_CONFIG}"
			;;
		wrong-default)
			# A candidate available only for restore must not pass the default-runtime check.
			jq '.workers[0].status.sandboxClasses[0] |=
			 (.restorableRuntimes = [.defaultRuntime] |
			 .defaultRuntime.version.attributes[1].value = "stock-bundle")' \
				"${work}/good-workers.json" > "${TEST_VERIFY_WORKERS}"
			;;
		wrong-architecture)
			jq '.workers[0].status.sandboxClasses[0].defaultRuntime.version.attributes[0].value = "arm64"' \
				"${work}/good-workers.json" > "${TEST_VERIFY_WORKERS}"
			;;
		no-workers) echo '{"workers": []}' > "${TEST_VERIFY_WORKERS}" ;;
	esac
	rm -rf "${ARTIFACTS}"
	if bash "${script_dir}/verify-artifacts.sh" "${work}/assets" after > "${work}/${failure}.log" 2>&1; then
		echo "Verification must fail for ${failure}" >&2
		exit 1
	fi
	[[ ! -f "${ARTIFACTS}/asset-verification-after/verified.json" ]]
done
echo 'Artifact verification rejects stock guests, wrong worker runtimes and missing workers'
