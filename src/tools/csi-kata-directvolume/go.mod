module kata-containers/csi-kata-directvolume

// Keep in sync with version in versions.yaml
go 1.25.7

// WARNING: Do NOT use `replace` directives as those break dependabot:
// https://github.com/kata-containers/kata-containers/issues/11020

require (
	github.com/container-storage-interface/spec v1.9.0
	github.com/diskfs/go-diskfs v1.4.0
	github.com/golang/glog v1.2.4
	github.com/golang/protobuf v1.5.4
	github.com/kubernetes-csi/csi-lib-utils v0.16.0
	github.com/pborman/uuid v1.2.1
	github.com/stretchr/testify v1.8.4
	golang.org/x/net v0.38.0
	golang.org/x/sys v0.31.0
	google.golang.org/grpc v1.63.2
	k8s.io/apimachinery v0.28.2
	k8s.io/klog/v2 v2.110.1
	k8s.io/mount-utils v0.28.2
	k8s.io/utils v0.0.0-20231127182322-b307cd553661
)

require (
	github.com/davecgh/go-spew v1.1.1 // indirect
	github.com/elliotwutingfeng/asciiset v0.0.0-20230602022725-51bbb787efab // indirect
	github.com/go-logr/logr v1.3.0 // indirect
	github.com/gogo/protobuf v1.3.2 // indirect
	github.com/google/uuid v1.6.0 // indirect
	github.com/kr/text v0.2.0 // indirect
	github.com/moby/sys/mountinfo v0.6.2 // indirect
	github.com/pierrec/lz4/v4 v4.1.17 // indirect
	github.com/pkg/xattr v0.4.9 // indirect
	github.com/pmezard/go-difflib v1.0.0 // indirect
	github.com/sirupsen/logrus v1.9.3 // indirect
	github.com/ulikunitz/xz v0.5.15 // indirect
	golang.org/x/text v0.23.0 // indirect
	google.golang.org/genproto/googleapis/rpc v0.0.0-20240227224415-6ceb2ff114de // indirect
	google.golang.org/protobuf v1.33.0 // indirect
	gopkg.in/djherbis/times.v1 v1.3.0 // indirect
	gopkg.in/inf.v0 v0.9.1 // indirect
	gopkg.in/yaml.v3 v3.0.1 // indirect
)

// WARNING: Do NOT use `replace` directives as those break dependabot:
// https://github.com/kata-containers/kata-containers/issues/11020
