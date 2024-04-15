module github.com/kata-containers/kata-containers/src/runtime

go 1.19

require (
	code.cloudfoundry.org/bytefmt v0.0.0-20211005130812-5bb3c17173e5
	github.com/BurntSushi/toml v1.2.1
	github.com/blang/semver v3.5.1+incompatible
	github.com/blang/semver/v4 v4.0.0
	github.com/container-orchestrated-devices/container-device-interface v0.6.0
	github.com/containerd/cgroups v1.1.0
	github.com/containerd/console v1.0.3
	github.com/containerd/containerd v1.7.2
	github.com/containerd/cri-containerd v1.19.0
	github.com/containerd/fifo v1.1.0
	github.com/containerd/ttrpc v1.2.2
	github.com/containerd/typeurl/v2 v2.1.1
	github.com/containernetworking/plugins v1.2.0
	github.com/containers/podman/v4 v4.2.0
	github.com/coreos/go-systemd/v22 v22.5.0
	github.com/docker/go-units v0.5.0
	github.com/fsnotify/fsnotify v1.6.0
	github.com/go-ini/ini v1.28.2
	github.com/go-openapi/errors v0.20.2
	github.com/go-openapi/runtime v0.19.21
	github.com/go-openapi/strfmt v0.21.1
	github.com/go-openapi/swag v0.21.1
	github.com/go-openapi/validate v0.22.0
	github.com/godbus/dbus/v5 v5.1.0
	github.com/hashicorp/go-multierror v1.1.1
	github.com/intel-go/cpuid v0.0.0-20210602155658-5747e5cec0d9
	github.com/mdlayher/vsock v1.1.0
	github.com/opencontainers/runc v1.1.12
	github.com/opencontainers/runtime-spec v1.1.0-rc.1
	github.com/opencontainers/selinux v1.11.0
	github.com/pbnjay/memory v0.0.0-20210728143218-7b4eea64cf58
	github.com/pkg/errors v0.9.1
	github.com/prometheus/client_golang v1.14.0
	github.com/prometheus/client_model v0.3.0
	github.com/prometheus/common v0.37.0
	github.com/prometheus/procfs v0.8.0
	github.com/safchain/ethtool v0.2.0
	github.com/sirupsen/logrus v1.9.0
	github.com/stretchr/testify v1.8.2
	github.com/urfave/cli v1.22.12
	github.com/vishvananda/netlink v1.2.1-beta.2
	github.com/vishvananda/netns v0.0.0-20210104183010-2eb08e3e575f
	gitlab.com/nvidia/cloud-native/go-nvlib v0.0.0-20220601114329-47893b162965
	go.opentelemetry.io/otel v1.14.0
	go.opentelemetry.io/otel/exporters/jaeger v1.0.0
	go.opentelemetry.io/otel/sdk v1.14.0
	go.opentelemetry.io/otel/trace v1.14.0
	golang.org/x/oauth2 v0.4.0
	golang.org/x/sys v0.19.0
	google.golang.org/grpc v1.53.0
	google.golang.org/protobuf v1.29.1
	k8s.io/apimachinery v0.26.2
	k8s.io/cri-api v0.27.1
)

