#!/usr/bin/env bash
# Copyright (c) 2026 Kata Containers contributors
# SPDX-License-Identifier: Apache-2.0
# shellcheck source-path=SCRIPTDIR
# Globals such as repo_root_dir come from the packaging library.
# shellcheck disable=SC2154

# No Docker daemon or kernel policy loading is used. A locally installed
# parser additionally checks the shipped profile and real include semantics.
set -euo pipefail
test_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export DEBUG=""
# shellcheck source=../kata-deploy-binaries.sh
source "${test_dir}/../kata-deploy-binaries.sh"
test_tmp="$(mktemp -d)"
trap 'rm -rf -- "${test_tmp:?}"' EXIT
unset AGENT_CONTAINER_BUILDER
real_parser="$(type -P apparmor_parser || true)"
case_number=0
failed=0

get_agent_image_name() { printf '%s\n' unit-test-builder; }
get_from_kata_deps() { printf '%s\n' unit-test-rust; }

command() {
	if [[ "${1:-}" == -v ]]; then
		case "${2:-}" in
			apparmor_parser) [[ "${mock_mode}" == host ]] ; return ;;
			docker) [[ "${mock_mode}" == docker ]] ; return ;;
		esac
	fi
	builtin command "$@"
}

mock_parser() {
	local include_root="$1" names=no arg
	shift
	printf 'parser %q ' "$@" >> "${trace}"
	printf '\n' >> "${trace}"
	[[ "$1" == --config-file=/dev/null ]] || return 91
	shift
	[[ "$1" == -Q && "$2" == -K && "$3" == -I && "$4" == "${include_root}" &&
		"$5" == -b && "$6" == "${include_root}" ]] || return 92
	for arg in "$@"; do [[ "${arg}" != -N ]] || names=yes; done
	[[ "${!#}" == "${include_root}/profiles/unit-test" ]] || return 93
	[[ "${mock_warning}" != yes ]] || printf '%s\n' 'parser warning on stderr' >&2
	[[ "${mock_compile_failure}" != yes || "${names}" == yes ]] || return 94
	[[ "${names}" != yes ]] || printf '%s\n' "${mock_names}"
}

apparmor_parser() {
	if [[ "${mock_real_parser}" == yes ]]; then
		"${real_parser}" "$@"
	else
		mock_parser "${policy_root}" "$@"
	fi
}

docker() {
	printf 'docker %q ' "$@" >> "${trace}"
	printf '\n' >> "${trace}"
	if [[ "$1" == build ]]; then
		[[ "$2" == --build-arg && "$3" == RUST_TOOLCHAIN=unit-test-rust &&
			"$4" == -t && "$5" == unit-test-builder && "$6" == "${static_build_dir}/agent" ]] || return 95
		[[ "${mock_build_failure}" != yes ]] || return 96
		[[ "${mock_builder_broken}" == yes ]] || : > "${builder_ready}"
		return 0
	fi
	[[ "$1" == run && "$2" == --rm ]] || return 97
	shift 2
	if [[ "$1" == --entrypoint && "$2" == /bin/sh ]]; then
		[[ "$3" == unit-test-builder && "$4" == -c &&
			"$5" == 'command -v apparmor_parser >/dev/null' ]] || return 98
		[[ -f "${builder_ready}" ]]
	else
		[[ "$1" == -v && "$2" == "${policy_root}:/policy:ro" &&
			"$3" == --entrypoint && "$4" == apparmor_parser && "$5" == unit-test-builder ]] || return 99
		shift 5
		mock_parser /policy "$@"
	fi
}

fixture() {
	policy_root="${test_tmp}/case-${case_number}/policy"
	mkdir -p "${policy_root}/profiles"
	printf 'profile unit-test { /tmp/** rw, }\n' > "${policy_root}/profiles/unit-test"
	trace="${test_tmp}/case-${case_number}/trace"
	builder_ready="${test_tmp}/case-${case_number}/builder-ready"
	: > "${trace}"
	mock_mode=host
	mock_warning=no
	mock_names=unit-test
	mock_compile_failure=no
	mock_build_failure=no
	mock_builder_broken=no
	mock_real_parser=no
}

check() {
	local description="$1" expected="$2" result
	shift 2
	case_number=$((case_number + 1))
	# The tested function must run outside an if/! context, otherwise Bash
	# disables errexit inside it and can hide failing assertions.
	set +e
	(set -e; fixture; "$@") > "${test_tmp}/stdout" 2> "${test_tmp}/stderr"
	result=$?
	set -e
	if [[ "${expected}" == pass && "${result}" -eq 0 ]] ||
		{ [[ "${expected}" != pass && "${result}" -ne 0 ]] &&
			grep -Fq "${expected}" "${test_tmp}/stderr"; }; then
		printf 'ok %s - %s\n' "${case_number}" "${description}"
	else
		printf 'not ok %s - %s (exit %s)\n' "${case_number}" "${description}" "${result}"
		cat "${test_tmp}/stdout" "${test_tmp}/stderr"
		failed=$((failed + 1))
	fi
}

