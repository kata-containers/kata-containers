module kata-containers/csi-kata-directvolume

go 1.20

require (
	github.com/container-storage-interface/spec v1.9.0
	github.com/diskfs/go-diskfs v1.4.0
	github.com/golang/glog v1.2.4
	github.com/golang/protobuf v1.5.4
	github.com/kubernetes-csi/csi-lib-utils v0.16.0
	github.com/pborman/uuid v1.2.1
	github.com/stretchr/testify v1.8.4
	golang.org/x/net v0.33.0
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
	github.com/sirupsen/logrus v1.9.0 // indirect
	github.com/ulikunitz/xz v0.5.11 // indirect
	golang.org/x/sys v0.28.0 // indirect
	golang.org/x/text v0.21.0 // indirect
	google.golang.org/genproto/googleapis/rpc v0.0.0-20240227224415-6ceb2ff114de // indirect
	google.golang.org/protobuf v1.33.0 // indirect
	gopkg.in/djherbis/times.v1 v1.3.0 // indirect
	gopkg.in/inf.v0 v0.9.1 // indirect
	gopkg.in/yaml.v3 v3.0.1 // indirect
)

replace k8s.io/api => k8s.io/api v0.28.2

replace k8s.io/apiextensions-apiserver => k8s.io/apiextensions-apiserver v0.28.2

replace k8s.io/apimachinery => k8s.io/apimachinery v0.28.2

replace k8s.io/apiserver => k8s.io/apiserver v0.28.2

replace k8s.io/cli-runtime => k8s.io/cli-runtime v0.28.2

replace k8s.io/client-go => k8s.io/client-go v0.28.2

replace k8s.io/cloud-provider => k8s.io/cloud-provider v0.28.2

replace k8s.io/cluster-bootstrap => k8s.io/cluster-bootstrap v0.28.2

replace k8s.io/code-generator => k8s.io/code-generator v0.28.2

replace k8s.io/component-base => k8s.io/component-base v0.28.2

replace k8s.io/component-helpers => k8s.io/component-helpers v0.28.2

replace k8s.io/controller-manager => k8s.io/controller-manager v0.28.2

replace k8s.io/cri-api => k8s.io/cri-api v0.28.2

replace k8s.io/csi-translation-lib => k8s.io/csi-translation-lib v0.28.2

replace k8s.io/dynamic-resource-allocation => k8s.io/dynamic-resource-allocation v0.28.2

replace k8s.io/kms => k8s.io/kms v0.28.2

replace k8s.io/kube-aggregator => k8s.io/kube-aggregator v0.28.2

replace k8s.io/kube-controller-manager => k8s.io/kube-controller-manager v0.28.2

replace k8s.io/kube-proxy => k8s.io/kube-proxy v0.28.2

replace k8s.io/kube-scheduler => k8s.io/kube-scheduler v0.28.2

replace k8s.io/kubectl => k8s.io/kubectl v0.28.2

replace k8s.io/kubelet => k8s.io/kubelet v0.28.2

replace k8s.io/legacy-cloud-providers => k8s.io/legacy-cloud-providers v0.28.2

replace k8s.io/metrics => k8s.io/metrics v0.28.2

replace k8s.io/mount-utils => k8s.io/mount-utils v0.28.2

replace k8s.io/pod-security-admission => k8s.io/pod-security-admission v0.28.2

replace k8s.io/sample-apiserver => k8s.io/sample-apiserver v0.28.2

replace k8s.io/endpointslice => k8s.io/endpointslice v0.28.2
