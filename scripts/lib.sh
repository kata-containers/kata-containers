function install_yq() {
	GOPATH=${GOPATH:-${HOME}/go}
	local yq_path="${GOPATH}/bin/yq"
	local yq_pkg="github.com/mikefarah/yq"
	[ -x  "${GOPATH}/bin/yq" ] && return

	case "$(arch)" in
	"aarch64")
		goarch=arm64
		;;

	"x86_64")
		goarch=amd64
		;;
	"*")
		echo "Arch $(arch) not supported"
		exit
		;;
	esac

	mkdir -p "${GOPATH}/bin"

	# Workaround to get latest release from github (to not use github token).
	# Get the redirection to latest release on github.
	yq_latest_url=$(curl -Ls -o /dev/null -w %{url_effective} "https://${yq_pkg}/releases/latest")
	# The redirected url should include the latest release version
	# https://github.com/mikefarah/yq/releases/tag/<VERSION-HERE>
	yq_version=$(basename "${yq_latest_url}")


	local yq_url="https://${yq_pkg}/releases/download/${yq_version}/yq_linux_${goarch}"
	curl -o "${yq_path}" -L ${yq_url}
	chmod +x ${yq_path}
}

function get_from_kata_deps(){
	dependency="$1"
	GOPATH=${GOPATH:-${HOME}/go}
	# This is needed in order to retrieve the version for qemu-lite
	install_yq >&2
	runtime_repo="github.com/kata-containers/runtime"
	runtime_repo_dir="$GOPATH/src/${runtime_repo}"
	versions_file="${runtime_repo_dir}/versions.yaml"
	mkdir -p "$(dirname ${runtime_repo_dir})"
	[ -d "${runtime_repo_dir}" ] ||  git clone --quiet https://${runtime_repo}.git "${runtime_repo_dir}"
	[ ! -f "$versions_file" ] && { echo >&2 "ERROR: cannot find $versions_file"; exit 1; }
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
