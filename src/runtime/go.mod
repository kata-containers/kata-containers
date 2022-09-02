module github.com/kata-containers/kata-containers/src/runtime

go 1.14

require (
	code.cloudfoundry.org/bytefmt v0.0.0-20211005130812-5bb3c17173e5
	github.com/BurntSushi/toml v1.2.0
	github.com/blang/semver v3.5.1+incompatible
	github.com/blang/semver/v4 v4.0.0
	github.com/containerd/cgroups v1.0.5-0.20220625035431-cf7417bca682
	github.com/containerd/console v1.0.3
	github.com/containerd/containerd v1.6.6
	github.com/containerd/cri-containerd v1.19.0
	github.com/containerd/fifo v1.0.0
	github.com/containerd/ttrpc v1.1.0
	github.com/containerd/typeurl v1.0.2
	github.com/containernetworking/plugins v1.1.1
	github.com/containers/podman/v4 v4.2.0
	github.com/coreos/go-systemd/v22 v22.3.2
	github.com/docker/go-units v0.4.0
	github.com/frankban/quicktest v1.13.1 // indirect
	github.com/fsnotify/fsnotify v1.5.4
	github.com/go-ini/ini v1.28.2
	github.com/go-openapi/errors v0.20.2
	github.com/go-openapi/runtime v0.19.21
	github.com/go-openapi/strfmt v0.21.1
	github.com/go-openapi/swag v0.21.1
	github.com/go-openapi/validate v0.22.0
	github.com/godbus/dbus/v5 v5.1.0
	github.com/gogo/protobuf v1.3.2
	github.com/hashicorp/go-multierror v1.1.1
	github.com/intel-go/cpuid v0.0.0-20210602155658-5747e5cec0d9
	github.com/mdlayher/vsock v1.1.0
	github.com/opencontainers/runc v1.1.3
	github.com/opencontainers/runtime-spec v1.0.3-0.20211214071223-8958f93039ab
	github.com/opencontainers/selinux v1.10.1
	github.com/pbnjay/memory v0.0.0-20210728143218-7b4eea64cf58
	github.com/pkg/errors v0.9.1
	github.com/prometheus/client_golang v1.12.1
	github.com/prometheus/client_model v0.2.0
	github.com/prometheus/common v0.32.1
	github.com/prometheus/procfs v0.7.3
	github.com/rogpeppe/go-internal v1.8.1-0.20210923151022-86f73c517451 // indirect
	github.com/safchain/ethtool v0.0.0-20210803160452-9aa261dae9b1
	github.com/sirupsen/logrus v1.9.0
	github.com/stretchr/testify v1.8.0
	github.com/urfave/cli v1.22.4
	github.com/vishvananda/netlink v1.1.1-0.20220115184804-dd687eb2f2d4
	github.com/vishvananda/netns v0.0.0-20210104183010-2eb08e3e575f
	gitlab.com/nvidia/cloud-native/go-nvlib v0.0.0-20220601114329-47893b162965
	go.opentelemetry.io/otel v1.3.0
	go.opentelemetry.io/otel/exporters/jaeger v1.0.0
	go.opentelemetry.io/otel/sdk v1.3.0
	go.opentelemetry.io/otel/trace v1.3.0
	golang.org/x/net v0.0.0-20220722155237-a158d28d115b
	golang.org/x/oauth2 v0.0.0-20220622183110-fd043fe589d2
	golang.org/x/sys v0.0.0-20220722155257-8c9f86f7a55f
	google.golang.org/grpc v1.47.0
	google.golang.org/protobuf v1.28.1
	k8s.io/apimachinery v0.22.5
	k8s.io/cri-api v0.23.1
)

replace (
	github.com/containerd/containerd => github.com/confidential-containers/containerd v1.6.7-0.20220619164525-4b12e77e79dc
	github.com/opencontainers/image-spec => github.com/opencontainers/image-spec v1.0.2
	github.com/opencontainers/runc => github.com/opencontainers/runc v1.0.3
	github.com/uber-go/atomic => go.uber.org/atomic v1.5.1
	google.golang.org/genproto => google.golang.org/genproto v0.0.0-20180817151627-c66870c02cf8
)
