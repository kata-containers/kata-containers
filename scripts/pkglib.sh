#!/bin/bash

# This is a helper library for the setup scripts of each package
# in this repository.

source ../versions.txt
PACKAGING_DIR=/var/packaging
LOG_DIR=${PACKAGING_DIR}/build_logs

# OBS Project info
OBS_PROJECT="${OBS_PROJECT:-katacontainers}"
OBS_SUBPROJECT="${OBS_SUBPROJECT:-release}"

# BUILD OPTIONS
BUILD_DISTROS=${BUILD_DISTROS:-Fedora_27 xUbuntu_16.04 CentOS_7}
BUILD_ARCH=${BUILD_ARCH:-}x86_64

COMMIT=false
BRANCH=false
LOCAL_BUILD=false
OBS_PUSH=false
VERBOSE=false

# Used for debian.control files
# Architecture: The architecture specifies which type of hardware this
# package was compiled for.
DEB_ARCH="${DEB_ARCH:-amd64}"

if command -v go; then
	export GO_ARCH=$(go env GOARCH)
else
	export GO_ARCH=amd64
	echo "Go not installed using $GO_ARCH to install go in dockerfile"
fi

function display_help()
{
	cat <<-EOL 
	$SCRIPT_NAME

	This script is intended to create Kata Containers 3.X packages for the OBS 
	(Open Build Service) platform.

    Usage:
        $SCRIPT_NAME [options]

	Options:

    -l         --local-build     Build the runtime locally
    -c         --commit-id       Build with a given commit ID
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
    $SCRIPT_NAME --commit-id a76f45c --push --api-url http://127.0.0.1
    $SCRIPT_NAME --commit-id a76f45c --push --obs-repository home:userx/repository
    $SCRIPT_NAME --commit-id a76f45c --push

	EOL
	exit 1
}

die()
{
	msg="$*"
	echo >&2 "ERROR: $msg"
	exit 1
}

info()
{
	msg="$*"
	echo "INFO: $msg"
}

function verify()
{
    # This function perform some checks in order to make sure
    # the script will run flawlessly.

    # Make sure this script is called from ./
    [ "$SCRIPT_DIR" != "." ] && die "The script must be called from its base dir."
    
    # Verify if osc is installed, exit otherwise.
    [ ! -x "$(command -v osc)" ] && die "osc is not installed."

    info "OK"
}

function clean()
{
    # This function clean generated files
    for file in "$@"
    do
        [ -e $file ] && rm -v $file
    done
    [ -e ./debian.changelog ] && git checkout ./debian.changelog
    [ -e ./release ] && git checkout ./release
    echo "Clean done."
}

function get_git_info()
{
    AUTHOR=${AUTHOR:-$(git config user.name)}
    AUTHOR_EMAIL=${AUTHOR_EMAIL:-$(git config user.email)}
}

function set_versions()
{
    local commit_hash="$1"

    if [ -n "$OBS_REVISION" ]
    then
	# Validate input is alphanumeric, commit ID
	# If a commit ID is provided, override versions.txt one
	if [ -n "$COMMIT" ] && [[ "$OBS_REVISION" =~ ^[a-zA-Z0-9][-a-zA-Z0-9]{0,40}[a-zA-Z0-9]$  ]]; then
            hash_tag=$OBS_REVISION
	elif [ -n "$BRANCH" ]
	then
            hash_tag=$commit_hash
	fi
    else
        hash_tag=$commit_hash
    fi
    short_hashtag="${hash_tag:0:7}"	
}

function changelog_update {
    d=$(date -R)
    cat <<< "$PKG_NAME ($VERSION) stable; urgency=medium

  * Update $PKG_NAME $VERSION ${hash_tag:0:7}

 -- $AUTHOR <$AUTHOR_EMAIL>  $d
" > debian.changelog
	# Append, so it can be copied to the OBS repository
	GENERATED_FILES+=('debian.changelog')
}

function local_build()
{
    [ ! -e $PACKAGING_DIR ] && mkdir $PACKAGING_DIR
    [ ! -e $LOG_DIR ] && mkdir $LOG_DIR

    pushd $OBS_WORKDIR

    BUILD_ARGS=('--local-package' '--no-verify' '--noservice' '--trust-all-projects' '--keep-pkgs=/var/packaging/results')
    [ "$OFFLINE" == "true" ] && BUILD_ARGS+=('--offline')

    osc service run
    for distro in ${BUILD_DISTROS[@]}
    do
        # If more distros are supported, add here the relevant validations.
        if [[ "$distro" =~ ^Fedora.* ]] || [[ "$distro" =~ ^CentOS.* ]]
        then
	    echo "Perform a local build for ${distro}"
	    osc build ${BUILD_ARGS[@]} \
                ${distro} $BUILD_ARCH *.spec | tee ${LOG_DIR}/${distro}_${PKG_NAME}_build.log

        elif [[ "$distro" =~ ^xUbuntu.* ]]
        then
	    echo "Perform a local build for ${distro}"
	    osc build ${BUILD_ARGS[@]} \
		${distro} $BUILD_ARCH *.dsc | tee ${LOG_DIR}/${distro}_${PKG_NAME}_build.log
        fi
    done
}

