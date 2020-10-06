#!/bin/bash

# This is a helper library for the setup scripts of each package
# in this repository.

source_dir_pkg_lib=$(dirname "${BASH_SOURCE[0]}")
source_dir_pkg_lib=$(realpath "${source_dir_pkg_lib}")
source "${source_dir_pkg_lib}/../../scripts/lib.sh"

# Verify that versions.txt exists
version_file="${source_dir_pkg_lib}/../versions.txt"
if [ -f "${version_file}" ]; then
	source "${version_file}"
else
	die "${version_file} does not exist, you need to run first the gen_versions_txt.sh"
fi

PACKAGING_DIR=/var/packaging
LOG_DIR=${PACKAGING_DIR}/build_logs

# OBS Project info
OBS_PROJECT="${OBS_PROJECT:-katacontainers}"
OBS_SUBPROJECT="${OBS_SUBPROJECT:-alpha}"

# BUILD OPTIONS
BUILD_DISTROS=${BUILD_DISTROS:-Fedora_27 xUbuntu_16.04 CentOS_7}
BUILD_ARCH="$(uname -m)"

COMMIT=false
BRANCH=false
LOCAL_BUILD=false
OBS_PUSH=false
VERBOSE=false

arch_to_golang()
{
	local -r arch="$1"

	case "$arch" in
		aarch64) echo "arm64";;
		ppc64le) echo "$arch";;
		x86_64) echo "amd64";;
		s390x) echo "s390x";;
		*) die "unsupported architecture: $arch";;
	esac
}
# Used for debian.control files
# Architecture: The architecture specifies which type of hardware this
# package was compiled for.

short_commit_length=10

arch=$(uname -m)
DEB_ARCH=$(arch_to_golang "$arch")
if [[ $DEB_ARCH == "ppc64le" ]]; then
       DEB_ARCH="ppc64el"
fi

GO_ARCH=$(arch_to_golang "$arch")
export GO_ARCH

function display_help() {
	cat <<-EOL
		$SCRIPT_NAME

		This script is intended to create Kata Containers packages for the OBS
		(Open Build Service) platform.

		    Usage:
		        $SCRIPT_NAME [options]

		Options:

		    -l         --local-build     Build the runtime locally
		    -b         --branch          Build with a given branch name
		    -p         --push            Push changes to OBS
		    -a         --api-url         Especify an OBS API (e.g. custom private OBS)
		    -r         --obs-repository  An OBS repository to push the changes.
		    -w         --workdir         Directory of a working copy of the OBS runtime repo
		    -v         --verbose         Set the -x flag for verbosity
		    -C         --clean           Clean the repository
		    -V         --verify          Verify the environment
		    -h         --help            Display this help message

		    Usage examples:

		    $SCRIPT_NAME --local-build --branch staging
		    $SCRIPT_NAME --push --api-url http://127.0.0.1
		    $SCRIPT_NAME --push --obs-repository home:userx/repository
		    $SCRIPT_NAME --push

	EOL
	exit 1
}

die() {
	msg="$*"
	echo >&2 "ERROR: $msg"
	exit 1
}

info() {
	msg="$*"
	echo "INFO: $msg"
}

function verify() {
	# This function perform some checks in order to make sure
	# the script will run flawlessly.

	# Make sure this script is called from ./
	[ "$SCRIPT_DIR" != "." ] && die "The script must be called from its base dir."

	# Verify if osc is installed, exit otherwise.
	[ ! -x "$(command -v osc)" ] && die "osc is not installed."

	info "OK"
}

function clean() {
	# This function clean generated files
	for file in "$@"; do
		[ -e $file ] && rm -v $file
	done
	[ -e ./debian.changelog ] && git checkout ./debian.changelog
	[ -e ./release ] && git checkout ./release
	echo "Clean done."
}

function get_git_info() {
	AUTHOR=${AUTHOR:-$(git config user.name)}
	AUTHOR_EMAIL=${AUTHOR_EMAIL:-$(git config user.email)}
}

function set_versions() {
	local commit_hash="$1"
	hash_tag="$commit_hash"
	short_hashtag="${hash_tag:0:7}"
}

