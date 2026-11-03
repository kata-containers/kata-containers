#!/usr/bin/env bash
# Copyright (c) 2026 Kata Containers contributors
# SPDX-License-Identifier: Apache-2.0
# shellcheck source-path=SCRIPTDIR
# shellcheck disable=SC2154

# Offline assembly tests: real packaging functions, isolated builder output,
# no Docker, package installation, mounts or kernel policy loading.
set -euo pipefail
export DEBUG=""
test_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../kata-deploy-binaries.sh
source "${test_dir}/../kata-deploy-binaries.sh"
extension_source="${repo_root_dir}/tools/packaging/apparmor-extension"
test_tmp="$(mktemp -d)"
trap 'rm -rf -- "${test_tmp:?}"' EXIT
real_parser="$(type -P apparmor_parser || true)"
checks=0
failed=0

get_kata_version() { printf 'test-version\n'; }
sudo() { "$@"; }
apparmor_parser() {
	if [[ -n "${real_parser}" ]]; then
		"${real_parser}" "$@"
	elif [[ " $* " == *' -N '* ]]; then
		awk '/^profile / { print $2 }' "${!#}"
	fi
}

fixture() {
	repo_root_dir="${test_tmp}/case-${checks}/repo"
	destdir="${test_tmp}/case-${checks}/dest"
	install_root="${destdir}/opt/kata/share/kata-containers"
	mkdir -p "${repo_root_dir}/tools/packaging/apparmor-extension" \
		"${repo_root_dir}/tools/osbuilder/image-builder"
	cp -a "${extension_source}/policy" "${extension_source}/test-policy" \
		"${repo_root_dir}/tools/packaging/apparmor-extension/"
	export MOCK_CAPTURE="${test_tmp}/case-${checks}/capture"
	export MOCK_IMAGE_MODE=normal
	unset APPARMOR_POLICY_DIR
	ARCH=x86_64
	cat > "${repo_root_dir}/tools/osbuilder/image-builder/image_builder.sh" <<'BUILDER'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == -o && $# -eq 3 ]]
[[ "${USE_DOCKER}" == 1 && "${FS_TYPE}" == erofs &&
   "${SKIP_DAX_HEADER}" == yes && "${SKIP_ROOTFS_CHECK}" == yes ]]
mkdir -p "${MOCK_CAPTURE}/${BUILD_VARIANT}"
cp -a "$3/." "${MOCK_CAPTURE}/${BUILD_VARIANT}/"
printf '%s\n' "$3" > "${MOCK_CAPTURE}/staged-root"
[[ "${MOCK_IMAGE_MODE}" != fail ]] || exit 42
[[ "${MOCK_IMAGE_MODE}" != no-image ]] || exit 0
printf '%s\n' "${BUILD_VARIANT}" > "$2"
if [[ "${MEASURED_ROOTFS}" == yes && "${MOCK_IMAGE_MODE}" != no-hash ]]; then
    hash_file="$(dirname -- "$2")/root_hash_${BUILD_VARIANT}.txt"
    if [[ "${MOCK_IMAGE_MODE}" == bad-hash ]]; then
        printf 'root_hash=broken\n' > "${hash_file}"
    else
        printf 'root_hash=%064d,salt=aa,data_blocks=1,data_block_size=4096,hash_block_size=4096\n' 0 > "${hash_file}"
    fi
fi
BUILDER
	chmod +x "${repo_root_dir}/tools/osbuilder/image-builder/image_builder.sh"
}

check() {
	local description="$1" expected="$2" result
	shift 2
	checks=$((checks + 1))
	set +e
	(set -e; fixture; "$@") > "${test_tmp}/stdout" 2> "${test_tmp}/stderr"
	result=$?
	set -e
	if find "${test_tmp}/case-${checks}/repo" -maxdepth 1 -name '.apparmor-extension-rootfs.*' | grep -q .; then
		printf 'not ok %s - staging directory leaked\n' "${checks}"
		failed=$((failed + 1))
	elif [[ "${expected}" == pass && "${result}" -eq 0 ]] ||
		{ [[ "${expected}" != pass && "${result}" -ne 0 ]] && grep -Fq "${expected}" "${test_tmp}/stderr"; }; then
		printf 'ok %s - %s\n' "${checks}" "${description}"
	else
		printf 'not ok %s - %s (exit %s)\n' "${checks}" "${description}" "${result}"
		cat "${test_tmp}/stdout" "${test_tmp}/stderr"
		failed=$((failed + 1))
	fi
}

production() {
	install_image_apparmor_extension
	local tree="${MOCK_CAPTURE}/apparmor-extension"
	[[ -f "${install_root}/kata-containers-apparmor-extension.img" ]]
	[[ -s "${install_root}/root_hash_apparmor-extension.txt" ]]
	[[ "$(find "${tree}/profiles" -type f -printf '%f\n')" == cri-containerd.apparmor.d ]]
	[[ ! -e "${tree}/profiles/kata-apparmor-test" ]]
	[[ ! -e "${tree}/apparmor_parser" && ! -e "${tree}/usr" && ! -e "${tree}/lib" ]]
	cmp "${extension_source}/policy/profiles/cri-containerd.apparmor.d" "${tree}/profiles/cri-containerd.apparmor.d"
	cmp "${extension_source}/policy/metadata/LICENSE.containerd" "${tree}/metadata/LICENSE.containerd"
	grep -Fx 'profiles=cri-containerd.apparmor.d ' "${tree}/metadata/manifest"
}

