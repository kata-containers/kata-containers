#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

# The go binary isn't installed, but we checkout the repos to the standard
# golang locations.
export GOPATH=${GOPATH:-${HOME}/go}

typeset -r script_name="${0##*/}"
typeset -r script_dir="$(cd "$(dirname "${0}")" && pwd)"

typeset -r docker_image="busybox"
typeset -r kata_project_url="github.com/kata-containers"
typeset -r test_repo="${kata_project_url}/tests"
typeset -r test_repo_url="https://${test_repo}"
typeset -r test_repo_dir="${GOPATH}/src/${test_repo}"
typeset -r kata_project_dir="${GOPATH}/src/${kata_project_url}"

typeset -r mgr="${test_repo_dir}/cmd/kata-manager/kata-manager.sh"
typeset -r doc_to_script="${test_repo_dir}/.ci/kata-doc-to-script.sh"

die()
{
	local msg="$*"
	echo >&2 "ERROR: $msg"
	exit 1
}

info()
{
	local msg="$*"
	echo "INFO: $msg"
}

usage()
{
	cat <<EOT
Description: Run Kata documentation CI tests.

Usage: $script_name [options]

Options:

 -h        : Show this help.
 -t <dir>  : Run all scripts ("\*.sh" files) in the specified
             directory.

Notes:
  - The '-t' option is not generally useful - it is used by this
    script which re-exec's itself with this option.

EOT
}

# Re-execute the running script from a temporary directory to allow the
# script to continue executing even if the original source file is deleted.
reexec_in_tmpdir()
{
	local -r test_dir="$1"

	[ -d "${test_dir}" ] || die "invalid test dir: ${test_dir}"

	if [ "${script_dir}" = "${test_dir}" ]
	then
		# Already running from temp directory so nothing else to do
		return
	fi

	local new
	new="${test_dir}/${script_name}"

	install --mode 750 "${0}" "${new}"

	info "Re-execing ${0} as ${new}"

	cd "${test_dir}"

	exec "${new}" -t "${test_dir}/tests"
}

# Grab a copy of the tests repository
get_tests_repo()
{
	[ -d "${test_repo_dir}" ] && return

	mkdir -p "${kata_project_dir}"

	git clone "${test_repo_url}" "${test_repo_dir}"
}

# Delete all local github repo clones.
#
# This is required to ensure that the tests themselves (re-)create these
# clones.
delete_kata_repos()
{
	[ -n "${KATA_DEV_MODE}" ] && die "Not continuing as this is a dev system"
	[ -z "${CI}" ] && die "Not continuing as this is a non-CI environment"

	local cwd="$PWD"

	info "Deleting all local kata repositories below ${kata_project_dir}"

	[ -d "${kata_project_dir}" ] && rm -rf "${kata_project_dir}" || true

	# Recreate the empty directory, taking care to handle the scenario
	# where the script is run from within the just-deleted directory.
	mkdir -p "$cwd" && cd "$cwd"
}

setup()
{
	source /etc/os-release || source /usr/lib/os-release

	mkdir -p "${GOPATH}"

	get_tests_repo

	[ -e "$mgr" ] || die "cannot find $mgr"
}

# Perform a simple test to create a container
create_kata_container()
{
	local -r test_name="$1"

	local -r msg=$(info "Successfully tested ${test_name} on distro ${ID} ${VERSION}")

	# Perform a basic test
	sudo -E docker run --rm -i --runtime "kata-runtime" "${docker_image}" echo "$msg"
}

# Run the kata manager to "execute" the install guide to ensure the commands
# it specified result in a working system.
test_distro_install_guide()
{
	info "Installing system from the $ID install guide"

	$mgr install-docker-system

	$mgr configure-image
	$mgr enable-debug

	local mgr_name="${mgr##*/}"

	local test_name="${mgr_name} to test install guide"

	info "Install using ${test_name}"

	create_kata_container "${test_name}"

	# Clean up
	$mgr remove-packages
}

