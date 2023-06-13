FROM mcr.microsoft.com/cbl-mariner/base/core:2.0
RUN tdnf install -y git build-essential protobuf-compiler protobuf-devel expect curl dnf openssl-devel clang-devel
RUN tdnf install -y libseccomp-devel parted qemu-img golang btrfs-progs-devel device-mapper-devel cmake
RUN tdnf install -y ca-certificates
RUN tdnf install -y rust

RUN git clone --branch cc-msft-prototypes https://github.com/microsoft/kata-containers.git
WORKDIR kata-containers/src/tools/genpolicy
RUN cargo build

RUN RUST_LOG=info target/debug/genpolicy -u < ../../agent/samples/policy/yaml/kubernetes/conformance/netexecrc.yaml > tmp.yaml
RUN mv tmp.yaml ../../agent/samples/policy/yaml/kubernetes/conformance/netexecrc.yaml
RUN git status