function checkout_repo()
{
    local REPO="$1"
    if [ -z "$OBS_WORKDIR" ]
    then
        # If no workdir is provided, use a temporary directory.
        temp=$(basename $0)
        OBS_WORKDIR=$(mktemp -d -u -t ${temp}.XXXXXXXXXXX) || exit 1
        osc $APIURL co $REPO -o $OBS_WORKDIR
    fi

    mv ${GENERATED_FILES[@]} $OBS_WORKDIR
    cp ${STATIC_FILES[@]} $OBS_WORKDIR
}


function obs_push()
{
    pushd $OBS_WORKDIR
    osc $APIURL addremove
    osc $APIURL commit -m "Update ${PKG_NAME} $VERSION: ${hash_tag:0:7}"
    popd
}

function cli()
{
	OPTS=$(getopt -o abclprwvCVh: --long api-url,branch,commit-id,local-build,push,obs-repository,workdir,verbose,clean,verify,help -- "$@")
	while true; do
		case "${1}" in
			-a | --api-url )        APIURL="$2"; shift 2;;
			-b | --branch )         BRANCH="true"; OBS_REVISION="$2"; shift 2;;
			-c | --commit-id )      COMMIT="true"; OBS_REVISION="$2"; shift 2;;
			-l | --local-build )    LOCAL_BUILD="true"; shift;;
			-p | --push )           OBS_PUSH="true"; shift;;
			-r | --obs-repository ) PROJECT_REPO="$2"; shift 2;;
			-w | --workdir )        OBS_WORKDIR="$2"; shift 2;;
			-v | --verbose )        VERBOSE="true"; shift;;
			-o | --offline )        OFFLINE="true"; shift;;
			-C | --clean )          clean ${GENERATED_FILES[@]}; exit $?;;
			-V | --verify )         verify; exit $?;;
			-h | --help )           display_help; exit $?;;
			-- )               shift; break ;;
			* )                break ;;
		esac
	done

}

function build_pkg()
{

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

function generate_files () {

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
	for replace in "${replace_list[@]}" ; do
		[[ "$replace" = *"="* ]] || die "invalid replace $replace"
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
		for replace in "${replace_list[@]}" ; do
			[[ "$replace" = *"="* ]] || die "invalid replace $replace"
			local key="${replace%%=*}"
			local value="${replace##*=}"
			export k="@${key}@"
			export v="$value"
			perl -p -e 's/$ENV{k}/$ENV{v}/g' "${genfile}" > "${genfile}.out"
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
	[ -n "${pkg_release}" ] || die "${FUNCNAME}: pkg release is needed"

	pkg_version="${project_version}"

	if [ -n "$commit_id" ]; then
		pkg_version+="+git.${commit_id:0:7}"
	fi
	echo "$pkg_version-${pkg_release}"
}

function get_obs_pkg_release() {
	local obs_pkg_name="$1"
	local pkg
	local repo_dir
	local release

	pkg=$(basename "${obs_pkg_name}")
	repo_dir=$(mktemp -d -u -t "${pkg}.XXXXXXXXXXX")

	out=$(osc ${APIURL} -q co "${obs_pkg_name}" -o "${repo_dir}") || die "failed to checkout:$out"

	spec_file=$(find "${repo_dir}" -maxdepth 1 -type f -name '*.spec' | head -1)
	release=$(grep -oP  'Release:\s+[0-9]+' "${spec_file}"  | grep -oP '[0-9]+')

	if [ -z "${release}" ]; then
		release=$(grep -oP  '%define\s+release\s+[0-9]+' "${spec_file}"  | grep -oP '[0-9]+')
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
	patches=$(find patches -type f -name '*.patch' -exec basename {} \;)
	n="1"
	rm -f debian.series
	for p in ${patches} ; do
		STATIC_FILES+=("patches/$p")
		RPM_PATCH_LIST+="Patch00${n}: $p"$'\n'
		RPM_APPLY_PATCHES+="%patch00${n} -p1"$'\n'
		echo "$p" >> debian.series
		((n++))
	done
	GENERATED_FILES+=(debian.series)
}