# Apart from the distro-specific install guides, users can choose to install
# using one of the following methods:
#
# - kata-manager ("Automatic" method).
# - kata-doc-to-script ("Scripted" method).
#
# Testing these is awkward because we need to "execute" the documents
# describing those install methods, but since those install methods should
# themselves entirely document/handle an installation method, we need to
# convert each install document to a script, then delete all the kata code
# repositories. This ensures that when each install method script is run, it
# does not rely on any local files (it should download anything it needs). But
# since we're deleting the repos, we need to copy this script to a temporary
# location, along with the install scripts this function generates, and then
# re-exec this script with an option to ask it to run the scripts the previous
# instance of this script just generated.
test_alternative_install_methods()
{
	local -a files
	files+=("installing-with-kata-manager.md")
	files+=("installing-with-kata-doc-to-script.md")

	local tmp_dir

	tmp_dir=$(mktemp -d)

	local script_file

	local file

	local tests_dir
	tests_dir="${tmp_dir}/tests"

	mkdir -p "${tests_dir}"

	local -i num=0

	# Convert the docs to scripts
	for file in "${files[@]}"
	do
		num+=1

		local file_path
		local script_file
		local script_file_path
		local test_name

		file_path="${script_dir}/../install/${file}"
		script_file=${file/.md/.sh}

		# Add a numeric prefix so the tests are run in the array order 
		test_name=$(printf "%.2d-%s" "${num}" "${script_file}")

		script_file_path="${tests_dir}/${test_name}"

		info "Creating test script ${test_name} from ${file}"

		bash "${doc_to_script}" "${file_path}" "${script_file_path}"
	done

	reexec_in_tmpdir "${tmp_dir}"

	# Not reached
	die "re-exec failed"
}

run_tests()
{
	test_distro_install_guide
	test_alternative_install_methods
}

# Detect if any installation documents changed. If so, execute all the
# documents to test they result in a working system.
check_install_docs()
{
	if [ -n "$TRAVIS" ]
	then
		info "Not testing install guide as Travis lacks modern distro support and VT-x"
		return
	fi

	# List of filters used to restrict the types of file changes.
	# See git-diff-tree(1) for further info.
	local filters=""

	# Added file
	filters+="A"

	# Copied file
	filters+="C"

	# Modified file
	filters+="M"

	# Renamed file
	filters+="R"

	# Unmerged (U) and Unknown (X) files. These particular filters
	# shouldn't be necessary but just in case...
	filters+="UX"

	# List of changed files
	local files=$(git diff-tree \
		--name-only \
		--no-commit-id \
		--diff-filter="${filters}" \
		-r \
		origin/master HEAD || true)

	# No files were changed
	[ -z "$files" ] && return

	changed=$(echo "${files}" | grep "^install/.*\.md$" || true)

	[ -z "$changed" ] && info "No install documents modified" && return

	info "Found modified install documents: ${changed}"

	# Regardless of which installation documents were changed, we test
	# them all where possible.
	run_tests
}

# Run the test scripts in the specified directory.
run_tests_from_dir()
{
	local -r test_dir="$1"

	[ -e "$test_dir" ] || die "invalid test dir: ${test_dir}"

	cd "${test_dir}"

	info "Looking for tests scripts to run in directory ${test_dir}"

	for t in $(ls -- *.sh)
	do
		# Ensure the test script cannot access any local files
		# (since it should be standalone and download any files
		# it needs).
		delete_kata_repos

		info "Running test script '$t'"
		bash -x "${t}"

		# Ensure it is possible to use the installed system
		create_kata_container "${t}"

		# Re-run setup to recreate the tests repo that was deleted
		# before the test ran.
		setup

		# Packaged install so clean up
		# (Note that '$mgr' should now exist again)
		$mgr remove-packages
	done

	# paranoia
	[ -d "${test_dir}" ] && rm -rf "${test_dir}"

	info "All tests passed"
}

main()
{
	local opt
	local test_dir

	setup

	while getopts "ht:" opt
	do
		case "$opt" in
			h) usage; exit 0;;
			t) test_dir="$OPTARG";;
			*) die "invalid option: $opt";;
		esac
	done

	if [ -n "$test_dir" ]
	then
		run_tests_from_dir "$test_dir"
		exit 0
	fi

	check_install_docs
}

main "$@"