require (
	github.com/AdaLogics/go-fuzz-headers v0.0.0-20230811130428-ced1acdcaa24 // indirect
	github.com/AdamKorcz/go-118-fuzz-build v0.0.0-20230306123547-8075edf89bb0 // indirect
	github.com/Microsoft/go-winio v0.6.1 // indirect
	github.com/Microsoft/hcsshim v0.10.0-rc.8 // indirect
	github.com/PuerkitoBio/purell v1.1.1 // indirect
	github.com/PuerkitoBio/urlesc v0.0.0-20170810143723-de5bf2ad4578 // indirect
	github.com/asaskevich/govalidator v0.0.0-20200907205600-7a23bdc65eef // indirect
	github.com/beorn7/perks v1.0.1 // indirect
	github.com/cespare/xxhash/v2 v2.2.0 // indirect
	github.com/cilium/ebpf v0.9.1 // indirect
	github.com/containerd/cgroups/v3 v3.0.2 // indirect
	github.com/containerd/continuity v0.4.2-0.20230616210509-1e0d26eb2381 // indirect
	github.com/containerd/go-runc v1.0.0 // indirect
	github.com/containernetworking/cni v1.1.2 // indirect
	github.com/cpuguy83/go-md2man/v2 v2.0.2 // indirect
	github.com/cyphar/filepath-securejoin v0.2.3 // indirect
	github.com/davecgh/go-spew v1.1.1 // indirect
	github.com/docker/go-events v0.0.0-20190806004212-e31b211e4f1c // indirect
	github.com/go-logr/logr v1.2.3 // indirect
	github.com/go-logr/stdr v1.2.2 // indirect
	github.com/go-openapi/analysis v0.21.2 // indirect
	github.com/go-openapi/jsonpointer v0.19.5 // indirect
	github.com/go-openapi/jsonreference v0.19.6 // indirect
	github.com/go-openapi/loads v0.21.1 // indirect
	github.com/go-openapi/spec v0.20.4 // indirect
	github.com/go-stack/stack v1.8.0 // indirect
	github.com/gogo/protobuf v1.3.2 // indirect
	github.com/golang/groupcache v0.0.0-20210331224755-41bb18bfe9da // indirect
	github.com/golang/protobuf v1.5.3 // indirect
	github.com/google/go-cmp v0.5.9 // indirect
	github.com/google/uuid v1.3.0 // indirect
	github.com/hashicorp/errwrap v1.1.0 // indirect
	github.com/josharian/intern v1.0.0 // indirect
	github.com/klauspost/compress v1.16.0 // indirect
	github.com/mailru/easyjson v0.7.6 // indirect
	github.com/matttproud/golang_protobuf_extensions v1.0.4 // indirect
	github.com/mdlayher/socket v0.2.0 // indirect
	github.com/mitchellh/mapstructure v1.5.0 // indirect
	github.com/moby/locker v1.0.1 // indirect
	github.com/moby/sys/mountinfo v0.6.2 // indirect
	github.com/moby/sys/sequential v0.5.0 // indirect
	github.com/moby/sys/signal v0.7.0 // indirect
	github.com/moby/sys/symlink v0.2.0 // indirect
	github.com/oklog/ulid v1.3.1 // indirect
	github.com/opencontainers/go-digest v1.0.0 // indirect
	github.com/opencontainers/image-spec v1.1.0-rc2.0.20221005185240-3a7f492d3f1b // indirect
	github.com/opencontainers/runtime-tools v0.9.1-0.20221107090550-2e043c6bd626 // indirect
	github.com/pmezard/go-difflib v1.0.0 // indirect
	github.com/rogpeppe/go-internal v1.8.1-0.20210923151022-86f73c517451 // indirect
	github.com/russross/blackfriday/v2 v2.1.0 // indirect
	github.com/syndtr/gocapability v0.0.0-20200815063812-42c35b437635 // indirect
	go.mongodb.org/mongo-driver v1.7.5 // indirect
	go.opencensus.io v0.24.0 // indirect
	golang.org/x/mod v0.9.0 // indirect
	golang.org/x/net v0.24.0 // indirect
	golang.org/x/sync v0.1.0 // indirect
	golang.org/x/text v0.14.0 // indirect
	golang.org/x/tools v0.7.0 // indirect
	google.golang.org/appengine v1.6.7 // indirect
	google.golang.org/genproto v0.0.0-20230306155012-7f2fa6fef1f4 // indirect
	gopkg.in/inf.v0 v0.9.1 // indirect
	gopkg.in/yaml.v2 v2.4.0 // indirect
	gopkg.in/yaml.v3 v3.0.1 // indirect
	sigs.k8s.io/yaml v1.3.0 // indirect
)

replace (
	github.com/go-openapi/swag => github.com/go-openapi/swag v0.21.1
	github.com/opencontainers/runc => github.com/opencontainers/runc v1.1.9
	github.com/stretchr/testify => github.com/stretchr/testify v1.8.0
	github.com/uber-go/atomic => go.uber.org/atomic v1.5.1
	golang.org/x/text => golang.org/x/text v0.7.0
	google.golang.org/grpc => google.golang.org/grpc v1.47.0
	gopkg.in/yaml.v3 => gopkg.in/yaml.v3 v3.0.1
)
