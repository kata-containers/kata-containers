#!/bin/bash
set -e

function script-directory() {
  pushd . > /dev/null
  local script_path="${BASH_SOURCE[0]}"
  if [ -h "${script_path}" ]; then
    while [ -h "${script_path}" ]; do cd "$(dirname "${script_path}")";
    script_path=$(readlink "${script_path}"); done
  fi
  cd "$(dirname "${script_path}")" > /dev/null
  script_path=$(pwd);
  popd  > /dev/null
  echo "${script_path}"
}

function main() {
  script_dir=$(script-directory)
  source "${script_dir}"/util.sh
  trust-apple-corp-root-cas

  source "${script_dir}"/install_dependencies.sh
  install_bats
  install_pcl
  install_go "${script_dir}"
  install_rust "${script_dir}"

  export CLUSTER_NAME="${CLUSTER_NAME:-kata-ci}"
  export PRIORITY_CLASS="${PRIORITY_CLASS:-p1}"
  export GLOBAL_CONFIG=${GLOBAL_CONFIG:-/kubeconfig}

  ksmith --kubeconfig "${GLOBAL_CONFIG}" spawn kube --force --priority "${PRIORITY_CLASS}" --owner "$(whoami)" "${CLUSTER_NAME}" --apc-system latest

  CURRENT_DIR=$(pwd)

  mkdir -p "${HOME}/.kube"
  mkdir -p "${HOME}/.ssh"
  mv "${CURRENT_DIR}"/kubeconfig.yaml "${HOME}"/.kube/config
  mv "${CURRENT_DIR}"/sshconfig "${HOME}"/.ssh/config
}

main "$@"
