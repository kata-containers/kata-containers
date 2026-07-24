module github.com/kata-containers/kata-containers/src/tools/kata-annotation-webhook

go 1.24.12

require (
	github.com/sirupsen/logrus v1.9.3
	github.com/slok/kubewebhook/v2 v2.3.0
	k8s.io/api v0.23.5
	k8s.io/apimachinery v0.33.0
)

require (
	github.com/fxamacker/cbor/v2 v2.7.0 // indirect
	github.com/x448/float16 v0.8.4 // indirect
	golang.org/x/net v0.43.0 // indirect
	golang.org/x/sys v0.35.0 // indirect
	sigs.k8s.io/randfill v1.0.0 // indirect
)

require (
	github.com/go-logr/logr v1.4.3 // indirect
	github.com/gogo/protobuf v1.3.2 // indirect
	github.com/json-iterator/go v1.1.12 // indirect
	github.com/kata-containers/kata-containers/src/runtime v0.0.0-20220325211203-a07956a369ab
	github.com/modern-go/concurrent v0.0.0-20180306012644-bacd9c7ef1dd // indirect
	github.com/modern-go/reflect2 v1.0.2 // indirect
	golang.org/x/text v0.28.0 // indirect
	gomodules.xyz/jsonpatch/v3 v3.0.1 // indirect
	gomodules.xyz/orderedmap v0.1.0 // indirect
	gopkg.in/inf.v0 v0.9.1 // indirect
	k8s.io/client-go v0.23.5 // indirect
	k8s.io/klog/v2 v2.130.1 // indirect
	k8s.io/utils v0.0.0-20241104100929-3ea5e8cea738 // indirect
	sigs.k8s.io/json v0.0.0-20241010143419-9aa6b5e7a4b3 // indirect
	sigs.k8s.io/structured-merge-diff/v4 v4.6.0 // indirect
	sigs.k8s.io/yaml v1.4.0 // indirect
)

replace github.com/kata-containers/kata-containers/src/runtime => ../../runtime