function changelog_update() {
	d=$(date -R)
	cat <<<"$PKG_NAME ($VERSION) stable; urgency=medium

  * Update $PKG_NAME $VERSION ${hash_tag:0:7}

 -- $AUTHOR <$AUTHOR_EMAIL>  $d
" >debian.changelog
	# Append, so it can be copied to the OBS repository
	GENERATED_FILES+=('debian.changelog')
}

function local_build() {
	[ ! -e $PACKAGING_DIR ] && mkdir $PACKAGING_DIR
	[ ! -e $LOG_DIR ] && mkdir $LOG_DIR

	pushd "${obs_repo_dir}"

	BUILD_ARGS=('--local-package' '--no-verify' '--noservice' '--trust-all-projects' '--keep-pkgs=/var/packaging/results')
	[ "$OFFLINE" == "true" ] && BUILD_ARGS+=('--offline')

	osc service run
	for distro in ${BUILD_DISTROS[@]}; do
		# If more distros are supported, add here the relevant validations.
		if [[ $distro =~ ^Fedora.* ]] || [[ $distro =~ ^CentOS.* ]]; then
			echo "Perform a local build for ${distro}"
			osc build ${BUILD_ARGS[@]} \
				${distro} $BUILD_ARCH *.spec | tee ${LOG_DIR}/${distro}_${PKG_NAME}_build.log

		elif [[ $distro =~ ^xUbuntu.* ]]; then
			echo "Perform a local build for ${distro}"
			osc build ${BUILD_ARGS[@]} \
				${distro} $BUILD_ARCH *.dsc | tee ${LOG_DIR}/${distro}_${PKG_NAME}_build.log
		fi
	done
	popd

}

function checkout_repo() {
	local repo="${1}"
	export obs_repo_dir="${repo}"

	mkdir -p "${obs_repo_dir}"
	osc co "${repo}" -o "${obs_repo_dir}"
	find "${obs_repo_dir}" -maxdepth 1 -mindepth 1 ! -name '.osc' -prune -exec echo remove {} \; -exec rm -rf {} \;

	mv "${GENERATED_FILES[@]}" "${obs_repo_dir}"
	cp "${STATIC_FILES[@]}" "$obs_repo_dir"
}

function obs_push() {
	pushd "${obs_repo_dir}"
	osc addremove
	osc commit -m "Update ${PKG_NAME} $VERSION: ${hash_tag:0:7}"
	popd
}

function cli() {
	OPTS=$(getopt -o abclprwvCVh: --long api-url,branch,commit-id,local-build,push,obs-repository,workdir,verbose,clean,verify,help -- "$@")
	while true; do
		case "${1}" in
		-b | --branch)
			BRANCH="true"
			OBS_REVISION="$2"
			shift 2
			;;
		-l | --local-build)
			LOCAL_BUILD="true"
			shift
			;;
		-p | --push)
			OBS_PUSH="true"
			shift
			;;
		-r | --obs-repository)
			PROJECT_REPO="$2"
			shift 2
			;;
		-v | --verbose)
			VERBOSE="true"
			shift
			;;
		-o | --offline)
			OFFLINE="true"
			shift
			;;
		-C | --clean)
			clean ${GENERATED_FILES[@]}
			exit $?
			;;
		-V | --verify)
			verify
			exit $?
			;;
		-h | --help)
			display_help
			exit $?
			;;
		--)
			shift
			break
			;;
		*) break ;;
		esac
	done

}

function build_pkg() {

	obs_repository="${1}"

	[ -z "${obs_repository}" ] && die "${FUNCNAME}: obs repository not provided"

	checkout_repo "${obs_repository}"

	if [ "$LOCAL_BUILD" == "true" ]; then
		info "Local build"
		local_build
	fi

	if [ "$OBS_PUSH" == "true" ]; then
		info "Push build to OBS"
		obs_push
	fi

}

