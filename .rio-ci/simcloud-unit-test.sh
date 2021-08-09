#!/bin/bash -e

PREFIX=/kata-containers

rm -rf ${PREFIX}/simcloud
mkdir -p ${PREFIX}/simcloud
cd ${PREFIX}/simcloud || exit 1

cat >run-test.sh <<EOT
#!/bin/bash

cd ${PREFIX}

export GOROOT=/usr/local/go
export GOPATH=\$HOME/go
export PATH=\$PATH:/usr/local/go/bin
export PATH=\$HOME/.cargo/bin:\$PATH

./ci/install_go.sh
./ci/install_rust.sh

make test SECCOMP=no

echo \$? >test_result
echo "Result of test: \$(cat test_result)"

if [[ "\$(cat test_result)" -ne "0" ]]; then
  mkdir -p "${PREFIX}/logs"
fi

EOT

chmod +x run-test.sh

source ${PREFIX}/.rio-ci/simcloud-run.sh

simcloud_execute
