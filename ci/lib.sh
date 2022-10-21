#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o nounset

export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="$GOPATH/src/$tests_repo"
export branch="${target_branch:-main}"

# Clones the tests repository and checkout to the branch pointed out by
# the global $branch variable.
# If the clone exists and `CI` is exported then it does nothing. Otherwise
# it will clone the repository or `git pull` the latest code.
#
clone_tests_repo()
{
	if [ -d "$tests_repo_dir" ]; then
		[ -n "${CI:-}" ] && return
		# git config --global --add safe.directory will always append
		# the target to .gitconfig without checking the existence of
		# the target, so it's better to check it before adding the target repo.
		local sd="$(git config --global --get safe.directory ${tests_repo_dir} || true)"
		if [ -z "${sd}" ]; then
			git config --global --add safe.directory ${tests_repo_dir}
		fi
		pushd "${tests_repo_dir}"
		git checkout "${branch}"
		git pull
		popd
	else
		git clone -q "https://${tests_repo}" "$tests_repo_dir"
		pushd "${tests_repo_dir}"
		git checkout "${branch}"
		popd
	fi
}

run_static_checks()
{
	clone_tests_repo
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a
	bash "$tests_repo_dir/.ci/static-checks.sh" "$@"
}

run_docs_url_alive_check()
{
	clone_tests_repo
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a
	bash "$tests_repo_dir/.ci/static-checks.sh" --docs --all "github.com/kata-containers/kata-containers"
}

run_get_pr_changed_file_details()
{
	clone_tests_repo
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a
	source "$tests_repo_dir/.ci/lib.sh"
	get_pr_changed_file_details
}

# branch: the target branch, the same as ${{github.base_ref}}
apply_depends_on() {
	pushd "${GOPATH}/src/github.com/kata-containers/kata-containers"
	label_lines=$(git log --format=%b "origin/${branch}.." | grep "Depends-on:" || true)
	if [ "${label_lines}" == "" ]; then
		popd
		return 0
	fi

	nb_lines=$(echo "${label_lines}" | wc -l)

	repos_found=()
	for i in $(seq 1 "${nb_lines}")
	do
		label_line=$(echo "${label_lines}" | sed "${i}q;d")
		label_str=$(echo "${label_line}" | cut -d ':' -f2)
		repo=$(echo "${label_str}" | tr -d '[:space:]' | cut -d'#' -f1)
		if [[ "${repos_found[@]}" =~ "${repo}" ]]; then
			echo "Repository $repo was already defined in a 'Depends-on:' tag."
			echo "Only one repository per tag is allowed."
			return 1
		fi
		repos_found+=("$repo")
		pr_id=$(echo "${label_str}" | cut -d'#' -f2)

		echo "This PR depends on repository: ${repo} and pull request: ${pr_id}"
		if [ ! -d "${GOPATH}/src/${repo}" ]; then
			go get -d "$repo" || true
		fi

		pushd "${GOPATH}/src/${repo}"
		git config user.email "you@example.com"
		git config user.name "Your Name"
		echo "Fetching pull request: ${pr_id} for repository: ${repo}"
		dependency_branch="p${pr_id}"
		git fetch origin "pull/${pr_id}/head:${dependency_branch}" && \
			git checkout "${dependency_branch}" && \
			git merge "origin/${branch}"
			# And show what we merged on top of to aid debugging
			git log --oneline "origin/${branch}~1..HEAD"
		popd
	done

	popd
}