validate() { validate_apparmor_profiles "${policy_root}"; }
warnings() { mock_warning=yes; validate; grep -q -- '-N' "${trace}"; }
wrong_name() { mock_names=wrong-name; validate; }
multiple_names() { mock_names=$'unit-test\nsecond'; validate; }
no_names() { mock_names=; validate; }
compile_failure() { mock_compile_failure=yes; validate; }
empty_profiles() { rm "${policy_root}/profiles/unit-test"; validate; }
invalid_name() { mv "${policy_root}/profiles/unit-test" "${policy_root}/profiles/bad name"; validate; }
reserved_name() { mv "${policy_root}/profiles/unit-test" "${policy_root}/profiles/unconfined"; validate; }
nested_profiles() { mkdir "${policy_root}/profiles/nested"; validate; }
include_line() { printf '%s\nprofile unit-test {}\n' "$1" > "${policy_root}/profiles/unit-test"; validate; }
closed_includes() {
	mkdir -p "${policy_root}/abstractions" "${policy_root}/tunables"
	printf '#include <tunables/common>\n' > "${policy_root}/abstractions/common"
	printf '# local tunables\n' > "${policy_root}/tunables/common"
	include_line '#include <abstractions/common>'
}
quoted_include() {
	mkdir "${policy_root}/abstractions"
	printf '# local rules\n' > "${policy_root}/abstractions/common"
	include_line 'include "abstractions/common"'
}
indirect_escape() {
	mkdir "${policy_root}/abstractions"
	printf '#include <../outside>\n' > "${policy_root}/abstractions/common"
	include_line '#include <abstractions/common>'
}
unused_escape() { printf '#include </etc/passwd>\n' > "${policy_root}/unused"; validate; }
ordinary_comments() {
	# These are literal policy comments, not shell variable references.
	# shellcheck disable=SC2016
	printf '%s\n' \
		'# include <../a-rule-comment>' \
		'# This ordinary comment mentions include </etc/passwd> and abi <$HOST>.' \
		'profile unit-test {' \
		' /tmp/** rw, # include </etc/passwd>' \
		' "/tmp/include text" r, # an ordinary comment with #include </etc/passwd>' \
		'}' > "${policy_root}/profiles/unit-test"
	validate
}
symlink_entry() { ln -s "$1" "${policy_root}/$2"; validate; }
symlink_root() { ln -s "${policy_root}" "${policy_root}-link"; validate_apparmor_profiles "${policy_root}-link"; }
symlink_parent() {
	ln -s "${policy_root%/policy}" "${policy_root%/policy}-link"
	validate_apparmor_profiles "${policy_root%/policy}-link/policy"
}
special_file() { mkfifo "${policy_root}/fifo"; validate; }
non_utf8() { printf '\377' > "${policy_root}/unused"; validate; }
device_boundary() {
	# Invoked indirectly by the validator in this fixture's subshell.
	# shellcheck disable=SC2317
	stat() {
		if [[ "${!#}" == "${policy_root}/profiles" ]]; then printf '%s\n' another-device;
		else builtin command stat "$@"; fi
	}
	validate
}
docker_existing() {
	mock_mode=docker
	: > "${builder_ready}"
	validate
	! grep -q 'docker build' "${trace}"
}
docker_rebuild() {
	mock_mode=docker
	validate
	grep -q 'docker build' "${trace}"
	grep -Eq '^[[:space:]]+apparmor[[:space:]]' "${static_build_dir}/agent/Dockerfile"
}
docker_warnings() { mock_warning=yes; docker_existing; }
docker_bad_build() { mock_mode=docker; mock_build_failure=yes; validate; }
docker_no_parser() { mock_mode=docker; mock_builder_broken=yes; validate; }
no_parser() { mock_mode=none; validate; }
real_profile() {
	mock_real_parser=yes
	cp "${repo_root_dir}/tools/packaging/apparmor-extension/policy/profiles/kata-apparmor-test" "${policy_root}/profiles/"
	validate
}
real_includes() { mock_real_parser=yes; closed_includes; }
real_quoted() { mock_real_parser=yes; closed_includes; quoted_include_after; }
quoted_include_after() { include_line 'include "abstractions/common"'; }
real_optional() { mock_real_parser=yes; include_line '#include if exists <tunables/global>'; }
real_comments() { mock_real_parser=yes; ordinary_comments; }
real_optional_whitespace() { mock_real_parser=yes; include_line $'#include\tif \t exists\t<optional/missing>'; }
real_invalid_rules() {
	mock_real_parser=yes
	printf 'profile unit-test { /tmp/** invalid-permissions, }\n' > "${policy_root}/profiles/unit-test"
	validate
}