test_image() {
	production
	install_image_apparmor_test_extension
	local tree="${MOCK_CAPTURE}/apparmor-test-extension"
	[[ -f "${install_root}/kata-containers-apparmor-test-extension.img" ]]
	[[ -s "${install_root}/root_hash_apparmor-test-extension.txt" ]]
	[[ "$(find "${tree}/profiles" -type f | wc -l)" -eq 2 ]]
	cmp "${MOCK_CAPTURE}/apparmor-extension/profiles/cri-containerd.apparmor.d" "${tree}/profiles/cri-containerd.apparmor.d"
	cmp "${extension_source}/test-policy/profiles/kata-apparmor-test" "${tree}/profiles/kata-apparmor-test"
	grep -F 'deny /tmp/kata-apparmor-deny w,' "${tree}/profiles/kata-apparmor-test"
	grep -Fx 'name=apparmor' "${tree}/metadata/manifest"
	grep -Fx 'variant=apparmor-test-extension' "${tree}/metadata/manifest"
	[[ "$(cat "${install_root}/kata-containers-apparmor-extension.img")" == apparmor-extension ]]
}

custom_root() {
	custom="${test_tmp}/case-${checks}/custom policy"
	mkdir -p "${custom}/profiles" "${custom}/abstractions"
	printf '#include <abstractions/site>\nprofile site-policy { /tmp/** r, }\n' > "${custom}/profiles/site-policy"
	printf '# self-contained site policy\n' > "${custom}/abstractions/site"
}
custom_argument() {
	custom_root
	install_image_apparmor_extension "${custom}"
	cmp "${custom}/abstractions/site" "${MOCK_CAPTURE}/apparmor-extension/abstractions/site"
	[[ ! -e "${MOCK_CAPTURE}/apparmor-extension/profiles/kata-apparmor-test" ]]
}
custom_environment() { custom_root; APPARMOR_POLICY_DIR="${custom}"; install_image_apparmor_extension; }
custom_cli() {
	custom_root
	workdir="${test_tmp}/case-${checks}/cli"
	handle_build() { [[ "${APPARMOR_POLICY_DIR}" == "${custom}" ]]; }
	OPTIND=1
	main --build=rootfs-image-apparmor-extension "--apparmor-policy-dir=${custom}"
}
production_fixture() {
	cp "${extension_source}/test-policy/profiles/kata-apparmor-test" \
		"${repo_root_dir}/tools/packaging/apparmor-extension/policy/profiles/"
	install_image_apparmor_extension
}
test_collision() {
	cp "${extension_source}/test-policy/profiles/kata-apparmor-test" \
		"${repo_root_dir}/tools/packaging/apparmor-extension/policy/profiles/"
	install_image_apparmor_test_extension
}
builder_mode() { export MOCK_IMAGE_MODE="$1"; install_image_apparmor_extension; }
stale_output() {
	mkdir -p "${install_root}"
	printf 'old image\n' > "${install_root}/kata-containers-apparmor-extension.img"
	printf 'old hash\n' > "${install_root}/root_hash_apparmor-extension.txt"
	# Catch the child failure to verify that old artifacts were not accepted or changed.
	if (builder_mode no-image); then return 1; fi
	grep -Fx 'old image' "${install_root}/kata-containers-apparmor-extension.img"
	grep -Fx 'old hash' "${install_root}/root_hash_apparmor-extension.txt"
}
nonmeasured() { ARCH=s390x; install_image_apparmor_extension; [[ ! -e "${install_root}/root_hash_apparmor-extension.txt" ]]; }
reproducible_default() {
	python3 "${extension_source}/render-default-profile.py" > "${test_tmp}/rendered"
	cmp "${test_tmp}/rendered" "${extension_source}/policy/profiles/cri-containerd.apparmor.d"
	# Independent, fixed snapshot hashes detect unintended edits to the upstream baseline.
	(cd "${extension_source}"; sha256sum --check <<'HASHES'
f6db7bef8b260a9b397b5179539d2d2539b7a176787ffd0a8bfef57d9c8e9ff2  upstream/containerd-v2.2.1.template
4bbe3b885e8cd1907ab4cf9a41e862e74e24b5422297a4f2fe524e6a30ada2b4  upstream/LICENSE
HASHES
	)
	# The image carries the complete original license in policy comment form.
	sed -e 's/^# //' -e 's/^#$//' "${extension_source}/policy/metadata/LICENSE.containerd" | cmp - "${extension_source}/upstream/LICENSE"
	grep -Fx '@{PROC}=/proc/' "${test_tmp}/rendered"
	! grep -Eq '^\s*(#?include|abi)[[:space:]<"]|\{\{' "${test_tmp}/rendered"
}

check 'production has only the default profile and policy metadata' pass production
check 'test image has the same default plus the real fixture, with independent artifacts' pass test_image
check 'custom policy root argument preserves its include closure' pass custom_argument
check 'custom policy root environment entry remains available' pass custom_environment
check 'CLI forwards custom policy roots with spaces' pass custom_cli
check 'production rejects the reserved test fixture' 'must not contain kata-apparmor-test' production_fixture
check 'test overlay cannot overwrite an existing fixture' 'already contains the reserved test fixture' test_collision
check 'builder failure is fatal and cleans staging' 'building the AppArmor extension image failed' builder_mode fail
check 'successful builder must actually produce an image' 'did not produce' builder_mode no-image
check 'measured images require their own root hash' 'Missing AppArmor extension root hash' builder_mode no-hash
check 'malformed verity metadata is fatal' 'Invalid AppArmor extension verity parameters' builder_mode bad-hash
check 'stale installed image and hash cannot hide a missing builder output' pass stale_output
check 's390x keeps the existing nonmeasured extension convention' pass nonmeasured
check 'default is reproducible from the pinned template and has no external imports' pass reproducible_default
printf '%s checks; %s failures\n' "${checks}" "${failed}"
[[ "${failed}" -eq 0 ]]
