module github.com/kata-containers/kata-containers/src/runtime

go 1.14

require (
	github.com/BurntSushi/toml v0.3.1
	github.com/blang/semver v3.5.1+incompatible
	github.com/blang/semver/v4 v4.0.0
	github.com/containerd/cgroups v1.0.1
	github.com/containerd/console v1.0.2
	github.com/containerd/containerd v1.5.7
	github.com/containerd/cri-containerd v1.11.1-0.20190125013620-4dd6735020f5
	github.com/containerd/fifo v1.0.0
	github.com/containerd/ttrpc v1.1.0
	github.com/containerd/typeurl v1.0.2
	github.com/containernetworking/plugins v0.9.1
	github.com/coreos/go-systemd/v22 v22.3.2
	github.com/cri-o/cri-o v1.0.0-rc2.0.20170928185954-3394b3b2d6af
	github.com/fsnotify/fsnotify v1.4.9
	github.com/go-ini/ini v1.28.2
	github.com/go-openapi/errors v0.18.0
	github.com/go-openapi/runtime v0.18.0
	github.com/go-openapi/strfmt v0.18.0
	github.com/go-openapi/swag v0.19.5
	github.com/go-openapi/validate v0.18.0
	github.com/godbus/dbus/v5 v5.0.4
	github.com/gogo/protobuf v1.3.2
	github.com/hashicorp/go-multierror v1.0.0
	github.com/intel-go/cpuid v0.0.0-20210602155658-5747e5cec0d9
	github.com/kata-containers/govmm v0.0.0-20210909155007-1b60b536f3c7
	github.com/mdlayher/vsock v0.0.0-20191108225356-d9c65923cb8f
	github.com/opencontainers/runc v1.0.2
	github.com/opencontainers/runtime-spec v1.0.3-0.20210326190908-1c3f411f0417
	github.com/opencontainers/selinux v1.8.2
	github.com/pkg/errors v0.9.1
	github.com/prometheus/client_golang v1.7.1
	github.com/prometheus/client_model v0.2.0
	github.com/prometheus/common v0.10.0
	github.com/prometheus/procfs v0.6.0
	github.com/safchain/ethtool v0.0.0-20190326074333-42ed695e3de8
	github.com/sirupsen/logrus v1.8.1
	github.com/smartystreets/goconvey v1.6.4 // indirect
	github.com/stretchr/testify v1.7.0
	github.com/urfave/cli v1.22.2
	github.com/vishvananda/netlink v1.1.1-0.20210924202909-187053b97868
	github.com/vishvananda/netns v0.0.0-20200728191858-db3c7e526aae
	go.opencensus.io v0.23.0 // indirect
	go.opentelemetry.io/otel v1.0.0
	go.opentelemetry.io/otel/exporters/jaeger v1.0.0
	go.opentelemetry.io/otel/sdk v1.0.0
	go.opentelemetry.io/otel/trace v1.0.0
	golang.org/x/net v0.0.0-20210226172049-e18ecbb05110
	golang.org/x/oauth2 v0.0.0-20210220000619-9bb904979d93
	golang.org/x/sys v0.0.0-20210426230700-d19ff857e887
	golang.org/x/text v0.3.5 // indirect
	google.golang.org/appengine v1.6.7 // indirect
	google.golang.org/genproto v0.0.0-20210303154014-9728d6b83eeb // indirect
	google.golang.org/grpc v1.36.0
	k8s.io/apimachinery v0.20.6
	k8s.io/cri-api v0.20.6
)

replace (
	github.com/containerd/containerd => github.com/containerd/containerd v1.5.8
	github.com/opencontainers/runc => github.com/opencontainers/runc v1.0.1
	github.com/uber-go/atomic => go.uber.org/atomic v1.5.1
	google.golang.org/genproto => google.golang.org/genproto v0.0.0-20180817151627-c66870c02cf8
)
