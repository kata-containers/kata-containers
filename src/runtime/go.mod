module github.com/kata-containers/kata-containers/src/runtime

go 1.14

require (
	github.com/BurntSushi/toml v0.3.1
	github.com/blang/semver v0.0.0-20190414102917-ba2c2ddd8906
	github.com/cilium/ebpf v0.0.0-20200421083123-d05ecd062fb1 // indirect
	github.com/containerd/cgroups v0.0.0-20190717030353-c4b9ac5c7601
	github.com/containerd/console v0.0.0-20191206165004-02ecf6a7291e
	github.com/containerd/containerd v1.2.1-0.20181210191522-f05672357f56
	github.com/containerd/continuity v0.0.0-20200413184840-d3ef23f19fbb // indirect
	github.com/containerd/cri v1.11.1 // indirect
	github.com/containerd/cri-containerd v1.11.1-0.20190125013620-4dd6735020f5
	github.com/containerd/fifo v0.0.0-20190226154929-a9fb20d87448
	github.com/containerd/go-runc v0.0.0-20200220073739-7016d3ce2328 // indirect
	github.com/containerd/ttrpc v1.0.0
	github.com/containerd/typeurl v1.0.1-0.20190228175220-2a93cfde8c20
	github.com/containernetworking/plugins v0.8.2
	github.com/cri-o/cri-o v1.0.0-rc2.0.20170928185954-3394b3b2d6af
	github.com/docker/distribution v2.7.1+incompatible // indirect
	github.com/docker/docker v1.13.1 // indirect
	github.com/docker/go-events v0.0.0-20190806004212-e31b211e4f1c // indirect
	github.com/go-ini/ini v1.28.2
	github.com/go-openapi/errors v0.18.0
	github.com/go-openapi/runtime v0.18.0
	github.com/go-openapi/strfmt v0.18.0
	github.com/go-openapi/swag v0.18.0
	github.com/go-openapi/validate v0.18.0
	github.com/gogo/googleapis v1.4.0 // indirect
	github.com/gogo/protobuf v1.3.1
	github.com/hashicorp/go-multierror v1.0.0
	github.com/intel-go/cpuid v0.0.0-20210602155658-5747e5cec0d9
	github.com/kata-containers/govmm v0.0.0-20210520142420-eb57f004d89f
	github.com/mdlayher/vsock v0.0.0-20191108225356-d9c65923cb8f
	github.com/opencontainers/image-spec v1.0.1 // indirect
	github.com/opencontainers/runc v1.0.0-rc9.0.20200102164712-2b52db75279c
	github.com/opencontainers/runtime-spec v1.0.2-0.20190408193819-a1b50f621a48
	github.com/opencontainers/selinux v1.4.0
	github.com/pkg/errors v0.9.1
	github.com/prometheus/client_golang v1.7.1
	github.com/prometheus/client_model v0.2.0
	github.com/prometheus/common v0.10.0
	github.com/prometheus/procfs v0.1.3
	github.com/safchain/ethtool v0.0.0-20190326074333-42ed695e3de8
	github.com/seccomp/libseccomp-golang v0.9.1 // indirect
	github.com/sirupsen/logrus v1.4.2
	github.com/smartystreets/goconvey v1.6.4 // indirect
	github.com/stretchr/testify v1.6.1
	github.com/syndtr/gocapability v0.0.0-20180916011248-d98352740cb2 // indirect
	github.com/urfave/cli v1.20.1-0.20170926034118-ac249472b7de
	github.com/vishvananda/netlink v1.0.1-0.20190604022042-c8c507c80ea2
	github.com/vishvananda/netns v0.0.0-20180720170159-13995c7128cc
	go.opentelemetry.io/otel v0.15.0
	go.opentelemetry.io/otel/exporters/trace/jaeger v0.15.0
	go.opentelemetry.io/otel/sdk v0.15.0
	golang.org/x/net v0.0.0-20200822124328-c89045814202
	golang.org/x/oauth2 v0.0.0-20200902213428-5d25da1a8d43
	golang.org/x/sys v0.0.0-20200905004654-be1d3432aa8f
	google.golang.org/grpc v1.31.1
	gotest.tools v2.2.0+incompatible // indirect
	k8s.io/apimachinery v0.18.2
)

replace (
	github.com/uber-go/atomic => go.uber.org/atomic v1.5.1
	google.golang.org/genproto => google.golang.org/genproto v0.0.0-20180817151627-c66870c02cf8
	google.golang.org/grpc => google.golang.org/grpc v1.19.0
	gotest.tools/v3 => gotest.tools v2.2.0+incompatible
)
