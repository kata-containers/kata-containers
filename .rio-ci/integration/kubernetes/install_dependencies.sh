#!/bin/bash
set -e

function install_bats() {
  BATS_REPO="https://github.com/bats-core/bats-core.git"
  echo "Install BATS from sources"
  rm -rf /bats
  mkdir -p /bats
  git clone ${BATS_REPO} /bats
  pushd /bats
  ./install.sh /usr
  popd
}

function install_pcl() {
  if ! command -v pcl &> /dev/null
  then
    export PCL_FLAVOR=${PCL_FLAVOR:-alpine}
    export PCL_VERSION=${PCL_VERSION:-0.15.0}
    echo "Install pcl"
    curl -o /bin/pcl https://artifacts.apple.com/libs-release/com/apple/pcl/pcl-cli-"${PCL_FLAVOR}"/"${PCL_VERSION}"/pcl-cli-"${PCL_FLAVOR}"-"${PCL_VERSION}".bin
    chmod +x /bin/pcl
  fi
}

function install_go() {
  local script_dir=$1

  echo "Install go"
  export GOROOT=/usr/local/go
  export GOPATH=$HOME/go
  export PATH=$PATH:/usr/local/go/bin
  "${script_dir}"/../../../ci/install_go.sh
}

function install_rust() {
  local script_dir=$1

  echo "Install rust"
  export PATH=$HOME/.cargo/bin:$PATH
  "${script_dir}"/../../../ci/install_rust.sh
}