function generate_files() {

	directory=$1
	replace_list=$2
	template_files=$(find $directory -type f -name "*-template")

	replace_list+=("deb_arch=$DEB_ARCH")

	#find_patches sets $RPM_PATCH_LIST and $RPM_PATCH_LIST
	# It also creates debian.series file
	find_patches
	replace_list+=("RPM_PATCH_LIST=$RPM_PATCH_LIST")
	replace_list+=("RPM_APPLY_PATCHES=$RPM_APPLY_PATCHES")

	# check replace list
	# key=val
	for replace in "${replace_list[@]}"; do
		[[ $replace == *"="* ]] || die "invalid replace $replace"
		local key="${replace%%=*}"
		local value="${replace##*=}"
		[ -n "$key" ] || die "${replace} key is empty"
		[ -n "$value" ] || die "${replace} val is empty"
		grep -q "@$key@" $template_files || die "@$key@ not found in any template file"
	done

	for f in ${template_files}; do
		genfile="${f%-template}"
		cp "$f" "${genfile}"
		info "Generate file ${genfile}"
		for replace in "${replace_list[@]}"; do
			[[ $replace == *"="* ]] || die "invalid replace $replace"
			local key="${replace%%=*}"
			local value="${replace##*=}"
			export k="@${key}@"
			export v="$value"
			perl -p -e 's/$ENV{k}/$ENV{v}/g' "${genfile}" >"${genfile}.out"
			mv "${genfile}.out" ${genfile}
		done
	done

}

function pkg_version() {
	local project_version="$1"
	# Used for
	# Release: in spec file
	# DebianRevisionNumber in dsc files
	local pkg_release="$2"
	local commit_id="$3"
	[ -n "${project_version}" ] || die "${FUNCNAME}: need version"

	pkg_version="${project_version}"

	if [ -n "$commit_id" ]; then
		pkg_version+="+git.${commit_id:0:${short_commit_length}}"
	fi
	if [ -n "$pkg_release" ]; then
		pkg_version+="-${pkg_release}"
	fi
	echo "$pkg_version"
}

function get_obs_pkg_release() {
	local obs_pkg_name="$1"
	local pkg
	local repo_dir
	local release=""

	pkg=$(basename "${obs_pkg_name}")
	repo_dir=$(mktemp -d -u -t "${pkg}.XXXXXXXXXXX")

	out=$(osc -v co "${obs_pkg_name}" -o "${repo_dir}") || die "failed to checkout:$out"

	spec_file=$(find "${repo_dir}" -maxdepth 1 -type f -name '*.spec' | head -1)
	# Find in specfile in Release: XX field.
	[ ! -f "${spec_file}" ] || release=$(grep -oP 'Release:\s+[0-9]+' "${spec_file}" | grep -oP '[0-9]+')

	if [ -z "${release}" ] && [ -f "${spec_file}" ] ; then
		# Not release number found find in "%define release XX"
		release=$(grep -oP '%define\s+release\s+[0-9]+' "${spec_file}" | grep -oP '[0-9]+')
	fi

	release_file=$(find "${repo_dir}" -maxdepth 1 -type f -name 'pkg-release')
	if [ -z "${release}" ] && [ -f "${release_file}" ]; then
		# Release still not found check pkg-release file
		release=$(grep -oP '[0-9]+' "${release_file}")
	fi
	if [ -z "${release}" ]; then
		# Not release number found, this is a new repository.
		release=1
	fi

	rm -r "${repo_dir}"
	echo "${release}"
}

#find_patches find patches in 'patches' directory.
# sets $RPM_PATCH_LIST and $RPM_PATCH_LIST
# RPM_PATCH_LIST fomat:
#  Patch<number>: patch.file
# RPM_APPLY_PATCHES fomat:
# %Patch<number> -p1
# It also creates debian.series file
function find_patches() {
	export RPM_PATCH_LIST="#Patches"$'\n'
	export RPM_APPLY_PATCHES="#Apply patches"$'\n'
	[ ! -d patches ] && info "No patches found" && return
	local patches
	patches=$(find patches/ -type f -name '*.patch' -exec basename {} \; | sort -t- -k1,1n)
	n="1"
	rm -f debian.series
	for p in ${patches}; do
		STATIC_FILES+=("patches/$p")
		RPM_PATCH_LIST+="Patch00${n}: $p"$'\n'
		RPM_APPLY_PATCHES+="%patch00${n} -p1"$'\n'
		echo "$p" >>debian.series
		((n++))
	done
	GENERATED_FILES+=(debian.series)
}
