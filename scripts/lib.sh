export GOPATH=${GOPATH:-${HOME}/go}
readonly kata_arch_sh="${GOPATH}/src/github.com/kata-containers/tests/.ci/kata-arch.sh"
hub_bin="hub-bin"

get_kata_arch() {
	go get -u github.com/kata-containers/tests || true
	[ -f "${kata_arch_sh}" ] || die "Not found $kata_arch_sh"
}

install_yq() {
	GOPATH=${GOPATH:-${HOME}/go}
	local yq_path="${GOPATH}/bin/yq"
	local yq_pkg="github.com/mikefarah/yq"
	[ -x "${GOPATH}/bin/yq" ] && return

	get_kata_arch
	goarch=$("${kata_arch_sh}" -g)

	mkdir -p "${GOPATH}/bin"

	# Workaround to get latest release from github (to not use github token).
	# Get the redirection to latest release on github.
	yq_latest_url=$(curl -Ls -o /dev/null -w %{url_effective} "https://${yq_pkg}/releases/latest")
	# The redirected url should include the latest release version
	# https://github.com/mikefarah/yq/releases/tag/<VERSION-HERE>
	yq_version=$(basename "${yq_latest_url}")

	local yq_url="https://${yq_pkg}/releases/download/${yq_version}/yq_linux_${goarch}"
	curl -o "${yq_path}" -L "${yq_url}"
	chmod +x "${yq_path}"
}

get_from_kata_deps() {
	local dependency="$1"
	local branch="${2:-master}"
	local runtime_repo="github.com/kata-containers/runtime"
	GOPATH=${GOPATH:-${HOME}/go}
	# This is needed in order to retrieve the version for qemu-lite
	install_yq >&2
	yaml_url="https://raw.githubusercontent.com/kata-containers/runtime/${branch}/versions.yaml"
	versions_file="versions_${branch}.yaml"
	[ ! -e "${versions_file}" ] || download_on_new_flag="-z ${versions_file}"
	curl --silent -o "${versions_file}" ${download_on_new_flag:-} "$yaml_url"
	result=$("${GOPATH}/bin/yq" read "$versions_file" "$dependency")
	[ "$result" = "null" ] && result=""
	echo "$result"
}

die() {
	echo >&2 "ERROR: $*"
	exit 1
}

info() {
	echo >&2 "INFO: $*"
}

get_repo_hash() {
	local repo_dir=${1:-}
	[ -d "${repo_dir}" ] || die "${repo_dir} is not a directory"
	pushd "${repo_dir}" >>/dev/null
	git rev-parse --verify HEAD
	popd >>/dev/null
}

build_hub() {
	info "Get hub"

	if cmd=$(command -v hub); then
		hub_bin="${cmd}"
		return
	else
		hub_bin="${tmp_dir:-/tmp}/hub-bin"
	fi

	local hub_repo="github.com/github/hub"
	local hub_repo_dir="${GOPATH}/src/${hub_repo}"
	[ -d "${hub_repo_dir}" ] || git clone --quiet --depth 1 "https://${hub_repo}.git" "${hub_repo_dir}"
	pushd "${hub_repo_dir}" >>/dev/null
	git checkout master
	git pull
	./script/build -o "${hub_bin}"
	popd >>/dev/null
}

get_kata_hash_from_tag() {
	repo=$1
	git ls-remote --tags "https://github.com/${project}/${repo}.git" | grep "refs/tags/${kata_version}^{}" | awk '{print $1}'
}