check 'stderr warnings do not count as profile names' pass warnings
check 'profile filename must match its declaration' 'declares itself' wrong_name
check 'multiple declared profiles are rejected' 'expected exactly 1' multiple_names
check 'empty name output is rejected' 'expected exactly 1' no_names
check 'full compilation runs before name inspection' 'failed to compile' compile_failure
check 'an empty profiles directory is rejected' 'policy tree is empty' empty_profiles
check 'invalid filenames are rejected' 'Invalid AppArmor profile file' invalid_name
check 'reserved profile names are rejected' 'Invalid AppArmor profile file' reserved_name
check 'profiles must be flat regular files' 'Invalid AppArmor profile file' nested_profiles
check 'nested includes are rooted in the full policy tree' pass closed_includes
check 'quoted relative includes are accepted' pass quoted_include
check 'missing optional root-relative include is accepted' pass include_line '#include if exists <optional/missing>'
check 'required includes cannot fall back to the host distro' 'not shipped in the policy root' include_line '#include <tunables/global>'
check 'absolute includes are rejected' 'unsafe AppArmor include' include_line '#include </etc/passwd>'
check 'parent traversal is rejected' 'unsafe AppArmor include' include_line '#include <../outside>'
check 'optional includes cannot escape' 'unsafe AppArmor include' include_line 'include if exists "../outside"'
check 'include variables are rejected' 'unsafe AppArmor include' include_line '#include <@{HOST}/outside>'
for path_character in '$' '*' '?' '[' ']'; do
	check "include paths reject literal-unsafe ${path_character}" 'unsafe AppArmor include' \
		include_line "#include <abstractions/${path_character}>"
	check "ABI paths reject literal-unsafe ${path_character}" 'unsafe AppArmor include' \
		include_line "abi <abi/${path_character}>,"
done
# The unsafe variable must reach the policy validator literally.
# shellcheck disable=SC2016
check 'optional include paths must also be literal' 'unsafe AppArmor include' \
	include_line 'include if exists "abstractions/$HOST"'
check 'if exists permits variable whitespace' pass include_line $'#include\tif \t exists\t<optional/missing>'
check 'rule comments and ordinary comments do not become directives' pass ordinary_comments
check 'inline include directives are rejected' 'unsafe AppArmor include' \
	include_line 'profile unit-test { include <abstractions/common>'
check 'inline #include directives are rejected' 'unsafe AppArmor include' \
	include_line 'profile unit-test { #include </etc/passwd>'
check 'inline ABI directives are rejected' 'unsafe AppArmor include' \
	include_line 'profile unit-test { abi <abi/features>,'
check 'continued include directives are rejected' 'unsafe AppArmor include' include_line $'#include <abstractions/\\\ncommon>'
check 'indirect include escapes are rejected' 'unsafe AppArmor include' indirect_escape
check 'unused policy files also participate in closure checks' 'unsafe AppArmor include' unused_escape
check 'absolute ABI paths cannot escape' 'unsafe AppArmor include' include_line 'abi </etc/passwd>,'
check 'profile symlinks are rejected' 'symlink' symlink_entry profiles/unit-test profiles/linked
check 'directory symlinks are rejected' 'symlink' symlink_entry profiles abstractions
check 'dangling symlinks are rejected' 'symlink' symlink_entry missing dangling
check 'a symlink policy root is rejected' 'must not be a symlink' symlink_root
check 'symlink ancestors of the policy root are rejected' 'must not contain symlinks' symlink_parent
check 'special files are rejected' 'special file' special_file
check 'all policy files must be UTF-8' 'not UTF-8 text' non_utf8
check 'cross-device policy entries are rejected' 'device boundary' device_boundary
check 'an existing Docker parser is probed without a rebuild' pass docker_existing
check 'Docker parser warnings also stay out of names' pass docker_warnings
check 'a builder without a parser is rebuilt and re-probed' pass docker_rebuild
check 'failed builder construction is fatal' 'Failed to build AppArmor parser image' docker_bad_build
check 'a builder still lacking a parser is fatal' 'Agent builder has no apparmor_parser' docker_no_parser
check 'absence of both parser and Docker is fatal' 'no apparmor_parser on the host' no_parser
if [[ -n "${real_parser}" ]]; then
	check 'the shipped profile compiles with the real parser' pass real_profile
	check 'real parser resolves angle includes from root' pass real_includes
	check 'real parser also checks quoted include resolution' 'failed to compile' real_quoted
	check 'real parser does not use host tunables for optional includes' pass real_optional
	check 'real parser accepts variable whitespace in if exists' pass real_optional_whitespace
	check 'real parser treats # include and ordinary comments as comments' pass real_comments
	check 'real parser rejects invalid policy rules' 'failed to compile' real_invalid_rules
else
	printf '# SKIP real parser checks: apparmor_parser is not installed\n'
fi
printf '%s checks; %s failures\n' "${case_number}" "${failed}"
[[ "${failed}" -eq 0 ]]
